//! `keyward demo` — a self-contained end-to-end run, no key and no network.
//! It stands up a mock Node and a Client over a real localhost
//! WebSocket and shows: dial-out pairing with a signed `sid`, TOFU key pinning,
//! a policy-allowed intent that streams + meters usage, and a policy-blocked
//! intent that the Client refuses before the provider is ever contacted.

use std::sync::Arc;

use anyhow::Result;
use ed25519_dalek::SigningKey;
use keyward_proto::{Budget, Policy, Rate};
use rand_core::OsRng;
use secrecy::SecretString;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::client::{self, ClientConfig};
use crate::secret::KeySource;
use crate::session::{self, NodeConfig};

pub async fn run() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let url = format!("ws://{addr}");
    let token = "pt_demo_one_time";

    println!("== Keyward demo ==");
    println!("node (holds NO key) listening at {url}\n");

    // The app's scripted work. Bodies are provider-NATIVE, sans credential — one
    // each in OpenAI Chat Completions, Anthropic Messages, and the OpenAI
    // Responses API (note `input` instead of `messages`), then a blocked model.
    let intents = vec![
        (
            "mock".to_string(),
            json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello Keyward — are you holding my key?"}], "stream": true}),
        ),
        (
            "mock".to_string(),
            json!({"model": "claude-sonnet-4-5", "max_tokens": 1024, "messages": [{"role": "user", "content": "And do you relay my native Anthropic body too?"}], "stream": true}),
        ),
        (
            "mock".to_string(),
            json!({"model": "gpt-4o", "input": "And the Responses API, with its own event shape?", "stream": true}),
        ),
        (
            "mock".to_string(),
            json!({"model": "gpt-4-turbo", "messages": [{"role": "user", "content": "now try a model the owner did not allow"}], "stream": true}),
        ),
    ];

    // The Client's identity; the Node allow-lists it (a SaaS would
    // allow-list its registered users exactly this way).
    let client_identity = SigningKey::generate(&mut OsRng);
    let client_pubkey = crate::wire::hex(&client_identity.verifying_key().to_bytes());

    let ocfg = NodeConfig {
        name: "acme-agent".into(),
        id: "node_acme".into(),
        pairing_token: token.into(),
        root: SigningKey::generate(&mut OsRng),
        authorized_clients: Some(vec![client_pubkey]),
        claimed_tokens: Default::default(),
        single_use_token: true,
        intents,
    };
    let server = tokio::spawn(async move {
        match listener.accept().await {
            Ok((stream, _)) => {
                if let Err(e) = session::serve(stream, ocfg).await {
                    eprintln!("[node] error: {e}");
                }
            }
            Err(e) => eprintln!("[node] accept failed: {e}"),
        }
    });

    // The owner's Client: holds the key, allows only gpt-4o* and Claude Sonnet.
    let policy = Policy {
        providers: Some(vec!["mock".into(), "openai".into()]),
        models: Some(vec!["gpt-4o*".into(), "claude-sonnet-*".into()]),
        nodes: Some(vec!["node_acme".into()]),
        budget: Some(Budget {
            limit_usd: 5.0,
            window: "month".into(),
            spent_usd: 0.0,
        }),
        rate: Some(Rate {
            rpm: Some(60),
            tpm: None,
        }),
        expires_at: None,
    };
    let cfg = ClientConfig {
        name: "keyward-client".into(),
        providers: vec!["mock".into()],
        policy,
        keys: KeySource::Fixed(SecretString::from(
            "sk-DEMO-this-string-never-leaves-the-client".to_string(),
        )),
        identity: client_identity,
        pinned: Arc::new(Mutex::new(None)),
        events: None,
    };

    client::run(&url, token, cfg).await?;
    let _ = server.await;
    println!("\n== demo complete ==");
    println!("note: the key string lived only in the Client process; it never appears in any frame above.");
    Ok(())
}

/// `keyward resume-demo` — §7 in action: stream an intent, drop the channel
/// mid-stream, let the Client re-dial and resume from where the Node
/// left off, then cancel a second intent.
pub async fn run_resume() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let url = format!("ws://{addr}");
    let token = "pt_demo_resume";

    println!("== Keyward resume / cancel demo ==");
    println!("node at {url}\n");

    let client_identity = SigningKey::generate(&mut OsRng);
    let client_pubkey = crate::wire::hex(&client_identity.verifying_key().to_bytes());

    let ocfg = NodeConfig {
        name: "acme-agent".into(),
        id: "node_acme".into(),
        pairing_token: token.into(),
        root: SigningKey::generate(&mut OsRng),
        authorized_clients: Some(vec![client_pubkey]),
        claimed_tokens: Default::default(),
        single_use_token: true,
        intents: Vec::new(), // this demo scripts its own two-connection flow
    };
    let server = tokio::spawn(async move {
        if let Err(e) = session::serve_resume_demo(listener, ocfg).await {
            eprintln!("[node] error: {e}");
        }
    });

    let policy = Policy {
        providers: Some(vec!["mock".into()]),
        models: Some(vec!["gpt-4o*".into()]),
        nodes: Some(vec!["node_acme".into()]),
        budget: Some(Budget {
            limit_usd: 5.0,
            window: "month".into(),
            spent_usd: 0.0,
        }),
        rate: Some(Rate {
            rpm: Some(120),
            tpm: None,
        }),
        expires_at: None,
    };
    let cfg = ClientConfig {
        name: "keyward-client".into(),
        providers: vec!["mock".into()],
        policy,
        keys: KeySource::Fixed(SecretString::from(
            "sk-DEMO-this-string-never-leaves-the-client".to_string(),
        )),
        identity: client_identity,
        pinned: Arc::new(Mutex::new(None)),
        events: None,
    };

    client::run(&url, token, cfg).await?;
    let _ = server.await;
    println!("\n== resume / cancel demo complete ==");
    Ok(())
}
