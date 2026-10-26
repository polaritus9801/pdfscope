# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is `0.x`,
minor releases may contain breaking changes; they are always called out.

## [Unreleased]

### Added

- Pinch startup no longer paints a queued previous-zoom frame; the first gesture sample is eased
  over one frame. Visual placement is folded into the scroll every frame, so the system horizontal
  thumb tracks the gesture and no longer jumps only when the pinch is released.
- Zoom placement is now one continuous function: the page blends from fit centring to the pointer
  anchor with a scale-free smoothstep weight, so crossing fit width no longer changes the page's
  pivot in one frame. Panel-drag geometry remains unchanged.
- The reader's horizontal axis keeps the system scrollbar when the zoomed page overflows sideways;
  only the native vertical track is hidden by the page ruler.
- The navigation rail shows a pointer-transparent marker at the hovered page while the page bar is
  hovered.
- Thumbnail rails and the page-reference manager now fit each page inside a common square bound while
  preserving its own aspect ratio, then wrap the tiles continuously: landscape pages become shorter
  full-width tiles, A4 becomes a narrower full-height tile, and no internal letterbox bands remain.
- A "显示页面标尺" switch hides ruler marks, hover previews and reference bands while keeping the
  ruler thumb as the scrollbar.
- Persistent page-ruler reference bands with rest/hover/focus states, shared by page frames, context
  chips, history rows and `@` pills; context clicks focus refs, preview images/files, navigate pages
  or report unavailable references.
- Ruler preview image/label now straddle the pointer with a centred 16 px gap, and the composer
  placeholder only appears for a structurally empty document.
- Page-range parity: deep clicked-anchor cap, total-page endpoint fallback, visual-base cap/gap
  rules, visible-page metrics for paged/spread layouts and the complete parity audit.
- A committed page action suppresses the same page/kind/side zone until the pointer moves to another
  zone, preventing the button and preview from reappearing under a stationary pointer.
- Page-range action buttons no longer flicker under the pointer: the toolbar is pointer-transparent,
  hit strips own mouse input, and the clicked zone stays suppressed until the pointer changes zone.
- Page-range bars and action toolbars now live in one global overlay with two full-height hit strips,
  keeping the old range state machine while removing per-page action DOM. A release-mode 1000/3000-page
  performance baseline covers load, hover, 500-page preview/commit and heap.
- Page-ruler and reference previews are scoped to the current document and cached; in-flight renders
  from a previous PDF are discarded. Page actions are lazy and only mounted for visible pages.
- Appearance now offers grey and purple page-ruler skins (`pdfscope.scroll-skin`); the ruler remains
  always active and skins only swap thumb variables, never reader geometry. Purple restores the
  accent ramp, round end caps and narrow divider.
- Page-ruler and manager polish: neutral 50% major ticks, stable layering, distinct current/selected
  manager states, and hover previews that do not re-anchor the reader.
- Reader page ruler: canvas page/chapter ticks, current-page line, draggable viewport thumb,
  hover page/section preview, click/drag/wheel/keyboard navigation and an ARIA scrollbar. The
  navigation pane and manager now use the quiet 4 px scrollbar treatment, and manager card drags
  cannot fall through to the file-drop layer.
- Page reference ranges now use one wide continuous bar per committed range (43.2 px, square ends,
  deep-body `#cbb8ec` with `#7f5cbf` end transitions) instead of a thin per-page line. The page
  actions are the supplied SVGs exposed directly at 3/4 bar width, white and shadowed rather than
  circular, and only the button under the pointer is visible.
- Page-range previews use a lighter two-colour pair (`#b49be2` border / `#dccff3` body) and show the
  actual operation: additions preview only newly added pages; fully committed candidate ranges
  preview the post-subtraction split with a low-contrast diagonal ghost, muted dashed outlines and
  cut marks at the removed edges. Ghost/cut marks fade over 140 ms on commit.
- Each page action owns a full-width third of the bar: top starts, middle toggles one page, bottom
  ends, and the icons sit at 1/6, 1/2 and 5/6 of the page. The bar sits 12 px off the sheet and the
  page actions' hit area is the full bar width. Endpoint fades are 64 px instead of 18 px.
- New setting 设置 → 阅读 → 页面引用条位置: left (default), right, both or hidden.
- Adding a page range and removing one share one rule: a candidate interval entirely inside the
  committed set subtracts, otherwise it adds. Invalid start-after-end clicks discard the first click.
- Repeated endpoint clicks can adjust the nearest range (default) or cancel, configurable under
  设置 → 阅读 → 重复点击同一端点.
