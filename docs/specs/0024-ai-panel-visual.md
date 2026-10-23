# 0024 — AI panel visual refresh

**Status:** accepted.

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
