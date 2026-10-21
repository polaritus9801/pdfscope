# 0007 — Model providers and streaming

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("你把 openai 和 anthropic 以及主流厂家接口都做一下")

## Goal

Any mainstream model can be configured in one place and streamed through one interface: OpenAI and
everything that copies it (DeepSeek, MiMo, Moonshot, OpenRouter, Ollama, LM Studio, Gemini's
compatibility endpoint), plus Anthropic's native Messages API. A provider that returns reasoning
tokens or cache statistics reports them separately instead of hiding them in the text.

## Non-goals

- No settings UI in this slice. Configuration is a value; where it is stored is 0008.
- No tool calling, no structured output, no vision input. Those arrive with the features that need
  them.
- No retries beyond one reconnect-free attempt; no rate-limit backoff yet.
- No token counting before the request; the provider's usage is reported, the estimate is local.

## Behaviour

Two wire protocols cover every target:

| Protocol | Providers | Endpoint shape |
|---|---|---|
| OpenAI Chat Completions | OpenAI, DeepSeek, MiMo, Moonshot, OpenRouter, Ollama, LM Studio, Gemini (compat) | `POST {base}/chat/completions`, SSE `data:` frames ending in `[DONE]` |
| Anthropic Messages | Claude, MiMo's Anthropic endpoint | `POST {base}/messages`, SSE `event:` + `data:` frames ending in `message_stop` |

Presets supply the base URL so a user picks a name, not a URL:

| Preset | Base URL | Protocol |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | OpenAI |
| Anthropic | `https://api.anthropic.com/v1` | Anthropic |
| DeepSeek | `https://api.deepseek.com/v1` | OpenAI |
| MiMo | `https://api.xiaomimimo.com/v1` | OpenAI |
| Moonshot | `https://api.moonshot.cn/v1` | OpenAI |
| OpenRouter | `https://openrouter.ai/api/v1` | OpenAI |
| Gemini (compat) | `https://generativelanguage.googleapis.com/v1beta/openai` | OpenAI |
| Ollama | `http://127.0.0.1:11434/v1` | OpenAI |

Streaming yields, in order: zero or more `Reasoning`, zero or more `Text`, at most one `Usage`, then
`Done`. A stream that ends without `Done` is an error, not a silent success.

## Data model

```rust
pub enum Protocol { OpenAi, Anthropic }

pub struct ProviderConfig {
    pub label: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub model: String,
    pub api_key: Secret,
}

pub struct ChatRequest {
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
}

pub enum StreamEvent {
    Reasoning(String),
    Text(String),
    Usage(Usage),
    Done,
}

pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cached_tokens: u32,
}
```

## Interface

```rust
pub async fn stream(
    config: &ProviderConfig,
    request: &ChatRequest,
) -> Result<impl Stream<Item = Result<StreamEvent, AiError>>, AiError>;
```

Both protocols are normalised into `StreamEvent`, so nothing above this crate knows which one is in
use.

## Interaction design

No interface in this slice: it is `pdfscope-ai` only. The settings surface is 0008.

## Test plan

- SSE parsing: a frame split across two reads, a comment line, an empty `data:` line, and a trailing
  `[DONE]` are all handled without losing text.
- OpenAI: `delta.content`, `delta.reasoning_content`, and a final usage chunk map to the right
  events; a stream that stops early is an error.
- Anthropic: `content_block_delta`, `message_start` input tokens and `message_delta` output tokens
  map correctly; `message_stop` ends the stream.
- Both: a non-2xx response surfaces the provider's error message, not a generic failure.
- Live: one call to MiMo `mimo-v2.6-flash` streaming a short answer, confirming `reasoning_content`
  is reported as `Reasoning` and not mixed into `Text`.

## Risks

- **Provider dialects drift.** Mitigation: each protocol has one parser and one mapper; a provider
  quirk is a test, not a branch.
- **SSE frames split across network reads** are the classic parser bug. Mitigation: the parser keeps
  a buffer and only consumes complete frames.
- **A key leaking into logs.** Mitigation: `Secret` redacts itself, and request logging records the
  model and URL but never the headers.

## Assumptions

- MiMo reports reasoning in `reasoning_content` and cache hits in `prompt_tokens_details.cached_tokens`
  (verified with a live call on 2026-10-21).
- Anthropic reports `input_tokens` at `message_start` and `output_tokens` at `message_delta`.

## Deviations

- `reqwest` 0.13 renamed its TLS feature: `rustls-tls` no longer exists, so the client is built with
  `rustls` + `rustls-native-certs`.
- MiMo accepts `stream_options.include_usage` and returns the totals in a final chunk whose `choices`
  array is empty. Verified live; the mapper therefore reads usage before considering choices.
- Anthropic sends `data: {}` on some events; the parser tolerates it instead of failing the stream.
- The live test is a separate integration test guarded by `MIMO_API_KEY`, so `cargo test` stays
  offline and free.

## Open questions

1. Should `max_tokens` default per provider (Anthropic requires it; OpenAI does not)? Proposed: always
   send it, default 2048.
2. Should a reasoning stream be shown by default or collapsed? Proposed: collapsed, one line, because
   it doubles the time to a readable answer.
