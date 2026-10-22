# 0021 — Page modes: single, spread and continuous

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-22
- **Approved by:** z1mLi ("继续", 2026-10-22)

## Goal

The reader can show one page at a time, or a facing spread, instead of only the continuous column.
Switching keeps the page the reader is on, selection and box capture still resolve to the right page,
and citations still land on the visible page.

This spec exists because a CSS-only attempt inside 0020 failed: hiding the other pages left the page
off the top of the viewport, since scroll position and metrics belong to the continuous layout. The
feature needs its own layout model, described here.

## Non-goals

- Reflowing text, or fitting two pages into one column by scaling text.
- Editing page order, rotating individual pages differently from the document.
- Changing the continuous layout; it stays the default and the reference implementation.

## Behaviour

- Three modes: **continuous** (today), **single**, **spread** (two facing pages).
- Switching modes keeps the current page. In spread, the current page is the left page when it is
  even, the right page when it is odd, with a control to shift the pairing by one.
- In single and spread, the whole page fits the viewport at `fit page`; zoom still works and pans.
- Paging: `PageUp`/`PageDown`, `←`/`→`, wheel with no modifier, and the existing progress bar and
  page field all move by one page (or one spread).
- Selection, box capture, reference tints and `[p.N]` jumps operate on the visible page(s) exactly as
  in continuous mode.
- Opening a document restores the mode last used for that document, when "remember position" is on.
- Errors: a spread at the last page shows the single remaining page centred, never a blank half.

## Layout model (decided)

The page box keeps its own geometry; the **layer** inside it (canvas + text layer) is scaled as one
unit, and the box is resized to the scaled size. This is the only arrangement where selection and
capture stay aligned: text layer and canvas scale together, and rectangles normalised against the
page box remain correct.

- `single`: scale `s = min(1, (stageHeight − 48) / pageHeight)`.
- `spread`: scale `s = min((stageWidth − 48 − gap) / (2 × pageWidth), (stageHeight − 48) / pageHeight)`.
- Pairing: page 1 alone; page `n` even pairs with `n + 1`; page `n` odd (n > 1) pairs with `n − 1`;
  a trailing unpaired page is centred.
- The zoom transform is not used for paging; the zoom control only affects the continuous column, and
  is reported as `fit page` while in paged modes.

## Data model

`settings.json` gains `page_mode: "continuous" | "single" | "spread"` (already scaffolded in 0020 for
continuous/single) and `spread_offset: bool`, both additive with defaults.

Per-document memory reuses the existing `pdfscope:pos:<name>` entry, extended from a number to
`{ page, mode, zoom }` with a fallback that reads the old number.

## Interface

```
PDFScope.setPageMode(mode)     // "continuous" | "single" | "spread"
PDFScope.pageMode()            // current mode
PDFScope.setSpreadOffset(bool) // shift the pairing by one
```

No new Tauri commands; the viewer owns the layout, Rust owns the page number.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Mode switch | 连续 / 单页 / 双页 | settings · 阅读, and a header segmented control | a document is open | click segment | `⌘1` `⌘2` `⌘3` | tap | layout changes, current page kept | switching back restores the previous page |
| Pairing shift | 奇偶偏移 | header, spread only | spread mode | click | `⌘⇧O` | tap | spread pairs shift by one page | click again returns |
| Next page | › | header and progress bar | not the last page | click | `PageDown`, `→`, wheel | swipe | next page or spread | at the end, disabled |
| Previous page | ‹ | header and progress bar | not the first page | click | `PageUp`, `←`, wheel | swipe | previous page or spread | at the start, disabled |

**Alternatives considered.**

- Chosen: a layout model inside the viewer, with the modes sharing the same page elements.
- Rejected: CSS `display: none` on hidden pages — the failed attempt; scroll and metrics stay bound to
  the continuous layout, so the page ends up off-screen.
- Rejected: rendering only the visible page and destroying the others — it would break the render
  window and make mode switching flash.

**Discoverability.** A segmented control in the header shows all three modes at once; the setting and
the control stay in sync, and the shortcut appears in the 快捷键 tab.

