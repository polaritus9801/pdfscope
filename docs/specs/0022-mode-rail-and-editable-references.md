# 0022 — A mode rail, editable references and page scope

- **Status:** draft
- **Author:** z1mLi
- **Date:** 2026-10-22
- **Approved by:** —

## Goal

Four tools replace three: a pointer that returns to normal reading, and one sticky capsule for text,
box and page modes. References can be adjusted on the page — quotes by their ends, regions by corners
and edges — and every reference change, including which pages are in scope, is undoable. The manual
"页码范围" field disappears: three icons choose what the model sees, and a manager dialog edits the
page set visually.

## Non-goals

- Writing references into the PDF; they live for the session.
- Undo for chat, settings or theme.
- Multi-select of references, copying references between documents.
- Annotations with comments or colours.

## Behaviour

### The rail

Four controls. The first is a separate hollow circle; the other three share one capsule whose top and
bottom radii match that circle.

1. **Pointer** — normal reading and selection. Clicking it leaves any sticky mode. Escape does the same.
   The floating "add reference" bar appears only here.
2. **Text** — sticky. Any text swipe immediately becomes a passage reference.
3. **Box** — sticky. A marquee becomes a region reference on release.
4. **Pages** — sticky page-scope editing (below).

Shortcuts: `V` `T` `B` `D` switch tools, but **never while a text field has focus**. Holding a
modifier temporarily borrows a mode for one gesture, then returns to the current tool:

| Hold (macOS) | Hold (Windows/Linux) | While held |
|---|---|---|
| `⌘` | `Ctrl` | text swipe adds a passage |
| `⌥` | `Alt` | drag draws a marquee |
| `⌃` | `Shift` | drag across pages extends the page set |

### Page scope

Three single-choice icons replace the range field, at the left of the composer row:

| Icon | Name | What the model receives |
|---|---|---|
| ▣ | 仅当前页 | text of the page in view |
| ▤ | 整份文档 | text of every page (bounded by the existing page cap; the cap is reported, not silent) |
| ❝ | 仅引用 | only the passages and regions the reader added; no page text |

To their right: **管理页面引用**, opening the page manager.

### Page marks and the per-page bar

A page in scope gets:

- a thin outline around the sheet;
- a purple bar on its outer edge — left edge in single mode, the outer edge in spread (left page:
  left, right page: right);
- three icon buttons on that bar, at the top, middle and bottom: **从这里开始**, **单独这一页**,
  **到这里结束**. They appear only when the pointer is near the bar.

Toggle logic, stated as a state machine:

- `anchor` is at most one pending endpoint: `Start(page)` or `End(page)`.
- **从这里开始 (p)**: set `anchor = Start(p)`; preview `p .. last page`. Clicking another start
  replaces it; clicking start on the same page clears it.
- **到这里结束 (p)**: with `anchor = Start(s)`, commit `min(s,p) .. max(s,p)` and clear the anchor.
  With no anchor, set `anchor = End(p)` and preview `1 .. p`; a following start closes it.
- **单独这一页 (p)**: toggles page `p`, clears any anchor first.
- Committed ranges are **merged into one page set**; overlap and nesting collapse by definition, so
  nested or duplicate ranges cannot create duplicates.
- `Escape` cancels a pending anchor without touching committed pages.
- The purple bar shows the preview while an anchor is pending, and preview and set are visually
  distinct (preview: dashed outline; committed: solid bar).

In page mode (4), clicking a page toggles it; dragging across pages selects the range on release.
`⌘`(Ctrl) during the drag adds, `⌥`(Alt) subtracts; without a modifier the drag replaces the set.
A drag is committed only if it crosses at least one page boundary.

### Editing a reference

Hover a reference: the ✕ appears at the top-right, its top edge level with the reference's top edge.
`[`/`]` for quotes, eight handles plus a move handle for regions.

**Hover and focus, organically combined.** A pure hover overlay flickers when the pointer leaves the
selection to reach a button that sits outside it. The model is:

- **Hover reveals** the overlay when the pointer is over the reference, the overlay itself, or within a
  12 px bridge between them. The bridge is what makes the buttons reachable.
- **Click pins** the overlay (focus). While pinned it ignores hover-out and stays until `Escape`, a
  click elsewhere, or a completed edit.
- **While dragging** a handle, pinning is irrelevant: the overlay is locked, hover changes nothing, and
  the drag captures the pointer.
- Pinned references draw a 1 px accent outline so the reader knows why the handles persist.

### Undo

`⌘Z` / `⌘⇧Z` (Ctrl / Ctrl+Shift elsewhere) walk one stack per document. Its entries: adding a
reference, deleting one, moving, resizing, re-anchoring a quote, and **changing the page set** (toggle
page, commit range, scope icon change, manager dialog edits including 清空). The stack is cleared when
another document opens. Deleting from the composer uses the same operation as the ✕ overlay.

## Data model

```rust
struct Passage { id: ReferenceId, page: u32, quote: String, rect: Rect }
struct Region  { id: ReferenceId, page: u32, image: String, rect: Rect }

enum PageScope { Current, WholeDocument, ReferencesOnly, Pages(BTreeSet<u32>) }

enum ReferenceEdit {
    Add(ReferenceSnapshot), Remove(ReferenceSnapshot),
    Geometry { id: ReferenceId, before: Rect, after: Rect },
    Anchor { id: ReferenceId, before: AnchorState, after: AnchorState },
    Scope { before: PageScope, after: PageScope },
}
```

