# 0022 — A mode rail and editable references

- **Status:** draft
- **Author:** z1mLi
- **Date:** 2026-10-22
- **Approved by:** —

## Goal

The reader gets four tools instead of three: a pointer that returns to normal reading, and a capsule
holding text, box and document modes that stay active until the pointer is chosen again. Every
reference already on the page can be adjusted in place — a text quote by dragging its start and end,
a region by dragging its corners, edges or body — and every reference change can be undone and redone.

## Non-goals

- Annotations as a document format: references live for the session, not inside the PDF.
- Undo for anything other than references (asking a question, deleting a chat turn, changing settings
  are not on the reference stack).
- Multi-select of references, copy/paste of references, reordering the tray.
- Touch gestures beyond what a pointer supports.

## Behaviour

**Rail.** Four controls, top to bottom:

1. **Pointer** — a hollow circular button. Normal reading and selection; no automatic capture. Choosing
   it ends any sticky mode. Shortcuts work from here (see below).
2. **Text** — part of one capsule with 3 and 4. Swiping across text immediately adds a passage
   reference; the old "add reference" floating bar is not needed in this mode.
3. **Box** — marquee capture, as today: drag a rectangle, it becomes a region reference on release.
4. **Document** — the sticky mode for page-scoped context. Its exact behaviour is the open question
   below; the recommended design is click-to-include pages instead of typing ranges.

Modes 2–4 are sticky: the reader keeps making references until they click the pointer button (or
press `Escape`). The shortcut layer under the pointer is `V` pointer, `T` text, `B` box, `D` document.

**Editing a reference.** Hovering a reference shows an overlay:

- A white circular ✕ at the selection's top-right, its top edge level with the selection's top edge.
  Click removes the reference (undoable).
- **Text** references: `[` and `]` handles at the start and end. Dragging either re-anchors the quote
  to the text under the pointer; the stored quote and rectangle update together.
- **Region** references: eight handles (four corners, four edges). Dragging resizes. Below the ✕ sits
  a circular move handle; dragging it moves the whole region.
- Editing changes the reference in the composer at the same time; its row shows the updated page and
  quote/thumbnail.

**Undo/redo.** `⌘Z` / `⌘⇧Z` (Ctrl on Windows/Linux) undo and redo, in order: adding a reference,
deleting it, moving/resizing it, and editing a text anchor. The stack is per document and cleared when
another document opens. The composer's remove (✕) is the same operation as the overlay's ✕.

**Errors.** A drag that ends outside the page clamps to the page edge; a drag under 8 × 8 px is
discarded and changes nothing (no undo entry). If the text layer cannot resolve a new anchor (scanned
page), the edit is rejected with a toast and the old geometry stays.

## Data model

References already exist as `Passage { page, quote, rect }` and `Region { page, image, rect }`.
This spec adds, on the Rust side:

```rust
enum ReferenceEdit {
    Add(ReferenceRef),
    Remove(ReferenceRef, ReferenceSnapshot),
    Geometry { id: ReferenceId, before: Rect, after: Rect },
    Anchor { id: ReferenceId, before: (Rect, String), after: (Rect, String) },
}
```

`ReferenceId` is a stable id assigned on creation (new field on `Passage`/`Region`). The undo history is
`Vec<ReferenceEdit>` with a cursor; redo is truncated on a new edit. Each reference keeps its own
normalised rectangle, so a resize only touches that entry.

## Interface

