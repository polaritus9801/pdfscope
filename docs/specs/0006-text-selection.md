# 0006 — Text layer and selection

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("你接着做，我想等你做完再去调整")

## Goal

Text on the page is selectable like text anywhere else: dragging across it highlights the words,
and releasing produces a small bar above the selection that offers to add it to the reference tray.
This is the first half of 0003 — the tray exists, but it only accepts passages.

## Non-goals

- No AI. Nothing is sent anywhere.
- No OCR: a page without a text layer selects nothing, and says so.
- No cross-page selection; a drag that leaves the page ends at the page edge.
- No history, no regions, no page ranges. Those are 0007 and 0008.
- No core anchoring yet: the tray holds a provisional passage (page, rect, text), because the
  per-run rectangles that `pdfscope-core::anchor` needs are the next slice's job.

## Behaviour

- The text layer sits exactly over the canvas and is invisible except for its selection highlight.
- Dragging selects text; releasing shows the bar after a 120 ms settle, so a slow drag does not flash
  it.
- The bar shows the page number and the first line of the quote, and two actions: `加入引用` and `复制`.
- `加入引用` appends a chip to the tray, clears the browser selection and dismisses the bar.
- `Esc`, a click elsewhere, or a page change dismisses the bar and clears the selection.
- The tray lists chips in order; `✕` removes one; the tray is empty by default and collapses when it
  has no items.
- A page with no text layer renders normally but shows a hint on the first attempted selection:
  `这一页是扫描件，暂时只能看`.

## Data model

```rust
pub struct Selection {
    pub page: PageIndex,
    pub rect: Rect,   // normalised to the page
    pub text: String, // whitespace-normalised by the UI before it crosses the boundary
}

pub struct Passage {
    pub page: PageIndex,
    pub rect: Rect,
    pub quote: String,
}
```

The tray is `Vec<Passage>` for this slice. It will become `Vec<Reference>` in 0007 without changing
the storage of a passage.

## Interface

The renderer gains one outbound callback and one state reset:

```js
window.__pdfscopeSelection({ page, x, y, width, height, text })  // called on selection release
```

Coordinates are normalised to the rendered page (0…1), so zoom and window size never enter the
model. The Rust side registers the callback once and ignores malformed values.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Selection | — | page | the page has a text layer | drag across text | `Shift`+arrows extend | long-press, then handles | words highlight in `--accent-tint` at 45%; on release the bar appears | clicking empty space or `Esc` clears it |
| Selection bar | page + quote | above the selection, inside the page | a selection exists | click an action | `Tab` then `↵` | tap | `加入引用` adds a chip; `复制` writes the quote | the bar never covers the selected line; it flips below if there is no room above |
| Add reference | `加入引用` | selection bar | a selection exists | click | `⌘⇧A` | tap | a chip appears in the tray; the bar closes | failure leaves the selection intact |
| Copy | `复制` | selection bar | a selection exists | click | `⌘C` copies plain text directly | tap | clipboard holds the quote; a toast confirms | clipboard denied → toast with the reason |
| Tray chip | `p.31 · 这段文字…` | under the header | the tray is non-empty | click the chip to jump to its page; `✕` removes | `Tab`, `Delete` | tap removes | the chip disappears; the page does not change | removing the last chip collapses the tray |
| Scanned page hint | `这一页是扫描件…` | where the bar would be | a drag selected nothing on a page without text | — | — | — | the hint appears once per page | any click dismisses it |

**Alternatives considered.**

- Selecting over the canvas with a custom hit-test. Rejected: the browser's own selection already
  handles double-click words, shift-extension, and accessibility; reimplementing it would be worse in
  every respect.
- Rendering the text layer only when the user asks. Rejected: selection must feel native, and the
  layer costs one extra render pass.

**Discoverability.** Selection is the most familiar interaction on a page; nothing needs teaching.
The bar appears exactly where the reader just acted.

**States.**

| State | Page drag | Selection bar | Tray |
|---|---|---|---|
| No selection | starts a selection | hidden | unchanged |
| Selected | replaces the selection | visible above | unchanged |
| Added | cleared | hidden | one more chip |

**Cost of being wrong.** A selection that does not match what the reader sees would poison every
later citation. Mitigation: the bar echoes the first line of the captured text, so a mismatch is
visible before it is added.

## Design review

- Reviewed with: z1mLi on 2026-10-21 (the bar and tray were described in the conversation)
- Decisions: the bar has exactly two actions; the tray is a single row for now; the provisional
  passage is acceptable because anchoring lands in the next slice.

## Test plan

- Unit (Rust): a malformed selection payload is ignored; a normalised rect outside 0…1 is rejected;
  adding the same quote twice keeps two chips (they are different places).
- Integration: after `加入引用`, the chip shows the right page; removing it leaves the page alone.
- Headless: with a 26-page text PDF, programmatically selecting the first text run produces a bar
  with the right page number and quote.
- Manual: drag across a line, release, check the bar does not cover the selection; select on a
  scanned page and see the hint; zoom to 300% and confirm the highlight follows the glyphs.

## Risks

- **The text layer drifts from the canvas** at non-integer scales, making highlights land beside the
  words. Mitigation: both are positioned from the same viewport, and the scale factor is set on the
  layer, not estimated.
- **CSS from PDF.js is required**, not optional: the layer is unpositioned without it. The minimal
  rules are vendored into `styles.css` with a comment pointing at their origin.
- **Selection across lines** produces multiple client rects; the bar anchors to their union, which can
  be wide. Accepted for now.

## Assumptions

- PDF.js `TextLayer` is available in the vendored build; verified by its export list.
- The browser selection API behaves the same in the Tauri webview on all three platforms; unverified
  on Windows and Linux.

## Deviations

- The scanned-page hint is drawn by CSS on the text layer (`::after`) rather than by the Rust side:
  Rust never learns whether a page had text, and inventing a second source of truth for that would
  be worse than a pseudo-element.
- The tray sits under the header, answering open question 1.
- Selection rectangles are the union of the browser's client rects, so a multi-line selection produces
  one tall rectangle rather than one per line. The bar anchors to its top edge.
- `复制` uses `navigator.clipboard`, which needs a secure context. The desktop webview provides one;
  a plain `http://` development server may not, and the button then does nothing.

## Open questions

1. Should the tray sit under the header or float above the page bottom? Proposed: under the header,
   because a floating tray competes with the selection bar for the same space.
