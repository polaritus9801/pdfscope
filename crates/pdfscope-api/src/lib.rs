//! The HTTP surface over the PDFScope domain.
//!
//! Both shells talk to this crate: the web build over TCP, the desktop build by dispatching the
//! same handlers through Tauri IPC. Keeping one implementation is what stops the two builds from
//! becoming two products.
//!
//! Handlers validate, delegate, and format. They do not contain domain rules — those live in
//! `pdfscope-core` — and they do not touch storage directly.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use axum::{Json, Router, extract::State, routing::get};
use pdfscope_proto::{HealthResponse, SERVICE_NAME};
use tower_http::trace::TraceLayer;

/// Shared state for every handler.
///
/// Small on purpose: anything that needs a database or a model client is added when a spec needs
/// it, not pre-emptively.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Version reported by the health endpoint.
    pub version: String,
}

impl AppState {
    /// State for the running build.
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

/// Builds the router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        service: SERVICE_NAME.to_owned(),
        version: state.version,
    })
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_reports_the_service_and_version() {
        let app = router(AppState::current());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let health: HealthResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(health.service, "pdfscope");
        assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn unknown_routes_are_not_found() {
        let app = router(AppState::current());

        let response = app
            .oneshot(Request::builder().uri("/nope").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
