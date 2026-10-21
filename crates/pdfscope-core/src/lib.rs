//! Pure domain types and rules for PDFScope.
//!
//! This crate is deliberately boring: it depends on no framework, performs no I/O, and reads no
//! clock. Everything here is a value that can be constructed in a test in one line.
//!
//! The rule that keeps it that way: if a change needs `tauri`, `axum`, `sqlx`, or the filesystem,
//! it does not belong in `pdfscope-core`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod anchor;
pub mod error;
pub mod geo;
pub mod ids;

pub use anchor::{
    Anchor, PageHash, PageText, Resolution, SelectedText, TextItem, capture, normalize, resolve,
};
pub use error::{CoreError, CoreResult};
pub use geo::Rect;
pub use ids::{BookId, PageIndex};
