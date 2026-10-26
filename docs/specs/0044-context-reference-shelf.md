# Context reference shelf

**Status:** accepted.

Date: 2026-10-27

## Layout

The composer's context shelf is one ordered surface:

1. screenshots and images in a single square media strip;
2. text and prior-answer references as full-width pale-purple rows, each text clamped to three lines, the whole section height-capped with vertical scrolling;
3. file cards in a horizontal strip: type icon, name, formatted size;
4. an always-visible page-reference row in a pale-purple rounded container: page icon + “页面引用” button opening the existing PageManager, followed by a one-line synchronized page-range input;
5. web-source cards below the page row.

Media items use the order they were added. Screenshots carry a purple crop badge; external images carry the supplied purple image badge. File type icons come from the copied DSH file-icon set. The OOBE PDF mark uses `file-type/pdf.svg`; the old icon implementation is retained temporarily.

## Page input

`page_set: Vec<u32>` remains the source of truth. The input displays canonical ranges (`1-3, 6-8`), accepts ASCII/Chinese commas, whitespace, `-` and `–`, and commits atomically only when the whole input is valid and within `1..=pages`. Invalid input displays an error and leaves the committed set unchanged. External updates (PageManager, reader page clicks, clear, undo) re-render the input unless the field has an unsaved draft. Each valid commit pushes one undo entry. Empty input clears the set.

## Data

`UserFile` gains a `size: u64` with serde default for old conversations. `readOneAttachment()` reports the original byte size. The file card formats it as B/KB/MB; old records show no size.

## Constraints

- Existing reference click/hover/focus/drag/remove behavior is preserved.
- Source links stay visually distinct from local files.
- The old composer corner “管理页面引用” button is removed; the page row owns it.
