<div align="center">

# Verso

**Read PDFs with AI. Page by page, with citations that actually point to the page.**

[![CI](https://github.com/polaritus9801/verso/actions/workflows/ci.yml/badge.svg)](https://github.com/polaritus9801/verso/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-black.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-black.svg)](rust-toolchain.toml)

</div>

---

Verso is a local-first PDF reader that puts an AI beside the page you are reading. Select text,
drag a box around a figure, or point it at a page range — the answer comes back with the page it
came from, so you can check it instead of trusting it.

It is not a notes app. Your notes stay in the tools you already use: copy a citation-ready block
into Obsidian, Typora, or anything that takes Markdown.

## Why

Feeding a whole PDF to a chat model loses the one thing that makes reading possible: **the page**.
Answers arrive without provenance, scans have no text layer at all, and a 400-page book becomes a
single undifferentiated blob.

Verso is built around the opposite assumption — the page is the unit of work.

- **Text, boxes, and ranges.** Ask about a selection, a dragged rectangle, one page, or a chapter.
- **Citations that resolve.** Every answer carries the page it drew from; click it and you are there.
- **Scans are first-class.** When a page has no text layer, it is rendered and read visually, not
  silently skipped.
- **Small and local.** Native binaries, keys in the OS keychain, no account, no upload of your library.

## Status

Early. The repository is a deliberately complete foundation — workspace boundaries, CI, decision
records, and an agent workflow — before any product behaviour exists. See
[`docs/specs/`](docs/specs/) for what is planned and [`docs/adr/`](docs/adr/) for what has been
decided and why.

## Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | [Tauri 2](https://tauri.app) | Native binaries, system WebView, small download |
| UI | [Leptos](https://leptos.dev) + design tokens | Rust end to end; no framework churn |
| PDF | [PDF.js](https://mozilla.github.io/pdf.js/) | The only mature text layer with selection and layout |
| Core | Rust workspace | Boundaries that survive contributors |
| Storage | SQLite via `sqlx` | One file, no server, same on every OS |

The reasoning, including the options rejected, is in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Build

```sh
git clone https://github.com/polaritus9801/verso.git
cd verso
just setup   # installs git hooks, checks toolchain
just check   # fmt + clippy + tests
```

Requires Rust (see [`rust-toolchain.toml`](rust-toolchain.toml)). `just` is optional but assumed
throughout this repository.

## Contributing

Read [`AGENTS.md`](AGENTS.md) first — it is the protocol every change follows, human or AI. In
short: no product code without an approved spec, one vertical slice per commit, and a definition
of done that includes tests and documentation.

- [Architecture](docs/ARCHITECTURE.md)
- [Specifications](docs/specs/)
- [Decision records](docs/adr/)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)

## License

[MIT](LICENSE)
