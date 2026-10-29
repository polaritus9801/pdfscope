# File previews and rendered code blocks

**Status:** accepted.

Date: 2026-10-29

## Decision

Use the bundled highlight.js distribution (BSD-3-Clause) for syntax highlighting; no network
requests. `ui/public/code.js` enhances every rendered `pre > code` in answers, transcripts and the
reference preview, and adds one copy control at the block's top-right. The original text is copied,
not the highlighted DOM.

The reference file preview is format-aware using the bytes already extracted for the attachment:

- Markdown is rendered with the existing safe CommonMark renderer and its fenced code is highlighted;
- CSV/TSV become a scrollable table (quoted cells, header row, 200-row preview limit);
- JSON is pretty-printed and highlighted;
- source-like extensions use a highlight.js language class;
- PDF/Office extracted text and plain text remain readable preformatted text;
- images continue to use the image preview.

The preview header shows the extension and original byte size when known. Every preview header uses
the shared panel close button.

## Theme

The settings sheet exposes a separate code theme: Follow System (default), Light or Dark. It is
stored under `pdfscope.code-theme` and applies `data-code-theme` to the document root, affecting
both answer code blocks and file previews. The setting is independent of the interface theme.

## Acceptance

- A Rust/Markdown/CSV/JSON preview selects the correct renderer without executing file HTML.
- Code blocks in streamed and completed answers are highlighted and have a copy control.
- Copying a code block writes its raw source.
- Long CSV tables scroll rather than expanding the dialog.
