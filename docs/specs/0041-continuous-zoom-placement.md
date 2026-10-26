# Continuous zoom placement

Date: 2026-10-26

## Problem

The `0208612` zoom model had two horizontal owners: auto margins while the page fit, and
`scrollLeft` once it overflowed. At the fit boundary an off-centre pointer changed the page's left
velocity in one frame. For a pointer at x=800 the page left went from 117.48 px at scale 0.9969 to
111.00 px at 1.0069 while `scrollLeft` went 0 → 5 px. The page did not jump position, but its pivot
changed abruptly; that is the visible "twist". Vertical fit/centre had the same ownership switch, and
integer `scrollTop` additions could make it worse.

## Placement

A single continuous function now owns the visual position during a zoom gesture. At the fit scale:

```text
centre(s)  = stage content origin + (content - base * s) / 2
pointer(s) = gesture origin - content point * s
overflow  = max(0, base * s - content)
distance  = max(1, abs(gesture origin - content centre))   // screen-space, no fixed scale band
q         = overflow / (overflow + distance)
weight    = q² (3 - 2q)                                    // smoothstep, zero slope at fit
left(s)   = centre(s) + (pointer(s) - centre(s)) * weight
```

At fit, `weight` is zero and the page is centred by the function, not by `margin: auto`. As the page
overflows, the pointer takes over smoothly. The transition length comes from the pointer's real
distance from the content centre; there is no `+0.5`, no fixed pixel band and no scale-mode flag.
The vertical axis uses the same formula with the content height.

`left(s)/top(s)` are absolute screen positions. `stage.scrollLeft/scrollTop` remain integers for the
scrollbars, while the page receives the difference in one transform (`--page-offset-x/y`). This is
not an accumulating residual: every frame resets the transform, measures the real layout, computes
the position from the gesture geometry and writes the offset once. `settleZoomPlacement()` folds the
offset into the scroll position before a gesture ends and keeps any clamped remainder, so the visible
page is continuous through settle.

## Constraints

- `body.panel-dragging` bypasses the placement entirely and keeps the `0208612` fixed-reading-anchor
  path.
- The page ruler keeps its viewport refresh hooks; the native horizontal scrollbar is untouched
  (`overflow-x: auto`). Only the native vertical track is hidden.
- No CSS transition, camera, fixed anchor mode or hardcoded fit-width threshold is used.

## Acceptance

- Zooming in and out through 100 % at a pointer left of, at, and right of centre produces a continuous
  page-left trace, with no one-frame ownership switch.
- The fit/centre boundary has zero placement slope change from the smoothstep weight.
- Page/scroll rect is identical immediately before and after settle.
- Panel divider drag remains identical to `0208612`.
