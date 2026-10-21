//! The UI shell.
//!
//! Leptos owns application state — which page, which zoom, what is selected, what the reference tray
//! holds — and the JavaScript module behind [`call`] owns PDF.js, the canvas and the text layer. The
//! two meet at `window.PDFScope` (calls in) and the `window.__pdfscope*` callbacks (reports out).
//! Page shells are Rust's: the renderer mounts its canvas and text layer into them, so tints and the
//! selection bar keep living in application state.

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

/// One page's size at scale 1, as the renderer measured it.
#[derive(Clone, Copy, PartialEq)]
struct PageSize {
    width: f64,
    height: f64,
}

/// The renderer's page sizes, used to reserve the right height before a canvas exists.
fn parse_page_sizes(value: &JsValue) -> Vec<PageSize> {
    let Some(entries) = value.dyn_ref::<js_sys::Array>() else {
        return Vec::new();
    };

    let mut sizes = Vec::with_capacity(entries.length() as usize);
    for entry in entries.iter() {
        let width = read_number(&entry, "width").unwrap_or(0.0);
        let height = read_number(&entry, "height").unwrap_or(0.0);
        let fallback = sizes.last().copied().unwrap_or(PageSize {
            width: 612.0,
            height: 792.0,
        });
        sizes.push(if width > 0.0 && height > 0.0 {
            PageSize { width, height }
        } else {
            fallback
        });
    }
    sizes
}

