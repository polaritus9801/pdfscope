# Scroll-anchored zoom

Date: 2026-10-26

## Decision

Zoom uses the same model as a conventional PDF viewer: the page column is laid out once, and
`scrollLeft/scrollTop` alone place it while the viewport is overflowed. There is no transform offset,
camera, continuous fit bridge or per-frame scroll fold.

While the column fits, CSS `margin: 0 auto` centres it and `scrollLeft` is zero. Once it overflows,
the pointer-anchored scroll is kept; the page is not forced back to the centre. This is deliberately
the `0208612` scroll model, not the experimental centred bridge.

## Startup and release

- A pending `zoomFrame` and stale target from the previous gesture are cancelled at `gesturestart`.
- The first `gesturechange` sample is eased over one frame. The first WebKit sample can carry a
  several-percent scale step; applying it directly made the first painted frame jump. Later samples
  follow the fingers directly.
- `gestureend` clears direct mode so any unapplied remainder eases instead of snapping.
- No transform offset exists to fold back at release, so the system horizontal scrollbar thumb and
  the page rectangle end the gesture on the same frame.

## Why not a continuous fit bridge

A custom bridge can make the fit-width pivot change smaller, but it necessarily keeps the page
closer to the fit centre for part of the zoom. That reads as "forced horizontal centring", and the
transform/fold cycle adds feedback to `scrollLeft` that can produce left/right jitter. The mature
scroll-anchored model is simpler and leaves the pointer scroll in charge as soon as the page
overflows. The fit boundary still changes the horizontal pivot velocity; this is inherent to a page
that is centred while it fits and scroll-anchored once it overflows.

## Constraints

- `body.panel-dragging` keeps the `0208612` panel path unchanged.
- The native horizontal scrollbar is untouched (`overflow-x: auto`); only the vertical native track
  is hidden by the page ruler.
- No CSS transition, camera, mode flag, fixed anchor or per-frame scroll compensation is used.

## Acceptance

- Zooming out to fit leaves the page centred by layout, not by a zoom transform.
- Once overflowed, a pointer-driven scroll position is preserved instead of being pulled to centre.
- Releasing a pinch does not change the page rectangle or `scrollLeft` in a final transaction.
- Panel divider drag remains identical to `0208612`.
