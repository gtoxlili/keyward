# Reference implementation — `v0` skeleton

> **Status: walking skeleton.** It runs end to end and exercises the load-bearing
> ideas — dial-out pairing, a pinned/verified Orchestrator identity, policy
> enforcement before the provider is touched, native-body passthrough, streamed
> relay with per-intent sequence numbers, and usage metering. Several pieces are
> deliberately stubbed (listed at the bottom). This is the thing to argue with and
> build on, not to ship.

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
    src/pricing.rs         budget cost from usage (stand-in table; see §6 note)
    src/executor.rs        dial out, pin+verify orchestrator key, enforce policy, relay
    src/orchestrator.rs    mock app: issues pairing token, signs sid, drives intents
    src/demo.rs            wires both ends over a localhost WS and runs two intents
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
and usage metered into budget spend the way each dialect reports it. The real
adapters are there too (OpenAI forces `stream_options.include_usage`; Anthropic
reads the split/cumulative usage without double-counting cache tokens; each
attaches the key at one call site and honours its `*_BASE_URL`) — the demo just
uses mocks so it needs no key.

The rest is stubbed, roughly in the order I'd reach for next:
- **Channel E2E crypto (Noise).** The reference channel is plain WSS to the
  Orchestrator; the Noise inner layer (for an untrusted relay) isn't wired yet.
- **Root-key + chained op-keys.** Pinning is a single key today; the SSH-CA-style
  root→operational-key chain (so SaaS rotation/autoscale needs no re-pair) is
  designed in SPEC §3/§9 but not implemented.
- **Resumption.** `seq` is emitted and gap-detectable, but the ring buffer +
  `resume`/`cancel` handling (SPEC §7) isn't built; a dropped channel just ends.
- **Pricing data.** `pricing.rs` is a tiny embedded table; the real plan is to
  vendor LiteLLM's `model_prices_and_context_window.json` and refresh it (SPEC §6).
- **`policy_digest`** is an FNV stand-in, not `sha256`.
- **Secret storage.** The CLI reads the key from an env var; OS-keychain
  (`keyring`) + `mlock`/zeroize hardening is not in yet.
- **Reproducible-build pipeline** (pinned toolchain/container, SLSA provenance).
