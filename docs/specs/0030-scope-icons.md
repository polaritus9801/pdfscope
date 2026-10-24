# 0030 — User-drawn scope icons

**Status:** accepted.

The three page-scope modes use the reader's own vector artwork from
`ui/public/页面视野三模式.svg`. The current revision is a 2481 × 3508 sheet with three drawings along
the top; the icons were redrawn with darker outlines and a dashed-reference mark.

## Mapping

| Mode | Artwork | File |
|---|---|---|
| 整份文档 | PDF file with the red PDF badge | `scope-icons/document.svg` |
| 仅当前页 | lined document | `scope-icons/page.svg` |
| 仅引用 | dashed document with an X mark | `scope-icons/references.svg` |

The sheet is not redrawn. Each icon is a standalone SVG with the original drawing and a viewBox crop
around its group (`105 109 201 201`, `405 109 176 201`, `671 109 176 201`); all three keep the full
201 px height.

## Presentation

This restores the pre-tile scope control:

- Trunk copies `public/scope-icons` into the bundle.
- The trough keeps its original pale lilac fill, one-pixel border and 10 px radius.
- Each `.scope-icon` is flex-centred at 28 × 28 in the toolbar, with a 7 px radius.
- The `<img>` has `height: 18px; width: auto`; each drawing keeps its native aspect ratio and all
  three share top/bottom edges and centre. Nothing is stretched horizontally.
- Hover uses the faint accent wash; the selected segment is the normal white tile with the existing
  accent inset ring. No white plates, needles or capsule pseudo-element are used.
- The reference-count badge is unchanged.

## Acceptance

- The three scope modes show the newly redrawn artwork, not the previous drawings.
- The icons share top/bottom edges and centres without horizontal stretching.
- The control matches the pre-tile appearance: pale lilac trough, white active tile, no extra plates
  or dividers.
- Active/hover surfaces and the badge still work.
- `just check` green.
