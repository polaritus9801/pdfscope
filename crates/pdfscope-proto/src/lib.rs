//! Wire types shared by both shells.
//!
//! This crate exists so the desktop build (Tauri IPC) and the web build (HTTP) cannot drift into
//! two dialects of the same API. It holds data and nothing else: no validation, no defaults that
//! hide a missing field, no methods beyond constructors that are trivially correct.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use serde::{Deserialize, Serialize};

/// Service name reported by every health check.
pub const SERVICE_NAME: &str = "pdfscope";

/// Body of `GET /healthz`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Always [`SERVICE_NAME`]; present so a client can detect the wrong service on a port.
    pub service: String,
    /// Version of the binary that answered.
    pub version: String,
}

impl HealthResponse {
    /// The response for the running binary.
    pub fn current() -> Self {
        Self {
            service: SERVICE_NAME.to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_reports_the_crate_version() {
        let health = HealthResponse::current();
        assert_eq!(health.service, "pdfscope");
        assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn round_trips_as_json() {
        let health = HealthResponse::current();
        let json = serde_json::to_string(&health).unwrap();
        let back: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(health, back);
        assert!(json.contains("\"service\":\"pdfscope\""));
    }
}
