# 0020 — From usable to consumer-grade

- **Status:** draft
- **Author:** z1mLi
- **Date:** 2026-10-22
- **Approved by:** —

## Goal

PDFScope works. This spec makes it feel like a product someone would pay for: the first minute is
self-explanatory, every state is designed, the interface has a consistent visual language, and the
common actions are one keystroke away instead of three clicks.

The desktop app is the subject. No accounts, no cloud, no web service — that is a later product, and
this spec deliberately does not prepare the architecture for it.

## What "mature" means here

Five dimensions, all testable:

1. **Nothing is undesigned.** Every state has a designed appearance: empty, loading, streaming,
   error, offline, no key, unsupported file, long filename, one-page document, 800-page document.
2. **The first minute teaches itself.** A new user can open a PDF, select text, ask a question and
   find the citation without reading documentation.
3. **The keyboard is complete.** Every frequent action has a shortcut, shortcuts are discoverable,
   and focus is always visible.
4. **The visual system is one system.** Spacing, type, radius, elevation and colour come from tokens;
   no component invents its own.
5. **It never feels slow or stuck.** Input responds in under 100 ms; long operations say what they
   are doing; nothing spins without a reason.

## Audit — where we are

| Area | Today | Target |
|---|---|---|
| First run | Empty stage, one button, no explanation | Three-step onboarding inside the empty state; a sample PDF action |
| Empty states | One line of text | Title, explanation, primary action, shortcut hint |
| Visual tokens | Incremental; some spacing hard-coded | One token set; a documented scale |
| Typography | System serif-ish defaults per component | One type scale, reading-optimised for the answer column |
| Motion | Only zoom easing | 120–180 ms transitions for panel, dialog, toast; reduced-motion respected |
| Keyboard | A handful of shortcuts, no discovery | Complete map + `⌘K` command palette + cheat sheet in settings |
| Settings | One dialog, five fields | Grouped preferences incl. appearance, reading, AI, shortcuts, about |
| Feedback | Status line only | Toasts for completed actions; inline errors where they happen |
| AI panel | Good | Thread management: rename, clear, copy-all, regenerate, continue |
| Reading | Continuous, zoom, rotation | Progress bar, outline, page thumbnails, remember position |
| Window | Fixed minimum | Sensible min size, remembers geometry, full-screen reading mode |
| Onboarding | None | Interactive, dismissible, one-time |

## Phases

Each phase ends with screenshots at three sizes (`1280×800`, `1512×982`, `900×700`) in light and dark,
a keyboard-only pass, and `just check`.

### P1 — Design system and shell

- A token file becomes the single source: spacing scale (4/8/12/16/24/32/48), radius (4/8/12/16),
  three elevations, a type scale (11/12/13/15/17/22), semantic colours that both themes satisfy.
- Header, panel, dialog and buttons rebuilt on those tokens; no hard-coded pixel values outside the
  token block.
- A `Card`, `Button`, `Field`, `EmptyState`, `Toast`, `Tooltip` vocabulary in CSS, used everywhere.
- Motion: 150 ms ease-out for panels and dialogs; `prefers-reduced-motion` disables all of it.
- Acceptance: a grep for `px` in component rules finds only tokens and icon sizes.

### P2 — First run

- The empty state becomes a three-step card: open a file (with `⌘O`), select text, ask a question.
  It disappears after the first successful question and never returns.
- A "试试示例" action opens a bundled sample page and pre-fills a question.
- Opening the app with no key shows a non-blocking banner: "还没有配置模型" with a settings action.
- Acceptance: delete the settings file, launch, and a first-time user can ask a question in under a
  minute without external instructions.

### P3 — Reading surface

- Progress bar at the bottom of the reader, draggable, showing `page / total`.
- Outline (PDF table of contents) in a collapsible left rail, with thumbnails in the same rail.
- Reading position remembered per document (page, zoom, mode) and restored on reopen.
- `⌘F` find in document with hit count and next/previous.
- Acceptance: reopen a 300-page book and land on the same page; 17 hits found in under a second.

