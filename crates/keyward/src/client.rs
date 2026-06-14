//! The Client: runs on the Owner's side, holds the key, dials OUT to the
//! Node, authenticates it (pinned Ed25519 identity, §9), enforces policy
//! (§6), injects the credential locally, and relays the provider stream (§5).
//!
//! Work is decoupled from the connection so a dropped channel SUSPENDS rather
//! than fails (§7): each intent has a producer task that keeps pulling from the
//! provider into a bounded ring buffer, and a per-connection delivery task that
//! streams from that buffer. On reconnect the Node sends `resume` and a
//! fresh delivery replays from `last_seq`. A deliberate `cancel` aborts; a
//! dropped channel does not.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use ed25519_dalek::{SigningKey, VerifyingKey};
use keyward_proto::{Body, Frame, Live, Peer, Policy, Usage};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::{Mutex, Notify, mpsc};
use tokio::time::{Duration, sleep};

use crate::identity;
use crate::pricing;
use crate::provider::{self, Event};
use crate::secret::KeySource;
use crate::transport;
use crate::wire;

/// Recent chunks retained per intent for replay-on-resume (§7).
const RING_CAP: usize = 256;

pub fn new_mid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// A structured status event from the running Client. The CLI ignores these (it
/// prints), a UI streams them. Internally tagged so the frontend gets a clean TS
/// discriminated union (`{ kind: "paired", ... }`). Never affects protocol behavior.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ClientEvent {
    /// Dialing the Node.
    Connecting { url: String },
    /// Paired, and the Node's identity chain verified against the pinned root.
    Paired {
        node: String,
        root_fingerprint: String,
        sid: String,
    },
    /// A work intent passed policy and was accepted (credential injected locally).
    Accepted {
        mid: String,
        provider: String,
        model: String,
    },
    /// A work intent finished, with metered usage and cost.
    Done {
        mid: String,
        provider: String,
        model: String,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
        spent_usd: f64,
    },
    /// A work intent was refused by policy before the provider was contacted.
    Denied {
        mid: String,
        provider: String,
        model: String,
        code: String,
    },
    /// A work intent failed (provider/network error, or a cancel).
    WorkFailed {
        mid: String,
        code: String,
        message: String,
    },
    /// The channel dropped; in-flight intents are suspended, not failed (§7).
    ConnectionLost { pending: usize },
    /// Reconnecting to resume the suspended intents.
    Reconnecting { attempt: u32 },
    /// The Client stopped — clean close, fatal error, or giving up reconnecting.
    Stopped { reason: String },
}

pub struct ClientConfig {
    pub name: String,
    pub providers: Vec<String>,
    pub policy: Policy,
    /// How the Client gets each provider's credential. Held/resolved only here;
    /// never serialized onto the wire.
    pub keys: KeySource,
    /// The Client's long-term identity key. Its public half is sent in `hello`
    /// and signs the pairing token, so the Node can authenticate this
    /// Client (e.g. allow-list registered users, §9).
    pub identity: SigningKey,
    /// TOFU store of the Node's pinned identity key (None until first contact).
    pub pinned: Arc<Mutex<Option<VerifyingKey>>>,
    /// Optional structured-event sink. The CLI leaves this `None` (it prints to stdout);
    /// a UI sets it to stream [`ClientEvent`]s. Purely observational.
    pub events: Option<mpsc::UnboundedSender<ClientEvent>>,
}

#[derive(Default)]
struct Runtime {
    spent_usd: f64,
    rpm_used: u32,
}

/// Terminal outcome of an intent, retained until delivered.
enum Terminal {
    Done {
        result: Option<Value>,
        usage: Usage,
    },
    Error {
        code: String,
        message: String,
        provider_status: Option<u16>,
    },
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
        IntentBuf {
            chunks: VecDeque::new(),
            base_seq: 0,
            next_seq: 0,
            terminal: None,
        }
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
    events: Option<mpsc::UnboundedSender<ClientEvent>>,
}

impl Shared {
    /// Forward a status event to the UI sink, if any. Best-effort: a closed receiver
    /// is silently ignored (the client keeps running regardless).
    fn emit(&self, ev: ClientEvent) {
        if let Some(tx) = &self.events {
            let _ = tx.send(ev);
        }
    }
}

enum Flow {
    Stop,
    Reconnect,
}

