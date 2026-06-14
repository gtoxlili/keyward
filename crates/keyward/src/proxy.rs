//! Local OpenAI-compatible proxy (`keyward proxy`, feature `proxy`).
//!
//! The proxy is the Orchestrator with an HTTP front: an existing app points
//! `OPENAI_BASE_URL` at it, the proxy relays each request to the paired Executor
//! over the Keyward protocol, and streams the native result back. The app never
//! sees the key — it stays on the Executor. This is the zero-code-change
//! integration path.
//!
//! v0 scope: a single paired Executor; the HTTP path selects the dialect
//! (`/v1/chat/completions` → openai, `/v1/responses` → openai-responses,
//! `/v1/messages` → anthropic). Streaming responses are relayed verbatim (the
//! app's SDK parses native chunks); non-streaming is assembled into a
//! chat.completion. App-level auth is left to the deployment.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::extract::State;
use axum::http::StatusCode;
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

enum ProxyEvent {
    Chunk(Value),
    Done(Usage),
    Error(String),
}

/// Shared state: the outbound channel to the Executor, the session id, and a
/// map of in-flight request `mid` → response channel (demux of relayed frames).
struct ProxyState {
    out: mpsc::Sender<Frame>,
    sid: Mutex<Option<String>>,
    pending: Mutex<HashMap<String, mpsc::Sender<ProxyEvent>>>,
}

impl ProxyState {
    async fn submit(&self, provider: &str, request: Value) -> mpsc::Receiver<ProxyEvent> {
        let mid = new_mid();
        let (tx, rx) = mpsc::channel(64);
        self.pending.lock().await.insert(mid.clone(), tx);
        let sid = self.sid.lock().await.clone();
        let _ = self
            .out
            .send(Frame::new(
                sid,
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

    let cfg = OrchestratorConfig {
        name: "keyward-proxy".into(),
        id: "orch_proxy".into(),
        pairing_token: token.clone(),
        root: SigningKey::generate(&mut OsRng),
        authorized_executors: authorized,
        claimed_tokens: Default::default(),
        single_use_token: true,
        intents: Vec::new(),
    };

    // 1. Wait for the Executor to dial in and pair.
    let ws = TcpListener::bind(&ws_listen).await?;
    println!("[proxy] waiting for an executor on ws://{ws_listen}  (pairing_token={token})");
    let (stream, _) = ws.accept().await?;
    let (mut write, mut read) = accept_async(stream).await?.split();

    let hello = wire::recv(&mut read)
        .await?
        .ok_or_else(|| anyhow!("closed before hello"))?;
    authenticate_executor(&hello.body, &cfg)?;
    let (sid, paired) = build_paired(&cfg);
    wire::send(&mut write, &paired).await?;
    println!("[proxy] executor paired (sid={sid})");

    // 2. Writer task + demux loop on the executor connection.
    let (out, mut out_rx) = mpsc::channel::<Frame>(64);
    tokio::spawn(async move {
        while let Some(f) = out_rx.recv().await {
            if wire::send(&mut write, &f).await.is_err() {
                break;
            }
        }
    });
    let state = Arc::new(ProxyState {
        out,
        sid: Mutex::new(Some(sid)),
        pending: Mutex::new(HashMap::new()),
    });
    {
        let state = state.clone();
        tokio::spawn(async move {
            while let Ok(Some(frame)) = wire::recv(&mut read).await {
                route_frame(&state, frame).await;
            }
            // Executor gone: fail every in-flight request.
            for (_, tx) in state.pending.lock().await.drain() {
                let _ = tx.send(ProxyEvent::Error("executor disconnected".into())).await;
            }
        });
    }

    // 3. HTTP front.
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/responses", post(responses))
        .route("/v1/messages", post(messages))
        .with_state(state);
    let http = TcpListener::bind(&http_listen).await?;
    println!("[proxy] OpenAI-compatible endpoint on http://{http_listen}");
    println!(
        "[proxy] point your app at it:  OPENAI_BASE_URL=http://{http_listen}/v1  OPENAI_API_KEY=anything"
    );
    axum::serve(http, app).await?;
    Ok(())
}

/// Route a relayed frame to the request waiting on its `mid`.
async fn route_frame(state: &ProxyState, frame: Frame) {
    let ev = match frame.body {
        Body::WorkChunk { delta, .. } => ProxyEvent::Chunk(delta),
        Body::WorkDone { usage, .. } => ProxyEvent::Done(usage),
        Body::WorkError { code, message, .. } => ProxyEvent::Error(format!("{code}: {message}")),
        _ => return,
    };
    let terminal = matches!(ev, ProxyEvent::Done(_) | ProxyEvent::Error(_));
    let tx = state.pending.lock().await.get(&frame.mid).cloned();
    if let Some(tx) = tx {
        let _ = tx.send(ev).await;
        if terminal {
            state.pending.lock().await.remove(&frame.mid);
        }
    }
}

// --- HTTP handlers: the path picks the dialect ----------------------------

async fn chat_completions(State(state): State<Arc<ProxyState>>, Json(body): Json<Value>) -> Response {
    relay(state, "openai", body).await
}
async fn responses(State(state): State<Arc<ProxyState>>, Json(body): Json<Value>) -> Response {
    relay(state, "openai-responses", body).await
}
async fn messages(State(state): State<Arc<ProxyState>>, Json(body): Json<Value>) -> Response {
    relay(state, "anthropic", body).await
}

async fn relay(state: Arc<ProxyState>, provider: &str, body: Value) -> Response {
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let rx = state.submit(provider, body).await;

    if streaming {
        // Relay native chunks verbatim as SSE; terminate with `data: [DONE]`.
        let stream = futures_util::stream::unfold((rx, false), |(mut rx, finished)| async move {
            if finished {
                return None;
            }
            match rx.recv().await {
                Some(ProxyEvent::Chunk(v)) => Some((
                    Ok::<_, std::convert::Infallible>(Event::default().data(v.to_string())),
                    (rx, false),
                )),
                Some(ProxyEvent::Done(_)) => Some((Ok(Event::default().data("[DONE]")), (rx, true))),
                Some(ProxyEvent::Error(e)) => Some((Ok(Event::default().event("error").data(e)), (rx, true))),
                None => None,
            }
        });
        Sse::new(stream).into_response()
    } else {
        // Collect chunks and assemble a chat.completion.
        let mut rx = rx;
        let mut content = String::new();
        let mut usage = Usage::default();
        while let Some(ev) = rx.recv().await {
            match ev {
                ProxyEvent::Chunk(v) => content.push_str(chunk_text(&v)),
                ProxyEvent::Done(u) => {
                    usage = u;
                    break;
                }
                ProxyEvent::Error(e) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({ "error": { "message": e } })),
                    )
                        .into_response();
                }
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
