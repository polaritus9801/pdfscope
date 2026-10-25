# 0037 — ProseMirror composer

Status: implemented

## Why

The contenteditable composer used a `span` with `contenteditable=false` plus hand-written
selection/backspace patches. The caret could still be pushed into the pill's inline box, and the
one-pixel boundary workaround created new visual problems. The editor now uses ProseMirror's document
model instead of patching the browser selection.

## Schema

```text
doc       paragraph+
paragraph inline*
text      (plain Markdown source)
mention   inline, atom, selectable, group inline
          attrs: key, label, orphan
```

A mention is an **atom**: ProseMirror has no valid document position inside it. Clicking its left or
right half maps to a `TextSelection` at the corresponding boundary. Backspace/Delete and arrow keys
are handled by editor commands, so a pill is removed or skipped as a whole.

## Integration

- `ui/composer-src/index.js` is the editor.
- `ui/scripts/build-composer.mjs` bundles it with esbuild into `ui/public/composer.js` (classic
  script, loaded before `bridge.js` and `viewer.js`).
- `ui/package.json` / `ui/package-lock.json` pin ProseMirror and esbuild.
- The public `window.Composer` API is unchanged, so the Rust panel does not know which editor is in
  use.
- Serialisation remains `@{key}` plus the ordered `keys` array. The Rust-side prompt and AI payload
  do not change.

## Markdown live source mode

A small dot sits in the top-right corner of the composer card, tangent to its top and right edges.
Its default state is a white-centred 8px accent ring; hover fills it with light accent, and live
Markdown on fills it deep purple. Clicking toggles
live Markdown and stores the choice in `localStorage["pdfscope.composer.markdown"]` (first run is
plain). The mode only changes decorations: the document is always the raw Markdown source, so
turning it off/on never rewrites or loses anything.

In live mode ProseMirror decorations render Markdown in place while the source characters stay in
the document. Inline delimiters are hidden only for paragraphs whose line is not active; entering a
line reveals all of that line's syntax, so `**bold**` and every other marker remain editable without
per-span cursor flips. The serialized question, draft and prompt history contain the raw Markdown.

The composer takes its natural content height up to the automatic cap described under Resizing. A
previous mode-height floor kept the taller of the plain and decorated layouts, which avoided a small
jump but left a blank white strip under the shorter layout (very visible with stacked display
formulas); it was removed. The page bottom stays fixed because the PDF.js hidden canvas no longer
creates body overflow. While the composer is being dragged vertically the body uses an explicit
`ns-resize` cursor, never the horizontal panel-resize cursor.

Supported now:

- inline: `**strong**`, `__strong__`, `*em*`, `_em_`, `~~strike~~`, `` `code` ``,
  `[label](url)`, `$...$` and `\(...\)` formulas
- blocks: `#`–`######`, `>` block quotes, nested `-`/`*`/`+` and `1.` lists (Tab/Shift+Tab), fenced
  ``` code blocks, single-paragraph `$$...$$` / `\[...\]` display formulas

List markers stay visible and use a fixed hanging indent, so `1.`/`2.` lines and wrapped lines keep
the same text column. Nested lists carry their leading-space depth as `--md-list-indent`; Tab inserts
two spaces in a list (Shift+Tab removes up to two), while Tab outside a list keeps normal focus
navigation. Paragraph-level arrow movement jumps between block ends instead of mapping through
hidden marker widths.

Not supported: tables, images, task checkboxes, footnotes, multi-paragraph display formulas and
nested emphasis. Those are document-level GFM features; pretending to support them with regex spans
would recreate exactly the cursor/layout bugs this pass fixes. Full parity needs a dedicated Markdown
editor (CodeMirror + Lezer/Obsidian-style renderer), not a chat-input decoration layer.

## Formulas

The composer reuses the existing KaTeX bundle (already loaded for transcript answers); no new
dependency is added. On an inactive line, `$...$` / `\(...\)` is replaced by a cached KaTeX widget and
`$$...$$` / `\[...\]` gets a display widget. The raw LaTeX remains the document text and is shown
again as soon as the line is active; clicking a formula moves the caret to its source. Code spans and
fences suppress formula parsing. Invalid LaTeX is rendered by KaTeX with `throwOnError: false` rather
than blocking input. Currency false positives are acceptable because the reader can turn live preview
off with the dot.

Mod-B/Mod-I/Mod-E wrap/unwrap the source selection in `**`, `*` or backticks by inserting literal
markers. The placeholder is inset 12px, matching the editor content padding.

## Resizing

PDF.js's hidden measurement canvas is kept out of flow (`position: fixed`, offscreen). In normal
flow it added a 19px inline line box to `body`; focusing the composer could scroll `body` by that
amount and shift the whole app even when the composer itself did not change height. As a second line
of defence, `html`/`body` and `.app`/`.panel` declare `overflow: hidden` then `overflow: clip`, so
modern engines remove shell scroll containers while older ones still clip. The composer anchors shell scroll only on composer focus, mode
toggles and caret placement. There is deliberately no `position: fixed` body and no global scroll
listener: both were removed to avoid mobile-keyboard/print side effects and scroll jank while the PDF
viewer is scrolling.

There is one enforced automatic height cap: `--composer-auto-max`, computed as 30% of the AI panel's
height and refreshed by a `ResizeObserver` (CSS fallback `30vh`, clamped to at least the 56px content floor). Content grows naturally up to that
cap; above it the input scrolls internally instead of pushing the answer away. A caret restored at
the end of a long draft is scrolled into view.

The top edge has an 8px drag target whose visible line appears only while the handle itself is
hovered, keyboard-focused or being dragged. The line is 32 × 3 px, square-ended, and its colour
strictly follows the card border: `#dccff3` when unfocused, `#7f5cbf` inside `:focus-within`. Dragging it sets
`--composer-min-height`, a genuine floor: the user can make the input taller even when the content is
short. Automatic growth is still capped at the 30% panel allowance, but a manual floor raises the CSS
maximum to `max(--composer-auto-max, --composer-min-height)` so the chosen height is respected.
Double-clicking the handle removes the manual floor and returns to automatic height. The handle is
keyboard accessible (↑/↓, Home/Enter). The baseline minimum stays the current 56px content area.

## Behaviour

- `@` opens the picker; it closes when the trigger disappears.
- Clicking a pill selects before/after it; hover still drives the PDF preview.
- Backspace/Delete deletes a whole pill; arrow keys step over it.
- Enter still sends, Shift+Enter splits a paragraph, Escape still closes/stops.
- Prompt history, drafts, partial recovery, file paste/drop, chip drops, orphan styling and the
  warning dialog are preserved.
- Programmatic restores (`setPlainText`, `setRich`, clear) are excluded from undo history and do not
  open the mention picker.
- Paste is plain text; file paste attaches files; dropping an attachment chip inserts a mention node.
- Markdown mode is a render preference, not a document conversion; raw Markdown is always the
  stored/restored form.

## Build

```sh
cd ui
npm install
npm run build:composer
```

The generated bundle is committed so a checkout without npm dependencies can still run `trunk build`.
Run the command after changing `composer-src/index.js`.

## Acceptance

- No `\u200B` or boundary spans in the editor DOM.
- Caret before/after a pill is normal height at the text baseline, including with no text on either
  side.
- Clicking a pill cannot produce a document selection inside it.
- Backspace/Delete removes a pill whole; undo restores it; redo removes it again.
- @ picker, hover previews, orphan warning, file paste/drop and drafts still work.
- Typing `**bold**`, moving away and back never removes the markers; they can be edited directly.
- The top-edge drag sets a floor, auto-growth still works, and double-click restores auto height.
