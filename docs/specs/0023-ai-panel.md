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

## 7c. Progress — the conversation library

- Each document's transcript is stored in the webview's local storage under its file name and loaded
  when that document opens, so closing the app no longer loses the conversation.
- `新对话` clears the transcript and its stored copy; it refuses while a turn streams.
- **Multiple conversations** — landed. A document owns a library of `Conversation { id, title,
  updated, pinned, turns }`, stored under `pdfscope.conversations.<document>`; the old single
  transcript is migrated into the first conversation. The drawer offers search across titles,
  questions and answers, rename inline, pin (which sorts first), two-step delete, and new
  conversation. Pin/rename/delete and new conversations all persist immediately.
- Remaining in P2: branching from an answer (edit-and-resend covers the common case), pinning a whole
  document, and exporting a conversation.

## 7d. Progress — export, budget and citations

- `导出` in the history drawer copies the active conversation as Markdown (`transcript_markdown` in
  core).
- A quiet token estimate sits beside the composer status; `estimate_tokens` in core charges three
  characters per token plus a flat allowance per image and page. It is a budget hint, not a promise.
- Every turn gains `复制引用`: the answer plus a deduplicated list of its cited pages.

## 7e. Progress — image attachments

- The composer has a paperclip control that reads one or more images as data URLs. They appear in
  their own shelf row as thumbnails, can be inserted as `@` pills, are counted by the token estimate
  and are removed by ✕ or by deleting their pill.
- Images travel through the existing region channel with `page: 0`; `build_request` now labels them
  `[附加图片，见附带的图片]` and gives attachment-specific size/format errors instead of talking
  about page 0. A desktop test covers the label.
- Images live only in the session (they are not written into persisted conversations), and adding an
  image is not part of the reference undo snapshot. Both are deliberate for now.

## 7f. Progress — temporary chat and branching

- `临时` in the history drawer starts a conversation that is never written to storage: it has no
  record in the library, the sync effect ignores it, and switching away discards it. The header shows
  a 临时 badge so the state is never implicit.
- `分支` on any answer creates a new stored conversation containing that turn and everything before
  it, titled after its parent with a `· 分支` suffix, and switches to it.

## 7g. Progress — text files

- The same paperclip accepts text files (`txt`, `md`, `csv`, `json`, `log`, `yaml`); each is read as
  text (200 KB limit) and appears in its own shelf row with its character count.
- Files are inserted as pills through `@` or by drag, count toward the token estimate, and each one
  is appended to the question as `[附件文件 name]` with a per-file 8 000-character limit
  (`file_block` in core, four tests). They live for one send and are cleared afterwards.
- Adding a file is not yet part of the reference undo snapshot, same as images.

## 7h. Progress — web search

- Settings → 接口 gains 搜索 API Key（Tavily）; the key is stored in its own `search-key` file with
  0600 permissions, never returned to the webview (`has_search_key` only).
- `pdfscope-ai::search::tavily` isolates the provider (endpoint, result shape, snippet limit) so
  another provider can replace it; six tests cover result parsing, missing titles/URLs, the five-result
  cap and snippet truncation.
- A globe toggle beside the composer turns search on for the next question. The command searches with
  the typed question, appends up to five numbered `[来源 n]` blocks to the model context, and keeps the
  title/URL pair in a 来源 shelf row. Clicking a chip opens it in the system browser.
- Failures are explicit: no key, browser build, or a provider error stops the turn with a message and
  does not fall back silently. An empty result set is reported and the question proceeds locally.
- The 来源 chips are the citation cards. Inline `[来源 n]` markers inside an answer are now linked:
  `link_source_markers` (four tests) rewrites them into spans carrying the escaped URL, and the
  transcript click handler opens them. Hover preview cards are still not built.
