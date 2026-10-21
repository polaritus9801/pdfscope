# Review — MVP walkthrough

- **Date:** 2026-10-21
- **Method:** automated checks plus a headless browser. No desktop session was available, so nothing in
  this document was verified through a real window. That limitation is the first finding.

## Artifacts (release UI build)

| Artifact | Size |
|---|---|
| UI WebAssembly | 295 KB |
| UI JavaScript glue | 37 KB |
| Stylesheet | 13 KB |
| Viewer module | 10 KB |
| Vendored PDF.js | 1.7 MB |
| Desktop binary (debug) | 32 MB |

The release desktop binary was not built; a Tauri release build with LTO takes long enough to be its
own task. The debug figure is recorded only so a later release figure has something to compare to.

## What is verified

- `just check`: format, clippy with warnings denied, 81 tests, rustdoc, UI build, desktop compile.
- The renderer against a real 26-page PDF: load, render, page navigation, selection payload.
- The AI layer against MiMo `mimo-v2.6-flash`: reasoning separated from text, usage reported.
- The shell's empty state and panel layout, in a headless browser.

## What is not verified

- The settings sheet saving to the keychain and reloading it.
- Any request made from inside the app.
- Clicking a citation, capturing a region, redeeming a reference — the UI paths exist and compile,
  but no click has ever been dispatched in a real window.

## Findings

| # | Finding | Severity | Status |
|---|---|---|---|
| F1 | A range over 20 pages was silently truncated | high | fixed |
| F2 | Per-token reactive updates made long answers quadratic | medium | fixed |
| F3 | No `⌘↵` to send, no `Esc` to stop | medium | fixed |
| F4 | The last ~50 ms of tokens could arrive after `done` | low | fixed |
| F5 | History has no in-page tint or scrollbar marker | medium | deferred: a passage does not store its rectangle, and the rail needs its own layout |
| F6 | Stop does not abort the HTTP request, so the provider may bill a few more tokens | low | accepted in the spec |
| F7 | Settings cannot test the connection before saving | medium | proposed |
| F8 | Up to 20 page texts are fetched sequentially at send time | low | acceptable for now; parallelising changes ordering semantics |
| F9 | The streaming answer is not announced by screen readers | medium | proposed: `aria-live="polite"` on the answer, throttled |
| F10 | No desktop session has exercised any path | high | needs a human |

## Acceptance list

| # | Criterion | Code | Verified in a window |
|---|---|---|---|
| 1 | Select text, ask, click `p.N`, land on the page | yes | no |
| 2 | Capture a region, ask, get an answer about the image | yes | no |
| 3 | Type `31-45`, ask about the chapter | yes | no |
| 4 | Ask with nothing selected, get an answer about the current page | yes | no |
| 5 | Stop mid-answer, keep what arrived | yes | no |

## Recommended order

1. A human runs the five acceptance steps and reports the first break.
2. F10 then dominates every other improvement: until a window has run the loop, further code is
   speculation.
3. F9 and F7 are cheap and worth doing in the same pass.
4. F5 when the passage model gains its rectangle.
