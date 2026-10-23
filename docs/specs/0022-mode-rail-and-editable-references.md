# 0022 — A mode rail, editable references and page scope

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-22
- **Approved by:** z1mLi (2026-10-22, with the middle-range model delegated)

## Goal

Four tools replace three: a pointer that returns to normal reading, and one sticky capsule for text,
box and page modes. References can be adjusted on the page — quotes by their ends, regions by corners
and edges — and every reference change, including which pages are in scope, is undoable. The manual
"页码范围" field disappears: three icons choose what the model sees, and a manager dialog edits the
page set visually.

## Non-goals

- Writing references into the PDF; they live for the session.
- Undo for chat, settings or theme.
- Multi-select of references, copying references between documents.
- Annotations with comments or colours.

## Behaviour

### The rail

Four controls. The first is a separate hollow circle; the other three share one capsule whose top and
bottom radii match that circle.

1. **Pointer** — normal reading and selection. Clicking it leaves any sticky mode. Escape does the same.
   The floating "add reference" bar appears only here.
2. **Text** — sticky. Any text swipe immediately becomes a passage reference.
3. **Box** — sticky. A marquee becomes a region reference on release.
4. **Pages** — sticky page-scope editing (below).

Shortcuts: `V` `T` `B` `D` switch tools, but **never while a text field has focus**. Holding a
modifier temporarily borrows a mode for one gesture, then returns to the current tool:

| Hold (macOS) | Hold (Windows/Linux) | While held |
|---|---|---|
| `⌘` / `⌃` | `Ctrl` | drag across pages extends the page set |
| `⇧` | `Shift` | text swipe adds a passage |
| `⌥` | `Alt` | drag draws a marquee |

### Page scope

Three single-choice icons replace the range field, at the left of the composer row:

| Icon | Name | What the model receives |
|---|---|---|
| ▤ | 整份文档 | text of every page (bounded by the existing page cap; the cap is reported, not silent) |
| ▣ | 仅当前页 | text of the page in view |
| ❝ | 仅引用 | only the passages and regions the reader added; no page text |

To their right: **管理页面引用**, opening the page manager.

### Page marks and the per-page bar

A page in scope gets:

- a thin outline around the sheet;
- a purple bar on its outer edge — left edge in single mode, the outer edge in spread (left page:
  left, right page: right);
- three icon buttons on that bar, at the top, middle and bottom: **从这里开始**, **单独这一页**,
  **到这里结束**. They appear only when the pointer is near the bar.

Toggle logic, stated as a state machine:

**The problem with the first draft.** With pages 1–3 and 9–11 already chosen, clicking *start* on 6
previewed 6–end and clicking *end* on 7 previewed 1–7; either gesture looked like it was destroying the
existing selection. The endpoints were doing too much.

**Revised model — endpoints are neutral, preview follows the pointer.**

- Clicking **从这里开始 (p)** places a dashed endpoint marker on `p`. It changes **nothing** about the
  committed set and draws **no range**.
- While an endpoint exists, hovering page `q` shows a **faint dashed preview** of the range between
  `p` and `q`; leaving the page removes the preview. The committed bars never dim.
- Clicking **到这里结束 (q)** commits `min(p,q) .. max(p,q)`, merged into the set, and clears the
  marker. Clicking **从这里开始** again moves the marker; clicking it on its own page clears it.
- **到这里结束** works symmetrically: with no marker it becomes the pending endpoint, and a following
  start closes the range. Both buttons are the same operation with different anchor labels.
- **单独这一页 (p)** toggles one page and clears any pending marker.
- `Escape` clears the marker. Committed ranges merge as a **set**, so overlap and nesting collapse —
  nested or duplicate ranges cannot exist.

