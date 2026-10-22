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

`latex2mathml` was tried first and rejected: it does not understand `\le`, `\ge`, `\mathbb` or
`\dots`, so ordinary answers produced `[PARSE ERROR: …]` in place of the formula.

KaTeX now typesets maths in the browser. The Markdown renderer leaves `$…$` and `$$…$$` untouched;
`ui/public/katex.js` observes the transcript, renders with KaTeX's auto-render extension, and
disconnects the observer while KaTeX mutates the DOM so its own insertions cannot retrigger it.
Only the WOFF2 fonts are vendored (596 KB for the whole KaTeX directory); code fences are untouched
because KaTeX skips `pre`/`code`.

## Fix: formulas survive Markdown

Reported from a real session: formulas appeared as raw LaTeX, often with `_` and `\` eaten. The cause
was ordering — Markdown parsed the formula before KaTeX saw it, so `v_{rel}` became emphasis and
`[p.3]` inside math became a citation.

`to_html` now lifts `$$…$$` and `$…$` out **before** parsing, leaves inert ASCII tokens in their place,
and puts the escaped formula back afterwards; KaTeX's auto-render then finds intact delimiters.
Six tests cover underscores and asterisks, multi-line display math, citations inside math, escaped
dollars, unpaired dollars, and HTML characters.

## Deviations

- A request that fails before any content is retried twice automatically (0.5 s, then 1 s), and the
  status line says `连接中断，正在重试（n/3）…` while it does; the line clears as soon as text arrives. Once text
  has arrived a retry would duplicate it, so that case is handed to 继续生成 instead. Stop is honoured
  between attempts.


- A stream that dies after text has arrived reports `连接中断，已保留生成的内容（…）` instead of a raw
  transport error: the partial answer is kept, and the wording says so. The cause is the connection,
  not the output budget — long streams simply have more time to be cut.


- Every `ask` ends in an event. A rejected invoke used to leave the promise dropped and the panel
  waiting forever; the bridge now turns a rejection into an `error` event and ends a stream that has
  produced nothing for two minutes.
- A finished turn is archived when the stream ends, not when the next question is sent. Before this,
  an answer that ended empty (the model spent its budget on reasoning) left no trace at all: the
  panel showed its "ask something" hint as if nothing had happened. The question and reasoning now
  stay on screen, and the status line says `模型只返回了思考过程，没有正文`.


- Models write `1.` on its own line, then a blank line, then the item's text. CommonMark reads that
  as a loose list with an empty first paragraph, which rendered as a large gap between the number and
  its text. `to_html` now joins a bare marker to its first line before parsing.
- `.answer-text` kept `white-space: pre-wrap` from the plain-text era; with HTML output it turned the
  newlines between block tags into visible blank lines. It is `normal` now.


- The transcript does not auto-scroll yet; the panel keeps whatever scroll position it had.
