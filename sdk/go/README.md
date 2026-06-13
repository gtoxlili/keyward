# keyward (Go)

The Keyward **Orchestrator SDK** for Go — integrate your app as the brain that
decides *what* to do but never holds the key. You bind a listener the Owner's
**Executor** dials into, pair, then submit work intents and stream the provider's
native response back. The key stays on the Executor.

The wire + crypto are byte-compatible with the Rust reference Executor (verified
cross-language in CI / by the example below).

```go
import keyward "github.com/gtoxlili/keyward/sdk/go"

cfg := keyward.NewConfig("my-app", "orch_myapp", "pt_one_time_token")
fmt.Println("show this to the user out of band — root fp", cfg.RootFingerprint())

session, err := keyward.ServeOne("127.0.0.1:8787", cfg) // waits for an executor to pair
if err != nil { log.Fatal(err) }

req := json.RawMessage(`{"model":"gpt-4o","messages":[{"role":"user","content":"hi"}],"stream":true}`)
events, _ := session.Submit("openai", req)
for ev := range events {
    switch ev.Kind {
    case keyward.Chunk: // relay ev.Delta (a native chunk) to your user
    case keyward.Done:  fmt.Printf("in=%d out=%d\n", ev.Usage.InputTokens, ev.Usage.OutputTokens)
    case keyward.Error: log.Println(ev.Err)
    }
}
```

Run the example against the real Rust executor:

```sh
go run ./example
# then, in another shell (provider "mock" needs no key):
KEYWARD_ORCH_URL=ws://127.0.0.1:8800 KEYWARD_PAIRING_TOKEN=pt_go cargo run -- executor
```

- `ServeOne(addr, cfg)` accepts one Executor, authenticates it (pairing token +
  identity + optional `AuthorizedExecutors` allow-list), and pairs (root→operational
  key chain).
- `(*Session).Submit(provider, request)` sends a work intent and returns a channel of
  native `Event`s (`Chunk` / `Done` / `Error`).
- For zero-code-change integration of an *existing* app, use `keyward proxy` instead.

Depends only on `github.com/coder/websocket` + the Go standard library
(`crypto/ed25519`). See the [protocol spec](../../docs/spec.md).