**Dragging is the fast path, and it adds.** Dragging across pages 6 → 7 adds exactly 6–7 to the
existing set; it never replaces it. `⌥`(Alt) during the drag subtracts; `⌘`(Ctrl) forces add (the
default, but explicit). Replace is not a drag gesture at all — 清空页面引用 is the only way to empty
the set, so a stray drag can never erase a carefully built range.

A drag commits only if it crosses at least one page boundary. Clicking a page in page mode toggles it.

### Editing a reference

Hover a reference: the ✕ appears at the top-right, its top edge level with the reference's top edge.
`[`/`]` for quotes, eight handles plus a move handle for regions.

**Hover and focus, organically combined.** A pure hover overlay flickers when the pointer leaves the
selection to reach a button that sits outside it. The model is:

- **Hover reveals** the overlay when the pointer is over the reference, the overlay itself, or within a
  12 px bridge between them. The bridge is what makes the buttons reachable.
- **Click pins** the overlay (focus). While pinned it ignores hover-out and stays until `Escape`, a
  click elsewhere, or a completed edit.
- **While dragging** a handle, pinning is irrelevant: the overlay is locked, hover changes nothing, and
  the drag captures the pointer.
- Pinned references draw a 1 px accent outline so the reader knows why the handles persist.

### Undo

`⌘Z` / `⌘⇧Z` (Ctrl / Ctrl+Shift elsewhere) walk one stack per document. Its entries: adding a
reference, deleting one, moving, resizing, re-anchoring a quote, and **changing the page set** (toggle
page, commit range, scope icon change, manager dialog edits including 清空). The stack is cleared when
another document opens. Deleting from the composer uses the same operation as the ✕ overlay.

## Data model

```rust
struct Passage { id: ReferenceId, page: u32, quote: String, rect: Rect }
struct Region  { id: ReferenceId, page: u32, image: String, rect: Rect }

enum PageScope { Current, WholeDocument, ReferencesOnly, Pages(BTreeSet<u32>) }

enum ReferenceEdit {
    Add(ReferenceSnapshot), Remove(ReferenceSnapshot),
    Geometry { id: ReferenceId, before: Rect, after: Rect },
    Anchor { id: ReferenceId, before: AnchorState, after: AnchorState },
    Scope { before: PageScope, after: PageScope },
}
```

The history is `Vec<ReferenceEdit>` plus a cursor; a new edit truncates the redo tail. `PageScope::Pages`
is a set, so range algebra removes duplicates by construction. The page cap for `WholeDocument` stays
20 pages; the response says how many pages were sent.

## Interface

```
PDFScope.setTool(tool)                  // pointer | text | box | pages
PDFScope.setScope(mode)                 // current | document | references
PDFScope.pageSet()                      // committed pages
window.__pdfscopeEdit(edit)             // renderer → Rust: geometry / anchor / page set
window.__pdfscopeToolChanged(tool)      // renderer → Rust
```

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Pointer | cursor | rail circle | always | click | `V`, `Esc` | tap | leaves sticky mode | — |
| Text | 文 | capsule top | document open | click | `T` | tap | sticky text capture | `Esc` |
| Box | 框 | capsule middle | document open | click | `B` | tap | sticky marquee | `Esc` |
| Pages | 页 | capsule bottom | document open | click | `D` | tap | sticky page toggling | `Esc` |
| Start here | ⇤ | page bar, top | pointer near bar | click | — | tap | pending start, preview to end | `Esc` cancels |
| This page | ▣ | page bar, middle | pointer near bar | click | — | tap | toggles one page | undo |
| End here | ⇥ | page bar, bottom | pointer near bar | click | — | tap | closes the pending range | undo |
| Scope: current | ▣ | composer, left | document open | click | — | tap | model gets the visible page | — |
| Scope: whole | ▤ | composer, left | document open | click | — | tap | model gets every page (capped) | cap reported |
| Scope: references | ❝ | composer, left | document open | click | — | tap | model gets only references | — |
| Manage pages | 管理页面引用 | composer, after icons | document open | click | — | tap | opens the page manager | `Esc` closes |
| Manager page | thumbnail | dialog grid | always | click toggles; drag toggles a range | — | tap | set changes live | `⌘` adds, `⌥` subtracts |
| Clear pages | 清空页面引用 | manager footer | set not empty | click | — | tap | empties the set | undo |
| Delete | ✕ | reference top-right | hover or pinned | click | `Delete` | tap | removes it | undo |
| Quote handles | `[` `]` | quote start/end | hover or pinned | drag | — | drag | re-anchors | < 8 px no-op |
| Region handles | 8 dots | corners/edges | hover or pinned | drag | — | drag | resizes | clamps |
| Move | ✥ | below ✕ | hover or pinned | drag | arrows when pinned | drag | moves | clamps |

