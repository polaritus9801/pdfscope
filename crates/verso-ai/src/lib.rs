//! Model providers, streaming, and secret handling.
//!
//! The crate is intentionally thin at this stage: it owns the two things that are expensive to get
//! wrong later — how a provider is configured, and how a credential is kept out of logs. The
//! request/stream layer arrives with the spec that needs it.
//!
//! Provider-agnostic by design: everything speaks the OpenAI-compatible surface, because that is
//! the one protocol every local runtime (Ollama, LM Studio, vLLM) and every hosted API either
//! implements or adapts to.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::fmt;

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
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// Errors produced by the AI layer.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum AiError {
    /// A base URL was missing or was not HTTP(S).
    #[error("invalid base URL `{0}`: expected an http:// or https:// URL")]
    InvalidBaseUrl(String),

    /// A model name was empty.
    #[error("model name must not be empty")]
    EmptyModel,
}

/// Everything needed to talk to one provider endpoint.
///
/// Constructed through [`ProviderConfig::new`], which rejects configurations that would fail at
/// request time instead of at startup.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    base_url: String,
    model: String,
    api_key: Secret,
}

impl ProviderConfig {
    /// Builds a configuration, validating the parts that are cheap to validate.
    pub fn new(
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

        Ok(Self {
            base_url,
            model,
            api_key,
        })
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_debug_is_redacted() {
        let secret = Secret::new("sk-live-should-never-appear");
        let rendered = format!("{secret:?}");
        assert!(!rendered.contains("should-never-appear"));
        assert_eq!(rendered, "Secret(<redacted>)");
    }

    #[test]
    fn config_debug_does_not_leak_the_secret() {
        let config = ProviderConfig::new(
            "https://api.example.com/v1",
            "some-model",
            Secret::new("hunter2"),
        )
        .unwrap();
        let rendered = format!("{config:?}");
        assert!(rendered.contains("some-model"));
        assert!(!rendered.contains("hunter2"));
    }

    #[test]
    fn rejects_a_base_url_without_a_scheme() {
        let result = ProviderConfig::new("api.example.com", "m", Secret::new("k"));
        assert_eq!(
            result.unwrap_err(),
            AiError::InvalidBaseUrl("api.example.com".to_owned())
        );
    }

    #[test]
    fn rejects_an_empty_model() {
        let result = ProviderConfig::new("https://api.example.com/v1", "  ", Secret::new("k"));
        assert_eq!(result.unwrap_err(), AiError::EmptyModel);
    }

    #[test]
    fn exposes_the_credential_only_on_request() {
        let secret = Secret::new("sk-123");
        assert_eq!(secret.expose(), "sk-123");
    }
}
