//! The Executor: runs on the Owner's side, holds the key, dials OUT to the
//! Orchestrator, authenticates it (pinned Ed25519 identity, §9), enforces policy
//! (§6), injects the credential locally, and relays the provider stream (§5).
//!
//! Work is decoupled from the connection so a dropped channel SUSPENDS rather
//! than fails (§7): each intent has a producer task that keeps pulling from the
//! provider into a bounded ring buffer, and a per-connection delivery task that
//! streams from that buffer. On reconnect the Orchestrator sends `resume` and a
//! fresh delivery replays from `last_seq`. A deliberate `cancel` aborts; a
//! dropped channel does not.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use futures_util::StreamExt;
use keyward_proto::{Body, Frame, Live, Peer, Policy, Usage};
use secrecy::SecretString;
use serde_json::Value;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::connect_async;

use crate::pricing;
use crate::provider::{self, Event};
use crate::wire;

/// Recent chunks retained per intent for replay-on-resume (§7).
const RING_CAP: usize = 256;

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

/// Terminal outcome of an intent, retained until delivered.
enum Terminal {
    Done { result: Option<Value>, usage: Usage },
    Error { code: String, message: String, provider_status: Option<u16> },
}

/// Per-intent ring buffer of native chunks + terminal state.
struct IntentBuf {
    chunks: VecDeque<(u64, Value)>,
    base_seq: u64,
    next_seq: u64,
    terminal: Option<Terminal>,
}

impl IntentBuf {
    fn new() -> Self {
        IntentBuf { chunks: VecDeque::new(), base_seq: 0, next_seq: 0, terminal: None }
    }
    fn push(&mut self, delta: Value) {
        self.chunks.push_back((self.next_seq, delta));
        self.next_seq += 1;
        while self.chunks.len() > RING_CAP {
            self.chunks.pop_front();
            self.base_seq += 1;
        }
    }
}

struct Intent {
    buf: Mutex<IntentBuf>,
    cancelled: AtomicBool,
    cancel: Notify,
}

/// State that must outlive any single connection: in-flight intents (keyed by
/// `mid`, so a fresh `sid` on reconnect doesn't lose them) and the rate/budget
/// counters.
struct Shared {
    store: Mutex<HashMap<String, Arc<Intent>>>,
    runtime: Mutex<Runtime>,
}

enum Flow {
    Stop,
    Reconnect,
}

/// Dial out, pair, and serve work — reconnecting to resume in-flight intents if
/// the channel drops, up to a bounded number of attempts.
pub async fn run(url: &str, pairing_token: &str, cfg: ExecutorConfig) -> Result<()> {
    let cfg = Arc::new(cfg);
    let shared = Arc::new(Shared {
        store: Mutex::new(HashMap::new()),
        runtime: Mutex::new(Runtime::default()),
    });

    let mut attempt = 0u32;
    let mut backoff = 200u64;
    loop {
        match serve_once(url, pairing_token, &cfg, &shared).await {
            Ok(Flow::Stop) => return Ok(()),
            Ok(Flow::Reconnect) | Err(_) => {
                let pending = shared.store.lock().await.len();
                if pending == 0 {
                    return Ok(());
                }
                attempt += 1;
                if attempt > 12 {
                    return Err(anyhow!("giving up reconnecting with {pending} intent(s) in flight"));
                }
                println!("[executor] channel lost; {pending} intent(s) in flight, reconnecting (attempt {attempt})…");
                sleep(Duration::from_millis(backoff)).await;
                backoff = (backoff * 2).min(2000);
            }
        }
    }
}