**Alternatives considered.**

- Chosen: sticky modes, per-page bars, a page manager dialog.
- Rejected: typing page ranges — the human's stated reason: unintuitive.
- Rejected: hover-only overlay — flickers when reaching buttons outside the selection; a short bridge
  plus click-to-pin solves both reach and calmness.
- Rejected: one cycling tool button — no visible state.
- Rejected: per-page checkboxes in a sidebar — the manager dialog gives thumbnails and space, and the
  bars handle the common case in place.

**Discoverability.** The circle is visually separate from the capsule; tooltips name tools and
shortcuts; hovering a page near the edge reveals the bar; the scope icons carry tooltips; the manager
is reachable from the composer where the page question is decided.

**States.**

| Tool | Text drag | Marquee | Page click | Floating bar |
|---|---|---|---|---|
| pointer | selects, bar shows | nothing | nothing | shown |
| text | adds immediately | nothing | nothing | hidden |
| box | selects | adds on release | nothing | hidden |
| pages | selects | nothing | toggles page | hidden |

**Cost of being wrong.** Every change is one `⌘Z` away; a misfired drag under 8 px does nothing; page
ranges merge by set semantics so a wrong range can be corrected page by page.

## Test plan

- Unit: range algebra (start/end, reversed, overlapping, nested, cancellation), undo/redo/truncate,
  clamping, the 8 px threshold, scope serialization.
- Integration (once a browser can be used again): each tool sticky; modifier borrow; hover bridge
  reaches ✕; click pins; drag locks; text re-anchor updates quote; region resize/move; manager drag
  select with ⌘/⌥; ⌘Z across all edit kinds; scope icons change what the request contains.
- Manual: purple bar placement in single and spread; pointer returns from every mode.

## Risks

- **Re-anchoring text** needs range APIs over the PDF.js text layer and must survive rotation; the
  hardest part of this spec.
- **Two writers on one list.** The edit enum must be the only writer of references and scope, or the
  undo stack desynchronises with the composer.
- **Hover overlays** near page edges can fight capture; overlays stop propagation and clamp to the page.
- **Modifier borrowing** can surprise: a modifier held while scrolling must not switch tools. Only a
  drag or a text swipe borrows, and the borrow ends on pointerup.

## Assumptions

- References and page scope remain session-only.
- The page cap is a setting, not a constant: 每次最多发送页数 is 全部 by default; 20 / 50 / 100 / 200
  are offered for readers who want to spend fewer tokens. Truncation is reported, never silent.
- Page-mode drag replaces unless a modifier says otherwise.

## Open questions

All answered on 2026-10-22: the modifier map above, reference-related undo including page scope, the
spread bar on the outer edge, and the middle-range redesign delegated to this document. The drag
gesture changed from *replace* to *add* as part of that redesign, because replacing made the
1–3 / 9–11 → 6–7 case impossible without clearing first.

## Progress

- **A. Rail and tools — first cut landed.** Four controls: a circular pointer and a capsule of text,
  box and pages. `V/T/B/D` switch tools and are ignored while a text field has focus; `Escape` returns
  to pointer. Box mode is sticky (re-arms after each capture); text mode auto-adds a passage instead of
  showing the bar, and holding Shift does the same for one gesture. `⌘`/`Ctrl` clicks a page to toggle
  it, `⌥`/`Alt` borrows a marquee for one gesture. The page set is collected but not yet drawn.
