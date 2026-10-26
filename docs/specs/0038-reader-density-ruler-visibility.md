# Page-ruler visibility

**Status:** accepted.

Date: 2026-10-26

## Aspect-bounded thumbnail slots

The thumbnail rail and the page-reference manager previously forced every page into a square tile.
A 16:9 page therefore received a square slot with letterbox bands, while A4 received a narrow tall
image inside the same square. Both views now fit the page inside a common square **bound** while
preserving its own aspect ratio, then wrap the tiles continuously on the flex row.

The manager tile sets `--thumb-ratio: width / height` from the rendered page. Its width is
`min(bound, bound × ratio)` and its aspect ratio is the page's ratio, where `bound` is a readable
`clamp(140px, six columns, 180px)`. A landscape row therefore becomes shorter and fills the width;
A4 becomes a narrower full-height tile without letterbox bands. The thumbnail rail uses the same
measured ratio, so images have no internal bands. The obsolete `--thumb-aspect` string writes and
the fixed square slot are removed.

## Page-ruler visibility

`pdfscope.page-ruler` stores `on`/`off` and defaults to `on`. The Reading tab's "显示页面标尺"
checkbox applies immediately.

The custom page ruler is also the reader's scrollbar, so "off" does not remove the thumb or break
scrolling. It hides the canvas marks, section ticks, hover line, tooltip, page preview and reference
bands. The rail background and draggable thumb remain. The renderer reads
`data-page-ruler="off"` at startup and exposes `setPageRulerVisible`; hover previews are skipped
while hidden.

## Zoom and panel drag

Not changed here. Zoom stays on the per-frame pointer implementation from `0208612`; the ruler
refreshes its viewport after the zoom frame without owning the scroll position.
