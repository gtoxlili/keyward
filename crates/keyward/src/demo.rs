//! `keyward demo` — a self-contained end-to-end run, no key and no network.
//! It stands up a mock Orchestrator and an Executor over a real localhost
//! WebSocket and shows: dial-out pairing with a signed `sid`, TOFU key pinning,
//! a policy-allowed intent that streams + meters usage, and a policy-blocked
//! intent that the Executor refuses before the provider is ever contacted.

use std::sync::Arc;

use anyhow::Result;
use ed25519_dalek::SigningKey;
use keyward_proto::{Budget, Policy, Rate};
use rand_core::OsRng;
use secrecy::SecretString;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::executor::{self, ExecutorConfig};
use crate::orchestrator::{self, OrchestratorConfig};

pub async fn run() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let url = format!("ws://{addr}");
    let token = "pt_demo_one_time";

    println!("== Keyward demo ==");
    println!("orchestrator (holds NO key) listening at {url}\n");

    // The app's scripted work. Bodies are provider-NATIVE, sans credential —
    // OpenAI Chat Completions for the first, Anthropic Messages for the second.
    let intents = vec![
        (
            "mock".to_string(),
            json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello Keyward — are you holding my key?"}], "stream": true}),
        ),
        (
            "mock".to_string(),
            json!({"model": "claude-3-5-sonnet-20241022", "max_tokens": 1024, "messages": [{"role": "user", "content": "And do you relay my native Anthropic body too?"}], "stream": true}),
        ),
        (
            "mock".to_string(),
            json!({"model": "gpt-4-turbo", "messages": [{"role": "user", "content": "now try a model the owner did not allow"}], "stream": true}),
        ),
    ];

    let ocfg = OrchestratorConfig {
        name: "acme-agent".into(),
        id: "orch_acme".into(),
        pairing_token: token.into(),
        signing: SigningKey::generate(&mut OsRng),
        intents,
    };
    let server = tokio::spawn(async move {
        match listener.accept().await {
            Ok((stream, _)) => {
                if let Err(e) = orchestrator::serve(stream, ocfg).await {
                    eprintln!("[orchestr] error: {e}");
                }
            }
            Err(e) => eprintln!("[orchestr] accept failed: {e}"),
        }
    });

    // The owner's Executor: holds the key, allows only gpt-4o* and one Claude.
    let policy = Policy {
        providers: Some(vec!["mock".into(), "openai".into()]),
        models: Some(vec!["gpt-4o*".into(), "claude-3-5-sonnet-*".into()]),
        orchestrators: Some(vec!["orch_acme".into()]),
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
    let cfg = ExecutorConfig {
        name: "keyward-exec".into(),
        providers: vec!["mock".into()],
        policy,
        provider_key: SecretString::from("sk-DEMO-this-string-never-leaves-the-executor".to_string()),
        pinned: Arc::new(Mutex::new(None)),
    };

    executor::run(&url, token, cfg).await?;
    let _ = server.await;
    println!("\n== demo complete ==");
    println!("note: the key string lived only in the Executor process; it never appears in any frame above.");
    Ok(())
}

/// `keyward resume-demo` — §7 in action: stream an intent, drop the channel
/// mid-stream, let the Executor re-dial and resume from where the Orchestrator
/// left off, then cancel a second intent.
pub async fn run_resume() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let url = format!("ws://{addr}");
    let token = "pt_demo_resume";

    println!("== Keyward resume / cancel demo ==");
    println!("orchestrator at {url}\n");

    let ocfg = OrchestratorConfig {
        name: "acme-agent".into(),
        id: "orch_acme".into(),
        pairing_token: token.into(),
        signing: SigningKey::generate(&mut OsRng),
        intents: Vec::new(), // this demo scripts its own two-connection flow
    };
    let server = tokio::spawn(async move {
        if let Err(e) = orchestrator::serve_resume_demo(listener, ocfg).await {
            eprintln!("[orchestr] error: {e}");
        }
    });

    let policy = Policy {
        providers: Some(vec!["mock".into()]),
        models: Some(vec!["gpt-4o*".into()]),
        orchestrators: Some(vec!["orch_acme".into()]),
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
    let cfg = ExecutorConfig {
        name: "keyward-exec".into(),
        providers: vec!["mock".into()],
        policy,
        provider_key: SecretString::from("sk-DEMO-this-string-never-leaves-the-executor".to_string()),
        pinned: Arc::new(Mutex::new(None)),
    };

    executor::run(&url, token, cfg).await?;
    let _ = server.await;
    println!("\n== resume / cancel demo complete ==");
    Ok(())
}
