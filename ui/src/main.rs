//! The UI shell.
//!
//! Leptos owns application state — which page, which zoom, what is selected, what the reference tray
//! holds — and the JavaScript module behind [`call`] owns PDF.js, the canvas and the text layer. The
//! two meet at `window.PDFScope` (calls in) and `window.__pdfscopeSelection` (one report out).

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlInputElement;

mod panel;

/// Calls one of the functions the renderer exposes.
pub(crate) fn call(name: &str, args: &[JsValue]) -> Option<JsValue> {
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

/// What the renderer reported after a drag: a page, a rectangle and the words.
#[derive(Clone, PartialEq)]
struct Selection {
    page: u32,
    rect: (f64, f64, f64, f64),
    text: String,
}

/// A rectangle the reader captured, as a PNG data URL.
#[derive(Clone, PartialEq)]
pub(crate) struct Region {
    pub(crate) page: u32,
    pub(crate) image: String,
    pub(crate) rect: (f64, f64, f64, f64),
}

/// A passage the reader added to the tray.
#[derive(Clone, PartialEq)]
pub(crate) struct Passage {
    pub(crate) page: u32,
    pub(crate) quote: String,
    pub(crate) rect: (f64, f64, f64, f64),
}

fn read_number(value: &JsValue, key: &str) -> Option<f64> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()?
        .as_f64()
}

fn read_string(value: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()?
        .as_string()
}

/// Rejects a payload that does not describe a real selection on a page.
fn parse_selection(value: &JsValue) -> Option<Selection> {
    let page = read_number(value, "page")? as u32;
    let x = read_number(value, "x")?;
    let y = read_number(value, "y")?;
    let width = read_number(value, "width")?;
    let height = read_number(value, "height")?;
    let text = read_string(value, "text")?;

    let in_unit = |n: f64| (0.0..=1.0).contains(&n);
    if page == 0 || !in_unit(x) || !in_unit(y) || !in_unit(width) || !in_unit(height) {
        return None;
    }
    if width <= 0.0 || height <= 0.0 || text.trim().is_empty() {
        return None;
    }

    Some(Selection {
        page,
        rect: (x, y, width, height),
        text,
    })
}

