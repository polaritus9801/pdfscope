# 0003 — Pointing the AI at content: the three scopes

- **Status:** draft
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** —

## Goal

A reader can indicate what a question is about in three ways — a passage of text, a rectangle of the
page, or a range of pages — without learning a mode system, without losing their place, and without
accidentally sending something to a paid model that they did not mean to send.

## Non-goals

- No answering or citation rendering. This spec ends where the request begins; 0004 covers the
  answer, its citation, and the copy block.
- No persistent highlights or annotations.
- No OCR: a scanned page is captured as an image, not read as text (that is 0005).
- No cross-page text selection. One selection lives on one page.
- No keyboard-only mode switcher beyond what is listed below.

## Behaviour

Three scopes exist. Exactly one is active at a time, and it is **always visible** in the AI panel:

| Scope | How it is set | What is sent |
|---|---|---|
| **Selected text** | drag over text, or arrow-select | The quote plus the paragraph around it |
| **This page** | click the page chip (the default) | The page's text, or its image if it has no text layer |
| **Page range** | drag on the page strip, type in the range field, or click the page indicator | The listed page segments, labelled per page |

Asking is always explicit: nothing is sent on selection, capture, or range change. The user presses
Send (or `⌘↵`). A capture is shown as a removable chip before it can be sent.

## Data model

No library schema change. The scope is request state:

```rust
pub enum AskScope {
    Selection { page: PageIndex, anchor: Anchor },
    Page { page: PageIndex },
    Range { pages: Vec<PageSpan> },
}
```

`PageSpan` is a contiguous inclusive run (`start <= end`). Validation: every span is ordered and
inside the document, spans are normalised (sorted, adjacent and overlapping spans merged), and an
empty or whitespace-only selection is refused at capture, not at send.

The multi-segment shape exists from the start so that typing `31-45, 60` is supported without a
schema change later. The v1 drag gesture produces a single span.

## Interface

- `POST /api/v1/ask` with `{ scope, question }` — the same body from both shells.
- Events: `scope` (echo of what was understood), `delta`, `citation`, `done`, `error`.
- Errors: `ScopeEmpty`, `RangeInvalid`, `RangeTooLarge`, `ModelUnavailable`, `Cancelled`.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Text selection | — | reading surface | page has a text layer | drag across glyphs, release | `Shift`+arrows extends | long-press, then drag handles | floating bar appears above the selection; scope chip reads `选中 · p.31` | clicking empty page area dismisses; nothing is sent |
| Ask | `问` | floating bar above selection | a selection exists | click | `⌘↵` | tap | AI panel focuses its input with the quote attached as a chip | `Esc` closes the bar; the selection remains |
| Capture area | `框选` | floating bar, and page surface | any page, including scans | click then drag; or hold `⌥` and drag directly | hold `⌥`, drag with arrows not supported in v1 | tap `框选`, then drag | a marquee is drawn; on release an image chip appears in the panel with a preview and `✕` | `Esc` before release cancels; the chip can be removed before sending |
| Page strip | page ticks | under the page, above the AI input | a document is open | drag across ticks selects a span; drag an edge to resize | `Shift`+arrows extend the span; `⌘G` focuses it | drag handles on the strip | the span highlights live and the range field rewrites on every tick crossed | `Esc` restores the previous span; a span above 20 pages shows an estimate and requires confirmation |
| Range field | `31–45` | AI panel, beside the strip | a document is open | click to edit | type freely; `↵` commits | tap to edit | a valid entry rewrites the strip and the scope chip immediately | invalid input (`from` > `to`, outside the document) marks the field only, leaves the strip and chip untouched, and disables Send |
| Page indicator | `31 / 404` | toolbar, right of the navigation | a document is open | click opens the range editor with the field focused | `⌘G` | tap | the editor opens; nothing changes until an edit is committed | closing the editor without a valid entry changes nothing |
| Scope chips | `选中` `本页` `范围` | AI panel header | panel open | click | `Tab` to move, `↵` to choose | tap | the next question uses that scope; a typed question is preserved | switching scope never clears the input |
| Copy quote | `复制` | floating bar | a selection exists | click | `⌘C` copies plain text; the button copies the Markdown block | tap | clipboard holds `> quote` + `p.31`; a toast confirms | clipboard denied → toast with the reason |

**Alternatives considered.**

- **A — three permanent toolbar buttons** (a mode bar with 选中 / 框选 / 范围). Obvious, but it puts a
  mode system on the reading surface, and two of the three modes are rare. It also introduces a
  classic failure: the user is in box mode, drags across text, and gets a screenshot of text.