- **Providers** — DeepSeek Search is the default (it reuses the model key; official endpoint
  `POST https://api.deepseek.com/v1/tools/search`, per-request billing). Tavily, Brave, Exa and Serper
  are selectable in 设置 → 接口. Each is parsed into the same `WebResult` and covered by parsing tests;
  unknown ids fall back to DeepSeek. Search keys live in `search-keys.json` (0600), one entry per
  provider, and the old single Tavily key file is migrated on read.

## 7i. Progress — per-answer citations

- `Turn` now carries the `sources` it was allowed to cite (`#[serde(default)]`, so old stored
  conversations still load). The archive step copies the live source list into the turn and clears
  the shelf row.
- Every answer renders its own numbered citation chips under the body; clicking one opens the URL in
  the system browser. The 来源 shelf row remains only for the turn in flight.
- Because sources live in the turn, switching conversations, branching, retrying and reloading keep
  the cards attached to the answer that used them.

## 7j. Review pass

Three state bugs found by reading the combined flows:

- **Renaming could erase itself.** Enter committed and closed the field; the blur that followed read an
  empty buffer and committed `新对话`. Blur now commits only when the same row is still renaming.
- **A failed search consumed references.** The move into “历史引用” happened before the search; a
  missing key therefore emptied the shelf for a request that never ran. References move only after the
  search succeeds.
- **A failed search erased the draft.** The composer was cleared when the button was pressed, not when
  the request started. Clearing now waits for `request_started`, which the request path bumps after
  the search; a failure leaves the draft, the pills and every attachment in place.
- Switching conversations, branching and editing are refused while a turn streams.
- The deferred clear is armed only when `send_text` really started streaming; an early return
  (missing configuration, empty question) cannot leave a stale clear waiting for the next request.

## 7k. Progress — rich files

The paperclip now accepts what mainstream assistants accept, extracted in the renderer:

| Kind | How | Notes |
|---|---|---|
| Images | data URL | 25 MB limit |
| PDF | PDF.js text per page | page markers `[附件第 n 页]`; scans produce an explicit error |
| DOCX | ZIP + `word/document.xml` | paragraphs from `w:p` / `w:t` |
| PPTX | ZIP + `ppt/slides/*.xml` | one section per slide |
| XLSX | ZIP + shared strings + sheets | `A1: value` rows |
| Text / code | UTF-8 | 40+ extensions, 25 MB |

Office files are read with a small ZIP reader (central directory, stored and raw-deflate via
`DecompressionStream("deflate-raw")`) and `DOMParser`; no new dependency. Extraction is capped at
200 000 characters, and each file is folded into the request with a 20 000-character
`[附件文件 …]` block. Unsupported kinds and unreadable files produce a specific message.

## 7l. Progress — budget guard and prompt history

- 设置 → 接口 gains 上下文上限（tokens）, default 128 000, zero disables it. The check runs **after**
  the page text is fetched and before anything is consumed: the request is refused with its estimate,
  and the draft, references and attachments stay in place for a retry.
- The composer recalls previous questions with ↑ at the start of the field and ↓ back down; the list
  keeps the last 30, is de-duplicated against the immediately previous send, and persists in local
  storage.

## 7m. Progress — drafts and duplicate references

- Every conversation stores its own draft (`pdfscope.draft.<document>.<conversation>`), written 300 ms
  after typing stops. Switching restores it as **rich content**: markers are turned back into pills
  when their attachment still exists and dropped when it does not, so a restart with no attachments
  cannot produce orphan pills. Sending a question or deleting a conversation removes its draft;
  temporary conversations are never written.
- The same character span (passage) or the same rectangle (region) is refused instead of becoming a
  second identical attachment; pages were already deduplicated by the page-set algebra.

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


## 11. Manual acceptance

Run these against the desktop build with a configured model and (for §6) a Tavily key.

