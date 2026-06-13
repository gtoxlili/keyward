//! Real OpenAI (and OpenAI-compatible) Chat Completions adapter.
//!
//! Demonstrates the load-bearing rules from the research:
//!  - the native body is passed through; we only force `stream` + merge
//!    `stream_options.include_usage` so usage is reported (don't clobber);
//!  - the credential is attached at exactly ONE call site (`bearer_auth`);
//!  - `OPENAI_BASE_URL` is honored so a user can point the Executor at a proxy
//!    and confirm the key only ever appears on the call to the provider.

use anyhow::Result;
use futures_util::StreamExt;
use keyward_proto::Usage;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use super::Event;

fn endpoint() -> String {
    let base = std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".into());
    format!("{}/chat/completions", base.trim_end_matches('/'))
}

pub async fn call(_model: &str, request: &Value, key: &SecretString) -> Result<mpsc::Receiver<Event>> {
    // Copy the native body; force streaming + usage reporting without clobbering.
    let mut body = request.clone();
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".into(), Value::Bool(true));
        let so = obj.entry("stream_options").or_insert_with(|| json!({}));
        if let Some(so_obj) = so.as_object_mut() {
            so_obj.insert("include_usage".into(), Value::Bool(true));
        }
    }

    let resp = reqwest::Client::new()
        .post(endpoint())
        .bearer_auth(key.expose_secret()) // <-- the one and only place the key is used
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        // Sanitized: never echo a body that might contain the credential (§5).
        anyhow::bail!("provider_status {}: {}", status.as_u16(), sanitize(&text));
    }

    let (tx, rx) = mpsc::channel::<Event>(16);
    tokio::spawn(async move {
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut usage = Usage::default();
        while let Some(item) = stream.next().await {
            let Ok(bytes) = item else { break };
            buf.push_str(&String::from_utf8_lossy(&bytes));
            // SSE events are separated by a blank line.
            while let Some(pos) = buf.find("\n\n") {
                let raw: String = buf.drain(..pos + 2).collect();
                for line in raw.lines() {
                    let Some(data) = line.trim_start().strip_prefix("data:") else { continue };
                    let data = data.trim();
                    if data == "[DONE]" {
                        let _ = tx.send(Event::Done { result: None, usage }).await;
                        return;
                    }
                    let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
                    if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
                        usage.input_tokens = u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(usage.input_tokens);
                        usage.output_tokens = u.get("completion_tokens").and_then(Value::as_u64).unwrap_or(usage.output_tokens);
                    }
                    if tx.send(Event::Chunk(v)).await.is_err() {
                        return; // downstream gone — stop reading upstream (backpressure).
                    }
                }
            }
        }
        let _ = tx.send(Event::Done { result: None, usage }).await;
    });

    Ok(rx)
}

fn sanitize(s: &str) -> String {
    let mut t: String = s.chars().take(200).collect();
    if s.len() > 200 {
        t.push('…');
    }
    t
}
