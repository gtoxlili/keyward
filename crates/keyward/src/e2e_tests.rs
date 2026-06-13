//! End-to-end integration tests that drive the real `executor::run` against a
//! test-written Orchestrator over a localhost WebSocket, asserting on the frames
//! the Executor produces. These complement the `demo`/`resume-demo` binaries
//! (which are smoke tests printed for a human) with machine-checked assertions.

use std::sync::Arc;

use ed25519_dalek::{Signer, SigningKey};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use keyward_proto::{Body, Frame, Peer, Policy};
use rand_core::OsRng;
use secrecy::SecretString;
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};

use crate::executor::{self, ExecutorConfig};
use crate::secret::KeySource;
use crate::{identity, wire};

type WsW = SplitSink<WebSocketStream<TcpStream>, Message>;
type WsR = SplitStream<WebSocketStream<TcpStream>>;

/// Bind a localhost listener and spawn the real Executor dialing it. Returns the
/// accepted server-side WebSocket halves and the Executor's join handle.
async fn harness(policy: Policy) -> (WsW, WsR, tokio::task::JoinHandle<anyhow::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("ws://{addr}");

    let cfg = ExecutorConfig {
        name: "test-exec".into(),
        providers: vec!["mock".into()],
        policy,
        keys: KeySource::Fixed(SecretString::from("sk-TEST-never-leaves".to_string())),
        identity: SigningKey::generate(&mut OsRng),
        pinned: Arc::new(Mutex::new(None)),
    };
    let exec = tokio::spawn(async move { executor::run(&url, "pt_test", cfg).await });

    let (stream, _) = listener.accept().await.unwrap();
    let (w, r) = accept_async(stream).await.unwrap().split();
    (w, r, exec)
}

/// Send a valid `paired` (fresh root + delegated op key) and return the sid.
async fn pair(w: &mut WsW, root: &SigningKey) -> String {
    let sid = "kw_sess_test".to_string();
    let op = SigningKey::generate(&mut OsRng);
    let cert = identity::issue_op_cert(root, &op.verifying_key(), identity::now_unix() + 3600);
    let sig = wire::hex(&op.sign(sid.as_bytes()).to_bytes());
    let frame = Frame::new(
        Some(sid.clone()),
        "m_paired",
        Body::Paired {
            orchestrator: Peer {
                name: "test-orch".into(),
                version: None,
                id: Some("orch_test".into()),
            },
            root_pubkey: wire::hex(&root.verifying_key().to_bytes()),
            op: cert,
            sig,
        },
    );
    wire::send(w, &frame).await.unwrap();
    sid
}

fn work(sid: &str, mid: &str, model: &str) -> Frame {
    Frame::new(
        Some(sid.to_string()),
        mid,
        Body::Work {
            provider: "mock".into(),
            request: json!({"model": model, "messages": [{"role": "user", "content": "hi"}], "stream": true}),
        },
    )
}

#[tokio::test]
async fn allowed_work_streams_with_monotonic_seq_and_usage() {
    let policy = Policy {
        providers: Some(vec!["mock".into()]),
        models: Some(vec!["gpt-4o".into()]),
        ..Default::default()
    };
    let (mut w, mut r, exec) = harness(policy).await;

    // hello -> pair
    assert!(matches!(
        wire::recv(&mut r).await.unwrap().unwrap().body,
        Body::Hello { .. }
    ));
    let root = SigningKey::generate(&mut OsRng);
    let sid = pair(&mut w, &root).await;

    wire::send(&mut w, &work(&sid, "work1", "gpt-4o")).await.unwrap();

    let mut expected_seq = 0u64;
    let usage = loop {
        let f = wire::recv(&mut r).await.unwrap().unwrap();
        if f.mid != "work1" {
            continue;
        }
        match f.body {
            Body::WorkAccepted {} => {}
            Body::WorkChunk { seq, .. } => {
                assert_eq!(seq, expected_seq, "seq must be monotonic from 0");
                expected_seq += 1;
            }
            Body::WorkDone { usage, .. } => break usage,
            other => panic!("unexpected frame: {other:?}"),
        }
    };
    assert!(expected_seq > 0, "expected at least one chunk");
    assert!(usage.output_tokens > 0, "usage should be metered");

    wire::send(
        &mut w,
        &Frame::new(
            Some(sid),
            "c",
            Body::Close {
                reason: "done".into(),
            },
        ),
    )
    .await
    .unwrap();
    exec.await.unwrap().unwrap();
}

