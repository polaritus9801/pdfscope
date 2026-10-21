# 0005 — Transform-preview zoom on a fixed page layout

- **Status:** accepted
- **Date:** 2026-10-21
- **Deciders:** z1mLi

## Context

The reader's zoom currently changes the page's CSS width, restores the scroll anchor, and then
re-renders the visible pages. Every trackpad tick and every button press therefore produces multiple
visual states; the old canvas does not scale, the text layer is rebuilt, and the browser can paint
the new layout before the scroll correction. The reader asked for continuous pinch, simultaneous pan,
smooth mouse/button zoom, no flash, and text that never disappears.

The renderer is our own module behind `window.PDFScope`; PDF.js only provides page rendering and the
text layer. The page shells, tints and selection bar belong to the Rust side. Any solution has to keep
the normalised page coordinate system that selection, capture and citations depend on.

## Options considered

### Option A — Re-render every animation frame

- Pros: always sharp.
- Cons: cancels itself constantly; page renders are tens of milliseconds and dominate the frame;
  the 400-page budget is already tight; this is the source of the current stepping.
- Rejected.

### Option B — Scale the canvas once and never re-render

- Pros: the simplest and smoothest possible; no render queue during a gesture.
- Cons: permanently soft text at high zoom, and no sharp resting state. A reading tool cannot trade
  all of its text quality for gesture smoothness.
- Rejected as the resting strategy.

### Option C — Fixed page layout, whole-column transform preview, canvas re-render at rest

- The column is laid out once at 100 % base width. During a gesture one CSS transform scales the
  whole column; a scaler element carries the scaled width/height so scroll extents are right. The
  text layer stays at base layout and never re-renders on zoom. After a quiet period the visible
  canvases are re-rendered at display scale and swapped atomically.
- Pros: one compositor property per frame; no page reflow; text and selection stay in one coordinate
  system; old canvas remains visible until the new one is ready.
- Cons: the preview is a scaled bitmap and can be soft; selection bar and marquee need counter-scaling
  or base-coordinate care; we own the scaling maths.

### Option D — Adopt PDF.js `drawingDelay` + `cssTransform`

- PDF.js already delays rendering during a scale change and previews with a canvas CSS transform.
  But its `postponeDrawing` path hides the text layer, which is exactly what the reader complained
  about. It also owns the page container, which would sit beside our Rust overlays.
- Rejected; we keep the delay/CSS-transform ideas, not the page model or the hidden text.

## Decision

We will keep the page column at a fixed base layout and zoom by transforming the column, with a
scaler element providing the scaled scroll size. The text layer is built once per base width; only
visible canvases are re-rendered after a gesture settles, and each new canvas is swapped in only
when complete.

## Consequences

- Pinch is one compositor transform per frame rather than a render pipeline; pan and zoom can be
  written together in one animation frame.
- Text stays visible and selectable throughout; selection and capture keep using normalised page
  coordinates.
- The resting canvas can be capped in pixels; above the cap the page is softer on purpose.
- The Rust zoom signal no longer drives layout. The renderer owns the transient scale and reports a
  display-only percent back to the header.
- If the transform layer becomes a memory problem on very large documents, the fallback is per-page
  transforms, not a return to reflow.
- Reverting means restoring the phase-1 CSS-width zoom; the spec's test stream gives the measurement
  for comparing.

## References

- `docs/specs/0017-smooth-zoom.md`
- [PDF.js `PDFPageView.update` / `cssTransform`](https://github.com/mozilla/pdf.js/blob/master/web/pdf_page_view.js)
- [PDF.js `PDFViewer.updateScale` / `panBy`](https://github.com/mozilla/pdf.js/blob/master/web/pdf_viewer.js)
- [Apple `GestureEvent`](https://developer.apple.com/documentation/webkitjs/gestureevent)
