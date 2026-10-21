# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is `0.x`,
minor releases may contain breaking changes; they are always called out.

## [Unreleased]

### Added

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
