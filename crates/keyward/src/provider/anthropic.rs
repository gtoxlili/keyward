//! Anthropic Messages adapter — Keyward's second native dialect.
//!
//! The point of having two dialects is to prove the architecture: the Executor
//! relays the provider's native SSE events verbatim and only a small per-dialect
//! *usage extractor* differs. Anthropic's is the awkward one — usage is split
//! across events, and the cache fields appear in BOTH `message_start` and
//! `message_delta`, so naively summing the two usage objects double-counts cache
//! tokens (a real 2026 bug in several libraries). The rule encoded below: input
//! and cache come ONLY from `message_start`; output comes ONLY from
//! `message_delta`.

use keyward_proto::Usage;
use serde_json::Value;

/// Correctly accumulates Anthropic streaming usage across events.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UsageAcc {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
}

impl UsageAcc {
    /// Fold one parsed SSE event into the running usage.
    pub fn on_event(&mut self, ev: &Value) {
        match ev.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                if let Some(u) = ev.pointer("/message/usage") {
                    self.input_tokens = u
                        .get("input_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(self.input_tokens);
                    self.cache_creation_tokens = u
                        .get("cache_creation_input_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(self.cache_creation_tokens);
                    self.cache_read_tokens = u
                        .get("cache_read_input_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(self.cache_read_tokens);
                }
            }
            Some("message_delta") => {
                // output only — deliberately NOT re-reading cache here.
                if let Some(u) = ev.get("usage") {
                    self.output_tokens = u
                        .get("output_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(self.output_tokens);
                }
            }
            _ => {}
        }
    }

    pub fn to_usage(&self) -> Usage {
        Usage {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
        }
    }
}

#[cfg(feature = "anthropic")]
pub async fn call(
    _model: &str,
    request: &Value,
    key: &secrecy::SecretString,
) -> anyhow::Result<tokio::sync::mpsc::Receiver<super::Event>> {
    use super::Event;
    use futures_util::StreamExt;
    use secrecy::ExposeSecret;

    let base = std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".into());
    let endpoint = format!("{}/v1/messages", base.trim_end_matches('/'));

    let mut body = request.clone();
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".into(), Value::Bool(true));
    }

    let resp = reqwest::Client::new()
        .post(endpoint)
        .header("x-api-key", key.expose_secret()) // <-- the one and only place the key is used
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "provider_status {}: {}",
            status.as_u16(),
            text.chars().take(200).collect::<String>()
        );
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(16);
    tokio::spawn(async move {
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut acc = UsageAcc::default();
        while let Some(item) = stream.next().await {
            let Ok(bytes) = item else { break };
            buf.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(pos) = buf.find("\n\n") {
                let raw: String = buf.drain(..pos + 2).collect();
                for line in raw.lines() {
                    let Some(data) = line.trim_start().strip_prefix("data:") else {
                        continue;
                    };
                    let Ok(v) = serde_json::from_str::<Value>(data.trim()) else {
                        continue;
                    };
                    acc.on_event(&v);
                    let is_stop = v.get("type").and_then(Value::as_str) == Some("message_stop");
                    if tx.send(Event::Chunk(v)).await.is_err() {
                        return;
                    }
                    if is_stop {
                        let _ = tx
                            .send(Event::Done {
                                result: None,
                                usage: acc.to_usage(),
                            })
                            .await;
                        return;
                    }
                }
            }
        }
        let _ = tx
            .send(Event::Done {
                result: None,
                usage: acc.to_usage(),
            })
            .await;
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn does_not_double_count_cache() {
        let mut acc = UsageAcc::default();
        // cache appears in message_start...
        acc.on_event(&json!({
            "type": "message_start",
            "message": { "usage": {
                "input_tokens": 100, "cache_creation_input_tokens": 50,
                "cache_read_input_tokens": 200, "output_tokens": 1
            }}
        }));
        // ...and AGAIN in message_delta (where we must ignore it).
        acc.on_event(&json!({
            "type": "message_delta",
            "delta": { "stop_reason": "end_turn" },
            "usage": { "output_tokens": 80, "cache_read_input_tokens": 200 }
        }));

        assert_eq!(acc.input_tokens, 100);
        assert_eq!(acc.output_tokens, 80);
        assert_eq!(
            acc.cache_read_tokens, 200,
            "cache read must not be doubled to 400"
        );
        assert_eq!(acc.cache_creation_tokens, 50);
    }
}
