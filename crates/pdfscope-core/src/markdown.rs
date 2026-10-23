//! Markdown, rendered to safe HTML.
//!
//! CommonMark plus tables, strikethrough and task lists, via `pulldown-cmark`. Two things are ours:
//! raw HTML is escaped rather than passed through, and page citations become links the interface can
//! intercept.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

use crate::citation::{Segment, segments};

/// Renders Markdown to HTML. The input is untrusted; raw HTML in it is escaped, never emitted.
///
/// Display and inline math are lifted out before parsing. Markdown would otherwise read `_` as
/// emphasis, eat backslashes and turn `[p.3]` into a citation **inside** a formula, and KaTeX can no
/// longer match the delimiters once tags sit between them.
pub fn to_html(input: &str) -> String {
    let (input, maths) = protect_math(input);
    let input = normalise(&input);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut in_code = false;
    let mut buffer = String::new();
    let mut events: Vec<Event<'_>> = Vec::new();

    for event in Parser::new_ext(&input, options) {
        match event {
            // Consecutive text is buffered: `[p.31]` arrives as five Text events, and a citation is
            // only recognisable once they are joined.
            Event::Text(text) if !in_code => buffer.push_str(&text),

            // Raw HTML is data here, not markup. Pushing it as Text escapes it exactly once.
            Event::Html(text) | Event::InlineHtml(text) => {
                flush(&mut buffer, &mut events);
                events.push(Event::Text(text));
            }

            Event::Start(tag) => {
                flush(&mut buffer, &mut events);
                let tag = sanitise(tag);
                if matches!(tag, Tag::CodeBlock(_)) {
                    in_code = true;
                }
                events.push(Event::Start(tag));
            }

            Event::End(tag) => {
                flush(&mut buffer, &mut events);
                if matches!(tag, TagEnd::CodeBlock) {
                    in_code = false;
                }
                events.push(Event::End(tag));
            }

            other => {
                flush(&mut buffer, &mut events);
                events.push(other);
            }
        }
    }

    flush(&mut buffer, &mut events);

    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());

    restore_math(output, &maths)
}

const MATH_PLACEHOLDER: &str = "PDFSCOPEMATHFORMULA";

/// Replaces `$$…$$` and `$…$` with inert tokens and returns the formulas.
///
/// A token is plain ASCII, so Markdown leaves it alone; the formula itself never passes through the
/// parser.
fn protect_math(input: &str) -> (String, Vec<String>) {
    let mut output = String::with_capacity(input.len());
    let mut formulas = Vec::new();
    let bytes = input.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 1 < bytes.len() && bytes[index + 1] == b'$' {
            // An escaped dollar is literal text, not a delimiter.
            output.push_str("\\$");
            index += 2;
            continue;
        }

        if bytes[index] == b'$' {
            let display = index + 1 < bytes.len() && bytes[index + 1] == b'$';
            let start = if display { index + 2 } else { index + 1 };

            if display {
                if let Some(offset) = input[start..].find("$$") {
                    let end = start + offset + 2;
                    let token = format!("{MATH_PLACEHOLDER}{}X", formulas.len());
                    output.push_str(&token);
                    formulas.push(input[index..end].to_owned());
                    index = end;
                    continue;
                }
            } else if let Some(close) = inline_math_end(bytes, start) {
                let content = &input[start..close];
                let balanced = !content.is_empty()
                    && !content.starts_with(char::is_whitespace)
                    && !content.ends_with(char::is_whitespace);
                if balanced {
                    let token = format!("{MATH_PLACEHOLDER}{}X", formulas.len());
                    output.push_str(&token);
                    formulas.push(input[index..=close].to_owned());
                    index = close + 1;
                    continue;
                }
            }
        }

        let character = input[index..].chars().next().unwrap_or('$');
        output.push(character);
        index += character.len_utf8();
    }

    (output, formulas)
}

/// The byte index of the `$` closing an inline formula, staying on one line.
fn inline_math_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => return None,
            b'\\' => index += 2,
            b'$' => return Some(index),
            _ => index += 1,
        }
    }
    None
}

/// Puts the formulas back, escaped for HTML but otherwise untouched, so KaTeX sees the delimiters.
fn restore_math(html: String, formulas: &[String]) -> String {
    let mut output = html;
    for (index, formula) in formulas.iter().enumerate() {
        let token = format!("{MATH_PLACEHOLDER}{index}X");
        output = output.replace(&token, &escape_text(formula));
    }
    output
}

