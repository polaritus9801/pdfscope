# 0025 — Reader and panel interaction fixes

**Status:** accepted.

Seven fixes reported after the visual refresh.

## 1. Tool shortcuts never fire while typing

`V/T/B/D` were guarded by `INPUT/TEXTAREA/SELECT` only. The composer is a `contenteditable`, so
typing `t` switched to the text tool and stole focus. The guard now uses
`closest("input, textarea, select, [contenteditable='true'], [role='textbox']")`.

## 2. A held modifier really borrows that mode

Holding `⌘/Ctrl`, `⇧` or `⌥` in pointer mode now sets `data-borrow="pages|text|box"` on the stage.
That gives the same cursor, the same page wash and the same pointer affordance as the sticky mode:
`⌘/Ctrl` shows the page pointer and wash, `⌥` the crosshair, `⇧` the text cursor. The borrow is
cleared on keyup, blur and tool changes, and is ignored while an editable field has focus.

## 3. The page under the pointer stays lit while dragging

CSS `:hover` lost the page as soon as the marquee started. JS now marks the page under the pointer
(`.pointer-page`) on every stage/repage move, and the wash uses that class as well as `:hover`. During
a page drag exactly the page under the pointer is lit; the first page turns off as the pointer enters
the second.

## 4. Official vector marks and model identity

- MiMo uses Xiaomi's orange square with the white `mi`; Moonshot keeps the crescent, DeepSeek the
  whale, OpenAI the knot, Anthropic the spark, Gemini the star, unknown providers a coloured
  monogram. Everything is SVG, square and scalable.
- Every model row in the header popover carries its provider tile before the name.
- The settings model list renders one chip per parsed name, each with the provider tile, so the
  choice is visible before the name.

## 5. Composer field

The editor's own `:focus-visible` ring was the inner purple rectangle with two rounded and two square
corners. It is removed; the card's `:focus-within` ring is the only focus treatment. The card border
is now 1.5 px `--border-strong` and accent on focus.

## 6. Panel edge and collapsed rail

- The divider no longer paints a purple hover block; only the 1 px line turns accent.
- Collapsing hides the whole header. The expand control fills the 44 px rail, vertically centred with
  a vertical 展开 label, so the entire strip is the button.

## 7. Smooth panel dragging

Previously every `pointermove` set a Leptos signal and queued a `restoreStageAnchor` rAF; the
ResizeObserver also refit pages repeatedly. Dragging now runs in JS:

- the panel width is written directly to the DOM at most once per animation frame;
- the stage keeps the same content point under its centre by setting `scrollLeft` in the same frame;
- the stage ignores pointer events while dragging, so no hover/selection repaint;
- ResizeObserver checks are skipped during the drag and run once after it;
- the final width is reported to Rust, persisted and applied to the signal; a pointerup outside the
  window still finishes correctly.

`just check` green; visual and pointer behaviour need a human pass.
