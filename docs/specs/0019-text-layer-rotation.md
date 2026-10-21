# 0019 — A text layer that follows page rotation

- **Status:** implemented
- **Author:** z1mLi
- **Date:** 2026-10-22
- **Approved by:** z1mLi ("bug不一定只有一个，你需要找全…多余的不要动", 2026-10-22)

## Goal

On a PDF page with `/Rotate 90` (or 180/270), selecting text must highlight the text under the
pointer, the quote must start where the reader started, and the selection bar must sit over the
selection. The page canvas is already landscape; the text layer must match it.

## What was wrong

PDF.js lays a text layer out in **unrotated page coordinates** and marks the container with
`data-main-rotation="90"` (or 180/270). The full `pdf_viewer.css` rotates that container with:

```css
[data-main-rotation="90"] {
  transform: rotate(90deg) translateY(-100%);
}
[data-main-rotation="180"] {
  transform: rotate(180deg) translate(-100%, -100%);
}
[data-main-rotation="270"] {
  transform: rotate(270deg) translateX(-100%);
}
```

Our hand-trimmed `.textLayer` rules copied the span positioning but not those three rules. On the
CA2 problem PDF, page 1 is 886 × 626 landscape, while its text layer was 626 × 886 portrait; every
span had a `rotate(-90deg)` transform. The browser therefore painted a vertical selection over a
horizontal page, and the selection bar's normalised rectangle inherited the bad geometry.

A second, independent defect: `reportSelection()` returned early when the browser selection became
empty, so the Rust selection bar stayed on screen. It could also outlive a page re-render, because
`paintTextLayer()` replaced the layer without telling Rust.

## Fix

- Add the three PDF.js rotation rules after `.textLayer` in `ui/public/styles.css`.
- In `viewer.js`, remember whether a selection was reported; when the selection becomes empty,
  clear the Rust bar through `__pdfscopeCleared`. Clear it before the text layer is rebuilt too.
- The PDF.js viewer branch already vendors the full `pdf_viewer.css`, so it is untouched.

## Non-goals

- No change to the selection-bar layout, text extraction, or the PDF.js viewer branch.
- No new rotation control; this only follows the PDF's own page rotation.
- No text-layer rebuild or new selection model.

## Data model

None. The renderer reports the same `{page, x, y, width, height, text}` payload.

## Interface

Unchanged: `__pdfscopeSelection(payload)` and `__pdfscopeCleared()`.

## Interaction design

| Field | Selection on a rotated page |
|---|---|
| Label | existing selection bar (`p.N`, quote, 加入引用, 复制) |
| Location | over the selected page, above the selection's bounding box |
| Enabled when | text is selected |
| Pointer | drag; the highlight follows the pointer's text |
| Keyboard | existing selection shortcuts |
| Touch | long-press/drag, same browser selection |
| Result | horizontal highlight; normalized rectangle matches the page |
| Cancel | click elsewhere, navigate, or re-render clears the bar |
| Error | none new |

Alternatives considered: rotating the canvas instead (would make the page portrait and contradict
the PDF's displayed rotation); adding a page-level CSS transform (would also rotate the canvas);
patching only the selection normalizer (would leave the visible highlight wrong). The CSS rule is
the smallest fix because PDF.js already emits the attribute.

**Cost of being wrong:** selection on rotated pages remains visually wrong, but the page still
renders and the rest of the reader is unaffected.

## Test plan

- Load `MA4804_Project_CA2_Problem.pdf`, page 1 (a 90° two-book spread).
- Select within the left column: highlight is horizontal, quote starts at the chosen line, bar is
  above the selection.
- Select across both columns: highlight is horizontal, bar is over the union.
- Clear the selection: the bar disappears.
- Repeat on the continuous branch; the rotated page must still select correctly when the page is
  scaled by the column transform.
- Run the existing 40-page and 400-page reader suites to check no regression on unrotated pages.
- Screenshot: `docs/images/reader-rotated-selection.png`.

## Risks

- The rotation rules are global. Only elements carrying `data-main-rotation` are affected; PDF.js
  sets that on the text layer only.
- A future text-layer rebuild must continue to clear the Rust bar; the explicit call in
  `paintTextLayer` covers it.

## Assumptions

- The PDF's page rotation is represented by PDF.js's viewport and `data-main-rotation`; verified on
  the CA2 PDF (`data-main-rotation="90"`, layer 626 × 886 inside an 886 × 626 canvas).
- The full `pdf_viewer.css` rules are the intended upstream behavior; the branch already uses them.

## Deviations

- Applied to `main` and `feat/custom-reader`; `feat/pdfjs-viewer` already has the full CSS and needs
  no change.
- Headless Chrome reproduced the fix with both a programmatic range and real mouse drags. The native
  Tauri trackpad pass remains manual.
