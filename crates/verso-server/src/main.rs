//! The web binary.
//!
//! This crate is composition only: parse the environment, build the router, bind a listener, run.
//! Any line here that starts looking like a rule belongs in `verso-api` or `verso-core` instead.

use std::net::SocketAddr;

use anyhow::Context;
use tracing_subscriber::EnvFilter;

/// Default port. Deliberately uncommon, so it does not collide with a development server already
/// running on the machine.
const DEFAULT_PORT: u16 = 8787;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let port = std::env::var("VERSO_PORT")
        .ok()
        .map(|raw| raw.parse::<u16>())
        .transpose()
        .context("VERSO_PORT must be a number between 0 and 65535")?
        .unwrap_or(DEFAULT_PORT);

    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind {address}"))?;

    tracing::info!(%address, "verso-server listening");

    axum::serve(listener, verso_api::router(verso_api::AppState::current()))
        .await
        .context("server stopped unexpectedly")?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
