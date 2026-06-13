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
    let _ = &key; // used by the `openai` adapter; the mock holds but doesn't send it
    match provider {
        // The demo provider: accepts an OpenAI-shaped body, makes NO network
        // call, and emits OpenAI-shaped chunks. Proves native passthrough +
        // metering without a key.
        "mock" => Ok(mock_openai(model, request)),
        #[cfg(feature = "openai")]
        "openai" => openai::call(model, request, key).await,
        #[cfg(not(feature = "openai"))]
        "openai" => anyhow::bail!("provider 'openai' needs a build with --features openai"),
        other => anyhow::bail!("unsupported_provider: {other}"),
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
    let last_user = request
        .get("messages")
        .and_then(|m| m.as_array())
        .and_then(|a| a.iter().rev().find(|m| m.get("role").and_then(Value::as_str) == Some("user")))
        .and_then(|m| m.get("content").and_then(Value::as_str))
        .unwrap_or("(no user message)")
        .to_string();
    let input_tokens = rough_tokens(&last_user) + rough_tokens(model);

    let reply = format!("Mock reply from {model}: I received {} chars of prompt.", last_user.len());
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
                usage: Usage { input_tokens, output_tokens },
            })
            .await;
    });

    rx
}
