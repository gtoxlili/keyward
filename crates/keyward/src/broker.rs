//! Multi-tenant broker (`keyward broker`, feature `broker`) — the "public station"
//! (§10).
//!
//! A shared/neutral Orchestrator: many Executors dial in, each registering a **routing
//! token**; an OpenAI-compatible HTTP front routes each request to the Executor whose
//! token matches the request's bearer credential, and streams the native result back.
//!
//! The "API key" the app sends is a Keyward **routing token, not a provider key** — the
//! provider key stays on the Executor, the broker never sees it, and the Executor's
//! policy still gates every call. So a leaked routing token can do no more than the
//! policy allows. The app is unaware: to it this is just an OpenAI endpoint + an API key.
//!
//! Routing token per Executor = its advertised `route_token` (from `KEYWARD_ROUTE_TOKEN`),
//! else derived from its identity pubkey. Response demux is by `mid`, exactly as the
//! single-tenant proxy.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use ed25519_dalek::SigningKey;
use futures_util::StreamExt;
use keyward_proto::{Body, Frame, Usage};
use rand_core::OsRng;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tokio_tungstenite::accept_async;

use crate::executor::new_mid;
use crate::orchestrator::{OrchestratorConfig, authenticate_executor, build_paired, chunk_text};
use crate::wire;

enum BrokerEvent {
    Chunk(Value),
    Done(Usage),
    Error(String),
    /// Opaque ciphertext relayed verbatim for the sealed path (§9) — never decrypted.
    Sealed(String),
}

/// One paired Executor connection, addressed by its routing token.
struct ExecutorConn {
    out: mpsc::Sender<Frame>,
    sid: Option<String>,
    pending: Mutex<HashMap<String, mpsc::Sender<BrokerEvent>>>,
}

impl ExecutorConn {
    async fn submit(&self, provider: &str, request: Value) -> mpsc::Receiver<BrokerEvent> {
        let mid = new_mid();
        let (tx, rx) = mpsc::channel(64);
        self.pending.lock().await.insert(mid.clone(), tx);
        let _ = self
            .out
            .send(Frame::new(
                self.sid.clone(),
                mid,
                Body::Work {
                    provider: provider.to_string(),
                    request,
                },
            ))
            .await;
        rx
    }

    /// Submit an opaque sealed work blob (§9) — the broker never inspects it.
    async fn submit_sealed(&self, blob: String) -> mpsc::Receiver<BrokerEvent> {
        let mid = new_mid();
        let (tx, rx) = mpsc::channel(64);
        self.pending.lock().await.insert(mid.clone(), tx);
        let _ = self
            .out
            .send(Frame::new(self.sid.clone(), mid, Body::Sealed { blob }))
            .await;
        rx
    }
}

/// Routing registry: token → the Executor connection that holds it.
struct Broker {
    routes: Mutex<HashMap<String, Arc<ExecutorConn>>>,
}

pub async fn run_cli() -> Result<()> {
    let ws_listen = std::env::var("KEYWARD_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let http_listen = std::env::var("KEYWARD_PROXY_LISTEN").unwrap_or_else(|_| "127.0.0.1:8088".into());
    let token = std::env::var("KEYWARD_PAIRING_TOKEN").unwrap_or_else(|_| "pt_dev_token".into());
    let authorized = std::env::var("KEYWARD_AUTHORIZED_EXECUTORS").ok().map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
    });

    let cfg = Arc::new(OrchestratorConfig {
        name: "keyward-broker".into(),
        id: "orch_broker".into(),
        pairing_token: token.clone(),
        root: SigningKey::generate(&mut OsRng),
        authorized_executors: authorized,
        claimed_tokens: Default::default(),
        single_use_token: false, // multi-tenant: one join-token admits many executors
        intents: Vec::new(),
    });
    let broker = Arc::new(Broker {
        routes: Mutex::new(HashMap::new()),
    });

    // Accept many executors, each on its own task.
    let ws = TcpListener::bind(&ws_listen).await?;
    println!("[broker] executors dial in on ws://{ws_listen}  (pairing_token={token})");
    {
        let broker = broker.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = ws.accept().await else {
                    continue;
                };
                let (broker, cfg) = (broker.clone(), cfg.clone());
                tokio::spawn(async move {
                    if let Err(e) = handle_executor(stream, broker, cfg).await {
                        eprintln!("[broker] executor session ended: {e}");
                    }
                });
            }
        });
    }

    // OpenAI-compatible HTTP front; routes by the request's bearer token.
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/responses", post(responses))
        .route("/v1/messages", post(messages))
        // Sealed relay (§9): the requester-side shim posts ciphertext, the broker forwards
        // it blind and streams back the sealed reply.
        .route("/kw/sealed", post(sealed_relay))
        .with_state(broker);
    let http = TcpListener::bind(&http_listen).await?;
    println!("[broker] OpenAI-compatible endpoint on http://{http_listen}");
    println!(
        "[broker] each user points their app at it with their routing token as the API key:\n         \
         OPENAI_BASE_URL=http://{http_listen}/v1  OPENAI_API_KEY=<their route token>"
    );
    axum::serve(http, app).await?;
    Ok(())
}

