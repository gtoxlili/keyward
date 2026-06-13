//! Transport plumbing for the WebSocket reference adapter: Frame ↔ tungstenite
//! Message, plus small hex / fingerprint / digest helpers.

use anyhow::{Result, anyhow};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use keyward_proto::Frame;
use sha2::{Digest, Sha256};
use tokio_tungstenite::tungstenite::Message;

/// Serialize and send one Keyward frame as a text message.
pub async fn send<S>(sink: &mut S, frame: &Frame) -> Result<()>
where
    S: Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    let txt = serde_json::to_string(frame)?;
    sink.send(Message::Text(txt.into())).await?;
    Ok(())
}

/// Receive the next Keyward frame, skipping control frames. Returns `None` on
/// channel close — which the protocol treats as session suspension (§7).
pub async fn recv<S>(stream: &mut S) -> Result<Option<Frame>>
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(msg) = stream.next().await {
        match msg? {
            Message::Text(t) => {
                let frame: Frame =
                    serde_json::from_str(t.as_str()).map_err(|e| anyhow!("malformed frame: {e}"))?;
                return Ok(Some(frame));
            }
            Message::Close(_) => return Ok(None),
            _ => continue, // ping / pong / binary
        }
    }
    Ok(None)
}

// --- small helpers, kept dependency-free ----------------------------------

/// Lowercase hex encoding.
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    s
}

/// Decode lowercase/uppercase hex. Returns `None` on malformed input.
pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    let mut i = 0;
    while i < b.len() {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    Some(out)
}

/// A short, human-comparable fingerprint of a public key (for the out-of-band
/// confirmation that closes the TOFU first-contact gap, §9).
pub fn fingerprint(pubkey: &[u8]) -> String {
    let h = hex(pubkey);
    [&h[0..4], &h[4..8], &h[8..12], &h[12..16]].join("-")
}

/// `sha256:…` over the canonical policy bytes (§3). Lets the Orchestrator notice
/// the limits changed without the Owner having to reveal them.
pub fn policy_digest(canonical: &str) -> String {
    let mut h = Sha256::new();
    h.update(canonical.as_bytes());
    format!("sha256:{}", hex(&h.finalize()))
}