async fn serve_once(url: &str, pairing_token: &str, cfg: &Arc<ExecutorConfig>, shared: &Arc<Shared>) -> Result<Flow> {
    let (ws, _resp) = connect_async(url).await.map_err(|e| anyhow!("dial-out to {url} failed: {e}"))?;
    let (mut write, mut read) = ws.split();

    let (out, mut out_rx) = mpsc::channel::<Frame>(64);
    let writer = tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            if wire::send(&mut write, &frame).await.is_err() {
                break;
            }
        }
    });

    out.send(hello_frame(cfg, pairing_token)).await.ok();

    let mut sid: Option<String> = None;
    let mut orch_id = String::from("unknown");

    let flow = loop {
        match wire::recv(&mut read).await {
            Ok(Some(frame)) => match frame.body {
                Body::Paired { orchestrator, pubkey, sig } => {
                    let s = frame.sid.clone().unwrap_or_default();
                    if let Err(e) = verify_and_pin(&cfg.pinned, &s, &pubkey, &sig).await {
                        eprintln!("[executor] {e}");
                        break Flow::Stop;
                    }
                    orch_id = orchestrator.id.clone().unwrap_or_else(|| orchestrator.name.clone());
                    println!("[executor] paired  sid={s}  orchestrator={orch_id}");
                    sid = Some(s);
                }
                Body::Work { provider, request } => {
                    spawn_work(frame.mid, sid.clone(), provider, request, cfg.clone(), shared.clone(), out.clone(), orch_id.clone());
                }
                Body::Resume { intent_mid, last_seq } => {
                    let start = if last_seq < 0 { 0 } else { last_seq as u64 + 1 };
                    spawn_resume(intent_mid, sid.clone(), shared.clone(), out.clone(), start);
                }
                Body::Cancel { intent_mid } => {
                    if let Some(intent) = shared.store.lock().await.get(&intent_mid) {
                        intent.cancelled.store(true, Ordering::SeqCst);
                        intent.cancel.notify_one();
                        println!("[executor] CANCEL {intent_mid} (aborting our read; note: provider may keep billing)");
                    }
                }
                Body::Close { reason } => {
                    println!("[executor] channel closed by orchestrator: {reason}");
                    break Flow::Stop;
                }
                _ => {}
            },
            Ok(None) => break Flow::Reconnect, // channel dropped without a Keyward close
            Err(e) => {
                eprintln!("[executor] recv error: {e}");
                break Flow::Reconnect;
            }
        }
    };

    writer.abort();
    Ok(flow)
}