### P4 — AI panel as a product surface

- Threads: clear current thread, copy the whole thread, rename? (single thread is fine for v1).
- Actions on every answer: copy, regenerate, continue (already), plus "在文档中定位" for the first
  citation.
- Empty/loading/error states with illustrations-free, typographic treatment.
- The composer grows with content up to 6 lines, then scrolls; drag-and-drop a selection onto it.
- Acceptance: every action available by mouse is available by keyboard.

### P5 — Settings and preferences

- Tabs: 通用 (theme, language, font size), 阅读 (default zoom, page mode, remembered positions),
  模型 (provider, endpoint, model, key, output budget, reasoning effort), 提示词, 快捷键, 关于.
- Every preference persists and applies immediately; a "恢复默认" per section.
- Acceptance: change any setting, quit, relaunch — it holds.

### P6 — Feedback and resilience

- Toast system: saved, copied, exported, failed; max 3 stacked; auto-dismiss 3 s; hover pauses.
- Inline errors sit next to the control that caused them.
- Offline and provider-down states explain what to do, not just what failed.
- Acceptance: pull the network, ask a question — the app says the connection is down and offers retry.

### P7 — Window and performance

- Minimum window 960×640; geometry and maximised state remembered.
- Reading mode (`⌥⌘F`) hides the header and panel.
- Budgets: first page visible under 1 s for a 10 MB PDF; typing latency under 50 ms; no layout shift
  when the answer streams.
- Acceptance: measured with the built app, recorded in `docs/performance.md`.

## Interaction design — new controls in P1 and P2

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Command palette | `⌘K` | overlay, centred | always | click item | type to filter, `↵` runs, `esc` closes | tap | runs the chosen action | `esc`; empty query lists recent actions |
| Palette item | action name | palette list | matches query | click | `↑↓` move, `↵` run | tap | palette closes, action runs | — |
| Onboarding step | 1·2·3 | empty state card | no document open | click step | `tab` focus, `↵` advances | tap | highlights the corresponding control | dismiss `×`, remembered |
| Sample document | 试试示例 | empty state card | no document open | click | `↵` | tap | opens the bundled sample, focuses the composer | fails → toast "示例打不开" |
| Missing-key banner | 还没有配置模型 | under the header | no key stored | click | `tab` to action | tap | opens settings on the model tab | dismiss for this session |
| Toast | message | bottom centre above the rail | on event | hover pauses | — | tap dismisses | disappears after 3 s | max 3 stacked |

**Alternatives considered.** A separate onboarding wizard: rejected — it delays the actual task and is
skipped by most users. A permanent help panel: rejected — it competes with the document for space.

**Cost of being wrong.** All onboarding is dismissible and resettable from settings; nothing blocks
the primary flow.

## Non-goals

- Accounts, sync, collaboration, sharing links, payments.
- Mobile or web deployment.
- Annotations, editing, OCR.
- A plugin system.

## Risks

- **Visual churn vs. stability.** Rebuilding the shell can regress the four paths that already work
  (selection, capture, tints, citations). Every phase re-runs `scripts/ui-smoke/reader.mjs`.
- **Token migration half-done.** If some components keep hard-coded values the interface drifts.
  P1 ends with the grep check, not with a feeling.
- **Scope.** Seven phases is months of work at this pace. Each phase is shippable alone; the order is
  chosen so the product improves even if we stop after P2.

## Test plan

- Screenshot matrix per phase: three sizes × two themes, stored under `docs/images/`.
- Keyboard-only pass: open, select, ask, copy, settings, close — no pointer.
- `scripts/ui-smoke/reader.mjs` and `scripts/ui-smoke/run.mjs` stay green.
- `just check` plus the acceptance line of each phase.

## Open questions

1. Language: is the product Chinese-first, or bilingual with an English toggle? (P5 depends on it.)
2. Typography: keep the system stack, or ship a reading serif for answers?
3. Does "成熟 To C" include a subscription-ready paywall later, or is pricing entirely out of scope
   for the desktop app?
