# Page-ruler visibility

Date: 2026-10-26

## Page-ruler visibility

`pdfscope.page-ruler` stores `on`/`off` and defaults to `on`. The Reading tab's "显示页面标尺"
checkbox applies immediately.

The custom page ruler is also the reader's scrollbar, so "off" does not remove the thumb or break
scrolling. It hides the canvas marks, section ticks, hover line, tooltip, page preview and reference
bands. The rail background and draggable thumb remain. The renderer reads
`data-page-ruler="off"` at startup and exposes `setPageRulerVisible`; hover previews are skipped
while hidden.

## Zoom and panel drag

Not changed here. Zoom stays on the per-frame pointer implementation from `0208612`; the ruler
refreshes its viewport after the zoom frame without owning the scroll position.
