//! Model providers and streaming.
//!
//! Two wire protocols cover every target: OpenAI Chat Completions — used by OpenAI, DeepSeek, MiMo,
//! Moonshot, OpenRouter, Ollama, LM Studio and Gemini's compatibility endpoint — and Anthropic
//! Messages, used by Claude and by MiMo's Anthropic endpoint.
//!
//! Everything above this crate sees [`StreamEvent`] and nothing else, so switching provider is a
//! configuration change rather than an integration.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod anthropic;
pub mod openai;
pub mod search;
pub mod sse;

use std::fmt;
use std::pin::Pin;

use futures::Stream;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A credential that must never appear in output.
///
/// `Debug` is implemented by hand to print a fixed placeholder, so the value cannot leak through a
/// derived `{:?}`, a `tracing` field, or a panic message that formats a struct containing it.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// Wraps a credential.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The credential itself. Call this only where the value is about to be sent to its provider.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether a credential was supplied at all.
    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// The wire protocol a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    /// OpenAI Chat Completions and every compatible server.
    OpenAi,
    /// Anthropic Messages.
    Anthropic,
}

/// A provider the settings screen can offer by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// What the user picks.
    pub label: &'static str,
    /// Which protocol it speaks.
    pub protocol: Protocol,
    /// Endpoint root, without a trailing slash.
    pub base_url: &'static str,
}

/// The presets shipped with the application.
///
/// A user may always type their own URL; these exist so that the common case is a name, not a URL.
pub const PRESETS: &[Preset] = &[
    Preset {
        label: "OpenAI",
        protocol: Protocol::OpenAi,
        base_url: "https://api.openai.com/v1",
    },
    Preset {
        label: "Anthropic",
        protocol: Protocol::Anthropic,
        base_url: "https://api.anthropic.com/v1",
    },
    Preset {
        label: "DeepSeek",
        protocol: Protocol::OpenAi,
        base_url: "https://api.deepseek.com/v1",
    },
    Preset {
        label: "MiMo",
        protocol: Protocol::OpenAi,
        base_url: "https://api.xiaomimimo.com/v1",
    },
    Preset {
        label: "Moonshot",
        protocol: Protocol::OpenAi,
        base_url: "https://api.moonshot.cn/v1",
    },
    Preset {
        label: "OpenRouter",
        protocol: Protocol::OpenAi,
        base_url: "https://openrouter.ai/api/v1",
    },
    Preset {
        label: "Gemini (compat)",
        protocol: Protocol::OpenAi,
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
    },
    Preset {
        label: "Ollama",
        protocol: Protocol::OpenAi,
        base_url: "http://127.0.0.1:11434/v1",
    },
];

/// Everything needed to talk to one endpoint.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    label: String,
    protocol: Protocol,
    base_url: String,
    model: String,
    api_key: Secret,
    /// How long one request may take; `None` waits as long as the provider does.
    timeout: Option<std::time::Duration>,
}

impl ProviderConfig {
    /// Builds a configuration, rejecting the values that would fail at request time.
    pub fn new(
        label: impl Into<String>,
        protocol: Protocol,
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Secret,
    ) -> Result<Self, AiError> {
        let base_url = base_url.into();
        let model = model.into();

        if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
            return Err(AiError::InvalidBaseUrl(base_url));
        }
        if model.trim().is_empty() {
            return Err(AiError::EmptyModel);
        }
        if api_key.is_empty() {
            return Err(AiError::EmptyKey);
        }

        Ok(Self {
            label: label.into(),
            protocol,
            base_url: base_url.trim_end_matches('/').to_owned(),
            model,
            api_key,
            timeout: None,
        })
    }

    /// Human-readable provider name.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The protocol this provider speaks.
    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    /// Endpoint root, without a trailing slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Model identifier as the provider expects it.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The credential, for the request layer.
    pub fn api_key(&self) -> &Secret {
        &self.api_key
    }

    /// Sets the per-request limit; `None` waits as long as the provider does.
    pub fn with_timeout(mut self, timeout: Option<std::time::Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// The configured limit, if any.
    pub(crate) fn timeout(&self) -> Option<std::time::Duration> {
        self.timeout
    }
}

/// Who is speaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The reader.
    User,
    /// The model.
    Assistant,
}

/// An image attached to a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// MIME type, for example `image/png`.
    pub mime: String,
    /// The bytes, base64-encoded, without the data-URL prefix.
    pub base64: String,
}

impl Image {
    /// Accepts `data:image/png;base64,…` and nothing else.
    pub fn from_data_url(url: &str) -> Option<Self> {
        let rest = url.strip_prefix("data:")?;
        let (meta, data) = rest.split_once(',')?;
        let mime = meta.strip_suffix(";base64")?;
        if !mime.starts_with("image/") || data.is_empty() {
            return None;
        }
        Some(Self {
            mime: mime.to_owned(),
            base64: data.to_owned(),
        })
    }
}

/// One turn of a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Who said it.
    pub role: Role,
    /// What was said.
    pub content: String,
    /// Images attached to this turn, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
}

impl Message {
    /// A message from the reader.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            images: Vec::new(),
        }
    }

    /// A message from the reader with images attached.
    pub fn user_with_images(content: impl Into<String>, images: Vec<Image>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            images,
        }
    }

    /// A message from the model.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            images: Vec::new(),
        }
    }
}

