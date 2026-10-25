//! One overlay for every page-range bar and page-action toolbar.
//!
//! The page shells carry only content and the committed `.in-scope` class. This module owns the
//! transient preview outlines, the range bars and the start/single/end affordances, so hovering a
//! document costs the same whether it has ten pages or three thousand.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::panel::PAGE_REPEAT_KEY;

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub(crate) struct PageMetric {
    #[serde(default)]
    pub top: f64,
    #[serde(default)]
    pub height: f64,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub(crate) struct PageGeometry {
    #[serde(default)]
    pub pages: Vec<PageMetric>,
}

#[derive(Clone, PartialEq)]
struct BarSpec {
    top: f64,
    height: f64,
    class: String,
    style: String,
}

#[derive(Clone, Copy, PartialEq)]
struct OutlineSpec {
    top: f64,
    height: f64,
    preview: bool,
    subtract: bool,
}

#[derive(Default)]
struct Draft {
    candidate: Option<(u32, u32)>,
    subtract: bool,
    open: Option<(u8, u32, bool)>,
}

fn page_ranges(set: &[u32]) -> Vec<(u32, u32)> {
    let mut ranges = Vec::new();
    for &page in set {
        match ranges.last_mut() {
            Some((_, end)) if *end + 1 == page => *end = page,
            _ => ranges.push((page, page)),
        }
    }
    ranges
}

fn subtract_interval(ranges: &[(u32, u32)], start: u32, end: u32) -> Vec<(u32, u32)> {
    let mut result = Vec::new();
    for &(from, to) in ranges {
        if end < from || start > to {
            result.push((from, to));
            continue;
        }
        if from < start {
            result.push((from, start - 1));
        }
        if to > end {
            result.push((end + 1, to));
        }
    }
    result
}

fn add_segments(start: u32, end: u32, set: &[u32]) -> Vec<(u32, u32)> {
    let mut result = Vec::new();
    let mut segment = None;
    for page in start..=end {
        if set.binary_search(&page).is_ok() {
            if let Some((from, to)) = segment.take() {
                result.push((from, to));
            }
        } else {
            segment = match segment {
                Some((from, _)) => Some((from, page)),
                None => Some((page, page)),
            };
        }
    }
    if let Some(segment) = segment {
        result.push(segment);
    }
    result
}

fn bar_gradient(first: bool, last: bool, top: &str, bottom: &str, body: &str) -> String {
    match (first, last) {
        (true, true) => format!(
            "linear-gradient(to bottom, {top} 0, {body} var(--range-end-fade), {body} calc(100% - var(--range-end-fade)), {bottom} 100%)"
        ),
        (true, false) => format!(
            "linear-gradient(to bottom, {top} 0, {body} var(--range-end-fade), {body} 100%)"
        ),
        (false, true) => format!(
            "linear-gradient(to bottom, {body} 0, {body} calc(100% - var(--range-end-fade)), {bottom} 100%)"
        ),
        (false, false) => body.to_owned(),
    }
}

fn open_gradient(kind: u8, placed: bool) -> String {
    let cap = if placed {
        "var(--deep-border)"
    } else {
        "var(--light-border)"
    };
    if kind == 0 {
        format!(
            "linear-gradient(to bottom, {cap} 0, var(--light-body) var(--range-end-fade), rgb(220 207 243 / 0) 100%)"
        )
    } else {
        format!(
            "linear-gradient(to bottom, rgb(220 207 243 / 0) 0, var(--light-body) calc(100% - var(--range-end-fade)), {cap} 100%)"
        )
    }
}

fn page_span(layout: &PageGeometry, start: u32, end: u32) -> Option<(f64, f64)> {
    let first = layout.pages.get(start.checked_sub(1)? as usize)?;
    let last = layout.pages.get(end.checked_sub(1)? as usize)?;
    Some((first.top, (last.top + last.height - first.top).max(1.0)))
}

fn draft(
    anchor: Option<(u8, u32)>,
    hover_page: Option<u32>,
    hover_kind: Option<u8>,
    set: &[u32],
) -> Draft {
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
        (Some((kind, anchor_page)), _, _) => open = Some((kind, anchor_page, true)),
        _ => {}
    }

    let subtract = candidate
        .map(|(start, end)| pdfscope_core::pages::range_fully_selected(set, start, end))
        .unwrap_or(false);
    Draft {
        candidate,
        subtract,
        open,
    }
}

