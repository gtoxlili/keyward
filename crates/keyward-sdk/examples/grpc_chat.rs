//! Same as `relay_chat`, but the Node speaks **gRPC**. Run it, then dial a
//! gRPC Client into it:
//!
//!   cargo run -p keyward-sdk --features grpc --example grpc_chat
//!   # then, in another shell:
//!   KEYWARD_NODE_URL=grpc://127.0.0.1:8810 KEYWARD_PAIRING_TOKEN=pt_grpc \
//!     cargo run -p keyward --features grpc -- client      # provider "mock" needs no key
//!
//! Set PROVIDER=openai (and build the client with --features grpc,openai + a key)
//! for a real call.

use keyward_sdk::{Config, Event, serve_one_grpc};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::new("sdk-grpc-example", "node_grpc", "pt_grpc");
    let addr = "127.0.0.1:8810".parse().unwrap();
    println!(
        "node (gRPC) on grpc://127.0.0.1:8810  (root fingerprint {})",
        cfg.root_fingerprint()
    );
    println!(
        "dial a client:  KEYWARD_NODE_URL=grpc://127.0.0.1:8810 KEYWARD_PAIRING_TOKEN=pt_grpc keyward client  (built --features grpc)"
    );

    let session = serve_one_grpc(addr, &cfg).await?;
    println!("client paired over gRPC — sending a work intent…");

    let provider = std::env::var("PROVIDER").unwrap_or_else(|_| "mock".into());
    let mut rx = session
        .submit(
            &provider,
            serde_json::json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": "Say hello to the Keyward gRPC transport."}],
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
