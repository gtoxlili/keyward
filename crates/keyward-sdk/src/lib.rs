//! # Keyward Orchestrator SDK
//!
//! Integrate your app as the **Orchestrator** — the brain that decides what to do
//! but never holds the key. You bind a listener the Owner's **Executor** dials into,
//! pair, then submit work intents and stream the provider's native response back.
//! The key stays on the Executor.
//!
//! ```no_run
//! # async fn run() -> anyhow::Result<()> {
//! use keyward_sdk::{serve_one, Config, Event};
//! use tokio::net::TcpListener;
//!
//! let cfg = Config::new("my-app", "orch_myapp", "pt_one_time_token");
//! println!("root fingerprint: {}", cfg.root_fingerprint());
//! let listener = TcpListener::bind("127.0.0.1:8787").await?;
//! let session = serve_one(&listener, &cfg).await?; // waits for an executor to pair
//!
//! let mut rx = session
//!     .submit("openai", serde_json::json!({
//!         "model": "gpt-4o",
//!         "messages": [{"role": "user", "content": "hi"}],
//!         "stream": true
//!     }))
//!     .await;
//! while let Some(ev) = rx.recv().await {
//!     match ev {
//!         Event::Chunk(c) => { /* relay native chunk to your user */ }
//!         Event::Done(usage) => { println!("in={} out={}", usage.input_tokens, usage.output_tokens); break; }
//!         Event::Error(e) => { eprintln!("{e}"); break; }
//!     }
//! }
//! # Ok(()) }
//! ```

mod identity;
mod wire;

pub use ed25519_dalek::SigningKey;
pub use keyward_proto::Usage;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use futures_util::StreamExt;
use keyward_proto::{Body, Frame};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tokio_tungstenite::accept_async;

use identity::{authenticate_executor, build_paired, new_mid};

/// Orchestrator configuration.
pub struct Config {
    /// Human-readable app name (sent to the Executor).
    pub name: String,
    /// Stable orchestrator id (the Executor's policy may scope to it).
    pub id: String,
    /// One-time pairing token the Owner pastes into their Executor.
    pub pairing_token: String,
    /// Long-term root identity. The Executor pins this on first contact.
    pub root: SigningKey,
    /// Optional allow-list of authorized Executor identity pubkeys (hex). `None`
    /// accepts any Executor that proves possession of its key.
    pub authorized_executors: Option<Vec<String>>,
}

impl Config {
    /// A config with a freshly generated root identity.
    pub fn new(name: impl Into<String>, id: impl Into<String>, pairing_token: impl Into<String>) -> Self {
        Config {
            name: name.into(),
            id: id.into(),
            pairing_token: pairing_token.into(),
            root: SigningKey::generate(&mut rand_core::OsRng),
            authorized_executors: None,
        }
    }

    /// This orchestrator's root fingerprint — show it to the Owner so they can
    /// confirm it out of band when pairing.
    pub fn root_fingerprint(&self) -> String {
        identity::root_fingerprint(self)
    }
}

/// One streamed step of a provider response, in the provider's native shape.
pub enum Event {
    /// A native streaming chunk (relay it to your user unchanged).
    Chunk(Value),
    /// Terminal success, with metered usage.
    Done(Usage),
    /// Terminal failure (`code: message`).
    Error(String),
}

/// A paired session with one Executor. Submit work and stream native results.
pub struct Session {
    out: mpsc::Sender<Frame>,
    sid: String,
    pending: Arc<Mutex<HashMap<String, mpsc::Sender<Event>>>>,
}

