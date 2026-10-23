# 0023 — The AI panel, second generation

**Status:** accepted. Decisions: the three scope controls stay three side-by-side icons, redrawn;
the composer uses inline pills; the whole program lands in dependency order, not in separate
approvals.
**Supersedes:** the composer and scope parts of 0022; it does not change the reader.

## 1. What is wrong today

The panel is a single column of stacked rows inside a floating rounded card. It works, but it
has no model of what a "context attachment" is: text quotes, region shots and page sets live in
three unrelated places, the composer cannot point at them, the answer cannot become context, and
the history, files and web search of every mature assistant are absent. The three scope glyphs
are decorative rather than explanatory.

## 2. Inventory: what mature assistants have

| Capability | Seen in | Decision |
|---|---|---|
| Conversation history, search, rename, pin | ChatGPT, Claude, Gemini | P2 — persistent, per document + global |
| Edit a question and resend (branch) | ChatGPT, Claude | P2 — replaces the turn from there |
| Attach files and images | all | P3 — PDF page shots already exist; add images and text files |
| Web search with citations | ChatGPT, Gemini, DeepSeek, Kimi | P4 — toggle; needs a search provider |
| `@` mentions of context | Notion AI, Gemini, ChatGPT | P1 — core of this spec |
| Quote an answer as new context | Claude, ChatGPT | P1 — see §3.4 |
| Token / context meter | dev tools, DSH | P4 |
| Export, copy, share | all | P3 |
| Prompt library | DSH, Claude | P4 |
| Temporary chat | ChatGPT | P4 |
| Canvas / artifacts, voice, memory, deep research | varies | **Not doing** |

## 3. Reference model v2

### 3.1 One attachment type

```text
Attachment =
  Passage { id, page, quote, rects }        // text selected in the PDF
  Region  { id, page, image, rect }         // box captured in the PDF
  Page    { page }                          // a page added by number
  Answer  { id, turn, quote }               // a previous assistant answer (or part of it)
  File    { id, name, mime, bytes }         // attached file (P3)
  Web     { id, title, url, snippet }       // search result (P4)
```

`Composer` holds an ordered `Vec<Attachment>`. Order is insertion order and only changes when the
reader drags a chip. Sending uses that order, and the payload numbers attachments from 1.

### 3.2 Identity and duplicates

- Every attachment carries a stable id for its lifetime.
- Adding the same thing twice is refused: passages compare `(page, start, end)`, regions compare
  `(page, rect)`, pages compare `page`, answers compare `(turn, range)`.
- Undo of an add restores the previous order.

### 3.3 The `@` mention

The composer becomes a `contenteditable` field. An attachment inserted with `@` or by dragging a
chip becomes a non-editable inline token: a small pill with its kind glyph, page and a short quote
(e.g. `▤ p.20 “coordinate frames…”`). The pill *is* the mention; there is no parallel numbering in
the text.

Rules:

| Situation | Behaviour |
|---|---|
| Type `@` | A picker opens at the caret: existing attachments first, then "add current page", "add selection", "pick a page". Filter while typing. `Esc` closes. |
| Pick an attachment | Its pill is inserted at the caret, the picker closes, no duplicate is created. |
| Delete a pill (Backspace or ✕) | The attachment is removed and one undo entry is recorded. |
| Delete the attachment from the shelf | Its pill is removed in the same undo entry; text around it is untouched. |
| Drag a chip into the text | A pill is inserted at the drop caret; dragging reorders only when dropped on the shelf, not in the text. |
| Reorder chips | Numbers are positions, so they follow the order; pills show labels, not numbers, so they never go stale. |
| Send | The question text and the attachment list are both sent; pills serialize to `[引用 n]` markers in the text the model sees, plus the numbered excerpts after the question. |
| Edit a sent question | Pills are restored as pills; removing one removes it from that request only. |

Why contenteditable: a textarea cannot contain inline objects. Chromium supports
`contenteditable="plaintext-only"`; paste is reduced to text, Enter sends, Shift+Enter breaks.

### 3.4 Answers as references

Every answer gets `引用此回答` in its hover actions. It creates an `Answer` attachment whose quote
is the whole answer or, when text is selected inside the answer, the selection. Pills render with
`✦ 回答 #3` and the first line. Answers are not re-fetched: the quote travels with the request, as
passages do. Editing the original answer later does not rewrite the attachment (it is a quote).

## 4. Panel shell

- The panel is a **flush column**: full height from below the header to the window bottom, square
  corners, no margin, no shadow, `border-left: 1px solid var(--border)`.
- The divider *is* the resize affordance: an 8 px transparent strip centred on the border, cursor
  `col-resize`; on hover or drag a 1 px accent line replaces the border. No separate purple bar.
- Collapsed state becomes a 44 px strip with the same border.
- Width is clamped to `[320, 720]`, double-click resets to 380, and the value persists.

### 4.1 Resizing without losing the place

On press, record the document point at the centre of the stage:

```text
anchor = (scrollLeft + viewportWidth / 2, scrollTop + viewportHeight / 2)
```

On every frame of the drag, apply the new width and restore `scrollLeft/scrollTop` so that the
same content point is at the centre again. In fit-width mode the page reflows, so the horizontal
anchor is the page's own centre instead; vertical anchoring is unchanged. The result is that the
text under the reader's eye does not slide while the panel grows.

