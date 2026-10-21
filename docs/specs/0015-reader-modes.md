# 0015 — A reading surface that behaves like a reader

- **Status:** draft
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** —

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

## Risks

- **The scroll model is the whole slice.** Selection, capture coordinates and citation jumps all
  assume one page in view; each has to be re-verified in continuous mode.
- **Memory.** A 500-page document must not keep 500 canvases; releasing bitmaps on exit from the
  window is the mechanism and needs a measurement, not an assumption.
- **Two implementations tempting each other.** If we adopt PDF.js's viewer component, our three
  function boundary must survive, or region capture and citations break.

## Open questions

1. Adopt `pdf_viewer.mjs` or rebuild the scroll model ourselves? Adopting is faster and more mature;
   rebuilding keeps the boundary and the bundle smaller.
2. Thumbnails: generate on demand from the rendered page (cheap) or render at low resolution ahead of
   time (smoother, more memory)?
3. Should the outline replace the floating tools' position, or live in a rail that can be hidden?