/// The CSS position of a normalised rectangle inside its page.
fn rect_style((x, y, width, height): (f64, f64, f64, f64)) -> String {
    format!(
        "left:{:.3}%; top:{:.3}%; width:{:.3}%; height:{:.3}%;",
        x * 100.0,
        y * 100.0,
        width * 100.0,
        height * 100.0,
    )
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

/// One page shell. Rust owns the shell and its overlays; the renderer fills `.page-layer` with a
/// canvas and a text layer only while this page is inside the render window.
#[component]
fn PageView(
    number: u32,
    width: f64,
    height: f64,
    passages: RwSignal<Vec<Passage>>,
    regions: RwSignal<Vec<Region>>,
    selection: RwSignal<Option<Selection>>,
    hovered: RwSignal<Option<(u32, (f64, f64, f64, f64))>>,
) -> impl IntoView {
    let add_reference = Callback::new(move |_: ()| {
        if let Some(current) = selection.get() {
            passages.update(|list| {
                list.push(Passage {
                    page: current.page,
                    quote: current.text.clone(),
                    rect: current.rect,
                });
            });
            selection.set(None);
            let _ = call("clearSelection", &[]);
        }
    });

    let copy_quote = Callback::new(move |_: ()| {
        if let Some(current) = selection.get() {
            let _ = call("copy", &[JsValue::from_str(&current.text)]);
        }
    });

    let id = format!("page-{number}");
    let style = format!("aspect-ratio: {width} / {height};");
    let page_attribute = number.to_string();

    view! {
        <div class="page" id=id data-page=page_attribute style=style>
            <div class="page-layer"></div>

            {move || {
                let mut tints = Vec::new();
                for passage in passages.get() {
                    if passage.page == number {
                        tints.push(passage.rect);
                    }
                }
                for region in regions.get() {
                    if region.page == number {
                        tints.push(region.rect);
                    }
                }
                tints
                    .into_iter()
                    .map(|rect| view! { <div class="ref-tint" style=rect_style(rect)></div> })
                    .collect_view()
            }}

            {move || {
                hovered
                    .get()
                    .filter(|(target, _)| *target == number)
                    .map(|(_, rect)| {
                        view! { <div class="hover-tint" style=rect_style(rect)></div> }
                    })
            }}

            {move || {
                selection
                    .get()
                    .filter(|current| current.page == number)
                    .map(|current| {
                        let (x, y, width, _) = current.rect;
                        let left = (x + width / 2.0) * 100.0;
                        let top = y * 100.0;
                        let style = format!("left: {left:.2}%; top: {top:.2}%;");
                        let page_label = format!("p.{}", current.page);
                        let quote = truncate(&current.text, 26);
                        view! {
                            <div class="selection-anchor" style=style>
                                <div class="selection-bar">
                                    <span class="selection-page">{page_label}</span>
                                    <span class="selection-quote">{quote}</span>
                                    <button
                                        class="button button-primary selection-action"
                                        on:click=move |_| add_reference.run(())
                                    >
                                        "加入引用"
                                    </button>
                                    <button
                                        class="button selection-action"
                                        on:click=move |_| copy_quote.run(())
                                    >
                                        "复制"
                                    </button>
                                </div>
                            </div>
                        }
                    })
            }}
        </div>
    }
}

#[component]
fn App() -> impl IntoView {
    let page = RwSignal::new(1_u32);
    let pages = RwSignal::new(0_u32);
    let page_sizes = RwSignal::new(Vec::<PageSize>::new());
    let zoom = RwSignal::new(100_u32);
    let ready = RwSignal::new(false);
    let file_name = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let settings_open = RwSignal::new(false);
    let reading = RwSignal::new(false);
    // Starts true so the banner does not flash before settings load.
    let configured = RwSignal::new(true);
    let toasts = RwSignal::new(Vec::<(f64, String)>::new());

    // A toast is the app's smallest unit of feedback: one line, three seconds, at most three.
    let notify = Callback::new(move |message: String| {
        let id = js_sys::Date::now();
        toasts.update(|list| {
            list.push((id, message));
            while list.len() > 3 {
                list.remove(0);
            }
        });

        let removal = Closure::once_into_js(move || {
            toasts.update(|list| list.retain(|(toast, _)| *toast != id));
        });

        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                removal.unchecked_ref(),
                3000,
            );
        }
    });
    let outline = RwSignal::new(Vec::<(String, u32, u8)>::new());
    let outline_open = RwSignal::new(false);
    let rail_tab = RwSignal::new(0_u8);

    let schedule_thumbnails = move || {
        if let Some(window) = web_sys::window() {
            let callback = Closure::once_into_js(move || {
                let _ = call("thumbnails", &[]);
            });
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.unchecked_ref(),
                60,
            );
        }
    };
    let find_open = RwSignal::new(false);
    let find_text = RwSignal::new(String::new());
    let find_total = RwSignal::new(0_u32);
    let find_pages = RwSignal::new(Vec::<u32>::new());
    let find_index = RwSignal::new(0_usize);

    let run_find = move || {
        let text = find_text.get();
        if let Some(handle) = call("find", &[JsValue::from_str(&text)]) {
            spawn_local(async move {
                if let Ok(value) = settle(handle).await {
                    let total = js_sys::Reflect::get(&value, &JsValue::from_str("total"))
                        .ok()
                        .and_then(|value| value.as_f64())
                        .unwrap_or(0.0) as u32;
                    let pages = js_sys::Reflect::get(&value, &JsValue::from_str("pages"))
                        .ok()
                        .map(|value| js_sys::Array::from(&value))
                        .map(|array| {
                            array
                                .iter()
                                .filter_map(|entry| entry.as_f64().map(|page| page as u32))
                                .collect::<Vec<u32>>()
                        })
                        .unwrap_or_default();
                    find_total.set(total);
                    find_index.set(0);
                    find_pages.set(pages);
                }
            });
        }
    };

    let step_find = move |direction: i32| {
        let pages = find_pages.get();
        if pages.is_empty() {
            return;
        }
        let count = pages.len() as i32;
        let current = find_index.get() as i32;
        let next = (current + direction).rem_euclid(count) as usize;
        find_index.set(next);
        page.set(pages[next]);
    };
    let selection = RwSignal::new(Option::<Selection>::None);
    let passages = RwSignal::new(Vec::<Passage>::new());
    let regions = RwSignal::new(Vec::<Region>::new());
    let hovered = RwSignal::new(None::<(u32, (f64, f64, f64, f64))>);

    // Explicit navigation dismisses what was selected on the old page. Scroll-driven page updates
    // intentionally do not: the page indicator follows the reader, but the selection stays put.
    let clear_selection = Callback::new(move |_: ()| {
        selection.set(None);
        let _ = call("clearSelection", &[]);
    });

    let navigate = Callback::new(move |target: u32| {
        let total = pages.get();
        if total == 0 {
            return;
        }
        clear_selection.run(());
        page.set(target.clamp(1, total));
    });

    let go = move |delta: i64| {
        let total = pages.get();
        if total == 0 {
            return;
        }
        let next = (i64::from(page.get()) + delta).clamp(1, i64::from(total)) as u32;
        navigate.run(next);
    };

    let zoom_by = move |delta: i64| {
        let _ = call("zoomBy", &[JsValue::from(delta)]);
    };

    let fit_width = move |_| {
        let _ = call("fitWidth", &[]);
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
        pages.set(0);
        page_sizes.set(Vec::new());
        zoom.set(100);
        ready.set(false);
        passages.set(Vec::new());
        regions.set(Vec::new());
        selection.set(None);

        spawn_local(async move {
            let Ok(buffer) = JsFuture::from(file.array_buffer()).await else {
                error.set(Some("这个文件读不出来。".to_owned()));
                return;
            };

            let bytes = js_sys::Uint8Array::new(&buffer);
            let Some(handle) = call("load", &[bytes.into(), file.name().into()]) else {
                error.set(Some("阅读器还没准备好，请重试。".to_owned()));
                return;
            };

            match settle(handle).await {
                Ok(count) => {
                    let count = count.as_f64().unwrap_or(0.0).max(0.0) as u32;
                    pages.set(count);
                    outline.set(Vec::new());
                    outline_open.set(false);

                    if let Some(handle) = call("outline", &[]) {
                        spawn_local(async move {
                            if let Ok(value) = settle(handle).await {
                                let entries = js_sys::Array::from(&value);
                                let mut list = Vec::new();
                                for entry in entries.iter() {
                                    let title =
                                        js_sys::Reflect::get(&entry, &JsValue::from_str("title"))
                                            .ok()
                                            .and_then(|value| value.as_string())
                                            .unwrap_or_default();
                                    let target =
                                        js_sys::Reflect::get(&entry, &JsValue::from_str("page"))
                                            .ok()
                                            .and_then(|value| value.as_f64())
                                            .unwrap_or(0.0)
                                            as u32;
                                    let depth =
                                        js_sys::Reflect::get(&entry, &JsValue::from_str("depth"))
                                            .ok()
                                            .and_then(|value| value.as_f64())
                                            .unwrap_or(0.0)
                                            as u8;
                                    list.push((title, target, depth));
                                }
                                outline.set(list);
                            }
                        });
                    }

                    let mut sizes = Vec::new();
                    if count > 0 {
                        if let Some(value) = call("pageSizes", &[]) {
                            sizes = parse_page_sizes(&value);
                        }
                    }
                    sizes.truncate(count as usize);
                    let fallback = sizes.last().copied().unwrap_or(PageSize {
                        width: 612.0,
                        height: 792.0,
                    });
                    sizes.resize(count as usize, fallback);
                    page_sizes.set(sizes);

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
        if pages.get() == 0 {
            return;
        }
        if let Ok(requested) = input.value().trim().parse::<u32>() {
            navigate.run(requested);
        }
        input.set_value(&page.get().to_string());
    };

    let on_key = move |event: web_sys::KeyboardEvent| {
        let typing = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
            .map(|element| matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA"))
            .unwrap_or(false);
        if typing {
            return;
        }

        if (event.meta_key() || event.ctrl_key())
            && !event.alt_key()
            && event.key().eq_ignore_ascii_case("f")
        {
            event.prevent_default();
            find_open.set(true);
            if let Some(input) = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.get_element_by_id("find-input"))
                .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
            {
                let _ = input.focus();
            }
            return;
        }

        if (event.meta_key() || event.ctrl_key())
            && event.alt_key()
            && event.key().eq_ignore_ascii_case("f")
        {
            event.prevent_default();
            reading.update(|on| *on = !*on);
            return;
        }

        if event.key() == "Escape" && reading.get() {
            reading.set(false);
            return;
        }

        if event.key() == "Escape" && find_open.get() {
            find_open.set(false);
            let _ = call("clearFind", &[]);
            return;
        }

        if (event.meta_key() || event.ctrl_key()) && event.key() == "ArrowUp" {
            navigate.run(1);
            event.prevent_default();
            return;
        }
        if (event.meta_key() || event.ctrl_key()) && event.key() == "ArrowDown" {
            navigate.run(pages.get());
            event.prevent_default();
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
            "+" | "=" => zoom_by(1),
            "-" => zoom_by(-1),
            "0" => fit_width(()),
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

    // The renderer clears the browser selection; the bar must go with it.
    Effect::new(move |_| {
        let callback = Closure::<dyn Fn()>::new(move || selection.set(None));
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopeCleared"),
                callback.as_ref(),
            );
        }
        callback.forget();
    });

    // The renderer owns the transient zoom; Rust only displays the percent it reports.
    Effect::new(move |_| {
        let callback = Closure::<dyn Fn(JsValue)>::new(move |value: JsValue| {
            let Some(reported) = read_number(&value, "percent") else {
                return;
            };
            let next = (reported.round() as u32).clamp(50, 400);
            if zoom.get() != next {
                zoom.set(next);
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopeZoom"),
                callback.as_ref(),
            );
        }
        callback.forget();
    });

    // The renderer reports the page under the reading line as the reader scrolls. Navigation in
    // the other direction (the page field, `‹`, citations) is a signal change that `render` sees.
    Effect::new(move |_| {
        let callback = Closure::<dyn Fn(JsValue)>::new(move |value: JsValue| {
            let Some(reported) = value.as_f64() else {
                return;
            };
            let total = pages.get();
            if total == 0 {
                return;
            }
            let next = (reported.round() as u32).clamp(1, total);
            if page.get() != next {
                page.set(next);
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopePage"),
                callback.as_ref(),
            );
        }
        callback.forget();
    });

    // Scroll the reader whenever the page changes; zoom is owned by the renderer.
    Effect::new(move |_| {
        let requested_page = page.get();
        if !ready.get() {
            return;
        }
        spawn_local(async move {
            if let Some(handle) = call("render", &[JsValue::from(requested_page)]) {
                let _ = settle(handle).await;
            }
        });
    });

    view! {
        <div
            class="app"
            class:reading=move || reading.get()
            id="app-root"
            tabindex="0"
            on:keydown=on_key
        >
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
                        class="button icon-only"
                        title="目录与缩略图"
                        prop:disabled=move || !ready.get()
                        on:click=move |_| outline_open.update(|open| *open = !*open)
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
                            <path d="M4 6h3M10 6h10M4 12h3M10 12h10M4 18h3M10 18h10" />
                        </svg>
                    </button>
                    <button
                        class="button"
                        prop:disabled=move || { !ready.get() || zoom.get() <= 50 }
                        on:click=move |_| zoom_by(-1)
                    >
                        "−"
                    </button>
                    <span class="zoom">{move || format!("{}%", zoom.get())}</span>
                    <button
                        class="button"
                        prop:disabled=move || { !ready.get() || zoom.get() >= 400 }
                        on:click=move |_| zoom_by(1)
                    >
                        "+"
                    </button>
                    <button
                        class="button"
                        prop:disabled=move || !ready.get()
                        on:click=move |_| fit_width(())
                    >
                        "适应宽度"
                    </button>
                    <button
                        class="button"
                        title="设置"
                        on:click=move |_| settings_open.set(true)
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                            <circle cx="12" cy="12" r="3" />
                            <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z" />
                        </svg>
                    </button>
                </div>
            </header>



            <Show when=move || !configured.get()>
                <div class="key-banner">
                    <span>"还没有配置模型，提问前需要填一个 API Key。"</span>
                    <button class="button" on:click=move |_| settings_open.set(true)>
                        "去设置"
                    </button>
                </div>
            </Show>

            <div class="workspace">
                {move || {
                    if !find_open.get() {
                        return ().into_any();
                    }
                    view! {
                        <div class="find-bar">
                            <input
                                id="find-input"
                                class="find-input"
                                type="text"
                                placeholder="查找… 回车"
                                prop:value=move || find_text.get()
                                on:input=move |event| find_text.set(event_target_value(&event))
                                on:keydown=move |event: web_sys::KeyboardEvent| {
                                    if event.key() == "Enter" {
                                        event.prevent_default();
                                        run_find();
                                    }
                                }
                            />
                            <span class="find-count">
                                {move || {
                                    if find_text.get().trim().is_empty() {
                                        String::new()
                                    } else if find_total.get() == 0 {
                                        "没有匹配".to_owned()
                                    } else {
                                        format!(
                                            "{} 处 · {}/{}",
                                            find_total.get(),
                                            find_index.get() + 1,
                                            find_pages.get().len(),
                                        )
                                    }
                                }}
                            </span>
                            <button
                                class="icon-button"
                                title="上一处"
                                prop:disabled=move || find_pages.get().is_empty()
                                on:click=move |_| step_find(-1)
                            >
                                "‹"
                            </button>
                            <button
                                class="icon-button"
                                title="下一处"
                                prop:disabled=move || find_pages.get().is_empty()
                                on:click=move |_| step_find(1)
                            >
                                "›"
                            </button>
                            <button
                                class="icon-button"
                                title="关闭"
                                on:click=move |_| {
                                    find_open.set(false);
                                    let _ = call("clearFind", &[]);
                                }
                            >
                                "✕"
                            </button>
                        </div>
                    }
                        .into_any()
                }}

                <Show when=move || outline_open.get()>
                    <aside class="outline">
                        <div class="outline-head">
                            <div class="rail-tabs">
                                <button
                                    class="rail-tab"
                                    class:active=move || rail_tab.get() == 0
                                    on:click=move |_| rail_tab.set(0)
                                >
                                    "目录"
                                </button>
                                <button
                                    class="rail-tab"
                                    class:active=move || rail_tab.get() == 1
                                    on:click=move |_| {
                                        rail_tab.set(1);
                                        schedule_thumbnails();
                                    }
                                >
                                    "缩略图"
                                </button>
                            </div>
                            <button
                                class="icon-button"
                                title="关闭"
                                on:click=move |_| outline_open.set(false)
                            >
                                "✕"
                            </button>
                        </div>
                        {move || {
                            if rail_tab.get() == 1 {
                                view! {
                                    <div class="thumb-list">
                                        {(1..=pages.get())
                                            .map(|number| {
                                                view! {
                                                    <button
                                                        class="thumb"
                                                        data-page=number
                                                        title=format!("第 {number} 页")
                                                        on:click=move |_| {
                                                            page.set(number);
                                                            outline_open.set(false);
                                                        }
                                                    >
                                                        <span class="thumb-number">{number}</span>
                                                    </button>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                }
                                    .into_any()
                            } else {
                                view! {
                                    <div class="outline-list">
                            {move || {
                                outline
                                    .get()
                                    .is_empty()
                                    .then(|| view! { <p class="outline-empty">"这份文件没有目录"</p> })
                            }}
                            {move || {
                                outline
                                    .get()
                                    .into_iter()
                                    .map(|(title, target, depth)| {
                                        let label = if title.trim().is_empty() {
                                            format!("第 {target} 页")
                                        } else {
                                            title
                                        };
                                        view! {
                                            <button
                                                class="outline-item"
                                                style=format!(
                                                    "padding-left: calc(var(--space-3) + {}px)",
                                                    u32::from(depth) * 12,
                                                )
                                                prop:disabled=target == 0
                                                on:click=move |_| {
                                                    if target > 0 {
                                                        page.set(target);
                                                        outline_open.set(false);
                                                    }
                                                }
                                            >
                                                {label}
                                            </button>
                                        }
                                    })
                                    .collect_view()
                            }}
                                    </div>
                                }
                                    .into_any()
                            }
                        }}
                    </aside>
                </Show>

                <div class="float-tools">
                    <button
                        class="tool"
                        title="文字选择：直接拖选正文"
                        on:click=move |_| {
                            let _ = call("disarmCapture", &[]);
                        }
                    >
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
                            <path d="M4 6h16M4 12h11M4 18h16" />
                        </svg>
                    </button>
                    <button
                        class="tool"
                        title="框选：拖出一个矩形"
                        prop:disabled=move || !ready.get()
                        on:click=move |_| {
                            let _ = call("beginCapture", &[]);
                        }
                    >
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
                        </svg>
                    </button>
                    <button
                        class="tool"
                        title="页面范围：在右侧输入页码"
                        on:click=move |_| {
                            if let Some(input) = web_sys::window()
                                .and_then(|window| window.document())
                                .and_then(|document| document.get_element_by_id("range-input"))
                                .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
                            {
                                let _ = input.focus();
                            }
                        }
                    >
                        <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                            <rect x="4" y="3" width="12" height="16" rx="1.5" />
                            <path d="M8 21h10a2 2 0 0 0 2-2V8" />
                        </svg>
                    </button>
                </div>
            <main
                class="stage"
                on:dblclick=move |_| {
                    if let Some(input) = web_sys::window()
                        .and_then(|window| window.document())
                        .and_then(|document| document.get_element_by_id("file-input"))
                        .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
                    {
                        input.click();
                    }
                }
            >

                <div
                    class="pages-scaler"
                    id="pages-scaler"
                    class:hidden=move || !ready.get()
                >
                    <div class="pages" id="pages">
                    {move || {
                        page_sizes
                            .get()
                            .into_iter()
                            .enumerate()
                            .map(|(index, size)| {
                                view! {
                                    <PageView
                                        number={(index + 1) as u32}
                                        width=size.width
                                        height=size.height
                                        passages=passages
                                        regions=regions
                                        selection=selection
                                        hovered=hovered
                                    />
                                }
                            })
                            .collect_view()
                    }}
                    </div>
                </div>

                <div class="empty" class:hidden=move || ready.get()>
                    <div class="welcome">
                        <p class="welcome-eyebrow">"PDFScope"</p>
                        <h1 class="welcome-title">"边读边问，答案带页码"</h1>
                        <ol class="welcome-steps">
                            <li>
                                <span class="step-index">"1"</span>
                                <div>
                                    <b>"打开一本 PDF"</b>
                                    <small>"⌘O，或把文件拖进来"</small>
                                </div>
                            </li>
                            <li>
                                <span class="step-index">"2"</span>
                                <div>
                                    <b>"选中一段文字"</b>
                                    <small>"也可以用左上角的「框」框住图表"</small>
                                </div>
                            </li>
                            <li>
                                <span class="step-index">"3"</span>
                                <div>
                                    <b>"在右侧提问"</b>
                                    <small>"答案里的 p.31 可以点，会带你回原文"</small>
                                </div>
                            </li>
                        </ol>
                        <button
                            class="button welcome-action"
                            on:click=move |_| {
                                let _ = call("openSample", &[JsValue::from_str("sample.pdf")]);
                            }
                        >
                            "试试示例"
                        </button>
                        <button
                            class="button button-primary welcome-action"
                            on:click=move |_| {
                                if let Some(input) = web_sys::window()
                                    .and_then(|window| window.document())
                                    .and_then(|document| document.get_element_by_id("file-input"))
                                    .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
                                {
                                    input.click();
                                }
                            }
                        >
                            "打开 PDF"
                        </button>
                    </div>
                </div>

                {move || {
                    error
                        .get()
                        .map(|message| {
                            view! { <p class="error">{message}</p> }
                        })
                }}

                <div class="progress-strip" class:hidden=move || !ready.get()>
                    <input
                        class="progress-range"
                        type="range"
                        min="1"
                        max=move || pages.get().max(1).to_string()
                        prop:value=move || page.get().to_string()
                        aria-label="阅读进度"
                        on:input=jump
                    />
                    <span class="progress-count">
                        {move || format!("{} / {}", page.get(), pages.get().max(1))}
                    </span>
                </div>
            </main>

            <Show when=move || reading.get()>
                <button
                    class="reading-exit"
                    title="退出阅读模式（Esc）"
                    on:click=move |_| reading.set(false)
                >
                    "退出阅读模式"
                </button>
            </Show>

            <div class="toasts">
                {move || {
                    toasts
                        .get()
                        .into_iter()
                        .map(|(_, message)| view! { <div class="toast">{message}</div> })
                        .collect_view()
                }}
            </div>

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
                clear_selection=clear_selection
                open=settings_open
                notify=notify
                configured=configured
            />
            </div>
        </div>
    }
}

fn main() {
    leptos::mount::mount_to_body(App);
}
