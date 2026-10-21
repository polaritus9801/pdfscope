# 0004 — Own the scroll model, do not adopt `pdf_viewer.mjs`

- **Status:** accepted
- **Date:** 2026-10-21
- **Deciders:** z1mLi

## Context

`0015-reader-modes` introduces continuous scrolling. PDF.js ships `pdfjs-dist/web/pdf_viewer.mjs`,
which already implements a page stack, a text layer, zoom and page modes. The product already has
three things that assume a single rendered page and a normalised page coordinate system: text
selection, box capture, and citation landing/tinting. Those interactions are the reason the reader
exists; a scroll model that loses them has failed regardless of how smoothly it scrolls.

The existing boundary is deliberately small: `viewer.js` owns PDF.js and the DOM around the canvas;
Rust owns application state and never imports PDF.js.

## Options considered

### Option A — Adopt `pdf_viewer.mjs`

- Pros: mature scrolling, page modes and zoom; less browser maths to own.
- Cons: a second page/event model beside our callbacks. Its viewer owns the page container, text
  layer and current page, so selection normalisation, region capture coordinates and citation
  scroll/flash would have to be re-expressed in its API; its CSS and event lifecycle become a
  dependency of our product controls. It also does not implement the exact ±1 render window or our
  overlay model.
- Cons: the bundle grows and the pinned PDF.js upgrade path now includes a component we do not own.

### Option B — Rebuild the minimal scroll model behind the current boundary

- Pros: one page model (`data-page` + normalised rects) used by selection, capture, tints and
  citations. The ±1 window and canvas release are observable and testable. Rust keeps the overlays.
- Cons: we own page metrics, scroll anchoring, resize/zoom handling, and render cancellation. These
  are the bugs this ADR is accepting.
- Cons: page modes, spreads and rotation in later phases will need more code than adopting a mature
  viewer.

## Decision

We will keep `ui/public/viewer.js` as the only PDF.js consumer and implement the continuous scroll
model ourselves, preserving the existing three-function boundary and per-page normalised geometry.

## Consequences

- Selection, capture, tints and citation jumps keep one coordinate system.
- The render window is explicit: visible pages plus one either side, keyed by cached page metrics.
- The scroll model is ours to test; `scripts/ui-smoke/reader.mjs` exercises it in a real browser.
- Adopting `pdf_viewer.mjs` later is a reversal, not an incremental change: it would replace the
  renderer module and require the four interaction paths to be re-proven.

## References

- `docs/specs/0015-reader-modes.md`
- [`pdf_viewer.mjs`](https://mozilla.github.io/pdf.js/web/viewer.html)
