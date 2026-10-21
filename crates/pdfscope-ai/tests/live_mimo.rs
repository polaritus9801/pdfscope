#![allow(clippy::print_stdout, clippy::unwrap_used, clippy::expect_used)]

//! A live call, ignored by default because it needs a key and spends money.
//!
//! Run with:
//!
//! ```sh
//! MIMO_API_KEY=… cargo test -p pdfscope-ai --test live_mimo -- --ignored --nocapture
//! ```

use futures::StreamExt;
use pdfscope_ai::{ChatRequest, Message, Protocol, ProviderConfig, Secret, StreamEvent, stream};

#[tokio::test]
#[ignore = "needs MIMO_API_KEY and spends tokens"]
async fn mimo_streams_reasoning_and_text() {
    let key = std::env::var("MIMO_API_KEY").expect("MIMO_API_KEY must be set");
    let config = ProviderConfig::new(
        "MiMo",
        Protocol::OpenAi,
        "https://api.xiaomimimo.com/v1",
        "mimo-v2.6-flash",
        Secret::new(key),
    )
    .expect("valid config");

    let request = ChatRequest {
        system: Some("你是一个简洁的助手。".to_owned()),
        messages: vec![Message::user("只回复两个字：收到")],
        max_tokens: 256,
        temperature: None,
    };

    let mut events = stream(&config, &request).await.expect("request starts");

    let mut text = String::new();
    let mut reasoning = String::new();
    let mut usage = None;
    let mut done = false;

    while let Some(event) = events.next().await {
        match event.expect("no transport error") {
            StreamEvent::Reasoning(delta) => reasoning.push_str(&delta),
            StreamEvent::Text(delta) => text.push_str(&delta),
            StreamEvent::Usage(totals) => usage = Some(totals),
            StreamEvent::Done => {
                done = true;
                break;
            }
        }
    }

    println!("reasoning: {} chars", reasoning.chars().count());
    println!("text: {text:?}");
    println!("usage: {usage:?}");

    assert!(done, "the stream must end with Done");
    assert!(
        text.contains("收到"),
        "expected the answer in Text, got {text:?}"
    );
    assert!(
        !text.contains("The user is asking"),
        "reasoning must not leak into the answer"
    );
    assert!(usage.is_some(), "MiMo reports usage");
}