/// Accept ONE Executor dialing into `listener` over **WebSocket**, authenticate + pair
/// it, and return a `Session`. (v0: one Executor per session.)
pub async fn serve_one(listener: &TcpListener, cfg: &Config) -> Result<Session> {
    let (stream, _) = listener.accept().await?;
    let (mut write, mut read) = accept_async(stream).await?.split();

    let (out, mut out_rx) = mpsc::channel::<Frame>(64);
    tokio::spawn(async move {
        while let Some(f) = out_rx.recv().await {
            if wire::send(&mut write, &f).await.is_err() {
                break;
            }
        }
    });
    let (in_tx, in_rx) = mpsc::channel::<Frame>(64);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                // Consumer dropped the receiver (e.g. auth rejected): stop and drop
                // `read` so the socket closes and the Executor sees the rejection.
                _ = in_tx.closed() => break,
                msg = wire::recv(&mut read) => match msg {
                    Ok(Some(frame)) => {
                        if in_tx.send(frame).await.is_err() {
                            break;
                        }
                    }
                    _ => break,
                },
            }
        }
    });

    pair_and_run(out, in_rx, cfg).await
}

/// Accept ONE Executor over **gRPC** at `addr`, authenticate + pair it, and return a
/// `Session`. Same protocol and `Session` API — only the transport differs (spec §1).
/// Requires the `grpc` feature.
#[cfg(feature = "grpc")]
pub async fn serve_one_grpc(addr: std::net::SocketAddr, cfg: &Config) -> Result<Session> {
    let (out, inbound) = keyward_grpc::accept_one(addr).await?;
    pair_and_run(out, inbound, cfg).await
}

/// Shared pairing + session loop over an already-established `(out, inbound)` frame
/// channel pair — transport-agnostic. Receives `hello`, authenticates the Executor,
/// signs and sends `paired`, then demultiplexes inbound work frames to per-intent
/// receivers until the channel closes.
async fn pair_and_run(
    out: mpsc::Sender<Frame>,
    mut inbound: mpsc::Receiver<Frame>,
    cfg: &Config,
) -> Result<Session> {
    let hello = inbound
        .recv()
        .await
        .ok_or_else(|| anyhow!("closed before hello"))?;
    authenticate_executor(&hello.body, cfg)?;
    let (sid, paired) = build_paired(cfg);
    out.send(paired)
        .await
        .map_err(|_| anyhow!("failed to send paired (executor went away)"))?;

    let pending: Arc<Mutex<HashMap<String, mpsc::Sender<Event>>>> = Arc::new(Mutex::new(HashMap::new()));
    {
        let pending = pending.clone();
        tokio::spawn(async move {
            while let Some(frame) = inbound.recv().await {
                route(&pending, frame).await;
            }
            for (_, tx) in pending.lock().await.drain() {
                let _ = tx.send(Event::Error("executor disconnected".into())).await;
            }
        });
    }

    Ok(Session { out, sid, pending })
}

impl Session {
    /// Send a work intent — a provider name and the provider-native request body,
    /// minus any credential — and stream the native response back.
    pub async fn submit(&self, provider: &str, request: Value) -> mpsc::Receiver<Event> {
        let mid = new_mid();
        let (tx, rx) = mpsc::channel(64);
        self.pending.lock().await.insert(mid.clone(), tx);
        let _ = self
            .out
            .send(Frame::new(
                Some(self.sid.clone()),
                mid,
                Body::Work {
                    provider: provider.to_string(),
                    request,
                },
            ))
            .await;
        rx
    }
}

async fn route(pending: &Arc<Mutex<HashMap<String, mpsc::Sender<Event>>>>, frame: Frame) {
    let ev = match frame.body {
        Body::WorkChunk { delta, .. } => Event::Chunk(delta),
        Body::WorkDone { usage, .. } => Event::Done(usage),
        Body::WorkError { code, message, .. } => Event::Error(format!("{code}: {message}")),
        _ => return,
    };
    let terminal = matches!(ev, Event::Done(_) | Event::Error(_));
    let tx = pending.lock().await.get(&frame.mid).cloned();
    if let Some(tx) = tx {
        let _ = tx.send(ev).await;
        if terminal {
            pending.lock().await.remove(&frame.mid);
        }
    }
}
