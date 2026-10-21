//! A small Markdown subset, rendered to safe HTML.
//!
//! The model's answer is untrusted text, so everything is escaped before any markup is added. The
//! subset is deliberate: headings, paragraphs, lists, fenced code, bold, italic, inline code, and
//! page citations as links the interface can intercept.

use crate::citation::{Segment, segments};

/// Renders the subset to HTML. Input is escaped; only the tags produced here reach the document.
pub fn to_html(input: &str) -> String {
    let mut renderer = Renderer::default();

    for line in input.lines() {
        renderer.line(line);
    }

    renderer.finish()
}

#[derive(Default)]
struct Renderer {
    out: String,
    paragraph: Vec<String>,
    list: Option<&'static str>,
    code: bool,
}

impl Renderer {
    fn line(&mut self, raw: &str) {
        let line = raw.trim_end();
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            self.flush();
            if self.code {
                self.out.push_str("</code></pre>");
                self.code = false;
            } else {
                self.out.push_str("<pre><code>");
                self.code = true;
            }
            return;
        }

        if self.code {
            self.out.push_str(&escape(line));
            self.out.push('\n');
            return;
        }

        if trimmed.is_empty() {
            self.flush();
            return;
        }

        if let Some((level, rest)) = heading(trimmed) {
            self.flush();
            self.out
                .push_str(&format!("<h{level}>{}</h{level}>", inline(rest)));
            return;
        }

        if let Some((tag, rest)) = list_item(trimmed) {
            self.flush_paragraph();
            match self.list {
                Some(current) if current == tag => {}
                Some(current) => {
                    self.out.push_str(&format!("</{current}>"));
                    self.out.push_str(&format!("<{tag}>"));
                    self.list = Some(tag);
                }
                None => {
                    self.out.push_str(&format!("<{tag}>"));
                    self.list = Some(tag);
                }
            }
            self.out.push_str(&format!("<li>{}</li>", inline(rest)));
            return;
        }

        if let Some(current) = self.list.take() {
            self.out.push_str(&format!("</{current}>"));
        }

        self.paragraph.push(inline(trimmed));
    }

    fn finish(mut self) -> String {
        self.flush();
        if self.code {
            self.out.push_str("</code></pre>");
        }
        self.out
    }

    fn flush(&mut self) {
        self.flush_paragraph();
        if let Some(list) = self.list.take() {
            self.out.push_str(&format!("</{list}>"));
        }
    }

    fn flush_paragraph(&mut self) {
        if self.paragraph.is_empty() {
            return;
        }
        let body = self.paragraph.join("<br>");
        self.out.push_str(&format!("<p>{body}</p>"));
        self.paragraph.clear();
    }
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=4).contains(&level) {
        return None;
    }
    let rest = line[level..].strip_prefix(' ')?;
    Some((level, rest.trim()))
}

fn list_item(line: &str) -> Option<(&'static str, &str)> {
    if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
        return Some(("ul", rest.trim()));
    }

    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        if let Some(rest) = line[digits..].strip_prefix(". ") {
            return Some(("ol", rest.trim()));
        }
    }

    None
}

fn inline(text: &str) -> String {
    segments(text)
        .into_iter()
        .map(|segment| match segment {
            Segment::Text(part) => format_inline(&escape(&part)),
            Segment::Citation(page) => {
                format!("<a href=\"#\" class=\"citation\" data-page=\"{page}\">p.{page}</a>")
            }
        })
        .collect()
}

fn format_inline(text: &str) -> String {
    let text = wrap_pairs(text, "`", "<code>", "</code>");
    let text = wrap_pairs(&text, "**", "<strong>", "</strong>");
    wrap_pairs(&text, "*", "<em>", "</em>")
}

fn wrap_pairs(text: &str, delimiter: &str, open: &str, close: &str) -> String {
    let parts: Vec<&str> = text.split(delimiter).collect();
    let mut out = String::with_capacity(text.len());
    for (index, part) in parts.iter().enumerate() {
        if index % 2 == 1 {
            out.push_str(open);
            out.push_str(part);
            out.push_str(close);
        } else {
            out.push_str(part);
        }
    }
    out
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_html_before_adding_markup() {
        let html = to_html("<script>alert(1)</script>");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn renders_bold_italic_and_code() {
        let html = to_html("**粗** 和 *斜* 和 `code`");
        assert!(html.contains("<strong>粗</strong>"));
        assert!(html.contains("<em>斜</em>"));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn renders_headings_and_paragraphs() {
        let html = to_html("## 标题\n\n第一段\n第二行");
        assert!(html.starts_with("<h2>标题</h2>"));
        assert!(html.contains("<p>第一段<br>第二行</p>"));
    }

    #[test]
    fn renders_unordered_and_ordered_lists() {
        let html = to_html("- 甲\n- 乙\n\n1. 丙");
        assert!(html.contains("<ul><li>甲</li><li>乙</li></ul>"));
        assert!(html.contains("<ol><li>丙</li></ol>"));
    }

    #[test]
    fn renders_fenced_code_without_formatting_it() {
        let html = to_html("```\nlet x = **1**;\n```");
        assert!(html.contains("<pre><code>let x = **1**;"));
        assert!(!html.contains("<strong>"));
    }

    #[test]
    fn citations_become_links_the_interface_can_intercept() {
        let html = to_html("如前所述[p.31]，成立。");
        assert!(html.contains("data-page=\"31\""));
        assert!(html.contains(">p.31</a>"));
    }

    #[test]
    fn a_plain_sentence_is_one_paragraph() {
        assert_eq!(to_html("没有格式。"), "<p>没有格式。</p>");
    }
}
