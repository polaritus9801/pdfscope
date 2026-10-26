# One zoom engine with sub-pixel scroll residual

Date: 2026-10-26

## Decision

The reference behaviour is commit `0208612` (the last good zoom). Commit `fe6ee81` replaced horizontal
point tracking with `centerHorizontally()`, so horizontal zoom always snapped to the page/stage
centre and changed the fit-width geometry. The three later modes were built on that broken baseline
and are removed. The implementation below restores the `0208612` model and adds only a sub-pixel
carry for the browser's integer scroll positions.

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

## Horizontal placement: restore 0208612

The horizontal point is measured per frame just like the vertical one:

```text
pointX = (origin.x - pages.left) / displayScale
```

While the column still fits the padded content box, auto margins centre it and `scrollLeft` is zero.
Once it truly overflows, the scroll position is set from the pointer point; it is never forced back
to the centre at the end of the frame or at settle. This is the exact 0208612 distinction:

```text
fit:      auto margins + scrollLeft = 0
overflow: pointer-driven scrollLeft, leave it there
```

The stage keeps the original 116 px padding gutters, not inset overlays. The page column is centred
in the padded content box.

Two browser-rounding details are carried in the page transform:

- the fractional `scrollTop` remainder (below);
- the fractional part of the horizontal target (and, on the first overflowing frame, any amount
  beyond the still-stale scroll range). `scrollLeft` is an integer; the remainder goes into
  `pageOffsetX`.

```css
transform:
  translate3d(pageOffsetX, scrollResidualY, 0)
  scale(viewScale);
```

While the page fits, `pageOffsetX` is zero; auto margins own centring. It only carries sub-pixel
horizontal scroll values or the first-frame overflow clamp. This does not reintroduce per-frame
margins or a camera.

At fit width, the auto-margin form and the overflow-scroll form agree, so crossing fit width does not
snap.

## Verification

Synthetic slow ctrl-wheel zoom from 100% to 200% at a fixed pointer:

```text
screen offset of the captured content point: < 0.001 px in x and y
```

The page's horizontal position is also exact while crossing the fit boundary. The three modes,
margin-based placement and fixed gesture anchor are not present.

## Removed

- `pdfscope.zoom-mode`
- `data-zoom-mode`
- the Reading “缩放方式” selector
- NSScrollView semantic and camera modes
