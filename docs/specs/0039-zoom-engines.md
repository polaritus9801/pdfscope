# Selectable zoom engines

Date: 2026-10-26

## Product surface

The Reading settings expose **缩放方式** with three modes. The choice is stored in
`localStorage["pdfscope.zoom-mode"]`, mirrored to `data-zoom-mode`, and applied immediately through
`window.PDFScope.setZoomMode`. There is no Tauri-only branch, so the same setting is available when
the reader ships as a web service.

The default is `scroll`, matching the existing reader and the lowest-risk behaviour.

## `scroll` — PDF.js-style scroll anchoring

The page column stays the scrollable object. A zoom gesture captures the document point under the
pointer and keeps it there while the visual scale changes. Scale changes ease over frames (except
pinch, which follows the fingers), and horizontal placement uses the single margin/scroll model
described in [0038](0038-reader-density-ruler-toggle-zoom-anchor.md): the fit-width transition does
not switch layout modes and therefore does not snap.

This is the PDF.js `updateScale({origin})` model: an origin point, not the viewport centre.

## `magnify` — NSScrollView semantics (web simulation)

This mode models `NSScrollView.setMagnification(_:centeredAt:)` semantics:

- magnification is applied immediately; there is no eased target animation;
- the document point under the pointer/centring point is preserved;
- the scroll view remains responsible for clamping and panning.

It is deliberately labelled **“NSScrollView 语义”**, not “native”. A WKWebView or a browser cannot
call the macOS NSScrollView API; this is the closest web semantic implementation, using the same
scroll container and the same unified horizontal placement as `scroll`, but with direct
magnification instead of easing.

## `camera` — temporary compositor camera

`camera` does not write layout or scroll offsets during the gesture. On the first frame it freezes
the current page layout and applies one affine transform to the rendered page column:

```text
translate3d(screenX - contentX * scale - layoutLeft,
            screenY - contentY * scale - layoutTop, 0)
scale(scale)
```

The anchor is captured at the gesture start. A moving two-finger centroid updates the screen anchor
while keeping the captured content point fixed, which is simultaneous zoom and pan. When the target
settles (120 ms), the camera transform is removed, the final scale is committed to the regular
scroll layout, and the scroll offsets are restored from the same anchor. This gives a
compositor-style preview without reflowing pages per frame.

Only the gesture is camera-transformed; selection, page wrapping, PDF.js rendering and the page
ruler continue to use the normal layout. This is the web-compatible equivalent of the camera model;
it is not a native platform view.

## Mainstream references

- [PDF.js `updateScale` origin](https://github.com/mozilla/pdf.js/commit/b7933d8750835d5743c4aa724b408701ded6588f)
- [PDF.js smooth trackpad pinch](https://github.com/mozilla/pdf.js/commit/9899ae0453fb5952f472df4688f7467a68b15aca)
- [Chromium PDF pinch zoom / `pinchPanVector`](https://codereview.chromium.org/2400743002)
- [`NSScrollView.setMagnification(_:centeredAt:)`](<https://developer.apple.com/documentation/appkit/nsscrollview/setmagnification(_:centeredat:)>)

## Limits

- The browser does not expose native macOS `NSScrollView` magnification. `magnify` is a semantic
  simulation on the web path.
- Trackpad input differs by engine: Safari may deliver `gesture*` events with a moving centre;
  Chromium commonly exposes pinch as `ctrl + wheel`. The camera mode uses the standard gesture
  centre when available and wheel deltas otherwise. True simultaneous pinch-and-pan on every
  trackpad/browser stack requires a native scroll view or a browser-level compositor.
- The camera preview is visual only during the gesture; page ruler marks and virtualised page
  selection catch up when the scale commits.
