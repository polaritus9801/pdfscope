//! Build script for the desktop shell: embeds the window configuration.

fn main() {
    // Trunk replaces the hashed JS/CSS files in ui/dist on every UI build. Tauri's context cache
    // otherwise keeps the previous hashed asset list and fails to compile when that file is gone.
    println!("cargo:rerun-if-changed=../../ui/dist");
    println!("cargo:rerun-if-changed=../../ui/dist/index.html");
    println!("cargo:rerun-if-changed=../../ui/dist/composer.js");
    tauri_build::build();
}