The history is `Vec<ReferenceEdit>` plus a cursor; a new edit truncates the redo tail. `PageScope::Pages`
is a set, so range algebra removes duplicates by construction. The page cap for `WholeDocument` stays
20 pages; the response says how many pages were sent.

## Interface

```
PDFScope.setTool(tool)                  // pointer | text | box | pages
PDFScope.setScope(mode)                 // current | document | references
PDFScope.pageSet()                      // committed pages
window.__pdfscopeEdit(edit)             // renderer → Rust: geometry / anchor / page set
window.__pdfscopeToolChanged(tool)      // renderer → Rust
```

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Pointer | cursor | rail circle | always | click | `V`, `Esc` | tap | leaves sticky mode | — |
| Text | 文 | capsule top | document open | click | `T` | tap | sticky text capture | `Esc` |
| Box | 框 | capsule middle | document open | click | `B` | tap | sticky marquee | `Esc` |
| Pages | 页 | capsule bottom | document open | click | `D` | tap | sticky page toggling | `Esc` |
| Start here | ⇤ | page bar, top | pointer near bar | click | — | tap | pending start, preview to end | `Esc` cancels |
| This page | ▣ | page bar, middle | pointer near bar | click | — | tap | toggles one page | undo |
| End here | ⇥ | page bar, bottom | pointer near bar | click | — | tap | closes the pending range | undo |
| Scope: current | ▣ | composer, left | document open | click | — | tap | model gets the visible page | — |
| Scope: whole | ▤ | composer, left | document open | click | — | tap | model gets every page (capped) | cap reported |
| Scope: references | ❝ | composer, left | document open | click | — | tap | model gets only references | — |
| Manage pages | 管理页面引用 | composer, after icons | document open | click | — | tap | opens the page manager | `Esc` closes |
| Manager page | thumbnail | dialog grid | always | click toggles; drag toggles a range | — | tap | set changes live | `⌘` adds, `⌥` subtracts |
| Clear pages | 清空页面引用 | manager footer | set not empty | click | — | tap | empties the set | undo |
| Delete | ✕ | reference top-right | hover or pinned | click | `Delete` | tap | removes it | undo |
| Quote handles | `[` `]` | quote start/end | hover or pinned | drag | — | drag | re-anchors | < 8 px no-op |
| Region handles | 8 dots | corners/edges | hover or pinned | drag | — | drag | resizes | clamps |
| Move | ✥ | below ✕ | hover or pinned | drag | arrows when pinned | drag | moves | clamps |

**Alternatives considered.**

- Chosen: sticky modes, per-page bars, a page manager dialog.
- Rejected: typing page ranges — the human's stated reason: unintuitive.
- Rejected: hover-only overlay — flickers when reaching buttons outside the selection; a short bridge
  plus click-to-pin solves both reach and calmness.
- Rejected: one cycling tool button — no visible state.
- Rejected: per-page checkboxes in a sidebar — the manager dialog gives thumbnails and space, and the
  bars handle the common case in place.

**Discoverability.** The circle is visually separate from the capsule; tooltips name tools and
shortcuts; hovering a page near the edge reveals the bar; the scope icons carry tooltips; the manager
is reachable from the composer where the page question is decided.

**States.**

| Tool | Text drag | Marquee | Page click | Floating bar |
|---|---|---|---|---|
| pointer | selects, bar shows | nothing | nothing | shown |
| text | adds immediately | nothing | nothing | hidden |
| box | selects | adds on release | nothing | hidden |
| pages | selects | nothing | toggles page | hidden |

**Cost of being wrong.** Every change is one `⌘Z` away; a misfired drag under 8 px does nothing; page
ranges merge by set semantics so a wrong range can be corrected page by page.

## Test plan

- Unit: range algebra (start/end, reversed, overlapping, nested, cancellation), undo/redo/truncate,
  clamping, the 8 px threshold, scope serialization.
- Integration (once a browser can be used again): each tool sticky; modifier borrow; hover bridge
  reaches ✕; click pins; drag locks; text re-anchor updates quote; region resize/move; manager drag
  select with ⌘/⌥; ⌘Z across all edit kinds; scope icons change what the request contains.
- Manual: purple bar placement in single and spread; pointer returns from every mode.

## Risks

- **Re-anchoring text** needs range APIs over the PDF.js text layer and must survive rotation; the
  hardest part of this spec.
- **Two writers on one list.** The edit enum must be the only writer of references and scope, or the
  undo stack desynchronises with the composer.
- **Hover overlays** near page edges can fight capture; overlays stop propagation and clamp to the page.
- **Modifier borrowing** can surprise: a modifier held while scrolling must not switch tools. Only a
  drag or a text swipe borrows, and the borrow ends on pointerup.

## Assumptions

- References and page scope remain session-only.
- The 20-page cap stays for "whole document" until the provider-side limit is revisited.
- Page-mode drag replaces unless a modifier says otherwise.

## Open questions

1. **Modifier map** — `⌘`/`⌥`/`⌃` on macOS and `Ctrl`/`Alt`/`Shift` elsewhere: does that feel right?
2. **End without start** — I propose it starts a centre-out selection (`End(p)` previews `1..p`, a
   following start closes it). Acceptable?
3. **Scope is undoable** — I have included page-set and scope-icon changes in the reference undo
   stack. Is that what you want?
4. **Spread bar** — for the right page, the bar and its three buttons sit on the right outer edge
   (facing away from the gutter), so the two bars never crowd the spine. Good?
5. **Page drag** — in page mode, a drag replaces the set unless `⌘`/`⌥` is held. Does replace-by-default
   match your intent?

## Deviations
