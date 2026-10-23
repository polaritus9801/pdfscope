//! One page shell: the box, the reference frames, the page bar and their pointer handling.

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
        scope_mode.set(2);
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
        scope_mode.set(2);
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
                                data-kind=if kind == 0 { "passage" } else { "region" }
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
