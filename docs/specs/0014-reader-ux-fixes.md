# 0014 — Reader UX fixes from the first real run

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi (eight issues reported after running the app)

## Goal

Fix what the first real session exposed: the app icon, a page that scrolled as a whole, a capture tool
that armed but never fired, references living in a toolbar instead of the composer, and a fixed-width
panel that could not be collapsed.

## Behaviour

1. **Icon.** A rounded square with a transparent margin, as macOS expects, in 16/32/64/128/256/1024.
2. **Fixed shell.** The window never scrolls; only the reader and the panel body do.
3. **Open by double-click.** Double-clicking the empty reader opens a file.
4. **Three floating tools**, vertical, top-left of the reader: text (`文`), box (`框`), range (`页`).
5. **Capture fires.** The marquee listens on the document, because the page element does not exist
   when the renderer module loads.
6. **References in the composer.** One row each, light purple, three lines maximum with ellipsis, a
   remove control, and a persistent tint on the page for every reference that sits on it.
7. **Panel collapse and resize.** A `›` collapses the panel to a strip with an expand control; the
   left edge is a drag handle, clamped to 260–640 px.
8. **Pinch zoom.** Trackpad pinch and ctrl+wheel change the zoom, reported back to the zoom field.

## Second pass (same day)

- The three floating tools are icons, not characters, at 42 px; hover keeps them opaque and tints
  them with the accent instead of fading.
- The panel's collapse and expand controls are 34 px icon buttons.
- Reasoning is shown in full while it streams and collapses to `思考 N 秒` when the answer begins.
- The answer is rendered as Markdown: headings, paragraphs, lists, fenced code, bold, italic and
  inline code. Everything is escaped first, and page citations become links with `data-page` that the
  panel intercepts — the renderer lives in `pdfscope-core::markdown`, so it is unit-tested.

## Third pass

- Dragging the panel divider no longer starts a text selection: the drag clears any existing
  selection, marks the document `resizing` (so the browser cannot start one), and prevents the
  default on pointer down and move. Both the system highlight and our own selection bar stay put.

## Non-goals

- Continuous scrolling, single/double page modes and page-by-page navigation are **not** in this
  slice. They are the next piece and are described below rather than pretended.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Text tool | `文` | floating rail | always | click | — | tap | disarms capture; selection is always available | — |
| Box tool | `框` | floating rail | a document is open | click | — | tap | crosshair; drag draws a marquee | `Esc` disarms |
| Range tool | `页` | floating rail | always | click | — | tap | focuses the range field | — |
| Collapse | `›` | panel head | expanded | click | — | tap | panel becomes a 28 px strip | `‹` restores it |
| Resize | — | panel left edge | expanded | drag | — | drag | width follows the pointer, 260–640 px | release ends the drag |
| Reference row | quote | composer | a reference exists | click `✕` removes | `Tab` | tap | row disappears; its tint leaves the page | — |

**Alternatives considered.** Keeping the tray toolbar: rejected — the reader asked for references to
sit with the question, and a toolbar duplicated the composer's job.

**Cost of being wrong.** Removing a reference is one click and re-adding is two; nothing is persisted.

## Next: reader modes

PDF.js ships a viewer component (`pdfjs-dist/web/pdf_viewer.mjs`) that already implements continuous
scrolling, page modes, text layer and zoom. The current module hand-rolls only the single-page case.
The next slice adopts that component or reimplements its scroll model, keeping the three-function
boundary so region capture and selection continue to work.

## Test plan

- Manual: the eight items above, in a real window.
- Headless: layout screenshot — floating rail present, no top tray, references only in the composer.
- Unit: none added; the changes are layout and event wiring.

## Deviations

*(filled during implementation)*
