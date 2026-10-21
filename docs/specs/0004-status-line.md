# 0004 — The status line

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("继续做")

## Goal

The reader can tell, at a glance and without opening anything, what the model is doing and what it
cost: how fast it is producing, how much of the context is used, and whether the prompt was served
from cache. One line of small text at the bottom of the AI panel, always present, never in the way.

## Non-goals

- No settings, no model switching, no spend dashboard. Those belong elsewhere.
- No charts, no history graph, no per-question breakdown beyond a hover card.
- No numbers the provider did not report. A missing value is `—`, never a plausible-looking guess.

## Behaviour

The line is laid out left to right, most useful first:

```
deepseek-chat · 上下文 8.4k / 128k · 缓存 1.2k · 42.1 tok/s · 首字 0.8s
```

| Field | Source | When unknown |
|---|---|---|
| model | the configured model | — |
| context used | prompt + completion tokens, or the local estimate before a response | shows the estimate with `≈` |
| cache | provider-reported cached prompt tokens | `—` |
| tok/s | completion tokens ÷ generation time, rolling 3 s | `—` while idle after a turn, last value during |
| time to first token | measured locally, request → first delta | `—` |

During streaming the line updates every 500 ms; `tok/s` is a rolling average so it does not flicker
with every token. When a request fails, the line keeps the last known vitals and appends the failure
in `--danger` for five seconds, then returns to normal.

Numbers use tabular figures so the line does not jitter as digits change.

## Data model

No persistence. A view model:

```rust
pub struct Vitals {
    pub model: String,
    pub context_used: Option<u32>,
    pub context_window: Option<u32>,
    pub cached_tokens: Option<u32>,
    pub tokens_per_second: Option<f32>,
    pub first_token: Option<Duration>,
}
```

All fields optional: the line renders what exists.

## Interface

No new endpoint. Values derive from the streaming response: provider `usage` where present, local
timestamps otherwise. A provider that reports nothing yields an empty line rather than a fabricated
one.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Status line | — | bottom edge of the AI panel | always | hover opens a card with exact numbers and the provider's raw usage | focusable with `Tab`; the card opens on focus | tap opens the card | card lists the same values with full precision | no interaction beyond reading; `Esc` closes the card |

**Alternatives considered.**

- A collapsible debug panel. Rejected for v1: it is opened once out of curiosity and then never
  again; the line already answers "is it slow, is it expensive".
- Nothing at all. Rejected: without it, a slow answer and an expensive one are indistinguishable,
  and the reader cannot learn to write cheaper questions.

**Discoverability.** It is simply there. The hover card exists for people who want the exact numbers,
not as the only way to see them.

**States.** Idle (last turn's values, muted), streaming (values live), error (last values + a red
message), no model configured (single line: `未配置模型 · 前往设置`).

**Cost of being wrong.** A wrong `tok/s` teaches the reader the wrong lesson about their setup.
Mitigation: the rolling window is stated in the hover card, and the value is labelled `≈` when
derived from a local estimate rather than provider usage.

## Test plan

- Unit: rolling average over a token stream; missing usage yields `—`; formatting of large counts
  (`8.4k`, `128k`, `1.2M`) and of sub-second first-token times.
- Integration: the line updates during streaming and settles after `done`; an error appends and
  clears after five seconds.
- Layout: the line never wraps; on a narrow panel it truncates from the left, keeping `tok/s` and
  `cache` visible.
- Accessibility: the line is `aria-live="off"` — it must not be announced on every update — and the
  hover card is reachable by keyboard.

## Risks

- **Numbers can be false precision.** `tok/s` depends on prompt length, cache, and network. The hover
  card says what was measured and over what window.
- **A status line invites scope creep** into a control panel. The rule: no interactive control other
  than the hover card without a new spec.

## Assumptions

- The OpenAI-compatible providers used here report `usage` at the end of a stream; DeepSeek and
  Ollama do. If a provider streams without usage, the estimate is labelled `≈`.
- `tokens_per_second` is meaningful only for generation, not for time-to-first-token.

## Deviations

*(empty)*

## Open questions

1. Should the line also show the **estimated cost** of the last turn when pricing is configured?
   Proposed: yes, later, once a pricing table exists; it is the number people actually want.