fn page_label(page: u32) -> String {
    format!("第 {page} 页")
}

fn read_page_repeat() -> String {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(PAGE_REPEAT_KEY).ok().flatten())
        .filter(|value| value == "cancel")
        .unwrap_or_else(|| "adjust".to_owned())
}

fn clear_draft(
    anchor: RwSignal<Option<(u8, u32)>>,
    hover_page: RwSignal<Option<u32>>,
    hover_kind: RwSignal<Option<u8>>,
    hover_side: RwSignal<Option<u8>>,
) {
    anchor.set(None);
    hover_page.set(None);
    hover_kind.set(None);
    hover_side.set(None);
}

fn adjust_same_endpoint(
    page: u32,
    kind: u8,
    page_set: RwSignal<Vec<u32>>,
    undo_push: Callback<()>,
    scope_mode: RwSignal<u8>,
) {
    let current = page_set.get_untracked();
    let mut next = current.clone();
    if let Some((range_start, range_end)) = pdfscope_core::pages::range_containing(&current, page) {
        if kind == 1 {
            if page < range_end {
                pdfscope_core::pages::subtract_range(&mut next, page + 1, range_end);
            }
        } else if page > range_start {
            pdfscope_core::pages::subtract_range(&mut next, range_start, page - 1);
        }
    } else {
        let ranges = page_ranges(&current);
        if kind == 1 {
            let nearest = ranges
                .iter()
                .filter(|(_, end)| *end < page)
                .max_by_key(|(_, end)| *end)
                .copied();
            match nearest {
                Some((_, end)) => pdfscope_core::pages::add_range(&mut next, end + 1, page),
                None => pdfscope_core::pages::add_range(&mut next, 1, page),
            }
        } else {
            let nearest = ranges
                .iter()
                .filter(|(start, _)| *start > page)
                .min_by_key(|(start, _)| *start)
                .copied();
            match nearest {
                Some((start, _)) => {
                    pdfscope_core::pages::add_range(&mut next, page, start.saturating_sub(1))
                }
                None => pdfscope_core::pages::add_range(
                    &mut next,
                    page,
                    page_set.get_untracked().last().copied().unwrap_or(page),
                ),
            }
        }
    }
    if next != current {
        undo_push.run(());
        page_set.set(next);
        scope_mode.set(2);
    }
}

fn choose_endpoint(
    page: u32,
    kind: u8,
    page_set: RwSignal<Vec<u32>>,
    anchor: RwSignal<Option<(u8, u32)>>,
    hover_page: RwSignal<Option<u32>>,
    hover_kind: RwSignal<Option<u8>>,
    hover_side: RwSignal<Option<u8>>,
    undo_push: Callback<()>,
    scope_mode: RwSignal<u8>,
) {
    match anchor.get_untracked() {
        None => {
            anchor.set(Some((kind, page)));
            hover_page.set(None);
            hover_kind.set(None);
            hover_side.set(None);
        }
        Some((anchor_kind, anchor_page)) => {
            if anchor_kind == kind {
                if read_page_repeat() == "cancel" {
                    clear_draft(anchor, hover_page, hover_kind, hover_side);
                } else {
                    adjust_same_endpoint(page, kind, page_set, undo_push, scope_mode);
                    clear_draft(anchor, hover_page, hover_kind, hover_side);
                }
            } else {
                let (start, end) = if anchor_kind == 0 {
                    (anchor_page, page)
                } else {
                    (page, anchor_page)
                };
                if start <= end {
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
                    clear_draft(anchor, hover_page, hover_kind, hover_side);
                } else {
                    anchor.set(Some((kind, page)));
                    hover_page.set(None);
                    hover_kind.set(None);
                    hover_side.set(None);
                }
            }
        }
    }
}

fn toggle_page(
    page: u32,
    page_set: RwSignal<Vec<u32>>,
    anchor: RwSignal<Option<(u8, u32)>>,
    hover_page: RwSignal<Option<u32>>,
    hover_kind: RwSignal<Option<u8>>,
    hover_side: RwSignal<Option<u8>>,
    undo_push: Callback<()>,
    scope_mode: RwSignal<u8>,
) {
    let current = page_set.get_untracked();
    let mut next = current.clone();
    pdfscope_core::pages::toggle_page(&mut next, page);
    if next != current {
        undo_push.run(());
        page_set.set(next);
        scope_mode.set(2);
    }
    clear_draft(anchor, hover_page, hover_kind, hover_side);
}

