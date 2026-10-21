//! Markdown, rendered to safe HTML.
//!
//! CommonMark plus tables, strikethrough and task lists, via `pulldown-cmark`. Two things are ours:
//! raw HTML is escaped rather than passed through, and page citations become links the interface can
//! intercept.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

use crate::citation::{Segment, segments};

/// Renders Markdown to HTML. The input is untrusted; raw HTML in it is escaped, never emitted.
pub fn to_html(input: &str) -> String {
    let (input, maths) = extract_math(input);
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

    for (index, mathml) in maths.iter().enumerate() {
        output = output.replace(&placeholder(index), mathml);
    }

    output
}

fn placeholder(index: usize) -> String {
    format!("\u{E000}{index}\u{E001}")
}

/// Replaces `$…$` and `$$…$$` with rendered MathML, leaving fenced code alone.
///
/// Math is extracted before Markdown parsing because the parser would otherwise split it across
/// events and escape its operators. A private-use placeholder survives parsing untouched and is
/// substituted afterwards.
fn extract_math(input: &str) -> (String, Vec<String>) {
    let mut out = String::with_capacity(input.len());
    let mut rendered = Vec::new();
    let mut fence = false;

    for line in input.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        out.push_str(&scan_line(line, &mut rendered));
        out.push('\n');
    }

    (out, rendered)
}

fn scan_line(line: &str, rendered: &mut Vec<String>) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'\\' && bytes.get(index + 1) == Some(&b'$') {
            out.push('$');
            index += 2;
            continue;
        }

        if bytes[index] == b'$' {
            let display = bytes.get(index + 1) == Some(&b'$');
            let width = if display { 2 } else { 1 };
            let start = index + width;

            let end = find_close(bytes, start, display);
            if let Some(end) = end {
                let latex = &line[start..end];
                let style = if display {
                    latex2mathml::DisplayStyle::Block
                } else {
                    latex2mathml::DisplayStyle::Inline
                };

                match latex2mathml::latex_to_mathml(latex, style) {
                    Ok(mathml) => {
                        rendered.push(mathml);
                        out.push_str(&placeholder(rendered.len() - 1));
                    }
                    Err(_) => out.push_str(&line[index..end + width]),
                }

                index = end + width;
                continue;
            }
        }

        let character = line[index..].chars().next().unwrap_or(' ');
        out.push(character);
        index += character.len_utf8();
    }

    out
}

fn find_close(bytes: &[u8], from: usize, display: bool) -> Option<usize> {
    let mut index = from;
    while index < bytes.len() {
        if bytes[index] == b'$' {
            if display {
                if bytes.get(index + 1) == Some(&b'$') {
                    return Some(index);
                }
            } else {
                return Some(index);
            }
        }
        index += 1;
    }
    None
}

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

/// Repairs the one shape models write constantly and CommonMark reads wrongly:
///
/// ```text
/// 1.
///
///    正文
/// ```
///
/// is parsed as a loose list whose item starts with an empty paragraph — which renders as a large
/// blank gap between the number and its text. Joining the marker to its first line fixes both the
/// gap and the alignment.
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

/// Keeps links with a safe scheme, and refuses the rest by dropping the destination.
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn inline_math_becomes_mathml() {
        let html = to_html("当 $x_i \\in \\{0,1\\}$ 时");
        assert!(html.contains("<math"), "got: {html}");
        assert!(!html.contains("$x_i"));
    }

    #[test]
    fn display_math_is_block_mathml() {
        let html = to_html("$$\\max \\sum_{i=1}^{12} x_i$$");
        assert!(html.contains("display=\"block\""), "got: {html}");
    }

    #[test]
    fn math_inside_fenced_code_stays_text() {
        let html = to_html("```\n$x$\n```");
        assert!(!html.contains("<math"));
    }

    #[test]
    fn invalid_latex_falls_back_to_the_source() {
        let html = to_html("$\\frac{$");
        assert!(!html.contains("<math"));
    }

    #[test]
    fn a_plain_sentence_is_one_paragraph() {
        assert_eq!(to_html("没有格式。"), "<p>没有格式。</p>\n");
    }
}