#[tokio::test]
async fn disallowed_model_is_refused_before_provider() {
    let policy = Policy {
        providers: Some(vec!["mock".into()]),
        models: Some(vec!["gpt-4o".into()]),
        ..Default::default()
    };
    let (mut w, mut r, exec) = harness(policy).await;
    assert!(matches!(
        wire::recv(&mut r).await.unwrap().unwrap().body,
        Body::Hello { .. }
    ));
    let root = SigningKey::generate(&mut OsRng);
    let sid = pair(&mut w, &root).await;

    wire::send(&mut w, &work(&sid, "work_bad", "gpt-4-turbo"))
        .await
        .unwrap();
    loop {
        let f = wire::recv(&mut r).await.unwrap().unwrap();
        if f.mid != "work_bad" {
            continue;
        }
        match f.body {
            Body::WorkError { code, .. } => {
                assert_eq!(code, "policy_model");
                break;
            }
            Body::WorkAccepted {} => panic!("disallowed model must not be accepted"),
            other => panic!("unexpected frame: {other:?}"),
        }
    }
    wire::send(
        &mut w,
        &Frame::new(
            Some(sid),
            "c",
            Body::Close {
                reason: "done".into(),
            },
        ),
    )
    .await
    .unwrap();
    exec.await.unwrap().unwrap();
}

#[tokio::test]
async fn work_before_pairing_is_rejected() {
    let (mut w, mut r, exec) = harness(Policy::default()).await;
    assert!(matches!(
        wire::recv(&mut r).await.unwrap().unwrap().body,
        Body::Hello { .. }
    ));

    // Send work without pairing first.
    wire::send(&mut w, &work("kw_sess_x", "early", "gpt-4o"))
        .await
        .unwrap();
    let f = wire::recv(&mut r).await.unwrap().unwrap();
    match f.body {
        Body::WorkError { code, .. } => assert_eq!(code, "bad_request"),
        other => panic!("expected bad_request, got {other:?}"),
    }
    wire::send(
        &mut w,
        &Frame::new(
            None,
            "c",
            Body::Close {
                reason: "done".into(),
            },
        ),
    )
    .await
    .unwrap();
    exec.await.unwrap().unwrap();
}

#[tokio::test]
async fn op_key_not_signed_by_claimed_root_is_refused() {
    let (mut w, mut r, exec) = harness(Policy::default()).await;
    assert!(matches!(
        wire::recv(&mut r).await.unwrap().unwrap().body,
        Body::Hello { .. }
    ));

    // Claim one root, but delegate the op key with a DIFFERENT root.
    let claimed_root = SigningKey::generate(&mut OsRng);
    let real_signer = SigningKey::generate(&mut OsRng);
    let op = SigningKey::generate(&mut OsRng);
    let sid = "kw_sess_forge".to_string();
    let cert = identity::issue_op_cert(&real_signer, &op.verifying_key(), identity::now_unix() + 3600);
    let sig = wire::hex(&op.sign(sid.as_bytes()).to_bytes());
    let frame = Frame::new(
        Some(sid.clone()),
        "m_paired",
        Body::Paired {
            orchestrator: Peer {
                name: "evil".into(),
                version: None,
                id: Some("orch_evil".into()),
            },
            root_pubkey: wire::hex(&claimed_root.verifying_key().to_bytes()),
            op: cert,
            sig,
        },
    );
    wire::send(&mut w, &frame).await.unwrap();

    // The Executor refuses to bind and tears down the channel (a clean close or a
    // reset); either way our reads end and no work is ever accepted.
    while let Ok(Some(frame)) = wire::recv(&mut r).await {
        assert!(
            !matches!(frame.body, Body::WorkAccepted {}),
            "must not accept work after a forged chain"
        );
    }
    exec.await.unwrap().unwrap();
}