fn action_icon(kind: u8) -> impl IntoView {
    match kind {
        0 | 1 => view! {
            <svg viewBox="0 0 1024 1024" width="32" height="32" aria-hidden="true">
                <path fill="currentColor" d="M855.866698 106.426915c0 23.451108-19.010983 42.463114-42.463114 42.463114L203.435319 148.890029c-23.451108 0-42.463114-19.010983-42.463114-42.463114l0 0c0-23.451108 19.010983-42.463114 42.463114-42.463114l609.968266 0C836.855716 63.963801 855.866698 82.974783 855.866698 106.426915L855.866698 106.426915zM511.826038 264.35069c-23.451108 0-42.463114 19.010983-42.463114 42.463114l0 609.968266c0 23.451108 19.010983 42.463114 42.463114 42.463114l0 0c23.451108 0 42.463114-19.010983 42.463114-42.463114L554.289152 306.812781C554.288129 283.361673 535.277146 264.35069 511.826038 264.35069L511.826038 264.35069zM541.961382 233.023194c-16.582677-16.582677-42.517349-17.534352-57.927317-2.124384L171.253362 543.67849c-15.408945 15.408945-14.458293 41.343617 2.124384 57.926294l0 0c16.582677 16.582677 42.517349 17.534352 57.926294 2.124384L544.085766 290.949488C559.494711 275.540544 558.544059 249.605871 541.961382 233.023194L541.961382 233.023194zM481.696834 233.023194c-16.582677 16.582677-17.534352 42.517349-2.124384 57.926294l312.780703 312.780703c15.408945 15.408945 41.343617 14.458293 57.926294-2.124384l0 0c16.582677-16.582677 17.534352-42.517349 2.124384-57.926294L539.623128 230.897787C524.214183 215.488843 498.27951 216.440517 481.696834 233.023194L481.696834 233.023194z"></path>
            </svg>
        }
        .into_any(),
        2 => view! {
            <svg viewBox="0 0 1024 1024" width="32" height="32" aria-hidden="true">
                <path fill="currentColor" d="M896 512c0 212.074667-171.925333 384-384 384S128 724.074667 128 512 299.925333 128 512 128s384 171.925333 384 384zM512 853.333333c188.522667 0 341.333333-152.810667 341.333333-341.333333S700.522667 170.666667 512 170.666667 170.666667 323.477333 170.666667 512s152.810667 341.333333 341.333333 341.333333z"></path>
                <path fill="currentColor" d="M277.333333 512a21.333333 21.333333 0 0 1 21.333334-21.333333h423.104a21.333333 21.333333 0 1 1 0 42.666666H298.666667a21.333333 21.333333 0 0 1-21.333334-21.333333z"></path>
                <path fill="currentColor" d="M510.229333 744.896a21.333333 21.333333 0 0 1-21.333333-21.333333V300.437333a21.333333 21.333333 0 0 1 42.666667 0v423.125334a21.333333 21.333333 0 0 1-21.333334 21.333333z"></path>
            </svg>
        }
        .into_any(),
        _ => view! { <svg viewBox="0 0 1 1" width="1" height="1"></svg> }.into_any(),
    }
}

#[component]
fn PageRangeToolbar(
    page: u32,
    top: f64,
    height: f64,
    side: &'static str,
    side_value: u8,
    hover_page: RwSignal<Option<u32>>,
    hover_kind: RwSignal<Option<u8>>,
    hover_side: RwSignal<Option<u8>>,
    action_page: Memo<u32>,
    activate: Callback<(u32, u8, u8)>,
) -> impl IntoView {
    let click = Callback::new(move |kind: u8| {
        activate.run((action_page.get_untracked(), kind, side_value));
    });
    view! {
        <div
            class=format!("page-bar-actions page-overlay-actions range-{side}")
            role="toolbar"
            style=format!("top: {top:.2}px; height: {height:.2}px;")
            class:active=move || hover_page.get() == Some(page) && hover_side.get() == Some(side_value)
            data-kind=move || {
                if hover_page.get() == Some(page) && hover_side.get() == Some(side_value) {
                    hover_kind.get().unwrap_or(2).to_string()
                } else {
                    "2".to_owned()
                }
            }
            aria-label=format!("{}的页面引用操作", page_label(page))
        >
            <button
                class="page-bar-btn page-bar-start"
                title="从这里开始"
                aria-label=format!("{}：从这里开始", page_label(page))
                on:click=move |_| click.run(0)
            >
                {action_icon(0)}
            </button>
            <button
                class="page-bar-btn page-bar-single"
                title="单独这一页"
                aria-label=format!("{}：单独这一页", page_label(page))
                on:click=move |_| click.run(2)
            >
                {action_icon(2)}
            </button>
            <button
                class="page-bar-btn page-bar-end"
                title="到这里结束"
                aria-label=format!("{}：到这里结束", page_label(page))
                on:click=move |_| click.run(1)
            >
                {action_icon(1)}
            </button>
        </div>
    }
}

