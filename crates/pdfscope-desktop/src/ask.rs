//! Asking a question about the reference tray.
//!
//! The prompt is the only place where product judgement lives in this file: excerpts are labelled
//! with their page, the model is told to cite them, and it is told not to invent pages it was not
//! given. Everything else is transport.

use futures::StreamExt;
use pdfscope_ai::{ChatRequest, Message, Protocol, ProviderConfig, StreamEvent};
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
    /// The answer is complete.
    Done,
    /// The request failed.
    Error {
        /// A message fit to show the reader.
        message: String,
    },
}

const SYSTEM_PROMPT: &str = "\
你是一名读者身边的助手，正在陪他读一本 PDF。\
他只给了你一些摘录，每条摘录前都有页码标记。\
回答时：\
1. 只用给出的摘录作为依据；摘录里没有的内容，直说不知道，不要补全。\
2. 引用摘录时用 [p.页码] 的形式标注来源。\
3. 用读者提问的语言回答；他没有提问语言时用中文。\
4. 简明扼要，先给结论，再给依据。";

/// Builds the request. Pure, so the prompt can be tested without a network.
pub(crate) fn build_request(question: &str, references: &[AskReference]) -> ChatRequest {
    let mut prompt = String::new();

    if references.is_empty() {
        prompt.push_str("（读者没有提供摘录。请基于一般知识回答，不要编造页码。）\n\n");
    } else {
        for reference in references {
            prompt.push_str(&format!(
                "[摘录 p.{}]\n{}\n\n",
                reference.page,
                reference.quote.trim()
            ));
        }
    }

    prompt.push_str("问题：");
    prompt.push_str(question.trim());

    ChatRequest {
        system: Some(SYSTEM_PROMPT.to_owned()),
        messages: vec![Message::user(prompt)],
        max_tokens: 2048,
        temperature: None,
    }
}

/// Starts a streamed answer and reports it through `ask:event`.
#[tauri::command]
pub(crate) async fn ask(
    app: tauri::AppHandle,
    question: String,
    references: Vec<AskReference>,
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

    let request = build_request(&question, &references);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpts_are_labelled_with_their_page() {
        let request = build_request(
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
        );

        let prompt = &request.messages[0].content;
        assert!(prompt.contains("[摘录 p.31]"));
        assert!(prompt.contains("[摘录 p.45]"));
        assert!(prompt.contains("这一节在讲什么？"));
        assert!(request.system.as_deref().unwrap().contains("不要补全"));
    }

    #[test]
    fn no_excerpts_says_so_instead_of_pretending() {
        let request = build_request("介绍一下背景", &[]);
        let prompt = &request.messages[0].content;
        assert!(prompt.contains("没有提供摘录"));
        assert!(prompt.contains("不要编造页码"));
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let request = build_request(
            "  问题  ",
            &[AskReference {
                page: 1,
                quote: "  引文  ".to_owned(),
            }],
        );
        let prompt = &request.messages[0].content;
        assert!(prompt.contains("[摘录 p.1]\n引文\n"));
        assert!(prompt.ends_with("问题：问题"));
    }
}
