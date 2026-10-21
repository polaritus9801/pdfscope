# 0003 — References: what one question is about

- **Status:** implemented
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("继续做")

## Goal

A reader can attach many things to one question — passages, screenshots, page ranges, and pieces of
earlier answers — see exactly what will be sent, and get any of it back later from a history without
having to find the place in the book again. Hovering a past reference shows where it lives on the
page and on the scrollbar, without moving the view.

## Non-goals

- No modes. A drag on the page always selects text.
- No persistent highlights written into the PDF. References live in the conversation.
- No OCR in this slice; a scanned region travels as an image.
- No cross-page text selection.
- No preset questions (`解释` / `翻译`). Parked.
- No automatic inclusion of history. A redeemed reference must be chosen deliberately.

## Behaviour

Two layers, deliberately distinct:

| Layer | What it holds | Sent with the next question |
|---|---|---|
| **Current references** (`引用栏`) | What this question is about | Yes |
| **History** (`历史引用`) | Everything referenced earlier in this conversation | **No**, until redeemed |

After a send, the current references move into history. They are not silently re-sent: the next
question starts empty. This is the answer to the stale-tray problem — history is recall, not context.

### Kinds

| Kind | Added by | Carries |
|---|---|---|
| Passage | select text → `加入引用` | quote, page, surrounding paragraph |
| Region | `⌥`-drag, or `框选` then drag | cropped image, page, rectangle |
| Pages | drag the page strip, or type `31-45, 60` | each page's text, labelled |
| Answer | select text in an answer → `引用`, or the answer's `引用整段` | the quoted words and the turn they came from |

An `Answer` reference has no page. It is labelled `AI · 第 3 轮` and is never presented as source
text: the model is told it is quoting its own earlier output, not the document.

### Redeeming from history

The history is a list under the tray, collapsed to the last three entries with a `全部` disclosure.

- **Hover a row** → the referenced area is tinted on the page with `--accent-strong` at 38%, and a
  marker appears on the vertical scrollbar at the proportional position. If the area is not on the
  screen, **the view does not move**; a `p.87 →` pill appears beside the row instead, which jumps on
  click.
- **Hover for 600 ms** → a preview card opens beside the row: kind, page, two lines of the quote, or
  a thumbnail for regions. It never covers the row itself and closes on leave.
- **Click the row** → redeems into the current tray. The row briefly shows a check. If an identical
  reference is already in the tray, the click focuses that chip instead of duplicating it.
- **`Shift` + click**, or the row's `定位` action, jumps to the page.
- **`Delete`** on a focused row removes it from history, with an undo toast.

History is per conversation. It survives switching pages and documents. It is not written into the
PDF, and it does not survive deleting the conversation.

### Unbounded count

There is no limit on how many references a question may carry. Layout, not policy, handles volume:

- The tray wraps to at most two rows; beyond that it scrolls vertically inside a fixed height, so
  the composer never jumps.
- A chip never shrinks below 96 px and never grows past 220 px; longer quotes are ellipsised with
  the full text in the hover preview.
- Above 24 chips, the tray shows a `+N` summary chip that expands the full list.
- The history list is virtualised; only visible rows are rendered.
- Region thumbnails are lazy-loaded and released when their row leaves the viewport.

## Data model

```rust
pub struct ReferenceId(u16);

pub enum Reference {
    Passage { id: ReferenceId, anchor: Anchor },
    Region  { id: ReferenceId, page: PageIndex, rect: Rect, image: ImageRef },
    Pages   { id: ReferenceId, spans: Vec<PageSpan> },
    Answer  { id: ReferenceId, turn: u16, quote: String },
}

pub struct PageSpan { pub start: PageIndex, pub end: PageIndex }
```

Validation: spans ordered, inside the document, overlapping spans merged; a quote is non-empty after
normalisation; an `Answer` reference must point at a turn that exists in this conversation. Duplicate
references are deduplicated by `(kind, page, normalised text or rect)`.

The tray is `Vec<Reference>` in insertion order; history is the same type, append-only per
conversation.

## Interface

`POST /api/v1/ask` carries `{ question, references }`. The response reports, per citation, which
reference id it came from, so an answer can point at a tray entry rather than an anonymous page.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Tray | `引用 (N)` | above the input | always | click a chip to inspect; `✕` removes | `Tab` cycles; `Delete` removes | tap inspects, long-press removes | chips in insertion order, two rows max | removing the last returns the empty hint |
| Add passage | `加入引用` | floating bar on selection | a selection exists | click | `⌘⇧A` | tap | passage chip added; selection stays | `Esc` closes the bar, adds nothing |
| Add region | `框选` | floating bar and page | any page | drag, release | `⌥` + drag | tap `框选`, then drag | image chip with thumbnail | `Esc` before release cancels |
| Add pages | strip + field | AI panel | document open | drag ticks or type `31-45, 60` | `↵` commits, `⌘G` focuses | drag handles | one `Pages` chip | invalid text marks the field only; tray unchanged |
| Add answer | `引用` / `引用整段` | answer bubble, answer footer | an answer exists | click; or select text → bubble | `⌘⇧A` after selecting | tap | `Answer` chip labelled with its turn | quoting an empty selection is refused |
| History row | — | list under the tray | history non-empty | hover tints page + scrollbar; click redeems; `Shift`+click jumps | arrows move, `↵` redeems, `Delete` removes | tap redeems | row shows a check; chip appears | removing asks nothing but offers undo |
| Locator pill | `p.87 →` | beside a history row | the reference is off-screen | click jumps | `↵` on the pill | tap | view moves to that page and flashes the area | closing the list leaves the view where it was |
| Send | `发送` | input | question non-empty and estimate inside the window | click | `⌘↵` | tap | references move to history; tray empties | failure keeps both; nothing is lost |

