//! Minimal Orchestrator using the SDK. Run it, then dial an Executor into it:
//!
//!   cargo run -p keyward-sdk --example relay_chat
//!   # then, in another shell:
//!   KEYWARD_ORCH_URL=ws://127.0.0.1:8799 KEYWARD_PAIRING_TOKEN=pt_sdk \
//!     cargo run -- executor        # provider "mock" needs no key
//!
//! Set PROVIDER=openai (and build the executor with --features openai + a key) for
//! a real call.

use keyward_sdk::{serve_one, Config, Event};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::new("sdk-example", "orch_sdk", "pt_sdk");
    let listener = TcpListener::bind("127.0.0.1:8799").await?;
    println!(
        "orchestrator on ws://127.0.0.1:8799  (root fingerprint {})",
        cfg.root_fingerprint()
    );
    println!("dial an executor:  KEYWARD_ORCH_URL=ws://127.0.0.1:8799 KEYWARD_PAIRING_TOKEN=pt_sdk keyward executor");

    let session = serve_one(&listener, &cfg).await?;
    println!("executor paired — sending a work intent…");

    let provider = std::env::var("PROVIDER").unwrap_or_else(|_| "mock".into());
    let mut rx = session
        .submit(
            &provider,
            serde_json::json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Say hello to the Keyward SDK."}],
                "stream": true
            }),
        )
        .await;

    let mut text = String::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            Event::Chunk(c) => {
                if let Some(t) = c.pointer("/choices/0/delta/content").and_then(|v| v.as_str()) {
                    text.push_str(t);
                }
            }
            Event::Done(u) => {
                println!(
                    "\nassembled: {text:?}\nusage in={} out={}",
                    u.input_tokens, u.output_tokens
                );
                break;
            }
            Event::Error(e) => {
                eprintln!("error: {e}");
                break;
            }
        }
    }
    Ok(())
}
