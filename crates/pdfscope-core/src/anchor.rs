//! Anchoring a passage to a page.
//!
//! An anchor is the product's memory of "this text, here". It is captured when the reader selects
//! something and resolved when the document is reopened. The two operations are deliberately
//! separate: capture trusts the UI, resolution trusts the text.
//!
//! The rule that keeps this honest: **resolution never invents a position**. When the quote is not
//! found, or is found more than once without enough context to choose, the result says so.

use std::ops::Range;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{BookId, CoreError, CoreResult, PageIndex, Rect};

/// How much text on each side of a quote is kept for re-anchoring.
const CONTEXT_CHARS: usize = 20;

/// A digest of a page's normalised text, used to tell "same page" from "same page, edited".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageHash(String);

impl PageHash {
    /// Hashes page text that has already been normalised by [`PageText`].
    pub fn of(text: &str) -> Self {
        let digest = Sha256::digest(text.as_bytes());
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use std::fmt::Write as _;
            // Writing into a String cannot fail; the result is discarded rather than unwrapped.
            let _ = write!(hex, "{byte:02x}");
        }
        Self(hex)
    }

    /// The digest as lowercase hex.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One run of text with the rectangle it occupied on the page.
///
/// This is the shape PDF.js produces: a page is a sequence of positioned strings, not one string.
#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    text: String,
    rect: Rect,
}

impl TextItem {
    /// Pairs a run of text with the area it was drawn in.
    pub fn new(text: impl Into<String>, rect: Rect) -> Self {
        Self {
            text: text.into(),
            rect,
        }
    }

    /// The run's text as extracted.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The run's area, normalised to the page.
    pub fn rect(&self) -> Rect {
        self.rect
    }
}

/// A page's text, normalised, with a map back to the rectangles it came from.
#[derive(Debug, Clone)]
pub struct PageText {
    page: PageIndex,
    text: String,
    spans: Vec<Span>,
}

/// A byte range of the normalised text and the rectangle it belongs to.
#[derive(Debug, Clone, Copy)]
struct Span {
    range: (usize, usize),
    rect: Rect,
}

impl PageText {
    /// Builds a page from positioned runs.
    ///
    /// Runs are normalised individually, then joined with a single space where the source did not
    /// already end in whitespace. Joining without a separator would glue the last word of one run
    /// to the first word of the next, which is a matching bug that only shows up on real PDFs.
    pub fn from_items(
        page: PageIndex,
        items: impl IntoIterator<Item = TextItem>,
    ) -> CoreResult<Self> {
        let mut text = String::new();
        let mut spans = Vec::new();

        for item in items {
            let chunk = normalize(item.text());
            if chunk.is_empty() {
                continue;
            }

            if !text.is_empty() {
                text.push(' ');
            }

            let start = text.len();
            text.push_str(&chunk);
            spans.push(Span {
                range: (start, text.len()),
                rect: item.rect(),
            });
        }

        if text.is_empty() {
            return Err(CoreError::EmptyPage(page.get()));
        }

        Ok(Self { page, text, spans })
    }

    /// Zero-based index of this page.
    pub fn page(&self) -> PageIndex {
        self.page
    }

    /// The normalised text, which is what anchors are matched against.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Digest of the normalised text.
    pub fn hash(&self) -> PageHash {
        PageHash::of(&self.text)
    }

    /// Union of the rectangles covering a byte range, or `None` if the range is outside the page.
    fn rect_for(&self, range: Range<usize>) -> Option<Rect> {
        let overlapping = self
            .spans
            .iter()
            .filter(|span| span.range.0 < range.end && range.start < span.range.1)
            .map(|span| span.rect);

        union(overlapping)
    }
}

/// What the reader had selected, as the UI reported it.
#[derive(Debug, Clone)]
pub struct SelectedText {
    book: BookId,
    page: PageIndex,
    rect: Rect,
    text: String,
}

impl SelectedText {
    /// Describes a selection. The text is normalised during capture, not here.
    pub fn new(book: BookId, page: PageIndex, rect: Rect, text: impl Into<String>) -> Self {
        Self {
            book,
            page,
            rect,
            text: text.into(),
        }
    }

    /// The book the selection belongs to.
    pub fn book(&self) -> &BookId {
        &self.book
    }

    /// The page the selection is on.
    pub fn page(&self) -> PageIndex {
        self.page
    }