### Composer
1. Type `@`: the picker opens above the field. `↑`/`↓` wrap the highlight, Enter inserts, Esc closes.
2. Choose a text reference: a lilac pill appears at the caret; typing continues after it.
3. Select text in the answer, click 引用此回答: the pill quotes only that selection.
4. Drag a shelf chip into the middle of a sentence: the pill lands at the drop point.
5. Backspace over a pill: the attachment leaves the shelf; ⌘Z restores it once.
6. Enter sends, Shift+Enter breaks a line, IME composition never sends mid-word.
6b. ↑ at the start of an empty field recalls the previous question; ↓ walks back down and clears past
    the newest; the history survives a restart.
7. Delete the attachment with the shelf ✕: the pill disappears too, one undo step.
7b. Type a draft, switch conversations and return: text and pills are back; send it and the draft is
    gone; restart with no attachments and the text returns without orphan pills.
7c. Select the same span twice: the second attempt adds nothing.

### Shelf
8. Text, region, image, file, page and answer rows appear only when non-empty and scroll sideways.
9. Clicking a chip jumps to its page; ✕ removes it; the panel width never changes.

### Answers
10. Copy copies the body; 复制引用 appends `参考：p.n…`.
11. 重试 resends; 编辑 puts the question back with a banner and replaces that turn and later ones;
    分支 creates a new conversation containing that turn and earlier ones.
12. A web answer keeps its numbered source cards after switching conversations and after restart;
    clicking a card opens (or at minimum exposes) the URL. A `[来源 n]` marker inside the answer is
    itself clickable and carries the same URL.

### History
13. 新对话 adds a conversation; search matches titles, questions and answers; rename survives blur and
    Enter; pin sorts first; delete needs a second click.
14. Switch, restart, and reopen the document: the same conversations and pinned order return.
15. 临时 shows the badge, is absent from the list, and disappears when switched away.
16. 导出 writes the active conversation as a `.md` file into Downloads (a second export becomes
    `name (1).md`); the toast shows the path. In a browser build it falls back to copying Markdown.

### Files and images
17. Attach two images, a `.md`, a PDF, a DOCX, a PPTX and an XLSX: each appears as its own row and is
    removable; the token hint grows. Scanned PDFs and unsupported kinds give a specific error.
18. Send: images arrive as pictures; document and text text is labelled `[附件文件 …]` with page or
    slide markers where they apply.

### Web search
19. Without a key, the globe search ends the turn with a message and keeps the draft and references.
20. With a key, an answer shows numbered 来源 cards; an empty result says so and still answers.
21. Turn the globe off: the next request does not search.
21b. Set 上下文上限 low, send a whole-document question: the send is refused with an estimate and the
     draft and references remain; raising the limit lets it through.

### Panel
22. The panel is flush with the window edge, its left border is the divider, and dragging it keeps the
    document's centre in place. Double-click the divider to reset it to 380; the width survives a
    restart.
23. The scope control shows three icons, an accent segment and a reference count badge; tooltips carry
    `全文 · N 页` and counts.

### Runtime-only checks
These cannot be verified by `just check` and need a human: caret behaviour in the contenteditable
field, pill deletion across wrapped lines, the real Tavily response shape, and window.open behaviour
for source cards in the desktop webview.


## 12. Progress — export to a file

`导出` now calls a Tauri command that sanitises the conversation title into a filename, refuses to
overwrite an existing export (`name (1).md`, …), writes UTF-8 into Downloads and returns the path,
which the toast shows. Four unit tests cover forbidden characters, empty titles, the length cap and
unique paths. Browser builds fall back to the clipboard.

## 12. Deviations

- **Chip reordering is not built.** Dragging a chip always inserts a pill into the question; dropping it
  on another chip does not change the shelf order. Insertion order is the send order.
- Duplicate detection covers passages and regions; attaching the same image or file twice still
  produces two attachments (the `@` picker reuses existing ones on the common path).
- Export writes into the system Downloads directory; there is no folder picker yet.
- **Source cards open via `window.open`.** If the desktop webview refuses new windows, both the cards
  and inline markers are inert; there is no clipboard fallback for them yet.
- **Images and files are outside the reference undo snapshot**; passages, regions, pages and answers
  are inside it.