fn page_at_pointer(event: &web_sys::MouseEvent, layout: &PageGeometry) -> Option<(u32, u8)> {
    let document = web_sys::window()?.document()?;
    let pages = document.get_element_by_id("pages")?;
    let rect = pages.get_bounding_client_rect();
    let html: &web_sys::HtmlElement = pages.unchecked_ref();
    let layout_height = f64::from(html.offset_height());
    if rect.height() <= 0.0 || layout_height <= 0.0 {
        return None;
    }
    let scale = rect.height() / layout_height;
    let base_y = (event.client_y() as f64 - rect.top()) / scale;
    if layout.pages.is_empty() {
        return None;
    }
    let mut low = 0usize;
    let mut high = layout.pages.len() - 1;
    let mut found = 0usize;
    while low <= high {
        let middle = (low + high) / 2;
        if layout.pages[middle].top <= base_y {
            found = middle;
            low = middle + 1;
        } else if middle == 0 {
            break;
        } else {
            high = middle - 1;
        }
    }
    let metric = layout.pages.get(found)?;
    if base_y < metric.top || base_y > metric.top + metric.height {
        return None;
    }
    let page = (found + 1) as u32;
    let ratio = (base_y - metric.top) / metric.height.max(1.0);
    let kind = if ratio < 1.0 / 3.0 {
        0
    } else if ratio < 2.0 / 3.0 {
        2
    } else {
        1
    };
    Some((page, kind))
}

