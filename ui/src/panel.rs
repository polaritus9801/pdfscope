//! The AI panel: settings, the question box, and the streamed answer.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use pdfscope_core::{Segment, parse_ranges, segments};

use crate::{Passage, Region, truncate};

/// Calls one function on `window.Bridge`.
fn bridge(name: &str, args: &[JsValue]) -> Option<JsValue> {
    let window = web_sys::window()?;
    let scope = js_sys::Reflect::get(&window, &JsValue::from_str("Bridge")).ok()?;
    let function = js_sys::Reflect::get(&scope, &JsValue::from_str(name)).ok()?;
    let function: js_sys::Function = function.dyn_into().ok()?;

    let arguments = js_sys::Array::new();
    for argument in args {
        arguments.push(argument);
    }

    function.apply(&scope, &arguments).ok()
}

fn field(value: &JsValue, key: &str) -> Option<JsValue> {
    js_sys::Reflect::get(value, &JsValue::from_str(key)).ok()
}

fn text_of(value: &JsValue, key: &str) -> Option<String> {
    field(value, key)?.as_string()
}

fn number_of(value: &JsValue, key: &str) -> Option<f64> {
    field(value, key)?.as_f64()
}

fn flag_of(value: &JsValue, key: &str) -> Option<bool> {
    field(value, key)?.as_bool()
}

/// The preset catalogue, mirroring `pdfscope_ai::PRESETS`.
fn preset(label: &str) -> Option<(&'static str, &'static str)> {
    Some(match label {
        "OpenAI" => ("openai", "https://api.openai.com/v1"),
        "Anthropic" => ("anthropic", "https://api.anthropic.com/v1"),
        "DeepSeek" => ("openai", "https://api.deepseek.com/v1"),
        "MiMo" => ("openai", "https://api.xiaomimimo.com/v1"),
        "Moonshot" => ("openai", "https://api.moonshot.cn/v1"),
        "OpenRouter" => ("openai", "https://openrouter.ai/api/v1"),
        "Gemini" => (
            "openai",
            "https://generativelanguage.googleapis.com/v1beta/openai",
        ),
        "Ollama" => ("openai", "http://127.0.0.1:11434/v1"),
        _ => return None,
    })
}

const PRESET_LABELS: &[&str] = &[
    "MiMo",
    "OpenAI",
    "Anthropic",
    "DeepSeek",
    "Moonshot",
    "OpenRouter",
    "Gemini",
    "Ollama",
];

/// Applies the answer text size to the document root.
fn apply_font_size(size: &str) {
    if let (Ok(size), Some(root)) = (
        size.parse::<u16>(),
        web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element()),
    ) {
        let _ = root.dyn_into::<web_sys::HtmlElement>().map(|element| {
            element
                .style()
                .set_property("--answer-size", &format!("{size}px"))
        });
    }
}

/// Whether the WebView thinks it has no network.
fn offline() -> bool {
    web_sys::window()
        .and_then(|window| {
            js_sys::Reflect::get(&window.into(), &JsValue::from_str("navigator"))
                .ok()
                .and_then(|navigator| {
                    js_sys::Reflect::get(&navigator, &JsValue::from_str("onLine"))
                        .ok()
                        .and_then(|value| value.as_bool())
                })
        })
        .map(|online| !online)
        .unwrap_or(false)
}

/// Copies text through the WebView's clipboard API.
fn copy_to_clipboard(text: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let window: JsValue = window.into();
    let Ok(navigator) = js_sys::Reflect::get(&window, &JsValue::from_str("navigator")) else {
        return;
    };
    let Ok(clipboard) = js_sys::Reflect::get(&navigator, &JsValue::from_str("clipboard")) else {
        return;
    };
    let Ok(write) = js_sys::Reflect::get(&clipboard, &JsValue::from_str("writeText")) else {
        return;
    };
    let Ok(write) = write.dyn_into::<js_sys::Function>() else {
        return;
    };
    let _ = write.call1(&clipboard, &JsValue::from_str(text));
}

/// Applies the chosen theme to the document root.
fn apply_theme(theme: &str) {
    if let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        let _ = root.set_attribute("data-theme", theme);
    }
}

