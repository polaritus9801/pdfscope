# 0030 — User-drawn scope icons

**Status:** accepted.

The three page-scope modes use the reader's own vector artwork from
`ui/public/页面视野三模式.svg`, a 751 × 201 sheet containing three drawings.

## Mapping

| Mode | Artwork | File |
|---|---|---|
| 整份文档 | PDF file with the red PDF badge | `scope-icons/document.svg` |
| 仅当前页 | lined document | `scope-icons/page.svg` |
| 仅引用 | dashed selection with folded corner | `scope-icons/references.svg` |

The sheet is not redrawn. Each icon is a standalone SVG with the original drawing and a viewBox crop
around its third of the sheet (`0 0 200 201`, `299 0 176 201`, `574 0 177 201`); all three keep the
full 201 px height.

## Presentation

- Trunk copies `public/scope-icons` into the bundle.
- The trough keeps its rounded-rectangle outline but is **transparent**: no fill. Its one-pixel border
  is one step darker (`--border-strong`).
- Each mode button is 28 × 28 and capsule-shaped. The three sit with no flex gap, so they are closer
  than the previous version; the reference-count badge remains.
- The `<img>` has `height: 18px; width: auto`, preserving each drawing's native proportions. All
  drawings carry a stronger edge shadow than the former hairline.
- The selected mode is a lavender capsule. Its pseudo-element bleeds four pixels on each side, sits
  above the dividers in stacking order, and therefore covers the neighbouring needle divider(s). At
  the first or last position it also covers the trough's outer edge. The active capsule uses the
  rounded-rectangle family radius (pill ends) and an accent-tinted hairline.
- A two-pixel needle divider sits between modes: 14 px tall, pointed at both ends, with one-pixel
  negative side margins. It never touches the trough top/bottom and adds no width between icons.
  It is painted below the active capsule.

## Acceptance

- The scope modes show the supplied artwork with native proportions and aligned top/bottom edges.
- The trough has no fill and a slightly darker border; icons are closer together.
- Each divider is a pointed needle that does not touch the top/bottom.
- Selecting any mode covers its adjacent divider(s); the first/last selection also covers the trough
  edge. The active capsule is clearly lavender, and all three drawings keep a visible edge shadow.
- Active/hover surfaces and the reference badge still work.
- `just check` green.