- **B. References — first layer landed.** References carry stable ids; a snapshot stack records every
  add/delete and `⌘Z` / `⌘⇧Z` restores it (fifty steps, cleared on a new edit). Deleting from the
  composer is the same operation and is undoable.
- **B. Overlay landed.** Every reference draws a frame with a transparent 12 px bridge; hovering it
  marks it active, clicking pins it (a 1 px accent outline shows why it persists). The white circular
  ✕ sits at the top-right with its top edge on the reference's top edge and deletes (undoable); regions
  also show a move handle. Dragging the frame moves the reference, clamped to the page, and the first
  6 px of movement records one undo entry.
- **B. Resizing landed.** A quote shows `[` and `]` on its ends; a region shows eight dots (corners and
  edges). Dragging either resizes in that direction, clamped to the page with a 1 % minimum; the first
  6 px records one undo entry, shared with moving.
- **Text and images follow their geometry.** On drop, a quote is re-read from the text layer
  (`PDFScope.textInRect`) and a region is re-cropped (`PDFScope.regionImage`), both undoable through
  the same snapshot. The resize maths lives in `pdfscope-core::geo::resize_rect` with **eight unit
  tests** (each edge, each corner, clamping, minimum size, unknown mode), so the geometry is verified
  without a browser.

## Progress — page scope

- **C. Page set, bars and endpoints landed.** Selected pages get an accent outline and a purple outer
  bar; hovering a page reveals three actions — start, this page, end. Endpoints are neutral: start
  places a marker, hovering any page previews the dashed span, and end commits the range. Dragging
  across pages adds the range (Alt subtracts); a click toggles one page. The page set joined the undo
  snapshot, so page edits undo with ⌘Z.
- The bar sits on the **left** in every mode for now; spread places the right page's bar on the right
  in the design and that is not built.
- **Scope icons and the manager.** The range field is gone: three icons choose 整份文档 / 仅当前页 /
  仅引用, and the request now selects pages from that mode (whole document and explicit pages are both
  limited only by the configured 每次最多发送页数 (全部 by default), with a notice when it truncates). 管理页面引用 opens a dialog of
  thumbnails that scrolls the current page to the middle via `scrollIntoView({ block: "center" })`;
  clicking toggles a page (Alt subtracts), 清空 empties the set, and every change is undoable.
  Dragging from one thumbnail to another selects the range; Alt subtracts it.
- **Spread bars.** In a spread the right-hand sheet carries its bar and three actions on its right
  edge, away from the spine, via a `data-spread` attribute the layout sets.
- **A, B and C are implemented.** What is missing is human visual verification (no browser was used),
  and the deviations below.

## Review pass

Four defects found by reading the code rather than looking at it:

- A reference frame never cleared its hover highlight on leave, so the overlay would have stayed on
  screen. It now clears unless pinned.
- Escape reset the tool but not a pending page endpoint; it now clears both.
- Scope-icon changes were outside the undo snapshot; the snapshot carries the scope now, so ⌘Z puts
  the icon back.
- The page gesture fired on the page-bar buttons and on reference frames; it now ignores both. A new
  document also clears the history, the ids and the scope, so nothing from an old file can be undone
  into a new one.

### Second review pass

- The Shift borrow lasted until the next pointerdown, so a shift-drag followed by a keyboard selection
  could add a reference nobody asked for. It now resets when the selection payload is sent.
- Alt is read when a page drag *ends*, so pressing it mid-drag subtracts correctly.
- A text or image re-read is now guarded by an edit epoch: a slow `textInRect` answering after a newer
  drag can no longer overwrite the quote with stale text.

### Third review pass

