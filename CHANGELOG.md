# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is `0.x`,
minor releases may contain breaking changes; they are always called out.

## [Unreleased]

### Changed

- Undo/redo moved to `pdfscope-core::history`, with six tests for truncation, the depth limit and the
  round trip.


- What a question carries is now `pdfscope-core::pages::request_pages`, with eight tests covering the
  cap, clamping, filtering and the empty cases.


- Page-set merging and subtraction moved to `pdfscope-core::pages` with nine unit tests; the UI calls
  the tested functions.


- In paged modes the zoom readout reports the share of a full page on screen (54 % single, 50 %
  spread) instead of the column's percentage.


- Single-page mode fits the whole page to the window instead of showing its top third.


- The reader smoke suite covers single mode switching: one page at the top, then back to the column
  (83 checks).


- The reader smoke suite can run with extra Chrome flags; with reduced motion forced it still passes
  81/81.


- `docs/performance.md` now records the 10 MB first-page budget (60 ms) and typing latency (17 ms),
  both inside the spec's targets.


- The shell was restyled without touching the palette or fonts: floating panel card, paper-like page,
  blurred header and a vertical tool pill with a real active state.
- The three tools are now modes (文字 / 框选 / 页); 框选 toggles and follows the renderer when Esc
  disarms it. Failures appear as an inline card with 重试.


- Every remaining pixel value in the stylesheet is either a token or annotated `/* size */`;
  `just tokens` enforces it in `just check`.


- Spacing, radii, type sizes and offsets now come from the token scale; off-scale spacing snapped to
  the nearest step (223 → 109 hard-coded declarations).


- The AI panel reads like a chat: questions are bubbles, answers carry 复制 and the last one
  重新生成, and an empty reply offers the same retry instead of going quiet.
- The prompt field spans the settings column, is 260 px tall and cannot be dragged smaller.


- Settings are a full dialog with a category rail (general, model, prompt) instead of a card inside
  the AI panel.


- Settings live behind a gear at the far right of the top bar and now include the theme and the
  system prompt; the panel's own settings button and the theme cycle button are gone.


- The system prompt distinguishes the book's content from general knowledge instead of refusing
  anything outside the excerpts: background, comparisons and follow-up questions are answered, with
  page citations reserved for what the book actually says.


- The API key is stored in a `0600` file next to the settings instead of the OS keychain: no prompt on
  launch, and the trade is written down in `SECURITY.md`.
- Markdown now uses `pulldown-cmark`: tables, strikethrough and task lists join headings, lists,
  emphasis and code. Raw HTML stays escaped and citations still become interceptable links.
- The panel keeps the conversation: finished exchanges stay on screen and the last four are sent with
  the next question so pronouns resolve.

### Added

- Drag a range in the page manager; in a spread, the right sheet's bar sits on the right edge.


- Three scope icons (whole document / current page / references) replace the page-range field, plus a
  page-manager dialog with thumbnails centred on the current page and a clear action.


- Page scope: purple bars and three page actions (start / this page / end), neutral pending endpoints
  with a hover preview, additive drag selection with Alt subtract, all undoable.


- Dragging a reference handle re-reads a quote's text or re-crops a region's image; the resize geometry
  is covered by eight core unit tests.


- Reference resizing: quote brackets at both ends, eight dots on a region; all clamped and undoable.


- Reference frames on the page: hover highlights, click pins, ✕ deletes, drag moves; all undoable.


- Reference undo/redo: adding or deleting a reference is one ⌘Z away, with redo on ⌘⇧Z.


- The four-tool rail: pointer plus a sticky text/box/pages capsule, `V/T/B/D` shortcuts, `Esc` back to
  pointer, and modifier borrowing (Shift text, Alt box, Cmd/Ctrl page toggle).


- 双页奇偶偏移: the cover can pair with page 2 instead of standing alone; the choice persists.


- 双页对开: page 1 alone, then facing pairs, each sheet scaled as one unit; single mode now fits the
  whole page through the same mechanism.


- Single-page reading mode, chosen in 阅读 → 翻页方式; the other sheets stay hidden and the current
  page returns to the top.