- Page-range previews, commits, merges and splits each follow the existing one-step snapshot undo;
  Cmd-Z with a pending endpoint cancels the draft before touching history.
- Rebuilt the 管理页面引用 and 目录与缩略图 panels around each page's real aspect ratio. Page
  thumbnails no longer sit in 3:4 letterboxes (the large purple blocks on landscape decks), keep
  their page-number badge after lazy rendering, render at 200 px, and mark selected/current pages.
- The page manager is wider with 160 px cards, a keyboard-focusable selection ring/check, current-page
  outline, interaction hint, selected count and a disabled clear action when empty; it centres the
  current page on open. The navigation rail is anchored to the reader's top-right, shows the page
  count, centres the current thumbnail, and gives directory rows hierarchy, target page numbers and
  current-section highlighting.
- Outline destinations that use PDF page references now resolve through `getPageIndex`;
  previously they became page 0, leaving the whole TOC disabled with blank page numbers.
- The composer resize handle is now a square-ended border segment shown only while the handle itself
  is hovered/focused/dragged; its colour follows the input border state.

### Fixed

- Locked shell scrolling with `overflow: hidden` fallbacks plus `overflow: clip` on `html`/`body` and `.app`/`.panel`, plus
  composer-focus and mode-toggle anchoring. Even if another element overflows, focusing or toggling
  the composer can no longer scroll a shell container and put the history/new-session buttons out of
  reach. Removed the heavier viewport-fixed body and global scroll listener; they were unnecessary
  once the shell containers are clipped and added mobile/print and scroll-jank risk.

- Removed the mode-height floor that kept the composer at the taller preview height: it left a blank
  white strip under the shorter plain-text layout, most obvious with stacked display formulas.
- Capped automatic composer growth at 30% of the AI panel height. Long content scrolls inside the
  input instead of hiding the answer; a manual top-edge drag is a floor that can raise it above the
  cap. Restored drafts scroll the caret into view.
- PDF.js's hidden measurement canvas no longer contributes a 19px line box to `body`. Focusing the
  composer after toggling live preview could scroll `body` by that amount, shifting the whole app and
  exposing the bottom bar/white gap. The canvas is now out of flow.

### Added

- A tiny top-right dot toggles Markdown live preview. Raw Markdown remains the document; whole-line
  syntax reveals on the active line, inline delimiters hide elsewhere, the preference is persisted,
  and the composer supports headings, quotes, hanging-indent lists, fenced code, links and inline
  emphasis.
- The dot is now a high-contrast ring when off, light-filled on hover and deep-filled when on.
  The composer follows its natural content height; the page bottom stays fixed.
- Nested lists render with a per-level hanging indent and Tab/Shift+Tab indent source lines; Tab
  outside a list keeps normal focus navigation. Vertical resize uses an explicit vertical cursor.
- Structured arrow navigation crosses paragraph boundaries at the block edge, so hidden syntax can
  never make Left/Right jump mid-word.
- The composer's top edge can be dragged to set a manual height floor. Automatic growth is capped at
  30% of the AI panel height; taller content scrolls inside the input. A manual drag can deliberately
  exceed that cap. Double-click restores the automatic height.
- Live KaTeX formulas: inline `$...$` / `\(...\)` and single-paragraph `$$...$$` / `\[...\]`.
  Raw LaTeX stays editable; inactive lines render the formula and clicking it restores the source.
  This reuses the existing KaTeX assets and adds no package.

### Changed

- Replaced the hand-written contenteditable composer with a ProseMirror editor whose `@` pills are
  inline atom nodes. The public Composer API and `@{key}` serialisation are unchanged; source lives
  in `ui/composer-src` and builds to `ui/public/composer.js`.


- The context budget trims page context to fit before refusing; only oversized references/attachments
  stop a request.


- 导出 writes a Markdown file into Downloads (collision-safe, path in the toast) instead of only
  copying to the clipboard; browsers fall back to copying.


- Web search offers DeepSeek Search (default, reusing the model key), Tavily, Brave, Exa and Serper,
  with a provider picker and per-provider keys.


- The AI panel is a flush column: its left border is the divider and the drag affordance, resizing
  keeps the document's centre in place, and the three scope icons are a refined segmented control
  with a reference count badge.


- The progress line now covers the header's bottom border, so it is flush at the edge, and the header
  paints above the page so the dot is no longer cut in half.


- The rail keeps its circle-plus-capsule shape while floating; the progress hairline is flush with the
  header edge, thicker on hover, with a plain purple dot.