- **B — gestures only** (no buttons at all). Clean, but undiscoverable, and a modifier key does not
  exist on touch.
- **C — chosen: scopes in the panel, gestures on the page.** The three "buttons" exist, but as
  *what the next question is about*, not as modes the page obeys. The panel is always visible, so
  the current scope is never a hidden state. The floating bar teaches the box gesture at the moment
  a user first selects text.

**Discoverability.** The page indicator is styled as an interactive control (`31 / 404` with a
chevron) rather than as a label. The floating bar shows `框选` on the first text selection, which is
where a user is already looking. The panel shows all three scope chips even when only one applies,
so the model of "three scopes" is visible before it is needed.

**States.**

| State | Drag on page | Floating bar | Send |
|---|---|---|---|
| Idle | starts text selection | hidden | sends current page |
| Text selected | replaces the selection | visible | sends the selection |
| Box armed | draws a marquee | hidden | sends the captured image |
| Range set | still selects text | visible | sends the range |
| Streaming | selection allowed | visible | disabled, shows Stop |

**Cost of being wrong.** A mis-drawn box sends the wrong region to a paid model; mitigated by the
preview chip and by never sending on capture. A mis-set range can send hundreds of pages; mitigated
by the estimate and the confirmation above 20 pages. A mis-set scope is recoverable until Send
because the chip states exactly what will go.

## Design review

- Reviewed with: z1mLi on 2026-10-21
- Decisions:
  - Box capture ships with **both** triggers: `⌥`-drag as the accelerator and the `框选` button as
    the guarantee. (Open question 2, answered.)
  - A page range is set by **both** a direct drag on the page strip and a typed field, bound to the
    same state in both directions. Typing valid text redraws the strip; dragging rewrites the text.
    Invalid text never mutates the strip. (Open question 3, answered.)
  - Preset questions (`解释` / `翻译`) on the floating bar are **deferred**, not rejected. (Open
    question 4, parked.)
  - The multi-segment model is accepted even though v1's drag produces one span, so that typed input
    does not force a schema change later.
- Still open: the primary placement of the three scopes (open question 1).

## Test plan

- Unit: range parsing (`31`, `31-45`, `31-45, 60`, full-width dash, whitespace), span ordering and
  merging, rejection of out-of-document pages, empty selection refusal, scope serialisation.
- Unit: the parser and the formatter are inverses — formatting a parsed range and parsing it again
  yields the same spans.
- Integration: scope chips drive the request body; switching scope preserves the typed question;
  a capture chip can be removed and is then absent from the request.
- Integration (synchronisation): typing a valid range redraws the strip; typing invalid text leaves
  the strip and scope chip unchanged and disables Send; dragging across the strip rewrites the field
  on every tick; committing from the page indicator focuses the field, not the strip.
- End-to-end: select → ask → the request carries the quote and page; drag a box → the request
  carries an image and no text; set `31–45` → the request carries 15 page labels.
- Accessibility: every control reachable by keyboard; the floating bar does not trap focus.

## Risks

- **The scope model may be one abstraction too many.** If users never change scope, the chips are
  noise; the fallback is to hide two of them behind a chevron. Watch for it in the first five
  people who use it.
- **Modifier-drag on macOS is intercepted by some trackpad gestures.** Verified only on one machine;
  the `框选` button is the dependable path, so the modifier is an accelerator, not the only way.
- **A capture chip that looks like an attachment may be missed.** It sits above the input, in the
  same place a reply would appear, and cannot be sent without being visible.

## Assumptions

- The floating bar can be positioned without covering the selection; if it cannot, it moves below.
- PDF.js exposes selection rectangles that map cleanly to normalised page coordinates.
- Touch support is not required for v1 but the controls must not break when tapped.

## Deviations

*(empty)*

## Open questions

1. **Where do the three scopes live?** Both options are wireframed in the review thread:

   - **A — three permanent buttons on the page** (`选中` / `框选` / `范围`). Clicking one arms a
     mode; the next drag does that thing. Discoverable, but the page now has modes, and "drag text
     while box mode is armed" is a real failure.
   - **C — scopes in the AI panel** (`选中 · p.31` / `本页` / `范围 · 31–45`), with the page always
     doing the direct thing: drag selects text, `⌥`-drag draws a box. The chips say what the next
     question is about and can also be clicked to change it.

   Proposed: C. It keeps the page mode-free, and it is the only option that accommodates both box
   triggers without adding a fourth button.

2. **Does a typed range accept several segments** (`31-45, 60`)? Proposed: yes, in the model and the
   parser; the strip edits the segment under the cursor.
