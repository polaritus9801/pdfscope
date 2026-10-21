# 0009 — Clickable citations

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("继续")

## Goal

A page reference in an answer is a control, not decoration: clicking `[p.31]` moves the reader to
page 31 and makes the page itself flash, so the claim can be checked in one gesture.

## Non-goals

- No excerpt-level highlight: the answer cites a page, not a rectangle, so only the page is marked.
- No citation parsing for other formats (footnotes, author-year). One shape, done well.
- No back-navigation to where the reader was; the page field writes history like any other jump.

## Behaviour

- `[p.31]`, `[p31]`, `[p 31]`, `[P.31]` and `[pp.31-33]` are recognised; the last one links to 31.
- A recognised reference renders as a small button labelled `p.31` inside the running text.
- Clicking it jumps to that page and flashes the page frame in the accent colour for 800 ms.
- A reference outside the document is rendered as plain text, not a dead control.
- The rest of the text is untouched, including line breaks.

## Data model

None persisted. Parsing is a pure function:

```rust
pub enum Segment {
    Text(String),
    Citation(u32),
}

pub fn segments(text: &str) -> Vec<Segment>;
```

## Interface

No protocol change. The renderer gains `flash()`: adds a class to the page element for 800 ms.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Citation | `p.31` | inline in the answer | the page exists in the document | click | `Tab` + `↵` | tap | the reader jumps to page 31; the page frame flashes once | out-of-range refs stay plain text; the flash is decorative and never blocks input |

**Alternatives considered.**

- Making the whole answer clickable. Rejected: ambiguous target, and destructive for selection.
- Hovering to preview the page instead of jumping. Rejected for this slice: a preview needs a
  thumbnail pipeline; jumping is the actual need.

**Discoverability.** The button is visibly a button (border, pointer, focus ring).

**States.** Streaming answers may contain a half-received `[p.3`; the parser leaves it as text until
the closing bracket arrives, so a citation never flickers into existence mid-token.

**Cost of being wrong.** A wrong page wastes a jump, not data. The flash tells the reader the jump
landed.

## Test plan

- Unit: every accepted shape parses to the right page; text without citations is one segment; an
  unterminated `[p.3` stays text; `[pp.31-33]` yields 31.
- Manual: ask a question that cites three pages, click each, confirm the page and the flash.

## Risks

- **Models write citations in many dialects.** Mitigation: the accepted set is documented and tested;
  anything else stays text rather than becoming a wrong link.
- **A click during streaming** is allowed; the answer keeps arriving.

## Assumptions

- The model follows the `[p.N]` instruction from 0008's system prompt most of the time; when it does
  not, the text simply stays text.

## Deviations

- The parser lives in `pdfscope-core`, not in the interface: recognising a citation is domain logic
  and belongs where it can be tested without a browser.
- Out-of-range citations are rendered as plain text rather than a disabled button, so a hallucinated
  page cannot be clicked and cannot look clickable.
- The flash is a CSS animation on the page frame; no protocol change was needed beyond `flash()`.

## Open questions

1. Should a citation also add its page to the tray? Proposed: no, one gesture one effect.
