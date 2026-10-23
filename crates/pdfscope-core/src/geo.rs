//! A rectangle in page coordinates.
//!
//! Coordinates are normalised to the unit square with the origin at the top-left, so a rectangle
//! stays valid across zoom levels, window sizes, and display densities. A stored rectangle is
//! therefore a *hint* about where something was, never a guarantee: text matching is what decides
//! where an anchor actually lands.

use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

/// A normalised rectangle, with `0.0 <= x, y, width, height` and `x + width <= 1.0`.
///
/// Construction validates the invariant, so any `Rect` you hold is already well-formed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Rect {
    /// Slack allowed when adding edges, so that sub-pixel rounding does not reject a rectangle
    /// that a browser produced from a real selection.
    const EPSILON: f32 = 1e-3;

    /// Builds a rectangle, rejecting values that are not finite or not inside the unit square.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> CoreResult<Self> {
        let finite = [x, y, width, height].iter().all(|v| v.is_finite());
        let in_range = x >= 0.0
            && y >= 0.0
            && width > 0.0
            && height > 0.0
            && x + width <= 1.0 + Self::EPSILON
            && y + height <= 1.0 + Self::EPSILON;

        if !finite || !in_range {
            return Err(CoreError::InvalidRect {
                x,
                y,
                width,
                height,
            });
        }

        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    /// Left edge, normalised to `0.0..=1.0`.
    pub fn x(self) -> f32 {
        self.x
    }

    /// Top edge, normalised to `0.0..=1.0`.
    pub fn y(self) -> f32 {
        self.y
    }

    /// Width, normalised to `0.0..=1.0`.
    pub fn width(self) -> f32 {
        self.width
    }

    /// Height, normalised to `0.0..=1.0`.
    pub fn height(self) -> f32 {
        self.height
    }

    /// Area in square normalised units.
    pub fn area(self) -> f32 {
        self.width * self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_rectangle_inside_the_page() {
        let rect = Rect::new(0.1, 0.2, 0.3, 0.4).unwrap();
        assert_eq!(rect.x(), 0.1);
        assert!((rect.area() - 0.12).abs() < f32::EPSILON);
    }

    #[test]
    fn accepts_a_full_page() {
        assert!(Rect::new(0.0, 0.0, 1.0, 1.0).is_ok());
    }

    #[test]
    fn accepts_rounding_at_the_right_edge() {
        assert!(Rect::new(0.5, 0.5, 0.5, 0.5).is_ok());
    }

    #[test]
    fn rejects_negative_origin() {
        assert!(Rect::new(-0.1, 0.0, 0.2, 0.2).is_err());
    }

    #[test]
    fn rejects_zero_area() {
        assert!(Rect::new(0.1, 0.1, 0.0, 0.2).is_err());
    }

    #[test]
    fn rejects_overflowing_the_page() {
        assert!(Rect::new(0.8, 0.8, 0.4, 0.4).is_err());
    }

    #[test]
    fn rejects_non_finite_values() {
        assert!(Rect::new(f32::NAN, 0.0, 0.1, 0.1).is_err());
    }

    #[test]
    fn survives_serde_round_trip() {
        let rect = Rect::new(0.25, 0.5, 0.5, 0.25).unwrap();
        let json = serde_json::to_string(&rect).unwrap();
        let back: Rect = serde_json::from_str(&json).unwrap();
        assert_eq!(rect, back);
    }
}

/// Which edge or corner a reference is being dragged by.
///
/// The numbers match the interface's `mode` byte so the renderer can pass it straight through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeMode {
    /// Move the whole rectangle.
    Move,
    /// Drag the left edge.
    Left,
    /// Drag the right edge.
    Right,
    /// Drag the top edge.
    Top,
    /// Drag the bottom edge.
    Bottom,
    /// Drag the top-left corner.
    TopLeft,
    /// Drag the top-right corner.
    TopRight,
    /// Drag the bottom-left corner.
    BottomLeft,
    /// Drag the bottom-right corner.
    BottomRight,
}

impl ResizeMode {
    /// Maps the interface's mode byte; anything unknown becomes a move.
    pub fn from_byte(mode: u8) -> Self {
        match mode {
            1 => Self::Left,
            2 => Self::Right,
            3 => Self::Top,
            4 => Self::Bottom,
            5 => Self::TopLeft,
            6 => Self::TopRight,
            7 => Self::BottomLeft,
            8 => Self::BottomRight,
            _ => Self::Move,
        }
    }

    const fn touches_left(self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }

    const fn touches_right(self) -> bool {
        matches!(self, Self::Right | Self::TopRight | Self::BottomRight)
    }

    const fn touches_top(self) -> bool {
        matches!(self, Self::Top | Self::TopLeft | Self::TopRight)
    }

    const fn touches_bottom(self) -> bool {
        matches!(self, Self::Bottom | Self::BottomLeft | Self::BottomRight)
    }
}

/// Applies a drag to a normalised rectangle, keeping it on the page and at least `minimum` large.
///
/// `dx` and `dy` are the pointer movement as a fraction of the page, so the caller never has to
/// convert pixels twice.
pub fn resize_rect(
    rect: (f64, f64, f64, f64),
    dx: f64,
    dy: f64,
    mode: ResizeMode,
    minimum: f64,
) -> (f64, f64, f64, f64) {
    let (mut x, mut y, mut width, mut height) = rect;

    if mode == ResizeMode::Move {
        x = (x + dx).clamp(0.0, 1.0 - width);
        y = (y + dy).clamp(0.0, 1.0 - height);
        return (x, y, width, height);
    }

    if mode.touches_left() {
        let edge = (x + dx).clamp(0.0, x + width - minimum);
        width += x - edge;
        x = edge;
    }
    if mode.touches_right() {
        width = (width + dx).clamp(minimum, 1.0 - x);
    }
    if mode.touches_top() {
        let edge = (y + dy).clamp(0.0, y + height - minimum);
        height += y - edge;
        y = edge;
    }
    if mode.touches_bottom() {
        height = (height + dy).clamp(minimum, 1.0 - y);
    }

    (x, y, width, height)
}

