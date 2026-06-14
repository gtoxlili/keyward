//! Node-side session primitives — pairing, Client authentication (§3/§9), and a
//! single-pair `serve` loop the self-contained demo drives in-process. Holds NO
//! provider key: it issues a one-time pairing token, presents a root-delegated
//! operational key, signs the assigned `sid`, then sends work intents and reads the
//! relayed stream.
//!
//! This is the demo/test-harness side. The deployable Node is `keyward node`
//! ([`crate::node`]); the real "app" is an unaware OpenAI client that just points at
//! a Node — it integrates nothing.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow, bail};
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{Sink, Stream, StreamExt};
use keyward_proto::{Body, Frame, Peer};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use crate::client::new_mid;
use crate::wire;

pub struct NodeConfig {
    pub name: String,
    pub id: String,
    pub pairing_token: String,
    /// Long-term root identity. Each connection mints a fresh operational key
    /// delegated by this root (§3), so the Client pins the root once.
    pub root: SigningKey,
    /// Optional allow-list of authorized Client identity pubkeys (hex). `None`
    /// accepts any Client that proves possession of its key; `Some` lets a SaaS
    /// admit only registered users (§9).
    pub authorized_clients: Option<Vec<String>>,
    /// Single-use bookkeeping: a pairing token binds to one Client identity
    /// (`token → pubkey`). The same identity may re-present it (reconnect/resume);
    /// a different identity is refused.
    pub claimed_tokens: Arc<Mutex<HashMap<String, String>>>,
    /// Enforce one-identity-per-token (the SaaS default). A multi-tenant **node**
    /// (§10) sets this `false`: one shared join-token admits many Clients, and
    /// per-user isolation comes from the routing token + the Client's own policy.
    pub single_use_token: bool,
    /// Scripted intents: (provider, native request body without credential).
    pub intents: Vec<(String, Value)>,
}

/// Serve a single dialed-in Client through pairing + the scripted intents.
pub async fn serve(stream: TcpStream, cfg: NodeConfig) -> Result<()> {
    let ws = accept_async(stream).await?;
    let (mut write, mut read) = ws.split();

    // 1. expect hello, authenticate the Client (token + identity + allow-list).
    let hello = wire::recv(&mut read)
        .await?
        .ok_or_else(|| anyhow!("closed before hello"))?;
    if let Err(e) = authenticate_client(&hello.body, &cfg) {
        let _ = wire::send(
            &mut write,
            &Frame::new(
                None,
                new_mid(),
                Body::Error {
                    code: "bad_request".into(),
                    message: e.to_string(),
                },
            ),
        )
        .await;
        return Err(e);
    }

    // 2. mint an operational key delegated by the root, sign the sid, send paired.
    let (sid, paired) = build_paired(&cfg);
    wire::send(&mut write, &paired).await?;

    // 3. run scripted intents, sequentially for clear output.
    for (i, (provider, request)) in cfg.intents.into_iter().enumerate() {
        let mid = new_mid();
        let model = request
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        println!("\n[node] --> work #{i}  provider={provider}  model={model}");
        wire::send(
            &mut write,
            &Frame::new(Some(sid.clone()), mid.clone(), Body::Work { provider, request }),
        )
        .await?;

        let mut assembled = String::new();
        loop {
            let Some(frame) = wire::recv(&mut read).await? else {
                return Err(anyhow!("channel dropped mid-intent"));
            };
            if frame.mid != mid {
                continue; // demux by echoed mid
            }
            match frame.body {
                Body::WorkAccepted {} => println!("[node]     accepted"),
                Body::WorkChunk { seq, delta } => {
                    let piece = chunk_text(&delta);
                    assembled.push_str(piece);
                    println!("[node]     chunk seq={seq}  {piece:?}");
                }
                Body::WorkDone { usage, .. } => {
                    println!("[node]     done  assembled={assembled:?}");
                    println!(
                        "[node]     usage in={} out={}",
                        usage.input_tokens, usage.output_tokens
                    );
                    break;
                }
                Body::WorkError { code, message, .. } => {
                    println!("[node]     ERROR  {code}: {message}");
                    break;
                }
                _ => {}
            }
        }
    }

    // 4. orderly close.
    wire::send(
        &mut write,
        &Frame::new(
            Some(sid),
            new_mid(),
            Body::Close {
                reason: "done".into(),
            },
        ),
    )
    .await?;
    Ok(())
}

