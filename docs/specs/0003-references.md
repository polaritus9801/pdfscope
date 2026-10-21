# 0003 — References: what one question is about

- **Status:** draft
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** —

## Goal

A reader can attach **many things at once** to a single question — several passages, several
screenshots, one or more page ranges — see exactly what is attached before sending, remove any of
them, and know that the answer is grounded in that set rather than in whichever thing was selected
last.

## Non-goals

- No modes. The page never enters a state where a drag means something other than "select text".
- No persistent highlights or annotations. References live in the conversation, not in the file.
- No OCR in this slice; a scanned region travels as an image.
- No cross-page text selection: one passage lives on one page.
- No preset questions (`解释` / `翻译`). Parked, not rejected.

## Behaviour

A question carries a **reference tray**: an ordered list of zero or more references, shown above the
input and sent with the question.

| Reference kind | How it is added | What is sent |
|---|---|---|
| Passage | select text, then `加入引用` | The quote, its page, and the paragraph around it |
| Region | `⌥`-drag a rectangle, or `框选` then drag | A cropped image, its page, and the rectangle |
| Pages | drag on the page strip, or type in the range field | Each page's text, labelled with its number |

All three kinds can coexist in one tray, in any order. The tray is the single source of truth for
"what this question is about" — there is no other scope anywhere in the interface.

Nothing is sent until Send. Every reference is visible before it is sent, and can be removed or
inspected (jump to page, preview image) without removing it.

## Data model

```rust
pub struct ReferenceId(u16);

pub enum Reference {
    Passage { id: ReferenceId, anchor: Anchor },
    Region  { id: ReferenceId, page: PageIndex, rect: Rect, image: ImageRef },
    Pages   { id: ReferenceId, spans: Vec<PageSpan> },
}

pub struct PageSpan { pub start: PageIndex, pub end: PageIndex }
```

The tray is `Vec<Reference>`; order is insertion order. Validation: spans are ordered, inside the
document, and merged when they overlap; duplicate passages on the same page are allowed (the reader
may want two different quotes); the tray has a soft cap of 12 references, above which the interface
warns but still allows sending.

## Interface

`POST /api/v1/ask` with:

```json
{ "question": "…", "references": [ { "kind": "passage", "page": 30, "quote": "…" }, … ] }
```

The response reports which reference each citation came from, so the answer can point back at the
tray entry rather than at an anonymous page.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Reference tray | `引用` | above the input, in the AI panel | always | click a chip to inspect; `✕` removes | `Tab` cycles chips, `Delete` removes | tap to inspect, long-press to remove | chips list every attached reference with kind icon and page | removing the last chip returns the tray to its empty hint |
| Add passage | `加入引用` | floating bar on a text selection | a selection exists | click | `⌘⇧A` | tap | a passage chip appears; selection stays visible | `Esc` closes the bar, adds nothing |
| Add region | `框选` | floating bar, and `⌥`-drag on the page | any page | drag, release | hold `⌥`, drag with pointer | tap `框选`, then drag | an image chip appears with a thumbnail | `Esc` before release cancels |
| Add pages | range field + page strip | AI panel | a document is open | drag ticks, or type `31-45, 60` | type, `↵` commits; `⌘G` focuses | drag handles | one `Pages` chip per committed span set | invalid text marks the field only; the tray is unchanged |
| Inspect | — | tray chip | always | click | `↵` on a focused chip | tap | the reader jumps to that page and flashes the area or image | clicking elsewhere returns, nothing is lost |
| Send | `发送` | input | tray valid and question non-empty | click | `⌘↵` | tap | question and references leave; the tray stays, marked as "will resend" | a failed send keeps the tray and the question intact |

**Alternatives considered.**

- **A — three permanent mode buttons** (`选中` / `框选` / `范围`). Rejected: mutually exclusive by
  construction, so it cannot express "two passages and a screenshot in one question", which is the
  actual use.
- **B — three scope chips, one active** (the earlier draft). Rejected for the same reason: a
  single-valued scope cannot accumulate.
- **C — chosen: a tray.** The three ways of pointing become three ways of *adding*, not three states.
  Nothing about the page changes; the page always does the obvious thing.

**Discoverability.** The tray's empty state says what can go in it: `选中文字、框选截图，或选择页码范围`.
The floating bar on a selection shows `加入引用` — the moment a user first selects text is the moment
they learn the tray exists.

**States.**

| State | Page drag | Tray | Send |
|---|---|---|---|
| Empty | selects text | hint text | disabled |
| One or more references | still selects text | chips visible | enabled when the question is non-empty |
| Adding a region | draws a marquee | unchanged until release | disabled while the marquee is open |
| Sending | allowed | chips disabled, progress shown | becomes Stop |

**Cost of being wrong.** A stale tray silently included in the next question is the dangerous case:
the user thinks they asked about one page and sent five. Mitigation: the tray is always visible,
chips state their page, and the composer shows a one-line summary above Send
(`将发送：2 段文字 · 1 张截图 · 12 页`). A mis-captured region is recoverable until Send because the
chip previews its image.

## Design review

- Reviewed with: z1mLi on 2026-10-21
- Decisions:
  - The three kinds are **combinable in one question**, expressed as a tray rather than a scope.
    (This replaced the earlier "three scopes" draft, and with it the open question about mode
    buttons.)
  - Both box triggers ship: `⌥`-drag and a `框选` button.
  - A page range is set by both a drag on the page strip and a typed field, synchronised in both
    directions. Invalid text never mutates the strip.
  - Multi-segment ranges are in the model from the start, so typing `31-45, 60` needs no schema
    change.
  - Preset questions are parked.
- Pending: whether the tray survives a send, and the soft cap. See open questions.

## Test plan

- Unit: span parsing (`31`, `31-45`, `31-45, 60`, full-width dash), merging, rejection out of range;
  tray accepts mixed kinds and preserves order; the 12-reference warning triggers at 13.
- Integration: adding from each of the three paths puts the right kind in the tray; removing a chip
  removes it from the request body; the send summary counts match the tray.
- Integration (synchronisation): typing a valid range redraws the strip; invalid text leaves the
  strip untouched and disables Send; dragging rewrites the field on every tick.
- End-to-end: one question with a passage, a screenshot and a range arrives as three references with
  distinct ids, and the answer's citations name the reference they came from.
- Accessibility: the tray is reachable by keyboard, chips announce their kind and page, removing a
  chip keeps focus in the tray.

## Risks

- **The tray can become invisible clutter.** Mitigation: the send summary line repeats what will go,
  so the tray is never the only place the truth lives.
- **A large tray can be expensive.** Mitigation: the summary includes an estimated token count and
  turns amber above a threshold; it is a warning, not a block.
- **"Kept after send" may surprise.** If the tray persists, the first follow-up may resend material
  the reader had forgotten. Mitigation: the summary stays on screen and the chips show a `将重发`
  marker.

## Assumptions

- A reference's page numbers are stable for the lifetime of the conversation; a replaced document
  invalidates its tray entries and they are marked as such.
- Images are stored per conversation and deleted with it; nothing is written next to the PDF.

## Deviations

*(empty)*

## Open questions

1. **Does the tray survive a send?** Proposed: yes, with a visible `将重发` marker and a
   `清空引用` action. Rejected alternative: clear on send, which forces re-selecting a passage for
   every follow-up.
2. **Soft cap or hard cap?** Proposed: warn above 12 references, never block; the cost estimate is
   the real limiter.
3. **When the configured model has no vision support**, should a region reference be refused at
   attach time, or sent and fail? Proposed: refuse at attach, with a message naming the setting to
   change.
