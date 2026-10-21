# 0002 — Web UI inside Tauri, PDF.js for rendering

- **Status:** accepted
- **Date:** 2026-10-21
- **Deciders:** z1mLi

## Context

PDFScope must ship on macOS, Windows, and Linux, and — because "read this PDF with me" is a request
people make from a browser — ideally on the web too, from one codebase. It must be small, start
fast, and render PDFs with a selectable text layer, because every core interaction (select, cite,
highlight back) depends on knowing where each glyph is.

"All Rust" is a preference about code quality, not a goal in itself: the constraint is that the
logic is Rust, not that every pixel is drawn by Rust.

## Options considered

### Option A — Native Rust GUI (Slint, Iced, egui)

- Pros: one language, no WebView, predictable performance.
- Cons: **no mature PDF view with text selection, zoom, and layout.** Every one of those would be
  built from scratch, which is a multi-year project disguised as a UI choice. Component ecosystems
  for rich text and accessibility are thin.

### Option B — Electron with a TypeScript UI

- Pros: the largest component ecosystem; PDF.js is a first-class citizen.
- Cons: 100 MB+ installers, a second runtime, and the UI would not be Rust. Contradicts the size
  goal on a reading tool that should feel instant.

### Option C — Tauri 2 with a Leptos UI, PDF.js in the WebView

- Pros: native binaries of a few megabytes; system WebView so no bundled browser; Rust for all
  logic and IPC; the same UI runs in a browser for a future web build; PDF.js supplies the text
  layer, selection model, and annotation layer that the product depends on.
- Cons: WebView differences across platforms (especially WebKitGTK); a Wasm front end has a thinner
  component ecosystem than React; PDF.js is JavaScript and must be vendored.

### Option D — Tauri with a TypeScript UI

Mature ecosystem, faster UI work, but abandons the Rust-everywhere preference for the layer with the
most code. Rejected on preference, not on merit; it remains the fallback if Leptos blocks progress.

## Decision

We will build the desktop shell with **Tauri 2**, the UI with **Leptos** compiled to Wasm, and render
PDFs with **PDF.js**, treating the renderer as a component we own but do not rewrite.

The web build serves the same UI assets from `pdfscope-server`; only the transport between UI and
`pdfscope-api` differs.

## Consequences

- Logic lives in Rust and is testable without a browser.
- UI work will occasionally be slower than the React equivalent; the escape hatch is Option D,
  which is possible because the UI talks to `pdfscope-api` rather than to Rust directly.
- Three WebViews must be tested for PDF.js behaviour. This is a known, budgeted cost, and it is
  cheaper than writing a renderer.
- PDF.js version, worker file, and CMaps are vendored and pinned. An unpinned renderer is an
  unreproducible bug.

## References

- [PDF.js text layer](https://mozilla.github.io/pdf.js/)
- [Tauri 2](https://tauri.app)
- [Leptos](https://leptos.dev)