/// Extract the text delta from a native chunk in either dialect — the demo's
/// stand-in for what a real provider SDK does on the Node side. Keyward
/// itself never looks inside the chunk; it just relays the bytes.
pub(crate) fn chunk_text(delta: &Value) -> &str {
    if let Some(t) = delta.pointer("/choices/0/delta/content").and_then(Value::as_str) {
        return t; // OpenAI Chat Completions
    }
    match delta.get("type").and_then(Value::as_str) {
        Some("content_block_delta") => {
            if let Some(t) = delta.pointer("/delta/text").and_then(Value::as_str) {
                return t; // Anthropic Messages
            }
        }
        Some("response.output_text.delta") => {
            if let Some(t) = delta.get("delta").and_then(Value::as_str) {
                return t; // OpenAI Responses
            }
        }
        _ => {}
    }
    ""
}

/// Authenticate the Client from its `hello` (§9): the pairing token, a possession
/// proof (the Client's signature over the token with its identity key), and — when
/// the Node runs an allow-list — that the identity is authorized. This lets a
/// SaaS admit only registered users, without ever weakening the Owner's ability to
/// inspect their own key. A pairing token is single-use *per identity*: it binds to
/// the first Client that claims it, and only that identity may re-present it — so
/// the resume demo re-pairs, but a different party can't reuse a leaked token.
pub(crate) fn authenticate_client(hello: &Body, cfg: &NodeConfig) -> Result<()> {
    let Body::Hello {
        pairing_token,
        client,
        pubkey,
        sig,
        ..
    } = hello
    else {
        bail!("expected hello");
    };
    if pairing_token != &cfg.pairing_token {
        bail!("pairing token rejected");
    }
    match (pubkey, sig) {
        (Some(pk), Some(sig)) => {
            let vk = crate::identity::parse_pubkey(pk)?;
            crate::identity::verify_detached(&vk, cfg.pairing_token.as_bytes(), sig)
                .map_err(|_| anyhow!("client identity signature invalid"))?;
        }
        _ => {
            if cfg.authorized_clients.is_some() {
                bail!("client identity required but not provided");
            }
        }
    }
    if let Some(allow) = &cfg.authorized_clients {
        let pk = pubkey.as_deref().unwrap_or("");
        if !allow.iter().any(|a| a == pk) {
            bail!("client not authorized");
        }
    }
    // Single-use: a token binds to one identity. The same identity may re-present
    // it (reconnect/resume); a different one is refused. A node disables this so one
    // join-token can admit many Clients (§10).
    if cfg.single_use_token
        && let Some(pk) = pubkey
    {
        let mut claimed = cfg.claimed_tokens.lock().unwrap();
        match claimed.get(pairing_token.as_str()) {
            Some(owner) if owner != pk => bail!("pairing token already bound to another client"),
            _ => {
                claimed.insert(pairing_token.clone(), pk.clone());
            }
        }
    }
    let fp = pubkey
        .as_deref()
        .and_then(wire::unhex)
        .filter(|b| b.len() >= 8)
        .map(|b| wire::fingerprint(&b))
        .unwrap_or_else(|| "none".into());
    println!("[node] hello from client '{}'  identity fp={fp}", client.name);
    Ok(())
}

// --- resume / cancel demo (two connections) -------------------------------

async fn authenticate_hello<S>(read: &mut S, cfg: &NodeConfig) -> Result<()>
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let hello = wire::recv(read)
        .await?
        .ok_or_else(|| anyhow!("closed before hello"))?;
    authenticate_client(&hello.body, cfg)
}

async fn send_paired<S>(write: &mut S, cfg: &NodeConfig) -> Result<String>
where
    S: Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    let (sid, paired) = build_paired(cfg);
    wire::send(write, &paired).await?;
    Ok(sid)
}

/// Build a `paired` frame: assign a sid, mint a fresh operational key, have the
/// root delegate it (1h), and sign the sid with it (the SSH-CA chain, §3).
/// A fresh op key per call is what lets reconnects rotate keys without re-pairing.
pub(crate) fn build_paired(cfg: &NodeConfig) -> (String, Frame) {
    let sid = format!("kw_sess_{}", &new_mid()[..8]);
    let op = SigningKey::generate(&mut rand_core::OsRng);
    let cert =
        crate::identity::issue_op_cert(&cfg.root, &op.verifying_key(), crate::identity::now_unix() + 3600);
    let sig = op.sign(sid.as_bytes());
    let frame = Frame::new(
        Some(sid.clone()),
        new_mid(),
        Body::Paired {
            node: Peer {
                name: cfg.name.clone(),
                version: None,
                id: Some(cfg.id.clone()),
            },
            root_pubkey: wire::hex(&cfg.root.verifying_key().to_bytes()),
            op: cert,
            sig: wire::hex(&sig.to_bytes()),
        },
    );
    (sid, frame)
}

