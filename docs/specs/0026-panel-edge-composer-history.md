# 0026 — Panel edge, composer polish and empty history

**Status:** accepted.

## 1. The light strip is the rail, not the panel or the scrollbar

A real DOM probe in headless WebKit finally identified the child list of `.workspace`:

```
stage (right 1054) | .rail (6 px, background surface-sunken) | .panel (left 1060)
```

The light purple band was the **page-position rail**: a six-pixel flex child between the reader and
the panel, with the same colour as `accent-soft`. It was never panel padding, the scrollbar, or the
resize handle, which is why changing those did nothing.

The rail is now the divider itself:

- width is 4 px, painted `--border`;
- a transparent 12 px `::before` overlays the stage edge as the drag hit area;
- the rail is `--border` at rest; `:hover` changes it to `#7F5CBF` (`--lilac-700`), and while
  `.resizing` is on the document it stays dark even as the pointer leaves the rail; the width stays
  4 px throughout;
- after release `.resizing` is removed and native `:hover` decides again; no pointer capture is used,
  so moving the pointer away clears the dark state;
- the panel's old invisible handle is retired (`display: none`), because it was clipped by
  `.panel { overflow: hidden }`;
- `:has(.panel.collapsed)` disables rail pointer events, so a collapsed panel cannot be dragged or
  turn purple; its resting colour is restored explicitly;
- no pointer capture is used; WebKit kept `:hover` pinned after a captured drag and left a purple
  band behind.

The stage's native scrollbar is restored, right padding is 0, and the panel left edge is 4 px from
the reader. A WebKit test measures the rail at 4 px at rest, 4 px on hover with only the colour
changing, and changes the panel from 380 px to 480 px on drag; the hover colour clears on release.

## 2. White corner tips in the composer

Reported behaviour: only while typing, never after paste; blurring and refocusing removes them; typing
brings them back. That is a repaint/compositing symptom, not a layout leak: WebKit promotes the
contenteditable surface to its own layer during caret updates, and that layer can escape the card's
rounded `overflow: hidden` at the top corners. The old 3 px lilac focus halo made the white tip
visible against the panel.

The card now:

- uses `clip-path: inset(0 round 14px)`, `contain: paint` and `isolation: isolate` to force the
  rounded clip;
- has no outer focus halo; focus is the inset 2 px accent border;
- keeps the white surface inside a rounded clip, so a composited child cannot show a square tip.

Headless WebKit screenshots after typing show clean corners; the Tauri webview needs the human pass.

## 3. No status/token row under the composer

The `composer-status` row (status text plus the token estimate) is removed, together with
`token_estimate` and its CSS. The composer card now sits at the bottom padding directly. `status`
itself stays in state because the error card still renders it.

## 4. Empty conversations are not history

- `saveConversations` serialises only conversations with at least one turn;
- loading discards stored empty conversations (and creates a transient empty one if nothing remains);
- 新对话 removes every empty scratch conversation before adding the new one;
- switching prunes the empty conversation being left, and deleting prunes empty leftovers;
- the history drawer filters empty rows as a final guard.

A conversation therefore enters history only when its first turn exists.

## Acceptance

- Panel expanded: 8 px strip, deep purple on hover, drag works anywhere on the strip.
- Collapsed: no strip highlight and no drag.
- Typing/backspace never reveals white corner tips.
- No status/token line under the input; card meets the bottom padding.
- New chat, close/reopen, and switch: no “新对话 · 0 轮” rows; an empty chat disappears when left.
- `just check` green.