#[cfg(test)]
mod resize_tests {
    use super::*;

    const START: (f64, f64, f64, f64) = (0.2, 0.2, 0.4, 0.3);
    const MIN: f64 = 0.01;

    fn close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    #[test]
    fn moving_translates_the_rectangle() {
        let (x, y, w, h) = resize_rect(START, 0.1, -0.05, ResizeMode::Move, MIN);
        close(x, 0.3);
        close(y, 0.15);
        close(w, 0.4);
        close(h, 0.3);
    }

    #[test]
    fn moving_stops_at_the_edges() {
        let (x, y, ..) = resize_rect(START, 5.0, -5.0, ResizeMode::Move, MIN);
        close(x, 0.6);
        close(y, 0.0);
    }

    #[test]
    fn the_right_edge_only_changes_the_width() {
        let (x, y, w, h) = resize_rect(START, 0.1, 0.4, ResizeMode::Right, MIN);
        close(x, 0.2);
        close(y, 0.2);
        close(w, 0.5);
        close(h, 0.3);
    }

    #[test]
    fn the_left_edge_moves_the_origin_with_the_width() {
        let (x, _, w, _) = resize_rect(START, 0.1, 0.0, ResizeMode::Left, MIN);
        close(x, 0.3);
        close(w, 0.3);
    }

    #[test]
    fn a_corner_moves_both_axes() {
        let (x, y, w, h) = resize_rect(START, 0.1, 0.1, ResizeMode::TopLeft, MIN);
        close(x, 0.3);
        close(y, 0.3);
        close(w, 0.3);
        close(h, 0.2);
    }

    #[test]
    fn an_edge_cannot_cross_its_opposite() {
        // Dragging the left edge right stops one minimum before the right edge.
        let (x, _, w, _) = resize_rect(START, 5.0, 0.0, ResizeMode::Left, MIN);
        close(x, 0.59);
        close(w, MIN);

        // Dragging it left stops at the page edge.
        let (x, _, _, _) = resize_rect(START, -5.0, 0.0, ResizeMode::Left, MIN);
        close(x, 0.0);

        let (_, _, w, _) = resize_rect(START, -5.0, 0.0, ResizeMode::Right, MIN);
        close(w, MIN);
    }

    #[test]
    fn the_bottom_edge_stops_at_the_page_bottom() {
        let (_, y, _, h) = resize_rect(START, 0.0, 5.0, ResizeMode::Bottom, MIN);
        close(y, 0.2);
        close(h, 0.8);
    }

    #[test]
    fn an_unknown_mode_is_a_move() {
        let moved = resize_rect(START, 0.1, 0.0, ResizeMode::from_byte(99), MIN);
        let direct = resize_rect(START, 0.1, 0.0, ResizeMode::Move, MIN);
        assert_eq!(moved, direct);
    }
}

/// The smallest rectangle containing all of `rects`, or `None` when there are none.
///
/// The interface trusts a selection's own union, but a malformed payload must never be able to paint
/// a frame larger than the boxes it contains.
pub fn union_of(rects: &[(f64, f64, f64, f64)]) -> Option<(f64, f64, f64, f64)> {
    let mut left = f64::INFINITY;
    let mut top = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    let mut seen = false;

    for &(x, y, width, height) in rects {
        if !(x.is_finite() && y.is_finite() && width.is_finite() && height.is_finite()) {
            continue;
        }
        if width <= 0.0 || height <= 0.0 {
            continue;
        }
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + width);
        bottom = bottom.max(y + height);
        seen = true;
    }

    if !seen {
        return None;
    }
    Some((left, top, right - left, bottom - top))
}

#[cfg(test)]
mod union_tests {
    use super::*;

    #[test]
    fn an_empty_list_has_no_union() {
        assert_eq!(union_of(&[]), None);
    }

    #[test]
    fn one_rectangle_is_its_own_union() {
        let (x, y, w, h) = union_of(&[(0.1, 0.2, 0.3, 0.4)]).unwrap();
        assert!((x - 0.1).abs() < 1e-9);
        assert!((y - 0.2).abs() < 1e-9);
        assert!((w - 0.3).abs() < 1e-9);
        assert!((h - 0.4).abs() < 1e-9);
    }

    #[test]
    fn several_lines_cover_the_smallest_box() {
        let union = union_of(&[(0.1, 0.1, 0.2, 0.05), (0.05, 0.2, 0.4, 0.05)]);
        let (x, y, w, h) = union.unwrap();
        assert!((x - 0.05).abs() < 1e-9);
        assert!((y - 0.1).abs() < 1e-9);
        assert!((w - 0.4).abs() < 1e-9);
        assert!((h - 0.15).abs() < 1e-9);
    }

    #[test]
    fn invalid_boxes_are_ignored() {
        assert_eq!(union_of(&[(f64::NAN, 0.0, 0.1, 0.1)]), None);
        assert_eq!(union_of(&[(0.0, 0.0, 0.0, 0.1)]), None);
    }
}
