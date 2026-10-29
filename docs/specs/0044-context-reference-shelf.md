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

## Acceptance

- Screenshots and external images interleave by insertion order in one square, horizontally scrolling strip.
- A text or answer reference clamps to three lines; the list height caps and scrolls vertically.
- File cards show type icon, name and formatted size, and old conversations without size remain loadable.
- The page row is present with an empty set and stays synchronized with valid page-range edits.
- Invalid page ranges do not mutate `page_set`.

## Controls and shelf height

Every screenshot, image, text/answer, file and page-group reference carries four equally sized
20px buttons (16px marks): eye, pin, @ and X. Eye is present only when pin is active; its slot is
reserved so the others do not shift. Text/file/page rows order them linearly; screenshots and images
place them at the four corners and hide the type badge while hovered. @ inserts the reference token
at the composer caret (or end when unfocused); X removes it; pin persists it in a document-scoped
snapshot; eye toggles inclusion in the current send. Pinned references survive a send and reload;
unpinned ones are consumed. Images/files are stored through the same local snapshot (quota failure
shows a notice). Hover state is an inset border on the existing rounded rect; no outer ring or native
title tooltip is used.

The non-page shelf content sits in one vertical scroll capped at `min(30vh, 300px)`. The media strip
is sticky at its top so text cannot push images out of reach. The file strip, still one fixed row with
a narrow horizontal scrollbar, sits outside that scroll; the page row sits below it and is never part
of the scroll or the collapse. A small chevron button, four times the old width and absolutely centred
in the shelf header, hides all non-page content. The page row uses an inner white rounded input and
keeps the outer pale-purple container; its label is optically nudged up against the icon.

## Interaction refinements

The four actions are a fixed 20px square with 16px marks. Eye is absent from the DOM until pin is
active; there is no hidden placeholder. A pinned pin and its card use the gold accent (pinned
hover uses a deeper gold); an unpinned pin remains lilac on hover. Bordered cards change their
existing one-pixel border for hover/focus/selection; the borderless text row uses one inset ring.
The file card keeps its icon, name and size visible and shows the action tray, with its own border
and background, at the vertical centre on the right. Text/answer action groups align to the same
right edge as the page-row actions. The collapse control is absolutely centred in the shelf header
and is 88px wide. The page label is nudged one pixel up against its icon.

## Constraints

- Existing reference click/hover/focus/drag/remove behavior is preserved.
- Source links stay visually distinct from local files.
- The old composer corner “管理页面引用” button is removed; the page row owns it.
