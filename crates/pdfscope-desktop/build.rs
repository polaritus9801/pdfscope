//! Build script for the desktop shell: embeds the window configuration and the UI assets.

use std::fs;
use std::path::Path;

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist");

    // Trunk stages copied assets in `dist/.stage` while applying a distribution. If a build starts
    // in that window, Tauri embeds transient `.stage/...` paths that do not exist afterwards. Remove
    // a leftover stage before Tauri scans the directory; normal UI builds run before this script.
    let stage = dist.join(".stage");
    if stage.is_dir() {
        let _ = fs::remove_dir_all(&stage);
    }

    // Trunk replaces the hashed JS/CSS files in ui/dist on every UI build. Tauri's context cache
    // otherwise keeps the previous hashed asset list and fails to compile when that file is gone.
    println!("cargo:rerun-if-changed=../../ui/dist");
    println!("cargo:rerun-if-changed=../../ui/dist/index.html");
    println!("cargo:rerun-if-changed=../../ui/dist/composer.js");
    tauri_build::build();
}
