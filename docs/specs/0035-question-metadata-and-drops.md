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
- Same-day values render `HH:MM`; another day this year `MM-DD`; an older day `YYYY-MM-DD`.
- The native title keeps the full `YYYY-MM-DD HH:MM` timestamp.
- Old turns without timestamps show neither label.

## Paste and composer drops

`Composer` now hands clipboard files to the existing attachment input; the Rust shelf then runs the
same `readAttachments` path as the paperclip button.

- Image and text/PDF/Office files are supported by that existing reader.
- Plain-text paste still inserts text.
- Dropping files on the AI panel attaches them to the composer.
- Dropping on the reader opens the first PDF.
- Dropping a non-PDF on the reader shows `阅读器只支持打开 PDF；其他文件请拖到右侧 AI 面板`.
- Drop targets get a purple outline while a file is dragged.

Tauri's native drag interception is disabled (`dragDropEnabled: false`) so the webview receives real
`File` objects. Dropped PDFs therefore open through the reader's normal file input; the bytes path is
used and the recents entry has no native path.

## Acceptance

- Copy-question exists beside edit and copies only the question.
- Ask and completion times survive reload; formatting follows the rules above.
- Pasting an image/file into the editor adds a composer attachment.
- Dropping an image/file on the panel adds it to the composer.
- Dropping a PDF on the reader opens it; an unsupported file shows the notice.
- `just check` green.
