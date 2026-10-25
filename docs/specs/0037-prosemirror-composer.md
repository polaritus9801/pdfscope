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
Its default state is an accent ring; when live Markdown is on it becomes filled. Clicking toggles
live Markdown and stores the choice in `localStorage["pdfscope.composer.markdown"]` (first run is
plain). The mode only changes decorations: the document is always the raw Markdown source, so
turning it off/on never rewrites or loses anything.

In live mode ProseMirror decorations render Markdown in place while the source characters stay in
the document. Delimiters are hidden only while the cursor/selection is outside that span; moving the
caret into the text (or onto a heading/quote/list line) reveals the delimiters again, so `**bold**`
and every other marker remain editable, Obsidian-Live-Preview style. The serialized question, draft
and prompt history contain the raw Markdown.

Supported now:

- inline: `**strong**`, `__strong__`, `*em*`, `_em_`, `~~strike~~`, `` `code` ``,
  `[label](url)`
- blocks: `#`–`######`, `>` block quotes, `-`/`*`/`+` and `1.` lists, fenced ``` code blocks

Not yet supported: tables, images, task checkboxes, footnotes, math and nested emphasis. Code fences
stay visible (rather than hidden) so they remain easy to edit.

Mod-B/Mod-I/Mod-E wrap/unwrap the source selection in `**`, `*` or backticks by inserting literal
markers. The placeholder is inset 12px, matching the editor content padding.

## Resizing

The top edge has an invisible 8px drag handle with a hover line. Dragging it changes the editor
`--composer-min-height`; the manual height is a **floor**, not a fixed height. Content still
auto-grows above it and shrinks back only to the floor. Dragging to the current minimum or
double-clicking the handle returns to auto. The handle is keyboard accessible (↑/↓, Home/Enter).
The baseline minimum stays the current 56px content area.

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
