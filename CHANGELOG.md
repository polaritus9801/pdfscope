# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is `0.x`,
minor releases may contain breaking changes; they are always called out.

## [Unreleased]

### Changed

- The API key is stored in a `0600` file next to the settings instead of the OS keychain: no prompt on
  launch, and the trade is written down in `SECURITY.md`.


- Markdown now uses `pulldown-cmark`: tables, strikethrough and task lists join headings, lists,
  emphasis and code. Raw HTML stays escaped and citations still become interceptable links.
- The panel keeps the conversation: finished exchanges stay on screen and the last four are sent with
  the next question so pronouns resolve.

### Added

- Smooth zoom: trackpad pinch (WKWebView `gesture*` and Chromium `ctrl`+wheel), eased `ctrl`/`⌘`+wheel,
  and animated `+`/`−`/`0`. The page is anchored under the pointer (or the reading line for
  buttons), text stays visible throughout, the sharp canvas is swapped in after the hand stops, and
  a column narrower than the stage re-centres. Max 400 %; see `0017-smooth-zoom.md`.
- The reader is a continuous column: every page is a placeholder sized from the PDF, only the visible
  pages plus one either side keep a canvas, the page field follows the scroll, zoom anchors the line
  the reader is on, and `⌘↑`/`⌘↓` jump to the ends. Selection, box capture, reference tints and
  `[p.N]` jumps resolve to the page that owns them rather than to whichever page was last drawn.
  Verified in headless Chrome against 40-page and 400-page fixtures (`scripts/ui-smoke/reader.mjs`).
- Hovering a history reference tints its rectangle when you are on that page and marks its position on
  a thin rail; hovering never scrolls the document.
- `测试连接` in the settings sheet: one tiny request verifies the key, endpoint and model before they
  are saved.
- The streaming answer is announced to screen readers; the composer has a label.

### Added

- Reasoning streams in full and collapses to `思考 N 秒` once the answer starts.
- Answers render as Markdown (headings, lists, code, emphasis, inline code, citations), escaped
  first, with citations as interceptable links.
- The floating tools are 42 px icons; the panel toggles are 34 px icons.

### Fixed

- Rotated PDF pages now select the text under the pointer: the text layer gets PDF.js's
  `data-main-rotation` transforms, and a stale selection bar clears when the selection collapses or
  the page re-renders.
- Zoom no longer flashes, jumps or leaves the text blank: the column is transformed as one layer,
  the text layer stays mounted, and the old canvas remains until the resting bitmap is ready.
- Arrow keys in the question box no longer turn pages; the keyboard shortcuts now ignore both inputs
  and textareas.
- Dragging the panel divider no longer flashes a text selection on either side: the drag suppresses
  selection and clears the current one.

- The window no longer scrolls as a whole: only the reader and the panel body do.
- Box capture fires: its pointer listeners are on the document, because the page element does not
  exist when the renderer module loads.
- References moved from a toolbar into the composer, one row each, with a persistent tint on the page.
- The panel collapses and can be resized; the app icon is a proper rounded square with a margin.
- The reader gains the three floating tools (text, box, range) and pinch/ctrl-wheel zoom.

### Changed

- Streaming updates are coalesced to one per 50 ms; the answer re-parses citations on every change, so
  per-token updates made long answers progressively slower.
- A page range longer than 20 pages is reported instead of being silently truncated.
- `⌘↵` sends and `Esc` stops, from the composer.

### Added

- Reference history: a send moves the tray into a history list; hovering a row reveals the quote after
  600 ms, a plain click redeems it back into the tray, and shift-click jumps to its page.
- Page context: a typed range (`31-45, 60`) or, when the tray is empty, the current page, is fetched
  page by page and sent with `[第 N 页]` labels. A scan is labelled as having no extractable text.
- Stop and retry: while streaming the send button becomes `停止`; a failure turns it into `重试`.
- Multimodal requests: a captured region is sent as an image part — `image_url` on the OpenAI protocol,
  an `image` block on Anthropic — with a page label in the prompt. Six images and 4 MB each are the
  caps, checked before anything is sent.
- Region capture: `框选` arms a marquee; the crop is re-rendered from the PDF at the scale needed for
  its long side to reach 1600 px and encoded as PNG, so it is sharp regardless of zoom or display
  density. The chip shows a thumbnail and can be removed. No OCR is involved.
- Clickable citations: `[p.31]` in an answer becomes a button that jumps to page 31 and flashes the
  page frame. Parsing lives in `pdfscope-core`, so the accepted dialects are unit-tested.
- Ask: a settings sheet (preset, endpoint, model, key), the key stored in the OS keychain, and a
  streamed answer panel with a reasoning counter and token usage. The request is built from exactly
  the reference tray; nothing else is sent.
- Model providers behind one interface: OpenAI-compatible endpoints (OpenAI, DeepSeek, MiMo,
  Moonshot, OpenRouter, Gemini's compatibility endpoint, Ollama) and Anthropic's native Messages API.
  Streaming normalises reasoning tokens, answer tokens and usage — including cache hits — into one
  event type.
- A live test against MiMo `mimo-v2.6-flash`, ignored by default because it needs a key and spends
  tokens.
- Text layer and selection: text on a page is selectable, the selection is reported as a page plus a
  normalised rectangle plus the quote, and `加入引用` turns it into a chip in the reference tray.
  Scanned pages say so instead of silently selecting nothing.
- Desktop shell: a Tauri window that opens a PDF through the system file picker, renders pages with
  PDF.js at fit width, and navigates and zooms with the keyboard or the toolbar. The page stays white
  in dark mode. PDF.js is vendored and reached through a three-function contract, so the Rust side
  never touches a canvas.
- The lilac theme tokens (`docs/design/tokens.md`), applied to the shell.
- `just ui`, `just desktop` and `just ui-dev` for the UI and the desktop loop.
- Anchoring: a selection is captured with its page, quote, surrounding context and a page hash, and
  resolved later to `Exact`, `Relocated`, `Ambiguous` or `NeedsReview` — never to a guessed
  position.
- Text normalisation shared by capture and resolution: whitespace runs, ideographic spaces and the
  usual PDF ligatures collapse identically on both sides.
- Workspace layout with one-way crate dependencies: `core`, `proto`, `store`, `ai`, `api`, `server`.
- Domain primitives: content-hash book identity, zero-based page indices that display one-based,
  normalised page rectangles with a validated invariant.
- Credential type whose `Debug` cannot leak the secret.
- SQLite library schema with an in-memory mode for tests.
- `GET /healthz` served by both shells from one router.
- CI: formatting, clippy with warnings denied, tests on Linux/macOS/Windows, documentation build,
  license and advisory checks, and a spec gate for pull requests.
- Pre-commit hook that refuses product code without a spec.
- Repository protocol in `AGENTS.md`, architecture in `docs/ARCHITECTURE.md`, decision records in
  `docs/adr/`, and specification templates in `docs/specs/`.

[Unreleased]: https://github.com/polaritus9801/pdfscope/commits/main
