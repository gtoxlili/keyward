# keyward-sdk

The Keyward **Node SDK** for Rust — embed a Node in-process. You bind a listener the
Owner's **Client** dials into, pair, then submit work intents and stream the provider's
native response back. Your code decides the requests; the key stays on the Client and
never reaches you.

This is the in-process path. The **zero-integration** path needs no SDK at all: an
unaware app just points its OpenAI base URL at a standalone `keyward node` (with a
routing token as its API key). Reach for this SDK when you want the Node logic *inside*
your own Rust process.

```rust
use keyward_sdk::{serve_one, Config, Event};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::new("my-app", "node_myapp", "pt_one_time_token");
    println!("show this to the user out of band — root fp {}", cfg.root_fingerprint());

    let listener = TcpListener::bind("127.0.0.1:8787").await?;
    let session = serve_one(&listener, &cfg).await?; // waits for a client to pair

    let mut rx = session.submit("openai", serde_json::json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "hi"}],
        "stream": true
    })).await;

    while let Some(ev) = rx.recv().await {
        match ev {
            Event::Chunk(c)   => { /* relay the native chunk to your user */ }
            Event::Done(u)    => { println!("in={} out={}", u.input_tokens, u.output_tokens); break; }
            Event::Error(e)   => { eprintln!("{e}"); break; }
        }
    }
    Ok(())
}
```

Run the example against the real Rust Client:

```sh
cargo run -p keyward-sdk --example relay_chat
# then, in another shell, dial a client in (provider "mock" needs no key):
KEYWARD_NODE_URL=ws://127.0.0.1:8799 KEYWARD_PAIRING_TOKEN=pt_sdk cargo run -- client
```

- `serve_one` accepts one Client, authenticates it (pairing token + identity +
  optional `authorized_clients` allow-list), and pairs (root→operational-key chain).
- `Session::submit(provider, request)` sends a work intent — the provider-native body
  minus any credential — and returns a stream of native `Event`s.
- For zero-code-change integration of an *existing, unaware* app, skip the SDK and run a
  standalone `keyward node` — the app just sets its OpenAI base URL + an API-key token.

See the [protocol spec](../../docs/spec.md) and the [integration guide](../../docs/en/integration.md).