- The four tool buttons float over the stage without a capsule behind them, and the page slider moved
  from the bottom strip to a hairline on the header's bottom edge with a hover-only handle.


- The 120-second request timeout and the bridge's silence watchdog are one setting
  (接口 → 响应超时), default unlimited.


- Whole-document questions now carry the whole document; the page cap moved to 设置 → 阅读
  (每次最多发送页数, 全部 by default).


- Split the panel: the page manager and the settings sheet are their own modules, and the architecture
  doc lists what each interface module owns.


- Split the interface into `model.rs` and `page.rs`: the data model and the page shell are no longer
  inside the 1900-line `main.rs`.


- Removed fourteen dead style blocks from the stylesheet and styled the icon-only header button.


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

- Per-question reference capsules now open a full reference popover. Hovering a passage/region/page
  highlights it without scrolling; offscreen pages show a thumbnail. Clicking adds the reference to
  the input context through the same path as active chips, and keeps the popover open.
- Question hover shows its ask time above the capsule; completed answers end with a completion time.
  A copy-question action sits beside edit. Times use today HH:MM, this year MM-DD, older YYYY-MM-DD.
- Pasting or dropping images/files on the AI panel attaches them to the composer; dropping a PDF on
  the reader opens it. Tauri native drag interception is disabled so the webview receives File objects.
- The OOBE preview `+N` tile now matches the last page thumbnail's height.

### Changed

- Settings → 阅读 now chooses whether a document resumes with the newest pinned conversation (default)
  or the newest conversation overall; both groups are newest-first.
- The reference popover extends from its capsule, flips/clamps at the AI panel edge, and stays
  within the panel.
- Older timestamps include the time (`MM-DD HH:MM`, `YYYY-MM-DD HH:MM`).
- Toasts are content-width and centered; a short copy confirmation no longer matches a longer one.
- New conversations focus the composer automatically.
- Duplicate reference activations add nothing and show a red “已经在上下文里了” notice.
- Dragging files dims/blurs only the target region with a large white upload glyph and the caption
  “打开文件” or “上传文件”.
- All three scope modes carry the active references, including page references. The modes only choose
  extra page bulk (all pages / current page / no extras).
- Opening a document resumes its newest conversation; pinning only affects drawer ordering.

### Removed

- The incomplete "历史引用" list and its `Stored`/`redeem` bookkeeping.

### Changed

- Scope tooltips read 整份文档和引用 / 仅当前页和引用 / 仅引用.
- OOBE is 920×720, keeps the 330px guide, and shows ten recent + ten recommended rows with two
  independently scrolling 50% halves. Recommended files are deduplicated by path and name+size.
- The @ picker floats directly above the input card and closes when the trigger becomes invalid.
- Context references and inline @ pills share hover highlight and click-to-locate; the shelf chip
  gets a hover/selected outline. Historical reference popovers gain “全部” to add all missing refs.
- Inline @ pills no longer leave a trailing space and keep the caret outside the pill; file/image
  pills are included in the inline citation numbering.

### Fixed

- Use one-pixel editable caret boundaries outside @ pills (not zero-width characters); Backspace/
  Delete removes the pill instead of entering it.
- The warning badge is 18px wide, 2pt further right and bottom-aligned; the send button hover no
  longer lifts so the badge stays put. The warning dialog gains a larger X and equal-width 仍然发送 /
  修改 buttons.

- The send warning uses the supplied SVG with a white exclamation, 85% of the old size and right/
  bottom aligned to the send button.
- The open reference capsule is #7f5cbf with white icon/count; layout is unchanged.
- The rail's page icon has a little more breathing room between the rounded page and the L corner.

- Removing a context reference now clears its PDF hover immediately and leaves its inline @ pill in
  place as a red orphan. A floating send warning and a centered “是 / 修改” dialog handle sending.
- The 330px capsule's hover is colour-only (no lift).
- @ after existing text opens the picker; the picker is 300px, sits above the caret and stays inside
  the AI panel; its rows share the historical hover/preview path.
- OOBE rows are 20 per column and metadata is pinned to the right edge.

- The `@` picker floats above the composer instead of pushing the context shelf up.
- Choosing an `@` reference now inserts a visible pill (the token builder no longer calls itself).
- Deleting an inline `@` pill no longer deletes the reference from the context shelf; removing the
  shelf chip removes both.
- Answer completion time is 11px instead of 10px.

- Thinking now collapses as soon as the first answer tokens arrive. The auto-collapse added in
  0adfe99 was checked only inside flush_pending, which the coalesced text path (718c71f) bypasses;
  it therefore ran only at done/stop and the pill stayed open during the answer.