#[component]
pub(crate) fn PageRangeOverlay(
    page_set: RwSignal<Vec<u32>>,
    anchor: RwSignal<Option<(u8, u32)>>,
    hover_page: RwSignal<Option<u32>>,
    hover_kind: RwSignal<Option<u8>>,
    hover_side: RwSignal<Option<u8>>,
    current_page: RwSignal<u32>,
    layout: RwSignal<PageGeometry>,
    undo_push: Callback<()>,
    scope_mode: RwSignal<u8>,
) -> impl IntoView {
    let bars = Memo::new(move |_| {
        let layout = layout.get();
        if layout.pages.is_empty() {
            return Vec::<BarSpec>::new();
        }
        let set = page_set.get();
        let state = draft(anchor.get(), hover_page.get(), hover_kind.get(), &set);
        let mut intervals = page_ranges(&set);
        if let Some((start, end)) = state.candidate {
            if state.subtract {
                intervals = subtract_interval(&intervals, start, end);
            }
        }
        if let Some((_, open_page, _)) = state.open {
            intervals = subtract_interval(&intervals, open_page, open_page);
        }

        let mut specs = Vec::new();
        for (start, end) in intervals {
            if let Some((top, height)) = page_span(&layout, start, end) {
                specs.push(BarSpec {
                    top,
                    height,
                    class: "page-range-bar global visible base".to_owned(),
                    style: format!(
                        "top:{top:.2}px;height:{height:.2}px;background:{};",
                        bar_gradient(
                            true,
                            true,
                            "var(--deep-border)",
                            "var(--deep-border)",
                            "var(--deep-body)"
                        )
                    ),
                });
            }
        }

        if let Some((start, end)) = state.candidate {
            if state.subtract {
                if let Some((top, height)) = page_span(&layout, start, end) {
                    specs.push(BarSpec {
                        top,
                        height,
                        class: "page-range-bar global visible ghost first last".to_owned(),
                        style: format!("top:{top:.2}px;height:{height:.2}px;"),
                    });
                }
            } else {
                for (from, to) in add_segments(start, end, &set) {
                    if let Some((top, height)) = page_span(&layout, from, to) {
                        specs.push(BarSpec {
                            top,
                            height,
                            class: "page-range-bar global visible add".to_owned(),
                            style: format!(
                                "top:{top:.2}px;height:{height:.2}px;background:{};",
                                bar_gradient(
                                    true,
                                    true,
                                    "var(--light-border)",
                                    "var(--light-border)",
                                    "var(--light-body)"
                                )
                            ),
                        });
                    }
                }
            }
        }

        if let Some((kind, page, placed)) = state.open {
            if let Some((top, height)) = page_span(&layout, page, page) {
                specs.push(BarSpec {
                    top,
                    height,
                    class: "page-range-bar global visible open last".to_owned(),
                    style: format!(
                        "top:{top:.2}px;height:{height:.2}px;background:{};",
                        open_gradient(kind, placed)
                    ),
                });
            }
        }
        specs
    });

    let outlines = Memo::new(move |_| {
        let layout = layout.get();
        if layout.pages.is_empty() {
            return Vec::<OutlineSpec>::new();
        }
        let set = page_set.get();
        let state = draft(anchor.get(), hover_page.get(), hover_kind.get(), &set);
        let subtract = state.candidate.filter(|_| state.subtract);
        let mut out = Vec::new();

        // Committed pages keep a solid outline unless a subtraction preview is replacing it.
        for page in &set {
            if subtract
                .map(|(start, end)| *page >= start && *page <= end)
                .unwrap_or(false)
            {
                continue;
            }
            if let Some(metric) = layout.pages.get((*page - 1) as usize) {
                out.push(OutlineSpec {
                    top: metric.top,
                    height: metric.height,
                    preview: false,
                    subtract: false,
                });
            }
        }

        if let Some((start, end)) = state.candidate {
            if state.subtract {
                for page in start..=end {
                    if let Some(metric) = layout.pages.get((page - 1) as usize) {
                        out.push(OutlineSpec {
                            top: metric.top,
                            height: metric.height,
                            preview: true,
                            subtract: true,
                        });
                    }
                }
            } else {
                for page in start..=end {
                    if set.binary_search(&page).is_err() {
                        if let Some(metric) = layout.pages.get((page - 1) as usize) {
                            out.push(OutlineSpec {
                                top: metric.top,
                                height: metric.height,
                                preview: true,
                                subtract: false,
                            });
                        }
                    }
                }
            }
        } else if let Some((_, page, _)) = state.open {
            if set.binary_search(&page).is_err() {
                if let Some(metric) = layout.pages.get((page - 1) as usize) {
                    out.push(OutlineSpec {
                        top: metric.top,
                        height: metric.height,
                        preview: true,
                        subtract: false,
                    });
                }
            }
        }
        out
    });

    let action_page = Memo::new(move |_| {
        let count = layout.get().pages.len() as u32;
        if count == 0 {
            return 0;
        }
        hover_page
            .get()
            .filter(|page| *page > 0 && *page <= count)
            .or_else(|| Some(current_page.get().clamp(1, count)))
            .unwrap_or(1)
    });

    // After a click the hover state is cleared. The old per-page buttons only re-appeared on a new
    // `mouseenter`; the global strip would otherwise reacquire on the next pointermove inside the
    // same third. Suppress the clicked page/kind/side until the pointer changes zone or leaves.
    let suppressed_action = RwSignal::new(Option::<(u32, u8, u8)>::None);

    let activate = Callback::new(move |(page, kind, side): (u32, u8, u8)| {
        suppressed_action.set(Some((page, kind, side)));
        match kind {
            0 => choose_endpoint(
                page, 0, page_set, anchor, hover_page, hover_kind, hover_side, undo_push,
                scope_mode,
            ),
            1 => choose_endpoint(
                page, 1, page_set, anchor, hover_page, hover_kind, hover_side, undo_push,
                scope_mode,
            ),
            _ => toggle_page(
                page, page_set, anchor, hover_page, hover_kind, hover_side, undo_push, scope_mode,
            ),
        }
    });

    let clear_for_side = Callback::new(move |side: u8| {
        if hover_side.get_untracked() == Some(side) {
            hover_page.set(None);
            hover_kind.set(None);
            hover_side.set(None);
        }
        suppressed_action.set(None);
    });

    let set_pointer = Callback::new(move |(page, kind, side): (u32, u8, u8)| {
        if suppressed_action.get_untracked() == Some((page, kind, side)) {
            return;
        }
        suppressed_action.set(None);
        let changed = hover_page.get_untracked() != Some(page)
            || hover_kind.get_untracked() != Some(kind)
            || hover_side.get_untracked() != Some(side);
        if changed {
            hover_page.set(Some(page));
            hover_kind.set(Some(kind));
            hover_side.set(Some(side));
        }
    });

    view! {
        <div class="page-overlay">
            {move || {
                outlines
                    .get()
                    .into_iter()
                    .map(|outline| {
                        let style = format!(
                            "top:{:.2}px;height:{:.2}px;",
                            outline.top, outline.height
                        );
                        let class = if outline.preview && outline.subtract {
                            "page-overlay-preview subtract"
                        } else if outline.preview {
                            "page-overlay-preview"
                        } else {
                            "page-overlay-solid"
                        };
                        view! { <div class=class style=style aria-hidden="true"></div> }
                    })
                    .collect_view()
            }}

            {move || {
                bars.get()
                    .into_iter()
                    .map(|bar| {
                        let left = format!("{} range-left", bar.class);
                        let right = format!("{} range-right", bar.class);
                        view! {
                            <div class=left style=bar.style.clone() aria-hidden="true"></div>
                            <div class=right style=bar.style aria-hidden="true"></div>
                        }
                    })
                    .collect_view()
            }}

            {move || {
                if layout.get().pages.is_empty() {
                    return ().into_any();
                }
                let page = action_page.get();
                let geometry = layout.get();
                let Some(metric) = geometry.pages.get(page.saturating_sub(1) as usize) else {
                    return ().into_any();
                };
                let top = metric.top;
                let height = metric.height;
                view! {
                    <PageRangeToolbar
                        page=page
                        top=top
                        height=height
                        side="left"
                        side_value=0
                        hover_page=hover_page
                        hover_kind=hover_kind
                        hover_side=hover_side
                        action_page=action_page
                        activate=activate
                    />
                    <PageRangeToolbar
                        page=page
                        top=top
                        height=height
                        side="right"
                        side_value=1
                        hover_page=hover_page
                        hover_kind=hover_kind
                        hover_side=hover_side
                        action_page=action_page
                        activate=activate
                    />
                }
                .into_any()
            }}

            {move || {
                if layout.get().pages.is_empty() {
                    return ().into_any();
                }
                view! {
                    <div
                        class="page-overlay-hit range-left"
                        aria-hidden="true"
                        on:pointermove=move |event: web_sys::PointerEvent| {
                            let layout = layout.get_untracked();
                            if let Some((page, kind)) = page_at_pointer(event.unchecked_ref::<web_sys::MouseEvent>(), &layout) {
                                set_pointer.run((page, kind, 0));
                            } else {
                                clear_for_side.run(0);
                            }
                        }
                        on:pointerleave=move |_| clear_for_side.run(0)
                        on:click=move |event: web_sys::MouseEvent| {
                            let layout = layout.get_untracked();
                            if let Some((page, kind)) = page_at_pointer(event.unchecked_ref::<web_sys::MouseEvent>(), &layout) {
                                set_pointer.run((page, kind, 0));
                                activate.run((page, kind, 0));
                            }
                        }
                    ></div>
                    <div
                        class="page-overlay-hit range-right"
                        aria-hidden="true"
                        on:pointermove=move |event: web_sys::PointerEvent| {
                            let layout = layout.get_untracked();
                            if let Some((page, kind)) = page_at_pointer(event.unchecked_ref::<web_sys::MouseEvent>(), &layout) {
                                set_pointer.run((page, kind, 1));
                            } else {
                                clear_for_side.run(1);
                            }
                        }
                        on:pointerleave=move |_| clear_for_side.run(1)
                        on:click=move |event: web_sys::MouseEvent| {
                            let layout = layout.get_untracked();
                            if let Some((page, kind)) = page_at_pointer(event.unchecked_ref::<web_sys::MouseEvent>(), &layout) {
                                set_pointer.run((page, kind, 1));
                                activate.run((page, kind, 1));
                            }
                        }
                    ></div>
                }
                .into_any()
            }}
        </div>
    }
}
