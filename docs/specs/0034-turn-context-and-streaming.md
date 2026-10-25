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
  full pill radius. The paperclip is followed by the number. While its popover is open the capsule
  becomes #7f5cbf with white icon/count; only colours change, so nothing shifts.
- Hovering the capsule exposes the full list, including the count, through the native title.
- No capsule for a question with no references; the bubble keeps its normal right alignment.
- The live question uses the same capsule from `current_context`, so references are visible while the
  answer streams.

### Reference popover

- Clicking the capsule opens one popover below it; clicking again, clicking outside (scrim), or Esc
  closes it. It stays open after a reference row is clicked, so several can be added.
- The popover is capped at 300px. It starts at the capsule's left edge and extends right; if that
  would cross the AI panel edge it shifts left only as far as needed, and on a narrow panel it is
  clamped to the panel width.
- The popover lists every stored reference with a kind icon, label and page/kind metadata.
- Hovering a passage, region or page sets the shared `hovered` highlight; the reader does **not**
  scroll. If that page is outside the viewport, a small page image appears beside the popover instead.
  Files, images, answers and web have no PDF rectangle and therefore no overlay.
- Clicking a row calls the same activation callbacks as clicking an active input reference: it adds
  the item to the current context shelf if absent, then jumps/flashes the page for page-bearing items.
  It never edits the historical turn.
- A duplicate is never added. The same red notice used by the composer (`range-error`) says the
  reference is already in the context; page-bearing duplicates still jump to the page.
- The popover header has a `+ 全部` button left of Close. It adds every historical reference not
  already in the active shelf (pages, text, regions, answers, images, files and web); if all are
  present it shows `这些历史引用已经在上下文里了`.
- Context-shelf chips and inline `@` pills share focus/locate callbacks. Hovering either highlights
  the PDF and outlines the matching shelf chip; clicking either scrolls to the source and leaves a
  persistent outline on the shelf chip. Clicking a shelf chip never re-adds it.
- The old "历史引用" list, `Stored`, `redeem` and its send-time bookkeeping were removed. Conversation
  history and prompt history are unaffected.

### Persistence

- `TurnContext` is part of `Turn` and is serialized in both `turns` and the conversation tree. New
  turns survive reload; `asked_at`/`completed_at` are serialized alongside it.
- Conversations created before this field have no context and cannot reconstruct it from old text;
  those questions intentionally show no capsule.
- The document load selects the newest `updated` conversation, ignoring pinning for the initial
  choice; pinning still orders the drawer.

## Acceptance

- First answer no longer rebuilds the transcript per token; the live turn node stays stable.
- Markdown and citations are parsed incrementally while tokens arrive.
- Copy does not appear during streaming; copy/regenerate appear when the turn is complete or stopped.
- A question with references displays a bottom-aligned paperclip/count capsule to the left of its bubble.
- The popover lists all references, keeps itself open on row clicks, and shares activation with input
  reference chips. Offscreen page hovers show a page thumbnail without scrolling.
- New conversations persist their contexts; opening a document resumes its newest conversation.
- Editing that question restores its passages, regions, pages, answer refs, images, files and web
  flag; resending records the same context on the new sibling.
- Old conversations load with empty contexts and no reference capsule.
- `just check` green.