/// Dial out, pair, and serve work — reconnecting to resume in-flight intents if
/// the channel drops, up to a bounded number of attempts.
pub async fn run(url: &str, pairing_token: &str, cfg: ClientConfig) -> Result<()> {
    let events = cfg.events.clone();
    let cfg = Arc::new(cfg);
    let shared = Arc::new(Shared {
        store: Mutex::new(HashMap::new()),
        runtime: Mutex::new(Runtime::default()),
        events,
    });

    shared.emit(ClientEvent::Connecting { url: url.to_string() });

    let mut attempt = 0u32;
    let mut backoff = 200u64;
    loop {
        match serve_once(url, pairing_token, &cfg, &shared).await {
            Ok(Flow::Stop) => return Ok(()),
            Ok(Flow::Reconnect) | Err(_) => {
                let pending = shared.store.lock().await.len();
                if pending == 0 {
                    shared.emit(ClientEvent::Stopped {
                        reason: "disconnected".into(),
                    });
                    return Ok(());
                }
                attempt += 1;
                if attempt > 12 {
                    shared.emit(ClientEvent::Stopped {
                        reason: format!("gave up reconnecting with {pending} intent(s) in flight"),
                    });
                    return Err(anyhow!(
                        "giving up reconnecting with {pending} intent(s) in flight"
                    ));
                }
                shared.emit(ClientEvent::ConnectionLost { pending });
                shared.emit(ClientEvent::Reconnecting { attempt });
                println!(
                    "[client] channel lost; {pending} intent(s) in flight, reconnecting (attempt {attempt})…"
                );
                sleep(Duration::from_millis(backoff)).await;
                backoff = (backoff * 2).min(2000);
            }
        }
    }
}

async fn serve_once(
    url: &str,
    pairing_token: &str,
    cfg: &Arc<ClientConfig>,
    shared: &Arc<Shared>,
) -> Result<Flow> {
    let (out, mut inbound) = transport::connect(url).await?;

    out.send(hello_frame(cfg, pairing_token)).await.ok();

    let mut sid: Option<String> = None;
    let mut node_id = String::from("unknown");
    let mut authenticated = false;

    let flow = loop {
        match inbound.recv().await {
            Some(frame) => match frame.body {
                Body::Paired {
                    node,
                    root_pubkey,
                    op,
                    sig,
                } => {
                    let s = frame.sid.clone().unwrap_or_default();
                    if let Err(e) = verify_chain_and_pin(&cfg.pinned, &s, &root_pubkey, &op, &sig).await {
                        eprintln!("[client] {e}");
                        shared.emit(ClientEvent::Stopped {
                            reason: e.to_string(),
                        });
                        break Flow::Stop;
                    }
                    node_id = node.id.clone().unwrap_or_else(|| node.name.clone());
                    let root_fingerprint = identity::parse_pubkey(&root_pubkey)
                        .map(|k| wire::fingerprint(&k.to_bytes()))
                        .unwrap_or_default();
                    println!("[client] paired  sid={s}  node={node_id}");
                    shared.emit(ClientEvent::Paired {
                        node: node_id.clone(),
                        root_fingerprint,
                        sid: s.clone(),
                    });
                    sid = Some(s);
                    authenticated = true;
                }
                Body::Work { provider, request } => {
                    if !authenticated {
                        let _ = out
                            .send(Frame::new(
                                sid.clone(),
                                frame.mid,
                                Body::WorkError {
                                    code: "bad_request".into(),
                                    message: "work before pairing".into(),
                                    provider_status: None,
                                },
                            ))
                            .await;
                        continue;
                    }
                    spawn_work(
                        frame.mid,
                        sid.clone(),
                        provider,
                        request,
                        cfg.clone(),
                        shared.clone(),
                        out.clone(),
                        node_id.clone(),
                    );
                }
                Body::Resume { intent_mid, last_seq } => {
                    if !authenticated {
                        continue;
                    }
                    let start = if last_seq < 0 { 0 } else { last_seq as u64 + 1 };
                    spawn_resume(intent_mid, sid.clone(), shared.clone(), out.clone(), start);
                }
                Body::Cancel { intent_mid } => {
                    if !authenticated {
                        continue;
                    }
                    if let Some(intent) = shared.store.lock().await.get(&intent_mid) {
                        intent.cancelled.store(true, Ordering::SeqCst);
                        intent.cancel.notify_one();
                        println!(
                            "[client] CANCEL {intent_mid} (aborting our read; note: provider may keep billing)"
                        );
                    }
                }
                Body::Close { reason } => {
                    println!("[client] channel closed by node: {reason}");
                    shared.emit(ClientEvent::Stopped {
                        reason: format!("closed by node: {reason}"),
                    });
                    break Flow::Stop;
                }
                #[cfg(feature = "seal")]
                Body::Sealed { blob } => {
                    if !authenticated {
                        continue;
                    }
                    spawn_sealed_work(
                        frame.mid,
                        sid.clone(),
                        blob,
                        cfg.clone(),
                        shared.clone(),
                        out.clone(),
                        node_id.clone(),
                    );
                }
                _ => {}
            },
            None => break Flow::Reconnect, // channel dropped → suspend & reconnect (§7)
        }
    };

    Ok(flow)
}

