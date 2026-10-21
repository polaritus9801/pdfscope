# 0015 — A reading surface that behaves like a reader

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi (asked to implement phase 1, 2026-10-21)

## Goal

The page area stops being a single-page canvas and becomes a reading surface: continuous scrolling by
default, page and spread modes, zoom that anchors where the reader is looking, an outline and
thumbnails, in-document search, and a scroll position that survives closing the tool. Everything the
selection, region capture and citation machinery already does must keep working on top of it.

## Why now

PDF.js ships a viewer component that implements most of this (`pdfjs-dist/web/pdf_viewer.mjs`). We
vendored only the core renderer and hand-rolled one page at a time, which is why the surface feels a
generation behind. This spec defines the behaviour; the implementation may adopt that component or
rebuild its scroll model behind our existing three-function boundary.

## Plan, in phases

### Phase 1 — Continuous scrolling (the default)

- Every page is laid out vertically inside one scroll container, sized from the PDF's own page sizes.
- Only pages in view (plus one either side) are rendered; the rest are placeholders with the right
  height, so the scrollbar does not jump.
- A page renders when it enters the window and its canvas is released when it leaves. Memory stays
  bounded on a 500-page document.
- The page indicator follows the scroll and typing a page scrolls to it.
- Zoom re-renders visible pages only, and keeps the reader's current line in place.
- `⌘↑`/`⌘↓` jump to the document's start and end.

**Acceptance:** open a 400-page scan, scroll from page 1 to 400 and back without a blank frame longer
than a blink; memory does not grow with pages visited.

**Phase 1 implementation decision.** We rebuild the scroll model in `ui/public/viewer.js` rather
than adopting `pdf_viewer.mjs`. The existing three-function boundary (`load`, `render`, `pageText`)
plus its small callbacks is what selection, capture and citations already speak; adopting a second
viewer would put a second page/coordinate model beside it. The cost is that the scroll maths is ours
to test. The decision is recorded in ADR 0004.

Layout is CSS-first: the Rust side renders one placeholder per page with an `aspect-ratio` from the
PDF's own page size, and the renderer mounts a canvas and text layer only into the ±1 window. Rust
continues to own application state and the tint/selection overlays, one set inside each page shell.

### Phase 2 — Modes and orientation

- **Continuous** (default), **single page**, **two-page spread**.
- A spread aligns on the document's first page and shows an odd/even correction control.
- Rotate 90° left/right; the text layer rotates with the page.
- The mode and rotation are remembered per document.

**Acceptance:** switching modes keeps the current page; a spread on a 300-page document never shows
the same page twice or skips one.

### Phase 3 — Navigation surfaces