- The first answer no longer flashes per token: the empty-state branch no longer reads the streaming
  answer, so tokens cannot rebuild the transcript. Markdown and `[p.N]` citations still render
  incrementally; copy is hidden until completion/stop.
- Each question records its full reference context in the conversation tree. A bottom-aligned
  paperclip/count capsule sits to the left of the question bubble, and edit/resend restores the
  original references.

### Added

- The empty reader is a floating OOBE overlay above the whole app (dimmed blurred backdrop, centred
  660px / 50-50 panel, internal scroll and a narrow-screen stack), with an inset vertical rule, a
  compact one-line library split into “最近打开” and “推荐打开” (7 rows each), and a delayed hover
  preview of the PDF's pages in a one-row bubble. Both sections show five compact rows with the
  supplied PDF type mark; recents keep bytes and paths in IndexedDB, while the desktop shell scans
  Downloads, Desktop, Documents and the five recent directories for direct PDF children, newest
  first. Native file picking/opening, missing-file handling and a browser fallback are included.

### Added

- The AI panel keeps the paragraph at a 35% reading line (shared with the PDF stage) while the panel
  is resized, and a FOLLOW-END state: sending arms it, transcript growth follows the end, and an upward scroll detaches
  until the reader returns to the bottom. No per-token IPC; MutationObserver/ResizeObserver follow
  synchronously, and a panel drag keeps one scroll mode from pointer-down to pointer-up.

### Fixed

- Dragging the AI panel now keeps a fixed page/fraction reading-line anchor through the drag and the
  final base reflow; releasing the pointer no longer jumps, flashes a different scale, or drifts
  vertically across repeated drags (including slow 1 px/frame drags). The text layer is clamped to
  its page layer so an old base-width box cannot flash the horizontal scrollbar. The final drag frame is painted before the base-layout commit, page canvases
  fill their page layer, and page gaps follow the page width instead of snapping back to a fixed
  value.

### Added

- The page-scope segment is back to its pre-tile appearance (pale lilac trough, white active tile) and uses the newly redrawn PDF/page/reference artwork; the icons keep native proportions.


### Added

- The thinking header shows a live timer and a collapsed thinking pill is compact; collapsed live and
  completed states match.
- Every answer can regenerate; each user bubble carries a chrome-free branch switch and an icon-only
  edit, with the caret at the end.
- History gains a copy icon and a trash delete; the branch-to-new-chat and copy-with-citations actions
  are removed. The attachment control uses the supplied SVG, the three tool icons are optically
  matched and share a 1.05 pt rendered wire, the model name fills its row before the branch controls,
  and panel scrollbars are 4 px.
- Editing/regenerating creates a sibling in a persisted conversation tree. Each user bubble has an
  icon-only edit and, when needed, a `< n / total >` switch for that node's alternatives; old branches
  remain reachable. Duplicated answer durations are gone.


- A cream accent for the model's reasoning surface and for a mode borrowed with a modifier; the
  pointer tool remains lilac. The reasoning header row stays white and a rule separates it from the
  cream body, in both live and completed states.


- The six-pixel purple band was the page-position rail; it is now a 4 px divider that darkens to
  `#7F5CBF` on hover and stays dark for the whole drag through `.resizing`, then returns to native
  `:hover`. It has an invisible 12 px drag hit area and is disabled while collapsed. The stage
  scrollbar is back, and the composer is clipped with `clip-path`/`contain: paint` with no outer halo,
  targeting the caret-repaint white tips. The token/status row is gone; empty conversations are never
  written to history.


- Search-provider cards with marks and a configured badge, a provider mark in the model popover, and
  an icon-bearing 厂商 menu in settings.


- Skeleton, structured error, focus rings and reduced-motion-aware transitions in the AI panel.


- The context shelf gains a count header, clear-all, type icons and collapsed page ranges.


- Message identity rails, question bubbles, reasoning pills, icon-only answer actions and a designed
  empty state in the AI panel.


- A unified icon family and provider marks; the panel header now carries provider identity, a custom
  model popover and icon actions, and the composer is one card with a toolbar and circular send.


- Crash recovery: a half-streamed answer is restored as an interrupted turn after a restart, ready for
  继续生成.


- Per-conversation model switching from a settings-defined model list, carried into the request as an
  override.


- `[来源 n]` markers inside a web answer are clickable inline citations, in addition to the source
  cards below it.


- Per-conversation drafts that restore text and pills after switching or restarting, and duplicate
  passage/region prevention.


