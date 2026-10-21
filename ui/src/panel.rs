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
) -> impl IntoView {
    let configured = RwSignal::new(false);
    let model = RwSignal::new("未配置".to_owned());
    let answer = RwSignal::new(String::new());
    let thinking = RwSignal::new(0_usize);
    let status = RwSignal::new(String::new());
    let streaming = RwSignal::new(false);
    let question = RwSignal::new(String::new());
    let open = RwSignal::new(false);
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
        page.set(target.clamp(1, total));
        let _ = crate::call("flash", &[]);
    };

    let form_preset = RwSignal::new("MiMo".to_owned());
    let form_protocol = RwSignal::new("openai".to_owned());
    let form_base = RwSignal::new("https://api.xiaomimimo.com/v1".to_owned());
    let form_model = RwSignal::new("mimo-v2.6-flash".to_owned());
    let form_key = RwSignal::new(String::new());
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

    let send = move |_| {
        if streaming.get() {
            return;
        }
        let asked = question.get();
        if asked.trim().is_empty() {
            return;
        }
        if !configured.get() {
            open.set(true);
            return;
        }

        answer.set(String::new());
        thinking.set(0);
        streaming.set(true);
        failed.set(false);
        status.set("准备中…".to_owned());

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
                            thinking.update(|count| *count += delta.chars().count());
                        }
                    }
                    "text" => {
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
                    "usage" => {
                        let prompt = number_of(&event, "prompt").unwrap_or(0.0) as u64;
                        let completion = number_of(&event, "completion").unwrap_or(0.0) as u64;
                        let cached = number_of(&event, "cached").unwrap_or(0.0) as u64;
                        status.set(format!("输入 {prompt} · 输出 {completion} · 缓存 {cached}"));
                    }
                    "stopped" => {
                        flush_pending();
                        thinking.set(0);
                        streaming.set(false);
                        status.set("已停止".to_owned());
                    }
                    "done" => {
                        flush_pending();
                        thinking.set(0);
                        streaming.set(false);
                        let elapsed = js_sys::Date::now() - started;
                        if elapsed > 0.0 {
                            let characters = answer.get().chars().count() as f64;
                            let rate = (characters / 2.0) / (elapsed / 1000.0);
                            status.update(|line| line.push_str(&format!(" · ≈ {rate:.0} tok/s")));
                        }
                    }
                    "error" => {
                        flush_pending();
                        thinking.set(0);
                        streaming.set(false);
                        failed.set(true);
                        status.set(
                            text_of(&event, "message").unwrap_or_else(|| "请求失败".to_owned()),
                        );
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
                    callback.as_ref().clone(),
                ],
            );
            callback.forget();
        });
    };

    let build_payload = move || {
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

        let key = form_key.get();
        if !key.trim().is_empty() {
            let _ = js_sys::Reflect::set(
                &payload,
                &JsValue::from_str("api_key"),
                &JsValue::from_str(&key),
            );
        }

        payload
    };

    let save = move |_| {
        let payload = build_payload();

        if let Some(promise) = bridge("saveSettings", &[payload.into()]) {
            spawn_local(async move {
                match JsFuture::from(js_sys::Promise::from(promise)).await {
                    Ok(_) => {
                        form_error.set(None);
                        open.set(false);
                        configured.set(true);
                        model.set(form_model.get());
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
                    if let Some(target) = event
                        .current_target()
                        .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
                    {
                        let _ = target.set_pointer_capture(event.pointer_id());
                    }
                    dragging.set(true);
                }
                on:pointermove=move |event: web_sys::PointerEvent| {
                    if !dragging.get() {
                        return;
                    }
                    let viewport = web_sys::window()
                        .and_then(|window| window.inner_width().ok())
                        .and_then(|value| value.as_f64())
                        .unwrap_or(1280.0);
                    let next = (viewport - f64::from(event.client_x())).clamp(260.0, 640.0);
                    width.set(next as u32);
                }
                on:pointerup=move |_| dragging.set(false)
            ></div>
            <div class="panel-head">
                <button
                    class="button"
                    title="收起"
                    on:click=move |_| collapsed.set(true)
                >
                    "›"
                </button>
                <span class="panel-title">"AI"</span>
                <span class="panel-model">{move || model.get()}</span>
                <button class="button" on:click=move |_| open.set(true)>
                    "设置"
                </button>
            </div>

            <button
                class="panel-expand"
                title="展开"
                on:click=move |_| collapsed.set(false)
            >
                "‹"
            </button>

            <div class="panel-body">
                {move || {
                    if !configured.get() {
                        view! { <p class="panel-hint">"未配置模型 · 前往设置"</p> }.into_any()
                    } else if answer.get().is_empty() && !streaming.get() {
                        view! { <p class="panel-hint">"选中文字加入引用，然后提问"</p> }.into_any()
                    } else {
                        view! {
                            <div class="answer">
                                {move || {
                                    if thinking.get() > 0 {
                                        view! {
                                            <p class="thinking">
                                                {format!("思考中 · {} 字", thinking.get())}
                                            </p>
                                        }
                                            .into_any()
                                    } else {
                                        ().into_any()
                                    }
                                }}
                                <p class="answer-text" aria-live="polite">
                                    {move || {
                                        segments(&answer.get())
                                            .into_iter()
                                            .map(|segment| match segment {
                                                Segment::Text(text) => {
                                                    view! { <span>{text}</span> }.into_any()
                                                }
                                                Segment::Citation(number) => {
                                                    let known = number >= 1 && number <= pages.get();
                                                    if known {
                                                        view! {
                                                            <button
                                                                class="citation"
                                                                on:click=move |_| jump(number)
                                                            >
                                                                {format!("p.{number}")}
                                                            </button>
                                                        }
                                                            .into_any()
                                                    } else {
                                                        view! { <span>{format!("p.{number}")}</span> }
                                                            .into_any()
                                                    }
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </p>
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
                                                    let total = pages.get();
                                                    if total > 0 {
                                                        page.set(page_number.clamp(1, total));
                                                    }
                                                    let _ = crate::call("flash", &[]);
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
                    on:input=move |event| question.set(event_target_value(&event))
                    on:keydown=move |event: web_sys::KeyboardEvent| {
                        if event.key() == "Enter" && (event.meta_key() || event.ctrl_key()) {
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
                <div class="sheet">
                    <div class="sheet-card">
                        <h2 class="sheet-title">"模型设置"</h2>

                        <label class="field">
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

                        <label class="field">
                            <span>"接口地址"</span>
                            <input
                                type="text"
                                prop:value=move || form_base.get()
                                on:input=move |event| form_base.set(event_target_value(&event))
                            />
                        </label>

                        <label class="field">
                            <span>"模型"</span>
                            <input
                                type="text"
                                prop:value=move || form_model.get()
                                on:input=move |event| form_model.set(event_target_value(&event))
                            />
                        </label>

                        <label class="field">
                            <span>"API Key"</span>
                            <input
                                type="password"
                                placeholder="保存在系统钥匙串"
                                prop:value=move || form_key.get()
                                on:input=move |event| form_key.set(event_target_value(&event))
                            />
                        </label>

                        {move || {
                            form_error
                                .get()
                                .map(|message| view! { <p class="error sheet-error">{message}</p> })
                        }}

                        {move || {
                            test_result
                                .get()
                                .map(|message| view! { <p class="range-summary">{message}</p> })
                        }}

                        <div class="sheet-actions">
                            <button
                                class="button"
                                on:click=test
                                prop:disabled=move || testing.get()
                            >
                                {move || if testing.get() { "测试中…" } else { "测试连接" }}
                            </button>
                            <button class="button" on:click=move |_| open.set(false)>
                                "取消"
                            </button>
                            <button class="button button-primary" on:click=save>
                                "保存"
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
        </aside>
    }
}
