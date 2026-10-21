//! Asking a question about the reference tray.
//!
//! The prompt is the only place where product judgement lives in this file: excerpts are labelled
//! with their page, the model is told to cite them, and it is told not to invent pages it was not
//! given. Everything else is transport.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::StreamExt;
use pdfscope_ai::{ChatRequest, Image, Message, Protocol, ProviderConfig, StreamEvent};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// One excerpt the reader attached.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AskReference {
    /// One-based page number, as the reader sees it.
    pub(crate) page: u32,
    /// The quoted words.
    pub(crate) quote: String,
}

/// One captured region the reader attached.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RegionInput {
    /// One-based page number.
    pub page: u32,
    /// A `data:image/png;base64,…` URL.
    pub image: String,
}

/// One earlier exchange, for resolving references like "this" and "it".
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct HistoryTurn {
    /// What the reader asked.
    pub question: String,
    /// What the model answered.
    pub answer: String,
}

/// Extracted text for one page, used for ranges and for the current-page default.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PageContext {
    /// One-based page number.
    pub page: u32,
    /// The page's text, possibly empty for a scan.
    pub text: String,
}

/// Shared cancellation for the running answer.
#[derive(Default)]
pub(crate) struct AskControl {
    stop: Arc<AtomicBool>,
}

/// What the panel receives while an answer streams.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum AskEvent {
    /// A reasoning token, kept out of the answer.
    Reasoning {
        /// The token text.
        text: String,
    },
    /// A visible answer token.
    Text {
        /// The token text.
        text: String,
    },
    /// Totals reported once.
    Usage {
        /// Prompt tokens.
        prompt: u32,
        /// Generated tokens.
        completion: u32,
        /// Prompt tokens served from cache.
        cached: u32,
    },
    /// The answer was stopped by the reader.
    Stopped,
    /// The answer is complete.
    Done,
    /// The request failed.
    Error {
        /// A message fit to show the reader.
        message: String,
    },
}

/// The most images one request may carry.
pub(crate) const MAX_IMAGES: usize = 6;
/// The most base64 one image may carry (about 3 MB of PNG).
pub(crate) const MAX_BASE64: usize = 4 * 1024 * 1024;

/// Builds the request. Pure, so the prompt can be tested without a network.
pub(crate) fn build_request(
    question: &str,
    references: &[AskReference],
    regions: &[RegionInput],
    context: &[PageContext],
    history: &[HistoryTurn],
    system: &str,
) -> Result<ChatRequest, String> {
    if regions.len() > MAX_IMAGES {
        return Err(format!(
            "一次最多附 {MAX_IMAGES} 张截图，现在有 {} 张",
            regions.len()
        ));
    }

    let mut prompt = String::new();

    if !history.is_empty() {
        prompt.push_str("之前的对话（只用于理解指代，不要重复引用里面的页码）：\n");
        for turn in history {
            prompt.push_str(&format!(
                "问：{}\n答：{}\n\n",
                turn.question.trim(),
                turn.answer.trim()
            ));
        }
    }

    if references.is_empty() && regions.is_empty() && context.is_empty() {
        prompt.push_str("（读者没有提供摘录或截图。请基于一般知识回答，不要编造页码。）\n\n");
    }

    for page in context {
        if page.text.trim().is_empty() {
            prompt.push_str(&format!(
                "[第 {} 页]\n（这一页没有可提取的文字，可能是扫描件。）\n\n",
                page.page
            ));
        } else {
            prompt.push_str(&format!("[第 {} 页]\n{}\n\n", page.page, page.text.trim()));
        }
    }

    for reference in references {
        prompt.push_str(&format!(
            "[摘录 p.{}]\n{}\n\n",
            reference.page,
            reference.quote.trim()
        ));
    }

    for region in regions {
        prompt.push_str(&format!("[截图 p.{}，见附带的图片]\n\n", region.page));
    }

    prompt.push_str("问题：");
    prompt.push_str(question.trim());

    let mut images = Vec::with_capacity(regions.len());
    for region in regions {
        if region.image.len() > MAX_BASE64 {
            return Err(format!("p.{} 的截图太大，请框小一点", region.page));
        }
        let image = Image::from_data_url(&region.image)
            .ok_or_else(|| format!("p.{} 的截图格式无法识别", region.page))?;
        images.push(image);
    }

    Ok(ChatRequest {
        max_tokens: Some(2048),
        system: Some(system.to_owned()),
        messages: vec![Message::user_with_images(prompt, images)],
        temperature: None,
    })
}

