//! Page citations inside an answer.
//!
//! Models write page references in several dialects. Recognising them is domain logic — it decides
//! what the reader can click — so it lives here rather than in the interface.

use serde::{Deserialize, Serialize};

/// A piece of an answer: words, or a page reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Segment {
    /// Ordinary text.
    Text(String),
    /// A page number the model cited, one-based.
    Citation(u32),
}

/// Splits an answer into text and citations.
///
/// An unterminated `[p.3` stays text, so a citation does not flicker into existence mid-token while
/// the answer is still streaming.
pub fn segments(text: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut start = 0;

    while index < bytes.len() {
        if bytes[index] == b'['
            && let Some((page, end)) = parse(text, index)
        {
            if start < index {
                out.push(Segment::Text(text[start..index].to_owned()));
            }
            out.push(Segment::Citation(page));
            index = end;
            start = end;
            continue;
        }
        index += 1;
    }

    if start < text.len() {
        out.push(Segment::Text(text[start..].to_owned()));
    }

    out
}

/// Parses `[p.31]`, `[p31]`, `[p 31]`, `[P.31]` and `[pp.31-33]` (which yields 31).
fn parse(text: &str, open: usize) -> Option<(u32, usize)> {
    let rest = text.get(open + 1..)?;
    let close = rest.find(']')?;
    let inner = rest[..close].trim().to_ascii_lowercase();

    let digits = inner
        .strip_prefix("pp.")
        .or_else(|| inner.strip_prefix("p."))
        .or_else(|| inner.strip_prefix('p'))?;
    let number: String = digits
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();

    if number.is_empty() {
        return None;
    }

    Some((number.parse().ok()?, open + 1 + close + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_common_shapes() {
        assert_eq!(segments("[p.31]"), vec![Segment::Citation(31)]);
        assert_eq!(segments("[p31]"), vec![Segment::Citation(31)]);
        assert_eq!(segments("[p 31]"), vec![Segment::Citation(31)]);
        assert_eq!(segments("[P.31]"), vec![Segment::Citation(31)]);
        assert_eq!(segments("[pp.31-33]"), vec![Segment::Citation(31)]);
    }

    #[test]
    fn keeps_surrounding_text_and_order() {
        assert_eq!(
            segments("如前所述[p.31]，这一点成立。"),
            vec![
                Segment::Text("如前所述".to_owned()),
                Segment::Citation(31),
                Segment::Text("，这一点成立。".to_owned()),
            ]
        );
    }

    #[test]
    fn leaves_an_unfinished_citation_as_text() {
        assert_eq!(segments("见[p.3"), vec![Segment::Text("见[p.3".to_owned())]);
    }

    #[test]
    fn ignores_brackets_that_are_not_citations() {
        assert_eq!(
            segments("[注] 和 [p.]"),
            vec![Segment::Text("[注] 和 [p.]".to_owned())]
        );
    }

    #[test]
    fn a_plain_answer_is_one_segment() {
        assert_eq!(
            segments("没有引用的一句话。"),
            vec![Segment::Text("没有引用的一句话。".to_owned())]
        );
    }
}
