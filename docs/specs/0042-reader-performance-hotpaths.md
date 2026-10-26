# Reader performance hot paths

**Status:** accepted.

Date: 2026-10-26

## Decision

The page ruler derives its marks from the cached base page metrics and `displayScale`; it no
longer reads every page's bounding rect. The base metrics are rebuilt only when the page layout
changes, which is the existing `metricsDirty` contract.

During an AI-panel drag, the ruler's full measure is skipped. The stage width and scaled column
change every frame, but the vertical page layout does not: cached metrics plus the current visual
scale are enough for the thumb and viewport. `commitPanelResize()` refreshes the ruler once after
the final base reflow.

## Why

At 3000 pages a panel drag caused ~3012 `getBoundingClientRect()` calls per frame from
`measurePageRuler()`, raising the frame from ~17 ms to ~32–35 ms. Deriving marks from metrics
removes that O(pages) layout read. Skipping the redundant measure while the panel drags avoids
redrawing the same ruler geometry dozens of times.

Zoom settle and window resize still refresh the ruler, but now from metrics; they do not scan page
rects either.

## Acceptance

- During a panel drag, per-frame `getBoundingClientRect()` calls stay bounded and independent of
  page count.
- The ruler marks and thumb are refreshed after release.
- Normal zoom, fit-width, page modes, reference bands and the page ruler all remain correct.
- The headless zoom/panel smoke includes a layout-read regression check.