    /// The area the reader dragged over.
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// The text the UI reported, before normalisation.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A stored selection: its page, its text, and enough context to find it again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    /// The book, by content hash.
    pub book: BookId,
    /// The page the selection was on.
    pub page: PageIndex,
    /// Where the selection was, as a hint. Text matching decides where it *is*.
    pub rect: Rect,
    /// The selected text, whitespace-normalised.
    pub quote: String,
    /// Up to 20 characters before the quote, matched during resolution.
    pub context_before: String,
    /// Up to 20 characters after the quote, matched during resolution.
    pub context_after: String,
    /// Digest of the page text at capture time.
    pub page_hash: PageHash,
}

/// The result of looking for an anchor's text on a page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resolution {
    /// Found once, on an unchanged page.
    Exact {
        /// Where the text is now.
        rect: Rect,
    },
    /// Found once, but the page changed since capture.
    Relocated {
        /// Where the text is now.
        rect: Rect,
    },
    /// Found more than once, and the context could not choose between them.
    Ambiguous {
        /// How many candidates were found, saturated at `u16::MAX`.
        matches: u16,
    },
    /// Not found. The anchor is kept but must not be drawn as if it were valid.
    NeedsReview,
}

/// Captures a selection as an anchor.
///
/// Fails when the selection is empty or when its text does not occur on the page it claims to come
/// from — both are caller mistakes that would otherwise produce an anchor pointing at nothing.
pub fn capture(page: &PageText, selection: &SelectedText) -> CoreResult<Anchor> {
    let quote = normalize(selection.text());

    if quote.is_empty() {
        return Err(CoreError::EmptySelection);
    }
    if selection.page() != page.page() {
        return Err(CoreError::PageMismatch {
            selection: selection.page().get(),
            page: page.page().get(),
        });
    }

    let Some((start, matched)) = page.text().match_indices(&quote).next() else {
        return Err(CoreError::QuoteNotFound {
            page: page.page().get(),
        });
    };
    let end = start + matched.len();

    Ok(Anchor {
        book: selection.book().clone(),
        page: page.page(),
        rect: selection.rect(),
        quote,
        context_before: before(page.text(), start),
        context_after: after(page.text(), end),
        page_hash: page.hash(),
    })
}

/// Looks for an anchor on the page it belongs to.
///
/// Order of preference: a unique match wins; otherwise the stored context chooses between
/// candidates; otherwise the result is [`Resolution::Ambiguous`]. A position is never guessed.
pub fn resolve(anchor: &Anchor, page: &PageText) -> Resolution {
    if anchor.page != page.page() {
        return Resolution::NeedsReview;
    }

    let candidates: Vec<(usize, usize)> = page
        .text()
        .match_indices(&anchor.quote)
        .map(|(start, matched)| (start, start + matched.len()))
        .collect();

    if candidates.is_empty() {
        return Resolution::NeedsReview;
    }

    let chosen = if candidates.len() == 1 {
        Some(candidates[0])
    } else {
        disambiguate(page, anchor, &candidates)
    };

    let Some((start, end)) = chosen else {
        return Resolution::Ambiguous {
            matches: u16::try_from(candidates.len()).unwrap_or(u16::MAX),
        };
    };

    let Some(rect) = page.rect_for(start..end) else {
        return Resolution::NeedsReview;
    };

    if anchor.page_hash == page.hash() {
        Resolution::Exact { rect }
    } else {
        Resolution::Relocated { rect }
    }
}

/// Chooses between identical quotes by comparing the text around each candidate.
fn disambiguate(
    page: &PageText,
    anchor: &Anchor,
    candidates: &[(usize, usize)],
) -> Option<(usize, usize)> {
    let text = page.text();
    let matching: Vec<(usize, usize)> = candidates
        .iter()
        .copied()
        .filter(|(start, end)| {
            let before_ok = anchor.context_before.is_empty()
                || before(text, *start).ends_with(&anchor.context_before);
            let after_ok = anchor.context_after.is_empty()
                || after(text, *end).starts_with(&anchor.context_after);
            before_ok && after_ok
        })
        .collect();

    match matching.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

/// The [`CONTEXT_CHARS`] characters immediately before a byte offset.
fn before(text: &str, index: usize) -> String {
    let mut chars: Vec<char> = text[..index].chars().collect();
    let start = chars.len().saturating_sub(CONTEXT_CHARS);
    chars.drain(..start);
    chars.into_iter().collect()
}

/// The [`CONTEXT_CHARS`] characters immediately after a byte offset.
fn after(text: &str, index: usize) -> String {
    text[index..].chars().take(CONTEXT_CHARS).collect()
}

/// Collapses whitespace, expands the ligatures that PDF extraction leaves behind, and trims.
///
/// Used for both the stored quote and the page text, so the two can be compared.
pub fn normalize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;

    for ch in input.chars() {
        let expansion = match ch {
            '\u{fb00}' => "ff",
            '\u{fb01}' => "fi",
            '\u{fb02}' => "fl",
            '\u{fb03}' => "ffi",
            '\u{fb04}' => "ffl",
            '\u{fb05}' | '\u{fb06}' => "st",
            _ => {
                if ch.is_whitespace() {
                    pending_space = !out.is_empty();
                    continue;
                }
                if pending_space {
                    out.push(' ');
                    pending_space = false;
                }
                out.push(ch);
                continue;
            }
        };

        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push_str(expansion);
    }

    out
}

