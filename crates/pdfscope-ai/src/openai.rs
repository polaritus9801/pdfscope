//! OpenAI Chat Completions, and every server that copies it.

use futures::StreamExt;
use serde_json::{Value, json};

use crate::sse;
use crate::{AiError, ChatRequest, EventStream, ProviderConfig, Role, StreamEvent, Usage};

/// Maps one streamed chunk to events.
///
/// Pure on purpose: the interesting bugs are in the mapping, and they are cheaper to test against a
/// literal chunk than against a network.
pub(crate) fn map_chunk(value: &Value) -> Vec<StreamEvent> {
    let mut events = Vec::new();

    if let Some(reasoning) = value
        .pointer("/choices/0/delta/reasoning_content")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        events.push(StreamEvent::Reasoning(reasoning.to_owned()));
    }

    if let Some(text) = value
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        events.push(StreamEvent::Text(text.to_owned()));
    }

    if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
        events.push(StreamEvent::Usage(Usage {
            prompt_tokens: token_count(usage, "/prompt_tokens"),
            completion_tokens: token_count(usage, "/completion_tokens"),
            cached_tokens: token_count(usage, "/prompt_tokens_details/cached_tokens"),
        }));
    }

    events
}

fn token_count(usage: &Value, pointer: &str) -> u32 {
    usage
        .pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|count| u32::try_from(count).ok())
        .unwrap_or(0)
}

fn messages(request: &ChatRequest) -> Value {
    let mut items = Vec::new();

    if let Some(system) = &request.system {
        items.push(json!({ "role": "system", "content": system }));
    }

    for message in &request.messages {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        if message.images.is_empty() {
            items.push(json!({ "role": role, "content": message.content }));
            continue;
        }

        let mut parts = vec![json!({ "type": "text", "text": message.content })];
        for image in &message.images {
            parts.push(json!({
                "type": "image_url",
                "image_url": { "url": format!("data:{};base64,{}", image.mime, image.base64) }
            }));
        }
        items.push(json!({ "role": role, "content": parts }));
    }

    Value::Array(items)
}

fn body(config: &ProviderConfig, request: &ChatRequest) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("model".to_owned(), json!(config.model()));
    body.insert("messages".to_owned(), messages(request));
    if let Some(limit) = request.max_tokens {
        body.insert("max_tokens".to_owned(), json!(limit));
    }
    body.insert("stream".to_owned(), json!(true));
    // Ask for the usage summary in the final frame; providers that do not know the field ignore it.
    body.insert(
        "stream_options".to_owned(),
        json!({ "include_usage": true }),
    );

    if let Some(temperature) = request.temperature {
        body.insert("temperature".to_owned(), json!(temperature));
    }

    Value::Object(body)
}

/// Streams one answer.
pub async fn stream(
    config: &ProviderConfig,
    request: &ChatRequest,
) -> Result<EventStream, AiError> {
    let url = format!("{}/chat/completions", config.base_url());

    let response = crate::client()?
        .post(url)
        .bearer_auth(config.api_key().expose())
        .json(&body(config, request))
        .send()
        .await?;

    let response = crate::ensure_success(response).await?;

    Ok(Box::pin(async_stream::try_stream! {
        let frames = sse::frames(response);
        futures::pin_mut!(frames);
        let mut finished = false;

        while let Some(frame) = frames.next().await {
            let frame = frame?;

            if frame.data.trim() == "[DONE]" {
                finished = true;
                yield StreamEvent::Done;
                break;
            }

            let value: Value = serde_json::from_str(&frame.data)?;
            for event in map_chunk(&value) {
                yield event;
            }
        }

        if !finished {
            Err(AiError::Incomplete)?;
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_answer_tokens() {
        let chunk = json!({
            "choices": [{ "delta": { "content": "收到" }, "index": 0 }]
        });
        assert_eq!(
            map_chunk(&chunk),
            vec![StreamEvent::Text("收到".to_owned())]
        );
    }

    #[test]
    fn keeps_reasoning_out_of_the_answer() {
        let chunk = json!({
            "choices": [{ "delta": { "content": null, "reasoning_content": "thinking" }, "index": 0 }]
        });
        assert_eq!(
            map_chunk(&chunk),
            vec![StreamEvent::Reasoning("thinking".to_owned())]
        );
    }

    #[test]
    fn ignores_empty_deltas() {
        let chunk = json!({
            "choices": [{ "delta": { "content": "", "reasoning_content": null }, "index": 0 }]
        });
        assert!(map_chunk(&chunk).is_empty());
    }

    #[test]
    fn reports_usage_including_cache_hits() {
        let chunk = json!({
            "choices": [],
            "usage": {
                "prompt_tokens": 1200,
                "completion_tokens": 40,
                "prompt_tokens_details": { "cached_tokens": 1024 }
            }
        });
        assert_eq!(
            map_chunk(&chunk),
            vec![StreamEvent::Usage(Usage {
                prompt_tokens: 1200,
                completion_tokens: 40,
                cached_tokens: 1024,
            })]
        );
    }

    #[test]
    fn missing_usage_details_are_zero_rather_than_a_guess() {
        let chunk = json!({
            "choices": [],
            "usage": { "prompt_tokens": 10, "completion_tokens": 2 }
        });
        assert_eq!(
            map_chunk(&chunk),
            vec![StreamEvent::Usage(Usage {
                prompt_tokens: 10,
                completion_tokens: 2,
                cached_tokens: 0,
            })]
        );
    }

    #[test]
    fn images_become_image_url_parts() {
        let mut request = ChatRequest::new("be brief", "看这个");
        request.messages = vec![crate::Message::user_with_images(
            "看这个",
            vec![crate::Image {
                mime: "image/png".to_owned(),
                base64: "AAAA".to_owned(),
            }],
        )];

        let messages = messages(&request);
        let content = &messages[1]["content"];
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image_url");
        assert_eq!(content[1]["image_url"]["url"], "data:image/png;base64,AAAA");
    }

    #[test]
    fn a_system_prompt_comes_first() {
        let mut request = ChatRequest::new("be brief", "hello");
        request.messages.push(crate::Message::assistant("hi"));

        let messages = messages(&request);
        let array = messages.as_array().unwrap();
        assert_eq!(array[0]["role"], "system");
        assert_eq!(array[1]["role"], "user");
        assert_eq!(array[2]["role"], "assistant");
    }
}
