//! The AI panel: settings, the question box, and the streamed answer.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use pdfscope_core::{Segment, segments};

use crate::scope_ui::PageManager;
use crate::settings::SettingsSheet;
use crate::{Passage, Region, truncate};

/// Calls one function on `window.Bridge`.
pub(crate) fn bridge(name: &str, args: &[JsValue]) -> Option<JsValue> {
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

pub(crate) fn text_of(value: &JsValue, key: &str) -> Option<String> {
    field(value, key)?.as_string()
}

pub(crate) fn number_of(value: &JsValue, key: &str) -> Option<f64> {
    field(value, key)?.as_f64()
}

pub(crate) fn flag_of(value: &JsValue, key: &str) -> Option<bool> {
    field(value, key)?.as_bool()
}

/// The preset catalogue, mirroring `pdfscope_ai::PRESETS`.
pub(crate) fn preset(label: &str) -> Option<(&'static str, &'static str)> {
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

pub(crate) const PRESET_LABELS: &[&str] = &[
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
pub(crate) fn apply_font_size(size: &str) {
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
pub(crate) fn apply_theme(theme: &str) {
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
    undo_push: Callback<()>,
    reset_history: Callback<()>,
    scope_mode: RwSignal<u8>,
    page_set: RwSignal<Vec<u32>>,
    page_limit: RwSignal<u32>,
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
    let history = RwSignal::new(Vec::<Stored>::new());
    let show_all = RwSignal::new(false);
    let notice = RwSignal::new(Option::<String>::None);
    let collapsed = RwSignal::new(false);
    let dragging = RwSignal::new(false);
    let width = RwSignal::new(380_u32);
    let stage_anchor = RwSignal::new(Option::<(f64, f64)>::None);
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

    let manager_open = RwSignal::new(false);
    let reference_count = move || passages.get().len() + regions.get().len() + page_set.get().len();

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
            reset_history.run(());
        }
        let current_page = page.get();
        let total = pages.get();
        let mode = scope_mode.get();
        let explicit = page_set.get();

        spawn_local(async move {
            // Page text is fetched here, not in Rust: the renderer owns the document.
            let mut context: Vec<(u32, String)> = Vec::new();

            let cap = match page_limit.get() {
                0 => None,
                limit => Some(limit as usize),
            };
            // "References" means everything the reader marked: page bars as well as quotes and boxes.
            let scope = match mode {
                0 => pdfscope_core::pages::Scope::Document,
                1 => pdfscope_core::pages::Scope::Current,
                _ => pdfscope_core::pages::Scope::Selected,
            };
            let (selected, wanted) =
                pdfscope_core::pages::request_pages(scope, total, current_page, &explicit, cap);

            if mode == 2 && explicit.is_empty() && passages_now.is_empty() && regions_now.is_empty()
            {
                notice.set(Some("还没有引用：这次只问问题，不发送页面".to_owned()));
            } else if wanted > selected.len() {
                let label = if mode == 0 {
                    "整份文档"
                } else {
                    "已选页面"
                };
                notice.set(Some(format!(
                    "{label}共 {wanted} 页，本次只发送前 {} 页（可在设置里调大）",
                    selected.len()
                )));
            } else {
                notice.set(None);
            }

            for number in &selected {
                context.push((*number, fetch_page_text(*number).await));
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

    Effect::new(move |_| {
        if !manager_open.get() {
            return;
        }
        if let Some(window) = web_sys::window() {
            let here = page.get();
            let schedule = Closure::once_into_js(move || {
                let _ = crate::call("thumbnails", &[]);
                let _ = crate::call(
                    "centerThumb",
                    &[
                        JsValue::from_str("manager-grid"),
                        JsValue::from_f64(f64::from(here)),
                    ],
                );
            });
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                schedule.unchecked_ref(),
                80,
            );
        }
    });

    Effect::new(move |_| {
        let handler =
            Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
                if event.key() == "Escape" && manager_open.get() {
                    manager_open.set(false);
                }
            });
        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
        }
        handler.forget();
    });

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
                    if let Some(anchor) = crate::call("captureStageAnchor", &[]) {
                        let x = js_sys::Reflect::get(&anchor, &JsValue::from_str("x"))
                            .ok()
                            .and_then(|value| value.as_f64());
                        let y = js_sys::Reflect::get(&anchor, &JsValue::from_str("y"))
                            .ok()
                            .and_then(|value| value.as_f64());
                        stage_anchor.set(x.zip(y));
                    }
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
                    let next = (viewport - f64::from(event.client_x())).clamp(320.0, 720.0);
                    width.set(next as u32);
                    if let Some((x, y)) = stage_anchor.get() {
                        let _ = crate::call(
                            "restoreStageAnchor",
                            &[JsValue::from_f64(x), JsValue::from_f64(y)],
                        );
                    }
                }
                on:pointerup=move |_| {
                    dragging.set(false);
                    stage_anchor.set(None);
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
                        .map(|passage| {
                            view! {
                                <div class="ref-row">
                                    <span class="ref-kind">"文"</span>
                                    <span class="ref-text">{passage.quote.clone()}</span>
                                    <span class="ref-page">{format!("p.{}", passage.page)}</span>
                                    <button
                                        class="ref-remove"
                                        title="移除"
                                        on:click=move |_| {
                                            undo_push.run(());
                                            let id = passage.id;
                                            passages.update(|list| list.retain(|entry| entry.id != id));
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
                        .map(|region| {
                            view! {
                                <div class="ref-row">
                                    <img class="ref-thumb" src=region.image alt="" />
                                    <span class="ref-text">"截图"</span>
                                    <span class="ref-page">{format!("p.{}", region.page)}</span>
                                    <button
                                        class="ref-remove"
                                        title="移除"
                                        on:click=move |_| {
                                            undo_push.run(());
                                            let id = region.id;
                                            regions.update(|list| list.retain(|entry| entry.id != id));
                                        }
                                    >
                                        "✕"
                                    </button>
                                </div>
                            }
                        })
                        .collect_view()
                }}
                <div class="scope-row">
                    <div class="scope-icons" role="group" aria-label="提问范围">
                        <button
                            class="scope-icon"
                            class:active=move || scope_mode.get() == 0
                            title=move || format!("整份文档 · {} 页", pages.get())
                            aria-label="整份文档"
                            aria-pressed=move || (scope_mode.get() == 0).to_string()
                            on:click=move |_| {
                                undo_push.run(());
                                scope_mode.set(0);
                            }
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                                <rect x="7" y="3.5" width="10" height="14" rx="1.6" />
                                <path d="M10 20.5h7a2 2 0 0 0 2-2V8" />
                            </svg>
                        </button>
                        <button
                            class="scope-icon"
                            class:active=move || scope_mode.get() == 1
                            title=move || format!("仅当前页 · p.{}", page.get())
                            aria-label="仅当前页"
                            aria-pressed=move || (scope_mode.get() == 1).to_string()
                            on:click=move |_| {
                                undo_push.run(());
                                scope_mode.set(1);
                            }
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M7 3.5h6l4 4v13H7z" />
                                <path d="M13 3.5v4h4" />
                            </svg>
                        </button>
                        <button
                            class="scope-icon"
                            class:active=move || scope_mode.get() == 2
                            title=move || format!("仅引用 · {} 项", reference_count())
                            aria-label="仅引用"
                            aria-pressed=move || (scope_mode.get() == 2).to_string()
                            on:click=move |_| {
                                undo_push.run(());
                                scope_mode.set(2);
                            }
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M4 20.5h16" />
                                <path d="M7.5 16.5l8-9 3 3-8 9h-3z" />
                                <path d="M12.5 6.5l3 3" />
                            </svg>
                            <span class="scope-badge">
                                {move || {
                                    let count = reference_count();
                                    if count == 0 { String::new() } else { count.to_string() }
                                }}
                            </span>
                        </button>
                    </div>
                    <button class="button scope-manage" on:click=move |_| manager_open.set(true)>
                        "管理页面引用"
                    </button>
                </div>
                {move || {
                    notice
                        .get()
                        .map(|message| view! { <p class="range-summary range-error">{message}</p> })
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

            <PageManager
                open=manager_open
                pages=pages
                page=page
                page_set=page_set
                scope_mode=scope_mode
                undo_push=undo_push
            />

            <SettingsSheet
                open=open
                configured=configured
                notify=notify
                model=model
                page_limit=page_limit
            />

        </aside>
    }
}
