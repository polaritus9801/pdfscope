//! Pure domain types and rules for PDFScope.
//!
//! This crate is deliberately boring: it depends on no framework, performs no I/O, and reads no
//! clock. Everything here is a value that can be constructed in a test in one line.
//!
//! The rule that keeps it that way: if a change needs `tauri`, `axum`, `sqlx`, or the filesystem,
//! it does not belong in `pdfscope-core`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod anchor;
pub mod attachments;
pub mod citation;
pub mod error;
pub mod geo;
pub mod history;
pub mod ids;
pub mod markdown;
pub mod pages;
pub mod ranges;
pub mod tokens;

pub use anchor::{
    Anchor, PageHash, PageText, Resolution, SelectedText, TextItem, capture, normalize, resolve,
};
pub use attachments::file_block;
pub use citation::{Segment, segments};
pub use error::{CoreError, CoreResult};
pub use geo::Rect;
pub use ids::{BookId, PageIndex};
pub use markdown::{to_html, transcript_markdown};
pub use pages::{add_range, remove_page, subtract_range, toggle_page};
pub use ranges::{PageSpan, RangeError, parse_ranges};
pub use tokens::estimate_tokens;