/// Demonstrate §7: stream an intent, drop the socket mid-stream, let the
/// Client re-dial, resume from `last_seq`, then deliberately cancel a second
/// intent. The Client's producer keeps running across the drop, so resume
/// replays the chunks the Node missed.
pub async fn serve_resume_demo(listener: TcpListener, cfg: NodeConfig) -> Result<()> {
    // ---- connection 1: stream, then drop mid-way ----
    let (s1, _) = listener.accept().await?;
    let (mut w1, mut r1) = accept_async(s1).await?.split();
    authenticate_hello(&mut r1, &cfg).await?;
    let sid1 = send_paired(&mut w1, &cfg).await?;
    println!("[node] paired (conn 1)  sid={sid1}");

    let mid = new_mid();
    let req = json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "Stream a sentence long enough to span many chunks so we can interrupt it midway and resume."}], "stream": true});
    println!("\n[node] --> work mid={}…  model=gpt-4o", &mid[..8]);
    wire::send(
        &mut w1,
        &Frame::new(
            Some(sid1.clone()),
            mid.clone(),
            Body::Work {
                provider: "mock".into(),
                request: req,
            },
        ),
    )
    .await?;

    let mut last_seq: i64 = -1;
    let mut got = 0;
    loop {
        let Some(frame) = wire::recv(&mut r1).await? else {
            break;
        };
        if frame.mid != mid {
            continue;
        }
        match frame.body {
            Body::WorkAccepted {} => println!("[node]     accepted"),
            Body::WorkChunk { seq, delta } => {
                println!("[node]     chunk seq={seq}  {:?}", chunk_text(&delta));
                last_seq = seq as i64;
                got += 1;
                if got >= 3 {
                    break;
                }
            }
            Body::WorkDone { .. } => {
                println!("[node]     (completed before we could interrupt)");
                break;
            }
            Body::WorkError { code, message, .. } => {
                println!("[node]     error {code}: {message}");
                return Ok(());
            }
            _ => {}
        }
    }
    println!("[node] !!! dropping the socket after seq={last_seq} — simulated channel loss\n");
    drop(w1);
    drop(r1);

    // ---- connection 2: the Client re-dials; resume ----
    let (s2, _) = listener.accept().await?;
    let (mut w2, mut r2) = accept_async(s2).await?.split();
    authenticate_hello(&mut r2, &cfg).await?;
    let sid2 = send_paired(&mut w2, &cfg).await?;
    println!("[node] client reconnected; paired (conn 2)  sid={sid2}");
    println!("[node] <-- resume mid={}…  last_seq={last_seq}", &mid[..8]);
    wire::send(
        &mut w2,
        &Frame::new(
            Some(sid2.clone()),
            new_mid(),
            Body::Resume {
                intent_mid: mid.clone(),
                last_seq,
            },
        ),
    )
    .await?;

    let mut tail = String::new();
    loop {
        let Some(frame) = wire::recv(&mut r2).await? else {
            return Err(anyhow!("channel dropped again during resume"));
        };
        if frame.mid != mid {
            continue;
        }
        match frame.body {
            Body::WorkChunk { seq, delta } => {
                let t = chunk_text(&delta);
                tail.push_str(t);
                println!("[node]     resumed seq={seq}  {t:?}");
            }
            Body::WorkDone { usage, .. } => {
                println!("[node]     done after resume; recovered tail (seq>{last_seq}) = {tail:?}");
                println!(
                    "[node]     usage in={} out={}",
                    usage.input_tokens, usage.output_tokens
                );
                break;
            }
            Body::WorkError { code, message, .. } => {
                println!("[node]     error {code}: {message}");
                break;
            }
            _ => {}
        }
    }

    // ---- cancel: a dropped channel suspends, an explicit cancel aborts ----
    let mid2 = new_mid();
    let req2 = json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "Begin a long answer that the owner will cancel partway through."}], "stream": true});
    println!(
        "\n[node] --> work mid={}…  model=gpt-4o (will cancel)",
        &mid2[..8]
    );
    wire::send(
        &mut w2,
        &Frame::new(
            Some(sid2.clone()),
            mid2.clone(),
            Body::Work {
                provider: "mock".into(),
                request: req2,
            },
        ),
    )
    .await?;
    let mut got2 = 0;
    loop {
        let Some(frame) = wire::recv(&mut r2).await? else {
            break;
        };
        if frame.mid != mid2 {
            continue;
        }
        match frame.body {
            Body::WorkAccepted {} => println!("[node]     accepted"),
            Body::WorkChunk { seq, .. } => {
                println!("[node]     chunk seq={seq}");
                got2 += 1;
                if got2 >= 2 {
                    println!("[node]     >>> cancel mid={}…", &mid2[..8]);
                    wire::send(
                        &mut w2,
                        &Frame::new(
                            Some(sid2.clone()),
                            new_mid(),
                            Body::Cancel {
                                intent_mid: mid2.clone(),
                            },
                        ),
                    )
                    .await?;
                }
            }
            Body::WorkError { code, message, .. } => {
                println!("[node]     terminated: {code} ({message})");
                break;
            }
            Body::WorkDone { .. } => {
                println!("[node]     (finished before cancel took effect)");
                break;
            }
            _ => {}
        }
    }

    wire::send(
        &mut w2,
        &Frame::new(
            Some(sid2),
            new_mid(),
            Body::Close {
                reason: "resume demo done".into(),
            },
        ),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;

    fn cfg(authorized: Option<Vec<String>>) -> NodeConfig {
        NodeConfig {
            name: "o".into(),
            id: "node".into(),
            pairing_token: "pt".into(),
            root: SigningKey::generate(&mut OsRng),
            authorized_clients: authorized,
            claimed_tokens: Default::default(),
            single_use_token: true,
            intents: vec![],
        }
    }

    fn hello(token: &str, key: &SigningKey, with_sig: bool) -> Body {
        Body::Hello {
            pairing_token: token.into(),
            client: Peer {
                name: "e".into(),
                version: None,
                id: None,
            },
            providers: vec![],
            policy_digest: "d".into(),
            pubkey: Some(wire::hex(&key.verifying_key().to_bytes())),
            sig: with_sig.then(|| crate::identity::sign_detached(key, token.as_bytes())),
            route_token: None,
        }
    }

    fn pk_hex(key: &SigningKey) -> String {
        wire::hex(&key.verifying_key().to_bytes())
    }

    #[test]
    fn authorized_client_accepted() {
        let k = SigningKey::generate(&mut OsRng);
        let c = cfg(Some(vec![pk_hex(&k)]));
        assert!(authenticate_client(&hello("pt", &k, true), &c).is_ok());
    }

    #[test]
    fn unauthorized_client_rejected() {
        let k = SigningKey::generate(&mut OsRng);
        let someone_else = pk_hex(&SigningKey::generate(&mut OsRng));
        let c = cfg(Some(vec![someone_else])); // allow-list does NOT include k
        assert!(authenticate_client(&hello("pt", &k, true), &c).is_err());
    }

    #[test]
    fn forged_signature_rejected() {
        let k = SigningKey::generate(&mut OsRng);
        let c = cfg(Some(vec![pk_hex(&k)]));
        // claim k's pubkey but sign with an imposter key
        let mut h = hello("pt", &k, false);
        if let Body::Hello { sig, .. } = &mut h {
            let imposter = SigningKey::generate(&mut OsRng);
            *sig = Some(crate::identity::sign_detached(&imposter, b"pt"));
        }
        assert!(authenticate_client(&h, &c).is_err());
    }

    #[test]
    fn allowlist_requires_an_identity() {
        let k = SigningKey::generate(&mut OsRng);
        let c = cfg(Some(vec![pk_hex(&k)]));
        assert!(authenticate_client(&hello("pt", &k, false), &c).is_err()); // no sig
    }

    #[test]
    fn no_allowlist_accepts_valid_possession() {
        let k = SigningKey::generate(&mut OsRng);
        assert!(authenticate_client(&hello("pt", &k, true), &cfg(None)).is_ok());
    }

    #[test]
    fn wrong_pairing_token_rejected() {
        let k = SigningKey::generate(&mut OsRng);
        let c = cfg(Some(vec![pk_hex(&k)]));
        assert!(authenticate_client(&hello("WRONG", &k, true), &c).is_err());
    }

    #[test]
    fn token_binds_to_one_identity() {
        let c = cfg(None);
        let alice = SigningKey::generate(&mut OsRng);
        let bob = SigningKey::generate(&mut OsRng);
        // Alice claims the token, and may re-present it (reconnect / resume).
        assert!(authenticate_client(&hello("pt", &alice, true), &c).is_ok());
        assert!(authenticate_client(&hello("pt", &alice, true), &c).is_ok());
        // A different identity is refused the already-claimed token.
        assert!(authenticate_client(&hello("pt", &bob, true), &c).is_err());
    }
}
