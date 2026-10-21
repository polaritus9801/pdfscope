# 0017 — Smooth, continuous zoom and pan

- **Status:** implemented
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("都听你的", 2026-10-21)

## Goal

Zooming feels like moving a sheet of paper, not stepping through renders. A trackpad pinch changes
the scale continuously around the fingers, two-finger panning keeps working at the same time, mouse
wheel zoom eases instead of snapping, and `+`/`−` animate. The words on the page stay visible from the
first frame of a gesture to the last; the canvas sharpens when the hand stops. When the document
becomes narrower than the reader (zoomed out), it re-centres instead of leaving the page stuck at an
edge.

## Why now

The phase-1 zoom changes the page's CSS width first, then restores the scroll anchor, then re-renders
the visible pages. That is three visual states per input event. Pinch arrives as a stream of
`ctrl`+wheel events, each rounded to a 10 % step and each triggering a full Rust signal → layout →
render cycle. The old canvas is sized in pixels, so it does not scale with the page while the new one
is being prepared. The text layer is rebuilt as part of that cycle. A button click sets the zoom
immediately, so the browser paints the new width before the scroll correction and the new canvas.

The result is the stepping, flashing and missing text the reader sees. This spec fixes the display
model, not the buttons around it.

## Non-goals

- Page modes, rotation, thumbnails and find (0015 phases 2, 3, 5).
- Re-rendering the PDF at 60 fps. The bitmap is allowed to be soft **while the hand is moving**.
- A new PDF renderer, a new dependency, or changing the PDF.js boundary beyond the functions below.
- Persisting zoom per document (phase 2).
- Making Linux/Wayland trackpad gesture events identical to macOS; best-effort only.

## Behavior

### One layout at a time

The column has a fixed **base layout** measured for 100 % (fit width). Pages keep that width and
height for the whole gesture. The visual scale is applied to the whole column with one CSS
`transform: scale()`. A scaler element around the column gets the scaled width and height, so the
scrollbar and scroll extents are correct while the inner 400 pages never reflow during a gesture.

```
.stage (scroll)
  .pages-scaler            width/height = base * scale
    .pages (fixed base)    transform: scale(scale), origin top left
      .page × N
```

The text layer is built at the base layout and stays mounted for the life of the page. It scales
with the transform, so selection rectangles, reference tints and the capture marquee keep their
normalised coordinates. A canvas is re-rendered only after the gesture settles, then swapped
atomically; until then the old canvas is scaled and remains visible. No path in a zoom changes
removes the text layer or leaves a blank page behind. UI chrome inside the page is counter-scaled
(the selection bar keeps its type size) or positioned in base coordinates (the marquee), so 400 %
does not produce giant controls.

A stage resize (window or panel width) is the only event that re-flows the base layout: it
re-measures the base width, rebuilds text layers and canvases once, and restores the reading line.
It does not happen per zoom frame.

### Input

- **WebKit trackpad pinch** (`gesturestart`/`gesturechange`/`gestureend`, macOS WKWebView): the
  cumulative `gesturechange.scale` multiplies the scale at gesture start. The gesture origin is the
  last pointer position or the reading line when no pointer is known. `preventDefault` stops page
  zoom.
- **Chromium trackpad pinch** (wheel with `ctrlKey`, `deltaX === 0`): a multiplicative factor
  accumulated from `deltaY` (`exp(-deltaY / 100)` with a sub-threshold accumulator, as PDF.js does),
  anchored at `clientX/clientY`.
- **Mouse wheel with `ctrl`/`⌘`**: the same multiplicative zoom, but eased over 120–180 ms instead of
  applied in one jump. One notch must not look like two paints.
- **Two-finger pan**: non-`ctrl` wheel events keep native scrolling and momentum. During a pinch,
  native panning may continue; wheel deltas that carry horizontal movement are applied as pan in the
  same animation frame. Zoom and pan are both written once per frame.
- **`+` / `−` / `适应宽度` and `+`/`−`/`0` keys**: ease 25 % steps around the reading line at 35 % of
  the stage height. The label updates while the animation runs; buttons disable at the limits.
- Range stays 50–400 %, rounded to whole percent for display only. Internal scale is continuous.

### Anchoring and re-centring

Every scale update adjusts `scrollTop`/`scrollLeft` in the same animation frame so the point under
the pointer (or the reading line for buttons) stays at the same screen position, within a couple of
CSS pixels. The visual scale and the scale used for the anchor calculation are kept in lockstep;
a stale CSS scale would amplify a sub-pixel error by the document offset. Scroll offsets are applied
once per frame. The page indicator is
computed from the fixed base metrics and the reading line, so it does not flicker as pages are
scaled.

When the scaled column is narrower than the stage, `scrollLeft` is zero and the column is centred;
when it is wider, scroll offsets are clamped. This is the "page became too small" case: no gesture
can leave a 50 % page stuck against the left edge. Vertical position keeps the reader's line; if the
entire document is shorter than the stage at rest, it is centred vertically.

### Rendering at rest

