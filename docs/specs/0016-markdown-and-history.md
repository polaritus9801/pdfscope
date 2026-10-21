# 0016 — Markdown that behaves, and a conversation that stays

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("先做 Markdown 升级 + 对话历史")

## Goal

Answers render as real Markdown rather than a hand-written subset, and the panel keeps the
conversation instead of replacing it on every question.

## Markdown

`pdfscope-core::markdown` now wraps `pulldown-cmark` (CommonMark, plus tables, strikethrough and task
lists). Two things are ours:

- **Raw HTML is escaped**, never passed through; the answer is untrusted text.
- **Citations become links.** Consecutive text events are buffered first, because `[p.31]` arrives as
  five separate events and is only recognisable once joined. Code spans and blocks are excluded.
- Links keep only `http`, `https` and `mailto`; anything else loses its destination.

## Conversation

- Each finished exchange is kept: question, answer, reasoning and its duration.
- The panel renders a transcript, newest at the bottom; the streaming turn is appended and becomes a
  finished turn on the next send.
- The last four turns are sent with the request so pronouns resolve, truncated by role rather than by
  size: the prompt labels them as context, not as evidence.
- The input clears after sending.

## Non-goals

- No persistence of the conversation between launches.
- No editing, branching or deleting a turn.

## Test plan

- Unit: nine Markdown tests — escaping, emphasis, headings, lists, tables, task lists, code blocks,
  citations inside and outside code, link schemes.
- Manual: ask three questions and confirm the earlier answers remain, and that a follow-up resolves
  "这一段" to the previous turn.

## Math

`$…$` and `$$…$$` are extracted before parsing, rendered with `latex2mathml`, and substituted back
through a private-use placeholder; fenced code is left alone and invalid LaTeX falls back to its
source. Inline math typesets correctly. Display math is centred but its internal layout depends on
the browser's MathML implementation, and constructs like `\max \sum` can place the operator awkwardly
— if that proves unacceptable, KaTeX is the fallback.

## Deviations

- Models write `1.` on its own line, then a blank line, then the item's text. CommonMark reads that
  as a loose list with an empty first paragraph, which rendered as a large gap between the number and
  its text. `to_html` now joins a bare marker to its first line before parsing.
- `.answer-text` kept `white-space: pre-wrap` from the plain-text era; with HTML output it turned the
  newlines between block tags into visible blank lines. It is `normal` now.


- The transcript does not auto-scroll yet; the panel keeps whatever scroll position it had.
