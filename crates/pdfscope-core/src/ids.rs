//! Identifiers.
//!
//! Identity is content-based, not path-based: a book is the SHA-256 of its bytes. Moving or
//! renaming a file therefore keeps its annotations, and two copies of the same file are one book.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Identity of a book: the lowercase hex SHA-256 digest of the file's bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BookId(String);

impl BookId {
    /// Wraps a digest.
    ///
    /// The digest is not re-derived here; callers obtain it from `pdfscope-store` when a file is
    /// imported. Storing a non-digest value is a programming error, not a user error, so this
    /// constructor does not validate.
    pub fn new(digest: impl Into<String>) -> Self {
        Self(digest.into())
    }

    /// The digest as stored.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A page position inside a book.
///
/// Stored and passed around zero-based, because that is how every API and library indexes pages.
/// It displays as one-based, because that is how every reader numbers them. The mismatch is a bug
/// factory if it is not centralised, so it is centralised here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageIndex(u32);

impl PageIndex {
    /// Creates an index from a zero-based position.
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// The zero-based position.
    pub fn get(self) -> u32 {
        self.0
    }

    /// The one-based number a reader would recognise.
    pub fn number(self) -> u32 {
        self.0.saturating_add(1)
    }

    /// The next page.
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for PageIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.number().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_index_displays_one_based() {
        assert_eq!(PageIndex::new(0).to_string(), "1");
        assert_eq!(PageIndex::new(30).to_string(), "31");
    }

    #[test]
    fn page_index_keeps_zero_based_access() {
        let page = PageIndex::new(7);
        assert_eq!(page.get(), 7);
        assert_eq!(page.number(), 8);
    }

    #[test]
    fn page_index_saturates_instead_of_panicking() {
        assert_eq!(PageIndex::new(u32::MAX).next().get(), u32::MAX);
    }

    #[test]
    fn book_id_round_trips_through_display() {
        let id = BookId::new("9f3c");
        assert_eq!(id.to_string(), "9f3c");
        assert_eq!(id.as_str(), "9f3c");
    }

    #[test]
    fn identifiers_serde_as_strings() {
        let json = serde_json::to_string(&BookId::new("abc")).unwrap();
        assert_eq!(json, "\"abc\"");

        let page: PageIndex = serde_json::from_str("4").unwrap();
        assert_eq!(page.get(), 4);
    }
}
