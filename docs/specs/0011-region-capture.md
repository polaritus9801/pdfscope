# 0011 — Region capture: re-render, do not screenshot

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("可以，做")

## Goal

Dragging a box over a page produces the clearest image the source can give — independent of the
window size, the zoom level, and the display's pixel density — and it appears in the reference tray
as a thumbnail. No OCR is involved: a region is an image, and that is the point.

## Non-goals

- No OCR of the region. A scanned page is exactly what this path is for.
- No sending the image to a model yet; the provider layer takes text only until 0013.
- No annotation, no persistence between launches.
- No `⌥`-drag on the text layer: capture is armed explicitly, then dragged.

## Behaviour

- A `框选` button in the header arms capture. The page shows a crosshair, the text layer stops
  receiving pointer events, and a hint appears: `拖出一个矩形，Esc 取消`.
- Dragging draws a marquee; releasing captures, disarms, and adds a region chip.
- `Esc` disarms without capturing; a click without a drag (under 4 px) is not a capture.
- The capture is rendered **from the PDF**, not cropped from the screen: the page is re-rendered at
  the scale needed for the crop's long side to reach 1600 px, then the rectangle is cut out and
  encoded as PNG.
- The chip shows a 40 px thumbnail, the page number, and `✕`.
- Zooming or resizing never changes a region that was already captured.

## Data model

```rust
pub struct Region {
    pub page: u32,
    pub rect: (f64, f64, f64, f64), // normalised to the page
    pub image: String,              // data URL, PNG
}
```

Kept in memory for the session; the tray shows it beside passages.

## Interface

The renderer gains `beginCapture()` and reports:

```js
window.__pdfscopeRegion({ page, x, y, width, height, image })
```

`image` is a `data:image/png;base64,…` string. The Rust side rejects anything without an `image`.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Arm capture | `框选` | header, beside Open | a document is open | click | `⌥⇧C` | tap | crosshair; text layer inert; hint shown | `Esc` or a second click disarms |
| Marquee | — | page | capture armed | drag | — | drag | a dashed rectangle follows the pointer | releasing under 4 px cancels |
| Region chip | thumbnail + `p.31` | tray | after a capture | `✕` removes | `Delete` | tap | chip disappears; image dropped | capture failure shows `这一块没截下来` in the hint |

**Alternatives considered.**

- Cropping the on-screen canvas (the previous plan). Rejected: resolution is hostage to the current
  zoom and to `devicePixelRatio`, and a 100% view produces a soft image.
- Sending the whole page and letting the model crop. Rejected: wastes tokens and loses locality.
- JPEG for everything. Rejected: text and line art are exactly the content that JPEG ruins; PNG is
  used for all captures, and the size is small because the crop is small.

**Discoverability.** The button is visible whenever a document is open; the hint names the gesture
and the escape hatch.

**States.** Idle → armed (crosshair, text inert) → dragging → captured (chip) → idle. `Esc` from any
state returns to idle without side effects.

**Cost of being wrong.** A bad crop is visible in the chip immediately and removable; nothing is sent.

## Test plan

- Unit: the scale function returns 1600 px for narrow crops, caps at 4× for tiny ones, and never
  drops below 1×.
- Manual: capture at 50% and at 300% zoom, compare the two images — both must be sharp; capture a
  scanned page and confirm the thumbnail is legible; `Esc` mid-drag leaves no chip.

## Risks

- **Re-rendering a page for every capture** costs one render pass (tens of milliseconds). Acceptable.
- **Very large crops** produce multi-megabyte data URLs. Mitigation: the long side is capped at 1600
  px, so a full-page capture is bounded.
- **Pointer events on the text layer** must be disabled while armed, or the marquee will select text
  instead of drawing.

## Assumptions

- PDF.js can render the same page twice concurrently if the first render is awaited per call; captures
  are user-paced, so contention is unlikely.

## Deviations

- The `⌥⇧C` shortcut was not implemented; the button and `Esc` are. The shortcut needs a keyboard
  handler that does not collide with the page's own shortcuts.
- The region's text caption (open question 1) is deferred to the multimodal request.
- The marquee is drawn inside the page element and positioned from the canvas rectangle, so it stays
  aligned at any zoom without extra maths.

## Open questions

1. Should the chip keep the region's text (from the text layer) as a caption? Proposed: later, with
   the multimodal request.
