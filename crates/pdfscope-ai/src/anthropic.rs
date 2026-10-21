//! Anthropic Messages.

use futures::StreamExt;
use serde_json::{Value, json};

use crate::sse;
use crate::{AiError, ChatRequest, EventStream, ProviderConfig, Role, StreamEvent, Usage};

/// Anthropic requires a version header; this is the only one in use.
const API_VERSION: &str = "2023-06-01";

/// Accumulates the two usage frames Anthropic sends, because they arrive separately.
#[derive(Debug, Default)]
pub(crate) struct Totals {
    prompt: u32,
    completion: u32,
    cached: u32,
}

impl Totals {
    /// Consumes one frame, returning whatever the frame contributes to the answer.
    pub(crate) fn apply(&mut self, event: Option<&str>, data: &Value) -> Vec<StreamEvent> {
        match event {
            Some("content_block_delta") => data
                .pointer("/delta/text")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(|text| vec![StreamEvent::Text(text.to_owned())])
                .unwrap_or_default(),

            Some("message_start") => {
                self.prompt = count(data, "/message/usage/input_tokens");
                self.cached = count(data, "/message/usage/cache_read_input_tokens");
                Vec::new()
            }

            Some("message_delta") => {
                self.completion = count(data, "/usage/output_tokens");
                Vec::new()
            }

            Some("message_stop") => vec![
                StreamEvent::Usage(Usage {
                    prompt_tokens: self.prompt,
                    completion_tokens: self.completion,
                    cached_tokens: self.cached,
                }),
                StreamEvent::Done,
            ],

            // ping, content_block_start/stop, and anything the protocol adds later.
            _ => Vec::new(),
        }
    }
}

fn count(data: &Value, pointer: &str) -> u32 {
    data.pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}

fn body(config: &ProviderConfig, request: &ChatRequest) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("model".to_owned(), json!(config.model()));
    body.insert("max_tokens".to_owned(), json!(request.max_tokens));
    body.insert("stream".to_owned(), json!(true));

    if let Some(system) = &request.system {
        body.insert("system".to_owned(), json!(system));
    }
    if let Some(temperature) = request.temperature {
        body.insert("temperature".to_owned(), json!(temperature));
    }

    // Anthropic takes no system role in the message list.
    let messages: Vec<Value> = request
        .messages
        .iter()
        .map(|message| {
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            };

            if message.images.is_empty() {
                return json!({ "role": role, "content": message.content });
            }

            let mut parts = vec![json!({ "type": "text", "text": message.content })];
            for image in &message.images {
                parts.push(json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": image.mime,
                        "data": image.base64
                    }
                }));
            }
            json!({ "role": role, "content": parts })
        })
        .collect();
    body.insert("messages".to_owned(), Value::Array(messages));

    Value::Object(body)
}

/// Streams one answer.
pub async fn stream(
    config: &ProviderConfig,
    request: &ChatRequest,
) -> Result<EventStream, AiError> {
    let url = format!("{}/messages", config.base_url());

    let response = crate::client()?
        .post(url)
        .header("x-api-key", config.api_key().expose())
        .header("anthropic-version", API_VERSION)
        .json(&body(config, request))
        .send()
        .await?;

    let response = crate::ensure_success(response).await?;

    Ok(Box::pin(async_stream::try_stream! {
        let frames = sse::frames(response);
        futures::pin_mut!(frames);
        let mut totals = Totals::default();

        while let Some(frame) = frames.next().await {
            let frame = frame?;
            let data: Value = match serde_json::from_str(&frame.data) {
                Ok(data) => data,
                // Anthropic sends `data: {}` on some events; anything else is a real decode error.
                Err(error) if frame.data.trim().is_empty() || frame.data.trim() == "{}" => {
                    let _ = error;
                    continue;
                }
                Err(error) => Err(AiError::Decode(error))?,
            };

            for event in totals.apply(frame.event.as_deref(), &data) {
                yield event;
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_deltas() {
        let mut totals = Totals::default();
        let data = json!({ "delta": { "type": "text_delta", "text": "你好" } });
        assert_eq!(
            totals.apply(Some("content_block_delta"), &data),
            vec![StreamEvent::Text("你好".to_owned())]
        );
    }

    #[test]
    fn combines_usage_from_two_frames() {
        let mut totals = Totals::default();

        let start = json!({ "message": { "usage": { "input_tokens": 900, "cache_read_input_tokens": 512 } } });
        assert!(totals.apply(Some("message_start"), &start).is_empty());

        let delta = json!({ "usage": { "output_tokens": 64 } });
        assert!(totals.apply(Some("message_delta"), &delta).is_empty());

        let events = totals.apply(Some("message_stop"), &json!({}));
        assert_eq!(
            events,
            vec![
                StreamEvent::Usage(Usage {
                    prompt_tokens: 900,
                    completion_tokens: 64,
                    cached_tokens: 512,
                }),
                StreamEvent::Done,
            ]
        );
    }

    #[test]
    fn ignores_events_it_does_not_understand() {
        let mut totals = Totals::default();
        assert!(totals.apply(Some("ping"), &json!({})).is_empty());
        assert!(
            totals
                .apply(Some("content_block_start"), &json!({}))
                .is_empty()
        );
    }

    #[test]
    fn images_become_image_blocks() {
        let config = ProviderConfig::new(
            "Anthropic",
            crate::Protocol::Anthropic,
            "https://api.anthropic.com/v1",
            "claude-sonnet-4",
            crate::Secret::new("k"),
        )
        .unwrap();

        let request = ChatRequest {
            system: None,
            messages: vec![crate::Message::user_with_images(
                "看这个",
                vec![crate::Image {
                    mime: "image/png".to_owned(),
                    base64: "AAAA".to_owned(),
                }],
            )],
            max_tokens: 10,
            temperature: None,
        };

        let body = body(&config, &request);
        let content = &body["messages"][0]["content"];
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["media_type"], "image/png");
        assert_eq!(content[1]["source"]["data"], "AAAA");
    }

    #[test]
    fn body_has_no_system_role_in_messages() {
        let config = ProviderConfig::new(
            "Anthropic",
            crate::Protocol::Anthropic,
            "https://api.anthropic.com/v1",
            "claude-sonnet-4",
            crate::Secret::new("k"),
        )
        .unwrap();
        let request = ChatRequest::new("be brief", "hello");

        let body = body(&config, &request);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(body["system"], "be brief");
        assert_eq!(body["max_tokens"], 2048);
    }
}
