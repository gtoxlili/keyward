//! Multi-tenant node (`keyward node`, feature `node`) — the "public station"
//! (§10).
//!
//! A shared/neutral Node: many Clients dial in, each registering a **routing
//! token**; an OpenAI-compatible HTTP front routes each request to the Client whose
//! token matches the request's bearer credential, and streams the native result back.
//!
//! The "API key" the app sends is a Keyward **routing token, not a provider key** — the
//! provider key stays on the Client, the node never sees it, and the Client's
//! policy still gates every call. So a leaked routing token can do no more than the
//! policy allows. The app is unaware: to it this is just an OpenAI endpoint + an API key.
//!
//! Routing token per Client = its advertised `route_token` (from `KEYWARD_ROUTE_TOKEN`),
//! else derived from its identity pubkey. Response demux is by `mid`, exactly as a
//! single-tenant node.

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

use crate::client::new_mid;
use crate::session::{NodeConfig, authenticate_client, build_paired, chunk_text};
use crate::wire;

enum NodeEvent {
    Chunk(Value),
    Done(Usage),
    Error(String),
    /// Opaque ciphertext relayed verbatim for the sealed path (§9) — never decrypted.
    Sealed(String),
}

/// One paired Client connection, addressed by its routing token.
struct ClientConn {
    out: mpsc::Sender<Frame>,
    sid: Option<String>,
    pending: Mutex<HashMap<String, mpsc::Sender<NodeEvent>>>,
}

