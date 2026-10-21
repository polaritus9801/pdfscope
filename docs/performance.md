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
| First page rendered after opening an 11.7 MB, 80-page image PDF | **60 ms** | 2026-10-22 |
| Typing latency in the question box (20 keystrokes, median) | **17 ms** (max 19 ms) | 2026-10-22 |
| Reader smoke suite (81 checks, 40 pages) | passes | 2026-10-22 |

## Method

The first-page number comes from a headless run that dispatches a real `change` event on the file
input and polls for `#page-1[data-rendered="true"]`, timing with `performance.now()`. It includes
PDF parsing, page measurement, shell creation and the first canvas paint.

## Method — typing

Twenty synthetic `input` events with increasing lengths, each followed by one animation frame; the
number is the time from dispatching the event to the frame that painted it. The handler also resizes
the textarea, so that cost is included.

## Verdict against the spec

- 10 MB first page under 1 s: **yes**, 60 ms.
- Typing under 50 ms: **yes**, 17 ms median.

## Caveats

- The 11.7 MB fixture is text plus photographic noise, not a scanned-and-OCR'd book; extraction and
  text-layer costs differ, but parsing and first paint are representative.
- Numbers come from headless Chrome, not the packaged Tauri window; the WebView adds startup cost
  that this table does not include.
- Measured on an M4 MacBook Air; slower machines will be slower.
