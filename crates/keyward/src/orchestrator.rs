//! A mock Orchestrator — the "app". Holds NO provider key. It issues a one-time
//! pairing token, proves its identity by signing the assigned `sid` with its
//! Ed25519 key (§3/§9), then sends work intents and reads the relayed stream.
//!
//! This is a test/demo harness, not a product surface. The real Orchestrator is
//! whatever app integrates the (future) SDK.

use anyhow::{anyhow, bail, Result};
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{Sink, Stream, StreamExt};
use keyward_proto::{Body, Frame, Peer};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use crate::executor::new_mid;
use crate::wire;

pub struct OrchestratorConfig {
    pub name: String,
    pub id: String,
    pub pairing_token: String,
    /// Long-term root identity. Each connection mints a fresh operational key
    /// delegated by this root (§3), so the Executor pins the root once.
    pub root: SigningKey,
    /// Optional allow-list of authorized Executor identity pubkeys (hex). `None`
    /// accepts any Executor that proves possession of its key; `Some` lets a SaaS
    /// admit only registered users (§9).
    pub authorized_executors: Option<Vec<String>>,
    /// Scripted intents: (provider, native request body without credential).
    pub intents: Vec<(String, Value)>,
}

/// Serve a single dialed-in Executor through pairing + the scripted intents.
pub async fn serve(stream: TcpStream, cfg: OrchestratorConfig) -> Result<()> {
    let ws = accept_async(stream).await?;
    let (mut write, mut read) = ws.split();

    // 1. expect hello, authenticate the Executor (token + identity + allow-list).
    let hello = wire::recv(&mut read)
        .await?
        .ok_or_else(|| anyhow!("closed before hello"))?;
    if let Err(e) = authenticate_executor(&hello.body, &cfg) {
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
        println!("\n[orchestr] --> work #{i}  provider={provider}  model={model}");
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
                Body::WorkAccepted {} => println!("[orchestr]     accepted"),
                Body::WorkChunk { seq, delta } => {
                    let piece = chunk_text(&delta);
                    assembled.push_str(piece);
                    println!("[orchestr]     chunk seq={seq}  {piece:?}");
                }
                Body::WorkDone { usage, .. } => {
                    println!("[orchestr]     done  assembled={assembled:?}");
                    println!(
                        "[orchestr]     usage in={} out={}",
                        usage.input_tokens, usage.output_tokens
                    );
                    break;
                }
                Body::WorkError { code, message, .. } => {
                    println!("[orchestr]     ERROR  {code}: {message}");
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

/// Standalone single-prompt Orchestrator for manual two-terminal testing.
pub async fn run_cli() -> Result<()> {
    use rand_core::OsRng;
    use tokio::net::TcpListener;

    let listen = std::env::var("KEYWARD_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let token = std::env::var("KEYWARD_PAIRING_TOKEN").unwrap_or_else(|_| "pt_dev_token".into());
    let provider = std::env::var("KEYWARD_PROVIDER").unwrap_or_else(|_| "mock".into());
    let model = std::env::var("KEYWARD_MODEL").unwrap_or_else(|_| "gpt-4o".into());
    let prompt =
        std::env::var("KEYWARD_PROMPT").unwrap_or_else(|_| "Hello from a Keyward orchestrator.".into());

    let listener = TcpListener::bind(&listen).await?;
    println!("[orchestr] listening on ws://{listen}   pairing_token={token}");
    println!("[orchestr] run the executor with:");
    println!("           KEYWARD_ORCH_URL=ws://{listen} KEYWARD_PAIRING_TOKEN={token} cargo run -- executor");
    let (stream, _) = listener.accept().await?;

    // Optional SaaS allow-list of Executor identity pubkeys (comma-separated hex).
    let authorized_executors = std::env::var("KEYWARD_AUTHORIZED_EXECUTORS").ok().map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
    });

    let cfg = OrchestratorConfig {
        name: "keyward-orch".into(),
        id: "orch_dev".into(),
        pairing_token: token,
        root: SigningKey::generate(&mut OsRng),
        authorized_executors,
        intents: vec![(
            provider,
            json!({"model": model, "messages": [{"role": "user", "content": prompt}], "stream": true}),
        )],
    };
    serve(stream, cfg).await
}

/// Extract the text delta from a native chunk in either dialect — the demo's
/// stand-in for what a real provider SDK does on the Orchestrator side. Keyward
/// itself never looks inside the chunk; it just relays the bytes.
fn chunk_text(delta: &Value) -> &str {
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

/// Authenticate the Executor from its `hello` (§9): the pairing token, a possession
/// proof (the Executor's signature over the token with its identity key), and — when
/// the Orchestrator runs an allow-list — that the identity is authorized. This lets a
/// SaaS admit only registered users, without ever weakening the Owner's ability to
/// inspect their own key. (The v0 skeleton relaxes single-use tokens so the resume
/// demo can re-pair; a real Orchestrator would not.)
fn authenticate_executor(hello: &Body, cfg: &OrchestratorConfig) -> Result<()> {
    let Body::Hello {
        pairing_token,
        executor,
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
                .map_err(|_| anyhow!("executor identity signature invalid"))?;
        }
        _ => {
            if cfg.authorized_executors.is_some() {
                bail!("executor identity required but not provided");
            }
        }
    }
    if let Some(allow) = &cfg.authorized_executors {
        let pk = pubkey.as_deref().unwrap_or("");
        if !allow.iter().any(|a| a == pk) {
            bail!("executor not authorized");
        }
    }
    let fp = pubkey
        .as_deref()
        .and_then(wire::unhex)
        .filter(|b| b.len() >= 8)
        .map(|b| wire::fingerprint(&b))
        .unwrap_or_else(|| "none".into());
    println!(
        "[orchestr] hello from executor '{}'  identity fp={fp}",
        executor.name
    );
    Ok(())
}

// --- resume / cancel demo (two connections) -------------------------------

async fn authenticate_hello<S>(read: &mut S, cfg: &OrchestratorConfig) -> Result<()>
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let hello = wire::recv(read)
        .await?
        .ok_or_else(|| anyhow!("closed before hello"))?;
    authenticate_executor(&hello.body, cfg)
}

async fn send_paired<S>(write: &mut S, cfg: &OrchestratorConfig) -> Result<String>
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
fn build_paired(cfg: &OrchestratorConfig) -> (String, Frame) {
    let sid = format!("kw_sess_{}", &new_mid()[..8]);
    let op = SigningKey::generate(&mut rand_core::OsRng);
    let cert =
        crate::identity::issue_op_cert(&cfg.root, &op.verifying_key(), crate::identity::now_unix() + 3600);
    let sig = op.sign(sid.as_bytes());
    let frame = Frame::new(
        Some(sid.clone()),
        new_mid(),
        Body::Paired {
            orchestrator: Peer {
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
/// Executor re-dial, resume from `last_seq`, then deliberately cancel a second
/// intent. The Executor's producer keeps running across the drop, so resume
/// replays the chunks the Orchestrator missed.
pub async fn serve_resume_demo(listener: TcpListener, cfg: OrchestratorConfig) -> Result<()> {
    // ---- connection 1: stream, then drop mid-way ----
    let (s1, _) = listener.accept().await?;
    let (mut w1, mut r1) = accept_async(s1).await?.split();
    authenticate_hello(&mut r1, &cfg).await?;
    let sid1 = send_paired(&mut w1, &cfg).await?;
    println!("[orchestr] paired (conn 1)  sid={sid1}");

    let mid = new_mid();
    let req = json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "Stream a sentence long enough to span many chunks so we can interrupt it midway and resume."}], "stream": true});
    println!("\n[orchestr] --> work mid={}…  model=gpt-4o", &mid[..8]);
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
            Body::WorkAccepted {} => println!("[orchestr]     accepted"),
            Body::WorkChunk { seq, delta } => {
                println!("[orchestr]     chunk seq={seq}  {:?}", chunk_text(&delta));
                last_seq = seq as i64;
                got += 1;
                if got >= 3 {
                    break;
                }
            }
            Body::WorkDone { .. } => {
                println!("[orchestr]     (completed before we could interrupt)");
                break;
            }
            Body::WorkError { code, message, .. } => {
                println!("[orchestr]     error {code}: {message}");
                return Ok(());
            }
            _ => {}
        }
    }
    println!("[orchestr] !!! dropping the socket after seq={last_seq} — simulated channel loss\n");
    drop(w1);
    drop(r1);

    // ---- connection 2: the Executor re-dials; resume ----
    let (s2, _) = listener.accept().await?;
    let (mut w2, mut r2) = accept_async(s2).await?.split();
    authenticate_hello(&mut r2, &cfg).await?;
    let sid2 = send_paired(&mut w2, &cfg).await?;
    println!("[orchestr] executor reconnected; paired (conn 2)  sid={sid2}");
    println!("[orchestr] <-- resume mid={}…  last_seq={last_seq}", &mid[..8]);
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
                println!("[orchestr]     resumed seq={seq}  {t:?}");
            }
            Body::WorkDone { usage, .. } => {
                println!("[orchestr]     done after resume; recovered tail (seq>{last_seq}) = {tail:?}");
                println!(
                    "[orchestr]     usage in={} out={}",
                    usage.input_tokens, usage.output_tokens
                );
                break;
            }
            Body::WorkError { code, message, .. } => {
                println!("[orchestr]     error {code}: {message}");
                break;
            }
            _ => {}
        }
    }

    // ---- cancel: a dropped channel suspends, an explicit cancel aborts ----
    let mid2 = new_mid();
    let req2 = json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "Begin a long answer that the owner will cancel partway through."}], "stream": true});
    println!(
        "\n[orchestr] --> work mid={}…  model=gpt-4o (will cancel)",
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
            Body::WorkAccepted {} => println!("[orchestr]     accepted"),
            Body::WorkChunk { seq, .. } => {
                println!("[orchestr]     chunk seq={seq}");
                got2 += 1;
                if got2 >= 2 {
                    println!("[orchestr]     >>> cancel mid={}…", &mid2[..8]);
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
                println!("[orchestr]     terminated: {code} ({message})");
                break;
            }
            Body::WorkDone { .. } => {
                println!("[orchestr]     (finished before cancel took effect)");
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

    fn cfg(authorized: Option<Vec<String>>) -> OrchestratorConfig {
        OrchestratorConfig {
            name: "o".into(),
            id: "orch".into(),
            pairing_token: "pt".into(),
            root: SigningKey::generate(&mut OsRng),
            authorized_executors: authorized,
            intents: vec![],
        }
    }

    fn hello(token: &str, key: &SigningKey, with_sig: bool) -> Body {
        Body::Hello {
            pairing_token: token.into(),
            executor: Peer {
                name: "e".into(),
                version: None,
                id: None,
            },
            providers: vec![],
            policy_digest: "d".into(),
            pubkey: Some(wire::hex(&key.verifying_key().to_bytes())),
            sig: with_sig.then(|| crate::identity::sign_detached(key, token.as_bytes())),
        }
    }

    fn pk_hex(key: &SigningKey) -> String {
        wire::hex(&key.verifying_key().to_bytes())
    }

    #[test]
    fn authorized_executor_accepted() {
        let k = SigningKey::generate(&mut OsRng);
        let c = cfg(Some(vec![pk_hex(&k)]));
        assert!(authenticate_executor(&hello("pt", &k, true), &c).is_ok());
    }

    #[test]
    fn unauthorized_executor_rejected() {
        let k = SigningKey::generate(&mut OsRng);
        let someone_else = pk_hex(&SigningKey::generate(&mut OsRng));
        let c = cfg(Some(vec![someone_else])); // allow-list does NOT include k
        assert!(authenticate_executor(&hello("pt", &k, true), &c).is_err());
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
        assert!(authenticate_executor(&h, &c).is_err());
    }

    #[test]
    fn allowlist_requires_an_identity() {
        let k = SigningKey::generate(&mut OsRng);
        let c = cfg(Some(vec![pk_hex(&k)]));
        assert!(authenticate_executor(&hello("pt", &k, false), &c).is_err()); // no sig
    }

    #[test]
    fn no_allowlist_accepts_valid_possession() {
        let k = SigningKey::generate(&mut OsRng);
        assert!(authenticate_executor(&hello("pt", &k, true), &cfg(None)).is_ok());
    }

    #[test]
    fn wrong_pairing_token_rejected() {
        let k = SigningKey::generate(&mut OsRng);
        let c = cfg(Some(vec![pk_hex(&k)]));
        assert!(authenticate_executor(&hello("WRONG", &k, true), &c).is_err());
    }
}
