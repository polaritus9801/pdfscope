//! One page shell: the box, the reference frames, the page bar and their pointer handling.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::{JsCast, JsValue};

use crate::model::{Passage, Region, Selection, rect_style, truncate};
use crate::{call, settle};

/// The visible page-state for a page, combining committed pages with the current draft.
#[derive(Clone, Copy, Default)]
struct PageVisual {
    base: bool,
    add: bool,
    subtract: bool,
    open: Option<(u8, bool)>, // kind (0 start, 1 end), placed?
}

/// Pure preview model for one page. A candidate range is a subtraction only when every
/// page in it is already committed; otherwise the new pages are an addition preview.
fn page_visual(
    page: u32,
    set: &[u32],
    anchor: Option<(u8, u32)>,
    hover_page: Option<u32>,
    hover_kind: Option<u8>,
) -> PageVisual {
    let mut candidate = None;
    let mut open = None;

    match (anchor, hover_page, hover_kind) {
        (None, Some(hovered), Some(kind)) => {
            if kind == 2 {
                candidate = Some((hovered, hovered));
            } else {
                open = Some((kind, hovered, false));
            }
        }
        (Some((anchor_kind, anchor_page)), Some(hovered), Some(kind)) => {
            if kind == 2 {
                candidate = Some((hovered, hovered));
            } else if anchor_kind == kind {
                open = Some((kind, hovered, false));
            } else {
                let (start, end) = if anchor_kind == 0 {
                    (anchor_page, hovered)
                } else {
                    (hovered, anchor_page)
                };
                if start <= end {
                    candidate = Some((start, end));
                } else {
                    open = Some((kind, hovered, false));
                }
            }
        }
        (Some((kind, anchor_page)), _, _) => {
            open = Some((kind, anchor_page, true));
        }
        _ => {}
    }

    let mut visual = PageVisual {
        base: set.binary_search(&page).is_ok(),
        ..PageVisual::default()
    };

    if let Some((start, end)) = candidate {
        if pdfscope_core::pages::range_fully_selected(set, start, end) {
            if page >= start && page <= end {
                visual.base = false;
                visual.subtract = true;
            }
        } else {
            visual.add = page >= start && page <= end && !visual.base;
        }
    }

    if let Some((kind, open_page, placed)) = open {
        if open_page == page {
            visual.open = Some((kind, placed));
        }
    }

    visual
}

fn page_label(page: u32) -> String {
    format!("第 {page} 页")
}

fn page_ranges(set: &[u32]) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for &page in set {
        match ranges.last_mut() {
            Some((_, end)) if *end + 1 == page => *end = page,
            _ => ranges.push((page, page)),
        }
    }
    ranges
}

fn bar_gradient(first: bool, last: bool, top: &str, bottom: &str, body: &str) -> String {
    match (first, last) {
        (true, true) => format!(
            "linear-gradient(to bottom, {top} 0, {body} var(--range-end-fade), {body} calc(100% - var(--range-end-fade)), {bottom} 100%)"
        ),
        (true, false) => format!(
            "linear-gradient(to bottom, {top} 0, {body} var(--range-end-fade), {body} 100%)"
        ),
        (false, true) => {
            format!(
                "linear-gradient(to bottom, {body} 0, {body} calc(100% - var(--range-end-fade)), {bottom} 100%)"
            )
        }
        (false, false) => body.to_owned(),
    }
}

fn read_page_repeat() -> String {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| {
            storage
                .get_item(crate::panel::PAGE_REPEAT_KEY)
                .ok()
                .flatten()
        })
        .filter(|value| value == "cancel")
        .unwrap_or_else(|| "adjust".to_owned())
}

