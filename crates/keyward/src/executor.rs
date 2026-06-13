//! The Executor: runs on the Owner's side, holds the key, dials OUT to the
//! Orchestrator, authenticates it (pinned Ed25519 identity, §9), enforces policy
//! (§6), injects the credential locally, and relays the provider stream (§5).

use std::sync::Arc;

use anyhow::{anyhow, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use futures_util::StreamExt;
use keyward_proto::{Body, Frame, Live, Peer, Policy};
use secrecy::SecretString;
use serde_json::Value;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::connect_async;

use crate::pricing;
use crate::provider::{self, Event};
use crate::wire;

pub fn new_mid() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub struct ExecutorConfig {
    pub name: String,
    pub providers: Vec<String>,
    pub policy: Policy,
    /// The provider credential. Held only here; never serialized onto the wire.
    pub provider_key: SecretString,
    /// TOFU store of the Orchestrator's pinned identity key (None until first contact).
    pub pinned: Arc<Mutex<Option<VerifyingKey>>>,
}

#[derive(Default)]
struct Runtime {
    spent_usd: f64,
    rpm_used: u32,
}

/// Dial out to `url`, pair, and serve work until the channel closes.
pub async fn run(url: &str, pairing_token: &str, cfg: ExecutorConfig) -> Result<()> {
    let (ws, _resp) = connect_async(url)
        .await
        .map_err(|e| anyhow!("dial-out to {url} failed: {e}"))?;
    let (mut write, mut read) = ws.split();

    // One writer task owns the sink; every producer sends frames through `out`.
    // The bounded channel is the outbound backpressure point.
    let (out, mut out_rx) = mpsc::channel::<Frame>(64);
    let writer = tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            if wire::send(&mut write, &frame).await.is_err() {
                break;
            }
        }
    });

    // hello (§3)
    let policy_canon = serde_json::to_string(&cfg.policy).unwrap_or_default();
    let hello = Frame::new(
        None,
        new_mid(),
        Body::Hello {
            pairing_token: pairing_token.to_string(),
            executor: Peer {
                name: cfg.name.clone(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                id: None,
            },
            providers: cfg.providers.clone(),
            policy_digest: wire::policy_digest_placeholder(&policy_canon),
            pubkey: None,
        },
    );
    out.send(hello).await.ok();

    let cfg = Arc::new(cfg);
    let rt = Arc::new(Mutex::new(Runtime::default()));
    let mut sid: Option<String> = None;
    let mut orch_id = String::from("unknown");

    while let Some(frame) = wire::recv(&mut read).await? {
        match frame.body {
            Body::Paired { orchestrator, pubkey, sig } => {
                let s = frame.sid.clone().unwrap_or_default();
                verify_and_pin(&cfg.pinned, &s, &pubkey, &sig).await?;
                orch_id = orchestrator.id.clone().unwrap_or_else(|| orchestrator.name.clone());
                println!("[executor] paired  sid={s}  orchestrator={orch_id}");
                sid = Some(s);
            }
            Body::Work { provider, request } => {
                let mid = frame.mid.clone();
                let (sid_c, cfg_c, rt_c, out_c, orch) =
                    (sid.clone(), cfg.clone(), rt.clone(), out.clone(), orch_id.clone());
                tokio::spawn(async move {
                    handle_work(mid, sid_c, provider, request, cfg_c, rt_c, out_c, orch).await;
                });
            }
            Body::Close { reason } => {
                println!("[executor] channel closed by orchestrator: {reason}");
                break;
            }
            other => {
                let _ = out
                    .send(Frame::new(
                        sid.clone(),
                        frame.mid,
                        Body::Error {
                            code: "unsupported_type".into(),
                            message: format!("executor does not handle this type: {:?}", Discriminant(&other)),
                        },
                    ))
                    .await;
            }
        }
    }

    drop(out);
    let _ = writer.await;
    Ok(())
}

