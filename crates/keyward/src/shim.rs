//! Requester-side shim (`keyward shim`, feature `shim`) — the trustless-node client (§9).
//!
//! An unaware OpenAI app points `OPENAI_BASE_URL` at this localhost endpoint. The shim
//! seals each request to the Client's **identity** key, posts the ciphertext to the
//! node (which routes it blind by the routing token), and decrypts the sealed reply —
//! so the node never sees the prompt or completion. The awareness lives here, in a
//! local shim, not in the app: app-unaware AND node-blind, at once.

use std::convert::Infallible;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use ed25519_dalek::VerifyingKey;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::seal::{self, Sealed};
use crate::session::chunk_text;
use crate::wire;

struct ShimState {
    client: reqwest::Client,
    sealed_url: String,
    token: String,
    client_pub: crypto_box::PublicKey,
}

pub async fn run_cli() -> Result<()> {
    let listen = std::env::var("KEYWARD_SHIM_LISTEN").unwrap_or_else(|_| "127.0.0.1:8099".into());
    let node = std::env::var("KEYWARD_NODE_URL").unwrap_or_else(|_| "http://127.0.0.1:8088".into());
    let token = std::env::var("KEYWARD_ROUTE_TOKEN")
        .map_err(|_| anyhow!("set KEYWARD_ROUTE_TOKEN (your client's routing token)"))?;
    let client_pubkey_hex = std::env::var("KEYWARD_CLIENT_PUBKEY")
        .map_err(|_| anyhow!("set KEYWARD_CLIENT_PUBKEY (the client's identity pubkey, hex)"))?;

    let vk_bytes: [u8; 32] = wire::unhex(&client_pubkey_hex)
        .filter(|b| b.len() == 32)
        .ok_or_else(|| anyhow!("KEYWARD_CLIENT_PUBKEY must be 32-byte hex"))?
        .try_into()
        .unwrap();
    let client_pub = seal::x25519_public(
        &VerifyingKey::from_bytes(&vk_bytes).map_err(|e| anyhow!("bad client pubkey: {e}"))?,
    );

    let state = Arc::new(ShimState {
        client: reqwest::Client::new(),
        sealed_url: format!("{}/kw/sealed", node.trim_end_matches('/')),
        token,
        client_pub,
    });

    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/responses", post(responses))
        .route("/v1/messages", post(messages))
        .with_state(state);
    let http = TcpListener::bind(&listen).await?;
    println!("[shim] OpenAI endpoint on http://{listen}  (sealing to the client; node stays blind)");
    println!("[shim] point your app:  OPENAI_BASE_URL=http://{listen}/v1  OPENAI_API_KEY=anything");
    axum::serve(http, app).await?;
    Ok(())
}

async fn chat_completions(State(s): State<Arc<ShimState>>, Json(body): Json<Value>) -> Response {
    relay(s, "openai", body).await
}
async fn responses(State(s): State<Arc<ShimState>>, Json(body): Json<Value>) -> Response {
    relay(s, "openai-responses", body).await
}
async fn messages(State(s): State<Arc<ShimState>>, Json(body): Json<Value>) -> Response {
    relay(s, "anthropic", body).await
}

fn gateway_error(msg: impl Into<String>) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({ "error": { "message": msg.into() } })),
    )
        .into_response()
}

async fn relay(s: Arc<ShimState>, provider: &str, body: Value) -> Response {
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Seal {provider, request} to the client's identity key.
    let inner = json!({ "provider": provider, "request": body });
    let (channel, epk) = Sealed::initiator(&s.client_pub);
    let sealed = match channel.seal(inner.to_string().as_bytes()) {
        Ok(x) => x,
        Err(e) => return gateway_error(e.to_string()),
    };
    let blob = format!("{epk}{sealed}");

    // Relay through the (blind) node, authorized by the routing token.
    let resp = match s
        .client
        .post(&s.sealed_url)
        .header("authorization", format!("Bearer {}", s.token))
        .body(blob)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return gateway_error(format!("node unreachable: {e}")),
    };
    let sse = match resp.text().await {
        Ok(t) => t,
        Err(e) => return gateway_error(e.to_string()),
    };

    // Parse the node's SSE: sealed-blob data lines → decrypt to native chunks; an
    // `event: error` data line is the cleartext terminal; `[DONE]` ends the stream.
    let mut deltas: Vec<Value> = Vec::new();
    let mut error: Option<String> = None;
    let mut last_event = "";
    for line in sse.lines() {
        if let Some(ev) = line.strip_prefix("event:") {
            last_event = ev.trim();
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if last_event == "error" {
                error = Some(data.to_string());
            } else if data != "[DONE]"
                && let Ok(pt) = channel.open(data)
                && let Ok(v) = serde_json::from_slice::<Value>(&pt)
                && let Some(chunk) = v.get("chunk")
            {
                deltas.push(chunk.clone());
            }
            last_event = "";
        }
    }
    if let Some(e) = error {
        return gateway_error(e);
    }

    if streaming {
        let events = deltas
            .into_iter()
            .map(|d| Ok::<_, Infallible>(Event::default().data(d.to_string())))
            .chain(std::iter::once(Ok(Event::default().data("[DONE]"))));
        Sse::new(futures_util::stream::iter(events)).into_response()
    } else {
        let content: String = deltas.iter().map(chunk_text).collect();
        Json(json!({
            "id": "chatcmpl-keyward-shim",
            "object": "chat.completion",
            "model": model,
            "choices": [{ "index": 0, "message": { "role": "assistant", "content": content }, "finish_reason": "stop" }]
        }))
        .into_response()
    }
}
