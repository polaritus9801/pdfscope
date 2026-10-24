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
full 201 px height, so their rendered top and bottom edges match.

## Presentation

- Trunk copies `public/scope-icons` into the bundle.
- Each `.scope-icon` button is 34 × 28 and flex-centred.
- The `<img>` has `height: 18px; width: auto`; each drawing keeps its native aspect ratio (the PDF
  is slightly wider), while the common 18 px height and flex centre align all three top, bottom and
  centre. Nothing is stretched horizontally.
- The existing active segment and reference-count badge remain unchanged.

## Acceptance

- The three scope modes show the supplied artwork, not the old line icons.
- The icons share top/bottom edges and centres without horizontal stretching; the drawings keep their
  native proportions.
- Active/hover surfaces and the badge still work.
- `just check` green.
