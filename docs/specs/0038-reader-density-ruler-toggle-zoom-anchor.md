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

## Pointer-fixed zoom anchor

Previously each zoom frame measured the point under the pointer again from the *previous* frame's
scroll position. At slow trackpad pinch or ctrl/cmd-wheel rates, the browser's `scrollTop` rounding
accumulated and the document slowly walked downward. The same code also forced horizontal scroll to
the absolute centre of the overflow, so zooming with the pointer near the page edge pulled the page
back to the centre.

At the first input of a zoom gesture the renderer captures one anchor in the page column's unscaled
coordinate space:

```text
anchor.screenX = pointer/reading-line X
anchor.screenY = pointer/reading-line Y
anchor.contentX = (anchor.screenX - pages.left) / displayScale
anchor.contentY = (anchor.screenY - pages.top) / displayScale
```

Every frame restores that same content point:

```text
stage.scrollTop  += pages.top  + anchor.contentY * newScale - anchor.screenY
stage.scrollLeft += pages.left + anchor.contentX * newScale - anchor.screenX
```

Horizontal placement uses one signed offset relative to the stage:

```text
left = anchor.screenX - stage.left - anchor.contentX * newScale
```

A non-negative offset (and any column narrower than the stage) is expressed as
`margin-left`; a negative offset on an overflowing column is expressed as `scrollLeft`. At the
exact fit width both forms agree (`left = 0`), so crossing fit width is continuous. This replaces
the old two-mode layout — auto-margin centring below fit width, scroll-overflow above it — which
caused a visible snap when a slow zoom crossed that threshold. When there is no pointer anchor
(initial load, resize, explicit fit) the same helper centres the column.

`finishZoom` may re-centre vertically, but never snaps horizontal overflow back to the centre.

The anchor is recaptured only when the input origin moves (for example, the fingers move) and is
cleared when the zoom settles or the document is torn down. This is a focused fix to anchor and
placement bookkeeping; the page layout, scale easing and render path are unchanged. It removes the
cumulative downward drift, the fit-width snap and the unnatural horizontal recentre.

## Mainstream reference

The anchor model is the same one used by established viewers, not a bespoke formula:

- [PDF.js `updateScale` with an `origin`](https://github.com/mozilla/pdf.js/commit/b7933d8750835d5743c4aa724b408701ded6588f)
  preserves a document point and has integration tests for cursor/pinch origin. Its
  [smooth trackpad-pinch commit](https://github.com/mozilla/pdf.js/commit/9899ae0453fb5952f472df4688f7467a68b15aca)
  also accumulates fractional scale factors instead of discarding sub-percent ticks.
- Chromium's PDF viewport tracks a `pinchPanVector` and combines it with the pinch centre in
  `setPinchZoomInternal`; see the [pinch-zoom implementation review](https://codereview.chromium.org/2400743002)
  and the [PDF viewport message bridge](https://chromium.googlesource.com/chromium/src/+/67ace07cb299e11dff5771497ec0698036477d3f/pdf/pdf_view_plugin_base.cc).
- Native PDFKit/Preview sits on `NSScrollView`, whose
  [`setMagnification(_:centeredAt:)`](https://developer.apple.com/documentation/appkit/nsscrollview/setmagnification(_:centeredat:))
  is the canonical anchor-at-point magnification API. The scroll view provides simultaneous
  magnification and pan without a layout threshold.

A fully compositor-style camera (single affine transform over a fixed page canvas) is the other
mature design. It is required for truly simultaneous pinch-and-pan on every input stack; the
scroll/margin implementation above is the minimal continuous version that fits the existing
virtualised DOM.
