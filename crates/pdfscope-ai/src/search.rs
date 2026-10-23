//! Web search providers.
//!
//! Each provider is reduced to the same `WebResult` shape; the interface never learns which one
//! answered. DeepSeek is the default because its Search API shares the model key and returns a long
//! summary meant for models.

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

/// Which search backend to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// DeepSeek's in-house search, authenticated with the model key.
    DeepSeek,
    /// Tavily.
    Tavily,
    /// Brave Search.
    Brave,
    /// Exa.
    Exa,
    /// Serper (Google).
    Serper,
}

impl Provider {
    /// The stable id stored in settings.
    pub fn id(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::Tavily => "tavily",
            Self::Brave => "brave",
            Self::Exa => "exa",
            Self::Serper => "serper",
        }
    }

    /// The display name.
    pub fn label(self) -> &'static str {
        match self {
            Self::DeepSeek => "DeepSeek Search",
            Self::Tavily => "Tavily",
            Self::Brave => "Brave",
            Self::Exa => "Exa",
            Self::Serper => "Serper (Google)",
        }
    }

    /// Maps the stored id; unknown ids fall back to DeepSeek.
    pub fn from_id(id: &str) -> Self {
        match id {
            "tavily" => Self::Tavily,
            "brave" => Self::Brave,
            "exa" => Self::Exa,
            "serper" => Self::Serper,
            _ => Self::DeepSeek,
        }
    }

    /// Whether the search can reuse the model key when no search key is stored.
    pub fn reuses_model_key(self) -> bool {
        self == Self::DeepSeek
    }
}

const MAX_RESULTS: usize = 5;
const MAX_SNIPPET: usize = 600;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Searches the web with the chosen provider.
pub async fn search(
    provider: Provider,
    key: &Secret,
    query: &str,
) -> Result<Vec<WebResult>, AiError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let value = match provider {
        Provider::DeepSeek => deepseek(key, query).await?,
        Provider::Tavily => tavily(key, query).await?,
        Provider::Brave => brave(key, query).await?,
        Provider::Exa => exa(key, query).await?,
        Provider::Serper => serper(key, query).await?,
    };
    Ok(parse(provider, &value))
}

async fn deepseek(key: &Secret, query: &str) -> Result<serde_json::Value, AiError> {
    let body = serde_json::json!({
        "query": query,
        "count": MAX_RESULTS,
        "need_content": false,
    });
    let response = crate::client(Some(TIMEOUT))?
        .post("https://api.deepseek.com/v1/tools/search")
        .bearer_auth(key.expose())
        .json(&body)
        .send()
        .await?;
    Ok(crate::ensure_success(response).await?.json().await?)
}

async fn tavily(key: &Secret, query: &str) -> Result<serde_json::Value, AiError> {
    let body = serde_json::json!({
        "api_key": key.expose(),
        "query": query,
        "search_depth": "basic",
        "max_results": MAX_RESULTS,
        "include_answer": false,
    });
    let response = crate::client(Some(TIMEOUT))?
        .post("https://api.tavily.com/search")
        .json(&body)
        .send()
        .await?;
    Ok(crate::ensure_success(response).await?.json().await?)
}

async fn brave(key: &Secret, query: &str) -> Result<serde_json::Value, AiError> {
    let response = crate::client(Some(TIMEOUT))?
        .get("https://api.search.brave.com/res/v1/web/search")
        .query(&[("q", query), ("count", "5")])
        .header("X-Subscription-Token", key.expose())
        .header("Accept", "application/json")
        .send()
        .await?;
    Ok(crate::ensure_success(response).await?.json().await?)
}

async fn exa(key: &Secret, query: &str) -> Result<serde_json::Value, AiError> {
    let body = serde_json::json!({
        "query": query,
        "numResults": MAX_RESULTS,
        "contents": { "text": { "maxCharacters": MAX_SNIPPET } },
    });
    let response = crate::client(Some(TIMEOUT))?
        .post("https://api.exa.ai/search")
        .header("x-api-key", key.expose())
        .json(&body)
        .send()
        .await?;
    Ok(crate::ensure_success(response).await?.json().await?)
}

async fn serper(key: &Secret, query: &str) -> Result<serde_json::Value, AiError> {
    let body = serde_json::json!({ "q": query, "num": MAX_RESULTS });
    let response = crate::client(Some(TIMEOUT))?
        .post("https://google.serper.dev/search")
        .header("X-API-KEY", key.expose())
        .json(&body)
        .send()
        .await?;
    Ok(crate::ensure_success(response).await?.json().await?)
}

/// Reads one provider's response into the shared shape.
pub(crate) fn parse(provider: Provider, value: &serde_json::Value) -> Vec<WebResult> {
    match provider {
        Provider::DeepSeek => parse_deepseek(value),
        Provider::Tavily => parse_tavily(value),
        Provider::Brave => parse_brave(value),
        Provider::Exa => parse_exa(value),
        Provider::Serper => parse_serper(value),
    }
}

fn string_field(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(key)?;
    }
    let text = current.as_str()?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn result(title: Option<String>, url: Option<String>, content: &str) -> Option<WebResult> {
    let url = url?;
    Some(WebResult {
        title: title.unwrap_or_else(|| url.clone()),
        url,
        content: truncate(content.trim(), MAX_SNIPPET),
    })
}

