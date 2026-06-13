//! WebSocket framing + small hex/fingerprint helpers (self-contained).

use anyhow::{anyhow, Result};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use keyward_proto::Frame;
use tokio_tungstenite::tungstenite::Message;

pub async fn send<S>(sink: &mut S, frame: &Frame) -> Result<()>
where
    S: Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    sink.send(Message::Text(serde_json::to_string(frame)?)).await?;
    Ok(())
}

/// Next Keyward frame, skipping control frames. `None` on channel close.
pub async fn recv<S>(stream: &mut S) -> Result<Option<Frame>>
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(msg) = stream.next().await {
        match msg? {
            Message::Text(t) => {
                return Ok(Some(
                    serde_json::from_str(t.as_str()).map_err(|e| anyhow!("malformed frame: {e}"))?,
                ));
            }
            Message::Close(_) => return Ok(None),
            _ => continue,
        }
    }
    Ok(None)
}

pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    s
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    let mut i = 0;
    while i < b.len() {
        out.push(((b[i] as char).to_digit(16)? * 16 + (b[i + 1] as char).to_digit(16)?) as u8);
        i += 2;
    }
    Some(out)
}

/// Short human-comparable fingerprint of a public key (for OOB confirmation).
pub fn fingerprint(pubkey: &[u8]) -> String {
    let h = hex(pubkey);
    [&h[0..4], &h[4..8], &h[8..12], &h[12..16]].join("-")
}
