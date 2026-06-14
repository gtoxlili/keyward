//! gRPC transport for Keyward.
//!
//! The protocol is transport-agnostic (spec §1): it needs exactly one reliable,
//! ordered, bidirectional, message-oriented channel. This crate provides that over a
//! gRPC bidirectional stream and exposes the **same `Frame` channels** the WebSocket
//! adapter does — so the Client and the Node SDK run identical logic on top,
//! whichever transport carries it.
//!
//! The Client dials OUT (it is the gRPC **client**); the Node listens (gRPC
//! **server**). That keeps the no-inbound-ports invariant: a single bidirectional
//! stream, opened by the Client, carries the whole session in both directions.
//!
//! Each gRPC message wraps one canonical Keyward JSON frame (`Frame { json }`) — gRPC
//! is the pipe, the JSON envelope from spec.md is unchanged.

use anyhow::{Result, anyhow};
use futures_util::StreamExt;
use keyward_proto::Frame;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

/// Generated protobuf + tonic stubs for `service Keyward`.
pub mod pb {
    tonic::include_proto!("keyward.v0");
}
use pb::Frame as PbFrame;
use pb::keyward_client::KeywardClient;
use pb::keyward_server::{Keyward, KeywardServer};

const CHAN: usize = 64;

fn to_pb(f: &Frame) -> PbFrame {
    PbFrame {
        json: serde_json::to_string(f).unwrap_or_default(),
    }
}

fn from_pb(p: &PbFrame) -> Option<Frame> {
    serde_json::from_str(&p.json).ok()
}

/// Forward an inbound gRPC stream of frames into a plain `Frame` channel. Stops on a
/// transport error or a malformed message, dropping the sender so the reader sees the
/// channel close (which the protocol treats as session suspension, §7).
async fn pump_in(mut stream: tonic::Streaming<PbFrame>, in_tx: mpsc::Sender<Frame>) {
    loop {
        tokio::select! {
            biased;
            // Consumer dropped the receiver → stop and drop the gRPC stream.
            _ = in_tx.closed() => break,
            item = stream.next() => match item {
                Some(Ok(pf)) => {
                    if let Some(f) = from_pb(&pf)
                        && in_tx.send(f).await.is_err()
                    {
                        break;
                    }
                }
                _ => break, // stream ended or errored
            },
        }
    }
}

/// **Client side.** Dial the Node's gRPC endpoint (an `http://…` or
/// `https://…` URL) and open the session stream. Returns `(out, inbound)`: send frames
/// on `out`, receive them from `inbound` — the same shape the WebSocket adapter yields,
/// so the caller's logic is transport-agnostic.
pub async fn dial(url: &str) -> Result<(mpsc::Sender<Frame>, mpsc::Receiver<Frame>)> {
    let mut client = KeywardClient::connect(url.to_string())
        .await
        .map_err(|e| anyhow!("gRPC dial-out to {url} failed: {e}"))?;

    let (out, out_rx) = mpsc::channel::<Frame>(CHAN);
    let outbound = ReceiverStream::new(out_rx).map(|f| to_pb(&f));
    let inbound = client
        .open(Request::new(outbound))
        .await
        .map_err(|e| anyhow!("gRPC Open failed: {e}"))?
        .into_inner();

    let (in_tx, in_rx) = mpsc::channel::<Frame>(CHAN);
    tokio::spawn(pump_in(inbound, in_tx));
    Ok((out, in_rx))
}

/// **Node side.** Serve gRPC at `addr` and accept ONE Client's session
/// stream, returning the same `(out, inbound)` frame channels as [`dial`]. (v0: one
/// Client per call, matching the SDK's `serve_one`.)
pub async fn accept_one(addr: SocketAddr) -> Result<(mpsc::Sender<Frame>, mpsc::Receiver<Frame>)> {
    let (handoff_tx, handoff_rx) = oneshot::channel();
    let svc = Svc {
        handoff: Arc::new(Mutex::new(Some(handoff_tx))),
    };
    tokio::spawn(async move {
        let _ = tonic::transport::Server::builder()
            .add_service(KeywardServer::new(svc))
            .serve(addr)
            .await;
    });
    handoff_rx
        .await
        .map_err(|_| anyhow!("gRPC server stopped before an client connected"))
}

type Handoff = oneshot::Sender<(mpsc::Sender<Frame>, mpsc::Receiver<Frame>)>;

struct Svc {
    handoff: Arc<Mutex<Option<Handoff>>>,
}

#[tonic::async_trait]
impl Keyward for Svc {
    type OpenStream = ReceiverStream<Result<PbFrame, Status>>;

    async fn open(
        &self,
        req: Request<tonic::Streaming<PbFrame>>,
    ) -> Result<Response<Self::OpenStream>, Status> {
        // Inbound: client → us, demoted to a plain Frame channel.
        let (in_tx, in_rx) = mpsc::channel::<Frame>(CHAN);
        tokio::spawn(pump_in(req.into_inner(), in_tx));

        // Outbound: us → client. The caller sends on `out`; we re-wrap as PbFrame
        // and feed the response stream.
        let (out, mut out_rx) = mpsc::channel::<Frame>(CHAN);
        let (pb_tx, pb_rx) = mpsc::channel::<Result<PbFrame, Status>>(CHAN);
        tokio::spawn(async move {
            while let Some(f) = out_rx.recv().await {
                if pb_tx.send(Ok(to_pb(&f))).await.is_err() {
                    break;
                }
            }
        });

        // Hand the channels to whoever is awaiting accept_one. A second connection
        // (handoff already taken) just gets an open-but-unserved stream.
        if let Some(tx) = self.handoff.lock().unwrap().take() {
            let _ = tx.send((out, in_rx));
        }
        Ok(Response::new(ReceiverStream::new(pb_rx)))
    }
}