#[component]
fn PageBarActions(
    number: u32,
    side: &'static str,
    hover_page: RwSignal<Option<u32>>,
    hover_kind: RwSignal<Option<u8>>,
    choose_start: Callback<web_sys::MouseEvent>,
    toggle_page: Callback<web_sys::MouseEvent>,
    choose_end: Callback<web_sys::MouseEvent>,
) -> impl IntoView {
    view! {
        <div
            class=format!("page-bar-actions range-{side}")
            on:mouseenter=move |_| hover_page.set(Some(number))
            on:mouseleave=move |_| {
                hover_page.set(None);
                hover_kind.set(None);
            }
        >
            <button
                class="page-bar-btn page-bar-start"
                title="从这里开始"
                aria-label=format!("{}：从这里开始", page_label(number))
                on:mouseenter=move |_| {
                    hover_page.set(Some(number));
                    hover_kind.set(Some(0));
                }
                on:click=move |event| choose_start.run(event)
            >
                <svg viewBox="0 0 1024 1024" width="32" height="32" aria-hidden="true">
                    <path fill="currentColor" d="M855.866698 106.426915c0 23.451108-19.010983 42.463114-42.463114 42.463114L203.435319 148.890029c-23.451108 0-42.463114-19.010983-42.463114-42.463114l0 0c0-23.451108 19.010983-42.463114 42.463114-42.463114l609.968266 0C836.855716 63.963801 855.866698 82.974783 855.866698 106.426915L855.866698 106.426915zM511.826038 264.35069c-23.451108 0-42.463114 19.010983-42.463114 42.463114l0 609.968266c0 23.451108 19.010983 42.463114 42.463114 42.463114l0 0c23.451108 0 42.463114-19.010983 42.463114-42.463114L554.289152 306.812781C554.288129 283.361673 535.277146 264.35069 511.826038 264.35069L511.826038 264.35069zM541.961382 233.023194c-16.582677-16.582677-42.517349-17.534352-57.927317-2.124384L171.253362 543.67849c-15.408945 15.408945-14.458293 41.343617 2.124384 57.926294l0 0c16.582677 16.582677 42.517349 17.534352 57.926294 2.124384L544.085766 290.949488C559.494711 275.540544 558.544059 249.605871 541.961382 233.023194L541.961382 233.023194zM481.696834 233.023194c-16.582677 16.582677-17.534352 42.517349-2.124384 57.926294l312.780703 312.780703c15.408945 15.408945 41.343617 14.458293 57.926294-2.124384l0 0c16.582677-16.582677 17.534352-42.517349 2.124384-57.926294L539.623128 230.897787C524.214183 215.488843 498.27951 216.440517 481.696834 233.023194L481.696834 233.023194z"></path>
                </svg>
            </button>
            <button
                class="page-bar-btn page-bar-single"
                title="单独这一页"
                aria-label=format!("{}：单独这一页", page_label(number))
                on:mouseenter=move |_| {
                    hover_page.set(Some(number));
                    hover_kind.set(Some(2));
                }
                on:click=move |event| toggle_page.run(event)
            >
                <svg viewBox="0 0 1024 1024" width="32" height="32" aria-hidden="true">
                    <path fill="currentColor" d="M896 512c0 212.074667-171.925333 384-384 384S128 724.074667 128 512 299.925333 128 512 128s384 171.925333 384 384zM512 853.333333c188.522667 0 341.333333-152.810667 341.333333-341.333333S700.522667 170.666667 512 170.666667 170.666667 323.477333 170.666667 512s152.810667 341.333333 341.333333 341.333333z"></path>
                    <path fill="currentColor" d="M277.333333 512a21.333333 21.333333 0 0 1 21.333334-21.333333h423.104a21.333333 21.333333 0 1 1 0 42.666666H298.666667a21.333333 21.333333 0 0 1-21.333334-21.333333z"></path>
                    <path fill="currentColor" d="M510.229333 744.896a21.333333 21.333333 0 0 1-21.333333-21.333333V300.437333a21.333333 21.333333 0 0 1 42.666667 0v423.125334a21.333333 21.333333 0 0 1-21.333334 21.333333z"></path>
                </svg>
            </button>
            <button
                class="page-bar-btn page-bar-end"
                title="到这里结束"
                aria-label=format!("{}：到这里结束", page_label(number))
                on:mouseenter=move |_| {
                    hover_page.set(Some(number));
                    hover_kind.set(Some(1));
                }
                on:click=move |event| choose_end.run(event)
            >
                <svg viewBox="0 0 1024 1024" width="32" height="32" aria-hidden="true">
                    <path fill="currentColor" d="M855.866698 106.426915c0 23.451108-19.010983 42.463114-42.463114 42.463114L203.435319 148.890029c-23.451108 0-42.463114-19.010983-42.463114-42.463114l0 0c0-23.451108 19.010983-42.463114 42.463114-42.463114l609.968266 0C836.855716 63.963801 855.866698 82.974783 855.866698 106.426915L855.866698 106.426915zM511.826038 264.35069c-23.451108 0-42.463114 19.010983-42.463114 42.463114l0 609.968266c0 23.451108 19.010983 42.463114 42.463114 42.463114l0 0c23.451108 0 42.463114-19.010983 42.463114-42.463114L554.289152 306.812781C554.288129 283.361673 535.277146 264.35069 511.826038 264.35069L511.826038 264.35069zM541.961382 233.023194c-16.582677-16.582677-42.517349-17.534352-57.927317-2.124384L171.253362 543.67849c-15.408945 15.408945-14.458293 41.343617 2.124384 57.926294l0 0c16.582677 16.582677 42.517349 17.534352 57.926294 2.124384L544.085766 290.949488C559.494711 275.540544 558.544059 249.605871 541.961382 233.023194L541.961382 233.023194zM481.696834 233.023194c-16.582677 16.582677-17.534352 42.517349-2.124384 57.926294l312.780703 312.780703c15.408945 15.408945 41.343617 14.458293 57.926294-2.124384l0 0c16.582677-16.582677 17.534352-42.517349 2.124384-57.926294L539.623128 230.897787C524.214183 215.488843 498.27951 216.440517 481.696834 233.023194L481.696834 233.023194z"></path>
                </svg>
            </button>
        </div>
    }
}

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
    scope_mode: RwSignal<u8>,
    tool: RwSignal<u8>,
    anchor: RwSignal<Option<(u8, u32)>>,
    hover_page: RwSignal<Option<u32>>,
    hover_kind: RwSignal<Option<u8>>,
    total_pages: RwSignal<u32>,
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

    let in_scope = move || {
        page_visual(
            number,
            &page_set.get(),
            anchor.get(),
            hover_page.get(),
            hover_kind.get(),
        )
        .base
    };
    let in_preview = move || {
        let visual = page_visual(
            number,
            &page_set.get(),
            anchor.get(),
            hover_page.get(),
            hover_kind.get(),
        );
        visual.add || (visual.open.is_some() && !visual.base)
    };

    let removing = move || {
        page_visual(
            number,
            &page_set.get(),
            anchor.get(),
            hover_page.get(),
            hover_kind.get(),
        )
        .subtract
    };

    let clear_draft = move || {
        anchor.set(None);
        hover_page.set(None);
        hover_kind.set(None);
    };

    let adjust_same_endpoint = move |kind: u8| {
        let current = page_set.get_untracked();
        let mut next = current.clone();
        if let Some((range_start, range_end)) =
            pdfscope_core::pages::range_containing(&current, number)
        {
            if kind == 1 {
                if number < range_end {
                    pdfscope_core::pages::subtract_range(&mut next, number + 1, range_end);
                }
            } else if number > range_start {
                pdfscope_core::pages::subtract_range(&mut next, range_start, number - 1);
            }
        } else {
            let ranges = page_ranges(&current);
            if kind == 1 {
                let nearest = ranges
                    .iter()
                    .filter(|(_, end)| *end < number)
                    .max_by_key(|(_, end)| *end)
                    .copied();
                match nearest {
                    Some((_, end)) => pdfscope_core::pages::add_range(&mut next, end + 1, number),
                    None => pdfscope_core::pages::add_range(&mut next, 1, number),
                }
            } else {
                let nearest = ranges
                    .iter()
                    .filter(|(start, _)| *start > number)
                    .min_by_key(|(start, _)| *start)
                    .copied();
                match nearest {
                    Some((start, _)) => {
                        pdfscope_core::pages::add_range(&mut next, number, start - 1)
                    }
                    None => pdfscope_core::pages::add_range(
                        &mut next,
                        number,
                        total_pages.get_untracked(),
                    ),
                }
            }
        }
        if next != current {
            undo_push.run(());
            page_set.set(next);
            scope_mode.set(2);
        }
        clear_draft();
    };

    let choose_endpoint = move |kind: u8| {
        match anchor.get_untracked() {
            None => {
                anchor.set(Some((kind, number)));
                hover_page.set(None);
                hover_kind.set(None);
            }
            Some((anchor_kind, anchor_page)) => {
                if anchor_kind == kind {
                    if read_page_repeat() == "cancel" {
                        clear_draft();
                    } else {
                        adjust_same_endpoint(kind);
                    }
                } else {
                    let (start, end) = if anchor_kind == 0 {
                        (anchor_page, number)
                    } else {
                        (number, anchor_page)
                    };
                    if start > end {
                        // The pair is illegal: discard the first click and keep this one as the anchor.
                        anchor.set(Some((kind, number)));
                        hover_page.set(None);
                        hover_kind.set(None);
                        return;
                    }
                    let current = page_set.get_untracked();
                    let mut next = current.clone();
                    if pdfscope_core::pages::range_fully_selected(&current, start, end) {
                        pdfscope_core::pages::subtract_range(&mut next, start, end);
                    } else {
                        pdfscope_core::pages::add_range(&mut next, start, end);
                    }
                    if next != current {
                        undo_push.run(());
                        page_set.set(next);
                        scope_mode.set(2);
                    }
                    clear_draft();
                }
            }
        }
    };

    let choose_start = Callback::new(move |_: web_sys::MouseEvent| choose_endpoint(0));
    let choose_end = Callback::new(move |_: web_sys::MouseEvent| choose_endpoint(1));

    let toggle_page = Callback::new(move |_: web_sys::MouseEvent| {
        let current = page_set.get_untracked();
        let mut next = current.clone();
        pdfscope_core::pages::toggle_page(&mut next, number);
        if next != current {
            undo_push.run(());
            page_set.set(next);
            scope_mode.set(2);
        }
        clear_draft();
    });

    let bar = move || -> (String, String) {
        let set = page_set.get();
        let anchor = anchor.get();
        let hovered = hover_page.get();
        let kind = hover_kind.get();
        let visual = page_visual(number, &set, anchor, hovered, kind);
        if visual.subtract {
            let first = !page_visual(number - 1, &set, anchor, hovered, kind).subtract;
            let last = !page_visual(number + 1, &set, anchor, hovered, kind).subtract;
            let class = match (first, last) {
                (true, true) => "page-range-bar visible ghost first last",
                (true, false) => "page-range-bar visible ghost first",
                (false, true) => "page-range-bar visible ghost last",
                (false, false) => "page-range-bar visible ghost",
            };
            return (class.to_owned(), String::new());
        }
        if let Some((open_kind, placed)) = visual.open {
            let cap = if placed {
                "var(--deep-border)"
            } else {
                "var(--light-border)"
            };
            let background = if open_kind == 0 {
                format!(
                    "linear-gradient(to bottom, {cap} 0, var(--light-body) var(--range-end-fade), rgb(220 207 243 / 0) 100%)"
                )
            } else {
                format!(
                    "linear-gradient(to bottom, rgb(220 207 243 / 0) 0, var(--light-body) calc(100% - var(--range-end-fade)), {cap} 100%)"
                )
            };
            return (
                "page-range-bar visible open last".to_owned(),
                format!("background: {background};"),
            );
        }

        if visual.add {
            let first = !page_visual(number - 1, &set, anchor, hovered, kind).add || number == 1;
            let last = !page_visual(number + 1, &set, anchor, hovered, kind).add;
            // The placed endpoint can be the first selected page or the last one: a reverse
            // start/end pair is legal, so the cap follows the anchor page, not the button label.
            let placed = anchor.map(|(_, page)| page == number).unwrap_or(false);
            let top = if first && placed {
                "var(--deep-border)"
            } else {
                "var(--light-border)"
            };
            let bottom = if last && placed {
                "var(--deep-border)"
            } else {
                "var(--light-border)"
            };
            let background = bar_gradient(first, last, top, bottom, "var(--light-body)");
            let class = if last {
                "page-range-bar visible add last"
            } else {
                "page-range-bar visible add"
            };
            return (class.to_owned(), format!("background: {background};"));
        }

        if visual.base {
            let first = !page_visual(number - 1, &set, anchor, hovered, kind).base || number == 1;
            let last = !page_visual(number + 1, &set, anchor, hovered, kind).base;
            let background = bar_gradient(
                first,
                last,
                "var(--deep-border)",
                "var(--deep-border)",
                "var(--deep-body)",
            );
            let class = if last {
                "page-range-bar visible base last"
            } else {
                "page-range-bar visible base"
            };
            return (class.to_owned(), format!("background: {background};"));
        }

        ("page-range-bar".to_owned(), String::new())
    };

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
            class:preview=in_preview
            class:removing=removing
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
                if event
                    .target()
                    .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                    .and_then(|element| element.closest(".page-bar-actions, .page-range-bar").ok().flatten())
                    .is_some()
                {
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

            <div
                class=move || format!("{} range-left", bar().0)
                style=move || bar().1
                aria-hidden="true"
            ></div>
            <div
                class=move || format!("{} range-right", bar().0)
                style=move || bar().1
                aria-hidden="true"
            ></div>
            <PageBarActions
                number=number
                side="left"
                hover_page=hover_page
                hover_kind=hover_kind
                choose_start=choose_start
                toggle_page=toggle_page
                choose_end=choose_end
            />
            <PageBarActions
                number=number
                side="right"
                hover_page=hover_page
                hover_kind=hover_kind
                choose_start=choose_start
                toggle_page=toggle_page
                choose_end=choose_end
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