- **The box tool silently dropped itself.** Every capture disarmed and re-armed, and the disarm was
  reported to Rust, which reset the tool to pointer — so after one box the rail said pointer while
  capture was armed. Sticky box now stays armed without a disarm/re-arm round trip.
- **Undo during a drag.** A drag recorded the edit epoch it began in; an undo or redo bumps the epoch,
  and a drag from an older epoch stops instead of writing over the restored state.

### Fourth review pass

- **Page edits never switched the scope.** The purple bar and page drag changed the page set, but the
  request kept whatever icon was active, so the selection was invisible to the model. Both now switch
  the scope to pages, and clearing the set falls back to the current page.
- A drag on a reference ended with a click, which toggled pinning; a move now suppresses that click.
- `V/T/B/D` fired while the settings dialog was open and treated `<select>` as not-typing; both fixed.
- Sending with the pages scope and an empty set now says so instead of silently sending nothing.

### Fifth review pass

- Reference rows in the composer deleted by their render index. After an undo changed the list, an
  index could point at the wrong reference; rows delete by id now.
- Sending moves references into the turn's history; the undo stack still held a state where they were
  active, so ⌘Z would have duplicated them. Sending now clears the reference history, recorded as the
  deviation below.
- Escape did not close the page manager; it does now.

## Verification without a browser

- `pdfscope-core::history::History` — 6 tests: undo returns the previous state, redo returns the
  undone one, a new edit truncates the redo branch, the limit drops the oldest state, undo and redo
  walk the same states, and an empty history reports nothing to do. The UI stores one `History` of
  snapshots instead of two vectors.


- `pdfscope-core::pages::request_pages` — 8 tests for what a question actually carries: whole document
  capped and reported, current page clamped, references-only empty, explicit set sorted/deduplicated/
  filtered, a large set capped, an empty document, and the stable byte mapping. The composer calls it;
  the notice ("names N pages, took the first 20") comes from the same numbers.


The two pieces of logic that a screenshot could not check are unit-tested:

- `pdfscope-core::geo::resize_rect` — 8 tests: each edge, each corner, clamping, minimum size,
  unknown mode.
- `pdfscope-core::pages` — 9 tests: toggle, merging, reversed ranges, nesting and duplicates,
  subtraction that splits a set, zero clamping, sorted-and-unique invariants.

Both are the functions the interface actually calls; the UI holds no second copy.

## Manual acceptance

Run these in order; each says what should happen. Anything else is a bug.

**Rail**
1. `V`, `T`, `B`, `D` switch tools; the capsule shows the active one. Typing in the question box does
   not switch tools.
2. Box mode survives several captures in a row; the rail still shows box, not pointer.
3. `Esc` returns to the pointer, and clears a pending page endpoint.
4. Hold `⇧` and swipe text with the pointer tool: the quote is added without the floating bar.
5. Hold `⌥` and drag a box with the pointer tool: the region is added.
6. Hold `⌘` (Ctrl) and click a page: it joins the page set.

**References**
7. Hover a reference: outline, ✕ at the top-right with its top on the reference's top edge. Move from
   the reference to the ✕ without leaving the highlight (the 12 px bridge).
8. Click the reference: it pins (1 px accent outline) and stays while the pointer leaves.
9. Drag `[`/`]`: the quote resizes; on release its text re-reads from the page.
10. Drag a region corner or edge: it resizes; on release the thumbnail re-crops.
11. Drag the frame body: it moves; a drag under 6 px changes nothing.
12. `⌘Z` undoes add, delete, move, resize and the text/image re-read; `⌘⇧Z` redoes.

**Page scope**
13. Hover a page: three buttons appear on the left bar (start / this page / end).
14. Click start on page 3, hover page 5: a dashed preview covers 3–5; click end on 5: the range is
    committed and the dashed preview disappears.
