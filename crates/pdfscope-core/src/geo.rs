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