/// Fetches a page's text from the renderer.
async fn fetch_page_text(number: u32) -> String {
    let Some(promise) = crate::call("pageText", &[JsValue::from(number)]) else {
        return String::new();
    };
    match JsFuture::from(js_sys::Promise::from(promise)).await {
        Ok(value) => value.as_string().unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// One finished exchange.
#[derive(Clone, PartialEq)]
pub(crate) struct Turn {
    question: String,
    answer: String,
    reasoning: String,
    seconds: f64,
    /// The stream was cut off, so the answer can be continued.
    interrupted: bool,
}

/// A reference that was used and moved to history.
#[derive(Clone, PartialEq)]
enum Stored {
    Passage(Passage),
    Region(Region),
}

#[component]
pub fn AiPanel(
    passages: RwSignal<Vec<Passage>>,
    regions: RwSignal<Vec<Region>>,
    page: RwSignal<u32>,
    pages: RwSignal<u32>,
    hovered: RwSignal<Option<(u32, (f64, f64, f64, f64))>>,
    clear_selection: Callback<()>,
    open: RwSignal<bool>,
    notify: Callback<String>,
    configured: RwSignal<bool>,
) -> impl IntoView {
    let model = RwSignal::new("未配置".to_owned());
    let answer = RwSignal::new(String::new());
    let turns = RwSignal::new(Vec::<Turn>::new());
    let current_question = RwSignal::new(String::new());
    let interrupted = RwSignal::new(false);
    let continue_target = RwSignal::new(None::<usize>);
    let reasoning = RwSignal::new(String::new());
    let think_started = RwSignal::new(0.0_f64);
    let think_seconds = RwSignal::new(0.0_f64);
    let status = RwSignal::new(String::new());
    let streaming = RwSignal::new(false);
    let question = RwSignal::new(String::new());
    let failed = RwSignal::new(false);
    let ranges = RwSignal::new(Vec::<pdfscope_core::PageSpan>::new());
    let range_input = RwSignal::new(String::new());
    let range_error = RwSignal::new(Option::<String>::None);
    let history = RwSignal::new(Vec::<Stored>::new());
    let show_all = RwSignal::new(false);
    let notice = RwSignal::new(Option::<String>::None);
    let testing = RwSignal::new(false);
    let collapsed = RwSignal::new(false);
    let dragging = RwSignal::new(false);
    let width = RwSignal::new(340_u32);
    let test_result = RwSignal::new(Option::<String>::None);
    let pending = RwSignal::new(String::new());
    let scheduled = RwSignal::new(false);

    let jump = move |target: u32| {
        let total = pages.get();
        if total == 0 {
            return;
        }
        clear_selection.run(());
        let target = target.clamp(1, total);
        page.set(target);
        let _ = crate::call("flash", &[JsValue::from(target)]);
    };

    let form_preset = RwSignal::new("MiMo".to_owned());
    let form_protocol = RwSignal::new("openai".to_owned());
    let form_base = RwSignal::new("https://api.xiaomimimo.com/v1".to_owned());
    let form_model = RwSignal::new("mimo-v2.6-flash".to_owned());
    let form_key = RwSignal::new(String::new());
    let form_prompt = RwSignal::new(String::new());
    let form_theme = RwSignal::new("system".to_owned());
    let form_max = RwSignal::new(String::new());
    let form_max_error = RwSignal::new(Option::<String>::None);
    let form_font_size = RwSignal::new("14".to_owned());
    let form_remember = RwSignal::new(true);
    let form_zoom = RwSignal::new("100".to_owned());
    let form_mode = RwSignal::new("continuous".to_owned());
    let form_spread_offset = RwSignal::new(false);
    let tab = RwSignal::new(0_u8);
    let form_error = RwSignal::new(Option::<String>::None);

    // Load the stored configuration once.
    Effect::new(move |_| {
        if let Some(promise) = bridge("loadSettings", &[]) {
            spawn_local(async move {
                let Ok(value) = JsFuture::from(js_sys::Promise::from(promise)).await else {
                    return;
                };
                configured.set(flag_of(&value, "has_key").unwrap_or(false));
                if let Some(stored) = text_of(&value, "model").filter(|text| !text.is_empty()) {
                    model.set(stored.clone());
                    form_model.set(stored);
                }
                if let Some(stored) = text_of(&value, "preset") {
                    form_preset.set(stored);
                }
                if let Some(stored) = text_of(&value, "protocol") {
                    form_protocol.set(stored);
                }
                if let Some(stored) = text_of(&value, "base_url") {
                    form_base.set(stored);
                }
                if let Some(stored) = text_of(&value, "system_prompt") {
                    form_prompt.set(stored);
                }
                if let Some(stored) = text_of(&value, "theme") {
                    apply_theme(&stored);
                    form_theme.set(stored);
                }
                form_font_size.set(
                    number_of(&value, "font_size")
                        .map(|size| (size as u16).to_string())
                        .unwrap_or_else(|| "14".to_owned()),
                );
                form_remember.set(flag_of(&value, "remember_position").unwrap_or(true));
                form_zoom.set(
                    number_of(&value, "default_zoom")
                        .map(|zoom| (zoom as u16).to_string())
                        .unwrap_or_else(|| "100".to_owned()),
                );
                form_mode
                    .set(text_of(&value, "page_mode").unwrap_or_else(|| "continuous".to_owned()));
                let _ = crate::call("setPageMode", &[JsValue::from_str(&form_mode.get())]);
                let _ = crate::call(
                    "setDefaultZoom",
                    &[JsValue::from_f64(
                        form_zoom.get().parse::<f64>().unwrap_or(100.0),
                    )],
                );
                apply_font_size(&form_font_size.get());
                let _ = crate::call(
                    "setRememberPosition",
                    &[JsValue::from_bool(form_remember.get())],
                );

                form_max.set(
                    number_of(&value, "max_tokens")
                        .map(|limit| (limit as u32).to_string())
                        .unwrap_or_default(),
                );
            });
        }
    });

    let add_range = move |_| match parse_ranges(&range_input.get(), pages.get().max(1)) {
        Ok(spans) => {
            ranges.set(spans);
            range_error.set(None);
        }
        Err(error) => range_error.set(Some(error.to_string())),
    };

    let redeem = move |index: usize| {
        if let Some(entry) = history.get().get(index).cloned() {
            match entry {
                Stored::Passage(passage) => passages.update(|list| list.push(passage)),
                Stored::Region(region) => regions.update(|list| list.push(region)),
            }
        }
    };

    let settle_seconds = move || {
        if think_seconds.get() == 0.0 && think_started.get() > 0.0 {
            think_seconds.set(((js_sys::Date::now() - think_started.get()) / 1000.0).max(0.1));
        }
    };

    // A finished turn is archived here rather than on the next send: an answer that ends empty
    // (reasoning consumed the budget) must still leave a trace instead of vanishing.
    let archive = move || {
        if current_question.get().trim().is_empty() {
            return;
        }
        if answer.get().trim().is_empty() && reasoning.get().trim().is_empty() {
            return;
        }

        if let Some(index) = continue_target.get() {
            let addition = answer.get();
            let cut = interrupted.get();
            turns.update(|list| {
                if let Some(turn) = list.get_mut(index) {
                    turn.answer.push_str(&addition);
                    turn.interrupted = cut;
                }
            });
            continue_target.set(None);
            current_question.set(String::new());
            answer.set(String::new());
            reasoning.set(String::new());
            return;
        }

        let finished = Turn {
            question: current_question.get(),
            answer: answer.get(),
            reasoning: reasoning.get(),
            seconds: think_seconds.get().max(0.1),
            interrupted: interrupted.get(),
        };
        turns.update(|list| list.push(finished));
        current_question.set(String::new());
        answer.set(String::new());
        reasoning.set(String::new());
    };

    let send_text = move |asked: String| {
        if streaming.get() {
            return;
        }
        if asked.trim().is_empty() {
            return;
        }
        if !configured.get() {
            open.set(true);
            return;
        }

        if continue_target.get().is_none() {
            interrupted.set(false);
        }
        current_question.set(asked.clone());
        question.set(String::new());
        answer.set(String::new());
        reasoning.set(String::new());
        think_started.set(js_sys::Date::now());
        think_seconds.set(0.0);
        streaming.set(true);
        failed.set(false);
        status.set("准备上下文…".to_owned());

        let passages_now = passages.get();
        let regions_now = regions.get();

        if !passages_now.is_empty() || !regions_now.is_empty() {
            history.update(|list| {
                list.extend(passages_now.iter().cloned().map(Stored::Passage));
                list.extend(regions_now.iter().cloned().map(Stored::Region));
            });
            passages.set(Vec::new());
            regions.set(Vec::new());
        }
        let ranges_now = ranges.get();
        let current_page = page.get();

        spawn_local(async move {
            // Page text is fetched here, not in Rust: the renderer owns the document.
            let mut context: Vec<(u32, String)> = Vec::new();
            let requested: u32 = ranges_now.iter().map(|span| span.pages()).sum();

            if !ranges_now.is_empty() {
                if requested > 20 {
                    notice.set(Some(format!("范围共 {requested} 页，这次只取了前 20 页")));
                } else {
                    notice.set(None);
                }

                'outer: for span in &ranges_now {
                    for number in span.start..=span.end {
                        if context.len() >= 20 {
                            break 'outer;
                        }
                        context.push((number, fetch_page_text(number).await));
                    }
                }
            } else if passages_now.is_empty() && regions_now.is_empty() {
                context.push((current_page, fetch_page_text(current_page).await));
            }

            let references = js_sys::Array::new();
            for passage in &passages_now {
                let item = js_sys::Object::new();
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("page"),
                    &JsValue::from(passage.page),
                );
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("quote"),
                    &JsValue::from_str(&passage.quote),
                );
                references.push(&item);
            }

            let images = js_sys::Array::new();
            for region in &regions_now {
                let item = js_sys::Object::new();
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("page"),
                    &JsValue::from(region.page),
                );
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("image"),
                    &JsValue::from_str(&region.image),
                );
                images.push(&item);
            }

            let pages_array = js_sys::Array::new();
            for (number, text) in &context {
                let item = js_sys::Object::new();
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("page"),
                    &JsValue::from(*number),
                );
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("text"),
                    &JsValue::from_str(text),
                );
                pages_array.push(&item);
            }

            let history = js_sys::Array::new();
            let recent: Vec<Turn> = turns
                .get()
                .iter()
                .rev()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            for turn in recent {
                let item = js_sys::Object::new();
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("question"),
                    &JsValue::from_str(&turn.question),
                );
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("answer"),
                    &JsValue::from_str(&turn.answer),
                );
                history.push(&item);
            }

            status.set("等待回答…".to_owned());
            let started = js_sys::Date::now();
            let callback = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
                let flush_pending = move || {
                    let text = pending.get();
                    pending.set(String::new());
                    if !text.is_empty() {
                        answer.update(|buffer| buffer.push_str(&text));
                    }
                };
                let kind = text_of(&event, "kind").unwrap_or_default();
                match kind.as_str() {
                    "reasoning" => {
                        if let Some(delta) = text_of(&event, "text") {
                            if think_started.get() == 0.0 {
                                think_started.set(js_sys::Date::now());
                            }
                            reasoning.update(|buffer| buffer.push_str(&delta));
                        }
                    }
                    "text" => {
                        status.update(|line| {
                            if line.contains("正在重试") {
                                line.clear();
                            }
                        });
                        if think_seconds.get() == 0.0 && think_started.get() > 0.0 {
                            think_seconds.set((js_sys::Date::now() - think_started.get()) / 1000.0);
                        }
                        if let Some(delta) = text_of(&event, "text") {
                            // Coalesce tokens into one reactive update per frame: the answer
                            // re-parses citations on every change, so per-token updates are O(n²).
                            pending.update(|buffer| buffer.push_str(&delta));
                            if !scheduled.get() {
                                scheduled.set(true);
                                let flush = Closure::once(move || {
                                    scheduled.set(false);
                                    let text = pending.get();
                                    pending.set(String::new());
                                    if !text.is_empty() {
                                        answer.update(|buffer| buffer.push_str(&text));
                                    }
                                });
                                if let Some(window) = web_sys::window() {
                                    let _ = window
                                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                                            flush.as_ref().unchecked_ref(),
                                            50,
                                        );
                                }
                                flush.forget();
                            }
                        }
                    }
                    "retrying" => {
                        let attempt = number_of(&event, "attempt").unwrap_or(0.0) as u32;
                        let of = number_of(&event, "of").unwrap_or(0.0) as u32;
                        status.set(format!("连接中断，正在重试（{attempt}/{of}）…"));
                    }
                    "usage" => {
                        let prompt = number_of(&event, "prompt").unwrap_or(0.0) as u64;
                        let completion = number_of(&event, "completion").unwrap_or(0.0) as u64;
                        let cached = number_of(&event, "cached").unwrap_or(0.0) as u64;
                        status.set(format!("输入 {prompt} · 输出 {completion} · 缓存 {cached}"));
                    }
                    "stopped" => {
                        flush_pending();
                        streaming.set(false);
                        settle_seconds();
                        let kept = !answer.get().trim().is_empty();
                        archive();
                        status.set(if kept {
                            "已停止".to_owned()
                        } else {
                            "已停止，没有生成正文".to_owned()
                        });
                    }
                    "done" => {
                        flush_pending();
                        streaming.set(false);
                        settle_seconds();

                        let elapsed = js_sys::Date::now() - started;
                        let characters = answer.get().chars().count() as f64;
                        let empty = answer.get().trim().is_empty();
                        let had_reasoning = !reasoning.get().trim().is_empty();
                        archive();

                        if empty {
                            status.set(if had_reasoning {
                                "模型只返回了思考过程，没有正文".to_owned()
                            } else {
                                "模型没有返回内容".to_owned()
                            });
                        } else if elapsed > 0.0 {
                            let rate = (characters / 2.0) / (elapsed / 1000.0);
                            status.update(|line| line.push_str(&format!(" · ≈ {rate:.0} tok/s")));
                        }
                    }
                    "error" => {
                        flush_pending();
                        streaming.set(false);
                        settle_seconds();
                        interrupted.set(!answer.get().trim().is_empty());
                        archive();
                        failed.set(true);
                        let message =
                            text_of(&event, "message").unwrap_or_else(|| "请求失败".to_owned());
                        status.set(if offline() {
                            "网络已断开，请检查连接后再试。".to_owned()
                        } else {
                            message
                        });
                    }
                    _ => {}
                }
            });

            let _ = bridge(
                "ask",
                &[
                    JsValue::from_str(&asked),
                    references.into(),
                    images.into(),
                    pages_array.into(),
                    history.into(),
                    callback.as_ref().clone(),
                ],
            );
            callback.forget();
        });
    };

    let send = move |_| send_text(question.get());

    let continue_last = move || {
        let all = turns.get();
        let Some(last) = all.len().checked_sub(1) else {
            return;
        };
        continue_target.set(Some(last));
        send_text("请从断开处继续写下去，不要重复已经写过的内容。".to_owned());
    };

    // Global shortcuts for the answer actions; they ignore plain typing so the composer is unaffected.
    Effect::new(move |_| {
        let Some(window) = web_sys::window() else {
            return;
        };

        let handler =
            Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
                if !(event.meta_key() || event.ctrl_key()) || !event.shift_key() {
                    return;
                }

                match event.key().to_ascii_lowercase().as_str() {
                    "c" => {
                        let text = turns
                            .get()
                            .last()
                            .map(|turn| turn.answer.clone())
                            .filter(|text| !text.trim().is_empty())
                            .unwrap_or_else(|| answer.get());
                        if text.trim().is_empty() {
                            return;
                        }
                        copy_to_clipboard(&text);
                        notify.run("已复制".to_owned());
                        event.prevent_default();
                    }
                    "r" => {
                        let Some(last) = turns.get().last().cloned() else {
                            return;
                        };
                        send_text(last.question);
                        event.prevent_default();
                    }
                    "g" => {
                        let interrupted = turns.get().last().is_some_and(|turn| turn.interrupted);
                        if interrupted {
                            continue_last();
                            event.prevent_default();
                        }
                    }
                    _ => {}
                }
            });

        let _ =
            window.add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
        handler.forget();
    });

    let build_payload = move || -> Option<js_sys::Object> {
        let payload = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("preset"),
            &JsValue::from_str(&form_preset.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("protocol"),
            &JsValue::from_str(&form_protocol.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("base_url"),
            &JsValue::from_str(&form_base.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("model"),
            &JsValue::from_str(&form_model.get()),
        );

        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("system_prompt"),
            &JsValue::from_str(&form_prompt.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("theme"),
            &JsValue::from_str(&form_theme.get()),
        );

        let limit = match form_max.get().trim() {
            "" => JsValue::NULL,
            text => match text.parse::<u32>() {
                Ok(value) if value > 0 => JsValue::from_f64(f64::from(value)),
                _ => {
                    form_max_error.set(Some("留空，或填一个正整数".to_owned()));
                    return None;
                }
            },
        };
        let _ = js_sys::Reflect::set(&payload, &JsValue::from_str("max_tokens"), &limit);
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("font_size"),
            &JsValue::from_f64(f64::from(form_font_size.get().parse::<u16>().unwrap_or(14))),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("remember_position"),
            &JsValue::from_bool(form_remember.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("spread_offset"),
            &JsValue::from_bool(form_spread_offset.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("page_mode"),
            &JsValue::from_str(&form_mode.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("default_zoom"),
            &JsValue::from_f64(form_zoom.get().parse::<f64>().unwrap_or(100.0)),
        );

        let key = form_key.get();
        if !key.trim().is_empty() {
            let _ = js_sys::Reflect::set(
                &payload,
                &JsValue::from_str("api_key"),
                &JsValue::from_str(&key),
            );
        }

        Some(payload)
    };

    let save = move |_| {
        let Some(payload) = build_payload() else {
            return;
        };

        if let Some(promise) = bridge("saveSettings", &[payload.into()]) {
            spawn_local(async move {
                match JsFuture::from(js_sys::Promise::from(promise)).await {
                    Ok(_) => {
                        form_error.set(None);
                        open.set(false);
                        configured.set(true);
                        model.set(form_model.get());
                        apply_theme(&form_theme.get());
                        apply_font_size(&form_font_size.get());
                        let _ = crate::call(
                            "setRememberPosition",
                            &[JsValue::from_bool(form_remember.get())],
                        );
                        let _ = crate::call("setPageMode", &[JsValue::from_str(&form_mode.get())]);
                        let _ = crate::call(
                            "setDefaultZoom",
                            &[JsValue::from_f64(
                                form_zoom.get().parse::<f64>().unwrap_or(100.0),
                            )],
                        );
                        notify.run("设置已保存".to_owned());
                        form_key.set(String::new());
                    }
                    Err(error) => {
                        form_error.set(Some(
                            error.as_string().unwrap_or_else(|| "保存失败".to_owned()),
                        ));
                    }
                }
            });
        }
    };

    let test = move |_| {
        if testing.get() {
            return;
        }
        testing.set(true);
        test_result.set(None);
        let payload = build_payload();

        if let Some(promise) = bridge("testSettings", &[payload.into()]) {
            spawn_local(async move {
                match JsFuture::from(js_sys::Promise::from(promise)).await {
                    Ok(value) => {
                        test_result.set(Some(
                            value.as_string().unwrap_or_else(|| "连接正常".to_owned()),
                        ));
                    }
                    Err(error) => {
                        test_result.set(Some(
                            error.as_string().unwrap_or_else(|| "连接失败".to_owned()),
                        ));
                    }
                }
                testing.set(false);
            });
        } else {
            testing.set(false);
        }
    };

    view! {
        <aside
            class="panel"
            class:collapsed=move || collapsed.get()
            style=move || {
                if collapsed.get() {
                    "width: 28px".to_owned()
                } else {
                    format!("width: {}px", width.get())
                }
            }
        >
            <div
                class="panel-resize"
                on:pointerdown=move |event: web_sys::PointerEvent| {
                    event.prevent_default();
                    if let Some(target) = event
                        .current_target()
                        .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
                    {
                        let _ = target.set_pointer_capture(event.pointer_id());
                    }
                    // Without this a fast drag starts selecting text on both sides of the split.
                    if let Some(root) = web_sys::window()
                        .and_then(|window| window.document())
                        .and_then(|document| document.document_element())
                    {
                        let _ = root.class_list().add_1("resizing");
                    }
                    let _ = crate::call("clearSelection", &[]);
                    dragging.set(true);
                }
                on:pointermove=move |event: web_sys::PointerEvent| {
                    if !dragging.get() {
                        return;
                    }
                    event.prevent_default();
                    let viewport = web_sys::window()
                        .and_then(|window| window.inner_width().ok())
                        .and_then(|value| value.as_f64())
                        .unwrap_or(1280.0);
                    let next = (viewport - f64::from(event.client_x())).clamp(260.0, 640.0);
                    width.set(next as u32);
                }
                on:pointerup=move |_| {
                    dragging.set(false);
                    if let Some(root) = web_sys::window()
                        .and_then(|window| window.document())
                        .and_then(|document| document.document_element())
                    {
                        let _ = root.class_list().remove_1("resizing");
                    }
                }
            ></div>
            <div class="panel-head">
                <button
                    class="panel-toggle"
                    title="收起"
                    on:click=move |_| collapsed.set(true)
                >
                    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M9 6l6 6-6 6" />
                    </svg>
                </button>
                <span class="panel-title">"AI"</span>
                <span class="panel-model">{move || model.get()}</span>

            </div>

            <button
                class="panel-expand"
                title="展开"
                on:click=move |_| collapsed.set(false)
            >
                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M15 6l-6 6 6 6" />
                </svg>
            </button>

            <div class="panel-body">
                {move || {
                    if !configured.get() {
                        view! { <p class="panel-hint">"未配置模型 · 前往设置"</p> }.into_any()
                    } else if turns.get().is_empty() && answer.get().is_empty() && !streaming.get() {
                        view! { <p class="panel-hint">"选中文字加入引用，然后提问"</p> }.into_any()
                    } else {
                        view! {
                            <div
                                id="transcript"
                                class="transcript"
                                on:click=move |event: web_sys::MouseEvent| {
                                    let Some(target) = event
                                        .target()
                                        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                                    else {
                                        return;
                                    };
                                    if let Some(number) = target
                                        .get_attribute("data-page")
                                        .and_then(|value| value.parse::<u32>().ok())
                                    {
                                        event.prevent_default();
                                        jump(number);
                                    }
                                }
                            >
                                {move || {
                                    if !(failed.get() && !streaming.get()) {
                                        return ().into_any();
                                    }
                                    view! {
                                        <div class="error-card">
                                            <span>{status.get()}</span>
                                            <button
                                                class="action"
                                                on:click=move |_| {
                                                    if let Some(last) = turns.get().last().cloned() {
                                                        send_text(last.question);
                                                    }
                                                }
                                            >
                                                "重试"
                                            </button>
                                        </div>
                                    }
                                        .into_any()
                                }}

                                {move || {
                                    let all = turns.get();
                                    let last = all.len().saturating_sub(1);
                                    all
                                        .into_iter()
                                        .enumerate()
                                        .map(|(index, turn)| {
                                            let detail = if turn.reasoning.is_empty() {
                                                ().into_any()
                                            } else {
                                                view! {
                                                    <details class="think">
                                                        <summary>
                                                            {format!("思考 {:.1} 秒", turn.seconds.max(0.1))}
                                                        </summary>
                                                        <pre class="think-text">{turn.reasoning.clone()}</pre>
                                                    </details>
                                                }
                                                    .into_any()
                                            };

                                            let body = if turn.answer.trim().is_empty() {
                                                view! {
                                                    <p class="turn-empty">
                                                        "模型只返回了思考过程，没有正文。"
                                                    </p>
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    <div
                                                        class="answer-text markdown"
                                                        inner_html=pdfscope_core::to_html(&turn.answer)
                                                    ></div>
                                                }
                                                    .into_any()
                                            };

                                            let copied = turn.answer.clone();
                                            let retry = turn.question.clone();
                                            let locate = segments(&turn.answer)
                                                .into_iter()
                                                .find_map(|segment| match segment {
                                                    Segment::Citation(target) => Some(target),
                                                    Segment::Text(_) => None,
                                                });
                                            let located = locate
                                                .map(|target| {
                                                    view! {
                                                        <button
                                                            class="action"
                                                            on:click=move |_| jump(target)
                                                        >
                                                            "定位"
                                                        </button>
                                                    }
                                                });
                                            let regenerate = (index == last)
                                                .then(|| {
                                                    view! {
                                                        <button
                                                            class="action"
                                                            on:click=move |_| send_text(retry.clone())
                                                        >
                                                            "重新生成"
                                                        </button>
                                                    }
                                                });
                                            let continued = (index == last && turn.interrupted)
                                                .then(|| {
                                                    view! {
                                                        <button
                                                            class="action"
                                                            on:click=move |_| continue_last()
                                                        >
                                                            "继续生成"
                                                        </button>
                                                    }
                                                });

                                            view! {
                                                <div class="turn">
                                                    <p class="turn-question">{turn.question.clone()}</p>
                                                    {detail}
                                                    {body}
                                                    <div class="turn-actions">
                                                        {located}
                                                        <button
                                                            class="action"
                                                            on:click=move |_| {
                                                                copy_to_clipboard(&copied);
                                                                notify.run("已复制".to_owned());
                                                            }
                                                        >
                                                            "复制"
                                                        </button>
                                                        {regenerate}
                                                        {continued}
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect_view()
                                }}

                                {move || {
                                    if answer.get().is_empty() && !streaming.get() {
                                        return ().into_any();
                                    }
                                    let streaming_now = streaming.get();
                                    let detail = if reasoning.get().is_empty() {
                                        ().into_any()
                                    } else {
                                        let label = if streaming_now {
                                            "思考中…".to_owned()
                                        } else {
                                            format!("思考 {:.1} 秒", think_seconds.get())
                                        };
                                        if streaming_now {
                                            view! {
                                                <div class="think">
                                                    <div class="think-head">{label}</div>
                                                    <pre class="think-text">{reasoning.get()}</pre>
                                                </div>
                                            }
                                                .into_any()
                                        } else {
                                            view! {
                                                <details class="think">
                                                    <summary>{label}</summary>
                                                    <pre class="think-text">{reasoning.get()}</pre>
                                                </details>
                                            }
                                                .into_any()
                                        }
                                    };
                                    view! {
                                        <div class="turn current">
                                            <p class="turn-question">
                                                {move || {
                                                    if continue_target.get().is_some() {
                                                        "继续生成…".to_owned()
                                                    } else {
                                                        current_question.get()
                                                    }
                                                }}
                                            </p>
                                            {detail}
                                            <div
                                                class="answer-text markdown"
                                                aria-live="polite"
                                                inner_html=move || pdfscope_core::to_html(&answer.get())
                                            ></div>
                                            <div class="turn-actions">
                                                <button
                                                    class="action"
                                                    on:click=move |_| {
                                                    copy_to_clipboard(&answer.get());
                                                    notify.run("已复制".to_owned());
                                                }
                                                >
                                                    "复制"
                                                </button>
                                            </div>
                                        </div>
                                    }
                                        .into_any()
                                }}
                            </div>
                        }
                            .into_any()
                    }
                }}
            </div>

            <div class="composer">
                {move || {
                    let all = history.get();
                    if all.is_empty() {
                        return ().into_any();
                    }
                    let start = if show_all.get() { 0 } else { all.len().saturating_sub(3) };
                    let entries: Vec<(usize, Stored)> = all
                        .iter()
                        .cloned()
                        .enumerate()
                        .skip(start)
                        .collect();
                    view! {
                        <div class="history">
                            <div class="history-head">
                                <span>"历史引用"</span>
                                <button
                                    class="link"
                                    on:click=move |_| show_all.update(|open| *open = !*open)
                                >
                                    {move || if show_all.get() { "收起" } else { "全部" }}
                                </button>
                            </div>
                            {entries
                                .into_iter()
                                .map(|(index, entry)| {
                                    let (label, preview, page_number, rect) = match &entry {
                                        Stored::Passage(passage) => (
                                            format!("p.{} · {}", passage.page, truncate(&passage.quote, 14)),
                                            passage.quote.clone(),
                                            passage.page,
                                            passage.rect,
                                        ),
                                        Stored::Region(region) => (
                                            format!("p.{} · 截图", region.page),
                                            "（截图）".to_owned(),
                                            region.page,
                                            region.rect,
                                        ),
                                    };
                                    view! {
                                        <div
                                            class="history-row"
                                            on:mouseenter=move |_| {
                                                hovered.set(Some((page_number, rect)));
                                            }
                                            on:mouseleave=move |_| hovered.set(None)
                                            on:click=move |event: web_sys::MouseEvent| {
                                                if event.shift_key() {
                                                    jump(page_number);
                                                } else {
                                                    redeem(index);
                                                }
                                            }
                                        >
                                            <span class="history-label">{label}</span>
                                            <span class="history-preview">{preview}</span>
                                            <span class="pill">{format!("p.{page_number} →")}</span>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                }}
                {move || {
                    passages
                        .get()
                        .into_iter()
                        .enumerate()
                        .map(|(index, passage)| {
                            view! {
                                <div class="ref-row">
                                    <span class="ref-kind">"文"</span>
                                    <span class="ref-text">{passage.quote.clone()}</span>
                                    <span class="ref-page">{format!("p.{}", passage.page)}</span>
                                    <button
                                        class="ref-remove"
                                        title="移除"
                                        on:click=move |_| {
                                            passages
                                                .update(|list| {
                                                    if index < list.len() {
                                                        list.remove(index);
                                                    }
                                                });
                                        }
                                    >
                                        "✕"
                                    </button>
                                </div>
                            }
                        })
                        .collect_view()
                }}
                {move || {
                    regions
                        .get()
                        .into_iter()
                        .enumerate()
                        .map(|(index, region)| {
                            view! {
                                <div class="ref-row">
                                    <img class="ref-thumb" src=region.image alt="" />
                                    <span class="ref-text">"截图"</span>
                                    <span class="ref-page">{format!("p.{}", region.page)}</span>
                                    <button
                                        class="ref-remove"
                                        title="移除"
                                        on:click=move |_| {
                                            regions
                                                .update(|list| {
                                                    if index < list.len() {
                                                        list.remove(index);
                                                    }
                                                });
                                        }
                                    >
                                        "✕"
                                    </button>
                                </div>
                            }
                        })
                        .collect_view()
                }}
                <div class="range-row">
                    <input
                        id="range-input"
                        class="range-input"
                        type="text"
                        placeholder="页码范围 31-45, 60"
                        prop:value=move || range_input.get()
                        on:input=move |event| range_input.set(event_target_value(&event))
                    />
                    <button class="button" on:click=add_range>
                        "加入"
                    </button>
                </div>
                {move || {
                    range_error
                        .get()
                        .map(|message| {
                            view! { <p class="range-summary range-error">{message}</p> }
                        })
                }}
                {move || {
                    notice
                        .get()
                        .map(|message| view! { <p class="range-summary range-error">{message}</p> })
                }}
                {move || {
                    if ranges.get().is_empty() {
                        ().into_any()
                    } else {
                        let label = ranges
                            .get()
                            .iter()
                            .map(|span| {
                                if span.start == span.end {
                                    format!("p.{}", span.start)
                                } else {
                                    format!("p.{}-{}", span.start, span.end)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" · ");
                        view! { <p class="range-summary">{label}</p> }.into_any()
                    }
                }}
                <textarea
                    class="ask"
                    aria-label="问题"
                    placeholder="问点什么…"
                    prop:value=move || question.get()
                    on:input=move |event| {
                        question.set(event_target_value(&event));
                        if let Some(target) = event
                            .current_target()
                            .and_then(|target| target.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
                        {
                            let _ = target.set_attribute("style", "height:auto");
                            let height = target.scroll_height().min(150);
                            let _ = target.set_attribute("style", &format!("height:{height}px"));
                        }
                    }
                    on:keydown=move |event: web_sys::KeyboardEvent| {
                        let modified = event.meta_key()
                            || event.ctrl_key()
                            || event.alt_key()
                            || event.shift_key();
                        if event.key() == "Enter" && !modified {
                            event.prevent_default();
                            send(());
                        } else if event.key() == "Escape" && streaming.get() {
                            let _ = bridge("stop", &[]);
                        }
                    }
                ></textarea>
                <div class="composer-row">
                    <span class="status">{move || status.get()}</span>
                    <button
                        class="button button-primary"
                        on:click=move |_| {
                            if streaming.get() {
                                let _ = bridge("stop", &[]);
                            } else {
                                send(());
                            }
                        }
                    >
                        {move || {
                            if streaming.get() {
                                "停止"
                            } else if failed.get() {
                                "重试"
                            } else {
                                "发送"
                            }
                        }}
                    </button>
                </div>
            </div>

            <Show when=move || open.get()>
                <div
                    class="settings-overlay"
                    on:click=move |event: web_sys::MouseEvent| {
                        // A click on the backdrop closes; a click inside the dialog does not.
                        if event.target() == event.current_target() {
                            open.set(false);
                        }
                    }
                >
                    <div class="settings-dialog">
                        <header class="settings-head">
                            <h2>"设置"</h2>
                            <button
                                class="icon-button"
                                title="关闭"
                                on:click=move |_| open.set(false)
                            >
                                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
                                    <path d="M6 6l12 12M18 6L6 18" />
                                </svg>
                            </button>
                        </header>

                        <div class="settings-body">
                            <nav class="settings-nav">
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 0
                                    on:click=move |_| tab.set(0)
                                >
                                    "通用设置"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 1
                                    on:click=move |_| tab.set(1)
                                >
                                    "阅读"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 2
                                    on:click=move |_| tab.set(2)
                                >
                                    "模型"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 3
                                    on:click=move |_| tab.set(3)
                                >
                                    "提示词"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 4
                                    on:click=move |_| tab.set(4)
                                >
                                    "快捷键"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 5
                                    on:click=move |_| tab.set(5)
                                >
                                    "关于"
                                </button>
                            </nav>

                            <div class="settings-content">
                                <Show when=move || tab.get() == 0>
                                    <section class="settings-section">
                                        <h3>"外观"</h3>
                                        <p class="settings-hint">"选择界面的明暗，跟随系统会随 macOS 切换。"</p>
                                        <div class="theme-cards">
                                            {[
                                                ("system", "跟随系统", "<rect x=\"3\" y=\"4\" width=\"18\" height=\"13\" rx=\"2\"/><path d=\"M8 20h8M12 17v3\"/>"),
                                                ("light", "浅色", "<circle cx=\"12\" cy=\"12\" r=\"4\"/><path d=\"M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M19 5l-1.5 1.5M6.5 17.5L5 19\"/>"),
                                                ("dark", "深色", "<path d=\"M20 14a8 8 0 1 1-9.9-7.7A7 7 0 0 0 20 14Z\"/>"),
                                            ]
                                                .into_iter()
                                                .map(|(value, label, path)| {
                                                    view! {
                                                        <button
                                                            class="theme-card"
                                                            class:active=move || form_theme.get() == value
                                                            on:click=move |_| form_theme.set(value.to_owned())
                                                        >
                                                            <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" inner_html=path></svg>
                                                            <span>{label}</span>
                                                        </button>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    </section>


                                </Show>

                                <Show when=move || tab.get() == 1>
                                    <section class="settings-section">
                                        <h3>"阅读"</h3>
                                        <p class="settings-hint">"重新打开文件时回到上次读到的位置。"</p>
                                        <label class="settings-row">
                                            <span>"翻页方式"</span>
                                            <select on:change=move |event| form_mode.set(event_target_value(&event))>
                                                <option value="continuous" selected=move || form_mode.get() == "continuous">
                                                    "连续滚动"
                                                </option>
                                                <option value="single" selected=move || form_mode.get() == "single">
                                                    "单页"
                                                </option>
                                                <option value="spread" selected=move || form_mode.get() == "spread">
                                                    "双页"
                                                </option>
                                            </select>
                                        </label>

                                        <label class="settings-row">
                                            <span>"双页奇偶偏移"</span>
                                            <input
                                                type="checkbox"
                                                prop:checked=move || form_spread_offset.get()
                                                on:change=move |event| {
                                                    form_spread_offset.set(event_target_checked(&event));
                                                }
                                            />
                                        </label>

                                        <label class="settings-row">
                                            <span>"默认缩放"</span>
                                            <select on:change=move |event| form_zoom.set(event_target_value(&event))>
                                                {[75_u16, 90, 100, 110, 125, 150]
                                                    .into_iter()
                                                    .map(|percent| {
                                                        let value = percent.to_string();
                                                        let selected = {
                                                            let value = value.clone();
                                                            move || form_zoom.get() == value
                                                        };
                                                        view! {
                                                            <option value=value.clone() selected=selected>
                                                                {format!("{percent}%")}
                                                            </option>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </select>
                                        </label>

                                        <div class="settings-row">
                                            <span>"恢复默认"</span>
                                            <button
                                                class="button"
                                                on:click=move |_| {
                                                    form_zoom.set("100".to_owned());
                                                    form_remember.set(true);
                                                }
                                            >
                                                "恢复阅读默认"
                                            </button>
                                        </div>

                                        <label class="settings-row">
                                            <span>"记住阅读位置"</span>
                                            <input
                                                type="checkbox"
                                                prop:checked=move || form_remember.get()
                                                on:change=move |event| {
                                                    form_remember.set(event_target_checked(&event));
                                                }
                                            />
                                        </label>
                                    </section>
                                </Show>

                                <Show when=move || tab.get() == 4>
                                    <section class="settings-section">
                                        <h3>"快捷键"</h3>
                                        <dl class="shortcuts">
                                            <dt>"⌘O"</dt><dd>"打开 PDF"</dd>
                                            <dt>"⌘F"</dt><dd>"文档内查找"</dd>
                                            <dt>"⌘↑ / ⌘↓"</dt><dd>"跳到首页 / 末页"</dd>
                                            <dt>"[ / ]"</dt><dd>"上一页 / 下一页"</dd>
                                            <dt>"Enter"</dt><dd>"发送问题"</dd>
                                            <dt>"Shift / ⌘ + Enter"</dt><dd>"在问题里换行"</dd>
                                            <dt>"Esc"</dt><dd>"关闭查找或停止回答"</dd>
                                            <dt>"⌘⇧C"</dt><dd>"复制最后一条回答"</dd>
                                            <dt>"⌘⇧R"</dt><dd>"重新生成"</dd>
                                            <dt>"⌘⇧G"</dt><dd>"继续生成（断流后）"</dd>
                                        </dl>
                                    </section>
                                </Show>

                                <Show when=move || tab.get() == 5>
                                    <section class="settings-section">
                                        <h3>"关于"</h3>
                                        <p class="settings-hint">"PDFScope 0.1.0 · 本地优先的 PDF 阅读与提问工具。"</p>
                                        <p class="settings-hint">
                                            "配置与密钥保存在本机应用目录，不会上传；只有提问时才会把引用内容发送给你选择的模型服务。"
                                        </p>
                                    </section>
                                </Show>

                                <Show when=move || tab.get() == 2>
                                    <section class="settings-section">
                                        <h3>"服务"</h3>
                                        <p class="settings-hint">"任何 OpenAI 兼容或 Anthropic 兼容的接口都可以。"</p>

                                        <label class="settings-row">
                                            <span>"厂商"</span>
                                            <select
                                                on:change=move |event| {
                                                    let chosen = event_target_value(&event);
                                                    if let Some((protocol, base)) = preset(&chosen) {
                                                        form_protocol.set(protocol.to_owned());
                                                        form_base.set(base.to_owned());
                                                    }
                                                    form_preset.set(chosen);
                                                }
                                            >
                                                {PRESET_LABELS
                                                    .iter()
                                                    .map(|label| {
                                                        let value = *label;
                                                        let selected = move || form_preset.get() == value;
                                                        view! {
                                                            <option value=value selected=selected>
                                                                {value}
                                                            </option>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </select>
                                        </label>

                                        <label class="settings-row">
                                            <span>"接口地址"</span>
                                            <input
                                                type="text"
                                                prop:value=move || form_base.get()
                                                on:input=move |event| form_base.set(event_target_value(&event))
                                            />
                                        </label>

                                        <label class="settings-row">
                                            <span>"模型"</span>
                                            <input
                                                type="text"
                                                prop:value=move || form_model.get()
                                                on:input=move |event| form_model.set(event_target_value(&event))
                                            />
                                        </label>

                                    <section class="settings-section">
                                        <h3>"字号"</h3>
                                        <p class="settings-hint">"回答正文的大小。"</p>
                                        <label class="settings-row">
                                            <span>"字号"</span>
                                            <select on:change=move |event| form_font_size.set(event_target_value(&event))>
                                                {(12..=18)
                                                    .map(|size| {
                                                        let value = size.to_string();
                                                        let selected = {
                                                            let value = value.clone();
                                                            move || form_font_size.get() == value
                                                        };
                                                        view! {
                                                            <option value=value.clone() selected=selected>
                                                                {format!("{size} px")}
                                                            </option>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </select>
                                        </label>
                                    </section>

                                    <section class="settings-section">
                                        <h3>"API Key"</h3>
                                        <p class="settings-hint">"保存在本机配置目录的 api-key 文件，权限 0600，只有这个用户能读。"</p>
                                        <label class="settings-row">
                                            <span>"密钥"</span>
                                            <input
                                                type="password"
                                                placeholder="留空表示沿用已保存的 key"
                                                prop:value=move || form_key.get()
                                                on:input=move |event| form_key.set(event_target_value(&event))
                                            />
                                        </label>
                                    </section>

                                        <label class="settings-row">
                                            <span>"最大输出"</span>
                                            <input
                                                type="number"
                                                min="1"
                                                placeholder="留空 = 不限制"
                                                prop:value=move || form_max.get()
                                                on:input=move |event| {
                                                    form_max_error.set(None);
                                                    form_max.set(event_target_value(&event));
                                                }
                                            />
                                        </label>
                                        {move || {
                                            form_max_error
                                                .get()
                                                .map(|message| {
                                                    view! { <p class="field-error">{message}</p> }
                                                })
                                        }}

                                        <div class="settings-row">
                                            <span>"连接"</span>
                                            <button
                                                class="button"
                                                on:click=test
                                                prop:disabled=move || testing.get()
                                            >
                                                {move || if testing.get() { "测试中…" } else { "测试连接" }}
                                            </button>
                                        </div>
                                    </section>
                                </Show>

                                <Show when=move || tab.get() == 3>
                                    <section class="settings-section">
                                        <h3>"系统提示词"</h3>
                                        <p class="settings-hint">"每次提问都会带上这段指令。清空会恢复默认。"</p>
                                        <button
                                            class="button"
                                            on:click=move |_| form_prompt.set(String::new())
                                        >
                                            "恢复默认提示词"
                                        </button>
                                        <textarea
                                            class="prompt-field"
                                            prop:value=move || form_prompt.get()
                                            on:input=move |event| form_prompt.set(event_target_value(&event))
                                        ></textarea>
                                    </section>
                                </Show>

                                {move || {
                                    form_error
                                        .get()
                                        .map(|message| view! { <p class="error settings-error">{message}</p> })
                                }}
                                {move || {
                                    test_result
                                        .get()
                                        .map(|message| view! { <p class="settings-ok">{message}</p> })
                                }}
                            </div>
                        </div>

                        <footer class="settings-foot">
                            <span class="settings-path">"配置保存在本机，不会上传。"</span>
                            <button class="button" on:click=move |_| open.set(false)>
                                "取消"
                            </button>
                            <button class="button button-primary" on:click=save>
                                "保存"
                            </button>
                        </footer>
                    </div>
                </div>
            </Show>
        </aside>
    }
}
