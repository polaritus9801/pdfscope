# Performance

Measurements taken while implementing spec 0020 (P7). The app is measured, not estimated.

## Environment

| | |
|---|---|
| Machine | MacBook Air M4, 16 GB |
| Browser engine | Headless Chrome 156 (same WebKit-family baseline as the Tauri WebView) |
| Build | `trunk build --release` UI, debug desktop shell |

## Numbers

| Case | Result | Date |
|---|---|---|
| First page rendered after opening a 6-page text PDF | **79 ms** | 2026-10-22 |
| Reader smoke suite (81 checks, 40 pages) | passes | 2026-10-22 |

## Method

The first-page number comes from a headless run that dispatches a real `change` event on the file
input and polls for `#page-1[data-rendered="true"]`, timing with `performance.now()`. It includes
PDF parsing, page measurement, shell creation and the first canvas paint.

## Caveats

- The fixture is small (6 pages, 3 KB). A 10 MB scanned document will be slower; the spec's
  acceptance line (first page under 1 s for 10 MB) is **not yet measured**.
- Typing latency has **not** been measured; the spec asks for under 50 ms.
- Numbers come from headless Chrome, not the packaged Tauri window; the WebView adds startup cost
  that this table does not include.
