# 0031 — Panel drag keeps one reading anchor

**Status:** accepted.

Dragging the AI panel changes the PDF viewport every frame. The previous implementation anchored the
page centre while dragging, then let `runResizeCheck` re-anchor to the reading line on release. The two
anchors disagreed and the base layout was reflowed after the visual scale had already been reset, so
every release jumped once and repeated drags accumulated a vertical drift.

## Algorithm

- During the drag, the PDF is anchored at the **reading line**, the same point used by the release
  path. The point under `stage.top + readingOffset()` stays stationary while the panel width and the
  fit scale change.
- Panel width changes are coalesced into one requestAnimationFrame; each frame changes the stage
  width, derives the fit scale and adjusts `scrollTop`/`scrollLeft` from measured page rectangles.
  No per-pointer-event reflow or Leptos render occurs.
- Short columns stay vertically centred during the drag as well, using the same formula as
  `centerAtRest`, so a one-page/zoomed-out document does not jump when the pointer lifts.
- On pointer-up, the reading-line anchor `{page, fraction}` is captured **before** anything else
  changes. The fit-width base layout is then reflowed to the final stage width, the saved zoom is
  restored, and `scrollToAnchor(anchor)` puts that same PDF point back on the reading line. The
  release path no longer calls `runResizeCheck` (which would choose a new anchor).
- The panel width is rounded only after the final frame; the ≤1 px difference is absorbed by the
  anchor restoration.

## Acceptance

- Dragging the divider scales the PDF continuously; the point at the reading line does not drift.
- Releasing the pointer moves the page by no more than the sub-pixel rounding of the final width
  (measured ≤1 px in the browser probe).
- Repeated push/pull cycles do not accumulate vertical movement.
- A short (vertically centred) document does not jump at release.
- `just check` green.