fn parse_deepseek(value: &serde_json::Value) -> Vec<WebResult> {
    let Some(results) = value.get("results").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            let content = string_field(entry, &["summary"])
                .or_else(|| string_field(entry, &["snippet"]))
                .or_else(|| string_field(entry, &["content"]))
                .unwrap_or_default();
            result(
                string_field(entry, &["title"]),
                string_field(entry, &["url"]),
                &content,
            )
        })
        .take(MAX_RESULTS)
        .collect()
}

fn parse_tavily(value: &serde_json::Value) -> Vec<WebResult> {
    let Some(results) = value.get("results").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            result(
                string_field(entry, &["title"]),
                string_field(entry, &["url"]),
                &string_field(entry, &["content"]).unwrap_or_default(),
            )
        })
        .take(MAX_RESULTS)
        .collect()
}

fn parse_brave(value: &serde_json::Value) -> Vec<WebResult> {
    let Some(results) = value
        .get("web")
        .and_then(|web| web.get("results"))
        .and_then(|r| r.as_array())
    else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            result(
                string_field(entry, &["title"]),
                string_field(entry, &["url"]),
                &string_field(entry, &["description"]).unwrap_or_default(),
            )
        })
        .take(MAX_RESULTS)
        .collect()
}

fn parse_exa(value: &serde_json::Value) -> Vec<WebResult> {
    let Some(results) = value.get("results").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            let content = string_field(entry, &["text"])
                .or_else(|| string_field(entry, &["summary"]))
                .unwrap_or_default();
            result(
                string_field(entry, &["title"]),
                string_field(entry, &["url"]),
                &content,
            )
        })
        .take(MAX_RESULTS)
        .collect()
}

fn parse_serper(value: &serde_json::Value) -> Vec<WebResult> {
    let Some(results) = value.get("organic").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            result(
                string_field(entry, &["title"]),
                string_field(entry, &["link"]),
                &string_field(entry, &["snippet"]).unwrap_or_default(),
            )
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
    fn deepseek_prefers_its_long_summary() {
        let value = serde_json::json!({
            "results": [{
                "title": "V4",
                "url": "https://example.com/v4",
                "snippet": "short",
                "summary": "the long model-facing summary"
            }]
        });
        let results = parse(Provider::DeepSeek, &value);
        assert_eq!(results[0].content, "the long model-facing summary");
    }

    #[test]
    fn tavily_reads_results() {
        let value = serde_json::json!({
            "results": [{"title": "Rust", "url": "https://rust-lang.org", "content": "A language."}]
        });
        assert_eq!(parse(Provider::Tavily, &value)[0].title, "Rust");
    }

    #[test]
    fn brave_reads_web_results_and_descriptions() {
        let value = serde_json::json!({
            "web": {"results": [{"title": "Brave", "url": "https://brave.com", "description": "Search."}]}
        });
        let results = parse(Provider::Brave, &value);
        assert_eq!(results[0].content, "Search.");
    }

    #[test]
    fn exa_reads_text_and_falls_back_to_summary() {
        let value = serde_json::json!({
            "results": [
                {"title": "A", "url": "https://a.example", "text": "full text"},
                {"title": "B", "url": "https://b.example", "summary": "short"}
            ]
        });
        let results = parse(Provider::Exa, &value);
        assert_eq!(results[0].content, "full text");
        assert_eq!(results[1].content, "short");
    }

    #[test]
    fn serper_reads_organic_results() {
        let value = serde_json::json!({
            "organic": [{"title": "G", "link": "https://g.example", "snippet": "google result"}]
        });
        let results = parse(Provider::Serper, &value);
        assert_eq!(results[0].url, "https://g.example");
    }

    #[test]
    fn missing_or_empty_results_are_empty() {
        assert!(parse(Provider::DeepSeek, &serde_json::json!({})).is_empty());
        assert!(parse(Provider::Brave, &serde_json::json!({"web": {}})).is_empty());
        assert!(parse(Provider::Serper, &serde_json::json!({"organic": []})).is_empty());
    }

    #[test]
    fn entries_without_a_url_are_dropped() {
        let value = serde_json::json!({
            "results": [
                {"title": "no url", "summary": "x"},
                {"title": "ok", "url": "https://ok.example", "summary": "x"}
            ]
        });
        assert_eq!(parse(Provider::DeepSeek, &value).len(), 1);
    }

    #[test]
    fn at_most_five_results_survive() {
        let results: Vec<serde_json::Value> = (0..8)
            .map(|index| {
                serde_json::json!({
                    "title": format!("t{index}"),
                    "url": format!("https://e{index}.example"),
                    "summary": "x"
                })
            })
            .collect();
        let value = serde_json::json!({ "results": results });
        assert_eq!(parse(Provider::DeepSeek, &value).len(), 5);
    }

    #[test]
    fn long_snippets_are_truncated() {
        let content = "字".repeat(MAX_SNIPPET + 10);
        let value = serde_json::json!({
            "results": [{"title": "t", "url": "https://e.example", "content": content}]
        });
        let snippet = &parse(Provider::Tavily, &value)[0].content;
        assert!(snippet.ends_with('…'));
        assert_eq!(snippet.chars().count(), MAX_SNIPPET + 1);
    }

    #[test]
    fn provider_ids_round_trip() {
        for provider in [
            Provider::DeepSeek,
            Provider::Tavily,
            Provider::Brave,
            Provider::Exa,
            Provider::Serper,
        ] {
            assert_eq!(Provider::from_id(provider.id()), provider);
        }
        assert_eq!(Provider::from_id("unknown"), Provider::DeepSeek);
        assert!(Provider::DeepSeek.reuses_model_key());
        assert!(!Provider::Brave.reuses_model_key());
    }
}
