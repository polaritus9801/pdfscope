//! The AI panel: settings, the question box, and the streamed answer.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use pdfscope_core::{Segment, segments};

use crate::Passage;

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

#[component]
pub fn AiPanel(
    passages: RwSignal<Vec<Passage>>,
    page: RwSignal<u32>,
    pages: RwSignal<u32>,
) -> impl IntoView {
    let configured = RwSignal::new(false);
    let model = RwSignal::new("未配置".to_owned());
    let answer = RwSignal::new(String::new());
    let thinking = RwSignal::new(0_usize);
    let status = RwSignal::new(String::new());
    let streaming = RwSignal::new(false);
    let question = RwSignal::new(String::new());
    let open = RwSignal::new(false);

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
        status.set("连接中…".to_owned());

        let references = js_sys::Array::new();
        for passage in passages.get() {
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

        let started = js_sys::Date::now();
        let callback = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
            let kind = text_of(&event, "kind").unwrap_or_default();
            match kind.as_str() {
                "reasoning" => {
                    if let Some(delta) = text_of(&event, "text") {
                        thinking.update(|count| *count += delta.chars().count());
                    }
                }
                "text" => {
                    if let Some(delta) = text_of(&event, "text") {
                        answer.update(|buffer| buffer.push_str(&delta));
                    }
                }
                "usage" => {
                    let prompt = number_of(&event, "prompt").unwrap_or(0.0) as u64;
                    let completion = number_of(&event, "completion").unwrap_or(0.0) as u64;
                    let cached = number_of(&event, "cached").unwrap_or(0.0) as u64;
                    status.set(format!("输入 {prompt} · 输出 {completion} · 缓存 {cached}"));
                }
                "done" => {
                    streaming.set(false);
                    let elapsed = js_sys::Date::now() - started;
                    if elapsed > 0.0 {
                        let characters = answer.get().chars().count() as f64;
                        // Roughly two characters per token for CJK; labelled as an estimate.
                        let rate = (characters / 2.0) / (elapsed / 1000.0);
                        status.update(|line| line.push_str(&format!(" · ≈ {rate:.0} tok/s")));
                    }
                }
                "error" => {
                    streaming.set(false);
                    status.set(text_of(&event, "message").unwrap_or_else(|| "请求失败".to_owned()));
                }
                _ => {}
            }
        });

        let _ = bridge(
            "ask",
            &[
                JsValue::from_str(&asked),
                references.into(),
                callback.as_ref().clone(),
            ],
        );
        callback.forget();
    };

    let save = move |_| {
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

    view! {
        <aside class="panel">
            <div class="panel-head">
                <span class="panel-title">"AI"</span>
                <span class="panel-model">{move || model.get()}</span>
                <button class="button" on:click=move |_| open.set(true)>
                    "设置"
                </button>
            </div>

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
                                <p class="answer-text">
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
                <textarea
                    class="ask"
                    placeholder="问点什么…"
                    prop:value=move || question.get()
                    on:input=move |event| question.set(event_target_value(&event))
                ></textarea>
                <div class="composer-row">
                    <span class="status">{move || status.get()}</span>
                    <button
                        class="button button-primary"
                        on:click=send
                        prop:disabled=move || streaming.get()
                    >
                        {move || if streaming.get() { "回答中…" } else { "发送" }}
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

                        <div class="sheet-actions">
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