- Keyboard paths for the answer actions: ⌘⇧C copy, ⌘⇧R regenerate, ⌘⇧G continue, listed in settings.


- 恢复阅读默认 and 恢复默认提示词 in settings, so a section can be put back without hunting values.


- A default zoom preference (75–150 %): newly opened documents start there.


- The window remembers its size, position and maximised state across launches.


- 试试示例 on the welcome card: a bundled three-page PDF, opened through the normal file path.
- A banner under the header when no API key is stored, with a shortcut into settings.


- Reading mode (`⌥⌘F`): the document only, with `Esc` or a chip to return.
- `docs/performance.md`, with the first measurement and its caveats.


- Settings gained 阅读, 快捷键 and 关于 tabs; answer font size and the remember-position switch apply
  immediately and persist.


- **定位** on answers that cite a page: jumps to the first citation in the document.
- The composer grows with its content up to 150 px, then scrolls.


- Page thumbnails in the same rail as the outline, rendered lazily and cached; a document without a
  table of contents can still use them.


- Toasts for completed actions (settings saved, copied), at most three at a time.
- A field-level error under 最大输出, and an offline-specific message when the network is down.


- `⌘F` find in document: match count across all pages, highlights on rendered pages, and `‹` / `›`
  to step through the pages that contain a hit.


- The document outline: a contents button in the header and a left panel listing the PDF's own
  table of contents, indented by depth; choosing an entry jumps to its page.


- A reading progress bar under the document, draggable and keyboard-adjustable, with `page / total`.
- The reading position is remembered per document: reopening a file returns to the page you left.


- Design tokens for spacing, radii, elevation, type and motion; the settings dialog and the shared
  control vocabulary now read from them, and `prefers-reduced-motion` disables motion entirely.


- **继续生成**: an answer cut off mid-stream can be continued from where it stopped, appended to the
  same turn instead of starting a new question.


- A **最大输出** setting: a token ceiling, or empty for no limit. OpenAI-compatible requests omit
  `max_tokens` when unset; Anthropic, which requires it, receives 8192.


- LaTeX maths renders with KaTeX: `$…$` inline and `$$…$$` as a centred block. `latex2mathml` was
  tried first and dropped — it could not parse `\le`, `\ge`, `\mathbb` or `\dots`.


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

- Reference rows delete by id, Escape closes the page manager, and sending no longer leaves an undo
  state that could duplicate the references.


- Page selections now switch the request scope; a drag no longer pins a reference, shortcuts stay out
  of open dialogs, and an empty page scope is reported.


- Sticky box mode no longer resets itself after one capture, and undo during a drag cancels that drag.


- Stale async re-reads can no longer overwrite a newer reference edit; the Shift borrow resets after
  one selection, and Alt is honoured when a page drag is released.


- Reference overlays clear on leave, Escape clears a pending page endpoint, scope changes undo, and
  page gestures ignore the bar/overlay controls; a new document resets history and scope.


- Toasts stay while hovered, and the tool rail is part of the layout so pages stay centred when
  zoomed out.


- On narrow windows the page no longer slides under the tool rail; the stage reserves room for it.


- Transient failures before the first token are retried twice automatically (0.5 s, then 1 s) with a
  visible `正在重试（n/3）` line; a
  stream cut after text arrived is left to 继续生成, because re-requesting would duplicate it.


- A dropped connection mid-answer now reports `连接中断，已保留生成的内容` instead of a bare transport
  error, and keeps the text that already arrived.


- A rejected request can no longer leave the panel waiting forever: failures and two minutes of
  silence both end the turn with a visible message.
- An answer that ends empty no longer erases the question and its reasoning: finished turns are
  archived when the stream ends, and the status line explains that no body text arrived.


- Markdown list items no longer leave a large blank line between the number and its text: a bare
  `1.` line is joined to its content before parsing.
- Answer paragraphs no longer show the blank lines that `pre-wrap` turned into real layout.

- The reasoning block can be collapsed again and never reports `0.0 秒`: the streaming state is a
  plain block, the finished state an uncontrolled `<details>`.
- `准备中` now says whether it is building context or waiting for the first token.
- `Enter` sends; any modifier plus `Enter` inserts a newline.

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
