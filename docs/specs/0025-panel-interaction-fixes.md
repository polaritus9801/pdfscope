# 0025 — Reader and panel interaction fixes

**Status:** accepted.

Seven fixes reported after the visual refresh.

## 1. Tool shortcuts never fire while typing

`V/T/B/D` were guarded by `INPUT/TEXTAREA/SELECT` only. The composer is a `contenteditable`, so
typing `t` switched to the text tool and stole focus. The guard now uses
`closest("input, textarea, select, [contenteditable='true'], [role='textbox']")`.

## 2. A held modifier really borrows that mode

Holding `⌘/Ctrl`, `⇧` or `⌥` in pointer mode now **switches the renderer to that tool** and restores
pointer on release. The borrowed gesture therefore has exactly the sticky mode's behaviour — capture,
page gestures, cursor, wash and selection — not a visual approximation.

The first attempt called a free `setTool()` that does not exist in the renderer, so the keydown
threw before changing anything; it also never told Rust, leaving a focused reference's handles and
wash visible. The borrow now calls `PDFScope.setTool()`, reports the borrowed mode to Rust, clears
`focused_ref`/`active_ref` exactly like the rail does, and lights the borrowed rail icon. The borrow is
cleared on keyup, blur and external tool changes, and is ignored while an editable field has focus.

## 3. The page under the pointer stays lit while dragging

CSS `:hover` lost the page as soon as the marquee started. JS now marks the page under the pointer
(`.pointer-page`) on every stage/repage move, inside the auto-scroll loop and at gesture end, and the
wash uses that class as well as `:hover`. During a page drag exactly the page under the pointer is
lit; the first page turns off as the pointer enters the second, and edge auto-scroll cannot leave a
stale page lit.

## 4. Official vector marks and model identity

- Official square SVG marks are downloaded into `ui/public/brands` (Simple Icons for DeepSeek,
  Anthropic, Gemini, Xiaomi, Moonshot, OpenRouter and Ollama; the OpenAI 2025 symbol from Wikimedia
  Commons). Nothing is hand-drawn. Unknown providers keep a coloured monogram fallback.
- Every model row in the header popover carries its provider tile before the name.
- The settings model list renders one chip per parsed name, each with the provider tile, so the
  choice is visible before the name.

## 5. Composer field

The editor's own `:focus-visible` ring was the inner purple rectangle with two rounded and two square
corners. It is removed; the card's `:focus-within` ring is the only focus treatment. The card border
is now 1.5 px `--border-strong` and accent on focus.

## 6. Panel edge and collapsed rail

- The divider no longer paints a purple hover block; only the 1 px line turns accent.
- Collapsing hides the whole header. The expand control fills the 44 px rail with a centred chevron
  and no text, so the entire strip is the button and no characters are shown.

## 7. Smooth panel dragging

Previously every `pointermove` set a Leptos signal and queued a `restoreStageAnchor` rAF; the
ResizeObserver also refit pages repeatedly. Dragging now runs in JS and **scales the document continuously**, not just moves it:

- the panel width is written directly to the DOM at most once per animation frame;
- the document is visually scaled by `contentWidth / baseWidth` (preserving the reader's zoom level),
  with the page centre anchored, so the page size follows the divider in real time;
- canvas/text-layer updates are skipped during the drag; at release the new base layout is measured
  at the same visual size, so there is no snap;
- the stage ignores pointer events while dragging, so no hover/selection repaint;
- ResizeObserver checks are skipped during the drag and run once after it;
- the final width is reported to Rust, persisted and applied to the signal; a pointerup outside the
  window still finishes correctly.

`just check` green; visual and pointer behaviour need a human pass.


## Investigation — two pages lit during a 1→3 drag (not changed)

Two independent effects can make page 2 and page 3 look lit at the same time:

1. **The wash owns two sources.** `:hover` and the JS `.pointer-page` class can both be active, and
   the page wash has `transition: opacity 120ms`. Moving quickly from page 2 to page 3 starts page 2's
   fade-out and page 3's fade-in in the same 120ms, so both are visibly washed. A slow drag makes the
   overlap invisible, which matches the roughly 50% reproduction rate.
2. **Page 2 is also a range-preview.** Every page intersecting the marquee gets `.range-preview` with
   a 2 px accent outline. That is intended range feedback, but it reads as a second highlighted page
   next to the pointer wash.

WebKit can also keep `:hover` on the pointer-down page during a prevented drag, which would leave
page 2 hovered until release. The robust fix is to make the wash single-owner: remove the `:hover`
wash in page mode, add a `.page-gesture` class while dragging, and disable the opacity transition
while that class is present; `.pointer-page` remains the only page with a wash. That change is
deliberately not made in this commit.
