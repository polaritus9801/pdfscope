# Reader density, page-ruler visibility and pointer-fixed zoom anchor

Date: 2026-10-26

## Aspect-preserving, square-bounded tiles

A fixed square tile made every page letterbox: a 16:9 slide became a wide strip with empty bands
above and below. The two earlier choices were a fixed square cell with letterboxing, or fitting each
page inside a square bound and letting the other dimension follow. The second is used.

For a page ratio `r = width / height` and a square bound `S`:

```text
tileW = S * min(1, r)
tileH = S * min(1, 1 / r)
```

The page therefore always keeps its own aspect ratio and is never larger than the square. A row of
16:9 pages is full-width and compact, exactly like the old thumbnail grid; an A4 page becomes a
narrower, full-height tile. No invisible square remains around the page.

The thumbnail rail is a wrapping flex row with `S = (container width - gap) / 2`; the page manager
uses `S = clamp(140px, (container width - 5 gaps) / 6, 180px)`, which gives the same six full-width
landscape columns as the old manager and drops columns gracefully on narrow windows. Both use the
same per-page `--thumb-ratio` (width/height), so the CSS needs no per-page width/height measurement
or JavaScript layout pass. `object-fit: contain` remains a
safety net, but at these dimensions the image fills the tile. Row height follows the tallest item in
that row; rows no longer inherit a fixed square height.

The obsolete fixed `aspect-ratio: 1 / 1` rules and per-page `--thumb-aspect` writes are gone.

## Page-ruler visibility

`pdfscope.page-ruler` stores `on`/`off` and defaults to `on`. The setting is on the Reading tab as
"显示页面标尺" and applies immediately.

The custom page ruler is also the reader's scrollbar, so "off" does not remove the thumb or break
scrolling. It hides the canvas marks, section ticks, hover line, tooltip, page preview and reference
bands. The rail background and draggable thumb remain. The renderer reads the
`data-page-ruler="off"` attribute at startup and also exposes `setPageRulerVisible`; hover previews
are skipped while hidden, so turning the ruler off does not keep rendering images nobody can see.

## Zoom

The zoom model was simplified again after this document: there is one scroll-following engine with
sub-pixel residual compensation and no selectable modes. See
[0040](0040-single-scroll-zoom.md).
