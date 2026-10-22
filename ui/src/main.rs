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
    pub(crate) id: u64,
    pub(crate) page: u32,
    pub(crate) image: String,
    pub(crate) rect: (f64, f64, f64, f64),
}

/// A passage the reader added to the tray.
#[derive(Clone, PartialEq)]
pub(crate) struct Passage {
    pub(crate) id: u64,
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

fn read_flag(value: &JsValue, key: &str) -> Option<bool> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()?
        .as_bool()
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
    next_ref: RwSignal<u64>,
    undo_push: Callback<()>,
    active_ref: RwSignal<Option<(u8, u64)>>,
    pinned_ref: RwSignal<Option<(u8, u64)>>,
    edit_epoch: RwSignal<u64>,
    page_set: RwSignal<Vec<u32>>,
    scope_mode: RwSignal<u8>,
    anchor: RwSignal<Option<(u8, u32)>>,
    hover_page: RwSignal<Option<u32>>,
) -> impl IntoView {
    // kind, id, start x/y, start rect, page width/height, whether undo was recorded
    // kind, id, start x/y, start rect, page w/h, undo recorded, mode (0 move, 1 left, 2 right,
    // 3 top, 4 bottom, 5 top-left, 6 top-right, 7 bottom-left, 8 bottom-right)
    type Drag = (
        u8,
        u64,
        f64,
        f64,
        (f64, f64, f64, f64),
        f64,
        f64,
        bool,
        u8,
        u64,
    );
    let drag = RwSignal::new(Option::<Drag>::None);
    let drag_moved = RwSignal::new(false);
    let suppress_click = RwSignal::new(false);
    let finish_drag = move |kind: u8, id: u64| {
        let moved = drag_moved.get();
        drag.set(None);
        drag_moved.set(false);
        suppress_click.set(moved);
        if !moved {
            return;
        }

        let rect = if kind == 0 {
            passages
                .get()
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| entry.rect)
        } else {
            regions
                .get()
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| entry.rect)
        };
        let Some((x, y, width, height)) = rect else {
            return;
        };
        let epoch = edit_epoch.get() + 1;
        edit_epoch.set(epoch);

        let rect_value = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&rect_value, &JsValue::from_str("x"), &JsValue::from_f64(x));
        let _ = js_sys::Reflect::set(&rect_value, &JsValue::from_str("y"), &JsValue::from_f64(y));
        let _ = js_sys::Reflect::set(
            &rect_value,
            &JsValue::from_str("width"),
            &JsValue::from_f64(width),
        );
        let _ = js_sys::Reflect::set(
            &rect_value,
            &JsValue::from_str("height"),
            &JsValue::from_f64(height),
        );

        if kind == 0 {
            // A quote's text follows its rectangle; re-read it from the text layer.
            let Some(handle) = call(
                "textInRect",
                &[JsValue::from_f64(f64::from(number)), rect_value.into()],
            ) else {
                return;
            };
            spawn_local(async move {
                if let Ok(value) = settle(handle).await {
                    if edit_epoch.get() != epoch {
                        return;
                    }
                    if let Some(text) = value.as_string() {
                        if !text.trim().is_empty() {
                            passages.update(|list| {
                                if let Some(entry) = list.iter_mut().find(|entry| entry.id == id) {
                                    entry.quote = text;
                                }
                            });
                        }
                    }
                }
            });
        } else {
            let Some(handle) = call(
                "regionImage",
                &[JsValue::from_f64(f64::from(number)), rect_value.into()],
            ) else {
                return;
            };
            spawn_local(async move {
                if let Ok(value) = settle(handle).await {
                    if edit_epoch.get() != epoch {
                        return;
                    }
                    if let Some(image) = value.as_string() {
                        if image.starts_with("data:image/") {
                            regions.update(|list| {
                                if let Some(entry) = list.iter_mut().find(|entry| entry.id == id) {
                                    entry.image = image;
                                }
                            });
                        }
                    }
                }
            });
        }
    };

    let begin_drag = move |event: web_sys::PointerEvent,
                           kind: u8,
                           id: u64,
                           start: (f64, f64, f64, f64),
                           mode: u8| {
        let Some(target) = event
            .current_target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        // Capture on the frame, not the handle: one set of move/up handlers serves every handle.
        let frame = if mode == 0 {
            target.clone()
        } else {
            target.parent_element().unwrap_or(target.clone())
        };
        let Some(page) = frame.parent_element() else {
            return;
        };
        let box_rect = page.get_bounding_client_rect();
        if box_rect.width() <= 0.0 || box_rect.height() <= 0.0 {
            return;
        }
        if let Some(element) = frame.dyn_into::<web_sys::HtmlElement>().ok() {
            let _ = element.set_pointer_capture(event.pointer_id());
        }
        drag_moved.set(false);
        drag.set(Some((
            kind,
            id,
            event.client_x() as f64,
            event.client_y() as f64,
            start,
            box_rect.width(),
            box_rect.height(),
            false,
            mode,
            edit_epoch.get(),
        )));
        event.prevent_default();
    };

    let drag_move = move |event: web_sys::PointerEvent| {
        let Some((kind, id, start_x, start_y, start, page_w, page_h, _, mode, epoch)) = drag.get()
        else {
            return;
        };
        if edit_epoch.get() != epoch {
            // An undo or a newer edit happened mid-drag; stop rather than fight it.
            drag.set(None);
            drag_moved.set(false);
            return;
        }

        if !drag_moved.get() {
            let moved = (event.client_x() as f64 - start_x).abs()
                + (event.client_y() as f64 - start_y).abs();
            if moved < 6.0 {
                return;
            }
            undo_push.run(());
            drag_moved.set(true);
        }

        let dx = (event.client_x() as f64 - start_x) / page_w;
        let dy = (event.client_y() as f64 - start_y) / page_h;
        let (x, y, width, height) = pdfscope_core::geo::resize_rect(
            start,
            dx,
            dy,
            pdfscope_core::geo::ResizeMode::from_byte(mode),
            0.01,
        );

        if kind == 0 {
            passages.update(|list| {
                if let Some(entry) = list.iter_mut().find(|entry| entry.id == id) {
                    entry.rect = (x, y, width, height);
                }
            });
        } else {
            regions.update(|list| {
                if let Some(entry) = list.iter_mut().find(|entry| entry.id == id) {
                    entry.rect = (x, y, width, height);
                }
            });
        }
    };

    let add_reference = Callback::new(move |_: ()| {
        if let Some(current) = selection.get() {
            undo_push.run(());
            passages.update(|list| {
                list.push(Passage {
                    id: {
                        let id = next_ref.get() + 1;
                        next_ref.set(id);
                        id
                    },
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

    let in_scope = move || page_set.get().contains(&number);
    let in_preview = move || {
        let Some((kind, endpoint)) = anchor.get() else {
            return false;
        };
        let Some(hovered) = hover_page.get() else {
            return false;
        };
        pdfscope_core::pages::in_preview(number, kind == 0, endpoint, hovered)
    };

    let commit_range = move |from: u32, to: u32| {
        undo_push.run(());
        page_set.update(|set| pdfscope_core::add_range(set, from, to));
        scope_mode.set(3);
    };

    let choose_start = move |_| {
        anchor.set(Some((0, number)));
        hover_page.set(Some(number));
    };
    let choose_end = move |_| {
        match anchor.get() {
            Some((0, start)) => {
                commit_range(start, number);
                anchor.set(None);
            }
            Some((1, end)) => {
                commit_range(end, number);
                anchor.set(None);
            }
            _ => anchor.set(Some((1, number))),
        }
        hover_page.set(None);
    };
    let toggle_page = move |_| {
        undo_push.run(());
        page_set.update(|set| pdfscope_core::toggle_page(set, number));
        scope_mode.set(3);
        anchor.set(None);
    };

    let id = format!("page-{number}");
    let style = format!("aspect-ratio: {width} / {height};");
    let page_attribute = number.to_string();

    view! {
        <div
            class="page"
            class:in-scope=in_scope
            class:preview=in_preview
            id=id
            data-page=page_attribute
            style=style
        >
            <div class="page-layer"></div>

            <div class="page-bar" aria-hidden="true"></div>
            <div
                class="page-bar-actions"
                on:mouseenter=move |_| hover_page.set(Some(number))
                on:mouseleave=move |_| hover_page.set(None)
            >
                <button class="page-bar-btn" title="从这里开始" on:click=choose_start>
                    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M5 12h13M9 6l-6 6 6 6" />
                    </svg>
                </button>
                <button class="page-bar-btn" title="单独这一页" on:click=toggle_page>
                    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="5" y="4" width="14" height="16" rx="1.5" />
                    </svg>
                </button>
                <button class="page-bar-btn" title="到这里结束" on:click=choose_end>
                    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M19 12H6M15 6l6 6-6 6" />
                    </svg>
                </button>
            </div>

            {move || {
                let mut frames = Vec::new();
                for passage in passages.get() {
                    if passage.page == number {
                        frames.push((0_u8, passage.id, passage.rect));
                    }
                }
                for region in regions.get() {
                    if region.page == number {
                        frames.push((1_u8, region.id, region.rect));
                    }
                }
                frames
                    .into_iter()
                    .map(|(kind, id, rect)| {
                        let is_active = move || {
                            active_ref.get() == Some((kind, id))
                                || pinned_ref.get() == Some((kind, id))
                        };
                        let move_handle = (kind == 1).then(|| {
                            view! {
                                <div class="ref-handle ref-handle-move" title="移动">
                                    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                                        <path d="M12 3v18M3 12h18M12 3l-3 3M12 3l3 3M12 21l-3-3M12 21l3-3M3 12l3-3M3 12l3 3M21 12l-3-3M21 12l-3 3" />
                                    </svg>
                                </div>
                            }
                        });
                        view! {
                            <div
                                class="ref-frame"
                                class:active=is_active
                                style=rect_style(rect)
                                on:mouseenter=move |_| active_ref.set(Some((kind, id)))
                                on:mouseleave=move |_| {
                                    if pinned_ref.get() != Some((kind, id)) {
                                        active_ref.set(None);
                                    }
                                }
                                on:pointerdown=move |event: web_sys::PointerEvent| {
                                    let Some(target) = event
                                        .current_target()
                                        .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
                                    else {
                                        return;
                                    };
                                    let _ = target.set_pointer_capture(event.pointer_id());
                                    begin_drag(event, kind, id, rect, 0);
                                }
                                on:pointermove=drag_move
                                on:pointerup=move |_| finish_drag(kind, id)
                                on:pointercancel=move |_| finish_drag(kind, id)
                                on:click=move |_| {
                                    if suppress_click.get() {
                                        suppress_click.set(false);
                                        return;
                                    }
                                    if pinned_ref.get() == Some((kind, id)) {
                                        pinned_ref.set(None);
                                    } else {
                                        pinned_ref.set(Some((kind, id)));
                                    }
                                }
                            >
                                <button
                                    class="ref-handle ref-handle-x"
                                    title="删除引用"
                                    on:pointerdown=move |event: web_sys::PointerEvent| event.stop_propagation()
                                    on:click=move |event: web_sys::MouseEvent| {
                                        event.stop_propagation();
                                        undo_push.run(());
                                        if kind == 0 {
                                            passages.update(|list| list.retain(|entry| entry.id != id));
                                        } else {
                                            regions.update(|list| list.retain(|entry| entry.id != id));
                                        }
                                        pinned_ref.set(None);
                                        active_ref.set(None);
                                    }
                                >
                                    "✕"
                                </button>
                                {if kind == 0 {
                                    view! {
                                        <div
                                            class="ref-handle ref-handle-open ref-handle-left"
                                            title="调整起点"
                                            on:pointerdown=move |event: web_sys::PointerEvent| {
                                                event.stop_propagation();
                                                begin_drag(event, kind, id, rect, 1);
                                            }
                                            on:click=move |event: web_sys::MouseEvent| event.stop_propagation()
                                        >
                                            "["
                                        </div>
                                        <div
                                            class="ref-handle ref-handle-open ref-handle-right"
                                            title="调整终点"
                                            on:pointerdown=move |event: web_sys::PointerEvent| {
                                                event.stop_propagation();
                                                begin_drag(event, kind, id, rect, 2);
                                            }
                                            on:click=move |event: web_sys::MouseEvent| event.stop_propagation()
                                        >
                                            "]"
                                        </div>
                                    }
                                        .into_any()
                                } else {
                                    view! {
                                        <div class="ref-dot dot-nw" on:pointerdown=move |event: web_sys::PointerEvent| { event.stop_propagation(); begin_drag(event, kind, id, rect, 5); }></div>
                                        <div class="ref-dot dot-n" on:pointerdown=move |event: web_sys::PointerEvent| { event.stop_propagation(); begin_drag(event, kind, id, rect, 3); }></div>
                                        <div class="ref-dot dot-ne" on:pointerdown=move |event: web_sys::PointerEvent| { event.stop_propagation(); begin_drag(event, kind, id, rect, 6); }></div>
                                        <div class="ref-dot dot-w" on:pointerdown=move |event: web_sys::PointerEvent| { event.stop_propagation(); begin_drag(event, kind, id, rect, 1); }></div>
                                        <div class="ref-dot dot-e" on:pointerdown=move |event: web_sys::PointerEvent| { event.stop_propagation(); begin_drag(event, kind, id, rect, 2); }></div>
                                        <div class="ref-dot dot-sw" on:pointerdown=move |event: web_sys::PointerEvent| { event.stop_propagation(); begin_drag(event, kind, id, rect, 7); }></div>
                                        <div class="ref-dot dot-s" on:pointerdown=move |event: web_sys::PointerEvent| { event.stop_propagation(); begin_drag(event, kind, id, rect, 4); }></div>
                                        <div class="ref-dot dot-se" on:pointerdown=move |event: web_sys::PointerEvent| { event.stop_propagation(); begin_drag(event, kind, id, rect, 8); }></div>
                                    }
                                        .into_any()
                                }}
                                {move_handle}
                            </div>
                        }
                    })
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
    // 0 pointer, 1 text, 2 box, 3 pages: the rail's active tool.
    let tool = RwSignal::new(0_u8);
    let page_set = RwSignal::new(Vec::<u32>::new());
    // Bumped on every finished edit; an async re-read may only apply if nothing newer happened.
    let edit_epoch = RwSignal::new(0_u64);
    let scope_mode = RwSignal::new(0_u8);
    // kind 0 = start anchor, 1 = end anchor; the hovered page completes the preview.
    let anchor = RwSignal::new(Option::<(u8, u32)>::None);
    let hover_page = RwSignal::new(Option::<u32>::None);
    let active_ref = RwSignal::new(Option::<(u8, u64)>::None);
    let pinned_ref = RwSignal::new(Option::<(u8, u64)>::None);

    let choose_tool = move |next: u8| {
        tool.set(next);
        let name = match next {
            1 => "text",
            2 => "box",
            3 => "pages",
            _ => "pointer",
        };
        let _ = call("setTool", &[JsValue::from_str(name)]);
    };
    // Starts true so the banner does not flash before settings load.
    let configured = RwSignal::new(true);
    let toasts = RwSignal::new(Vec::<(f64, String)>::new());

    // A toast is the app's smallest unit of feedback: one line, three seconds, at most three.
    // Hovering holds it: the timer checks the paused set when it fires, and leaving re-arms it.
    let paused_toasts = RwSignal::new(Vec::<f64>::new());

    let arm = move |id: f64| {
        let removal = Closure::once_into_js(move || {
            if paused_toasts.get().contains(&id) {
                return;
            }
            toasts.update(|list| list.retain(|(toast, _)| *toast != id));
            paused_toasts.update(|list| list.retain(|toast| *toast != id));
        });

        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                removal.unchecked_ref(),
                3000,
            );
        }
    };

    let notify = Callback::new(move |message: String| {
        let id = js_sys::Date::now();
        toasts.update(|list| {
            list.push((id, message));
            while list.len() > 3 {
                list.remove(0);
            }
        });
        arm(id);
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
    let next_ref = RwSignal::new(0_u64);
    type Snapshot = (Vec<Passage>, Vec<Region>, Vec<u32>, u8);
    let history = RwSignal::new(pdfscope_core::history::History::<Snapshot>::new(50));

    let current_state = move || {
        (
            passages.get(),
            regions.get(),
            page_set.get(),
            scope_mode.get(),
        )
    };

    let apply_state = move |state: Snapshot| {
        passages.set(state.0);
        regions.set(state.1);
        page_set.set(state.2);
        scope_mode.set(state.3);
        selection.set(None);
    };

    let undo_push = Callback::new(move |_: ()| {
        let state = current_state();
        history.update(|history| history.push(state));
    });

    let undo_reference = move || {
        edit_epoch.update(|epoch| *epoch += 1);
        let current = current_state();
        let mut stack = history.get();
        if let Some(previous) = stack.undo(current) {
            history.set(stack);
            apply_state(previous);
        }
    };

    let redo_reference = move || {
        edit_epoch.update(|epoch| *epoch += 1);
        let current = current_state();
        let mut stack = history.get();
        if let Some(next) = stack.redo(current) {
            history.set(stack);
            apply_state(next);
        }
    };
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
        // A new document owns a new history and scope; nothing from the old one may be undone into it.
        history.set(pdfscope_core::history::History::new(50));
        next_ref.set(0);
        page_set.set(Vec::new());
        scope_mode.set(0);
        anchor.set(None);
        hover_page.set(None);
        active_ref.set(None);
        pinned_ref.set(None);

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
            .map(|element| matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"))
            .unwrap_or(false);
        if typing || settings_open.get() {
            return;
        }

        if (event.meta_key() || event.ctrl_key()) && event.key().eq_ignore_ascii_case("z") {
            event.prevent_default();
            if event.shift_key() {
                redo_reference();
            } else {
                undo_reference();
            }
            return;
        }

        if !event.meta_key() && !event.ctrl_key() && !event.alt_key() {
            match event.key().to_ascii_lowercase().as_str() {
                "v" => {
                    choose_tool(0);
                    event.prevent_default();
                    return;
                }
                "t" => {
                    choose_tool(1);
                    event.prevent_default();
                    return;
                }
                "b" => {
                    choose_tool(2);
                    event.prevent_default();
                    return;
                }
                "d" => {
                    choose_tool(3);
                    event.prevent_default();
                    return;
                }
                _ => {}
            }
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

        if event.key() == "Escape" && (tool.get() != 0 || anchor.get().is_some()) {
            anchor.set(None);
            hover_page.set(None);
            choose_tool(0);
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
            let Some(parsed) = parse_selection(&value) else {
                return;
            };
            let auto = read_flag(&value, "auto").unwrap_or(false);
            if auto {
                undo_push.run(());
                passages.update(|list| {
                    list.push(Passage {
                        id: {
                            let id = next_ref.get() + 1;
                            next_ref.set(id);
                            id
                        },
                        page: parsed.page,
                        quote: parsed.text.clone(),
                        rect: parsed.rect,
                    })
                });
                selection.set(None);
                let _ = call("clearSelection", &[]);
            } else {
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

    // Page clicks in page mode toggle a page; the set is the document scope.
    Effect::new(move |_| {
        let callback = Closure::<dyn Fn(f64)>::new(move |page: f64| {
            let page = page as u32;
            if page == 0 {
                return;
            }
            undo_push.run(());
            page_set.update(|set| pdfscope_core::toggle_page(set, page));
        });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopeTogglePage"),
                callback.as_ref(),
            );
        }
        callback.forget();
    });

    // A drag across pages adds (or, with Alt, subtracts) the covered range.
    Effect::new(move |_| {
        let callback =
            Closure::<dyn Fn(f64, f64, f64)>::new(move |from: f64, to: f64, subtract: f64| {
                let (from, to) = (from as u32, to as u32);
                if from == 0 || to == 0 {
                    return;
                }
                undo_push.run(());
                let (low, high) = (from.min(to), from.max(to));
                page_set.update(|set| {
                    if subtract != 0.0 {
                        set.retain(|page| *page < low || *page > high);
                    } else {
                        for page in low..=high {
                            if !set.contains(&page) {
                                set.push(page);
                            }
                        }
                        set.sort_unstable();
                    }
                });
                anchor.set(None);
            });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopePageRange"),
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
                undo_push.run(());
                regions.update(|list| {
                    list.push(Region {
                        id: {
                            let id = next_ref.get() + 1;
                            next_ref.set(id);
                            id
                        },
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

    // The renderer reports arming; the rail follows, including when Esc disarms it.
    Effect::new(move |_| {
        let callback = Closure::<dyn Fn(bool)>::new(move |armed: bool| {
            // Arming can be a borrowed gesture (Alt); it must not change the chosen tool.
            if !armed && tool.get() == 2 {
                tool.set(0);
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopeCapture"),
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
                        title="上一页"
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
                        title="下一页"
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
                        title="缩小"
                        prop:disabled=move || { !ready.get() || zoom.get() <= 50 }
                        on:click=move |_| zoom_by(-1)
                    >
                        "−"
                    </button>
                    <span class="zoom">{move || format!("{}%", zoom.get())}</span>
                    <button
                        class="button"
                        title="放大"
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

                <div class="tool-rail">
                    <button
                        class="rail-pointer"
                        class:active=move || tool.get() == 0
                        title="指针：正常阅读与选择（V）"
                        on:click=move |_| choose_tool(0)
                    >
                        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M5 3l6.5 16 2.4-6.1L20 10.5 5 3z" />
                        </svg>
                    </button>
                    <div class="rail-capsule">
                        <button
                            class="rail-tool"
                            class:active=move || tool.get() == 1
                            title="文字：划选即加入引用（T · ⇧ 临时）"
                            prop:disabled=move || !ready.get()
                            on:click=move |_| choose_tool(1)
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
                                <path d="M4 6h16M4 12h11M4 18h16" />
                            </svg>
                        </button>
                        <button
                            class="rail-tool"
                            class:active=move || tool.get() == 2
                            title="框选：拖出矩形（B · ⌥ 临时）"
                            prop:disabled=move || !ready.get()
                            on:click=move |_| choose_tool(2)
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
                            </svg>
                        </button>
                        <button
                            class="rail-tool"
                            class:active=move || tool.get() == 3
                            title="页面：点击或拖动选择页面（D · ⌘ 临时）"
                            prop:disabled=move || !ready.get()
                            on:click=move |_| choose_tool(3)
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                                <rect x="5" y="3" width="12" height="16" rx="1.5" />
                                <path d="M9 21h8a2 2 0 0 0 2-2V8" />
                            </svg>
                        </button>
                    </div>
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
                                        next_ref=next_ref
                                        undo_push=undo_push
                                        active_ref=active_ref
                                        pinned_ref=pinned_ref
                                        edit_epoch=edit_epoch
                                        page_set=page_set
                                        scope_mode=scope_mode
                                        anchor=anchor
                                        hover_page=hover_page
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
                        .map(|(id, message)| {
                            view! {
                                <div
                                    class="toast"
                                    on:mouseenter=move |_| {
                                        paused_toasts.update(|list| {
                                            if !list.contains(&id) {
                                                list.push(id);
                                            }
                                        });
                                    }
                                    on:mouseleave=move |_| {
                                        paused_toasts.update(|list| {
                                            list.retain(|toast| *toast != id);
                                        });
                                        arm(id);
                                    }
                                >
                                    {message}
                                </div>
                            }
                        })
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
                undo_push=undo_push
                reset_history=Callback::new(move |_| {
                    history.set(pdfscope_core::history::History::new(50));
                })
                scope_mode=scope_mode
                page_set=page_set
            />
            </div>
        </div>
    }
}

fn main() {
    leptos::mount::mount_to_body(App);
}
