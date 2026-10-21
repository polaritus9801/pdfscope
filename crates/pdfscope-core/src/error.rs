//! Errors produced by domain rules.
//!
//! Library crates return these variants; binaries decide how to present them.

use thiserror::Error;

/// A domain rule was violated.
///
/// Variants carry the offending values so a caller can report them without re-deriving them.
#[derive(Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CoreError {
    /// A rectangle was not finite, not inside the unit square, or had no area.
    #[error("invalid rectangle: x={x}, y={y}, width={width}, height={height}")]
    InvalidRect {
        /// Left edge, normalised.
        x: f32,
        /// Top edge, normalised.
        y: f32,
        /// Width, normalised.
        width: f32,
        /// Height, normalised.
        height: f32,
    },

    /// A page number was requested that the document does not have.
    #[error("page {page} is outside the document's {page_count} pages")]
    PageOutOfRange {
        /// Requested page, zero-based.
        page: u32,
        /// Number of pages in the document.
        page_count: u32,
    },
}

/// Convenience alias for results produced by this crate.
pub type CoreResult<T> = Result<T, CoreError>;