/// Pair one Executor, register its routing token, and demux its frames until it drops.
async fn handle_executor(
    stream: tokio::net::TcpStream,
    broker: Arc<Broker>,
    cfg: Arc<OrchestratorConfig>,
) -> Result<()> {
    let (mut write, mut read) = accept_async(stream).await?.split();
    let hello = wire::recv(&mut read)
        .await?
        .ok_or_else(|| anyhow!("closed before hello"))?;
    authenticate_executor(&hello.body, &cfg)?;

    let (route_token, peer) = match &hello.body {
        Body::Hello {
            route_token,
            pubkey,
            executor,
            ..
        } => {
            let tok = route_token
                .clone()
                .or_else(|| pubkey.as_ref().map(|pk| derive_token(pk)))
                .ok_or_else(|| anyhow!("executor sent no route_token and no pubkey to derive from"))?;
            (tok, executor.name.clone())
        }
        _ => return Err(anyhow!("expected hello")),
    };

    let (sid, paired) = build_paired(&cfg);
    wire::send(&mut write, &paired).await?;

    let (out, mut out_rx) = mpsc::channel::<Frame>(64);
    tokio::spawn(async move {
        while let Some(f) = out_rx.recv().await {
            if wire::send(&mut write, &f).await.is_err() {
                break;
            }
        }
    });
    let conn = Arc::new(ExecutorConn {
        out,
        sid: Some(sid),
        pending: Mutex::new(HashMap::new()),
    });

    let replaced = broker
        .routes
        .lock()
        .await
        .insert(route_token.clone(), conn.clone())
        .is_some();
    let n = broker.routes.lock().await.len();
    println!(
        "[broker] '{peer}' paired → routing token '{route_token}'{}  ({n} executor(s) online)",
        if replaced {
            " (replaced a prior holder)"
        } else {
            ""
        }
    );

    while let Ok(Some(frame)) = wire::recv(&mut read).await {
        route_frame(&conn, frame).await;
    }

    // Gone: deregister (only if the slot is still ours) and fail in-flight requests.
    {
        let mut routes = broker.routes.lock().await;
        if routes.get(&route_token).is_some_and(|c| Arc::ptr_eq(c, &conn)) {
            routes.remove(&route_token);
        }
    }
    for (_, tx) in conn.pending.lock().await.drain() {
        let _ = tx.send(BrokerEvent::Error("executor disconnected".into())).await;
    }
    println!("[broker] '{peer}' (token '{route_token}') disconnected");
    Ok(())
}

/// Stable per-executor token derived from its identity pubkey — the fallback when the
/// Executor advertises no `route_token`. Note: derivable by anyone who knows the pubkey,
/// so it's a weak capability; set `KEYWARD_ROUTE_TOKEN` for a real secret.
fn derive_token(pubkey_hex: &str) -> String {
    format!("kw-{}", &pubkey_hex[..pubkey_hex.len().min(16)])
}

async fn route_frame(conn: &ExecutorConn, frame: Frame) {
    let ev = match frame.body {
        Body::WorkChunk { delta, .. } => BrokerEvent::Chunk(delta),
        Body::WorkDone { usage, .. } => BrokerEvent::Done(usage),
        Body::WorkError { code, message, .. } => BrokerEvent::Error(format!("{code}: {message}")),
        Body::Sealed { blob } => BrokerEvent::Sealed(blob),
        _ => return,
    };
    let terminal = matches!(ev, BrokerEvent::Done(_) | BrokerEvent::Error(_));
    let tx = conn.pending.lock().await.get(&frame.mid).cloned();
    if let Some(tx) = tx {
        let _ = tx.send(ev).await;
        if terminal {
            conn.pending.lock().await.remove(&frame.mid);
        }
    }
}

// --- HTTP handlers: path picks the dialect, bearer token picks the executor --------