**Hover indication is three-part and must agree:** the tinted area on the page, the scrollbar marker,
and the preview card. If the page is not the one being read, only the marker and the pill appear —
the reader is never yanked away by a hover.

**Alternatives considered.**

- Three permanent mode buttons. Rejected: mutually exclusive, cannot express a mixed reference set.
- One active scope chip. Rejected for the same reason.
- Re-sending the previous references automatically. Rejected: it silently spends money and pollutes
  the next question; history with explicit redemption is the honest version.

**Discoverability.** The tray's empty state names the three ways to fill it. The history's
`全部` disclosure appears as soon as there is history. The answer footer's `引用整段` makes the
fourth kind visible without documentation.

**States.**

| State | Page drag | Tray | History | Send |
|---|---|---|---|---|
| Empty | selects text | hint | as-is | disabled |
| With references | still selects text | chips | as-is | enabled when the question is non-empty |
| Hovering history | unchanged | unchanged | row highlighted | unchanged |
| Sending | allowed | disabled, progress shown | disabled | becomes Stop |

**Cost of being wrong.** The dangerous case is sending more than intended, so the composer states it
twice: the tray itself, and a one-line summary above Send (`将发送：2 段文字 · 1 张截图 · 12 页`)
with an estimated token count. Above 60% of the model's context the summary turns amber; above the
window, Send is disabled with a specific message rather than a generic failure.

## Design review

- Reviewed with: z1mLi on 2026-10-21
- Decisions:
  - References of all kinds combine in one tray; there are no modes.
  - After send, references move to history and are **not** re-sent until redeemed.
  - Hover shows location without jumping; jumping is always a click.
  - Answer text can become a reference, labelled by turn, never as document text.
  - No cap on references; overflow is a layout problem, not a policy.
- Pending: history persistence across sessions (open question 1).

## Test plan

- Unit: span parsing and merging; dedupe by kind and content; history append moves references out of
  the tray; AI reference requires a live turn.
- Integration: each add path produces the right kind; redemption adds to the tray and does not
  mutate history; removing from the tray does not remove from history; the send summary counts match.
- Layout: 1, 12, 40 and 120 chips produce no composer jump and no horizontal scrollbar on the panel;
  the history list renders at most a screenful of rows.
- Hover: an off-screen reference tints nothing and moves nothing; the marker appears at the right
  proportion; the preview opens only after the delay and closes on leave.
- End-to-end: one question with a passage, a screenshot, a range and an answer quote arrives as four
  references; the answer cites them by id.
- Accessibility: history is a listbox; focus order is tray → input → history; every hover affordance
  has a focus equivalent.

## Risks

- **History becomes a second, invisible truth.** Mitigation: it is never sent implicitly, and its
  rows state their page and kind at all times.
- **Hover tint on a dense page is noise.** Mitigation: 38% tint of one accent, no dimming of the rest
  of the page, and no animation after the first 120 ms.
- **Scrollbar markers collide** when many references sit on nearby pages. Mitigation: markers merge
  into a band above three in the same 2% of the track.
- **Unbounded references can exceed the context window.** Mitigation: the estimate is exact enough to
  refuse before sending, and the refusal names the count to remove.

## Assumptions

- The provider reports cache and token usage; where it does not, the status line shows `—` rather
  than a fabricated zero.
- Images are stored per conversation and deleted with it. Nothing is written next to the PDF.

## Deviations

- History keeps passages and regions only; page-range and answer references join when their slices
  land.
- Hovering a row shows a delayed preview (600 ms, CSS), a `p.N →` jump control, a tint over the
  rectangle when the reference is on the page being read, and a marker on the rail at the reference's
  position in the document. Hovering never scrolls, as designed.
- Shift+click jumps and flashes the page; a plain click redeems into the tray.
- History lives in memory for the session, as designed.

## Open questions

1. **History persistence:** conversation-only (proposed) or written to disk so it survives a restart?
   Disk adds a privacy surface and a migration path; conversation-only means closing the app loses it.
2. **Hover delay:** 600 ms proposed. Shorter feels twitchy, longer feels dead — worth confirming on a
   real page with a trackpad.
