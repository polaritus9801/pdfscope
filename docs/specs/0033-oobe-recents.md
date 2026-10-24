# 0033 — OOBE: guide and library in one panel

**Status:** accepted.

## Layout

The empty reader is a single surface (`980px` max) with one internal vertical rule:

- **Left** — the existing guide (eyebrow, title, three steps, two actions), compressed to a `330px`
  column so the right side gets the space. The actions have an 8 px gap. `text-wrap: pretty` keeps the
  step copy from leaving an orphan line.
- **Divider** — 1 px `--border`, inset 24 px from the top and bottom; it never reaches the panel edges.
- **Right** — two equal sections stacked vertically:
  - **最近打开** — the five newest opens.
  - **推荐打开** — the five newest PDFs found in the recommended directories.
- Each row is compact: the supplied red PDF type mark, one-line name, `size · relative age`, and a
  chevron. Both sections use the same header, count, row, empty and loading styles. Five entries per
  section keeps the panel shallow while showing enough files.
- At a stage narrower than 760 px (container query on `.empty`), the panel stacks vertically, hides
  the vertical rule, and scrolls if needed.
- Double-clicking the panel does not bubble to the stage’s “open file” handler.

## Recent documents

- A recent stores the document **bytes**, not a path: the browser file picker exposes no path.
  IndexedDB `pdfscope-recents` / `documents`, keyPath `id`, index `updated`; `id` is
  `path-or-name::byteLength`. Newest first, capped at 8; five shown.
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
- Entries are sorted by modification time descending and capped; the UI shows five and excludes paths
  already present in 最近打开.
- A missing file between scan and click: the row is removed immediately and a clear error is shown.
  A file deleted before scanning is simply absent.
- Browser builds have no native commands; the recommended section shows its empty state. The old HTML
  picker remains as the browser fallback; native builds use the native picker for the header, OOBE,
  Cmd/Ctrl+O and empty-stage double-click.

## Acceptance

- One panel, internal vertical rule not touching top/bottom, compressed left guide and right library.
- Both right sections use the same compact row style and the supplied PDF icon.
- Five recent / five recommended entries at most; both reopen through the normal document path.
- Native scan uses Downloads, Desktop, Documents and recent parent directories, direct PDFs only,
  newest first; missing files never break the shelf.
- Recents and their paths survive restart; sample/browser recents still reopen.
- `just check` green.
