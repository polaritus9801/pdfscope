//! The desktop shell.
//!
//! This crate is a composition root: it opens a window, loads the built UI, and nothing else. Every
//! rule it might be tempted to hold belongs in `pdfscope-api` or `pdfscope-core` instead.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use anyhow::Context;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    init_tracing();

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .context("the desktop shell stopped unexpectedly")?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
