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
    src/provider/openai.rs    real OpenAI Chat Completions + Responses API adapters
                              (feature = "openai")
    src/provider/anthropic.rs Anthropic Messages adapter + a tested usage accumulator
                              (feature = "anthropic")
    src/pricing.rs         budget cost from usage × vendored LiteLLM prices (data/)
    src/secret.rs          per-provider key resolution: OS keychain (keyring) -> env
    src/executor.rs        dial out, verify orchestrator key chain, enforce policy, relay;
                           per-intent ring buffer + reconnect/resume/cancel (§7)
    src/identity.rs        root -> operational-key chain: issue/verify op certs (§3/§9)
    src/orchestrator.rs    mock app: issues pairing token, signs sid, drives intents
    src/demo.rs            wires both ends over a localhost WS; `demo` runs three intents,
                           `resume-demo` drops the channel mid-stream and resumes
    src/e2e_tests.rs       integration tests: drive the real executor, assert on its frames
```

## Run the demo (no key, no network)

```sh
cargo run -- demo
```

You'll see: the Executor dial out; the Orchestrator present a root-delegated
operational key and sign the freshly-assigned `sid`; the Executor **TOFU-pin the
root** and verify the op key chains to it (fingerprints match); three intents in
three native dialects — `gpt-4o` Chat Completions, `claude-sonnet-4-5` Anthropic
Messages, and a `gpt-4o` Responses-API call (note `input` instead of `messages`) —
each stream back in sequenced native chunks with usage metered the way that
dialect reports it; and a `gpt-4-turbo` intent **rejected with `policy_model`**
before the provider is contacted.

## Run the resume / cancel demo (§7)

```sh
cargo run -- resume-demo
```

The Orchestrator streams an intent, reads a few chunks, then **drops the socket
mid-stream**. The Executor's producer keeps pulling from the provider into a
ring buffer while the channel is down; the Executor re-dials, re-pairs (the
pinned **root** still matches; the operational key may rotate — no second TOFU),
and on `resume` **replays exactly the chunks the Orchestrator missed**, then
finishes. A second intent is then
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
one-time token, the Ed25519 orchestrator identity as a root→operational-key chain
(the Executor pins the root TOFU, verifies each connection's root-signed op cert +
its `sid` signature, refuses a changed root or an op key the root didn't sign, and
gates work/resume/cancel on a verified pairing — so the orchestrator can rotate op
keys across reconnects without re-pairing), the policy engine with the §6 ordering
and trailing-`*` globs, native-body passthrough in three dialects (OpenAI Chat
Completions, the OpenAI Responses API, and Anthropic Messages), the streamed
relay with a per-intent `seq`,
and usage metered into budget spend the way each dialect reports it. Resumption is
real too: each intent's producer is decoupled from the connection and buffers into
a bounded ring, so a dropped channel suspends rather than fails — the Executor
re-dials, re-pairs against the pinned key, and replays from `resume`'s `last_seq`;
`cancel` aborts. The real adapters are there too (Chat Completions forces
`stream_options.include_usage`; Responses reads usage off the terminal
`response.completed`; Anthropic reads the split/cumulative usage without
double-counting cache tokens; each attaches the key at one call site and honours
its `*_BASE_URL`) — the demo just uses mocks so it needs no key. The `openai` and
`openai-responses` providers share one OpenAI credential. The real CLI resolves
each provider's credential from the OS keychain (env fallback), per
provider, so one Executor can front several accounts.

The rest is stubbed, roughly in the order I'd reach for next:
- **Channel E2E crypto (Noise).** The reference channel is plain WSS to the
  Orchestrator; the Noise inner layer (for an untrusted relay) isn't wired yet.
- **Single-use pairing tokens + OOB fingerprint.** Reconnect re-pairs with the
  *same* token (the skeleton relaxes single-use so the resume demo can re-pair);
  and nothing yet forces the out-of-band root-fingerprint confirmation that closes
  the TOFU first-contact gap (SPEC §3).
- **Executor identity.** The Orchestrator does not yet authenticate the *Executor*
  (the `hello.pubkey` field exists but isn't pinned).
- **Secret hardening beyond the keychain.** Keys resolve per provider from the OS
  keychain — native backends only (macOS Keychain, Windows Credential Manager,
  Linux kernel keyutils; no D-Bus / secret-service dependency) — with an env
  fallback, wrapped in `SecretString` (redacted Debug, zeroized on drop), set via
  `keyward set-key`. The Linux kernel keyring is session-scoped (doesn't survive a
  reboot), so headless hosts may prefer the env fallback. Still TODO:
  `mlock`/`setrlimit` to keep the key out of swap/core dumps, and a real
  hidden-TTY prompt (the key is read from stdin but currently echoes).
- **Byte-reproducible builds.** CI (fmt/clippy/test) and a release workflow that
  publishes checksums + a signed SLSA build-provenance attestation are in
  `.github/workflows/`; the pinned-container build that makes the binary
  *bit-for-bit* reproducible is still TODO.