async fn chat_completions(
    State(b): State<Arc<Broker>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    relay(b, &headers, "openai", body).await
}
async fn responses(State(b): State<Arc<Broker>>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    relay(b, &headers, "openai-responses", body).await
}
async fn messages(State(b): State<Arc<Broker>>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    relay(b, &headers, "anthropic", body).await
}

/// The request's routing token: `Authorization: Bearer <t>` (OpenAI clients) or
/// `x-api-key: <t>` (Anthropic clients).
fn route_token_of(headers: &HeaderMap) -> Option<String> {
    if let Some(v) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        return Some(v.strip_prefix("Bearer ").unwrap_or(v).trim().to_string());
    }
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
}

/// The Executor connection for the request's routing token, or a 401.
fn no_route() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": {
            "message": "no executor for this routing token — pair one, and use its route token as the API key",
            "type": "keyward_unknown_route"
        } })),
    )
        .into_response()
}

async fn conn_for(b: &Broker, headers: &HeaderMap) -> Option<Arc<ExecutorConn>> {
    let token = route_token_of(headers).filter(|t| !t.is_empty())?;
    b.routes.lock().await.get(&token).cloned()
}

/// Sealed relay (§9): the shim posts an opaque ciphertext blob with its routing token;
/// the broker forwards it to the matching Executor and streams the sealed reply back —
/// never decrypting. Content stays end-to-end encrypted; only metadata terminals
/// (done/error) are cleartext.
async fn sealed_relay(State(b): State<Arc<Broker>>, headers: HeaderMap, body: String) -> Response {
    let Some(conn) = conn_for(&b, &headers).await else {
        return no_route();
    };
    println!(
        "[broker] sealed relay: {} hex chars of ciphertext, forwarded blind (prefix {}…)",
        body.len(),
        &body[..body.len().min(32)]
    );
    let rx = conn.submit_sealed(body).await;
    let stream = futures_util::stream::unfold((rx, false), |(mut rx, finished)| async move {
        if finished {
            return None;
        }
        match rx.recv().await {
            Some(BrokerEvent::Sealed(blob)) => Some((
                Ok::<_, std::convert::Infallible>(Event::default().data(blob)),
                (rx, false),
            )),
            Some(BrokerEvent::Done(_)) => Some((Ok(Event::default().data("[DONE]")), (rx, true))),
            Some(BrokerEvent::Error(e)) => Some((Ok(Event::default().event("error").data(e)), (rx, true))),
            Some(BrokerEvent::Chunk(_)) | None => None,
        }
    });
    Sse::new(stream).into_response()
}

async fn relay(b: Arc<Broker>, headers: &HeaderMap, provider: &str, body: Value) -> Response {
    let Some(conn) = conn_for(&b, headers).await else {
        return no_route();
    };

    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let rx = conn.submit(provider, body).await;

    if streaming {
        let stream = futures_util::stream::unfold((rx, false), |(mut rx, finished)| async move {
            if finished {
                return None;
            }
            match rx.recv().await {
                Some(BrokerEvent::Chunk(v)) => Some((
                    Ok::<_, std::convert::Infallible>(Event::default().data(v.to_string())),
                    (rx, false),
                )),
                Some(BrokerEvent::Done(_)) => Some((Ok(Event::default().data("[DONE]")), (rx, true))),
                Some(BrokerEvent::Error(e)) => {
                    Some((Ok(Event::default().event("error").data(e)), (rx, true)))
                }
                Some(BrokerEvent::Sealed(_)) | None => None,
            }
        });
        Sse::new(stream).into_response()
    } else {
        let mut rx = rx;
        let mut content = String::new();
        let mut usage = Usage::default();
        while let Some(ev) = rx.recv().await {
            match ev {
                BrokerEvent::Chunk(v) => content.push_str(chunk_text(&v)),
                BrokerEvent::Done(u) => {
                    usage = u;
                    break;
                }
                BrokerEvent::Error(e) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({ "error": { "message": e } })),
                    )
                        .into_response();
                }
                BrokerEvent::Sealed(_) => {}
            }
        }
        Json(json!({
            "id": "chatcmpl-keyward",
            "object": "chat.completion",
            "model": model,
            "choices": [{ "index": 0, "message": { "role": "assistant", "content": content }, "finish_reason": "stop" }],
            "usage": {
                "prompt_tokens": usage.input_tokens,
                "completion_tokens": usage.output_tokens,
                "total_tokens": usage.input_tokens + usage.output_tokens
            }
        }))
        .into_response()
    }
}