/// Shortens a quote for a chip or a bar without cutting a multi-byte character.
pub(crate) fn truncate(text: &str, maximum: usize) -> String {
    let mut characters = text.chars();
    let prefix: String = characters.by_ref().take(maximum).collect();
    if characters.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
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
    let selection = RwSignal::new(Option::<Selection>::None);
    let passages = RwSignal::new(Vec::<Passage>::new());
    let regions = RwSignal::new(Vec::<Region>::new());
    let hovered = RwSignal::new(None::<(u32, (f64, f64, f64, f64))>);

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
        passages.set(Vec::new());
        regions.set(Vec::new());
        selection.set(None);

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
        if let Ok(requested) = input.value().trim().parse::<u32>() {
            page.set(requested.clamp(1, total));
        }
        input.set_value(&page.get().to_string());
    };

    let add_reference = move |_| {
        if let Some(current) = selection.get() {
            passages.update(|list| {
                list.push(Passage {
                    page: current.page,
                    quote: current.text.clone(),
                    rect: current.rect,
                });
            });
            selection.set(None);
            call("clearSelection", &[]);
        }
    };

    let copy_quote = move |_| {
        if let Some(current) = selection.get() {
            call("copy", &[JsValue::from_str(&current.text)]);
        }
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
            "Escape" => {
                selection.set(None);
                call("clearSelection", &[]);
            }
            _ => return,
        }
        event.prevent_default();
    };

    // The renderer reports a selection through one global callback, registered once.
    Effect::new(move |_| {
        let callback = Closure::<dyn Fn(JsValue)>::new(move |value: JsValue| {
            if let Some(parsed) = parse_selection(&value) {
                selection.set(Some(parsed));
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopeSelection"),
                callback.as_ref(),
            );
        }
        callback.forget();
    });

    // The renderer reports a captured region the same way it reports a selection.
    Effect::new(move |_| {
        let callback = Closure::<dyn Fn(JsValue)>::new(move |value: JsValue| {
            let page = read_number(&value, "page").unwrap_or(0.0) as u32;
            let image = read_string(&value, "image").unwrap_or_default();
            let x = read_number(&value, "x").unwrap_or(0.0);
            let y = read_number(&value, "y").unwrap_or(0.0);
            let width = read_number(&value, "width").unwrap_or(0.0);
            let height = read_number(&value, "height").unwrap_or(0.0);
            if page == 0 {
                return;
            }
            if image.starts_with("data:image/") {
                regions.update(|list| {
                    list.push(Region {
                        page,
                        image,
                        rect: (x, y, width, height),
                    })
                });
            } else {
                error.set(Some("这一块没截下来。".to_owned()));
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopeRegion"),
                callback.as_ref(),
            );
        }
        callback.forget();
    });

    // A page change invalidates whatever was selected on the previous page.
    Effect::new(move |_| {
        let _ = page.get();
        selection.set(None);
    });

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
                    <button
                        class="button"
                        prop:disabled=move || !ready.get()
                        on:click=move |_| {
                            let _ = call("beginCapture", &[]);
                        }
                    >
                        "框选"
                    </button>
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

            <section
                class="tray"
                class:hidden=move || { passages.get().is_empty() && regions.get().is_empty() }
            >
                <span class="tray-label">
                    {move || format!("引用 {}", passages.get().len())}
                </span>
                <div class="chips">
                    {move || {
                        regions
                            .get()
                            .into_iter()
                            .enumerate()
                            .map(|(index, region)| {
                                view! {
                                    <span class="chip chip-region">
                                        <img class="chip-thumb" src=region.image alt="" />
                                        <span class="chip-label">
                                            {format!("p.{}", region.page)}
                                        </span>
                                        <button
                                            class="chip-remove"
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
                                    </span>
                                }
                            })
                            .collect_view()
                    }}
                    {move || {
                        passages
                            .get()
                            .into_iter()
                            .enumerate()
                            .map(|(index, passage)| {
                                let label = format!(
                                    "p.{} · {}",
                                    passage.page,
                                    truncate(&passage.quote, 18),
                                );
                                view! {
                                    <span class="chip">
                                        <span class="chip-label">{label}</span>
                                        <button
                                            class="chip-remove"
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
                                    </span>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </section>

            <div class="workspace">
            <main class="stage">
                <div class="page" id="page" class:hidden=move || !ready.get()>
                    <canvas id="page-canvas"></canvas>
                    <div id="text-layer" class="textLayer"></div>

                    {move || {
                        hovered
                            .get()
                            .filter(|(target, _)| *target == page.get())
                            .map(|(_, (x, y, width, height))| {
                                let style = format!(
                                    "left:{:.3}%; top:{:.3}%; width:{:.3}%; height:{:.3}%;",
                                    x * 100.0,
                                    y * 100.0,
                                    width * 100.0,
                                    height * 100.0,
                                );
                                view! { <div class="hover-tint" style=style></div> }
                            })
                    }}

                    {move || {
                        selection
                            .get()
                            .map(|current| {
                                let (x, y, width, _) = current.rect;
                                let left = (x + width / 2.0) * 100.0;
                                let top = y * 100.0;
                                let style = format!("left: {left:.2}%; top: {top:.2}%;");
                                let page_label = format!("p.{}", current.page);
                                let quote = truncate(&current.text, 26);
                                view! {
                                    <div class="selection-bar" style=style>
                                        <span class="selection-page">{page_label}</span>
                                        <span class="selection-quote">{quote}</span>
                                        <button
                                            class="button button-primary selection-action"
                                            on:click=add_reference
                                        >
                                            "加入引用"
                                        </button>
                                        <button
                                            class="button selection-action"
                                            on:click=copy_quote
                                        >
                                            "复制"
                                        </button>
                                    </div>
                                }
                            })
                    }}
                </div>

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

            <div class="rail">
                {move || {
                    hovered
                        .get()
                        .map(|(target, _)| {
                            let total = pages.get().max(1) as f64;
                            let top = (target as f64 - 0.5) / total * 100.0;
                            view! {
                                <div
                                    class="rail-marker"
                                    style=format!("top:{top:.2}%")
                                ></div>
                            }
                        })
                }}
            </div>

            <panel::AiPanel
                passages=passages
                regions=regions
                page=page
                pages=pages
                hovered=hovered
            />
            </div>
        </div>
    }
}

fn main() {
    leptos::mount::mount_to_body(App);
}