// Tiny helper so we can debug-print "which variant" without leaking contents.
struct Discriminant<'a>(&'a Body);
impl std::fmt::Debug for Discriminant<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", std::mem::discriminant(self.0))
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_work(
    mid: String,
    sid: Option<String>,
    provider: String,
    request: Value,
    cfg: Arc<ExecutorConfig>,
    rt: Arc<Mutex<Runtime>>,
    out: mpsc::Sender<Frame>,
    orchestrator: String,
) {
    let model = request.get("model").and_then(Value::as_str).unwrap_or("").to_string();

    // Policy check in the §6 order, against live counters.
    let denied = {
        let r = rt.lock().await;
        let live = Live { rpm_used: r.rpm_used, spent_usd: r.spent_usd, now_rfc3339: "" };
        cfg.policy.check(&provider, &model, &orchestrator, live).err()
    };
    if let Some(d) = denied {
        println!("[executor] DENY  provider={provider} model={model}  ->  {}", d.code());
        let _ = out
            .send(Frame::new(
                sid,
                mid,
                Body::WorkError {
                    code: d.code().into(),
                    message: "rejected by policy".into(),
                    provider_status: None,
                },
            ))
            .await;
        return; // provider never contacted
    }

    {
        let mut r = rt.lock().await;
        r.rpm_used += 1;
    }
    let _ = out.send(Frame::new(sid.clone(), mid.clone(), Body::WorkAccepted {})).await;
    println!("[executor] ACCEPT provider={provider} model={model}  (key injected locally, never sent upstream-of-executor)");

    let mut rx = match provider::call(&provider, &model, &request, &cfg.provider_key).await {
        Ok(rx) => rx,
        Err(e) => {
            let _ = out
                .send(Frame::new(
                    sid,
                    mid,
                    Body::WorkError {
                        code: "provider_network".into(),
                        message: e.to_string(),
                        provider_status: None,
                    },
                ))
                .await;
            return;
        }
    };

    let mut seq: u64 = 0;
    while let Some(ev) = rx.recv().await {
        match ev {
            Event::Chunk(delta) => {
                if out
                    .send(Frame::new(sid.clone(), mid.clone(), Body::WorkChunk { seq, delta }))
                    .await
                    .is_err()
                {
                    return;
                }
                seq += 1;
            }
            Event::Done { result, usage } => {
                let cost = pricing::cost_usd(&model, &usage);
                {
                    let mut r = rt.lock().await;
                    r.spent_usd += cost;
                    println!(
                        "[executor] DONE  {seq} chunks  usage in={} out={}  cost=${:.5}  window_spent=${:.5}",
                        usage.input_tokens, usage.output_tokens, cost, r.spent_usd
                    );
                }
                let _ = out.send(Frame::new(sid, mid, Body::WorkDone { result, usage })).await;
                return;
            }
        }
    }
}

/// Verify the Orchestrator's signature over the assigned `sid`, then pin its key
/// (TOFU). A reconnect under a *different* key, or a bad signature, is refused —
/// this is what makes a stolen pairing token alone insufficient to bind (§9).
async fn verify_and_pin(
    pinned: &Arc<Mutex<Option<VerifyingKey>>>,
    sid: &str,
    pubkey_hex: &str,
    sig_hex: &str,
) -> Result<()> {
    let pk: [u8; 32] = wire::unhex(pubkey_hex)
        .ok_or_else(|| anyhow!("bad pubkey hex"))?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("pubkey must be 32 bytes"))?;
    let vk = VerifyingKey::from_bytes(&pk).map_err(|e| anyhow!("bad pubkey: {e}"))?;
    let sig: [u8; 64] = wire::unhex(sig_hex)
        .ok_or_else(|| anyhow!("bad sig hex"))?
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("sig must be 64 bytes"))?;
    let sig = Signature::from_bytes(&sig);
    vk.verify(sid.as_bytes(), &sig)
        .map_err(|_| anyhow!("orchestrator signature INVALID over sid — refusing to bind"))?;

    let mut guard = pinned.lock().await;
    match guard.as_ref() {
        Some(existing) if existing.to_bytes() != vk.to_bytes() => Err(anyhow!(
            "pinned orchestrator key changed — refusing (possible impersonation / stolen token)"
        )),
        Some(_) => Ok(()), // matches the pin
        None => {
            println!("[executor] TOFU: pinning orchestrator key  fp={}", wire::fingerprint(&vk.to_bytes()));
            *guard = Some(vk);
            Ok(())
        }
    }
}

/// Standalone executor: dial a real Orchestrator using env config.
pub async fn run_cli() -> Result<()> {
    let url = std::env::var("KEYWARD_ORCH_URL")
        .map_err(|_| anyhow!("set KEYWARD_ORCH_URL, e.g. ws://127.0.0.1:8787"))?;
    let token = std::env::var("KEYWARD_PAIRING_TOKEN")
        .map_err(|_| anyhow!("set KEYWARD_PAIRING_TOKEN"))?;
    let key = std::env::var("KEYWARD_PROVIDER_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default();
    let providers: Vec<String> = std::env::var("KEYWARD_PROVIDERS")
        .unwrap_or_else(|_| "mock,openai".into())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let policy = Policy {
        providers: Some(providers.clone()),
        budget: Some(keyward_proto::Budget { limit_usd: 5.0, window: "month".into(), spent_usd: 0.0 }),
        rate: Some(keyward_proto::Rate { rpm: Some(60), tpm: None }),
        ..Default::default()
    };
    let cfg = ExecutorConfig {
        name: "keyward-exec".into(),
        providers,
        policy,
        provider_key: SecretString::from(key),
        pinned: Arc::new(Mutex::new(None)),
    };
    println!("[executor] dialing {url} …");
    run(&url, &token, cfg).await
}
