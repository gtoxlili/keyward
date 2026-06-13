//! Provider adapters. The work intent carries the provider-NATIVE request body
//! minus the credential; an adapter selects the endpoint, injects the key, and
//! emits native chunks back (§4). Keyward relays those bytes verbatim and only
//! parses a copy to meter usage — so new provider event shapes flow through
//! untouched and only the usage extractor ever needs updating.

use anyhow::Result;
use keyward_proto::Usage;
use secrecy::SecretString;
use serde_json::{json, Value};
use tokio::sync::mpsc;

pub mod anthropic;
#[cfg(feature = "openai")]
pub mod openai;

/// One streamed step from a provider, in the provider's native shape.
#[derive(Debug)]
pub enum Event {
    /// A native streaming chunk, relayed verbatim as `work_chunk.delta`.
    Chunk(Value),
    /// Terminal success: optional full result + metered usage.
    Done { result: Option<Value>, usage: Usage },
}

/// Begin a provider call. Returns a bounded receiver of native events; the
/// bound is the backpressure point — if the downstream channel stalls, the
/// adapter stops reading the upstream body and TCP throttles the provider.
pub async fn call(
    provider: &str,
    model: &str,
    request: &Value,
    key: &SecretString,
) -> Result<mpsc::Receiver<Event>> {
    let _ = &key; // used by the real adapters; the mock holds but doesn't send it
    match provider {
        // The demo provider: makes NO network call and emits native chunks in
        // whichever dialect the model implies. Proves native passthrough +
        // per-dialect usage metering without a key.
        "mock" => Ok(mock_call(model, request)),
        #[cfg(feature = "openai")]
        "openai" => openai::call(model, request, key).await,
        #[cfg(not(feature = "openai"))]
        "openai" => anyhow::bail!("provider 'openai' needs a build with --features openai"),
        #[cfg(feature = "anthropic")]
        "anthropic" => anthropic::call(model, request, key).await,
        #[cfg(not(feature = "anthropic"))]
        "anthropic" => anyhow::bail!("provider 'anthropic' needs a build with --features anthropic"),
        other => anyhow::bail!("unsupported_provider: {other}"),
    }
}

/// Pick the mock dialect by model family — Claude models get the Anthropic event
/// shape, everything else the OpenAI shape.
fn mock_call(model: &str, request: &Value) -> mpsc::Receiver<Event> {
    if model.starts_with("claude") {
        mock_anthropic(model, request)
    } else {
        mock_openai(model, request)
    }
}

/// Crude whitespace token estimate — stand-in for tiktoken / count_tokens, used
/// only by the mock. Real adapters meter from provider-reported `usage`.
fn rough_tokens(s: &str) -> u64 {
    s.split_whitespace().count().max(1) as u64
}

fn mock_openai(model: &str, request: &Value) -> mpsc::Receiver<Event> {
    let (tx, rx) = mpsc::channel::<Event>(16);

    // Echo back something derived from the last user message, in a few chunks.
    let last_user = last_user_text(request);
    let input_tokens = rough_tokens(&last_user) + rough_tokens(model);

    let reply = format!(
        "Mock reply from {model}: I received {} chars of prompt.",
        last_user.len()
    );
    let pieces: Vec<String> = reply.split_inclusive(' ').map(str::to_string).collect();
    let output_tokens = pieces.len() as u64;

    tokio::spawn(async move {
        for piece in pieces {
            let delta = json!({ "choices": [{ "delta": { "content": piece } }] });
            if tx.send(Event::Chunk(delta)).await.is_err() {
                return; // downstream gone; stop "reading the provider".
            }
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        }
        let _ = tx
            .send(Event::Done {
                result: None,
                usage: Usage {
                    input_tokens,
                    output_tokens,
                },
            })
            .await;
    });

    rx
}

/// The most recent user message's text, for the mocks to echo.
fn last_user_text(request: &Value) -> String {
    request
        .get("messages")
        .and_then(|m| m.as_array())
        .and_then(|a| {
            a.iter()
                .rev()
                .find(|m| m.get("role").and_then(Value::as_str) == Some("user"))
        })
        .and_then(|m| m.get("content").and_then(Value::as_str))
        .unwrap_or("(no user message)")
        .to_string()
}

/// Mock provider in the Anthropic Messages dialect: emits the native event
/// sequence (`message_start` → `content_block_delta`* → `message_delta` →
/// `message_stop`) and meters usage through the same `UsageAcc` the real adapter
/// uses — so the split/cumulative usage handling is exercised without a key.
fn mock_anthropic(model: &str, request: &Value) -> mpsc::Receiver<Event> {
    let (tx, rx) = mpsc::channel::<Event>(16);

    let last_user = last_user_text(request);
    let input_tokens = rough_tokens(&last_user) + rough_tokens(model);
    let reply = format!("Mock Claude reply from {model}: {} chars in.", last_user.len());
    let pieces: Vec<String> = reply.split_inclusive(' ').map(str::to_string).collect();
    let output_tokens = pieces.len() as u64;

    let mut events: Vec<Value> = Vec::new();
    events.push(json!({
        "type": "message_start",
        "message": { "usage": {
            "input_tokens": input_tokens, "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0, "output_tokens": 1
        }}
    }));
    events.push(
        json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" }}),
    );
    for piece in pieces {
        events.push(json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": piece }}));
    }
    events.push(json!({ "type": "content_block_stop", "index": 0 }));
    events.push(json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": output_tokens }}));
    events.push(json!({ "type": "message_stop" }));

    tokio::spawn(async move {
        let mut acc = anthropic::UsageAcc::default();
        for ev in events {
            acc.on_event(&ev);
            if tx.send(Event::Chunk(ev)).await.is_err() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }
        let _ = tx
            .send(Event::Done {
                result: None,
                usage: acc.to_usage(),
            })
            .await;
    });

    rx
}
