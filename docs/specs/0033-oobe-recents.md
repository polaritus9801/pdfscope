# 0033 — OOBE: guide and recents, side by side

**Status:** accepted.

## Layout

The empty reader is a two-column welcome surface centred in the stage:

- **Left** — the existing PDFScope introduction: eyebrow, “边读边问，答案带页码”, the three
  numbered steps, and the two actions. The actions now sit in a column with an 8 px gap instead of
  touching.
- **Right** — “最近打开”, with a short subtitle and a live count. A row shows a document glyph, the
  file name (single-line ellipsis), `size · age`, and a chevron. The whole row is the button.
- Empty state: “还没有打开过文档 / 打开一本 PDF，下次它会出现在这里。”
- Loading state while IndexedDB is read: “正在读取最近打开…”
- The cards stretch to the same height at wide widths and wrap naturally when the stage is too narrow.
- Double-clicking the welcome shell no longer bubbles to the stage's “open file” double-click handler.

## Persistence

The HTML file picker does not expose a file path, so a recent entry stores the document bytes, not a
path:

- `ui/public/recents.js` opens IndexedDB database `pdfscope-recents`, object store `documents`
  (keyPath `id`, index `updated`).
- `id = "${name}::${byteLength}"`; saving the same name/size replaces the record and refreshes its
  timestamp. At most 8 records are kept; older records are pruned.
- `list()` returns metadata only; `save(name, bytes)` stores a copied `Uint8Array`; `load(id)` returns
  `{name, bytes}`.
- A successful document load saves the recent. The byte copy is made **before** `PDFScope.load`:
  PDF.js may transfer the input buffer to its worker, which would otherwise leave a detached view for
  IndexedDB.
- The recents shelf refreshes after a save and on app start. Opening a recent uses the same
  `load_document` path as the file picker, so all state resets, outline/page sizes, and the recent
  timestamp update identically.
- Storage/indexedDB failures degrade to an empty shelf; they do not block opening files.

## Acceptance

- Empty reader shows the existing guide on the left and “最近打开” on the right, with a visible gap
  between the two actions.
- Opening the sample or a picked PDF then reloading shows it in recents with name, size and relative
  time; clicking it reopens the same document.
- Recents are newest first, capped at 8, and persist across restarts.
- No HTTP path is stored; no cloud call is made.
- `just check` green.
