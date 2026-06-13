//! A mock Orchestrator — the "app". Holds NO provider key. It issues a one-time
//! pairing token, proves its identity by signing the assigned `sid` with its
//! Ed25519 key (§3/§9), then sends work intents and reads the relayed stream.
//!
//! This is a test/demo harness, not a product surface. The real Orchestrator is
//! whatever app integrates the (future) SDK.

use anyhow::{anyhow, Result};
use ed25519_dalek::{Signer, SigningKey};
use futures_util::StreamExt;
use keyward_proto::{Body, Frame, Peer};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::accept_async;

use crate::executor::new_mid;
use crate::wire;

pub struct OrchestratorConfig {
    pub name: String,
    pub id: String,
    pub pairing_token: String,
    pub signing: SigningKey,
    /// Scripted intents: (provider, native request body without credential).
    pub intents: Vec<(String, Value)>,
}

/// Serve a single dialed-in Executor through pairing + the scripted intents.
pub async fn serve(stream: TcpStream, cfg: OrchestratorConfig) -> Result<()> {
    let ws = accept_async(stream).await?;
    let (mut write, mut read) = ws.split();

    // 1. expect hello, check the one-time pairing token.
    let hello = wire::recv(&mut read).await?.ok_or_else(|| anyhow!("closed before hello"))?;
    let exec_name = match &hello.body {
        Body::Hello { pairing_token, executor, .. } => {
            if pairing_token != &cfg.pairing_token {
                let _ = wire::send(
                    &mut write,
                    &Frame::new(None, new_mid(), Body::Error {
                        code: "bad_request".into(),
                        message: "pairing token rejected".into(),
                    }),
                )
                .await;
                return Err(anyhow!("pairing token mismatch"));
            }
            executor.name.clone()
        }
        _ => return Err(anyhow!("expected hello")),
    };
    println!("[orchestr] hello from executor '{exec_name}'");

    // 2. assign sid, sign it (identity proof), send paired.
    let sid = format!("kw_sess_{}", &new_mid()[..8]);
    let sig = cfg.signing.sign(sid.as_bytes());
    let vk = cfg.signing.verifying_key();
    println!("[orchestr] signing sid with identity key  fp={}", wire::fingerprint(&vk.to_bytes()));
    wire::send(
        &mut write,
        &Frame::new(Some(sid.clone()), new_mid(), Body::Paired {
            orchestrator: Peer { name: cfg.name.clone(), version: None, id: Some(cfg.id.clone()) },
            pubkey: wire::hex(&vk.to_bytes()),
            sig: wire::hex(&sig.to_bytes()),
        }),
    )
    .await?;

    // 3. run scripted intents, sequentially for clear output.
    for (i, (provider, request)) in cfg.intents.into_iter().enumerate() {
        let mid = new_mid();
        let model = request.get("model").and_then(Value::as_str).unwrap_or("?").to_string();
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
                    println!("[orchestr]     usage in={} out={}", usage.input_tokens, usage.output_tokens);
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
        &Frame::new(Some(sid), new_mid(), Body::Close { reason: "done".into() }),
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
    let prompt = std::env::var("KEYWARD_PROMPT").unwrap_or_else(|_| "Hello from a Keyward orchestrator.".into());

    let listener = TcpListener::bind(&listen).await?;
    println!("[orchestr] listening on ws://{listen}   pairing_token={token}");
    println!("[orchestr] run the executor with:");
    println!("           KEYWARD_ORCH_URL=ws://{listen} KEYWARD_PAIRING_TOKEN={token} cargo run -- executor");
    let (stream, _) = listener.accept().await?;

    let cfg = OrchestratorConfig {
        name: "keyward-orch".into(),
        id: "orch_dev".into(),
        pairing_token: token,
        signing: SigningKey::generate(&mut OsRng),
        intents: vec![(provider, json!({"model": model, "messages": [{"role": "user", "content": prompt}], "stream": true}))],
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
    if delta.get("type").and_then(Value::as_str) == Some("content_block_delta") {
        if let Some(t) = delta.pointer("/delta/text").and_then(Value::as_str) {
            return t; // Anthropic Messages
        }
    }
    ""
}
