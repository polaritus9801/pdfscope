# PDFScope — one entry point for every routine task.
#
# `just` is a convenience, not a dependency: every recipe below is a short shell command that can be
# copied and run directly. The point of the file is that humans and agents run the *same* commands,
# so "works on my machine" has one fewer place to hide.

set shell := ["bash", "-euo", "pipefail", "-c"]
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

fmt:  # Format the workspace.
    cargo fmt --all

fmt-check:  # Fail if formatting differs.
    cargo fmt --all --check

lint:  # Run clippy with warnings denied.
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:  # Run the test suite.
    cargo test --workspace --all-features

doc:  # Build documentation, treating warnings as errors.
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

check: fmt-check lint test doc  # Everything CI runs, locally.
    @printf '\nAll checks passed.\n'

check-fast: fmt lint  # The cheap half, for a quick loop.

ci: check  # Alias used by CI so the pipeline and the developer agree.

audit:  # Licenses and advisories. Requires `cargo install cargo-deny`.
    cargo deny check

outdated:  # Dependency freshness. Requires `cargo install cargo-outdated`.
    cargo outdated --workspace

run:  # Run the web server on the default port.
    cargo run -p pdfscope-server

new-spec name:  # Scaffold docs/specs/NNNN-name.md.
    @./scripts/new-doc.sh spec "{{name}}"

new-adr name:  # Scaffold docs/adr/NNNN-name.md.
    @./scripts/new-doc.sh adr "{{name}}"

clean:  # Remove build artefacts.
    cargo clean