impl ClientConn {
    async fn submit(&self, provider: &str, request: Value) -> mpsc::Receiver<NodeEvent> {
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

    /// Submit an opaque sealed work blob (§9) — the node never inspects it.
    async fn submit_sealed(&self, blob: String) -> mpsc::Receiver<NodeEvent> {
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

/// Routing registry: token → the Client connection that holds it.
struct Node {
    routes: Mutex<HashMap<String, Arc<ClientConn>>>,
    /// Single-tenant mode: a request with no/unknown routing token is routed to the sole
    /// connected Client. OFF by default — a multi-tenant (public-station) node MUST require
    /// a valid token, because the set of connected Clients is transient: routing a tokenless
    /// request to "the only one currently connected" would hand a requester's prompt to an
    /// arbitrary Owner and spend *their* key. Only an operator running their own personal
    /// node (one Client) opts in, via `KEYWARD_SINGLE_TENANT=1`.
    single_tenant: bool,
}

pub async fn run_cli() -> Result<()> {
    let ws_listen = std::env::var("KEYWARD_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let http_listen = std::env::var("KEYWARD_HTTP_LISTEN").unwrap_or_else(|_| "127.0.0.1:8088".into());
    let token = std::env::var("KEYWARD_PAIRING_TOKEN").unwrap_or_else(|_| "pt_dev_token".into());
    let authorized = std::env::var("KEYWARD_AUTHORIZED_CLIENTS").ok().map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
    });

    let cfg = Arc::new(NodeConfig {
        name: "keyward-node".into(),
        id: "node_node".into(),
        pairing_token: token.clone(),
        root: SigningKey::generate(&mut OsRng),
        authorized_clients: authorized,
        claimed_tokens: Default::default(),
        single_use_token: false, // multi-tenant: one join-token admits many clients
        intents: Vec::new(),
    });
    // Multi-tenant by default; opt into the personal (single-Client, tokenless) shortcut.
    let single_tenant =
        std::env::var("KEYWARD_SINGLE_TENANT").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let node = Arc::new(Node {
        routes: Mutex::new(HashMap::new()),
        single_tenant,
    });

    // Accept many clients, each on its own task.
    let ws = TcpListener::bind(&ws_listen).await?;
    println!("[node] clients dial in on ws://{ws_listen}  (pairing_token={token})");
    {
        let node = node.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = ws.accept().await else {
                    continue;
                };
                let (node, cfg) = (node.clone(), cfg.clone());
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, node, cfg).await {
                        eprintln!("[node] client session ended: {e}");
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
        // Sealed relay (§9): the requester-side shim posts ciphertext, the node forwards
        // it blind and streams back the sealed reply.
        .route("/kw/sealed", post(sealed_relay))
        .with_state(node);
    let http = TcpListener::bind(&http_listen).await?;
    println!("[node] OpenAI-compatible endpoint on http://{http_listen}");
    if single_tenant {
        println!(
            "[node] SINGLE-TENANT mode: one Client, no routing token needed (personal node).\n         \
             OPENAI_BASE_URL=http://{http_listen}/v1  OPENAI_API_KEY=anything"
        );
    } else {
        println!(
            "[node] multi-tenant: every request needs a valid routing token (set KEYWARD_SINGLE_TENANT=1\n         \
             for a personal one-Client node). Each user points their app at it with their token as the key:\n         \
             OPENAI_BASE_URL=http://{http_listen}/v1  OPENAI_API_KEY=<their route token>"
        );
    }
    axum::serve(http, app).await?;
    Ok(())
}

/// Pair one Client, register its routing token, and demux its frames until it drops.
async fn handle_client(stream: tokio::net::TcpStream, node: Arc<Node>, cfg: Arc<NodeConfig>) -> Result<()> {
    let (mut write, mut read) = accept_async(stream).await?.split();
    let hello = wire::recv(&mut read)
        .await?
        .ok_or_else(|| anyhow!("closed before hello"))?;
    authenticate_client(&hello.body, &cfg)?;

    let (route_token, peer) = match &hello.body {
        Body::Hello {
            route_token,
            pubkey,
            client,
            ..
        } => {
            let tok = route_token
                .clone()
                .or_else(|| pubkey.as_ref().map(|pk| derive_token(pk)))
                .ok_or_else(|| anyhow!("client sent no route_token and no pubkey to derive from"))?;
            (tok, client.name.clone())
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
    let conn = Arc::new(ClientConn {
        out,
        sid: Some(sid),
        pending: Mutex::new(HashMap::new()),
    });

    let replaced = node
        .routes
        .lock()
        .await
        .insert(route_token.clone(), conn.clone())
        .is_some();
    let n = node.routes.lock().await.len();
    println!(
        "[node] '{peer}' paired → routing token '{route_token}'{}  ({n} client(s) online)",
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
        let mut routes = node.routes.lock().await;
        if routes.get(&route_token).is_some_and(|c| Arc::ptr_eq(c, &conn)) {
            routes.remove(&route_token);
        }
    }
    for (_, tx) in conn.pending.lock().await.drain() {
        let _ = tx.send(NodeEvent::Error("client disconnected".into())).await;
    }
    println!("[node] '{peer}' (token '{route_token}') disconnected");
    Ok(())
}

/// Stable per-client token derived from its identity pubkey — the fallback when the
/// Client advertises no `route_token`. Note: derivable by anyone who knows the pubkey,
/// so it's a weak capability; set `KEYWARD_ROUTE_TOKEN` for a real secret.
fn derive_token(pubkey_hex: &str) -> String {
    format!("kw-{}", &pubkey_hex[..pubkey_hex.len().min(16)])
}

async fn route_frame(conn: &ClientConn, frame: Frame) {
    let ev = match frame.body {
        Body::WorkChunk { delta, .. } => NodeEvent::Chunk(delta),
        Body::WorkDone { usage, .. } => NodeEvent::Done(usage),
        Body::WorkError { code, message, .. } => NodeEvent::Error(format!("{code}: {message}")),
        Body::Sealed { blob } => NodeEvent::Sealed(blob),
        _ => return,
    };
    let terminal = matches!(ev, NodeEvent::Done(_) | NodeEvent::Error(_));
    let tx = conn.pending.lock().await.get(&frame.mid).cloned();
    if let Some(tx) = tx {
        let _ = tx.send(ev).await;
        if terminal {
            conn.pending.lock().await.remove(&frame.mid);
        }
    }
}

// --- HTTP handlers: path picks the dialect, bearer token picks the client --------

async fn chat_completions(
    State(b): State<Arc<Node>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    relay(b, &headers, "openai", body).await
}
async fn responses(State(b): State<Arc<Node>>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    relay(b, &headers, "openai-responses", body).await
}
async fn messages(State(b): State<Arc<Node>>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
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

/// The Client connection for the request's routing token, or a 401.
fn no_route() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": {
            "message": "no client for this routing token — pair one, and use its route token as the API key",
            "type": "keyward_unknown_route"
        } })),
    )
        .into_response()
}

