# Reference implementation — `v0` skeleton

> **Status: walking skeleton.** It runs end to end and exercises the load-bearing
> ideas — dial-out pairing, a pinned/verified Orchestrator identity, policy
> enforcement before the provider is touched, native-body passthrough, streamed
> relay with per-intent sequence numbers, usage metering, and survive-a-dropped-
> channel resumption (plus explicit cancel). Several pieces are deliberately
> stubbed (listed at the bottom). This is the thing to argue with and build on,
> not to ship.

## Layout

```
crates/
  keyward-proto/   wire types (envelope, messages) + the policy engine (§6). No async,
                   no HTTP, no crypto — compiles to every target unchanged.
  keyward/         the reference Executor + a mock Orchestrator for the demo.
    src/wire.rs            Frame ↔ WebSocket, hex/fingerprint/digest helpers
    src/provider/mod.rs       provider adapters; the `mock` provider (no key, no net),
                              dialect chosen by model family
    src/provider/openai.rs    real OpenAI Chat-Completions adapter (feature = "openai")
    src/provider/anthropic.rs Anthropic Messages adapter + a tested usage accumulator
                              (feature = "anthropic")
    src/pricing.rs         budget cost from usage × vendored LiteLLM prices (data/)
    src/executor.rs        dial out, pin+verify orchestrator key, enforce policy, relay;
                           per-intent ring buffer + reconnect/resume/cancel (§7)
    src/orchestrator.rs    mock app: issues pairing token, signs sid, drives intents
    src/demo.rs            wires both ends over a localhost WS; `demo` runs three intents,
                           `resume-demo` drops the channel mid-stream and resumes
```

## Run the demo (no key, no network)

```sh
cargo run -- demo
```

You'll see: the Executor dial out; the Orchestrator sign the freshly-assigned
`sid`; the Executor **TOFU-pin** that identity key (fingerprints match); a
`gpt-4o` intent (OpenAI dialect) and a `claude-3-5-sonnet` intent (Anthropic
dialect) each stream back in sequenced native chunks with usage metered the way
that dialect reports it; and a `gpt-4-turbo` intent **rejected with
`policy_model`** before the provider is contacted.

## Run the resume / cancel demo (§7)

```sh
cargo run -- resume-demo
```

The Orchestrator streams an intent, reads a few chunks, then **drops the socket
mid-stream**. The Executor's producer keeps pulling from the provider into a
ring buffer while the channel is down; the Executor re-dials, re-pairs (the
pinned key still matches — no second TOFU), and on `resume` **replays exactly the
chunks the Orchestrator missed**, then finishes. A second intent is then
**cancelled** part-way, showing the other half of §7: a dropped channel suspends,
an explicit `cancel` aborts.

## Run the two ends separately

```sh
# terminal 1 — the app (holds no key)
cargo run -- orchestrator                      # prints a pairing token + the exact executor command

# terminal 2 — the executor (holds the key), using the token printed above
KEYWARD_ORCH_URL=ws://127.0.0.1:8787 \
KEYWARD_PAIRING_TOKEN=pt_dev_token \
cargo run -- executor
```

## Verify the core promise yourself (the proxy recipe)

This is the check that actually substantiates "the Orchestrator never has your
key": point the Executor's provider calls at a proxy and confirm the credential
appears **only** on the call to the provider, never on the channel to the
Orchestrator.

```sh
# 1. start mitmproxy (or any logging proxy) on :8080
mitmproxy -p 8080

# 2. run the real adapter, sending provider traffic through the proxy
cargo run --features openai -- executor   # with OPENAI_API_KEY set
#   OPENAI_BASE_URL is honored, so:
OPENAI_BASE_URL=http://127.0.0.1:8080/v1 OPENAI_API_KEY=sk-… \
KEYWARD_ORCH_URL=ws://… KEYWARD_PAIRING_TOKEN=… \
cargo run --features openai -- executor
```

In the proxy you will see the `Authorization: Bearer sk-…` header **only** on the
request to the provider. Capture the WebSocket to the Orchestrator in parallel and
confirm the key is absent there. (Honest limit: a proxy shows *which endpoint* the
key goes to, not that a compromised binary couldn't exfiltrate it some other way —
that's what reproducible builds + signed provenance are for. See the README's
"What you can actually verify".)

## What's real vs. what I faked

Enough is real to believe the shape: the dial-out WS transport, pairing with a
one-time token, the Ed25519 orchestrator identity (signed `sid`, TOFU pin, and a
refusal if the key changes on reconnect), the policy engine with the §6 ordering
and trailing-`*` globs, native-body passthrough in two dialects (OpenAI Chat
Completions and Anthropic Messages), the streamed relay with a per-intent `seq`,
and usage metered into budget spend the way each dialect reports it. Resumption is
real too: each intent's producer is decoupled from the connection and buffers into
a bounded ring, so a dropped channel suspends rather than fails — the Executor
re-dials, re-pairs against the pinned key, and replays from `resume`'s `last_seq`;
`cancel` aborts. The real adapters are there too (OpenAI forces
`stream_options.include_usage`; Anthropic reads the split/cumulative usage without
double-counting cache tokens; each attaches the key at one call site and honours
its `*_BASE_URL`) — the demo just uses mocks so it needs no key.

The rest is stubbed, roughly in the order I'd reach for next:
- **Channel E2E crypto (Noise).** The reference channel is plain WSS to the
  Orchestrator; the Noise inner layer (for an untrusted relay) isn't wired yet.
- **Root-key + chained op-keys.** Pinning is a single key today; the SSH-CA-style
  root→operational-key chain (so SaaS rotation/autoscale needs no re-pair) is
  designed in SPEC §3/§9 but not implemented.
- **Resume auth + single-use tokens.** Reconnect re-pairs with the *same* token
  (the skeleton relaxes single-use); resume isn't yet bound by a fresh signature.
- **Secret storage.** The CLI reads the key from an env var; OS-keychain
  (`keyring`) + `mlock`/zeroize hardening is not in yet.
- **Byte-reproducible builds.** CI (fmt/clippy/test) and a release workflow that
  publishes checksums + a signed SLSA build-provenance attestation are in
  `.github/workflows/`; the pinned-container build that makes the binary
  *bit-for-bit* reproducible is still TODO.
