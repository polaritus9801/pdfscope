# 0036 — Mention picker and attachment ownership

**Status:** accepted.

## Floating picker

The `@` mention picker is absolutely positioned above the composer:

```css
.mention-picker {
  position: absolute;
  right: var(--space-3);
  left: var(--space-3);
  bottom: calc(100% + 6px);
  max-height: min(240px, 42vh);
}
```

Opening it therefore does not change the composer/context-shelf height or push the transcript
upward. It is positioned just above the caret, centred on the `@` when possible and shifted inward at the AI
panel edges; width is capped at 300px. It scrolls internally and closes when the `@` trigger stops
being valid, including deleting the `@` or typing another character.

## Token insertion

`makeToken(key, label)` now creates the pill directly:

```html
<span class="mention" contenteditable="false" data-key="p:3">@p.3</span>
```

The previous implementation called `makeToken` from inside itself, so choosing a picker row
overflowed the stack and never inserted a visible token.

## Attachments own references

The context shelf is the owner of references, not the inline token.

- Deleting an `@` pill from the editor removes only that text occurrence. The corresponding passage,
  region, page, image, file or answer stays in the context shelf and is still sent.
- Tokens have no trailing space and no zero-width guards. Clicking the left/right half places the
  caret before/after the pill; Backspace/Delete adjacent to a pill removes the pill and keeps the
  caret outside.
- Removing a reference from the shelf keeps its token, marks it red as an orphan and shows a small
  warning on the send button. The reader can delete the red pill or confirm sending via the centered
  “是 / 修改” dialog; “修改” focuses just after the first red pill.
- Clear-context still clears both the shelf and all tokens.

## Answer time

`.turn-time` is 11px (previously 10px); layout and colour are unchanged.

## Acceptance

- Typing `@` after existing text opens the picker; it floats above the caret, capped at 300px and
  clamped inside the AI panel.
- Choosing a picker item inserts a visible `@...` pill.
- Deleting that pill leaves the shelf count/reference intact.
- Removing the shelf reference turns the pill red, shows the send warning and asks before sending.
- Hovering an `@` pill, an @ picker row, or a shelf chip highlights the corresponding PDF location and
  outlines the shelf chip; clicking either the pill or shelf chip locates it and leaves the chip
  outlined.
- The capsule has a hover treatment; its popover exposes `+ 全部`, which adds only missing historical
  references or shows the red already-present notice.
- Answer completion time renders at 11px.

## How an inline reference reaches the model

The pill is a UI-only object. Its `data-key` uses the same key as the context shelf:

```text
p:<id>     passage excerpt
r:<id>     captured region
page:<n>   page reference
a:<id>     previous answer
file:<id>  text/file attachment
img:<id>   image attachment
```

`Composer.serialise()` turns each pill into `@{key}` and returns the key list. On send, the panel
builds an ordered list of all active keys (passages, regions, pages, answers, files, images) and
`inline_question` replaces each marker with `[引用 N]`; a marker with no active key becomes
`[引用已移除]`.

The actual model request is not that marker text alone:

- `references`: `{page, quote}` for passage excerpts, rendered as `[摘录 p.N]` blocks.
- `images`: region/image data URLs with page numbers, rendered as screenshot/image notes plus image
  parts.
- `pages`: `{page, text}` for the page bulk selected by the scope, rendered as `[第 N 页]` blocks.
- `history`: the last four question/answer pairs.
- `file` attachments are appended to the question as file blocks.
- web results are appended as web-source blocks before the question.

The Tauri `ask` command receives these arrays and `build_request` assembles the final user message:
pages first, then excerpts, screenshot notes, and finally `问题：…`. The `[引用 N]` markers are UI
numbering; the model reads the actual page/excerpt/file contents from the structured blocks and image
parts, not from the `@` token itself.
