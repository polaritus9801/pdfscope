//! One page shell: the box, the reference frames and their pointer handling. Page-range bars,
//! outlines and actions live in the single `page_overlay` overlay.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::{JsCast, JsValue};

use crate::model::{Passage, Region, Selection, rect_style, truncate};
use crate::{call, settle};

#[component]
pub(crate) fn PageView(
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
    focused_ref: RwSignal<Option<(u8, u64)>>,
    edit_epoch: RwSignal<u64>,
    page_set: RwSignal<Vec<u32>>,
    tool: RwSignal<u8>,
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
    let finish_drag = move |id: u64| {
        let moved = drag_moved.get();
        drag.set(None);
        drag_moved.set(false);
        suppress_click.set(moved);
        if !moved {
            return;
        }

        // Only regions use the rectangle drag now; passages are edited by their character grips.
        let rect = regions
            .get()
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.rect);
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

        {
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
        // Capture on the element that was pressed: a grip receives its own move/up events, and the
        // page is found from the tree rather than assumed to be the frame's parent.
        let capture = target.clone();
        let Some(page) = capture.closest(".page").ok().flatten() else {
            return;
        };
        let box_rect = page.get_bounding_client_rect();
        if box_rect.width() <= 0.0 || box_rect.height() <= 0.0 {
            return;
        }
        if let Some(element) = capture.dyn_into::<web_sys::HtmlElement>().ok() {
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
            // The same character span is one reference, however many times it is selected.
            let duplicate = passages.get().iter().any(|passage| {
                passage.page == current.page
                    && passage.start == current.start
                    && passage.end == current.end
                    && current.start < current.end
            });
            if duplicate {
                selection.set(None);
                let _ = call("clearSelection", &[]);
                return;
            }
            undo_push.run(());
            let rects = current.rects.clone();
            // The frame must be exactly the box around the lines it draws, whatever came in the payload.
            let rect = pdfscope_core::geo::union_of(&rects).unwrap_or(current.rect);
            passages.update(|list| {
                list.push(Passage {
                    id: {
                        let id = next_ref.get() + 1;
                        next_ref.set(id);
                        id
                    },
                    page: current.page,
                    quote: current.text.clone(),
                    rect,
                    rects,
                    start: current.start,
                    end: current.end,
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

    let in_scope = move || page_set.get().binary_search(&number).is_ok();
    let rect_of = move |kind: u8, id: u64| -> Option<(f64, f64, f64, f64)> {
        if kind == 0 {
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
        }
    };
    let tight_of = move |id: u64| -> Vec<(f64, f64, f64, f64)> {
        passages
            .get()
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| {
                if entry.rects.is_empty() {
                    vec![entry.rect]
                } else {
                    entry.rects.clone()
                }
            })
            .unwrap_or_default()
    };

    let id = format!("page-{number}");
    let style = format!("aspect-ratio: {width} / {height};");
    let page_attribute = number.to_string();

    let tap_start = RwSignal::new(Option::<(f64, f64)>::None);

    // Which reference, if any, lies under a point, when the pointer tools are not the active one.
    let reference_at = move |client_x: f64, client_y: f64, page: &web_sys::Element| {
        let box_rect = page.get_bounding_client_rect();
        if box_rect.width() <= 0.0 || box_rect.height() <= 0.0 {
            return None;
        }
        let x = (client_x - box_rect.left()) / box_rect.width();
        let y = (client_y - box_rect.top()) / box_rect.height();

        let inside = |rect: (f64, f64, f64, f64)| {
            x >= rect.0 && x <= rect.0 + rect.2 && y >= rect.1 && y <= rect.1 + rect.3
        };

        if let Some(region) = regions
            .get()
            .iter()
            .rev()
            .find(|region| region.page == number && inside(region.rect))
        {
            return Some((1_u8, region.id));
        }
        passages
            .get()
            .iter()
            .rev()
            .find(|passage| passage.page == number && inside(passage.rect))
            .map(|passage| (0_u8, passage.id))
    };

    view! {
        <div
            class="page"
            class:in-scope=in_scope
            id=id
            data-page=page_attribute
            style=style
            on:pointerdown=move |event: web_sys::PointerEvent| {
                tap_start.set(Some((event.client_x() as f64, event.client_y() as f64)));
            }
            on:pointermove=move |event: web_sys::PointerEvent| {
                if !matches!(tool.get(), 1 | 2) {
                    return;
                }
                let Some(page) = event
                    .current_target()
                    .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                else {
                    return;
                };
                active_ref.set(reference_at(
                    event.client_x() as f64,
                    event.client_y() as f64,
                    &page,
                ));
            }
            on:pointerleave=move |_| {
                if focused_ref.get().is_none() {
                    active_ref.set(None);
                }
            }
            on:click=move |event: web_sys::MouseEvent| {
                if !matches!(tool.get(), 1 | 2) {
                    return;
                }
                let Some((start_x, start_y)) = tap_start.get() else {
                    return;
                };
                let moved = (event.client_x() as f64 - start_x).abs()
                    + (event.client_y() as f64 - start_y).abs();
                if moved > 4.0 {
                    return;
                }
                let Some(page) = event
                    .current_target()
                    .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                else {
                    return;
                };
                focused_ref.set(reference_at(
                    event.client_x() as f64,
                    event.client_y() as f64,
                    &page,
                ));
            }
        >
            <div class="page-layer"></div>

            <For
                each=move || {
                    let mut marks = Vec::<(u64, usize)>::new();
                    for passage in passages.get() {
                        if passage.page != number {
                            continue;
                        }
                        let count = if passage.rects.is_empty() {
                            1
                        } else {
                            passage.rects.len()
                        };
                        for index in 0..count {
                            marks.push((passage.id, index));
                        }
                    }
                    marks
                }
                key=|(id, index)| format!("{id}:{index}")
                children=move |(id, index)| {
                    view! {
                        <div
                            class="passage-mark"
                            style=move || {
                                let rect = passages
                                    .get()
                                    .iter()
                                    .find(|entry| entry.id == id)
                                    .map(|entry| {
                                        if entry.rects.is_empty() {
                                            entry.rect
                                        } else {
                                            entry.rects.get(index).copied().unwrap_or(entry.rect)
                                        }
                                    })
                                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                                rect_style(rect)
                            }
                        ></div>
                    }
                }
            />

            {move || {
                let mut frames = Vec::new();
                for passage in passages.get() {
                    if passage.page == number {
                        frames.push((0_u8, passage.id, passage.rect, passage.start, passage.end));
                    }
                }
                for region in regions.get() {
                    if region.page == number {
                        frames.push((1_u8, region.id, region.rect, 0, 0));
                    }
                }
                frames
                    .into_iter()
                    .map(|(kind, id, rect, start, end)| {
                        let grip_style = move |at_end: bool| -> String {
                            let union = rect_of(0, id).unwrap_or(rect);
                            let tight = tight_of(id);
                            let point = if at_end {
                                tight
                                    .last()
                                    .map(|line| (line.0 + line.2, line.1 + line.3 / 2.0))
                            } else {
                                tight.first().map(|line| (line.0, line.1 + line.3 / 2.0))
                            };
                            let Some((x, y)) = point else {
                                return String::new();
                            };
                            let unit = |value: f64, origin: f64, span: f64| {
                                if span <= 0.0 {
                                    0.0
                                } else {
                                    (value - origin) / span * 100.0
                                }
                            };
                            format!(
                                "left:{:.2}%; top:{:.2}%;",
                                unit(x, union.0, union.2),
                                unit(y, union.1, union.3)
                            )
                        };
                        let start_position = move || grip_style(false);
                        let end_position = move || grip_style(true);
                        let is_active = move || {
                            active_ref.get() == Some((kind, id))
                                || focused_ref.get() == Some((kind, id))
                        };
                        let is_focused = move || focused_ref.get() == Some((kind, id));
                        // Only the pointer tool manipulates directly; elsewhere a click focuses first.
                        let editable = move || tool.get() == 0 || is_focused();
                        view! {
                            <div
                                class="ref-frame"
                                data-kind=if kind == 0 { "passage" } else { "region" }
                                data-ref=id
                                class:active=is_active
                                class:focused=is_focused
                                class:editable=editable
                                style=move || rect_style(rect_of(kind, id).unwrap_or(rect))
                                on:mouseenter=move |_| active_ref.set(Some((kind, id)))
                                on:mouseleave=move |_| {
                                    if focused_ref.get() != Some((kind, id)) {
                                        active_ref.set(None);
                                    }
                                }
                                on:pointerdown=move |event: web_sys::PointerEvent| {
                                    if !editable() || kind == 0 {
                                        return;
                                    }
                                    let Some(target) = event
                                        .current_target()
                                        .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
                                    else {
                                        return;
                                    };
                                    let _ = target.set_pointer_capture(event.pointer_id());
                                    begin_drag(event, kind, id, rect_of(kind, id).unwrap_or(rect), 0);
                                }
                                on:pointermove=drag_move
                                on:pointerup=move |_| finish_drag(id)
                                on:pointercancel=move |_| finish_drag(id)
                                on:click=move |_| {
                                    if !editable() {
                                        return;
                                    }
                                    if suppress_click.get() {
                                        suppress_click.set(false);
                                        return;
                                    }
                                    if focused_ref.get() == Some((kind, id)) {
                                        focused_ref.set(None);
                                    } else {
                                        focused_ref.set(Some((kind, id)));
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
                                        focused_ref.set(None);
                                        active_ref.set(None);
                                    }
                                >
                                    "✕"
                                </button>
                                {if kind == 0 {
                                    view! {
                                        <div
                                            class="text-grip grip-start"
                                            style=start_position
                                            title="调整起点"
                                            on:pointerdown=move |event: web_sys::PointerEvent| {
                                                event.stop_propagation();
                                                event.prevent_default();
                                                let _ = call(
                                                    "beginGripDrag",
                                                    &[
                                                        JsValue::from_f64(f64::from(number)),
                                                        JsValue::from_f64(id as f64),
                                                        JsValue::from_f64(start as f64),
                                                        JsValue::from_f64(end as f64),
                                                        JsValue::from_str("start"),
                                                        JsValue::from_f64(event.client_x() as f64),
                                                        JsValue::from_f64(event.client_y() as f64),
                                                    ],
                                                );
                                            }
                                            on:click=move |event: web_sys::MouseEvent| event.stop_propagation()
                                        ></div>
                                        <div
                                            class="text-grip grip-end"
                                            style=end_position
                                            title="调整终点"
                                            on:pointerdown=move |event: web_sys::PointerEvent| {
                                                event.stop_propagation();
                                                event.prevent_default();
                                                let _ = call(
                                                    "beginGripDrag",
                                                    &[
                                                        JsValue::from_f64(f64::from(number)),
                                                        JsValue::from_f64(id as f64),
                                                        JsValue::from_f64(start as f64),
                                                        JsValue::from_f64(end as f64),
                                                        JsValue::from_str("end"),
                                                        JsValue::from_f64(event.client_x() as f64),
                                                        JsValue::from_f64(event.client_y() as f64),
                                                    ],
                                                );
                                            }
                                            on:click=move |event: web_sys::MouseEvent| event.stop_propagation()
                                        ></div>
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
