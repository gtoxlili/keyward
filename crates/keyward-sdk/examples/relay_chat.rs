//! Minimal Node using the SDK. Run it, then dial a Client into it:
//!
//!   cargo run -p keyward-sdk --example relay_chat
//!   # then, in another shell:
//!   KEYWARD_NODE_URL=ws://127.0.0.1:8799 KEYWARD_PAIRING_TOKEN=pt_sdk \
//!     cargo run -- client        # provider "mock" needs no key
//!
//! Set PROVIDER=openai (and build the client with --features openai + a key) for
//! a real call.

use keyward_sdk::{Config, Event, serve_one};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::new("sdk-example", "node_sdk", "pt_sdk");
    let listener = TcpListener::bind("127.0.0.1:8799").await?;
    println!(
        "node on ws://127.0.0.1:8799  (root fingerprint {})",
        cfg.root_fingerprint()
    );
    println!(
        "dial a client:  KEYWARD_NODE_URL=ws://127.0.0.1:8799 KEYWARD_PAIRING_TOKEN=pt_sdk keyward client"
    );

    let session = serve_one(&listener, &cfg).await?;
    println!("client paired — sending a work intent…");

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
