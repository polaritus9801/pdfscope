# PDFScope — one entry point for every routine task.
#
# `just` is a convenience, not a dependency: every recipe below is a short shell command that can be
# copied and run directly. The point of the file is that humans and agents run the *same* commands,
# so "works on my machine" has one fewer place to hide.

set shell := ["bash", "-euo", "pipefail", "-c"]

# rustup was installed without touching the shell profile, so make `cargo` and `trunk` reachable
# from every recipe regardless of how the terminal was configured.
export PATH := env_var("HOME") + "/.cargo/bin:" + env_var_or_default("PATH", "/usr/local/bin:/usr/bin:/bin")
set dotenv-load := false

_default: help

help:  # List available recipes.
    @just --list --unsorted

setup:  # Configure git hooks and verify the toolchain.
    git config core.hooksPath .githooks
    git config commit.template .gitmessage
    @printf 'hooks: %s\n' "$(git config core.hooksPath)"
    @printf 'commit template: %s\n' "$(git config commit.template)"
    @rustc --version
    @cargo --version
    @printf '\nReady. Run `just check`.\n'

fmt:  # Format the workspace and the UI.
    cargo fmt --all
    cd ui && cargo fmt

fmt-check:  # Fail if formatting differs.
    cargo fmt --all --check
    cd ui && cargo fmt --check

lint:  # Run clippy with warnings denied.
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:  # Run the test suite.
    cargo test --workspace --all-features

doc:  # Build documentation, treating warnings as errors.
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

tokens:  # Fail if a component rule hard-codes a pixel value without saying why.
    node scripts/check-tokens.mjs

check: ui tokens fmt-check lint test doc  # Everything CI runs, locally (UI first: the shell embeds it).
    @printf '\nAll checks passed.\n'

check-fast: fmt lint  # The cheap half, for a quick loop.

ci: check  # Alias used by CI so the pipeline and the developer agree.

audit:  # Licenses and advisories. Requires `cargo install cargo-deny`.
    cargo deny check

outdated:  # Dependency freshness. Requires `cargo install cargo-outdated`.
    cargo outdated --workspace

run:  # Run the web server on the default port.
    cargo run -p pdfscope-server

ui:  # Build the UI bundle into ui/dist.
    cd ui && trunk build

ui-dev:  # Serve the UI with hot reload on :8080.
    cd ui && trunk serve --port 8080

desktop: ui  # Build the UI, then run the desktop shell.
    cargo run -p pdfscope-desktop

desktop-check: ui  # Type-check the desktop shell.
    cargo check -p pdfscope-desktop

new-spec name:  # Scaffold docs/specs/NNNN-name.md.
    @./scripts/new-doc.sh spec "{{name}}"

new-adr name:  # Scaffold docs/adr/NNNN-name.md.
    @./scripts/new-doc.sh adr "{{name}}"

clean:  # Remove build artefacts.
    cargo clean
