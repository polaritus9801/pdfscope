# 0024 — AI panel visual refresh

**Status:** accepted and implemented.

The design rationale and the comparison live in `docs/design/ai-panel.md`; this spec is the
implementation contract.

## Goal

Make the AI panel read like a mature assistant rather than a form: identity, message hierarchy,
one icon language, designed states. Behaviour, request logic and the reader stay unchanged.

## Phases

1. **P0** — one `Icon` family and provider marks; a 52 px header (provider tile, title, model chip
   with a custom popover, history/new/collapse); a composer card with toolbar and circular send/stop.
2. **P1** — answer identity rail, right-aligned question bubble, reasoning pill, one icon action row,
   designed empty state.
3. **P2** — grouped context shelf with header, counts, clear-all and page ranges.
4. **P3** — skeleton, streaming caret, structured error card, focus rings, 120 ms motion.
5. **P4** — settings search-provider cards with marks/status; model popover polish.

## Acceptance

- No behaviour change: the same buttons do the same things; only markup/CSS/icons.
- Provider identity is visible in the header and on every answer.
- One icon family; no text glyphs or emoji as interface icons.
- The composer is one contained field with a toolbar; send is a round icon button and becomes stop.
- Light/dark at 320, 420 and 720 px panel widths; no horizontal overflow.
- `just check` green; each phase has its own commit.

## Progress

- P0 landed: icons/provider marks, header, model popover, composer card.
- P1 landed: each answer has a provider tile, model name and duration; questions are right-aligned
  bubbles; reasoning is a pill with a brain icon and chevron; answer actions are icon buttons with
  tooltips and accessible names; the empty state has a mark, copy and three suggestion cards.
  `Turn.model` records the model that produced each answer (serde-defaulted for old data), and the
  streaming answer gets a caret.
- P2 landed: the shelf has a `上下文 · N 项` header and 清空, rows use type icons, page chips collapse
  into inclusive ranges (`p.12–14`) that remove the whole range, and the remaining text glyphs (✕ ✎ ★)
  are icons.
- P3 landed: a three-line skeleton before the first token, a structured error card (icon, cause, retry
  and settings), focus-visible rings across the panel, a quiet turn entrance and skeleton shimmer,
  all disabled under `prefers-reduced-motion`.
- P4 landed: the search provider is a two-column card list with marks, a hint and a selected check;
  the key row carries an 已配置 badge; the model popover gains the provider mark in its header.
- All phases landed. Behaviour is unchanged apart from the per-answer `model` display field and the
  clear-all path, which now also removes the corresponding pills while keeping the typed text.


## Regression — opening a PDF burned 100% CPU

Sampling the WebKit content process showed the main thread inside
`FileChooser::chooseFiles → dispatchChangeEvent → performMicrotaskCheckpoint → wasm`, recursing
rather than rendering the PDF.

The document-load effect called `restore_draft`, which read the attachment signals to label pills.
The composer's input callback then removed "missing" attachments with `update`, which notifies even
when nothing changed; that notification re-ran the load effect, which restored the same stale draft
again. A draft containing an image/file pill whose attachment no longer existed (images and files are
session-only) therefore looped forever.

Fixes:

- every signal read used to label or restore a draft is untracked, so the load effect depends only on
  the document key;
- attachment removal checks that the entry still exists and never calls `update` for a no-op;
- file pills are now removed like image pills (they were previously ignored);
- a restored draft is written back immediately in its cleaned form.

Verified with `just check`; the running app must be rebuilt/restarted to pick it up.
