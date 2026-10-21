# 0012 — Page ranges and on-demand OCR

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("继续")

## Goal

A page-range reference can include pages that have no text layer, and the reader decides per
reference whether that is worth the time: a checkbox, an engine, an estimate. The application ships
with **no OCR engine inside it**; it uses the operating system's, or downloads a small one on demand.

## Non-goals

- No OCR of regions (0011: a region is an image).
- No OCR of selections (if text can be selected, there is text).
- No whole-book OCR by default, and no background OCR of pages the reader has not asked about.
- No bundled binary engine: it would add tens of megabytes and a notarisation burden.
- No page classification heuristics. The checkbox is the signal; the heuristic from the design
  discussion is not part of the product.

## Behaviour

- Typing a range accepts `31`, `31-45`, `31-45, 60`, full-width punctuation, and whitespace.
  Overlapping spans merge; spans outside the document are rejected with a message naming the page.
- The range editor shows the computed summary: `12 页有文字 · 3 页是扫描件`.
- The `识别扫描页` checkbox is per reference, off unless the reader turns it on.
- Default: checked on first use **only if a system OCR engine is available**; afterwards the
  reader's last choice is remembered.
- When unchecked, pages without text are listed, not silently dropped:
  `第 42–44 页是扫描件，未包含`.
- Engine choice: `系统原生` (default, zero download), `轻量引擎` (first use downloads about 6 MB),
  `模型` (per page, priced, explicit).
- OCR runs when the reference is added, with a progress row and a cancel action; Send is disabled
  until it finishes or the reader chooses to skip.

## Data model

```rust
pub struct PageSpan { pub start: u32, pub end: u32 }   // one-based, inclusive

pub fn parse_ranges(input: &str, page_count: u32) -> Result<Vec<PageSpan>, RangeError>;

pub struct PageRangeReference {
    pub spans: Vec<PageSpan>,
    pub ocr: bool,
    pub engine: OcrEngine,
}

pub enum OcrEngine { System, Light, Model }
```

## Interface

- `ocr_engines() -> { system: bool, light: bool, model: bool }` — availability, no side effects.
- `ocr_pages(pages: Vec<u32>) -> Vec<PageText>` — streams progress per page.
- The renderer supplies the page image as PNG; the engine never reads the PDF itself.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Range field | `31–45` | range editor | a document is open | type | `↵` commits | type | spans update; invalid input marks the field only | out-of-range names the page |
| Summary | `12 页有文字 · 3 页是扫描件` | under the field | spans valid | — | — | — | recomputed on every change | — |
| OCR toggle | `识别扫描页` | under the summary | the span contains a page without text | click | `Space` | tap | adds the pages to the reference; shows an estimate | no engine available → disabled with `未检测到引擎` |
| Engine select | `系统原生 ▾` | beside the toggle | the toggle is on | choose | arrows | tap | switching to `轻量引擎` starts a one-time download | download fails → falls back to system, message names the cause |
| Progress | `识别中 3/12` | in the tray chip | OCR running | — | — | — | counter updates; `取消` stops | cancel keeps finished pages |
| Skip | `跳过扫描页` | beside progress | OCR running or failed | click | — | tap | the reference is added without those pages | the prompt lists the skipped pages |

**Alternatives considered.**

- A global toolbar toggle. Rejected: whether OCR is worth it depends on the reference, not the app.
- Detecting scans automatically and OCR-ing without asking. Rejected: the reader pays in time and
  never consented.
- Bundling an engine. Rejected: install size, notarisation, and an update path for the model.
- Comparing engines on one page to choose a default. Rejected for v1: it delays the first action, and
  availability already answers the question well enough.

**Discoverability.** The summary line appears as soon as a range contains a scan, so the checkbox is
never a surprise.

**States.** No scans in range (toggle hidden) → scans present (toggle shown, default from
capability) → OCR running (progress, Send disabled) → done / skipped / failed.

**Cost of being wrong.** OCR spends seconds per page; the estimate is shown before it starts, and
cancel keeps what finished. A skipped page is named, so a wrong answer can be traced to a missing
page rather than to a bad model.

## Test plan

- Unit: `parse_ranges` handles `31`, `31-45`, `31-45, 60`, `31–45` (en dash), `31，45` (full-width
  comma), whitespace, reversed spans, out-of-range, empty; overlapping spans merge.
- Unit: the summary counts match the spans against a page-kind map.
- Manual: a range with one scan and OCR off shows the skipped-page note; with OCR on it runs and the
  answer cites the recognised page.

## Risks

- **OCR quality varies by engine.** Mitigation: the engine is named in the reference, and a low-
  confidence page offers `用模型重读这一页`.
- **A download can fail behind a proxy.** Mitigation: fall back to the system engine and say so.
- **Long ranges block Send.** Mitigation: the estimate is shown first and skipping is always allowed.

## Assumptions

- The system engine is available on macOS and Windows; on Linux it is not, so `轻量引擎` is the only
  offline option there.

## Deviations

- `PageSpan::len` was renamed to `PageSpan::pages`: clippy is right that a `len` without an
  `is_empty` is a confusing pair, and a span of pages has no meaningful empty state.
- The pure range parser landed first, as its own commit, so the OCR binding can be verified on its
  own rather than tangled with parsing.

## Open questions

1. Should the estimate use a fixed seconds-per-page figure or the last measured one? Proposed: the
   measured one once a page has been recognised, the fixed one before.