/// A request to a provider.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// Instructions that apply to the whole conversation.
    pub system: Option<String>,
    /// The turns so far, oldest first.
    pub messages: Vec<Message>,
    /// An upper bound on generated tokens; `None` means no limit where the provider allows it.
    pub max_tokens: Option<u32>,
    /// Sampling temperature, when the provider takes one.
    pub temperature: Option<f32>,
}

impl ChatRequest {
    /// A single-question request with a system prompt and a default token budget.
    pub fn new(system: impl Into<String>, question: impl Into<String>) -> Self {
        Self {
            system: Some(system.into()),
            messages: vec![Message::user(question)],
            max_tokens: Some(2048),
            temperature: None,
        }
    }
}

/// What a provider reported about a request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    /// Tokens sent.
    pub prompt_tokens: u32,
    /// Tokens generated.
    pub completion_tokens: u32,
    /// Prompt tokens served from the provider's cache, when it reports them.
    pub cached_tokens: u32,
}

/// One step of a streamed answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// A reasoning token, when the model exposes its thinking separately.
    Reasoning(String),
    /// A visible answer token.
    Text(String),
    /// Reported once the provider knows the totals.
    Usage(Usage),
    /// The stream ended normally.
    Done,
}

/// A streamed answer.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, AiError>> + Send>>;

/// Errors produced by the AI layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AiError {
    /// A base URL was missing or was not HTTP(S).
    #[error("invalid base URL `{0}`: expected an http:// or https:// URL")]
    InvalidBaseUrl(String),

    /// A model name was empty.
    #[error("model name must not be empty")]
    EmptyModel,

    /// No credential was supplied.
    #[error("an API key is required for this provider")]
    EmptyKey,

    /// The request could not be sent or the connection failed mid-stream.
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),

    /// The provider answered with a non-success status.
    #[error("provider returned {status}: {body}")]
    Status {
        /// HTTP status code.
        status: u16,
        /// The provider's own message, truncated for display.
        body: String,
    },

    /// A frame was not the JSON the protocol promised.
    #[error("could not decode a provider frame: {0}")]
    Decode(#[from] serde_json::Error),

    /// The stream ended without its terminating event.
    #[error("the provider ended the stream without completing the answer")]
    Incomplete,
}

/// Streams an answer from the configured provider.
pub async fn stream(
    config: &ProviderConfig,
    request: &ChatRequest,
) -> Result<EventStream, AiError> {
    match config.protocol() {
        Protocol::OpenAi => openai::stream(config, request).await,
        Protocol::Anthropic => anthropic::stream(config, request).await,
    }
}

/// Shared HTTP client settings.
pub(crate) fn client(timeout: Option<std::time::Duration>) -> Result<reqwest::Client, AiError> {
    let mut builder = reqwest::Client::builder();
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    builder.build().map_err(AiError::Transport)
}

/// Reads a failed response body so the provider's message reaches the user.
pub(crate) async fn ensure_success(
    response: reqwest::Response,
) -> Result<reqwest::Response, AiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response.text().await.unwrap_or_default();
    let body = body.chars().take(400).collect::<String>();
    Err(AiError::Status {
        status: status.as_u16(),
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_png_data_url_becomes_an_image() {
        let image = Image::from_data_url("data:image/png;base64,AAAA").unwrap();
        assert_eq!(image.mime, "image/png");
        assert_eq!(image.base64, "AAAA");
    }

    #[test]
    fn other_data_urls_are_rejected() {
        assert!(Image::from_data_url("data:text/plain;base64,AAAA").is_none());
        assert!(Image::from_data_url("data:image/png;base64,").is_none());
        assert!(Image::from_data_url("https://example.com/a.png").is_none());
    }

    fn config(model: &str, key: &str) -> Result<ProviderConfig, AiError> {
        ProviderConfig::new(
            "Test",
            Protocol::OpenAi,
            "https://example.com/v1/",
            model,
            Secret::new(key),
        )
    }

    #[test]
    fn secret_debug_is_redacted() {
        let secret = Secret::new("sk-live-should-never-appear");
        let rendered = format!("{secret:?}");
        assert!(!rendered.contains("should-never-appear"));
        assert_eq!(rendered, "Secret(<redacted>)");
    }

    #[test]
    fn config_debug_does_not_leak_the_secret() {
        let config = config("some-model", "hunter2").unwrap();
        let rendered = format!("{config:?}");
        assert!(rendered.contains("some-model"));
        assert!(!rendered.contains("hunter2"));
    }

    #[test]
    fn base_url_loses_its_trailing_slash() {
        assert_eq!(
            config("m", "k").unwrap().base_url(),
            "https://example.com/v1"
        );
    }

    #[test]
    fn rejects_a_base_url_without_a_scheme() {
        let result = ProviderConfig::new(
            "Test",
            Protocol::OpenAi,
            "api.example.com",
            "m",
            Secret::new("k"),
        );
        assert!(matches!(result, Err(AiError::InvalidBaseUrl(_))));
    }

    #[test]
    fn rejects_an_empty_model() {
        assert!(matches!(config("  ", "k"), Err(AiError::EmptyModel)));
    }

    #[test]
    fn rejects_an_empty_key() {
        assert!(matches!(config("m", "  "), Err(AiError::EmptyKey)));
    }

    #[test]
    fn presets_are_unique_and_well_formed() {
        for preset in PRESETS {
            assert!(preset.base_url.starts_with("http"));
            assert!(!preset.label.is_empty());
        }
        let mut labels: Vec<_> = PRESETS.iter().map(|preset| preset.label).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), PRESETS.len());
    }
}
