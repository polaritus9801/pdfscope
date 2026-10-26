# Reader density, page-ruler visibility and fixed zoom anchor

Date: 2026-10-26

## Uniform thumbnail slots

The thumbnail rail and the page-reference manager used the page's own aspect ratio as the tile's
aspect ratio. A tall A4 page therefore produced a 3:4 tile while a landscape page produced a 4:3
tile, so the same grid showed different row counts and page areas for different documents.

Both views now use a **square tile slot** (`aspect-ratio: 1 / 1`) and fit the rendered page inside it
with `object-fit: contain`. The page aspect only changes the letterboxing inside the tile, never the
row height or the slot area. Two reciprocal page shapes (3:4 and 4:3) display the same image area in
the square; other shapes differ only by the smaller fitting dimension, which is the minimum possible
variance for a uniform grid. This is deliberately not a page-aspect variable per item: all items in
both views share one stable slot, so the grid can be laid out and scanned without per-row
compaction.

The obsolete per-page `--thumb-aspect` writes are removed. The thumbnail rail still uses two columns;
the manager still uses `auto-fill, minmax(160px, 1fr)`. Every thumbnail and every manager tile now
has width equal to height.

## Page-ruler visibility

`pdfscope.page-ruler` stores `on`/`off` and defaults to `on`. The setting is on the Reading tab as
"显示页面标尺" and applies immediately.

The custom page ruler is also the reader's scrollbar, so "off" does not remove the thumb or break
scrolling. It hides the canvas marks, section ticks, hover line, tooltip, page preview and reference
bands. The rail background and draggable thumb remain. The renderer reads the
`data-page-ruler="off"` attribute at startup and also exposes `setPageRulerVisible`; hover previews
are skipped while hidden, so turning the ruler off does not keep rendering images nobody can see.

## Fixed zoom anchor

Previously each zoom frame measured the point under the pointer again from the *previous* frame's
scroll position. At slow trackpad pinch or ctrl/cmd-wheel rates, the browser's scrollTop rounding
accumulated and the document slowly walked downward.

At the first input of a zoom gesture the renderer now captures one anchor:

```text
anchor.screenY = pointer/reading-line Y
anchor.contentY = (anchor.screenY - pages.top) / displayScale
```

Every frame restores that same content point:

```text
stage.scrollTop += pages.top + anchor.contentY * newScale - anchor.screenY
```

The anchor is captured again only when the input origin changes (for example, the fingers move) and
is cleared when the zoom settles or the document is torn down. Horizontal zoom remains page-centred.
This makes the result independent of frame cadence and eliminates the cumulative baseline error.
