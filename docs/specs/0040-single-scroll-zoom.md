# One zoom engine with sub-pixel scroll residual

Date: 2026-10-26

## Decision

The three selectable zoom engines are removed. They shared the same event → rAF → scroll pipeline,
and the only differences were anchor bookkeeping and a short-lived transform, so they did not produce
a meaningful feel difference. The reader returns to the single per-frame scroll-following model that
felt direct, with two narrowly scoped compensations.

## Restore the per-frame baseline

Each zoom frame measures the document point currently under the pointer before changing the scale,
then measures again after scaling:

```text
pointY = (origin.y - pages.top) / displayScale
stage.scrollTop += pages.topAfter + pointY * newScale - origin.y
```

This is the original feel: the point under the pointer is recomputed every frame, and the page
follows the pointer continuously rather than being locked to a gesture-start anchor.

## Stop the downward drift

WebKit rounds `scrollTop` to whole CSS pixels. The original code therefore lost the fractional part
of every correction, and slow zoom walked downward. The visible page now uses two pieces:

```text
effectiveScroll = stage.scrollTop - scrollResidualY + delta
whole           = trunc(effectiveScroll)
scrollResidualY = whole - effectiveScroll
stage.scrollTop = whole
```

and the column gets:

```css
transform: translate3d(pageOffsetX, scrollResidualY, 0) scale(viewScale);
```

The residual is a screen-space fraction. It is carried from frame to frame, so the pointer mapping
stays exact without changing the per-frame baseline or adding a mode. It is well below a pixel and
is removed on document teardown.

## Horizontal placement without layout

The horizontal fit/anchor offset is also composited:

```text
left = anchor.x - stage.left - anchor.contentX * newScale
```

- a non-negative left value is the page's screen-space `translateX`;
- a negative left on an overflowing column is split into an integer `scrollLeft` plus the
  fractional `translateX`, so the visual position is exact.

No `margin-left` is written during zoom. The page scaler has `margin: 0`; all movement is either
scroll or transform. This removes the per-frame layout work that made the previous fixed-anchor
version feel worse than the original.

At fit width, `left = 0`, so the transform and scroll forms agree; crossing fit width cannot snap.

## Verification

Synthetic slow ctrl-wheel zoom from 100% to 200% at a fixed pointer:

```text
screen offset of the captured content point: < 0.001 px in x and y
```

The same test with the old margin implementation accumulated several pixels of horizontal drift and
a visible fit-width snap. The fixed-anchor implementation removed the drift but did not match the
per-frame feel; this version keeps the original per-frame measurement and only carries the
scrollTop rounding error.

## Removed

- `pdfscope.zoom-mode`
- `data-zoom-mode`
- the Reading “缩放方式” selector
- NSScrollView semantic and camera modes