Display scale and render scale are separate. The old canvas is the preview. After 120 ms without
input (or after a button animation ends), the renderer paints the visible pages (±1) once at the
current display scale and swaps each canvas when its new bitmap is ready. A stale render is cancelled
or discarded; a page never has more than one render in flight. If the scale changes again before the
swap, the old canvas stays and the new bitmap is thrown away.

The canvas backing store is capped (max area and dimension); above the cap the preview is allowed to
be softer rather than allocating an unbounded bitmap. Text is a separate transparent layer at base
layout, so it never becomes a bitmap and never waits for the canvas.

### States

| State | Trigger | Visible content | Input | Leaves when |
|---|---|---|---|---|
| Idle | no gesture, render finished | sharp canvas + text | any zoom input | zoom starts |
| Zooming | wheel / gesture / ease | old canvas scaled, text visible | continuous updates | input stops and scale reaches target |
| Settling | 120 ms quiet | old canvas scaled, text visible | new input cancels settle | render starts |
| Rendering | settle elapsed | old canvas scaled, text visible | new input marks render stale | new canvas swapped |

Selection, copy, capture, tints and citation jumps remain available; a zoom-in-progress does not
clear a selection. Starting box capture during a gesture cancels the gesture, as scrolling already
does.

## Data model

No persisted data. The renderer owns:

```text
baseWidth, baseHeight          measured at 100 % for the stage
scale { display, target }      continuous, clamped 50–400 %
gesture { startScale, origin } only while a WebKit gesture is active
renderScale per page            the scale of the current canvas backing store
```

The header keeps a display-only `zoom_percent` reported by the renderer. The Rust `zoom` signal no
longer drives page layout.

## Interface

```js
PDFScope.zoomTo(percent, { origin, animate }): void
PDFScope.zoomBy(steps, { origin }): void
PDFScope.fitWidth(): void
PDFScope.render(page): Promise<void>          // scroll page into view, paint its window
window.__pdfscopeZoom({ percent, settled })   // renderer -> header
```

`pageSizes`, `pageText`, `flash`, `beginCapture`, `disarmCapture`, `clearSelection` keep their
current meanings. `render(page, zoom)` loses the zoom argument; zoom input goes through the
functions above so the viewer can coalesce it.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Trackpad pinch | — | reading surface | a document is open | two-finger pinch | — | — | continuous scale around the fingers; text stays visible; sharpens after release | browser page zoom is prevented; no document change |
| Trackpad pan | — | reading surface | a document is open | two-finger drag | — | — | native scroll, including momentum; can coincide with pinch | native bounds |
| Mouse zoom | — | reading surface | a document is open | `ctrl`/`⌘` + wheel | `+` `-` `0` | — | eased zoom around the pointer; one re-render after the gesture | clamped 50–400 % |
| Zoom out/in | `−` `+` | header, right | 50 < zoom < 400 | click | `-` `+` | tap | 25 % step animated over ~160 ms around the reading line; text never disappears | buttons disable at the limits |
| Fit width | `适应宽度` | header, right | a document is open | click | `0` | tap | eases to 100 %, re-centres horizontally | — |
| Recentre | — | reading surface | content narrower than stage | after zoom-out | — | — | scrollLeft 0, column centred; no snap past bounds | — |

**Alternatives considered.**

- **Re-render on every animation frame.** Sharpest possible, but the 400-page budget and the current
  jank make it a non-starter. Rejected.
- **Scale the canvas forever without re-rendering (CSS-only zoom).** Smooth but permanently soft at
  high zoom. Rejected as the resting state; accepted only as the gesture preview.
- **Adopt PDF.js `drawingDelay`/`cssTransform`.** It already delays rendering and previews with CSS,
  but it hides the text layer while a resized render is pending. The reader explicitly asked for
  text to stay visible. We keep its two good ideas (delay, CSS preview), not its hide-text choice.
- **Transform each page separately.** More layers to manage and the same visual result; a single
  column transform keeps the gap and page shadows consistent and makes the marquee/selection maths
  one scale. Rejected.
- **Interpolate after the pinch (ease the final scale).** Feels laggy under the fingers. Direct
  updates for pinch; easing only for discrete mouse/button input.
- **Snap to 10 % steps.** The current behavior; it is the complaint. Rejected.

**Discoverability.** Pinch and `ctrl`+wheel are standard reading gestures; the header buttons remain
the discoverable path. No new control is introduced.

**Cost of being wrong.** A badly anchored zoom loses the paragraph the reader was looking at; the
reading line is the fallback, and `适应宽度` recovers. A stale preview never changes page, selection or
references; it is only pixels. No zoom action is destructive.

## Test plan

- **Headless wheel stream:** dispatch 40 `ctrl`+wheel events at ~60 Hz over a paragraph; sample
  `--view-scale` and scroll each frame. Assert the scale changes continuously, the paragraph point
  moves < 2 CSS px, and no frame has a layout wider than the stage without a matching scroll update.
- **Text stays:** record the text layer node and span count before a gesture; assert the same node is
  present with the same spans at every sample and after settling. The canvas node may only change
  after the quiet period.
- **No double paint on buttons:** click `+`, sample five intermediate scales before the final one,
  and assert the target page's scroll anchor moves monotonically (no initial layout jump).
