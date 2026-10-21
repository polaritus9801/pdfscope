# 0005 — Desktop shell and reading surface

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("继续做")

## Goal

A person can launch the app, open a PDF, see a page, move between pages, and change the zoom — on
macOS, Windows and Linux, from one codebase. Nothing about AI yet: this slice exists so the reading
surface is real before anything is layered on it.

## Non-goals

- No AI, no references, no library, no persistence, no annotations.
- No text selection behaviour beyond what the browser gives for free (selection arrives in 0006).
- No OCR.
- No auto-update, no signing, no installer. `just run-desktop` is the entry point.

## Behaviour

- The window opens at 1280×860, minimum 900×600, titled `PDFScope`.
- An empty state says what to do: open a file.
- Opening a file renders page 1 at **fit width**.
- Page navigation: previous, next, and a page field that accepts a number and `↵`.
- Zoom: 50%…300% in steps, plus **fit width** (the 100% preset means fit width, not a physical size).
- A failed load shows a message and leaves the app usable.
- The window follows the system colour scheme; the page stays white in both.
- Keyboard: `←/→` or `PageUp/PageDown` navigate, `+/-` zoom, `⌘O` opens.

## Interface

The reading surface is PDF.js, vendored in `ui/public/pdfjs/`, wrapped by a single module that
exposes three functions on `window.PDFScope`:

```js
load(bytes: Uint8Array): Promise<number>   // resolves to the page count
render(page: number, zoomPercent: number): Promise<void>
pageText(page: number): Promise<string>    // for 0006; unused here
```

The boundary is deliberate: **PDF.js never leaks into the Rust side** and the Rust side never
re-implements rendering. Leptos owns application state; the module owns the document and the canvas.

The shell is Tauri 2, with `frontendDist` pointing at the Trunk output. The web build (0009) serves
the same assets from `pdfscope-server`; nothing in the UI knows which one it is running in.

## Data model

None. No file is remembered between launches in this slice.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Open | `打开 PDF` | header, left | always | click opens the system file picker | `⌘O` | tap | page 1 renders at fit width; the filename replaces the title | unreadable file → inline message under the header; the previous document stays |
| Previous / next | `‹` `›` | header, centre | a document is open and the move is in range | click | `←` `→`, `PageUp` `PageDown` | tap | the canvas redraws; the page field updates | at the first or last page the button is disabled, not hidden |
| Page field | `31 / 404` | header, centre | a document is open | click, type, `↵` | focus with `Tab`, `↵` commits | tap | jumps to that page; out-of-range input is clamped and the field snaps back | non-numeric input reverts on blur without a message |
| Zoom | `−` `100%` `+` | header, right | a document is open | click; hold to repeat | `+` `-` | tap | the canvas redraws at the new scale, keeping the top of the page in view | at the limits the buttons disable |
| Fit width | `适应宽度` | zoom menu | a document is open | click | `0` | tap | zoom returns to 100% | — |
| Theme | `跟随系统` | header, far right | always | cycles system → light → dark | `Tab` + `↵` | tap | the shell repaints; the PDF stays white | — |

**Alternatives considered.**

- Rendering with a Rust PDF library into a native surface. Rejected in ADR 0002: no mature text layer
  or selection, which the product depends on.
- A native file dialog through a Tauri plugin for the first slice. Rejected for now: the webview's
  file input is one element, works identically in the browser build, and adds no permission surface.
  The Rust-side dialog arrives with the library (0007).

**Discoverability.** The empty state names the only action. The `⌘O` hint sits under it in
`--ink-faint`.

**States.**

| State | Header controls | Canvas | Empty state |
|---|---|---|---|
| No document | only Open and Theme are enabled | hidden | visible |
| Loading | disabled | the previous page remains | hidden |
| Ready | all enabled | current page | hidden |
| Failed | Open remains enabled | hidden | replaced by the error message |

**Cost of being wrong.** A rendering failure must not lose the open document: the file stays loaded
and the error is reported without clearing the canvas. A zoom change must not scroll the reader to
the top of the page.

## Design review

- Reviewed with: z1mLi on 2026-10-21 (the shell was described in the conversation; the controls are
  the minimum needed to read a page)
- Decisions: file input over a native dialog for this slice; `100%` means fit width; page stays white
  in dark mode.

## Test plan

- Build: `trunk build` produces `ui/dist`, and `cargo build -p pdfscope-desktop` embeds it.
- Manual checklist on macOS: open a text PDF, open a 400-page scanned PDF, navigate past the ends,
  zoom to both limits, type an out-of-range page, delete the file while it is open.
- Worker: the PDF.js worker is loaded from the copied asset; the console shows no 404.
- Layout: the canvas fits the window at 900 px width without a horizontal scrollbar.

## Risks

- **WebView differences.** PDF.js worker and canvas behave differently on WebKitGTK. The manual
  checklist must be run on Linux before this slice is called done; until then the README says macOS
  only.
- **A large PDF blocks the first paint.** Mitigation: rendering is per page, and navigation cancels
  the previous render.
- **Fit-width math ignores the vertical scrollbar**, producing a 15 px horizontal scrollbar.
  Mitigation: measure the host element, not the window.

## Assumptions

- Tauri 2's `frontendDist` accepts a relative path to the Trunk output.
- The file input works inside every target webview without extra permissions; unverified on Windows.

## Deviations

- Tauri requires a window icon even with `bundle.active: false`, and the icon must be RGBA. The
  icons are generated from the brand colour and committed under `crates/pdfscope-desktop/icons/`.
- The window could not be captured in the build environment (no display session). The shell is
  verified by a headless browser driving the same UI bundle, and by a renderer check that loads a
  real 26-page PDF and reports the resulting canvas width. The window itself needs a manual check on
  a desktop session; the README says so.
- `ui/` is excluded from the Cargo workspace because it builds for `wasm32` and has its own release
  profile. `just fmt` and `just check` therefore format it separately.

## Open questions

1. Should the window remember its size and the last document path? Proposed: with the library (0007),
   not here.
