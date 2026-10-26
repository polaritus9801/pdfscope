# Panel zoom keeps the viewport content anchor

**Status:** accepted.

Date: 2026-10-26

## Problem

At 200 %+ zoom the panel divider preserved the vertical reading anchor but left an arbitrary
`scrollLeft`. Narrowing the stage could therefore move the PDF right before the shrinking page
moved left, and it did not preserve the visible column the user was looking at. The previous fix
forced the PDF's own centre to the stage centre, which is also wrong: the page itself may be
off-centre in the viewport.

## Decision

At pointer-down capture the content point currently at the stage content centre as a fraction of
the page width:

```text
fraction = (stageContentCentre - page.left) / page.width
```

Every panel-resize frame, after the visual scale and vertical reading anchor are applied, set
`scrollLeft` so that the same fraction lands on the new stage content centre:

```text
desiredLeft = stageContentCentre - fraction * page.width
scrollLeft += page.left - desiredLeft
```

When the page fits horizontally, auto margins own its centring and `scrollLeft` stays zero. The
normal zoom path is unchanged. The base-layout commit reuses the same fraction after the reflow,
so release does not switch back to a different horizontal anchor.

## Acceptance

- A content point at the viewport centre remains at the viewport centre while a 200%+ page is
  scaled by the divider.
- A page that is already off-centre stays off-centre relative to the viewport; it is not forced to
  its own centre.
- Fit-width panel dragging still uses auto margins.
- Vertical reading anchor, page ruler and reference bands remain correct.
