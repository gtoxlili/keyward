//! Real OpenAI adapters (gated behind `--features openai`): Chat Completions
//! (`/v1/chat/completions`) and the Responses API (`/v1/responses`).
//!
//! Two distinct endpoints — different request bodies, different streaming events —
//! kept as separate parsers per the research (don't try to unify them); each has
//! its own usage extractor (unit-tested below). Shared rules: the native body is
//! passed through, we only set the streaming flag(s); the credential is attached
//! at exactly ONE call site; `OPENAI_BASE_URL` is honored for the proxy recipe.

use anyhow::Result;
use futures_util::StreamExt;
use keyward_proto::Usage;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::Event;

fn base_url() -> String {
    std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".into())
}

/// Chat Completions usage — present only on the forced final `usage` chunk.
fn chat_usage(chunk: &Value) -> Option<Usage> {
    let u = chunk.get("usage").filter(|u| !u.is_null())?;
    Some(Usage {
        input_tokens: u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
        output_tokens: u.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// Responses API usage — carried natively on the terminal `response.completed`
/// event (no opt-in flag needed), under `response.usage`.
fn responses_usage(event: &Value) -> Option<Usage> {
    if event.get("type").and_then(Value::as_str)? != "response.completed" {
        return None;
    }
    let u = event.pointer("/response/usage")?;
    Some(Usage {
        input_tokens: u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0),
        output_tokens: u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// Chat Completions: force streaming + usage reporting, then relay native chunks.
pub async fn call(_model: &str, request: &Value, key: &SecretString) -> Result<mpsc::Receiver<Event>> {
    let mut body = request.clone();
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".into(), Value::Bool(true));
        let so = obj.entry("stream_options").or_insert_with(|| json!({}));
        if let Some(so_obj) = so.as_object_mut() {
            so_obj.insert("include_usage".into(), Value::Bool(true));
        }
    }
    let endpoint = format!("{}/chat/completions", base_url().trim_end_matches('/'));
    stream_sse(endpoint, body, key, Dialect::Chat).await
}

/// Responses API: usage streams natively on `response.completed`; just force stream.
pub async fn call_responses(
    _model: &str,
    request: &Value,
    key: &SecretString,
) -> Result<mpsc::Receiver<Event>> {
    let mut body = request.clone();
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".into(), Value::Bool(true));
    }
    let endpoint = format!("{}/responses", base_url().trim_end_matches('/'));
    stream_sse(endpoint, body, key, Dialect::Responses).await
}

enum Dialect {
    Chat,
    Responses,
}

async fn stream_sse(
    endpoint: String,
    body: Value,
    key: &SecretString,
    dialect: Dialect,
) -> Result<mpsc::Receiver<Event>> {
    let resp = reqwest::Client::new()
        .post(endpoint)
        .bearer_auth(key.expose_secret()) // <-- the one and only place the key is used
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
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
            while let Some(pos) = buf.find("\n\n") {
                let raw: String = buf.drain(..pos + 2).collect();
                for line in raw.lines() {
                    let Some(data) = line.trim_start().strip_prefix("data:") else {
                        continue;
                    };
                    let data = data.trim();
                    if data == "[DONE]" {
                        let _ = tx.send(Event::Done { result: None, usage }).await;
                        return;
                    }
                    let Ok(v) = serde_json::from_str::<Value>(data) else {
                        continue;
                    };
                    let terminal = match dialect {
                        Dialect::Chat => {
                            if let Some(u) = chat_usage(&v) {
                                usage = u;
                            }
                            false
                        }
                        Dialect::Responses => {
                            if let Some(u) = responses_usage(&v) {
                                usage = u;
                            }
                            v.get("type").and_then(Value::as_str) == Some("response.completed")
                        }
                    };
                    if tx.send(Event::Chunk(v)).await.is_err() {
                        return; // downstream gone — stop reading upstream (backpressure).
                    }
                    if terminal {
                        let _ = tx.send(Event::Done { result: None, usage }).await;
                        return;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_usage_only_on_final_chunk() {
        assert!(chat_usage(&json!({"choices":[{"delta":{"content":"hi"}}]})).is_none());
        let u =
            chat_usage(&json!({"choices":[],"usage":{"prompt_tokens":11,"completion_tokens":22}})).unwrap();
        assert_eq!((u.input_tokens, u.output_tokens), (11, 22));
    }

    #[test]
    fn responses_usage_only_on_completed() {
        assert!(responses_usage(&json!({"type":"response.output_text.delta","delta":"hi"})).is_none());
        let ev = json!({"type":"response.completed","response":{"usage":{"input_tokens":30,"output_tokens":40,"total_tokens":70}}});
        let u = responses_usage(&ev).unwrap();
        assert_eq!((u.input_tokens, u.output_tokens), (30, 40));
    }
}