15. With pages 1–3 and 9–11 selected, click start on 6 and end on 7: the result is 1–3, 6–7, 9–11.
16. Drag from page 6 to 7 with Alt held: 6–7 are removed; without Alt they are added.
17. The scope row shows 整份文档 / 仅当前页 / 仅引用; the third sends no page text.
18. 管理页面引用 opens the manager, current page centred; clicking a thumbnail toggles, dragging
    selects a range, Alt subtracts, 清空 empties, `Esc` closes.
19. In a spread, the right page's bar and buttons sit on its right edge.

## Cleanup

Fourteen style blocks that nothing referenced (the old page-range row, tray chips, the old sheet and
thinking styles, the previous reference tint) were deleted, and an audit for classes used without a
rule found the icon-only header button, which now has one.

## Field report fixes

The first real session found four regressions:

- **Box capture was dead.** `beginCapture` existed only as a method, while the tool switch and the Alt
  borrow called a bare `beginCapture()` — a runtime error, so nothing armed. The body moved into a
  module-level `armCapture()` that both call.
- **Selections broke mid-drag.** The settle timer reported (and, in text mode, added and cleared) a
  selection after 120 ms of quiet, even with the pointer still down — slow drags were cut short, fast
  ones lost their tail. Reporting now waits for pointerup.
- **References were rectangles.** The rectangle frame replaces the old tint, so a passage lost its
  highlight. Passages are now painted on the **text spans** under their rectangle
  (`highlightPassages`, re-applied after every page render); the frame carries no fill and no outline,
  and the `[` / `]` handles sit at the top-left and bottom-right of the text flow, where a selection
  starts and ends.
- **Page buttons were unreachable.** They only appeared on page hover, so moving toward them left the
  page and hid them. They are now always hit-testable, fade in when the page **or the buttons
  themselves** are hovered, and the scope outline is 2 px solid instead of a faint hairline.

## Structure

`ui/src/main.rs` had grown to 1900 lines with a single 1250-line `App`. The first split, behaviour
unchanged:

- `model.rs` (123) — `Passage`, `Region`, `Selection`, `PageSize`, the `window.__pdfscope*` payload
  parsers and `rect_style` / `truncate`.
- `page.rs` (509) — the page shell, reference frames, drag maths, page bar and their pointer handling.
- `main.rs` (1300) — the shell: state, callbacks, header, rail, stage.

- `scope_ui.rs` (112) — the page-manager dialog, with its own drag state.
- `settings.rs` (642) — the settings sheet: form state, the load-once effect, save, test.
- `panel.rs` (1881 → 1203) — the conversation only.

`docs/ARCHITECTURE.md` now carries the module map. The renderer (`viewer.js`, ~1900 lines) is the
remaining monolith and is the next split.

## Field fix: send the whole document

The user's read is right: a question about the document should carry the document. The hard-coded
20-page cap is gone. The concern it encoded (tokens, latency, cost) now lives in 设置 → 阅读 →
每次最多发送页数, default 全部, with 20 / 50 / 100 / 200 as opt-in limits. The value persists through
`page_limit` in the desktop settings (zero means all), and the composer still says when a limit cut
the request.

## Field fix: references include the page bars

A reader marked pages with the purple bars, chose 仅引用, and the model saw nothing of those pages.
The four-scope model had a hidden "selected pages" state with no icon, so a page selection and the
references icon disagreed about what "references" meant.

Now there are only three scopes. The page bars, page drags and the manager all switch to **仅引用**, and
that scope sends the selected pages **and** the quote/region references. With no pages and no
references, the composer says so before asking. The hidden fourth state is gone from the interface.

## Field fixes: a floating rail and the progress line

- The rail sat in the layout, so its column covered the left edge of the page. It now floats over the
  stage and keeps the approved shape: the pointer as a circle, and one capsule holding text, box and
  pages. (An attempt to split it into four separate circles was rejected — the capsule is the design.)
