# 0029 — Panel action cleanup

**Status:** accepted.

## Layout

- The model row beneath a question carries the branch navigation and the icon-only edit on its right,
  vertically centred with the model name.
- Turn padding is reduced (`4px 0 8px`, gap 6 px), tightening the gap between an answer and the next
  question bubble.
- Branch arrows have no button chrome: transparent at rest and hover, 15 px chevrons, count in small
  tabular type.

## Actions

- “从这里开始一条新对话” is removed with its `branch_conversation` code; the tree replaces it.
- Conversation history gains a copy icon (copies the active conversation as Markdown) and the delete
  cross becomes a trash icon; the two-click confirmation remains.
- “复制回答并附上引用页码” is removed with its code; plain copy remains.
- Regenerate is a permanent icon on every completed answer. It creates a sibling at that node; it is
  not limited to the last turn or errors.
- The branch action no longer appears in answer actions.

## Composer

- The attachment control uses the supplied paperclip SVG (`viewBox="0 0 16 16"`).
- The three tools are optically matched: the 24-grid at glyph renders at 18 px, the globe at 20 px,
  and the 16-grid paperclip at 16 px. Their visual bounds are all about 15 px and their centres share
  the 28 px button's centre, so the top and bottom edges align.
- Their rendered wire is also matched: the paperclip's filled wire is 1.4004 px (1.05 pt) thick. The
  at glyph uses SVG stroke-width 1.87 at 18 px, and the globe 1.68 at 20 px; both render at 1.4 px
  (1.05 pt). No shape is redrawn.
- Editing recalls a question with the caret at the end (set synchronously and again on the next frame,
  because WebKit restores its old caret after focus).

## Acceptance

- Thinking timer visibly advances during reasoning, stops at the first answer token, and the collapsed
  pill is compact.
- Branch/edit controls align with the model name; the name stretches until the branch controls and
  only then truncates; arrows have no grey background.
- Panel scroll surfaces use a 4 px scrollbar in both axes.
- History has a copy icon and a trash delete; no branch-to-new-chat action remains.
- Regenerate exists on every answer and creates a sibling.
- Paperclip is the 16 px supplied path; the three tool icons align.
- Clicking edit leaves the caret after the last character.
- `just check` green.
