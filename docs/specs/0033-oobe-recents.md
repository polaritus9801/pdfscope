# 0033 — OOBE: guide and library in one panel

**Status:** accepted.

## Layout

The empty reader is a **fixed overlay above the whole application**, not a child of the PDF stage:
a dimmed/blurred backdrop covers the header, stage, rail and AI panel, and a floating panel is
centred in the remaining viewport with 32px breathing room. The panel is `920px` wide and `720px` tall (clamped by the viewport), centred vertically with its
original 330px guide on the left and the library taking the remaining width. Below an 800px overlay
width it stacks and releases the fixed height.
This makes the OOBE independent of the PDF stage/panel width and usable at any window size.

Inside that floating panel is a single surface with one internal vertical rule:

- **Left** — the existing guide (eyebrow, title, three steps, two actions), compressed to a `330px`
  column so the right side gets the space. The actions have an 8 px gap. `text-wrap: pretty` keeps the
  step copy from leaving an orphan line.
- **Divider** — 1 px `--border`, inset 24 px from the top and bottom; it never reaches the panel edges.
- **Right** — two equal sections stacked vertically:
  - **最近打开** — the ten newest opens (scrolls; each half is capped at 50% of the library height).
  - **推荐打开** — a larger scan of the recommended directories, deduplicated by native path and
    name+size, then the ten newest remaining PDFs (scrolls; capped at 50% height).
- Section subtitles are intentionally absent. Each row is a **single line**: the supplied red PDF type
  mark, name, `size · relative age`, and a chevron. Both sections use the same header, count, row,
  empty and loading styles. Each list owns its scroll and is capped at half the library height.
- At an overlay narrower than 800 px (container query on `.oobe-overlay`), the panel stacks
  vertically, hides the vertical rule, and scrolls internally if the height is short.
- Double-clicking the panel does not bubble to the stage’s “open file” handler; the backdrop blocks
  the rest of the application. Errors raised while the OOBE is open appear in an `.oobe-error` toast
  above the backdrop.
- Hovering a row for 600 ms renders a one-row preview bubble above it: up to the number of page
  thumbnails that fit in 780px (at most 12), each numbered, with a `+N` tile for longer documents.
  The `+N` tile takes the measured height of the last displayed page thumbnail.
  Recent previews read IndexedDB bytes; recommended previews read the native path. Previews are
  cached briefly, cancel on leave/scroll/resize/Escape, and fail silently if the file moved.

## Recent documents

- A recent stores the document **bytes**, not a path: the browser file picker exposes no path.
  IndexedDB `pdfscope-recents` / `documents`, keyPath `id`, index `updated`; `id` is
  `path-or-name::byteLength`. Newest first, capped at 20; ten shown.
- Native opens also store the path, which feeds the recent-directory set for recommendations. Browser
  opens and the sample have an empty path and still reopen from bytes.
- The input buffer is copied **before** `PDFScope.load`: PDF.js can transfer/detach it to its worker.
- Clicking a recent reopens through the normal load path; a missing original path does not matter
  because the bytes are retained. If the bytes are unreadable, the panel reports it.

## Recommended documents

The desktop shell adds native file access because a file path is required:

- `pick_pdf` uses `rfd` to return a path; `read_pdf` returns the bytes as a raw IPC response (no JSON
  array); `scan_recommended(recent_dirs, limit)` scans directories.
- Directories: `dirs::download_dir`, `desktop_dir`, `document_dir`, plus the parent directories of the
  five most recent opens. Duplicates are removed.
- Only **direct child** `.pdf` files (case-insensitive extension) are considered; hidden files and
  nested folders are skipped. Missing/unreadable directories are ignored.
- Entries are sorted by modification time descending and capped; the UI shows ten after excluding entries
  whose path or name+size already appears in 最近打开.
- A missing file between scan and click: the row is removed immediately and a clear error is shown.
  A file deleted before scanning is simply absent.
- Browser builds have no native commands; the recommended section shows its empty state. The old HTML
  picker remains as the browser fallback; native builds use the native picker for the header, OOBE,
  Cmd/Ctrl+O and empty-stage double-click.

## Acceptance

- One panel, internal vertical rule not touching top/bottom, compressed left guide and right library.
- Both right sections use the same compact one-line row style and the supplied PDF icon.
- Ten recent / ten recommended entries at most; both reopen through the normal document path.
- The recommended list is the newest 10 after removing files already present in the recent list.
- Hovering a row shows the page contact-sheet bubble after the delay; the bubble never blocks clicks.
- Native scan uses Downloads, Desktop, Documents and recent parent directories, direct PDFs only,
  newest first; missing files never break the shelf.
- Recents and their paths survive restart; sample/browser recents still reopen.
- `just check` green.