- The progress line is drawn by its container and sits flush with the header's bottom edge (3 px,
  4 px while hovered); its dot is painted purple with no ring and appears only on hover or focus.
  Flush needed two things: the line covers the header's 1 px bottom border instead of sitting above
  it, and the header paints above the stage (z-index) so the dot may straddle the edge without the
  page covering its lower half. The dot is clamped 5 px from either end so page 1 and the last page
  never cut it.
- The page slider left the bottom strip (which is gone) and became a hairline on the bottom edge of
  the header: page 1 at the far left, the last page at the far right, with the accent fill showing
  progress. The handle appears only when the pointer is on the line or it has keyboard focus.

## Field fixes: precision, focus and page gestures

**Passages are the selection, not its bounding box.** The renderer now reports the selection Range's
tight line boxes (`getClientRects`), stored on the passage and painted one box per line. Selecting one
word highlights one word. Re-reading after a handle drag goes through `caretRangeFromPoint` at the two
corners, so the quote and its boxes follow character boundaries instead of whole text-layer spans. The
old `[` / `]` are replaced by mobile-style purple tabs at the bottom-left of the first line and the
bottom-right of the last.

The ✕ sits outside the frame (left: 100%, top-aligned), and the region's separate move button is gone;
the frame itself moves a region.

**Focus before editing.** In text, box and page modes a reference is inert background: it does not take
the pointer, so text selection and box capture pass straight through it. A click (under 4 px of
movement) focuses the reference under the pointer; only then do the frame, its handles and its ✕
become interactive. Hover shows a faint tint and a dashed outline, nothing else. Switching tools,
pressing outside any frame, or right-clicking clears the focus immediately. The pointer tool keeps
direct manipulation: hover reveals the controls and drags edit without a preliminary click.

**Page gestures.** Page mode disables text selection, tints the page under the pointer, and runs click
and drag through one path: a thin solid purple box follows the pointer from press to release, and every
page it covers gets the same outline while the gesture is live. The purple page bar still appears only
after release, when the set is committed.

### Follow-up: the three blockers

- **Selections reported nothing.** `normalisedSelectionRect` used `unit()` four lines before its
  `const` initialiser — a temporal-dead-zone error, so no payload ever reached the interface and both
  the selection bar and text-mode adding looked dead. The helper is declared first now.
- **Inert references were not inert.** `setTool` never wrote `data-tool` on the stage, so the CSS that
  gives frames `pointer-events: none` never matched. It does now, which is what lets text selection,
  box capture and page clicks pass through an unfocused frame.
- **Page mode** gained the hover wash (a 16 % lilac overlay, lighter than a committed selection), and a
  gesture's meaning is decided by its **starting page**: start on an unselected page and the batch
  adds, start on a selected one and it subtracts. The live outline and marquee turn **white** while
  subtracting; click and drag share the same rule.

### Follow-up: page scrolling, grip dragging

- A subtracting page gesture is a very light grey now, and the hover wash also applies to pages that
  are already in the set.
- Dragging a page gesture to the top or bottom edge of the stage scrolls it, faster the closer the
  pointer is to the edge (64 px zone, up to 24 px per frame), and the marquee and page outlines follow
  the scroll. Releasing outside a page commits the range up to the last page the gesture covered.
- A passage frame never moves: selection is not a box. Only its two endpoint grips resize it.
- The grips were unreachable because pointer capture sat on the frame while the presses started on the
  grip. Capture now sits on the grip itself, which also carries the move/release handlers, so it
  receives every event. While a grip is dragged the marks cannot follow (text is re-read on release),
  so the grips travel to the frame's own edges until the drop.

### Follow-up: grips are range editors

The grips were still rectangle editors: they moved the union box, so no character was added, the marks
never changed, and the release re-read failed often enough that the grip snapped back. That model is
gone.

A passage now stores the selection's absolute character offsets in the page's text layer. A grip press
hands the gesture to the renderer (`beginGripDrag`), which:

