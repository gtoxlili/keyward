//! Transport selection for the Client. It always dials OUT over exactly one
//! bidirectional channel (§1); both adapters yield the same `(out, inbound)` `Frame`
//! channels, so `serve_once` is transport-agnostic. The URL scheme picks the adapter:
//! `ws://` / `wss://` → WebSocket; `grpc://` / `grpcs://` → gRPC (feature `grpc`).

use anyhow::{Result, anyhow};
use futures_util::StreamExt;
use keyward_proto::Frame;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;

use crate::wire;

const CHAN: usize = 64;

/// Dial `url` and return `(out, inbound)`: send frames on `out`, receive from `inbound`.
/// When either task sees its socket end, the channels close — which the caller treats
/// as session suspension and reconnects (§7).
pub async fn connect(url: &str) -> Result<(mpsc::Sender<Frame>, mpsc::Receiver<Frame>)> {
    if url.starts_with("ws://") || url.starts_with("wss://") {
        ws_connect(url).await
    } else if url.starts_with("grpc://") || url.starts_with("grpcs://") {
        grpc_connect(url).await
    } else {
        Err(anyhow!(
            "unsupported node URL scheme: {url} (use ws://, wss://, grpc:// or grpcs://)"
        ))
    }
}

async fn ws_connect(url: &str) -> Result<(mpsc::Sender<Frame>, mpsc::Receiver<Frame>)> {
    let (ws, _resp) = connect_async(url)
        .await
        .map_err(|e| anyhow!("dial-out to {url} failed: {e}"))?;
    let (mut write, mut read) = ws.split();

    let (out, mut out_rx) = mpsc::channel::<Frame>(CHAN);
    tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            if wire::send(&mut write, &frame).await.is_err() {
                break;
            }
        }
    });

    let (in_tx, in_rx) = mpsc::channel::<Frame>(CHAN);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                // The consumer dropped `inbound` (serve_once returned): stop and drop
                // `read` so the socket closes — otherwise we'd park on recv forever and
                // the peer would never see EOF.
                _ = in_tx.closed() => break,
                msg = wire::recv(&mut read) => match msg {
                    Ok(Some(frame)) => {
                        if in_tx.send(frame).await.is_err() {
                            break;
                        }
                    }
                    _ => break, // clean close or error → channel ends → suspend (§7)
                },
            }
        }
    });

    Ok((out, in_rx))
}

#[cfg(feature = "grpc")]
async fn grpc_connect(url: &str) -> Result<(mpsc::Sender<Frame>, mpsc::Receiver<Frame>)> {
    // tonic speaks http(s). Map the scheme: grpc:// → http://, grpcs:// → https://.
    // `replacen(.., 1)` rewrites only the leading scheme, never a "grpc" in the host.
    let http = url.replacen("grpc", "http", 1);
    keyward_grpc::dial(&http).await
}

#[cfg(not(feature = "grpc"))]
async fn grpc_connect(url: &str) -> Result<(mpsc::Sender<Frame>, mpsc::Receiver<Frame>)> {
    Err(anyhow!(
        "{url}: this build has no gRPC support — rebuild with `--features grpc`"
    ))
}
