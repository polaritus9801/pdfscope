# 0031 — Panel drag keeps one reading anchor

**Status:** accepted.

Dragging the AI panel changes the PDF viewport every frame. The previous implementation anchored the
page centre while dragging, then let `runResizeCheck` re-anchor to the reading line on release. The two
anchors disagreed and the base layout was reflowed after the visual scale had already been reset, so
every release jumped once and repeated drags accumulated a vertical drift.

## Algorithm

- During the drag, the PDF is anchored at the **reading line**, the same point used by the release
  path. The anchor is a fixed `{page, fraction}` captured at pointer-down, not a point recomputed from
  the current scroll. Each frame restores that absolute page point to the line; sub-pixel scroll
  rounding therefore cannot feed back into the next frame. A 1 px/frame drag used to accumulate ~60 px
  of downward drift over 300 px; the fixed anchor keeps the error under 1 px.
- The release commit reuses this same page/fraction, so it does not adopt the small residual rounding
  error from the last drag frame.
- Panel width changes are coalesced into one requestAnimationFrame; each frame changes the stage
  width, derives the fit scale and adjusts `scrollTop`/`scrollLeft` from measured page rectangles.
  No per-pointer-event reflow or Leptos render occurs.
- Short columns stay vertically centred during the drag as well, using the same formula as
  `centerAtRest`, so a one-page/zoomed-out document does not jump when the pointer lifts.
- The panel reflow also changes each page's base width. Mounted canvases used to keep the CSS width
  and height of the old base viewport, so after the reflow they overflowed their page at the old
  base size for one frame, until the async rerender replaced them. The canvas CSS box now fills its
  `page-layer` (`width/height: 100%`) and has no inline base-size styles; the old bitmap is briefly
  stretched into the new box until `rerenderCanvas` catches up.
- PDF.js writes the old base width/height inline on the text layer as well. Even though the text is
  transparent, that box widened the stage's `scrollWidth` for one frame and flashed the horizontal
  scrollbar. The text layer now fills and is clamped by its page layer (`width/height: 100%`,
  `max-width/max-height: 100%`, `overflow: clip`) until its next render.
- On pointer-up, the final pointer coordinate is applied on the next animation frame. The anchor
  `{page, fraction}` is captured while that frame's drag scale is still applied; the base layout is
  reflowed and the saved zoom restored only on the following frame. This lets the last drag frame
  reach the screen first: its visual page size already equals the post-reflow size, so no
  different-scale frame is painted. The release path no longer calls `runResizeCheck` (which would
  choose a new anchor).
- The panel width is rounded only after the final frame; the ≤1 px difference is absorbed by the
  anchor restoration.
- Horizontally, the page's screen centre at pointer-down is held for the drag, clamped when the
  shrinking stage can no longer contain the page. The page does **not** re-centre on every frame
  (that made the document slide left with the divider), and the normal centred layout resumes on
  commit. This is the visual complement to the fixed page/fraction vertical anchor.

- The page gap is part of the base layout. It is stored as a ratio captured from the initial CSS gap
  (`16 / 1032` in the probe) and applied as `ratio × baseWidth` on every reflow. During a drag the
  whole column is transformed, so the visual gap is `baseGap × dragScale`; after reflow the new base
  gap is `ratio × newBaseWidth`, which equals that same visual value. A fixed 16 px base gap would
  snap from 14.73 px back to 16 px at release.
- Zoom is a transform, not a reflow: the gap deliberately scales with `displayScale` (16 px at 100 %,
  64 px at 400 %, 8 px at 50 %). It is part of the page column, not fixed UI chrome.

## Acceptance

- Dragging the divider scales the PDF continuously; the point at the reading line does not drift.
- Releasing the pointer moves the page by no more than the sub-pixel rounding of the final width
  (measured ≤1 px in the browser probe).
- Repeated push/pull cycles do not accumulate vertical movement, including a slow 1 px/frame drag.
- The fixed page/fraction is reused at release; residual movement is ≤1 px.
- A short (vertically centred) document does not jump at release.
- Page gaps during the drag and after release match within 0.02 px; the zoomed gap scales with the
  page column rather than snapping to a fixed screen size.
- A mounted canvas or text layer never exceeds its page layer during the reflow; only their backing
  content is replaced asynchronously.
- The stage has no spurious horizontal overflow frame after a panel drag, whether the horizontal
  scrollbar was initially present or absent.
- `just check` green.
