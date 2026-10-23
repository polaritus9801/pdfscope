//! The desktop shell.
//!
//! This crate is a composition root: it opens a window, loads the built UI, and nothing else. Every
//! rule it might be tempted to hold belongs in `pdfscope-api` or `pdfscope-core` instead.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod ask;
mod settings;

use anyhow::Context;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    init_tracing();

    tauri::Builder::default()
        .manage(ask::AskControl::default())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                if let Some(state) = settings::remembered_window(app.handle()) {
                    let _ = window.set_size(tauri::PhysicalSize::new(state.width, state.height));
                    let _ = window.set_position(tauri::PhysicalPosition::new(state.x, state.y));
                    if state.maximized {
                        let _ = window.maximize();
                    }
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let Ok(size) = window.outer_size() else {
                    return;
                };
                let Ok(position) = window.outer_position() else {
                    return;
                };
                let maximized = window.is_maximized().unwrap_or(false);

                let _ = settings::remember_window(
                    window.app_handle(),
                    settings::WindowState {
                        width: size.width,
                        height: size.height,
                        x: position.x,
                        y: position.y,
                        maximized,
                    },
                );
            }
        })
        .invoke_handler(tauri::generate_handler![
            settings::load_settings,
            settings::save_settings,
            settings::test_settings,
            settings::web_search,
            ask::ask,
            ask::stop_ask
        ])
        .run(tauri::generate_context!())
        .context("the desktop shell stopped unexpectedly")?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