/// Starts a streamed answer and reports it through `ask:event`.
#[tauri::command]
pub(crate) async fn ask(
    app: tauri::AppHandle,
    question: String,
    references: Vec<AskReference>,
    regions: Vec<RegionInput>,
    context: Vec<PageContext>,
    history: Vec<HistoryTurn>,
    control: tauri::State<'_, AskControl>,
) -> Result<(), String> {
    if question.trim().is_empty() {
        return Err("问题不能为空".to_owned());
    }

    let (settings, key) = crate::settings::provider(&app)?;
    let protocol = if settings.protocol == "anthropic" {
        Protocol::Anthropic
    } else {
        Protocol::OpenAi
    };

    let config = ProviderConfig::new(
        settings.preset,
        protocol,
        settings.base_url,
        settings.model,
        key,
    )
    .map_err(|error| error.to_string())?;

    // Four turns is enough to resolve pronouns without spending the context on history.
    let history: Vec<HistoryTurn> = history
        .into_iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let mut request = build_request(
        &question,
        &references,
        &regions,
        &context,
        &history,
        &settings.system_prompt,
    )?;
    request.max_tokens = settings.max_tokens;
    let stop = control.stop.clone();
    stop.store(false, Ordering::SeqCst);

    tauri::async_runtime::spawn(async move {
        let emitter = app.clone();
        let send = move |event: AskEvent| {
            let _ = emitter.emit("ask:event", event);
        };

        let mut events = match pdfscope_ai::stream(&config, &request).await {
            Ok(events) => events,
            Err(error) => {
                send(AskEvent::Error {
                    message: error.to_string(),
                });
                return;
            }
        };

        while let Some(event) = events.next().await {
            if stop.load(Ordering::SeqCst) {
                send(AskEvent::Stopped);
                return;
            }

            match event {
                Ok(StreamEvent::Reasoning(text)) => send(AskEvent::Reasoning { text }),
                Ok(StreamEvent::Text(text)) => send(AskEvent::Text { text }),
                Ok(StreamEvent::Usage(usage)) => send(AskEvent::Usage {
                    prompt: usage.prompt_tokens,
                    completion: usage.completion_tokens,
                    cached: usage.cached_tokens,
                }),
                Ok(StreamEvent::Done) => {
                    send(AskEvent::Done);
                    return;
                }
                Err(error) => {
                    send(AskEvent::Error {
                        message: error.to_string(),
                    });
                    return;
                }
            }
        }

        send(AskEvent::Error {
            message: "回答在中途结束了".to_owned(),
        });
    });

    Ok(())
}

/// Stops the running answer, if any.
#[tauri::command]
pub(crate) fn stop_ask(control: tauri::State<'_, AskControl>) {
    control.stop.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpts_are_labelled_with_their_page() {
        let mut request = build_request(
            "这一节在讲什么？",
            &[
                AskReference {
                    page: 31,
                    quote: "当事人担负辩论参与者的角色".to_owned(),
                },
                AskReference {
                    page: 45,
                    quote: "规范的有效性".to_owned(),
                },
            ],
            &[],
            &[],
            &[],
            crate::settings::DEFAULT_SYSTEM_PROMPT,
        )
        .unwrap();

        let prompt = &request.messages[0].content;
        assert!(prompt.contains("[摘录 p.31]"));
        assert!(prompt.contains("[摘录 p.45]"));
        assert!(prompt.contains("这一节在讲什么？"));
        let system = request.system.as_deref().unwrap();
        assert!(system.contains("一般知识"));
        assert!(system.contains("不要给它们编页码"));
    }

    #[test]
    fn no_excerpts_says_so_instead_of_pretending() {
        let mut request = build_request(
            "介绍一下背景",
            &[],
            &[],
            &[],
            &[],
            crate::settings::DEFAULT_SYSTEM_PROMPT,
        )
        .unwrap();
        let prompt = &request.messages[0].content;
        assert!(prompt.contains("没有提供摘录"));
        assert!(prompt.contains("不要编造页码"));
    }

    #[test]
    fn a_region_adds_a_label_and_an_image() {
        let mut request = build_request(
            "这个公式在说什么？",
            &[],
            &[RegionInput {
                page: 12,
                image: "data:image/png;base64,AAAA".to_owned(),
            }],
            &[],
            &[],
            crate::settings::DEFAULT_SYSTEM_PROMPT,
        )
        .unwrap();

        assert!(request.messages[0].content.contains("[截图 p.12"));
        assert_eq!(request.messages[0].images.len(), 1);
        assert_eq!(request.messages[0].images[0].mime, "image/png");
    }

    #[test]
    fn page_context_is_labelled_and_marks_scans() {
        let mut request = build_request(
            "这一章讲了什么？",
            &[],
            &[],
            &[
                PageContext {
                    page: 31,
                    text: "正文内容".to_owned(),
                },
                PageContext {
                    page: 32,
                    text: "   ".to_owned(),
                },
            ],
            &[],
            crate::settings::DEFAULT_SYSTEM_PROMPT,
        )
        .unwrap();

        let prompt = &request.messages[0].content;
        assert!(prompt.contains("[第 31 页]\n正文内容"));
        assert!(prompt.contains("[第 32 页]\n（这一页没有可提取的文字"));
    }

    #[test]
    fn too_many_regions_is_refused() {
        let regions: Vec<RegionInput> = (0..7)
            .map(|page| RegionInput {
                page,
                image: "data:image/png;base64,AAAA".to_owned(),
            })
            .collect();
        assert!(
            build_request(
                "q",
                &[],
                &regions,
                &[],
                &[],
                crate::settings::DEFAULT_SYSTEM_PROMPT
            )
            .is_err()
        );
    }

    #[test]
    fn a_broken_image_is_refused_before_it_is_sent() {
        let result = build_request(
            "q",
            &[],
            &[RegionInput {
                page: 1,
                image: "not-a-data-url".to_owned(),
            }],
            &[],
            &[],
            crate::settings::DEFAULT_SYSTEM_PROMPT,
        );
        assert!(result.is_err());
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let mut request = build_request(
            "  问题  ",
            &[AskReference {
                page: 1,
                quote: "  引文  ".to_owned(),
            }],
            &[],
            &[],
            &[],
            crate::settings::DEFAULT_SYSTEM_PROMPT,
        )
        .unwrap();
        let prompt = &request.messages[0].content;
        assert!(prompt.contains("[摘录 p.1]\n引文\n"));
        assert!(prompt.ends_with("问题：问题"));
    }
}
