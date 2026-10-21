//! The UI shell.
//!
//! Leptos owns application state — which page, which zoom, whether a document is open — and the
//! JavaScript module behind [`call`] owns PDF.js and the canvas. The two meet at `window.PDFScope`,
//! which is the only contract between them.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlInputElement;

/// Calls one of the three functions the renderer exposes.
fn call(name: &str, args: &[JsValue]) -> Option<JsValue> {
    let window = web_sys::window()?;
    let scope = js_sys::Reflect::get(&window, &JsValue::from_str("PDFScope")).ok()?;
    let function = js_sys::Reflect::get(&scope, &JsValue::from_str(name)).ok()?;
    let function: js_sys::Function = function.dyn_into().ok()?;

    let arguments = js_sys::Array::new();
    for argument in args {
        arguments.push(argument);
    }

    function.apply(&scope, &arguments).ok()
}

/// Awaits a value the renderer returned as a promise.
async fn settle(value: JsValue) -> Result<JsValue, JsValue> {
    JsFuture::from(js_sys::Promise::from(value)).await
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    fn attribute(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Light => "浅色",
            Self::Dark => "深色",
        }
    }
}

#[component]
fn App() -> impl IntoView {
    let page = RwSignal::new(1_u32);
    let pages = RwSignal::new(0_u32);
    let zoom = RwSignal::new(100_u32);
    let ready = RwSignal::new(false);
    let file_name = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let theme = RwSignal::new(Theme::System);

    let go = move |delta: i64| {
        let total = pages.get();
        if total == 0 {
            return;
        }
        let next = (i64::from(page.get()) + delta).clamp(1, i64::from(total)) as u32;
        page.set(next);
    };

    let zoom_by = move |delta: i64| {
        let next = (i64::from(zoom.get()) + delta).clamp(50, 300) as u32;
        zoom.set(next);
    };

    let open = move |event: web_sys::Event| {
        let Some(input) = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            return;
        };

        file_name.set(file.name());
        error.set(None);

        spawn_local(async move {
            let Ok(buffer) = JsFuture::from(file.array_buffer()).await else {
                error.set(Some("这个文件读不出来。".to_owned()));
                return;
            };

            let bytes = js_sys::Uint8Array::new(&buffer);
            let Some(handle) = call("load", &[bytes.into()]) else {
                error.set(Some("阅读器还没准备好，请重试。".to_owned()));
                return;
            };

            match settle(handle).await {
                Ok(count) => {
                    pages.set(count.as_f64().unwrap_or(0.0) as u32);
                    page.set(1);
                    ready.set(true);
                }
                Err(_) => error.set(Some("这个 PDF 打不开，可能已损坏或被加密。".to_owned())),
            }
        });
    };

    let jump = move |event: web_sys::Event| {
        let Some(input) = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
        else {
            return;
        };
        let total = pages.get();
        if total == 0 {
            return;
        }
        match input.value().trim().parse::<u32>() {
            Ok(requested) => page.set(requested.clamp(1, total)),
            Err(_) => {}
        }
        input.set_value(&page.get().to_string());
    };

    let on_key = move |event: web_sys::KeyboardEvent| {
        let typing = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
            .map(|element| element.tag_name() == "INPUT")
            .unwrap_or(false);
        if typing {
            return;
        }

        if (event.meta_key() || event.ctrl_key()) && event.key().eq_ignore_ascii_case("o") {
            if let Some(input) = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.get_element_by_id("file-input"))
                .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
            {
                input.click();
            }
            event.prevent_default();
            return;
        }

        match event.key().as_str() {
            "ArrowLeft" | "PageUp" => go(-1),
            "ArrowRight" | "PageDown" => go(1),
            "+" | "=" => zoom_by(25),
            "-" => zoom_by(-25),
            "0" => zoom.set(100),
            _ => return,
        }
        event.prevent_default();
    };

    // Repaint whenever the page or the zoom changes, once a document exists.
    Effect::new(move |_| {
        let requested_page = page.get();
        let requested_zoom = zoom.get();
        if !ready.get() {
            return;
        }
        spawn_local(async move {
            if let Some(handle) = call(
                "render",
                &[JsValue::from(requested_page), JsValue::from(requested_zoom)],
            ) {
                let _ = settle(handle).await;
            }
        });
    });

    // Keep the document's theme attribute in step with the signal.
    Effect::new(move |_| {
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            let _ = root.set_attribute("data-theme", theme.get().attribute());
        }
    });

    view! {
        <div class="app" id="app-root" tabindex="0" on:keydown=on_key>
            <header class="bar">
                <div class="bar-left">
                    <span class="brand">"PDFScope"</span>
                    <label class="button button-primary">
                        "打开 PDF"
                        <input
                            id="file-input"
                            class="file-input"
                            type="file"
                            accept="application/pdf"
                            on:change=open
                        />
                    </label>
                    <span class="file-name">{move || file_name.get()}</span>
                </div>

                <div class="bar-center">
                    <button
                        class="button"
                        prop:disabled=move || { page.get() <= 1 }
                        on:click=move |_| go(-1)
                    >
                        "‹"
                    </button>
                    <input
                        class="page-field"
                        type="text"
                        inputmode="numeric"
                        prop:value=move || page.get().to_string()
                        prop:disabled=move || !ready.get()
                        on:change=jump
                    />
                    <span class="page-total">
                        {move || {
                            if pages.get() == 0 { "/ —".to_owned() } else { format!("/ {}", pages.get()) }
                        }}
                    </span>
                    <button
                        class="button"
                        prop:disabled=move || { pages.get() == 0 || page.get() >= pages.get() }
                        on:click=move |_| go(1)
                    >
                        "›"
                    </button>
                </div>

                <div class="bar-right">
                    <button
                        class="button"
                        prop:disabled=move || { zoom.get() <= 50 }
                        on:click=move |_| zoom_by(-25)
                    >
                        "−"
                    </button>
                    <span class="zoom">{move || format!("{}%", zoom.get())}</span>
                    <button
                        class="button"
                        prop:disabled=move || { zoom.get() >= 300 }
                        on:click=move |_| zoom_by(25)
                    >
                        "+"
                    </button>
                    <button class="button" on:click=move |_| zoom.set(100)>
                        "适应宽度"
                    </button>
                    <button
                        class="button"
                        on:click=move |_| theme.set(theme.get().next())
                    >
                        {move || theme.get().label()}
                    </button>
                </div>
            </header>

            <main class="stage">
                <canvas id="page-canvas" class:hidden=move || !ready.get()></canvas>

                <div class="empty" class:hidden=move || ready.get()>
                    <p class="empty-title">"打开一本 PDF"</p>
                    <p class="empty-hint">"⌘O 也可以"</p>
                </div>

                {move || {
                    error
                        .get()
                        .map(|message| {
                            view! { <p class="error">{message}</p> }
                        })
                }}
            </main>
        </div>
    }
}

fn main() {
    leptos::mount::mount_to_body(App);
}