## 5. Layout

```text
┌ panel ─────────────────────────────┐
│ document · model · history · new   │  header, 44 px
├────────────────────────────────────┤
│ transcript (scrolls)               │  turns, reasoning, actions
├────────────────────────────────────┤
│ context shelf                      │
│  ▤ quotes, horizontally scrollable │
│  [thumb] [thumb]  images row       │
│  p.12 p.13  page chips             │
├────────────────────────────────────┤
│ composer                           │
│  @  attach  web  scope      send   │
└────────────────────────────────────┘
```

- **Text quotes** are one line each, chips in a horizontal scroller; clicking scrolls the PDF to
  the quote; ✕ removes.
- **Regions** are 64×48 thumbnails in their own horizontal strip; click opens the page at the box.
- **Pages** are compact chips `p.12`, ranges collapse to `p.12–14`.
- The shelf only takes space when it has content; at most two rows are shown and the rest is a
  "+3" button opening the full list.
- The composer keeps a single visual input; tools sit in a row beneath it, so the writing area is
  never crowded.

## 6. Scope, redesigned

The three glyphs are replaced by a **context control** that says what it does. Options in §7.

Whatever is chosen, it must show counts (`全文 · 67 页`, `本页 · p.20`, `引用 · 3`), open with a
description of each mode, and stay valid when the state changes (an empty reference scope says so
before sending).

## 7. Decisions

1. **Scope control** — three side-by-side icons, redrawn: stacked sheets, a folded page, a
   highlighter; one segmented container, an accent segment for the active mode, and a count badge
   on the reference icon only. Descriptions live in the tooltip (`全文 · 67 页`).
2. **Composer** — contenteditable inline pills, the shelf as the source of truth.
3. **Sequence** — the program lands as one effort in dependency order; the first slice is the shell,
   the resizer and the scope control (shipped with this draft), then the attachment model.

## 7b. Progress

- **Shell, resizer, scope control** — landed. Flush column, border-as-divider with a 9 px drag strip,
  content-anchored resize, three redrawn icons in a segmented control with a reference count badge.
- **Composer** — landed as a contenteditable editor (`ui/public/composer.js`): attachment pills are
  non-editable spans, `@` opens the picker above the field, choosing an item inserts a pill at the
  caret, Backspace removes a pill and its attachment (one undo step), paste is reduced to text, and
  Enter sends. Pills serialise as `@{kind:id}`; the interface turns them into `[引用 n]` in the order
  passages → regions → pages, and an orphaned marker becomes `[引用已移除]`.
- **Context shelf and answers** — landed. One horizontally scrolling row per kind (文字 / 截图 /
  页面 / 回答) instead of stacked rows; each chip jumps to its page and removes its attachment; a
  region shows as a thumbnail. Every answer now offers `引用此回答`, which adds a `✦` answer chip,
  is undoable through the same snapshot, appears in the `@` picker, and is appended to the request as
  `[引用的先前回答]` with numbered quotes.
- **Drag-to-insert and partial answer quotes** — landed. Every shelf chip is draggable; dropping it on
  the editor inserts its pill at the drop caret (the renderer converts the point to a caret with
  `caretRangeFromPoint`). `引用此回答` quotes the text selected inside that answer when there is a
  selection, otherwise the whole answer.
- **Picker keyboard and edit-and-resend** — landed. The picker responds to ↑/↓ (wrapping, with the
  active row highlighted) and Enter picks; Esc closes it. Every turn has 编辑, which puts its question
  back in the editor behind a banner and, on send, replaces that turn and everything after it.

## 8. Gaps beyond the first inventory

A second pass over mature assistants adds these, which the program should cover rather than
discover later:

- **Message-level actions**: copy, copy with citations, retry, edit-and-resend, branch from here,
  quote as reference, delete turn.
- **Conversation management**: history list with search, rename, pin, delete, per-document and
  global scopes, resume after restart.
- **Composer ergonomics**: drag files anywhere on the panel, paste an image, `⌘Enter` send,
  `Shift+Enter` newline, autogrow, IME-safe, token meter, stop while streaming.
- **Context control**: show exactly what will be sent before sending; a dry-run preview; warn on
  truncation; per-attachment cost in tokens.
- **Reading-specific**: "summarise this page", "ask about selected region", follow-up suggestions,
  citation hover cards that show the source line without jumping.
- **Search and files**: web search with cited results, attached images/text, page screenshots as
  first-class attachments.
- **Trust**: show provider and model on every turn, retry policy, error categories, never lose a
  streamed answer.

These join the phases below rather than forming a second spec.

## 9. Phases

| Phase | Delivers |
|---|---|
| P1 | Flush panel, resizable with anchored centre, context shelf, new scope control, attachment model, `@` pills, answers as references |
| P2 | Conversation history (persisted, searchable), edit-and-resend, pin |
| P3 | Image/text file attachments, export markdown, copy answer with citations |
| P4 | Web search with citation cards, token meter, prompt library, temporary chat |

## 10. Risks

- `contenteditable` caret and IME handling are the main source of bugs; the shelf remains the
  source of truth and serialization is one function, so a textarea fallback stays possible.
- Persisted history needs a schema before P2; it must record attachments by value, not by pointer.
- Web search depends on a provider and adds cost; it is opt-in per turn.
