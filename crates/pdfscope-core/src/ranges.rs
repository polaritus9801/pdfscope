//! Page ranges, as a reader types them.
//!
//! `31`, `31-45`, `31-45, 60`, with either kind of dash and either kind of comma. Spans are one-based
//! and inclusive, merged when they overlap, and rejected when they leave the document — a range that
//! silently clamps is a range that answers about the wrong page.

use serde::{Deserialize, Serialize};

/// A contiguous, inclusive run of pages, one-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSpan {
    /// First page.
    pub start: u32,
    /// Last page.
    pub end: u32,
}

impl PageSpan {
    /// Number of pages in the span.
    pub fn pages(self) -> u32 {
        self.end - self.start + 1
    }

    /// Whether the span contains a page.
    pub fn contains(self, page: u32) -> bool {
        (self.start..=self.end).contains(&page)
    }
}

/// Why a range could not be understood.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RangeError {
    /// Nothing was typed.
    #[error("请输入页码范围")]
    Empty,

    /// A piece was not a number or a pair of numbers.
    #[error("无法理解 `{0}`，请用 31 或 31-45 这样的写法")]
    Unparsed(String),

    /// A page was zero, or past the end of the document.
    #[error("第 {page} 页不在文档里（共 {page_count} 页）")]
    OutOfRange {
        /// The offending page.
        page: u32,
        /// Pages in the document.
        page_count: u32,
    },

    /// A span was written backwards.
    #[error("`{0}` 的起始页大于结束页")]
    Reversed(String),
}

/// Parses a typed range into merged, ordered spans.
pub fn parse_ranges(input: &str, page_count: u32) -> Result<Vec<PageSpan>, RangeError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(RangeError::Empty);
    }

    let mut spans = Vec::new();

    for piece in text.split([',', '，', '、']) {
        let piece = piece.trim();
        if piece.is_empty() {
            continue;
        }

        let normalised = piece.replace(['–', '—', '~', '～'], "-");
        let (start, end) = match normalised.split_once('-') {
            Some((left, right)) => (number(left.trim(), piece)?, number(right.trim(), piece)?),
            None => {
                let page = number(&normalised, piece)?;
                (page, page)
            }
        };

        if start > end {
            return Err(RangeError::Reversed(piece.to_owned()));
        }
        if start == 0 || end > page_count {
            let offender = if start == 0 { start } else { end };
            return Err(RangeError::OutOfRange {
                page: offender,
                page_count,
            });
        }

        spans.push(PageSpan { start, end });
    }

    if spans.is_empty() {
        return Err(RangeError::Empty);
    }

    Ok(merge(spans))
}

fn number(text: &str, original: &str) -> Result<u32, RangeError> {
    text.parse::<u32>()
        .map_err(|_| RangeError::Unparsed(original.to_owned()))
}

/// Sorts and merges overlapping or adjacent spans.
fn merge(mut spans: Vec<PageSpan>) -> Vec<PageSpan> {
    spans.sort_by_key(|span| span.start);
    let mut merged: Vec<PageSpan> = Vec::with_capacity(spans.len());

    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start <= last.end.saturating_add(1) => {
                last.end = last.end.max(span.end);
            }
            _ => merged.push(span),
        }
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_page() {
        assert_eq!(
            parse_ranges("31", 100).unwrap(),
            vec![PageSpan { start: 31, end: 31 }]
        );
    }

    #[test]
    fn a_range() {
        assert_eq!(
            parse_ranges("31-45", 100).unwrap(),
            vec![PageSpan { start: 31, end: 45 }]
        );
    }

    #[test]
    fn several_segments_with_an_en_dash_and_a_full_width_comma() {
        assert_eq!(
            parse_ranges("31–45，60", 100).unwrap(),
            vec![
                PageSpan { start: 31, end: 45 },
                PageSpan { start: 60, end: 60 },
            ]
        );
    }

    #[test]
    fn overlapping_spans_merge() {
        assert_eq!(
            parse_ranges("10-20, 15-25, 30", 100).unwrap(),
            vec![
                PageSpan { start: 10, end: 25 },
                PageSpan { start: 30, end: 30 },
            ]
        );
    }

    #[test]
    fn adjacent_spans_merge() {
        assert_eq!(
            parse_ranges("1-5, 6-9", 100).unwrap(),
            vec![PageSpan { start: 1, end: 9 }]
        );
    }

    #[test]
    fn whitespace_is_ignored() {
        assert_eq!(
            parse_ranges("  31 - 33 ,  40 ", 100).unwrap(),
            vec![
                PageSpan { start: 31, end: 33 },
                PageSpan { start: 40, end: 40 },
            ]
        );
    }

    #[test]
    fn a_reversed_span_is_an_error() {
        assert!(matches!(
            parse_ranges("45-31", 100),
            Err(RangeError::Reversed(_))
        ));
    }

    #[test]
    fn a_page_past_the_end_is_an_error() {
        assert_eq!(
            parse_ranges("31-101", 100),
            Err(RangeError::OutOfRange {
                page: 101,
                page_count: 100
            })
        );
    }

    #[test]
    fn page_zero_is_an_error() {
        assert!(matches!(
            parse_ranges("0-5", 100),
            Err(RangeError::OutOfRange { page: 0, .. })
        ));
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(parse_ranges("   ", 100), Err(RangeError::Empty));
    }

    #[test]
    fn nonsense_is_an_error_naming_the_piece() {
        assert!(matches!(
            parse_ranges("31-四十五", 100),
            Err(RangeError::Unparsed(_))
        ));
    }

    #[test]
    fn spans_report_their_length() {
        assert_eq!(PageSpan { start: 31, end: 45 }.pages(), 15);
        assert!(PageSpan { start: 31, end: 45 }.contains(31));
        assert!(!PageSpan { start: 31, end: 45 }.contains(46));
    }
}