**States.**

| Mode | Page elements | Scroll | Capture/selection target |
|---|---|---|---|
| continuous | all, render window | vertical, free | page under the pointer |
| single | all, one visible | none; wheel pages | the visible page |
| spread | all, two visible | none; wheel pages | either visible page |

**Cost of being wrong.** Mode switching never edits the document; the worst case is a wrong page in
the viewport, recoverable with the page field. Selection is cleared on every mode change, so a stale
selection cannot point at a hidden page.

## Design review

- Reviewed with: — (pending)
- Decisions: —

## Test plan

- Unit: `spread_offset` pairing maths for odd/even and first/last page.
- Integration: extend `scripts/ui-smoke/reader.mjs` with a mode section — switch to single, assert one
  visible page and that its top is at the viewport top; switch to spread, assert two adjacent pages;
  select text and box-capture in both; `⌘↓` in spread lands on the last page.
- End-to-end: screenshot matrix gains a spread column.

## Risks

- **The render window assumes a column.** In single/spread the window is one or two pages; the
  IntersectionObserver must be replaced by an explicit "render these" path or it will thrash.
- **Zoom anchoring** is written for a scrolling column; in paged modes the anchor is the viewport
  centre. Reusing it blindly will drift.
- **Selection across a spread** spans two pages; restrict selection to one page and say so.

## Assumptions

- Most readers use continuous; single/spread are for projection and for reading two-page scans.
- `fit page` is the sensible default for paged modes; `fit width` remains for continuous.

## Progress

- **Single mode** implemented: `PDFScope.setPageMode("single")` hides the other sheets, collapses the
  continuous transform, returns the scroll to zero, and marks the current page. The 阅读 setting
  翻页方式 persists the choice. Measured: page top 80 px (stage top 56 + padding 24), scroll 0, one
  page visible.
- **Single mode fits the page**: entering it computes a fit-page ratio and drives the existing zoom
  transform, anchored at the top. Measured with the sample on an 800 px window: page 609 px tall in a
  657 px stage, 54 %, scroll 0, top 80.
- **Single mode is covered by the smoke suite**: it switches, asserts exactly one page is visible and
  that its top sits within the reading area, then switches back and asserts the column returns. The
  suite is now **83 checks**.
- **Spread implemented**: page 1 alone, then (2, 3), (4, 5)…; at page 2 the pair renders side by side
  (388 × 549 each in an 800 px stage). Both sheets render through an explicit window: in paged modes
  the scroll no longer decides the page, so entering the mode cannot jump to the last page.
- **Coverage**: the smoke suite switches to single (one page at the top), back to continuous, then to
  spread at page 2 and asserts pages 2 and 3 sit side by side. **84 checks**.

## Deviations

- **Spread was attempted and reverted.** The layer-scaling model above works for a single sheet and
  centred page 1 correctly (392 × 555 in a spread row), but it conflicted with the zoom-transform
  fit-page that single mode already uses: with the column transform removed, single mode stopped
  fitting (800 × 1132 instead of ~464 × 657). Two mechanisms cannot own the same page. The code was
  reverted; single mode stays on its verified path.
- **Next step is unification, not another attempt**: move single mode onto the layer-scaling model
  (fit is computed, not via the zoom transform), then spread is the same code with a pair window. The
  smoke suite already covers single, so the migration is checkable.


- Both modes now scale the page **layer** (canvas + text layer together) and resize the box to match;
  the continuous zoom transform is not used while paged. The zoom readout still shows the column's
  zoom, not the paged fit ratio.
- Paging is by page, not by spread: the pair window changes when the current page crosses a pair
  boundary. The pairing offset control from the design is not built.
- Mode changes reset the scroll; a finer anchor (keep the same paragraph) is not implemented.

## Open questions

1. Does the spread need a cover-page rule (first page alone), as most readers do?
2. Should paging in single mode animate (a slide) or cut?
3. Is a header segmented control worth the space, or is the settings switch enough for v1?