fn hello_frame(cfg: &ClientConfig, pairing_token: &str) -> Frame {
    let policy_canon = serde_json::to_string(&cfg.policy).unwrap_or_default();
    Frame::new(
        None,
        new_mid(),
        Body::Hello {
            pairing_token: pairing_token.to_string(),
            client: Peer {
                name: cfg.name.clone(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                id: None,
            },
            providers: cfg.providers.clone(),
            policy_digest: wire::policy_digest(&policy_canon),
            pubkey: Some(wire::hex(&cfg.identity.verifying_key().to_bytes())),
            sig: Some(identity::sign_detached(&cfg.identity, pairing_token.as_bytes())),
            // Routing token for a multi-tenant node (§8); the user puts the same
            // value in their app's API-key slot. Single-tenant nodes ignore it.
            route_token: std::env::var("KEYWARD_ROUTE_TOKEN")
                .ok()
                .filter(|s| !s.is_empty()),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_work(
    mid: String,
    sid: Option<String>,
    provider: String,
    request: Value,
    cfg: Arc<ClientConfig>,
    shared: Arc<Shared>,
    out: mpsc::Sender<Frame>,
    node: String,
) {
    tokio::spawn(async move {
        let model = request
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        // Policy check in the §6 order, against live counters.
        let denied = {
            let r = shared.runtime.lock().await;
            let live = Live {
                rpm_used: r.rpm_used,
                spent_usd: r.spent_usd,
                now_rfc3339: "",
            };
            cfg.policy.check(&provider, &model, &node, live).err()
        };
        if let Some(d) = denied {
            println!(
                "[client] DENY  provider={provider} model={model}  ->  {}",
                d.code()
            );
            shared.emit(ClientEvent::Denied {
                mid: mid.clone(),
                provider: provider.clone(),
                model: model.clone(),
                code: d.code().into(),
            });
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
            return; // provider never contacted, nothing resumable
        }

        shared.runtime.lock().await.rpm_used += 1;
        let _ = out
            .send(Frame::new(sid.clone(), mid.clone(), Body::WorkAccepted {}))
            .await;
        println!("[client] ACCEPT provider={provider} model={model}  (key injected locally)");
        shared.emit(ClientEvent::Accepted {
            mid: mid.clone(),
            provider: provider.clone(),
            model: model.clone(),
        });

        let intent = Arc::new(Intent {
            buf: Mutex::new(IntentBuf::new()),
            cancelled: AtomicBool::new(false),
            cancel: Notify::new(),
        });
        shared.store.lock().await.insert(mid.clone(), intent.clone());

        spawn_producer(
            intent.clone(),
            provider,
            model,
            request,
            cfg,
            shared.clone(),
            mid.clone(),
        );
        deliver(intent, shared, out, sid, mid, 0).await;
    });
}

/// Serve a sealed work intent (§9): decrypt with the Client's identity-derived key,
/// enforce policy, call the provider, and seal each response chunk back over the same
/// channel. The node only ever relayed ciphertext. A decryption failure is dropped —
/// the node can't help, and there is no shared channel to report an error over.
#[cfg(feature = "seal")]
#[allow(clippy::too_many_arguments)]
fn spawn_sealed_work(
    mid: String,
    sid: Option<String>,
    blob: String,
    cfg: Arc<ClientConfig>,
    shared: Arc<Shared>,
    out: mpsc::Sender<Frame>,
    node: String,
) {
    tokio::spawn(async move {
        // envelope = hex(ephemeral_pubkey)(64 chars) ‖ sealed(nonce‖ciphertext)
        if blob.len() < 64 {
            return;
        }
        let (epk, sealed_req) = blob.split_at(64);
        let Ok(channel) = crate::seal::Sealed::responder(&crate::seal::x25519_secret(&cfg.identity), epk)
        else {
            return;
        };
        let Ok(pt) = channel.open(sealed_req) else {
            return;
        };
        let inner: Value = serde_json::from_slice(&pt).unwrap_or(Value::Null);
        let provider = inner
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let request = inner.get("request").cloned().unwrap_or(Value::Null);
        let model = request
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        // Seal a JSON value into a response frame over the same channel.
        let reply = |v: Value| -> Frame {
            let blob = channel.seal(v.to_string().as_bytes()).unwrap_or_default();
            Frame::new(sid.clone(), mid.clone(), Body::Sealed { blob })
        };

        // Policy (§6) — same as the cleartext path.
        let denied = {
            let r = shared.runtime.lock().await;
            let live = Live {
                rpm_used: r.rpm_used,
                spent_usd: r.spent_usd,
                now_rfc3339: "",
            };
            cfg.policy.check(&provider, &model, &node, live).err()
        };
        if let Some(d) = denied {
            shared.emit(ClientEvent::Denied {
                mid: mid.clone(),
                provider: provider.clone(),
                model: model.clone(),
                code: d.code().into(),
            });
            // Terminals are cleartext: they carry only metadata (a code), never content,
            // and let the blind node manage the relay lifecycle.
            let _ = out
                .send(Frame::new(
                    sid.clone(),
                    mid.clone(),
                    Body::WorkError {
                        code: d.code().into(),
                        message: "rejected by policy".into(),
                        provider_status: None,
                    },
                ))
                .await;
            return;
        }
        shared.runtime.lock().await.rpm_used += 1;
        shared.emit(ClientEvent::Accepted {
            mid: mid.clone(),
            provider: provider.clone(),
            model: model.clone(),
        });
        println!(
            "[client] ACCEPT (sealed) provider={provider} model={model}  (decrypted locally; node saw only ciphertext)"
        );

        let key = cfg.keys.resolve(&provider);
        let mut rx = match provider::call(&provider, &model, &request, &key).await {
            Ok(rx) => rx,
            Err(e) => {
                shared.emit(ClientEvent::WorkFailed {
                    mid: mid.clone(),
                    code: "provider_network".into(),
                    message: e.to_string(),
                });
                let _ = out
                    .send(Frame::new(
                        sid.clone(),
                        mid.clone(),
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
        loop {
            match rx.recv().await {
                Some(Event::Chunk(delta)) => {
                    // Only the content is sealed.
                    let _ = out.send(reply(serde_json::json!({ "chunk": delta }))).await;
                }
                Some(Event::Done { usage, .. }) => {
                    let cost = pricing::cost_usd(&model, &usage);
                    let spent = {
                        let mut r = shared.runtime.lock().await;
                        r.spent_usd += cost;
                        r.spent_usd
                    };
                    shared.emit(ClientEvent::Done {
                        mid: mid.clone(),
                        provider: provider.clone(),
                        model: model.clone(),
                        input_tokens: usage.input_tokens,
                        output_tokens: usage.output_tokens,
                        cost_usd: cost,
                        spent_usd: spent,
                    });
                    println!(
                        "[client] DONE (sealed) {mid}  usage in={} out={}  cost=${cost:.5}",
                        usage.input_tokens, usage.output_tokens
                    );
                    let _ = out
                        .send(Frame::new(
                            sid.clone(),
                            mid.clone(),
                            Body::WorkDone { result: None, usage },
                        ))
                        .await;
                    return;
                }
                None => {
                    let _ = out
                        .send(Frame::new(
                            sid.clone(),
                            mid.clone(),
                            Body::WorkError {
                                code: "provider_network".into(),
                                message: "stream ended".into(),
                                provider_status: None,
                            },
                        ))
                        .await;
                    return;
                }
            }
        }
    });
}

fn spawn_resume(mid: String, sid: Option<String>, shared: Arc<Shared>, out: mpsc::Sender<Frame>, start: u64) {
    tokio::spawn(async move {
        let intent = shared.store.lock().await.get(&mid).cloned();
        match intent {
            Some(intent) => {
                println!("[client] RESUME {mid} from seq={start}");
                deliver(intent, shared, out, sid, mid, start).await;
            }
            None => {
                let _ = out
                    .send(Frame::new(
                        sid,
                        mid,
                        Body::WorkError {
                            code: "unrecoverable".into(),
                            message: "no such in-flight intent".into(),
                            provider_status: None,
                        },
                    ))
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
    cfg: Arc<ClientConfig>,
    shared: Arc<Shared>,
    mid: String,
) {
    tokio::spawn(async move {
        // Resolve the credential per provider, locally, at call time.
        let key = cfg.keys.resolve(&provider);
        let mut rx = match provider::call(&provider, &model, &request, &key).await {
            Ok(rx) => rx,
            Err(e) => {
                shared.emit(ClientEvent::WorkFailed {
                    mid: mid.clone(),
                    code: "provider_network".into(),
                    message: e.to_string(),
                });
                intent.buf.lock().await.terminal = Some(Terminal::Error {
                    code: "provider_network".into(),
                    message: e.to_string(),
                    provider_status: None,
                });
                return;
            }
        };

        loop {
            if intent.cancelled.load(Ordering::SeqCst) {
                println!("[client] {mid} cancelled — stopping read");
                shared.emit(ClientEvent::WorkFailed {
                    mid: mid.clone(),
                    code: "cancelled".into(),
                    message: "cancelled by node".into(),
                });
                intent.buf.lock().await.terminal = Some(Terminal::Error {
                    code: "cancelled".into(),
                    message: "cancelled by node".into(),
                    provider_status: None,
                });
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
                        println!("[client] DONE  {mid}: {n} chunks  usage in={} out={}  cost=${cost:.5}  window_spent=${spent:.5}", usage.input_tokens, usage.output_tokens);
                        shared.emit(ClientEvent::Done {
                            mid: mid.clone(),
                            provider: provider.clone(),
                            model: model.clone(),
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                            cost_usd: cost,
                            spent_usd: spent,
                        });
                        return;
                    }
                    None => {
                        shared.emit(ClientEvent::WorkFailed { mid: mid.clone(), code: "provider_network".into(), message: "stream ended".into() });
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
async fn deliver(
    intent: Arc<Intent>,
    shared: Arc<Shared>,
    out: mpsc::Sender<Frame>,
    sid: Option<String>,
    mid: String,
    start_seq: u64,
) {
    let mut cursor = start_seq;
    loop {
        enum Step {
            Send(u64, Value),
            Terminal(Box<Frame>),
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
                Step::Terminal(Box::new(terminal_frame(sid.clone(), mid.clone(), t)))
            } else {
                Step::Wait
            }
        };
        match step {
            Step::Send(seq, delta) => {
                if out
                    .send(Frame::new(
                        sid.clone(),
                        mid.clone(),
                        Body::WorkChunk { seq, delta },
                    ))
                    .await
                    .is_err()
                {
                    return; // suspend: connection dropped, keep the intent for resume
                }
                cursor += 1;
            }
            Step::Terminal(frame) => {
                let _ = out.send(*frame).await;
                shared.store.lock().await.remove(&mid);
                return;
            }
            Step::Unrecoverable => {
                let _ = out
                    .send(Frame::new(
                        sid,
                        mid.clone(),
                        Body::WorkError {
                            code: "unrecoverable".into(),
                            message: "resume past retained buffer".into(),
                            provider_status: None,
                        },
                    ))
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
        Terminal::Done { result, usage } => Frame::new(
            sid,
            mid,
            Body::WorkDone {
                result: result.clone(),
                usage: usage.clone(),
            },
        ),
        Terminal::Error {
            code,
            message,
            provider_status,
        } => Frame::new(
            sid,
            mid,
            Body::WorkError {
                code: code.clone(),
                message: message.clone(),
                provider_status: *provider_status,
            },
        ),
    }
}

/// Pin the Node's **root** key (TOFU), verify the connection's operational
/// key chains to it and signed the assigned `sid`. A reconnect under a different
/// root, an op key not delegated by the pinned root, an expired op key, or a bad
/// `sid` signature is refused — a stolen pairing token alone can't satisfy this
/// (§3/§9). Rotating operational keys across reconnects is fine.
async fn verify_chain_and_pin(
    pinned: &Arc<Mutex<Option<VerifyingKey>>>,
    sid: &str,
    root_pubkey_hex: &str,
    op: &keyward_proto::OpCert,
    sig_hex: &str,
) -> Result<()> {
    let root = identity::parse_pubkey(root_pubkey_hex)?;
    let root_fp = wire::fingerprint(&root.to_bytes());

    // Out-of-band confirmation (closes the TOFU first-contact gap, §3): if the
    // Owner pre-states the expected root fingerprint, refuse anything else.
    if let Ok(expected) = std::env::var("KEYWARD_EXPECT_ROOT_FP")
        && !expected.is_empty()
        && !expected.eq_ignore_ascii_case(&root_fp)
    {
        return Err(anyhow!(
            "node root fingerprint {root_fp} != expected {expected} — refusing to bind"
        ));
    }

    // Pin the root on first contact; refuse a changed root thereafter.
    {
        let mut guard = pinned.lock().await;
        match guard.as_ref() {
            Some(existing) if existing.to_bytes() != root.to_bytes() => {
                return Err(anyhow!(
                    "pinned root key changed — refusing (possible impersonation / stolen token)"
                ));
            }
            Some(_) => {}
            None => {
                println!("[client] TOFU: pinning node ROOT key  fp={root_fp}");
                *guard = Some(root);
            }
        }
    }

    let op_key = identity::verify_op_cert(&root, op, identity::now_unix())?;
    identity::verify_sid_sig(&op_key, sid, sig_hex)?;
    println!(
        "[client] verified op-key fp={} via pinned root fp={}",
        wire::fingerprint(&op_key.to_bytes()),
        wire::fingerprint(&root.to_bytes())
    );
    Ok(())
}

/// Load the Owner policy from `KEYWARD_POLICY` (a JSON file matching the §6 policy
/// object), or build a sensible default from `KEYWARD_PROVIDERS`.
fn load_policy() -> Result<Policy> {
    if let Ok(path) = std::env::var("KEYWARD_POLICY") {
        let text = std::fs::read_to_string(&path).map_err(|e| anyhow!("read policy file {path}: {e}"))?;
        let policy: Policy =
            serde_json::from_str(&text).map_err(|e| anyhow!("parse policy file {path}: {e}"))?;
        println!("[client] loaded policy from {path}");
        return Ok(policy);
    }
    let providers: Vec<String> = std::env::var("KEYWARD_PROVIDERS")
        .unwrap_or_else(|_| "mock,openai,openai-responses,anthropic".into())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(Policy {
        providers: Some(providers),
        budget: Some(keyward_proto::Budget {
            limit_usd: 5.0,
            window: "month".into(),
            spent_usd: 0.0,
        }),
        rate: Some(keyward_proto::Rate {
            rpm: Some(60),
            tpm: None,
        }),
        ..Default::default()
    })
}

/// Standalone client: dial a real Node using env config.
pub async fn run_cli() -> Result<()> {
    let url = std::env::var("KEYWARD_NODE_URL")
        .map_err(|_| anyhow!("set KEYWARD_NODE_URL, e.g. ws://127.0.0.1:8787"))?;
    let token = std::env::var("KEYWARD_PAIRING_TOKEN").map_err(|_| anyhow!("set KEYWARD_PAIRING_TOKEN"))?;

    // Owner policy: a JSON file at KEYWARD_POLICY, else a sensible built-in default
    // (allow the KEYWARD_PROVIDERS, any model, $5/month, 60 rpm).
    let policy = load_policy()?;
    let providers: Vec<String> = policy.providers.clone().unwrap_or_default();
    let identity = identity::load_or_create_identity();
    println!(
        "[client] identity fp={}  (give the node this pubkey to be allow-listed: {})",
        wire::fingerprint(&identity.verifying_key().to_bytes()),
        wire::hex(&identity.verifying_key().to_bytes())
    );
    let cfg = ClientConfig {
        name: "keyward-client".into(),
        providers,
        policy,
        keys: KeySource::Keychain,
        identity,
        pinned: Arc::new(Mutex::new(None)),
        events: None,
    };
    println!("[client] dialing {url} …  (keys resolved from the OS keychain, then env)");
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

    #[test]
    fn example_policy_file_parses() {
        let p: Policy = serde_json::from_str(include_str!("../../../docs/policy.example.json")).unwrap();
        assert_eq!(p.providers.as_ref().unwrap().len(), 3);
        assert!(p.budget.is_some() && p.rate.is_some());
        assert_eq!(p.models.as_ref().unwrap()[0], "gpt-4o*");
    }
}