- reads a caret under the pointer with `caretRangeFromPoint` and converts it to an offset;
- rebuilds the Range between that offset and the fixed opposite end, so **crossing lines and adding
  words work exactly like a native selection**;
- paints the live selection itself (`.passage-live` inside the frame, which is why its percentages and
  the grips share one origin) and moves both grips to the live ends;
- reports `{start, end, text, rect, rects}` once, on release, through `__pdfscopePassageEdited`; the
  interface records one undo step and re-renders.

While dragging, frames, grips and the live layer drop `pointer-events`, or the caret lookup would hit
the grip instead of text. A pair of offsets that is empty is refused, so old references cannot start a
drag from a stale zero.

Also: a page gesture anchors both ends in the stage's **content** space, so scrolling no longer drags
the starting edge along the viewport or drops the outline from pages already covered; the live box and
the covered pages now agree with the committed result. Double-click opens a file only when no document
is open.

### Follow-up: the doubled selection and grip alignment

The second layer was not a nested reference: it was a **stale node**. Marks were rendered as a plain
`Vec` of views whose `style` was a fixed string, so Leptos reused the existing elements and never
rewrote their geometry; a longer selection appended boxes while the old ones stayed. Marks are now a
`<For>` keyed by passage id plus the rectangle itself, so changed geometry replaces its boxes, and the
frame's position and the grips' positions are reactive attributes that read the current passage. A
region drag also starts from the rectangle read at press time instead of one captured at render.

The grips sit at the vertical centre of the first and last line now, because dragging them is caret
logic, not point logic.

### Follow-up: one source of truth while a grip drags

The grip gesture had two painters — the renderer drew a live layer while the interface also held the
passage — and the two disagreed after every drop. That whole layer is gone. A grip drag now only
reports: the renderer reads the caret, rebuilds the character Range and sends
`__pdfscopePassageEdited`; the interface applies quote, rectangle, boxes and offsets, and every visual
(mark, frame, both grips) is a reactive expression over that one state. `__pdfscopePassageBegin` takes
the single undo snapshot before the first change.

Marks are keyed by passage and line index and their geometry is a reactive style, so dragging updates
the same nodes instead of replacing them, and a changed selection cannot leave a stale copy behind.

### Follow-up: the flatter second layer (second attempt)

It was the **native text selection**, not a second passage. The stylesheet gives
`.textLayer span::selection` the same lilac as a passage mark; the browser paints it at the text's em
box, a little flatter than the line box a mark uses, so after adding a reference the two sat on top of
each other.

`clearSelection` removed the ranges once, before the click's default action finished, and the browser
could paint the old selection again afterwards. It now clears synchronously, on the next animation
frame, and once more after 60 ms.

Two more sources were possible and are handled as well: a Range can report a line box plus smaller
run boxes inside it (around styled or subscript spans), so rects are merged into one box per line
before painting; and while the old selection is being cleared, native paint is muted until the next
pointer press, so a late repaint cannot show. The frame is derived from the union of the merged line
boxes (`geo::union_of`, four tests) instead of the payload union.

### Regression: selections stopped reporting

The rect-merging helper returned only `width`/`height`; the payload code still read `rect.right` and
`rect.bottom`, produced `NaN`, failed the unit-range check and dropped every selection. The merged
rects carry all four edges again, and the payload derives an edge from `left + width` if it is ever
missing.

## Deviations

- The reference undo stack is cleared when a question is sent: references become part of that turn's
  history, and restoring them into the composer would duplicate them.

- The frame itself is the move surface; the region's circular handle is an affordance, not a separate
  drag target.
- The re-read text joins whatever text-layer spans intersect the rectangle; a reader who drags a
  handle across a line break gets the words but not the original spacing.

- Dragging in page mode adds rather than replaces. This supersedes the earlier replace-by-default
  answer, because replace made the middle-range case destructive; 清空 is the only replace gesture.
