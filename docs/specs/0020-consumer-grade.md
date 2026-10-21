# 0020 — From usable to consumer-grade

- **Status:** draft
- **Author:** z1mLi
- **Date:** 2026-10-22
- **Approved by:** —

## Goal

The app is usable; it should feel like a product someone would pay for. After this work: the first
minute teaches itself, every state is designed, the keyboard covers the frequent actions, and the
visual system is one system rather than per-component choices. This spec covers the desktop app
only; accounts, cloud and the eventual web service are explicitly out of scope.

## Non-goals

- Accounts, sync, collaboration, sharing links, payments, telemetry.
- Mobile or web deployment.
- Annotations, editing, OCR, plugins.
- Re-architecting the reader or the provider layer; those work and are not the subject.

## Behaviour

- **First run.** With no settings file, the empty reader explains three steps (open, select, ask),
  offers a sample document, and shows a banner when no key is stored. The onboarding is dismissible
  and stays dismissed.
- **Every state is designed.** Empty, loading, streaming, stopped, empty answer, offline, no key,
  unreadable file, one-page document, long filename. Each has deliberate copy and layout.
- **Keyboard.** `⌘K` opens a command palette listing every action with its shortcut; `⌘/` shows the
  cheat sheet; focus is always visible; every mouse action has a keyboard path.
- **Feedback.** Saving, copying, exporting and failures produce a toast (max 3, auto-dismiss 3 s,
  hover pauses). Validation errors appear next to the field.
- **Reading.** The reader shows progress, remembers position per document, offers an outline and
  `⌘F` find. A 300-page book reopens on the same page.
- **Resilience.** A dropped stream is retried before the first token and continues after it (already
  true); the offline state explains what to do instead of printing a transport error.

Unhappy paths are part of the contract: no text layer, unreadable file, provider unavailable,
cancelled mid-stream, empty selection.

## Data model

Additive fields in `settings.json`, all with defaults so older files load:

```
font_size: u16        // 12–18, default 14
language: "zh" | "en" // default "zh"
default_zoom: string  // fit-width | fit-page | percent
page_mode: string     // continuous | single | spread
onboarding_done: bool
reading_positions: { [doc_hash]: { page, zoom, mode } }  // max 50 entries, LRU
```

`reading_positions` is a new store table rather than settings JSON if it exceeds one entry per
document; the migration is additive and the rollback is dropping the table.

## Interface

- Settings JSON: the fields above; `load_settings` / `save_settings` return them.
- New Tauri command: `reading_position(doc_hash)` / `save_reading_position(hash, page, zoom, mode)`.
- New callbacks on `window.PDFScope`: `outline()`, `find(query)`, `findNext()`, `progress()`.
- Shortcuts: `⌘K`, `⌘/`, `⌘F`, `⌥⌘F`, `⌘O`, `⌘,`, `⌘↵`.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Command palette | `⌘K` | centred overlay | always | click item | type, `↑↓`, `↵`, `esc` | tap | runs the action | `esc`; empty query lists recent |
| Onboarding card | 1·2·3 | empty reader | no document | click step | `tab`, `↵` | tap | highlights the matching control | `×` dismisses, remembered |
| Sample document | 试试示例 | onboarding card | no document | click | `↵` | tap | opens bundled sample, focuses composer | failure → toast |
| Missing-key banner | 还没有配置模型 | under header | no key | click action | `tab` | tap | settings opens on 模型 | dismiss for session |
| Toast | message | bottom centre | on event | hover pauses | — | tap dismisses | disappears after 3 s | max 3 stacked |
| Progress bar | p / n | reader bottom | document open | drag to seek | `[` `]` | drag | scrolls to position | release keeps position |

**Alternatives considered.**

- Chosen: onboarding inside the empty state, `⌘K` palette, toasts.
- Rejected: a separate multi-step wizard — delays the task and is skipped; a permanent help panel —
  competes with the document; a status-only feedback line — invisible for completed actions.

**Discoverability.** The palette is advertised in the header (`⌘K` chip), onboarding names each step,
shortcuts appear in tooltips, and settings has a cheat sheet.

**States.** Palette: closed / filtering / no matches. Onboarding: hidden / step 1–3 / dismissed.
Toast: entering / visible / leaving. Reader: no document / loading / ready / error.

**Cost of being wrong.** Onboarding is dismissible and resettable; the palette never mutates
documents; toasts never block input. A mis-shown banner costs one click, not data.

## Design review

- Reviewed with: — (pending)
- Decisions: —

## Test plan

- Unit: settings round-trip with new fields and an old file; reading-position LRU; palette filter.
- Integration: `scripts/ui-smoke/reader.mjs` stays green after each phase.
- End-to-end: first-run walkthrough (delete settings → open sample → select → ask → citation);
  keyboard-only walkthrough (open, select, ask, copy, settings, close); screenshot matrix, three
  sizes × two themes.

## Risks

- **Regression of the four working paths** (selection, capture, tints, citations) while the shell
  changes. Mitigation: the reader smoke test runs every phase.
- **Half-migrated tokens** produce drift. Mitigation: P1 ends with a grep check, not an opinion.
- **Scope**: seven phases. Mitigation: each phase ships alone, ordered so P1–P2 already improve the
  product.

## Assumptions

- The target is Chinese-first with an English toggle later; copy is written in Chinese.
- Bundling a sample PDF (≈200 KB) is acceptable for the app size.
- The system font stack is kept; a reading serif is a later experiment.
- Per-document positions can be stored in SQLite without a migration of existing data.

## Deviations

## Open questions

1. Is `⌘K` acceptable on Windows/Linux builds, or should it be `Ctrl+K` with platform mapping?
2. Should the sample document ship in the binary or download on demand?
3. Is a single AI thread enough for v1, or does "成熟" require multiple threads?
