# Page-range overlay parity audit

This is the old `PageView` page-range logic compared with the global `PageRangeOverlay`. The
overlay is a different DOM/event shape; this is the rule-by-rule mapping.

## State and bars

| Old per-page rule | Global overlay |
|---|---|
| `page_visual()` candidate/open/base/add/subtract model | `draft()` computes the same candidate/open/base/subtract state once |
| Candidate subtracts only when `range_fully_selected` | Same `range_fully_selected` decision |
| Add preview covers candidate pages that are not committed | `add_segments()` splits the candidate around committed pages |
| Precedence subtract → open → add → base | Same precedence when building bar specs |
| Committed range uses deep caps at both real range ends | One base segment uses `--deep-border` / `--deep-body`, flags computed from visual base neighbours |
| Add range: clicked anchor end deep, hover end light | Segment cap is deep when `anchor_page == segment start/end` |
| Open anchor: placed cap deep, hover endpoint light | `open_gradient(kind, placed)` unchanged |
| Bottom gap after a non-last page | Base/add/ghost spans include the internal gaps; a base segment before an open committed page extends through the gap and drops its bottom cap |
| Ghost range: diagonal body plus first/last cut marks | Same classes/body/cut marks |
| `total_pages` fallback when extending the last start endpoint | `adjust_same_endpoint()` receives the layout page count and extends to it |

## Outlines and hit semantics

| Old rule | Global overlay |
|---|---|
| `.page.in-scope` solid outline | Solid `page-overlay-solid` box |
| `.page.preview` dashed, `.page.removing` lighter dashed | Dashed preview/subtract boxes; subtract replaces solid |
| Three hit buttons per page/side, thirds start/single/end | Two full-height strips; pointer Y maps to page/third with binary search |
| `mouseenter` enters a button; `mouseleave` leaves the page column | `pointermove` updates only a page/third/side change; gap/leave clears |
| After click, same-button wiggle does not re-enter | Suppressed `(page, kind, side)` until zone changes or strip is left |
| Toolbar was the actual interactive button | Toolbar is pointer-transparent; hit strip owns mouse, toolbar buttons remain the keyboard path |
| Only visible pages can receive page actions | Metrics carry `visible`; paged/spread hidden pages reject both hover and click |

## Page modes and keyboard

- Paged/spread metrics refresh after `pagedLayout()` and after page navigation, so actions follow the
  visible sheet rather than a hidden one.
- The keyboard path is intentionally a single global toolbar at the current page (three tab stops)
  instead of three buttons per page. Enter/Space still invoke start/single/end, and page navigation
  changes the target page.

## Regression assertions

`just perf-baseline` checks:

- zero per-page action/range DOM remains;
- the active toolbar is `pointer-events: none` and the hit strip is the pointer target;
- post-click movement inside the same third leaves action and preview hidden;
- the placed anchor cap is deep and the hover cap light on a 500-page addition preview;
- 1000/3000-page geometry still produces the expected page shells and overlays.
