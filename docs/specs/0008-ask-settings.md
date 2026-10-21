# 0008 — Ask: settings, credentials, streaming

- **Status:** accepted
- **Author:** z1mLi
- **Date:** 2026-10-21
- **Approved by:** z1mLi ("继续")

## Goal

The reader can configure one model once, then ask a question about whatever is in the reference tray
and watch the answer stream into a panel beside the page — with reasoning folded away, usage reported
at the end, and the key stored in the operating system's keychain rather than in a file.

## Non-goals

- No multiple saved providers; one active configuration at a time.
- No OCR, no region images in the request yet: only passage quotes are sent.
- No history of past questions beyond the current session.
- No cost table; usage is reported in tokens, not money.

## Behaviour

- First run: the AI panel shows `未配置模型 · 前往设置`.
- Settings: pick a preset (or type a URL), protocol, model, and key. Saving writes the non-secret
  fields to a JSON file and the key to the keychain.
- Asking requires a non-empty question; the tray may be empty (then the question is about the
  current page), or carry passages.
- The answer streams into the panel: reasoning in a collapsed line, text as it arrives, a status line
  at the bottom with `tok/s`, first-token time, and token usage.
- An error replaces the status line with the provider's own message; the question stays in the input
  so nothing is lost.

## Data model

```rust
pub struct Settings {
    pub preset: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub model: String,
}
```

Stored at `<app config dir>/settings.json`. The key lives in the keychain under service
`app.pdfscope.desktop`, account `api-key`. The key is never written to the JSON file, never logged,
and never returned to the webview — the view exposes only `has_key: bool`.

## Interface

Tauri commands, one event channel:

```rust
#[tauri::command] fn load_settings(app) -> Result<SettingsView, String>
#[tauri::command] fn save_settings(app, settings: SettingsInput) -> Result<(), String>
#[tauri::command] fn ask(app, question: String, references: Vec<AskReference>) -> Result<(), String>
```

`ask` returns immediately and streams `ask:event` payloads:

```json
{ "kind": "reasoning", "text": "…" }
{ "kind": "text", "text": "…" }
{ "kind": "usage", "prompt": 900, "completion": 64, "cached": 512 }
{ "kind": "done" }
{ "kind": "error", "message": "provider returned 401: …" }
```

## Interaction design

| Control | Label | Location | Enabled when | Pointer | Keyboard | Touch | Result | Cancel / error |
|---|---|---|---|---|---|---|---|---|
| Open settings | `设置` | AI panel header | always | click | `Tab` + `↵` | tap | the settings sheet opens over the panel | `Esc` closes without saving |
| Preset | dropdown | settings sheet | always | choose | arrows | tap | base URL and protocol fill in | choosing "Custom" keeps the typed URL |
| API key | password field | settings sheet | always | type | — | type | stored only on save | saving with an empty key keeps the previous one |
| Save | `保存` | settings sheet | the form is valid | click | `↵` | tap | sheet closes; the panel header shows the model | keychain denied → inline message, settings not saved |
| Ask | `发送` | AI panel input | the question is non-empty and a model is configured | click | `⌘↵` | tap | the answer area clears and fills as the stream arrives | a second send is ignored while streaming |
| Stop | `停止` | where Send was | while streaming | click | `Esc` | tap | the stream is dropped; the partial answer stays | — |

**Alternatives considered.**

- Store the key in the JSON file. Rejected: a plain-text credential in the user's config directory is
  the most common way local tools leak keys.
- A full settings page with providers, temperature and limits. Rejected for this slice: one provider,
  four fields, one button.

**Discoverability.** The panel's empty state names the next action. `⌘,` also opens settings.

**States.** Unconfigured (settings hint), idle (input), streaming (stop button, status live), error
(provider message, input preserved).

**Cost of being wrong.** Sending the wrong tray costs tokens and misleads the reader. Mitigation: the
request is built from exactly the tray, and the panel header states the model before sending.

## Test plan

- Unit: prompt assembly puts each reference under `--- p.N ---` in order and never invents a page;
  an empty question is rejected before a request is made; settings JSON round-trips without the key.
- Manual: save settings with a MiMo key, reload the app, confirm the key survives while the JSON file
  contains only four fields; ask with one passage and see the page cited; kill the network mid-stream
  and see the error line.
- Security: `grep -r "sk-" <app config dir>` returns nothing.

## Risks

- **Keychain prompts on macOS** can confuse a first run. Mitigation: the error message names the
  keychain, not the API.
- **Streaming into a webview event channel** can outpace the UI. Mitigation: the panel batches
  deltas per animation frame.
- **The web build has no keychain.** It is out of scope here; the bridge reports "unavailable" and the
  panel says settings are desktop-only.

## Assumptions

- `keyring` 4.x uses the login keychain on macOS without extra configuration.
- Tauri's `withGlobalTauri` is acceptable for this slice; a typed client arrives with the web build.

## Deviations

- The keychain round-trip could not be exercised in this environment (no interactive keychain). The
  paths are compiled and the settings JSON is unit-tested; the first real save must be checked by
  hand on a desktop session.
- `tok/s` is estimated from the number of characters in the answer, because the stream delivers
  fragments, not tokens. It is shown with `≈`.
- The web build knows nothing about settings: `Bridge.loadSettings` returns `null`, and the panel
  reports "未配置模型" rather than pretending.
- `withGlobalTauri` is enabled for this slice; a typed client replaces it when the web build is real.
- A `测试连接` button sends one 16-token request before saving, using the typed key when there is one
  and the stored key otherwise. It exists because a settings sheet that cannot be checked turns a
  typo into a mystery.
- The answer carries `aria-live="polite"` and the composer has an `aria-label`, so a screen reader
  announces the stream instead of silently filling the panel.
- **The key moved out of the keychain** into its own file with `0600` permissions, after the first
  real session: a prompt on every launch is a worse trade for a local reading tool than a file only
  the user can read. The threat this gives up — another process running as the same user — is not one
  the keychain removed either.

## Open questions

1. Should the panel remember the last question per document? Proposed: with history, later.