- Outline (the PDF's own table of contents) in a collapsible left rail; clicking an entry jumps.
- Thumbnails in the same rail, generated lazily and cached per page.
- A reading-progress bar that is also a scrubber.
- Keyboard: `↑ ↓ PageUp PageDown Home End`, `[` `]` for previous/next page, `0`–`4` for zoom presets.

**Acceptance:** a PDF with a 40-entry outline navigates correctly; thumbnails for a 400-page document
do not block the first paint.

### Phase 4 — Zoom that behaves

- Presets: fit width, fit page, actual size, 50 %–400 %.
- Trackpad pinch and `ctrl`+wheel zoom **around the pointer**, not around the top-left corner.
- After a zoom change the content under the pointer stays under the pointer.
- Renders are debounced to one per animation frame and stale renders are cancelled.

**Acceptance:** pinch over a paragraph keeps that paragraph under the fingers; zooming while
scrolling never queues more than one render per page.

**Detailed design:** `0017-smooth-zoom.md`.

### Phase 5 — Find in document

- `⌘F` opens a find bar: type, see `3 / 17`, `↵` and `⇧↵` step through hits.
- Hits are highlighted in the text layer of every rendered page, and the current hit scrolls into
  view.
- Searching is per document and cleared when the document closes.

**Acceptance:** searching a word that appears 17 times finds all of them on a 100-page document in
under a second.

### Phase 6 — Everything else that already exists must still work

- Text selection per page, with the same normalised rectangles.
- Region capture: the marquee is clipped to one page; scrolling while armed cancels it.
- Reference tints follow their page in every mode.
- Citation links scroll to the page and flash it.
- Page-range text extraction is unaffected.

**Acceptance:** the five-step list in `docs/acceptance.md` passes in continuous mode as well as in
single-page mode.

## Interaction design — phase 1

No new control is introduced. The existing controls keep their labels and locations; what changes is
what "the page" means when more than one is on screen.

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Reading surface | — | centre | a document is open | wheel / trackpad scrolls every page in one column | `PageUp` `PageDown` and `←` `→` move one page, as before; `Home` `End` reserved for phase 3 | drag | the page indicator follows the page crossing the reading line (35 % from the top); render window updates after the scroll frame | scrolling while capture is armed disarms capture |
| Page field | `31 / 404` | header, centre | a document is open | click, type, `↵` or blur | focus with `Tab`, `↵` commits | tap | the column scrolls so page 31's top edge is 16 px below the stage top; out-of-range input is clamped | a non-number reverts without a message |
| Previous / next | `‹` `›` | header, centre | a document is open | click | `←` `→`, `PageUp` `PageDown` | tap | scrolls one page and clears the selection, as before | disabled at the ends |
| Document ends | — | reading surface | a document is open | — | `⌘↑` / `⌘↓` | — | scrolls to the first/last page and clears the selection | no document leaves the keys inert |
| Citation | `p.31` | inline in the answer | the page exists | click | `Tab` + `↵` | tap | scrolls to page 31, flashes its frame for 800 ms, and leaves the page indicator at 31 | out-of-range citations stay plain text |
| Zoom | `−` `100%` `+` | header, right | a document is open | click; trackpad pinch / `ctrl`+wheel over the page | `+` `-` | pinch | visible pages re-render at the new scale; the reading line keeps its page and fraction within the page | at the limits the buttons disable |
| Selection | — | page | the page has a text layer | drag | `Shift`+arrows | long-press | the bar appears on the page that holds the selection; the reported rectangle is normalised to that page | clearing or navigating dismisses it; a drag that spans two pages is not reported (cross-page selection stays a non-goal) |
| Box capture | `框` | floating rail | a document is open | drag inside one page | `Esc` cancels | drag | the marquee and the crop belong to the page under the pointer | scrolling cancels; a drag under 4 px cancels; the marquee is clipped to its page |
| Reference tint | — | on its page | a passage or region exists | — | — | — | stays on the original page while scrolling, whether or not that page currently has a canvas | removing the reference removes the tint |

**Alternatives considered.**

- **Adopt `pdf_viewer.mjs`.** Faster to a mature scroll model, but it brings its own page/event model
  and would sit beside the selection/capture/citation callbacks that already work. Rejected for
  phase 1; ADR 0004 keeps the door open if our maths starts costing more than it saves.
- **Virtualise with `IntersectionObserver`.** It is concise, but it reports intersections
  asynchronously and does not express "one page either side" directly, which is what the acceptance
  criterion measures. Scroll-position maths over cached page metrics is deterministic and testable.
- **Keep all 400 canvases and let the browser discard them.** Rejected: canvas memory is not
  reliably reclaimed, and the spec promises bounded memory, not a hope.
- **Position tints in one overlay above the column.** Rejected: every tint would need a dependency on
  scroll and page offsets; one tint inside each page's own coordinate space is the simpler invariant.
- **Page indicator by "page with the most visible area".** Rejected: it jumps when a short page and a
  tall page share the viewport. The 35 % reading line changes exactly when the reader crosses into
  another page.

**Cost of being wrong.** The reader loses their place, or clicks a citation and lands on the wrong
page. Both are recoverable with the page field, but a tint attached to the wrong page would be a
false citation; the page-normalised tint and the page stored in the reference are the same number,
and the DOM keeps that association even when the canvas is released.

## Non-goals

- Annotations, highlights, drawing, forms, signing, printing.
- Reflowing the PDF text into an ebook layout.
- Cross-page text selection.

## Data model

```rust
pub struct ViewState {
    pub mode: Mode,          // Continuous | Single | Spread
    pub zoom: Zoom,          // FitWidth | FitPage | Actual | Percent(u16)
    pub rotation: u16,       // 0, 90, 180, 270
    pub page: u32,
    pub scroll: f32,         // 0..1 within the document
}
```

Stored per document hash alongside the reading position, so reopening returns to the same place.

Phase 1 carries only the layout inputs that the renderer needs; the persisted `ViewState` arrives
with phase 2.

## Interface — phase 1

The existing boundary stays; two functions gain a continuous-page meaning and one is added.

```js
pageSizes(): Array<{ width, height }>   // base size per page, for Rust's placeholders
render(page, zoomPercent): Promise<void> // make `page` visible, apply zoom, paint the window
flash(page?: number): void              // scroll to the page if needed, flash its frame
window.__pdfscopePage(page)             // new: the page crossing the reading line
```

`__pdfscopeSelection`, `__pdfscopeRegion`, `__pdfscopeCleared` and `__pdfscopeZoom` are unchanged.
`pageSizes` is synchronous; `render` remains the one awaitable call.

## Test plan — phase 1

- **Headless browser** (`scripts/ui-smoke/reader.mjs`, a generated 40-page PDF with a text layer):
  page shells exist for every page; after scrolling to page 18, only pages 17–19 (plus the visible
  page) have canvases; scrolling back releases page 18 and mounts page 1 again; the page field
  follows the scroll.
- **Multi-page view:** at 50 %, page 1 and the start of page 2 share the viewport, and the render
  window stays bounded.
- **Selection:** programmatically select a line on page 18; the reported `page` matches, the bar sits
  on page 18, `加入引用` creates a tint on page 18; scroll to page 30 and the tint is still on page 18,
  not on the current page.
- **Box capture:** arm `框`, drag inside page 18, receive a region whose `page` is 18 and whose rect
  is inside the unit square; arm again, start a drag, scroll the column, and assert the drag was
  cancelled and no second region appeared. The saved crop is inspected at full size, not only as a
  thumbnail.
- **Citation jump:** with the fake Tauri bridge answering `[p.31]`, click the citation and assert the
  column shows page 31, the indicator is 31 and page 31 carries the flash class.
- **Zoom:** at 125 %, page 18 keeps its page and its fraction under the reading line, and only the
  window pages re-render.
- **Keyboard:** `⌘↓` reaches the last page and `⌘↑` returns to the first.
- **400 pages:** `READER_PAGES=400 node scripts/ui-smoke/reader.mjs /tmp/reader-stress` passes the
  same assertions; only the visible window (±1) stays mounted and the last page is reached and released.
- **Screenshots:** `reader-01-continuous.png`, `reader-02-multi-page.png`,
  `reader-03-scroll-window.png`, `reader-04-selection.png`, `reader-05-reference-tint.png`,
  `reader-06-marquee.png`, `reader-07-region.png`, `reader-08-citation.png`, `reader-09-zoom.png`;
  the raw region crop is `reader-region-crop.png`.
- **Manual (not available in this environment):** the same steps in the Tauri window on macOS; a real
  400-page scan; and a heap profile for the memory claim. The headless run observes canvas count, not
  process memory.

## Deviations

- Page shells are rendered by Rust, and the renderer fills a `.page-layer` inside each one. This was
  not in the phase-1 bullet list but is what lets tints, the selection bar and the citation flash stay
  in application state while the canvas is virtualised.
- Page sizes are measured for every page during `load` (eight at a time) so the placeholders have the
  PDF's own heights. A 400-page fixture passes the whole suite in about 13 s on this machine,
  including the 0017 zoom assertions.
- Zoom uses CSS width plus a re-render of the window, anchored by page and fraction. Pointer-centred
  pinch stays in phase 4; the existing `ctrl`+wheel zoom now anchors the reading line instead of the
  top-left corner.
- `Home`/`End` stay reserved for phase 3; `⌘↑`/`⌘↓` are implemented because phase 1 names them.
- The 400-page check is a generated text PDF, not a scan, and canvas count is the memory evidence. A
  real scan and a heap measurement remain manual. Blank-frame duration is not profiled either: the
  test asserts the visible page reaches `data-rendered` after a scroll, not how many frames the
  placeholder was visible.
- The persisted `ViewState` is phase 2; phase 1 keeps the position for the open session only. Phases
  2–6 (modes, navigation surfaces, pointer-centred zoom, find, the full acceptance walkthrough) remain
  unimplemented and are not claimed by this change.

## Risks

- **The scroll model is the whole slice.** Selection, capture coordinates and citation jumps all
  assume one page in view; each has to be re-verified in continuous mode.
- **Memory.** A 500-page document must not keep 500 canvases; releasing bitmaps on exit from the
  window is the mechanism and needs a measurement, not an assumption.
- **Two implementations tempting each other.** If we adopt PDF.js's viewer component, our three
  function boundary must survive, or region capture and citations break.

## Open questions

1. Adopt `pdf_viewer.mjs` or rebuild the scroll model ourselves? Adopting is faster and more mature;
   rebuilding keeps the boundary and the bundle smaller. **Phase 1:** rebuild, recorded in ADR 0004;
   the decision is re-openable if page modes and rotation outgrow this module.
2. Thumbnails: generate on demand from the rendered page (cheap) or render at low resolution ahead of
   time (smoother, more memory)?
3. Should the outline replace the floating tools' position, or live in a rail that can be hidden?
