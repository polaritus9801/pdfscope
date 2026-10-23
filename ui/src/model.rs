//! Values the interface passes around: references, a selection, page sizes and payload helpers.

use wasm_bindgen::{JsCast, JsValue};

/// What the renderer reported after a drag: a page, a rectangle and the words.
#[derive(Clone, PartialEq)]
pub(crate) struct Selection {
    pub(crate) page: u32,
    pub(crate) rect: (f64, f64, f64, f64),
    /// The tight line boxes of the selection itself, not the whole line.
    pub(crate) rects: Vec<(f64, f64, f64, f64)>,
    pub(crate) text: String,
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
    /// The union, used for the frame and its handles.
    pub(crate) rect: (f64, f64, f64, f64),
    /// What the reader actually selected, line by line.
    pub(crate) rects: Vec<(f64, f64, f64, f64)>,
}

/// One page's size at scale 1, as the renderer measured it.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PageSize {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

/// The renderer's page sizes, used to reserve the right height before a canvas exists.
pub(crate) fn parse_page_sizes(value: &JsValue) -> Vec<PageSize> {
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
pub(crate) fn rect_style((x, y, width, height): (f64, f64, f64, f64)) -> String {
    format!(
        "left:{:.3}%; top:{:.3}%; width:{:.3}%; height:{:.3}%;",
        x * 100.0,
        y * 100.0,
        width * 100.0,
        height * 100.0,
    )
}

pub(crate) fn read_number(value: &JsValue, key: &str) -> Option<f64> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()?
        .as_f64()
}

pub(crate) fn read_flag(value: &JsValue, key: &str) -> Option<bool> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()?
        .as_bool()
}

pub(crate) fn read_string(value: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()?
        .as_string()
}

/// Rejects a payload that does not describe a real selection on a page.
pub(crate) fn parse_selection(value: &JsValue) -> Option<Selection> {
    let page = read_number(value, "page")? as u32;
    let x = read_number(value, "x")?;
    let y = read_number(value, "y")?;
    let width = read_number(value, "width")?;
    let height = read_number(value, "height")?;
    let text = read_string(value, "text")?;
    let rects = parse_rects(value);

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
        rects,
        text,
    })
}

/// Reads an array of normalised rectangles, dropping anything outside the page.
pub(crate) fn parse_rects(value: &JsValue) -> Vec<(f64, f64, f64, f64)> {
    let Some(entries) = read_array(value, "rects") else {
        return Vec::new();
    };

    let mut rects = Vec::with_capacity(entries.length() as usize);
    for entry in entries.iter() {
        let Some(x) = read_number(&entry, "x") else {
            continue;
        };
        let Some(y) = read_number(&entry, "y") else {
            continue;
        };
        let Some(width) = read_number(&entry, "width") else {
            continue;
        };
        let Some(height) = read_number(&entry, "height") else {
            continue;
        };
        let in_unit = |n: f64| (0.0..=1.0).contains(&n);
        if in_unit(x)
            && in_unit(y)
            && in_unit(width)
            && in_unit(height)
            && width > 0.0
            && height > 0.0
        {
            rects.push((x, y, width, height));
        }
    }
    rects
}

fn read_array(value: &JsValue, key: &str) -> Option<js_sys::Array> {
    let field = js_sys::Reflect::get(value, &JsValue::from_str(key)).ok()?;
    field.dyn_into::<js_sys::Array>().ok()
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