- A send-time context budget guard (configurable, 128k default) and ↑/↓ recall of previous questions.


- Attach PDF, DOCX, PPTX, XLSX, text and code files; the renderer extracts their text (PDF.js and a
  dependency-free ZIP/XML reader) and folds it into the request with size and length limits.


- Each answer keeps its own web sources and shows numbered citation cards; the live 来源 shelf row is
  only for the turn in flight.


- Web search through Tavily: a search key in settings, a globe toggle in the composer, numbered
  results appended to the request and a 来源 shelf row that opens citations.


- Attach text files to a question; they appear in the shelf, are counted in the budget and are sent
  as labelled, character-limited blocks.


- Temporary conversations that are never persisted, and 分支 to continue from any answer in a new
  stored conversation.


- Attach images to a question from the composer; they show as thumbnails, become inline pills, count
  toward the token hint, and are labelled as attachments by the request builder.


- Export the current conversation as Markdown, a rough token estimate beside the composer, and
  复制引用 (answer plus its cited pages).


- A conversation library per document: search, rename, pin, delete, new conversation, with the old
  single transcript migrated into it.


- Conversations persist per document across restarts, with a 新对话 action to start clean.


- The @ picker is keyboard-driven (↑/↓/Enter/Esc), and any sent question can be edited and resent,
  replacing that turn and the ones after it.


- Chips drag into the question field as inline pills, and 引用此回答 can quote just the selection
  inside an answer.


- Answers can be quoted as context (`引用此回答`), and references now sit in a horizontally scrolling
  context shelf with one row per kind.


- The question field is a contenteditable editor with inline attachment pills: type `@` to insert a
  passage, region or page, delete a pill to remove its attachment (undoable), and send with Enter.


- Navigation and zoom buttons have labels, and the spec carries a 19-step manual acceptance list.


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

- Tool shortcuts no longer fire while typing in the composer; held modifiers now call the real
  `setTool`, mirror the borrowed rail icon and clear reference handles; the vendor menu commits and
  closes on the first pointer.
- During a page drag only the page under the pointer is washed: the hover wash is removed, a
  `.page-gesture` state disables the opacity transition, and the pointer page is refreshed during
  auto-scroll and at gesture end.
- The composer's inner focus rectangle is gone, its card border is stronger, the divider no longer
  paints a purple bar, and the collapsed rail is one full-height icon-only button.
- Official SVG marks are downloaded for every provider; model rows and settings chips carry them.
- Panel drag scales the document continuously instead of snapping at release.

Spec: docs/specs/0025-panel-interaction-fixes.md


- Opening a PDF could pin the webview at 100% CPU. The document-load effect read signals it also
  wrote (`next_conversation`, `known_mentions`) and re-subscribed itself; those reads are untracked,
  draft/attachment reads are untracked, and no-op removals no longer notify.


- A failed web search no longer consumes references or clears the draft; rename blur cannot reset a
  title; conversation actions are refused while streaming.


- Selections report again: the rect-merging helper no longer drops `right`/`bottom`.


- Overlapping Range boxes are merged into one box per line, and native selection paint is muted while
  the old selection is cleared, so no flatter second layer can survive under a reference.


- Grip dragging now has a single source of truth: the interface state drives marks, frame and grips
  reactively, so no second highlight layer can appear and the grips stay on the selection.


- Text selections no longer stack stale highlight boxes after an edit, and the endpoint grips sit on
  the line's centre.


- Text grips now edit character ranges across lines and follow the caret; page-scroll previews stay
  anchored to content; double-click no longer opens a file while one is open.


- Page gestures scroll at the stage edge and turn light grey when subtracting; passages no longer
  move as boxes and their endpoint grips drag again.


- Text selection reports again; unfocused references really let the pointer through; page mode has a
  hover wash and subtracts a batch when the gesture starts on a selected page, with white feedback.


- Passages highlight the exact lines selected instead of whole text-layer spans; endpoints are
  character-accurate, the ✕ sits outside the frame, references are focus-to-edit outside the pointer
  tool, and page drags show a live preview box with per-page outlines.


- 仅引用 now sends the pages marked with the purple bars as well as the quote/region references; the
  hidden fourth scope state is gone.


- LaTeX formulas are no longer mangled: math is protected from Markdown before parsing, so `_`, `*`,
  backslashes and `[p.n]` inside a formula survive and KaTeX renders it.


- Box capture arms again, selections no longer break mid-drag, passages highlight the actual text with
  `[`/`]` at its start and end, page buttons are reachable from outside the page, and scope outlines
  are visible.


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