/// Smallest rectangle covering every input rectangle.
///
/// A single rectangle is returned untouched: recomputing its edges from itself would introduce
/// float rounding and make an exact resolution compare unequal to the rectangle it came from.
fn union(rects: impl Iterator<Item = Rect>) -> Option<Rect> {
    rects.reduce(|acc, rect| {
        let x0 = acc.x().min(rect.x());
        let y0 = acc.y().min(rect.y());
        let x1 = (acc.x() + acc.width()).max(rect.x() + rect.width());
        let y1 = (acc.y() + acc.height()).max(rect.y() + rect.height());

        Rect::new(x0, y0, x1 - x0, y1 - y0).unwrap_or(acc)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h).unwrap()
    }

    fn page_with(lines: &[(&str, Rect)]) -> PageText {
        PageText::from_items(
            PageIndex::new(30),
            lines.iter().map(|(text, rect)| TextItem::new(*text, *rect)),
        )
        .unwrap()
    }

    fn book() -> BookId {
        BookId::new("9f3c")
    }

    const PROSE: &str =
        "只要一切可能的当事人担负辩论参与者的角色，那么他们是否表示认同，就会制约规范的有效性。";

    #[test]
    fn normalization_collapses_whitespace_and_expands_ligatures() {
        assert_eq!(normalize("  a\t\tb\n\nc  "), "a b c");
        assert_eq!(normalize("di\u{fb00}erent"), "different");
        assert_eq!(normalize("公共\u{3000}领域"), "公共 领域");
        assert_eq!(normalize("   "), "");
    }

    #[test]
    fn page_text_joins_runs_without_gluing_words() {
        let page = page_with(&[
            ("Hello", rect(0.1, 0.1, 0.2, 0.02)),
            ("world", rect(0.3, 0.1, 0.2, 0.02)),
        ]);
        assert_eq!(page.text(), "Hello world");
    }

    #[test]
    fn page_text_rejects_an_empty_page() {
        let result = PageText::from_items(PageIndex::new(0), Vec::new());
        assert!(matches!(result, Err(CoreError::EmptyPage(0))));
    }

    #[test]
    fn capture_records_quote_context_and_hash() {
        let page = page_with(&[(PROSE, rect(0.1, 0.2, 0.8, 0.05))]);
        let selection = SelectedText::new(
            book(),
            PageIndex::new(30),
            rect(0.1, 0.2, 0.4, 0.05),
            "  当事人担负辩论参与者的角色 ",
        );

        let anchor = capture(&page, &selection).unwrap();

        assert_eq!(anchor.quote, "当事人担负辩论参与者的角色");
        assert_eq!(anchor.page, PageIndex::new(30));
        assert_eq!(anchor.book, book());
        assert_eq!(anchor.page_hash, page.hash());
        assert!(!anchor.context_before.is_empty());
        assert!(!anchor.context_after.is_empty());
        assert!(PROSE.contains(&anchor.context_before));
    }

    #[test]
    fn capture_refuses_an_empty_selection() {
        let page = page_with(&[(PROSE, rect(0.1, 0.2, 0.8, 0.05))]);
        let selection =
            SelectedText::new(book(), PageIndex::new(30), rect(0.1, 0.2, 0.1, 0.1), "  ");
        assert!(matches!(
            capture(&page, &selection),
            Err(CoreError::EmptySelection)
        ));
    }

    #[test]
    fn capture_refuses_text_that_is_not_on_the_page() {
        let page = page_with(&[(PROSE, rect(0.1, 0.2, 0.8, 0.05))]);
        let selection = SelectedText::new(
            book(),
            PageIndex::new(30),
            rect(0.1, 0.2, 0.1, 0.1),
            "这句话不在页面上",
        );
        assert!(matches!(
            capture(&page, &selection),
            Err(CoreError::QuoteNotFound { page: 30 })
        ));
    }

    #[test]
    fn capture_refuses_a_page_mismatch() {
        let page = page_with(&[(PROSE, rect(0.1, 0.2, 0.8, 0.05))]);
        let selection =
            SelectedText::new(book(), PageIndex::new(31), rect(0.1, 0.2, 0.1, 0.1), PROSE);
        assert!(matches!(
            capture(&page, &selection),
            Err(CoreError::PageMismatch { .. })
        ));
    }

    #[test]
    fn resolve_finds_a_unique_quote_exactly() {
        let page = page_with(&[(PROSE, rect(0.1, 0.2, 0.8, 0.05))]);
        let anchor = capture(
            &page,
            &SelectedText::new(
                book(),
                PageIndex::new(30),
                rect(0.1, 0.2, 0.4, 0.05),
                "当事人",
            ),
        )
        .unwrap();

        assert_eq!(
            resolve(&anchor, &page),
            Resolution::Exact {
                rect: rect(0.1, 0.2, 0.8, 0.05)
            }
        );
    }

    #[test]
    fn resolve_reports_an_ambiguous_quote_instead_of_guessing() {
        let page = page_with(&[
            ("同样的句子出现在这里", rect(0.1, 0.1, 0.5, 0.02)),
            ("同样的句子出现在那里", rect(0.1, 0.3, 0.5, 0.02)),
        ]);
        let anchor = capture(
            &page,
            &SelectedText::new(
                book(),
                PageIndex::new(30),
                rect(0.1, 0.1, 0.3, 0.02),
                "同样的句子",
            ),
        )
        .unwrap();

        // The stored context points at the first occurrence, so this one is still resolvable.
        assert!(matches!(resolve(&anchor, &page), Resolution::Exact { .. }));

        // With the context stripped, nothing can choose between the two.
        let mut contextless = anchor.clone();
        contextless.context_before.clear();
        contextless.context_after.clear();
        assert_eq!(
            resolve(&contextless, &page),
            Resolution::Ambiguous { matches: 2 }
        );
    }

    #[test]
    fn resolve_relocates_when_the_page_changed_but_the_quote_survived() {
        let original = page_with(&[(PROSE, rect(0.1, 0.2, 0.8, 0.05))]);
        let anchor = capture(
            &original,
            &SelectedText::new(
                book(),
                PageIndex::new(30),
                rect(0.1, 0.2, 0.4, 0.05),
                "当事人",
            ),
        )
        .unwrap();

        // Same text, different layout: the page hash changes, the quote is still there.
        let edited = page_with(&[
            ("新增的一段引言。", rect(0.1, 0.05, 0.8, 0.02)),
            (PROSE, rect(0.1, 0.3, 0.8, 0.05)),
        ]);

        assert!(matches!(
            resolve(&anchor, &edited),
            Resolution::Relocated { .. }
        ));
    }

    #[test]
    fn resolve_needs_review_when_the_text_is_gone() {
        let original = page_with(&[(PROSE, rect(0.1, 0.2, 0.8, 0.05))]);
        let anchor = capture(
            &original,
            &SelectedText::new(
                book(),
                PageIndex::new(30),
                rect(0.1, 0.2, 0.4, 0.05),
                "当事人",
            ),
        )
        .unwrap();

        let replaced = page_with(&[("这一页已经换成了别的文字。", rect(0.1, 0.2, 0.8, 0.05))]);
        assert_eq!(resolve(&anchor, &replaced), Resolution::NeedsReview);
    }

    #[test]
    fn resolve_needs_review_for_the_wrong_page() {
        let page = page_with(&[(PROSE, rect(0.1, 0.2, 0.8, 0.05))]);
        let anchor = capture(
            &page,
            &SelectedText::new(
                book(),
                PageIndex::new(30),
                rect(0.1, 0.2, 0.4, 0.05),
                "当事人",
            ),
        )
        .unwrap();

        let other_page = PageText::from_items(
            PageIndex::new(31),
            [TextItem::new(PROSE, rect(0.1, 0.2, 0.8, 0.05))],
        )
        .unwrap();

        assert_eq!(resolve(&anchor, &other_page), Resolution::NeedsReview);
    }

    #[test]
    fn a_quote_spanning_runs_gets_the_union_of_their_rectangles() {
        let page = page_with(&[
            ("只要一切可能的当事人", rect(0.1, 0.2, 0.4, 0.02)),
            ("担负辩论参与者的角色", rect(0.1, 0.24, 0.5, 0.02)),
        ]);
        let anchor = capture(
            &page,
            &SelectedText::new(
                book(),
                PageIndex::new(30),
                rect(0.1, 0.2, 0.4, 0.02),
                "当事人 担负辩论",
            ),
        )
        .unwrap();

        match resolve(&anchor, &page) {
            Resolution::Exact { rect: found } => {
                assert!((found.y() - 0.2).abs() < 1e-6);
                assert!((found.height() - 0.06).abs() < 1e-6);
            }
            other => panic!("expected an exact resolution, got {other:?}"),
        }
    }
}
