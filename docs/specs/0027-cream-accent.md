# 0027 — Cream accent for reasoning and borrowed tools

**Status:** accepted.

## Why a second accent

The product palette was white/black/lilac. Reasoning is a different mode of output from an answer,
and a borrowed tool is different from an active tool; both were using the same lilac as primary
actions, so the distinctions were colourless. Cream gives those two states their own quiet surface
without adding a competing call-to-action colour.

## Tokens

A four-step cream ramp is added for light and dark themes (`docs/design/tokens.md`):

- `--cream-50` — reasoning surface
- `--cream-100` — borrowed rail icon
- `--cream-200` — reasoning border
- `--cream-700` — reasoning text and borrowed icon ink

Cream is not used for buttons, references, citations, selection, or status. Purple remains primary.

## Application

### Reasoning

- The pill/card itself stays on the normal white surface.
- The header row (“思考 N 秒” or “思考中…”) is white.
- The body below it is cream with cream ink, separated from the header by a 1 px `--cream-200` rule;
  this is identical for an expanded completed block and the live streaming block, so opening does not
  change the shape.
- A closed “思考 N 秒” pill shows only its white header; no cream is visible.

### Borrowed modes

- A sticky tool keeps `active` (lilac).
- The renderer reports the borrowed mode to Rust; the corresponding `.rail-tool.borrowed` uses the
  cream fill and ink.
- The pointer tool stays lilac, so when `⌘/⌥/⇧` is held exactly one rail icon is lilac and one is
  cream; releasing clears the cream.

## Acceptance

- Reasoning content is pale cream in both themes; text remains AA-readable.
- Holding a modifier in pointer mode leaves the pointer icon lilac and turns only the borrowed mode
  cream; releasing restores both.
- No other component uses cream.
- `just check` green.