fn hello_frame(cfg: &ExecutorConfig, pairing_token: &str) -> Frame {
    let policy_canon = serde_json::to_string(&cfg.policy).unwrap_or_default();
    Frame::new(
        None,
        new_mid(),
        Body::Hello {
            pairing_token: pairing_token.to_string(),
            executor: Peer { name: cfg.name.clone(), version: Some(env!("CARGO_PKG_VERSION").to_string()), id: None },
            providers: cfg.providers.clone(),
            policy_digest: wire::policy_digest_placeholder(&policy_canon),
            pubkey: None,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_work(
    mid: String,
    sid: Option<String>,
    provider: String,
    request: Value,
    cfg: Arc<ExecutorConfig>,
    shared: Arc<Shared>,
    out: mpsc::Sender<Frame>,
    orchestrator: String,
) {
    tokio::spawn(async move {
        let model = request.get("model").and_then(Value::as_str).unwrap_or("").to_string();

        // Policy check in the §6 order, against live counters.
        let denied = {
            let r = shared.runtime.lock().await;
            let live = Live { rpm_used: r.rpm_used, spent_usd: r.spent_usd, now_rfc3339: "" };
            cfg.policy.check(&provider, &model, &orchestrator, live).err()
        };
        if let Some(d) = denied {
            println!("[executor] DENY  provider={provider} model={model}  ->  {}", d.code());
            let _ = out
                .send(Frame::new(sid, mid, Body::WorkError { code: d.code().into(), message: "rejected by policy".into(), provider_status: None }))
                .await;
            return; // provider never contacted, nothing resumable
        }

        shared.runtime.lock().await.rpm_used += 1;
        let _ = out.send(Frame::new(sid.clone(), mid.clone(), Body::WorkAccepted {})).await;
        println!("[executor] ACCEPT provider={provider} model={model}  (key injected locally)");

        let intent = Arc::new(Intent { buf: Mutex::new(IntentBuf::new()), cancelled: AtomicBool::new(false), cancel: Notify::new() });
        shared.store.lock().await.insert(mid.clone(), intent.clone());

        spawn_producer(intent.clone(), provider, model, request, cfg, shared.clone(), mid.clone());
        deliver(intent, shared, out, sid, mid, 0).await;
    });
}

fn spawn_resume(mid: String, sid: Option<String>, shared: Arc<Shared>, out: mpsc::Sender<Frame>, start: u64) {
    tokio::spawn(async move {
        let intent = shared.store.lock().await.get(&mid).cloned();
        match intent {
            Some(intent) => {
                println!("[executor] RESUME {mid} from seq={start}");
                deliver(intent, shared, out, sid, mid, start).await;
            }
            None => {
                let _ = out
                    .send(Frame::new(sid, mid, Body::WorkError { code: "unrecoverable".into(), message: "no such in-flight intent".into(), provider_status: None }))
                    .await;
            }
        }
    });
}

fn spawn_producer(
    intent: Arc<Intent>,
    provider: String,
    model: String,
    request: Value,
    cfg: Arc<ExecutorConfig>,
    shared: Arc<Shared>,
    mid: String,
) {
    tokio::spawn(async move {
        let mut rx = match provider::call(&provider, &model, &request, &cfg.provider_key).await {
            Ok(rx) => rx,
            Err(e) => {
                intent.buf.lock().await.terminal = Some(Terminal::Error { code: "provider_network".into(), message: e.to_string(), provider_status: None });
                return;
            }
        };

        loop {
            if intent.cancelled.load(Ordering::SeqCst) {
                println!("[executor] {mid} cancelled — stopping read");
                intent.buf.lock().await.terminal = Some(Terminal::Error { code: "cancelled".into(), message: "cancelled by orchestrator".into(), provider_status: None });
                return;
            }
            tokio::select! {
                biased;
                _ = intent.cancel.notified() => { /* loop; the flag check above handles it */ }
                ev = rx.recv() => match ev {
                    Some(Event::Chunk(delta)) => intent.buf.lock().await.push(delta),
                    Some(Event::Done { result, usage }) => {
                        let cost = pricing::cost_usd(&model, &usage);
                        let spent = {
                            let mut r = shared.runtime.lock().await;
                            r.spent_usd += cost;
                            r.spent_usd
                        };
                        let n = { let mut b = intent.buf.lock().await; b.terminal = Some(Terminal::Done { result, usage: usage.clone() }); b.next_seq };
                        println!("[executor] DONE  {mid}: {n} chunks  usage in={} out={}  cost=${cost:.5}  window_spent=${spent:.5}", usage.input_tokens, usage.output_tokens);
                        return;
                    }
                    None => {
                        intent.buf.lock().await.terminal = Some(Terminal::Error { code: "provider_network".into(), message: "stream ended".into(), provider_status: None });
                        return;
                    }
                }
            }
        }
    });
}

/// Stream an intent to the current connection starting at `start_seq`: replay any
/// retained chunks, then tail live until the terminal frame. Returns on send
/// failure (channel dropped → suspend) without removing the intent.
async fn deliver(intent: Arc<Intent>, shared: Arc<Shared>, out: mpsc::Sender<Frame>, sid: Option<String>, mid: String, start_seq: u64) {
    let mut cursor = start_seq;
    loop {
        enum Step {
            Send(u64, Value),
            Terminal(Frame),
            Unrecoverable,
            Wait,
        }
        let step = {
            let buf = intent.buf.lock().await;
            if cursor < buf.base_seq {
                Step::Unrecoverable
            } else if cursor < buf.next_seq {
                let (seq, delta) = buf.chunks[(cursor - buf.base_seq) as usize].clone();
                Step::Send(seq, delta)
            } else if let Some(t) = &buf.terminal {
                Step::Terminal(terminal_frame(sid.clone(), mid.clone(), t))
            } else {
                Step::Wait
            }
        };
        match step {
            Step::Send(seq, delta) => {
                if out.send(Frame::new(sid.clone(), mid.clone(), Body::WorkChunk { seq, delta })).await.is_err() {
                    return; // suspend: connection dropped, keep the intent for resume
                }
                cursor += 1;
            }
            Step::Terminal(frame) => {
                let _ = out.send(frame).await;
                shared.store.lock().await.remove(&mid);
                return;
            }
            Step::Unrecoverable => {
                let _ = out
                    .send(Frame::new(sid, mid.clone(), Body::WorkError { code: "unrecoverable".into(), message: "resume past retained buffer".into(), provider_status: None }))
                    .await;
                shared.store.lock().await.remove(&mid);
                return;
            }
            Step::Wait => sleep(Duration::from_millis(5)).await,
        }
    }
}

fn terminal_frame(sid: Option<String>, mid: String, t: &Terminal) -> Frame {
    match t {
        Terminal::Done { result, usage } => Frame::new(sid, mid, Body::WorkDone { result: result.clone(), usage: usage.clone() }),
        Terminal::Error { code, message, provider_status } => {
            Frame::new(sid, mid, Body::WorkError { code: code.clone(), message: message.clone(), provider_status: *provider_status })
        }
    }
}

/// Verify the Orchestrator's signature over the assigned `sid`, then pin its key
/// (TOFU). A reconnect under a *different* key, or a bad signature, is refused —
/// this is what makes a stolen pairing token alone insufficient to bind (§9).
async fn verify_and_pin(pinned: &Arc<Mutex<Option<VerifyingKey>>>, sid: &str, pubkey_hex: &str, sig_hex: &str) -> Result<()> {
    let pk: [u8; 32] = wire::unhex(pubkey_hex).ok_or_else(|| anyhow!("bad pubkey hex"))?.as_slice().try_into().map_err(|_| anyhow!("pubkey must be 32 bytes"))?;
    let vk = VerifyingKey::from_bytes(&pk).map_err(|e| anyhow!("bad pubkey: {e}"))?;
    let sig: [u8; 64] = wire::unhex(sig_hex).ok_or_else(|| anyhow!("bad sig hex"))?.as_slice().try_into().map_err(|_| anyhow!("sig must be 64 bytes"))?;
    let sig = Signature::from_bytes(&sig);
    vk.verify(sid.as_bytes(), &sig).map_err(|_| anyhow!("orchestrator signature INVALID over sid — refusing to bind"))?;

    let mut guard = pinned.lock().await;
    match guard.as_ref() {
        Some(existing) if existing.to_bytes() != vk.to_bytes() => Err(anyhow!("pinned orchestrator key changed — refusing (possible impersonation / stolen token)")),
        Some(_) => Ok(()),
        None => {
            println!("[executor] TOFU: pinning orchestrator key  fp={}", wire::fingerprint(&vk.to_bytes()));
            *guard = Some(vk);
            Ok(())
        }
    }
}

/// Standalone executor: dial a real Orchestrator using env config.
pub async fn run_cli() -> Result<()> {
    let url = std::env::var("KEYWARD_ORCH_URL").map_err(|_| anyhow!("set KEYWARD_ORCH_URL, e.g. ws://127.0.0.1:8787"))?;
    let token = std::env::var("KEYWARD_PAIRING_TOKEN").map_err(|_| anyhow!("set KEYWARD_PAIRING_TOKEN"))?;
    let key = std::env::var("KEYWARD_PROVIDER_KEY").or_else(|_| std::env::var("OPENAI_API_KEY")).unwrap_or_default();
    let providers: Vec<String> = std::env::var("KEYWARD_PROVIDERS")
        .unwrap_or_else(|_| "mock,openai,anthropic".into())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ring_buffer_evicts_and_tracks_base() {
        let mut b = IntentBuf::new();
        for i in 0..(RING_CAP as u64 + 5) {
            b.push(json!({ "i": i }));
        }
        // The 5 oldest chunks are evicted; the window is the last RING_CAP seqs.
        assert_eq!(b.next_seq, RING_CAP as u64 + 5);
        assert_eq!(b.base_seq, 5);
        assert_eq!(b.chunks.len(), RING_CAP);
        assert_eq!(b.chunks.front().unwrap().0, 5);
        // A resume cursor below base_seq is what deliver() reports as unrecoverable.
        assert!(3 < b.base_seq, "seq 3 fell off the ring → unrecoverable");
    }
}
