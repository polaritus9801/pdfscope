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
| **Page range** | click the page indicator, or edit the range chip | Pages `a..=b`, labelled per page |

Asking is always explicit: nothing is sent on selection, capture, or range change. The user presses
Send (or `⌘↵`). A capture is shown as a removable chip before it can be sent.

## Data model

No library schema change. The scope is request state:

```rust
pub enum AskScope {
    Selection { page: PageIndex, anchor: Anchor },
    Page { page: PageIndex },
    Range { start: PageIndex, end: PageIndex },
}
```

Validation: `Range` requires `start <= end` and `end < page_count`; an empty or whitespace-only
selection is refused at capture, not at send.

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
| Page range | `31 / 404` | toolbar, right of the page navigation | a document is open | click opens a small popover with `from` / `to` fields | `⌘G` focuses the same popover | tap the indicator | chip becomes `范围 · 31–45`; the popover closes | invalid (`from` > `to`, outside document) → inline message, confirm disabled; a range above 20 pages shows an estimate and requires confirmation |
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

- Reviewed with: — (pending)
- Decisions: —

## Test plan

- Unit: range validation, empty selection refusal, scope serialisation.
- Integration: scope chips drive the request body; switching scope preserves the typed question;
  a capture chip can be removed and is then absent from the request.
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

1. **Which is the primary path?** Three permanent buttons (option A) or scopes-in-panel (option C)?
   C is proposed because it keeps the page free of modes, but A is more discoverable on day one.
2. **Box capture trigger:** `⌥`-drag (fast, invisible) or click `框选` first (explicit, one extra
   step)? Proposed: both, with `⌥`-drag as the accelerator and the button as the guarantee.
3. **Page range input:** a `from`/`to` picker, or a text field accepting `31-45, 60`? Proposed:
   picker first, text parsing later — parsing is a feature with its own error messages.
4. **Should the floating bar offer preset questions** (`解释` / `翻译`) beside `问`? Proposed: no in
   v1; presets multiply the surface before the core loop is proven.