fn escape_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Keeps links with a safe scheme, and refuses the rest by dropping the destination.
fn flush<'a>(buffer: &mut String, events: &mut Vec<Event<'a>>) {
    if buffer.is_empty() {
        return;
    }

    for segment in segments(buffer) {
        match segment {
            Segment::Text(part) => events.push(Event::Text(part.into())),
            Segment::Citation(page) => events.push(Event::InlineHtml(
                format!("<a href=\"#\" class=\"citation\" data-page=\"{page}\">p.{page}</a>")
                    .into(),
            )),
        }
    }

    buffer.clear();
}

fn normalise(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if is_bare_marker(line) {
            let mut next = index + 1;
            while next < lines.len() && lines[next].trim().is_empty() {
                next += 1;
            }
            if next < lines.len() {
                out.push(format!("{} {}", line.trim_end(), lines[next].trim()));
                index = next + 1;
                continue;
            }
        }

        out.push(line.to_owned());
        index += 1;
    }

    out.join("\n")
}

fn is_bare_marker(line: &str) -> bool {
    let trimmed = line.trim();

    if matches!(trimmed, "-" | "*" | "+") {
        return true;
    }

    let digits: String = trimmed.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return false;
    }

    matches!(trimmed[digits.len()..].trim(), "." | ")" | "")
}

fn sanitise(tag: Tag<'_>) -> Tag<'_> {
    match tag {
        Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        } => {
            if is_safe_url(&dest_url) {
                Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                }
            } else {
                Tag::Link {
                    link_type,
                    dest_url: "".into(),
                    title,
                    id,
                }
            }
        }
        other => other,
    }
}

fn is_safe_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

/// Turns `[来源 n]` markers in rendered HTML into clickable inline source links.
///
/// The marker stays visible; the span carries the URL so the interface can open it. Attributes are
/// escaped here because provider titles are untrusted.
pub fn link_source_markers(html: &str, sources: &[(String, String)]) -> String {
    let mut output = html.to_owned();
    for (index, (title, url)) in sources.iter().enumerate() {
        let number = index + 1;
        let span = format!(
            "<span class=\"source-inline\" data-url=\"{}\" title=\"{}\">[来源 {}]</span>",
            escape_attribute(url),
            escape_attribute(title),
            number
        );
        output = output.replace(&format!("[来源 {number}]"), &span);
        output = output.replace(&format!("[来源{number}]"), &span);
    }
    output
}

fn escape_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod source_link_tests {
    use super::*;

    fn sources() -> Vec<(String, String)> {
        vec![
            ("One".to_owned(), "https://one.example".to_owned()),
            (
                "Two \"q\"".to_owned(),
                "https://two.example/?a=1&b=2".to_owned(),
            ),
        ]
    }

    #[test]
    fn markers_become_clickable_spans() {
        let html = link_source_markers("<p>see [来源 1]</p>", &sources());
        assert!(html.contains("data-url=\"https://one.example\""));
        assert!(html.contains(">[来源 1]</span>"));
    }

    #[test]
    fn both_spacing_variants_are_linked() {
        let html = link_source_markers("<p>[来源2]</p>", &sources());
        assert!(html.contains("data-url=\"https://two.example/?a=1&amp;b=2\""));
    }

    #[test]
    fn titles_and_urls_are_escaped() {
        let html = link_source_markers("[来源 2]", &sources());
        assert!(html.contains("title=\"Two &quot;q&quot;\""));
        assert!(!html.contains("&b=2\""));
    }

    #[test]
    fn unknown_or_out_of_range_markers_stay_text() {
        let html = link_source_markers("[来源 9] [来源 1]", &sources()[..1]);
        assert!(!html.contains("[来源 9]</span>"));
        assert!(html.contains("[来源 1]</span>"));
    }
}