- **Render debounce:** 30 scale changes produce one canvas swap per visible page after the quiet
  period; while the hand moves, the old canvas remains.
- **Recentre:** zoom to 50 % with a non-zero `scrollLeft`; assert `scrollLeft === 0` and the page is
  centred within one CSS pixel.
- **400 pages:** the same stream on the 400-page fixture; only the visible window (±1, at most five
  canvases in this test), frame durations sampled, last page reachable. A real 60 fps check on the
  M-series GPU remains part of the manual Tauri pass.
- **Regression:** the 0015 chains (selection, box capture, tints, `[p.N]`) and all existing
  `reader.mjs` checks still pass; the suite grew from 49 to 81 assertions.
- **Manual (Tauri macOS):** trackpad pinch, pinch+two-finger pan, mouse wheel, buttons; repeat on a
  400-page document. Screenshots mid-gesture and after settle.

## Slices

1. Fixed base layout + scaler + transform; buttons use the JS zoom controller; no render during the
   gesture. Verify 0015 chains and virtualization still pass.
2. Continuous input: `gesture*`, `ctrl`+wheel, easing, pointer anchoring, pan, live label.
3. Rest render: deferred canvas re-render, atomic swap, canvas cap, recentre.
4. Performance and regression: 400 pages, frame sampling, screenshots, manual Tauri pass.

## Risks

- **WKWebView gesture events are non-standard.** Verified support is manual; the `ctrl`+wheel path
  remains the fallback. If WebKit does not deliver gestures, pinch is still handled where the webview
  emits `ctrl`+wheel.
- **Transform + browser selection.** The text layer is now scaled rather than laid out at display
  size. Normalised coordinates cancel the scale, but this needs a real selection test at 50 %, 100 %
  and 300 %.
- **Scrollbar and clamping drift during repeated fractional updates.** Keep fractional scroll
  positions as PDF.js's `panBy` does; test long gesture streams.
- **Transform layer memory.** A 400-page column is a large compositing layer. The scaler keeps the
  layout cheap; if GPU memory becomes a problem, the fallback is per-page transforms, not a return to
  reflow.
- **Canvas cap.** Above the cap the resting canvas is softer. This is a deliberate trade for memory
  and must be visible in the manual notes.
- **Two-finger pan + pinch on macOS.** Native pan and gesture zoom may interleave; the final position
  is clamped and re-centred. Exact simultaneous centroid tracking is not promised if the platform
  does not expose it.

## Assumptions

- WKWebView emits `gesturestart`/`gesturechange`/`gestureend` for trackpad pinch, as it does in other
  Tauri apps; Chromium emits `ctrl`+wheel. Both paths are feature-detected.
- A CSS transform on the column can be composited without reflowing the 400 page shells.
- Rendering the text layer at base layout and transforming it is visually identical to rendering it
  at the display scale for selection and tint geometry.

## Resolved questions

1. **Tauri or Chrome?** Both input paths are implemented: WebKit `gesture*` and Chromium
   `ctrl`+wheel. Headless tests cover both synthetic paths; the real WKWebView trackpad pass is the
   remaining manual check.
2. **Transient softness?** Accepted: the old canvas scales during the gesture and is replaced by a
   sharp render after the quiet period. Text is a separate layer and is never a bitmap.
3. **Maximum zoom?** 400 %, with a 32 MP canvas cap; above the cap the resting page is softer rather
   than unbounded in memory.

## Deviations

- Zoom commands now live in the renderer; Rust keeps a display-only percent. The page field, page
  indicator, selection and capture remain application state.
- The base width is rounded to an integer. A scrollbar appearing changed `clientWidth` by a fraction
  of a pixel; over a 400-page column that moved page 18 by several pixels during a zoom. Rounding
  removes the reflow.
- The canvas backing store is capped at 32 MP and 32767 px per side. Above the cap the resting bitmap
  is softer rather than larger; the text layer is unaffected.
- The selection bar is counter-scaled through a zero-size anchor, so its type stays readable at
  400 %. The marquee is drawn in the page's untransformed base coordinates.
- Starting box capture during a live zoom cancels the zoom before arming, as the interaction table
  requires.
- `beginCapture`/wheel filtering follows the platform: WebKit `gesture*` events, Chromium
  `ctrl`+wheel, and a physical Ctrl/Meta wheel is eased. Horizontal wheel deltas are applied as pan;
  native two-finger pan is not intercepted.
- Test result: 81 assertions pass on the 40-page fixture and on the 400-page fixture. The wheel
  stream's pointer error stayed below 1.1 CSS px; the text layer node identity and span count were
  unchanged through every sampled frame; the canvas was replaced once after the quiet period. The
  button zoom produced 16 sampled frames before settling. On the 400-page fixture the stream's
  slowest sampled frame was 17.5 ms (13.3 ms average). `just check` passes.
- The Tauri macOS trackpad check (pinch + two-finger pan in the real WKWebView) is still a manual
  step; headless Chrome cannot deliver native WebKit gesture traffic. The ctrl+wheel and synthetic
  gesture paths were exercised.
