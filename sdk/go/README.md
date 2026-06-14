# keyward (Go)

The Keyward **Node SDK** for Go — embed a Node in-process. You bind a listener the
Owner's **Client** dials into, pair, then submit work intents and stream the provider's
native response back. Your code decides the requests; the key stays on the Client and
never reaches you.

This is the in-process path. The **zero-integration** path needs no SDK at all: an
unaware app just points its OpenAI base URL at a standalone `keyward node` (with a
routing token as its API key). Reach for this SDK when you want the Node logic *inside*
your own Go process.

The wire + crypto are byte-compatible with the Rust reference Client (verified
cross-language in CI / by the example below).

```go
import keyward "github.com/gtoxlili/keyward/sdk/go"

cfg := keyward.NewConfig("my-app", "node_myapp", "pt_one_time_token")
fmt.Println("show this to the user out of band — root fp", cfg.RootFingerprint())

session, err := keyward.ServeOne("127.0.0.1:8787", cfg) // waits for a client to pair
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

Run the example against the real Rust Client:

```sh
go run ./example
# then, in another shell (provider "mock" needs no key):
KEYWARD_NODE_URL=ws://127.0.0.1:8800 KEYWARD_PAIRING_TOKEN=pt_go cargo run -- client
```

- `ServeOne(addr, cfg)` accepts one Client, authenticates it (pairing token +
  identity + optional `AuthorizedClients` allow-list), and pairs (root→operational
  key chain).
- `(*Session).Submit(provider, request)` sends a work intent and returns a channel of
  native `Event`s (`Chunk` / `Done` / `Error`).
- For zero-code-change integration of an *existing, unaware* app, skip the SDK and run
  a standalone `keyward node` — the app just sets its OpenAI base URL + API-key token.

Depends only on `github.com/coder/websocket` + the Go standard library
(`crypto/ed25519`). See the [protocol spec](../../docs/spec.md).