async fn conn_for(b: &Node, headers: &HeaderMap) -> Option<Arc<ClientConn>> {
    let routes = b.routes.lock().await;
    // Token routing — the only path on a multi-tenant node. A request must name a connected
    // Client by its routing token; an unknown/absent token is rejected (returns None).
    if let Some(token) = route_token_of(headers).filter(|t| !t.is_empty())
        && let Some(c) = routes.get(&token)
    {
        return Some(c.clone());
    }
    // Single-tenant convenience ONLY: with the operator's explicit opt-in AND exactly one
    // Client connected, a tokenless request goes to it (the personal-node case, ≈ the old
    // proxy). Never falls back on client count alone — see `Node::single_tenant`.
    if b.single_tenant && routes.len() == 1 {
        return routes.values().next().cloned();
    }
    None
}

/// Sealed relay (§9): the shim posts an opaque ciphertext blob with its routing token;
/// the node forwards it to the matching Client and streams the sealed reply back —
/// never decrypting. Content stays end-to-end encrypted; only metadata terminals
/// (done/error) are cleartext.
async fn sealed_relay(State(b): State<Arc<Node>>, headers: HeaderMap, body: String) -> Response {
    let Some(conn) = conn_for(&b, &headers).await else {
        return no_route();
    };
    println!(
        "[node] sealed relay: {} hex chars of ciphertext, forwarded blind (prefix {}…)",
        body.len(),
        &body[..body.len().min(32)]
    );
    let rx = conn.submit_sealed(body).await;
    let stream = futures_util::stream::unfold((rx, false), |(mut rx, finished)| async move {
        if finished {
            return None;
        }
        match rx.recv().await {
            Some(NodeEvent::Sealed(blob)) => Some((
                Ok::<_, std::convert::Infallible>(Event::default().data(blob)),
                (rx, false),
            )),
            Some(NodeEvent::Done(_)) => Some((Ok(Event::default().data("[DONE]")), (rx, true))),
            Some(NodeEvent::Error(e)) => Some((Ok(Event::default().event("error").data(e)), (rx, true))),
            Some(NodeEvent::Chunk(_)) | None => None,
        }
    });
    Sse::new(stream).into_response()
}

async fn relay(b: Arc<Node>, headers: &HeaderMap, provider: &str, body: Value) -> Response {
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
                Some(NodeEvent::Chunk(v)) => Some((
                    Ok::<_, std::convert::Infallible>(Event::default().data(v.to_string())),
                    (rx, false),
                )),
                Some(NodeEvent::Done(_)) => Some((Ok(Event::default().data("[DONE]")), (rx, true))),
                Some(NodeEvent::Error(e)) => Some((Ok(Event::default().event("error").data(e)), (rx, true))),
                Some(NodeEvent::Sealed(_)) | None => None,
            }
        });
        Sse::new(stream).into_response()
    } else {
        let mut rx = rx;
        let mut content = String::new();
        let mut usage = Usage::default();
        while let Some(ev) = rx.recv().await {
            match ev {
                NodeEvent::Chunk(v) => content.push_str(chunk_text(&v)),
                NodeEvent::Done(u) => {
                    usage = u;
                    break;
                }
                NodeEvent::Error(e) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({ "error": { "message": e } })),
                    )
                        .into_response();
                }
                NodeEvent::Sealed(_) => {}
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
