# keyward-sdk

The Keyward **Orchestrator SDK** for Rust — integrate your app as the brain that
decides *what* to do but never holds the key. You bind a listener the Owner's
**Executor** dials into, pair, then submit work intents and stream the provider's
native response back. The key stays on the Executor.

```rust
use keyward_sdk::{serve_one, Config, Event};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::new("my-app", "orch_myapp", "pt_one_time_token");
    println!("show this to the user out of band — root fp {}", cfg.root_fingerprint());

    let listener = TcpListener::bind("127.0.0.1:8787").await?;
    let session = serve_one(&listener, &cfg).await?; // waits for an executor to pair

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

Run the example against a real executor:

```sh
cargo run -p keyward-sdk --example relay_chat
# then, in another shell, dial an executor in (provider "mock" needs no key):
KEYWARD_ORCH_URL=ws://127.0.0.1:8799 KEYWARD_PAIRING_TOKEN=pt_sdk cargo run -- executor
```

- `serve_one` accepts one Executor, authenticates it (pairing token + identity +
  optional `authorized_executors` allow-list), and pairs (root→operational-key chain).
- `Session::submit(provider, request)` sends a work intent — the provider-native body
  minus any credential — and returns a stream of native `Event`s.
- For zero-code-change integration of an *existing* app, use `keyward proxy` instead;
  this SDK is for embedding the orchestrator in-process.

See the [protocol spec](../../docs/spec.md) and the [integration guide](../../docs/en/integration.md).
