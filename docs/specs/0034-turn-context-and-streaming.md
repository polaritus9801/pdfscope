# 0034 — Stable streaming and per-question references

**Status:** accepted.

## Streaming answer

The first turn alone used to rebuild the whole transcript on every token. The panel's empty-state
branch read `answer.get()` while `turns` was still empty, so each token re-ran that branch and replaced
the transcript. From the second turn onward `turns.get().is_empty()` short-circuited before evaluating
`answer`, so the bug appeared only on the first question.

The fix is to make the empty-state condition depend only on `turns`, `streaming` and
`current_question` (set at send). The live answer keeps the normal Markdown/citation renderer, so
**Markdown, `[p.N]` citations and emphasis are parsed while tokens arrive**; the surrounding
`.turn.current` stays the same node.

- Live answer: `<div class="answer-text markdown" inner_html=to_html(answer)>`.
- Copy is hidden while streaming; it appears after completion or stop, alongside the other completed
  turn actions.
- `answer` is not read by the outer branch, so token updates cannot recreate the transcript.
- Text tokens are coalesced into a 50ms flush. Both that flush and `flush_pending` now collapse the
  thinking details on the first non-empty answer. The auto-collapse was introduced in `0adfe99`
  (2026-10-24 13:54) after coalescing existed, but its check was placed only in `flush_pending`,
  which normal coalesced text bypasses; it therefore fired only at done/stop/error.

## Per-question references

Every `Turn` now stores a serialized `TurnContext`:

```text
passages, regions, pages, answers, images, files, web, scope_mode, page
```

- `send` snapshots the current context before the request clears the shelf.
- `regenerate` restores that turn's context as the current context.
- `edit` restores the context into the panel signals, composer shelf and ID counters, so resending
  uses the original attachments.
- New turns serialize with the conversation tree; old turns without `context` deserialize as empty.

## Compact display

A question with references gets a small capsule immediately to the left of its bubble, bottom-aligned
with the bubble:

```text
[ 📎 4 ]  问题气泡
```

- Count = passages + regions + pages + previous answers + images + files + web — every attached
  context item, not just the visible text.
- The capsule uses the surface background (white in light mode), a thin light-purple border and a
  full pill radius. The paperclip is followed by the number.
- Hovering exposes the full list, including the count, through the native title.
- No capsule for a question with no references; the bubble keeps its normal right alignment.
- The live question uses the same capsule from `current_context`, so references are visible while the
  answer streams.

## Acceptance

- First answer no longer rebuilds the transcript per token; the live turn node stays stable.
- Markdown and citations are parsed incrementally while tokens arrive.
- Copy does not appear during streaming; copy/regenerate appear when the turn is complete or stopped.
- A question with references displays a bottom-aligned paperclip/count capsule to the left of its bubble.
- Editing that question restores its passages, regions, pages, answer refs, images, files and web
  flag; resending records the same context on the new sibling.
- Old conversations load with empty contexts and no reference line.
- `just check` green.