/// Exports a conversation as Markdown: one numbered section per question and answer.
pub fn transcript_markdown(turns: &[(String, String)]) -> String {
    let mut out = String::new();
    for (index, (question, answer)) in turns.iter().enumerate() {
        out.push_str(&format!(
            "## {}. {}\n\n{}\n\n",
            index + 1,
            question.trim(),
            answer.trim()
        ));
    }
    let trimmed = out.trim_end();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

#[cfg(test)]
mod transcript_tests {
    use super::*;

    #[test]
    fn an_empty_conversation_exports_nothing() {
        assert_eq!(transcript_markdown(&[]), "");
    }

    #[test]
    fn each_turn_becomes_a_numbered_section() {
        let md = transcript_markdown(&[
            ("问题一".to_owned(), "回答一".to_owned()),
            ("问题二".to_owned(), "回答二".to_owned()),
        ]);
        assert!(md.starts_with("## 1. 问题一"));
        assert!(md.contains("回答一"));
        assert!(md.contains("## 2. 问题二"));
        assert!(md.ends_with('\n'));
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let md = transcript_markdown(&[("  q  ".to_owned(), "  a  ".to_owned())]);
        assert_eq!(md, "## 1. q\n\na\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_math_keeps_underscores_and_asterisks() {
        let html = to_html("设 $v_{rel}$ 与 $a*b$ 成正比");
        assert!(html.contains("$v_{rel}$"), "got: {html}");
        assert!(html.contains("$a*b$"), "got: {html}");
        assert!(!html.contains("<em>"), "got: {html}");
    }

    #[test]
    fn display_math_keeps_braces_and_newlines() {
        let html = to_html("$$\n\\frac{a}{b}\n$$");
        assert!(html.contains("$$\n\\frac{a}{b}\n$$"), "got: {html}");
    }

    #[test]
    fn math_is_not_scanned_for_citations() {
        let html = to_html("$x^{[p.3]}$");
        assert!(!html.contains("class=\"citation\""), "got: {html}");
        assert!(html.contains("$x^{[p.3]}$"), "got: {html}");
    }

    #[test]
    fn an_escaped_dollar_stays_literal() {
        let html = to_html("costs \\$5 today");
        assert!(html.contains("$5 today"), "got: {html}");
    }

    #[test]
    fn an_unpaired_dollar_is_plain_text() {
        let html = to_html("costs $5 today");
        assert!(html.contains("$5 today"), "got: {html}");
        assert!(!html.contains("MATH"), "got: {html}");
    }

    #[test]
    fn html_inside_math_is_escaped() {
        let html = to_html("$a<b & c$");
        assert!(html.contains("$a&lt;b &amp; c$"), "got: {html}");
    }

    #[test]
    fn escapes_raw_html() {
        let html = to_html("<script>alert(1)</script>");
        assert!(html.contains("&lt;script&gt;"), "got: {html}");
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn renders_emphasis_and_code() {
        let html = to_html("**粗** 和 *斜* 和 `code` 和 ~~删~~");
        assert!(html.contains("<strong>粗</strong>"));
        assert!(html.contains("<em>斜</em>"));
        assert!(html.contains("<code>code</code>"));
        assert!(html.contains("<del>删</del>"));
    }

    #[test]
    fn renders_headings_lists_and_tables() {
        let html = to_html("## 标题\n\n- 甲\n- 乙\n\n| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(html.contains("<h2>标题</h2>"));
        assert!(html.contains("<ul>"));
        assert!(html.contains("<table>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn renders_task_lists() {
        let html = to_html("- [x] 做完\n- [ ] 没做");
        assert!(html.contains("type=\"checkbox\""));
    }

    #[test]
    fn keeps_code_blocks_verbatim() {
        let html = to_html("```rust\nlet x = **1**;\n```");
        assert!(html.contains("let x = **1**;"));
        assert!(!html.contains("<strong>"));
    }

    #[test]
    fn citations_become_interceptable_links() {
        let html = to_html("如前所述[p.31]，成立。");
        assert!(html.contains("data-page=\"31\""), "got: {html}");
        assert!(html.contains(">p.31</a>"));
    }

    #[test]
    fn citations_inside_code_stay_text() {
        let html = to_html("`[p.31]`");
        assert!(!html.contains("data-page"));
        assert!(html.contains("[p.31]"));
    }

    #[test]
    fn links_keep_only_safe_schemes() {
        let html = to_html("[a](https://example.com) [b](javascript:alert(1))");
        assert!(html.contains("href=\"https://example.com\""));
        assert!(!html.contains("javascript:"));
    }

    #[test]
    fn a_bare_number_is_joined_to_its_text() {
        let html = to_html("1.\n\n   盟军的作用是自上而下。");
        assert!(!html.contains("<li><p></p>"));
        assert!(html.contains("<ol>"));
        assert!(html.contains("盟军的作用是自上而下。"));
    }

    #[test]
    fn a_number_with_text_on_the_same_line_is_untouched() {
        let html = to_html("1. 内容");
        assert!(html.contains("<li>内容"));
    }

    #[test]
    fn a_bare_bullet_is_joined_too() {
        let html = to_html("-\n\n  项目");
        assert!(html.contains("项目"));
        assert!(!html.contains("<li><p></p>"));
    }

    #[test]
    fn a_plain_sentence_is_one_paragraph() {
        assert_eq!(to_html("没有格式。"), "<p>没有格式。</p>\n");
    }
}