```
PDFScope.setTool(tool)                 // "pointer" | "text" | "box" | "document"
PDFScope.hoverReference(id)            // shows the edit overlay
window.__pdfscopeReferenceEdit(edit)   // renderer → Rust: geometry/anchor changes
window.__pdfscopeToolChanged(tool)     // renderer → Rust: Escape returned to pointer
```

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Pointer | ✥ / cursor | rail, circular | always | click | `V`, `Esc` | tap | normal reading; ends sticky mode | — |
| Text | 文 | capsule slot 1 | a document is open | click | `T` | tap | sticky; swiping text adds a reference | `Esc` returns to pointer |
| Box | 框 | capsule slot 2 | a document is open | click | `B` | tap | sticky; marquee adds a region | `Esc` returns |
| Document | 页 | capsule slot 3 | a document is open | click | `D` | tap | sticky page selection (open question 1) | `Esc` returns |
| Delete | ✕ | reference top-right | reference hovered | click | `Delete` with reference focused | tap | reference removed | undo restores it |
| Text handles | `[` `]` | reference start/end | text reference hovered | drag | — | drag | re-anchors quote + rect | release clamps; no movement < 8 px |
| Region handles | 8 dots | reference corners/edges | region hovered | drag | — | drag | resizes region | clamps to page |
| Move handle | ✥ | under the ✕ | region hovered | drag | arrow keys when focused | drag | moves region | clamps to page |
| Undo | — | — | history not empty | — | `⌘Z` | — | reverts last reference edit | no-op at stack bottom |
| Redo | — | — | cursor before end | — | `⌘⇧Z` | — | reapplies it | no-op at top |

**Alternatives considered.**

- Chosen: a sticky capsule plus a pointer button that exits; editing directly on the page.
- Rejected: one button cycling tools — users cannot see which mode they are in and overshoot.
- Rejected: editing references only in the composer — a quote's extent is a spatial decision and must
  be made on the page.
- Rejected: a modal editor — it hides the context the reader is adjusting against.

**Discoverability.** The pointer is circular and separate, so the capsule reads as "modes"; tooltips
name each mode and its shortcut; hovering a reference reveals its handles; the first reference after
an edit shows a toast that `⌘Z` undoes.

**States.**

| Mode | Text drag | Marquee | Page click | Floating bar |
|---|---|---|---|---|
| pointer | selects, bar appears | nothing | nothing | shown |
| text | immediately adds | nothing | nothing | hidden |
| box | selects without adding | adds on release | nothing | hidden |
| document | selects | nothing | toggles page (open question 1) | hidden |

**Cost of being wrong.** Every reference operation is undoable; a mistaken edit costs one keystroke.
A drag that misfires (under 8 px) changes nothing. Removing a reference from the composer is on the
same stack, so it is recoverable too.

## Test plan

- Unit: the undo stack (push/undo/redo/truncate), clamping, the 8 px threshold, anchor updates.
- Integration (when the browser is available again): rail modes switch and stay; swipe in text mode
  adds without the bar; marquee in box mode adds; hover shows ✕ and handles; dragging a `[` updates
  the quote; region resize and move clamp; ⌘Z/⌘⇧Z across add/remove/move/edit.
- Manual: pointer returns from every mode; `Esc` behaves the same.

## Risks

- **Text re-anchoring** is the hardest part: mapping a drag position back to a text range needs range
  APIs over the PDF.js text layer and must survive rotated pages.
- **Undo and the composer share one list.** If the two write independently the stack will desynchronise;
  the edit enum must be the only writer.
- **Hover overlays** can fight selection and capture handlers; each overlay is inside the page box and
  stops propagation while dragging.

## Assumptions

- References stay session-only; persistence is out of scope.
- The composer keeps showing one row per reference; editing there is limited to removal.
- The document mode is the page-inclusion idea unless the human asks for something else.

## Open questions

1. **Document mode**: is click-to-include pages (click a page to add it to the question's page set,
   click again to remove, a badge shows the count) the interaction you want? Alternatives: a
   thumbnails strip where you tick pages, or "ask about everything visible".
2. **Shortcuts**: `V` / `T` / `B` / `D` are free. Any conflict with your muscle memory?
3. **Undo scope**: confirm it covers only references, not chat or settings.
4. **Hover vs click** to reveal handles: hover is faster but can flicker while selecting; click-to-focus
   is calmer. Which?

## Deviations
