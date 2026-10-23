//! Web search through Tavily.
//!
//! Tavily returns a title, a URL and a content snippet per result, which is exactly what a citation
//! card needs. The provider is isolated here so another one can replace it without touching the
//! interface.

use serde::{Deserialize, Serialize};

use crate::{AiError, Secret};

/// One search result, ready to be shown and cited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebResult {
    /// Result title.
    pub title: String,
    /// Result URL.
    pub url: String,
    /// A snippet, truncated for the context budget.
    pub content: String,
}

const ENDPOINT: &str = "https://api.tavily.com/search";
const MAX_RESULTS: usize = 5;
const MAX_SNIPPET: usize = 600;

/// Searches the web for `query` and returns at most five results.
pub async fn tavily(key: &Secret, query: &str) -> Result<Vec<WebResult>, AiError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let body = serde_json::json!({
        "api_key": key.expose(),
        "query": query,
        "search_depth": "basic",
        "max_results": MAX_RESULTS,
        "include_answer": false,
    });

    let response = crate::client(None)?
        .post(ENDPOINT)
        .json(&body)
        .send()
        .await?;
    let response = crate::ensure_success(response).await?;
    let value: serde_json::Value = response.json().await?;
    Ok(parse_results(&value))
}

/// Reads Tavily's `results` array into our shape.
pub(crate) fn parse_results(value: &serde_json::Value) -> Vec<WebResult> {
    let Some(results) = value.get("results").and_then(|results| results.as_array()) else {
        return Vec::new();
    };

    results
        .iter()
        .filter_map(|entry| {
            let title = entry.get("title")?.as_str()?.trim();
            let url = entry.get("url")?.as_str()?.trim();
            if url.is_empty() {
                return None;
            }
            let content = entry
                .get("content")
                .and_then(|content| content.as_str())
                .unwrap_or_default()
                .trim();
            Some(WebResult {
                title: if title.is_empty() {
                    url.to_owned()
                } else {
                    title.to_owned()
                },
                url: url.to_owned(),
                content: truncate(content, MAX_SNIPPET),
            })
        })
        .take(MAX_RESULTS)
        .collect()
}

fn truncate(text: &str, limit: usize) -> String {
    let mut characters = text.chars();
    let prefix: String = characters.by_ref().take(limit).collect();
    if characters.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_keep_title_url_and_snippet() {
        let value = serde_json::json!({
            "results": [
                {"title": "Rust", "url": "https://rust-lang.org", "content": "A language."}
            ]
        });
        let results = parse_results(&value);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust");
        assert_eq!(results[0].url, "https://rust-lang.org");
        assert_eq!(results[0].content, "A language.");
    }

    #[test]
    fn a_missing_title_falls_back_to_the_url() {
        let value = serde_json::json!({
            "results": [{"title": "  ", "url": "https://example.com", "content": ""}]
        });
        assert_eq!(parse_results(&value)[0].title, "https://example.com");
    }

    #[test]
    fn entries_without_a_url_are_dropped() {
        let value = serde_json::json!({
            "results": [
                {"title": "no url", "content": "x"},
                {"title": "ok", "url": "https://example.com", "content": "x"}
            ]
        });
        let results = parse_results(&value);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "ok");
    }

    #[test]
    fn at_most_five_results_survive() {
        let results: Vec<serde_json::Value> = (0..8)
            .map(|index| {
                serde_json::json!({
                    "title": format!("t{index}"),
                    "url": format!("https://e{index}.com"),
                    "content": "x"
                })
            })
            .collect();
        let value = serde_json::json!({ "results": results });
        assert_eq!(parse_results(&value).len(), 5);
    }

    #[test]
    fn a_missing_results_array_is_empty() {
        assert!(parse_results(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn long_snippets_are_truncated_with_an_ellipsis() {
        let content = "字".repeat(MAX_SNIPPET + 10);
        let value = serde_json::json!({
            "results": [{"title": "t", "url": "https://e.com", "content": content}]
        });
        let snippet = &parse_results(&value)[0].content;
        assert!(snippet.ends_with('…'));
        assert_eq!(snippet.chars().count(), MAX_SNIPPET + 1);
    }
}
