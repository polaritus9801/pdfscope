//! Text attachments folded into a question.

/// A labelled block for one attached text file, with a hard character limit.
///
/// The limit is per file so one stray log cannot eat the whole context; the ellipsis says the model
/// is not seeing everything.
pub fn file_block(name: &str, text: &str, limit: usize) -> String {
    let name = name.trim();
    let label = if name.is_empty() { "附件" } else { name };
    let limit = limit.max(1);
    let mut body: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        body.push('…');
    }
    format!("[附件文件 {label}]\n{}\n", body.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_labelled_and_trimmed() {
        assert_eq!(
            file_block("notes.md", "  hello  ", 100),
            "[附件文件 notes.md]\nhello\n"
        );
    }

    #[test]
    fn an_empty_name_falls_back_to_attachment() {
        assert!(file_block("  ", "x", 10).starts_with("[附件文件 附件]"));
    }

    #[test]
    fn the_limit_is_in_characters_and_marks_the_cut() {
        let block = file_block("a.txt", "abcdef", 3);
        assert!(block.contains("abc…"), "got: {block}");
        assert!(!block.contains("abcd"));
    }

    #[test]
    fn text_exactly_at_the_limit_has_no_ellipsis() {
        let block = file_block("a.txt", "abc", 3);
        assert!(block.contains("abc"));
        assert!(!block.contains('…'));
    }
}
