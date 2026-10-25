# 0035 — Question metadata, copy and file drops

**Status:** accepted.

## Question copy

The completed question's action row (the edit pencil, visually under the bubble) gains a copy icon to
the right of edit. It copies **only the question text**, not the answer.

## Timestamps

`Turn` stores `asked_at` and `completed_at` in epoch milliseconds.

- The completed answer's action icons are followed by `completed_at` when present.
- Hovering a question row reveals `asked_at` above the paperclip capsule, right-aligned, with its top
  line at the bubble top.
- Same-day values render `HH:MM`; another day this year `MM-DD HH:MM`; an older day
  `YYYY-MM-DD HH:MM`.
- The native title keeps the full `YYYY-MM-DD HH:MM` timestamp.
- Old turns without timestamps show neither label.
- Copying a question shows `已复制问题`; toasts are content-width and centered, so a short toast is not
  stretched to the width of a longer one.
- Creating a new conversation focuses `#ask-editor` with the caret at the end.

## Paste and composer drops

`Composer` now hands clipboard files to the existing attachment input; the Rust shelf then runs the
same `readAttachments` path as the paperclip button.

- Image and text/PDF/Office files are supported by that existing reader.
- Plain-text paste still inserts text.
- Dropping files on the AI panel attaches them to the composer.
- Dropping on the reader opens the first PDF.
- Dropping a non-PDF on the reader shows `阅读器只支持打开 PDF；其他文件请拖到右侧 AI 面板`.
- Drop targets get a purple outline while a file is dragged.
- While files are dragged, only the region under the pointer dims and blurs: the PDF stage or the AI
  panel. A large white upload glyph (the supplied SVG) sits in the middle; the white caption at the
  bottom reads “打开文件” on the reader and “上传文件” on the panel. There is no third overlay state.

Tauri's native drag interception is disabled (`dragDropEnabled: false`) so the webview receives real
`File` objects. Dropped PDFs therefore open through the reader's normal file input; the bytes path is
used and the recents entry has no native path.

## Acceptance

- Copy-question exists beside edit and copies only the question.
- Ask and completion times survive reload; formatting follows the rules above.
- Pasting an image/file into the editor adds a composer attachment.
- Dropping an image/file on the panel adds it to the composer.
- Dropping a PDF on the reader opens it; an unsupported file shows the notice.
- Dragging over either side dims/blurs only that region and shows the correct white caption.
- Duplicate reference activation adds nothing and shows the red “已经在上下文里了” notice.
- Creating a new conversation focuses the composer.
- `just check` green.


## Orphan @ pills

Removing a reference from the context shelf no longer deletes the matching inline `@` pill. The pill
turns red and the send button gets the supplied red/white warning SVG at its lower-right corner
(50% of the previous size, 2pt further right, bottom-aligned, absolute, no layout change). If the reader sends with red pills:

1. The panel dims and shows a centered confirmation dialog with an X close button.
2. The actions are two equal-width buttons: `仍然发送` (white) sends anyway; `修改` (purple) focuses
   the composer immediately after the first red pill. The X has the same effect as 修改.
3. The dialog disappears when all red pills are deleted.

The composer does **not** use zero-width characters around pills. It uses one-pixel editable boundary
elements on each side; clicking the left/right half puts the native caret in that boundary, outside the
pill. Backspace or Delete adjacent to a pill removes the pill instead of letting the caret enter it. The field also nudges its scroll/paint pass after opening the @ picker so a
long, scrolled message does not leave a stale blank run beside the @.
