# 0013 — MVP completion: images, ranges, current page, stop

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("这次定目标一次做到 MVP，历史引用赎回也要做")

## Goal

Close the three gaps that keep the product from being usable end to end: a captured region reaches the
model as an image, a page range (or the current page, when nothing is selected) provides the context,
and a running answer can be stopped. After this, the five-item acceptance list from the MVP review
can be walked in one sitting.

## Non-goals

- No OCR engine (0012 stays on its own schedule; regions plus a vision model cover the scanned case).
- No history redemption (0003's second half, tracked separately).
- No streaming of images back from the model; answers are text.
- No retry button with backoff: one retry of the same request, user-initiated.

## Behaviour

**Images.** A region reference is sent as an image part after its page label. At most 6 images per
request; each is rejected above 4 MB of base64. A request with images must go to a model that accepts
them; when the provider rejects the image, the error names the image and the request is not retried
silently.

**Context.** A question carries, in order: passage excerpts, region labels, page-range text, and then
the question itself. When the tray is empty, the current page's text is used instead and the prompt
says so. When the current page has no text, the message says the page has no text rather than
inventing one.

**Stop.** While streaming, Send becomes `停止`. Stopping drops the stream, keeps the partial answer,
and marks it `已停止`. The next send starts clean.

**Retry.** After an error, `重试` resends the same question and references; the failed answer is
cleared only when the retry starts.

## Data model

```rust
pub struct Image {
    pub mime: String,
    pub base64: String,
}

pub struct Message {
    pub role: Role,
    pub content: String,
    pub images: Vec<Image>,
}
```

`Image::from_data_url` accepts `data:image/png;base64,…` and rejects anything else.

## Interface

`ask` gains `regions: Vec<{ page: u32, image: String }>` and `page: Option<u32>`,
`page_text: Option<String>` for the current-page default. Events gain `stopped`.

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Send | `发送` | composer | question non-empty, model configured | click | `⌘↵` | tap | stream starts; button becomes 停止 | — |
| Stop | `停止` | composer | streaming | click | `Esc` | tap | partial answer kept, marked 已停止 | — |
| Retry | `重试` | under the error line | the last request failed | click | `↵` | tap | the same request is sent again | repeated failure leaves the previous message |

**Alternatives considered.**

- Auto-retry. Rejected: it spends money without consent.
- Sending regions as base64 inside the text prompt. Rejected: models do not decode it; the protocol has
  an image part for a reason.

**Discoverability.** Stop replaces Send exactly where the eye already is.

**States.** Idle → streaming → done / stopped / failed (retry available).

**Cost of being wrong.** Sending six large images is expensive; the composer counts them and the
request is refused above the cap rather than truncated.

## Test plan

- Unit: a data URL parses to mime + base64; a non-image or empty payload is rejected; the OpenAI body
  puts images in `content` as `image_url` parts; the Anthropic body puts them as `image` blocks.
- Unit: more than six images is an error naming the cap.
- Manual: capture a formula, ask "这是什么", confirm the answer references it; select a range; ask with
  nothing selected and confirm the current page is used; stop mid-answer.

## Risks

- **Provider image limits differ.** Mitigation: the 6-image and 4 MB caps are below every provider's
  documented limit, and the error surfaces the provider's own message.
- **Base64 inflates memory.** Mitigation: images are dropped as soon as the request is built.

## Assumptions

- MiMo's OpenAI-compatible endpoint accepts `image_url` parts with a data URL; to be verified with one
  live call.

## Deviations

- The image caps are enforced in the desktop command, not in `pdfscope-ai`, because the limits are a
  product decision rather than a protocol one.
- Page text is fetched by the renderer and handed to Rust as plain strings; Rust never parses a PDF.
- A range is capped at 20 pages per request; a longer range now says so instead of truncating
  silently. The notice sits above the composer and is cleared on the next send.
- Retry reuses the send button (its label becomes `重试`) instead of a separate control.
- Stop is a shared flag checked between stream events; the in-flight HTTP request is not aborted, so a
  provider may still bill for a few more tokens.
- Answer tokens are coalesced into one reactive update per 50 ms. The answer re-parses citations on
  every change, so per-token updates were quadratic in the answer length.
- `⌘↵` sends and `Esc` stops, handled on the composer rather than globally so they never fight the
  page's own shortcuts.
