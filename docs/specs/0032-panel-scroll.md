# 0032 — AI panel scroll: reading anchor and FOLLOW-END

**Status:** accepted.

## 1. Why the panel needs its own scroll policy

The transcript is a scrolling column whose text reflows whenever the panel width changes. Native
scroll anchoring cannot see streaming text; a fixed `scrollTop` lets the paragraph under the reader's
eye slide, while aggressive auto-scroll fights a reader who deliberately moved up. The panel therefore
owns two explicit behaviours: a width-change reading anchor and a FOLLOW-END state.

## 2. Centre anchor during panel resize

When FOLLOW-END is off:

1. Before setting a new panel width, find the smallest block (`p`, `li`, `pre`, heading, question,
   answer, thinking block, turn) at the vertical centre of `.panel-body`. Fall back to the nearest
   turn when the centre lands in a gap, then to a scroll-height ratio.
2. Record that element and the fraction of its height at the centre.
3. After the reflow, set `scrollTop` so the same element/fraction is at the centre again.
4. Repeat this on every drag frame and on the final rounded-width commit.

Only the vertical reading point is anchored; horizontal centring is already the panel layout's job.
`.panel-body` sets `overflow-anchor: none` so the browser cannot add a second, conflicting adjustment.

## 3. FOLLOW-END state machine

`panelFollowing` is the single source of truth. It starts **off** so merely resizing a restored
conversation does not yank the reader to the end. Binding the transcript when the reader is already at
the end arms it; binding while reading above the end leaves it off.

| Event | Transition / effect |
|---|---|
| Send, regenerate, edit-and-resend, continue | **Arm.** Set on, scroll to `scrollHeight`, bind observers. |
| Mutation or ResizeObserver while armed | Coalesce in a rAF and scroll to the new end. |
| Panel width changes while armed | Scroll to the new end after reflow. |
| Wheel up | **Disarm immediately**, before the first scroll event. |
| Scroll moves up and distance-from-end > 12 px | **Disarm.** |
| Scroll lands within 8 px of the end | **Arm and snap to end.** |
| Growth while disarmed | No scroll. |
| Scroll down but not near the end while disarmed | Stay disarmed. |

A send always re-arms, even if the reader was away; this is the explicit request to see the new answer.
The reverse is never automatic: new tokens never pull a reader who moved up back down. Returning to
the bottom is the only passive re-arm.

Observers: a `MutationObserver` on `.panel-body` and a `ResizeObserver` on `#transcript`. While
FOLLOW-END is on they scroll synchronously inside the observer callback, before the frame paints; a
deferred rAF left a one-frame gap that looked like a bottom-drag fight. Scroll handling is
direction-aware (`scrollTop` decreased), so a late event from an auto-follow cannot disarm because the
transcript grew after the scroll landed.

A panel drag is a **single-mode operation**: the mode is chosen at pointer-down (FOLLOW-END if the
transcript is at/near the end, otherwise the centre anchor) and scroll events caused by reflow cannot
toggle it midway. Width changes at the bottom therefore stay pinned to the end instead of alternating
between the two algorithms.

## 4. Interaction

- While armed, a panel resize prioritises FOLLOW-END.
- While disarmed, a panel resize uses the centre anchor.
- User scrolling up during streaming disarms; the ensuing panel resize keeps the reading point.
- The send path calls `PDFScope.panelSent()`; the JS owns the rest. No Rust-side scroll state and no
  per-token IPC.

## Acceptance

- Dragging the panel while reading static content keeps the paragraph/fraction at the vertical centre
  (measured 0.75 → 0.739 on a long answer).
- After sending, the panel is at the end and stays there while tokens arrive.
- Wheel-up or an upward scroll away from the end stops auto-follow; further growth does not move the
  panel.
- Scrolling back within 8 px of the end re-arms and snaps to the end; subsequent growth follows.
- Repeated panel drags with FOLLOW-END off do not accumulate drift.
- Dragging while at the end stays within 1 px of the end on every frame; no END/centre oscillation.
- `just check` green.
