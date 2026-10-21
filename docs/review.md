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
| Desktop binary (release) | 17 MB |

The release desktop binary was not built; a Tauri release build with LTO takes long enough to be its
own task. The debug figure is recorded only so a later release figure has something to compare to.

## UI smoke test

`scripts/ui-smoke` drives the built UI in headless Chrome against a fake `window.__TAURI__`, so the
real components run: settings load, question entry, send, streaming, citation rendering, status line.
It caught one bug (a stale reasoning counter) that no amount of compilation would have found.

Latest run:

```
model label: mimo-v2.6-flash
answer: "这一段讨论的是公共领域的规范有效性。p.31 它把合法性建立在交往前提上。"
status: "输入 812 · 输出 41 · 缓存 512 · ≈ 29 tok/s"
```

The citation renders as a control only when the page exists; with no document open, `p.31` stays
plain text, which is the intended behaviour.

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
| F5 | History has no in-page tint or scrollbar marker | medium | fixed |
| F6 | Stop does not abort the HTTP request, so the provider may bill a few more tokens | low | accepted in the spec |
| F7 | Settings cannot test the connection before saving | medium | fixed |
| F8 | Up to 20 page texts are fetched sequentially at send time | low | acceptable for now; parallelising changes ordering semantics |
| F9 | The streaming answer is not announced by screen readers | medium | fixed: `aria-live="polite"` on the answer |
| F11 | The reasoning counter stayed on screen after the answer finished | medium | fixed (found by the smoke test) |
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
