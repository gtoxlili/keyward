# Integrate Keyward — for app builders

🌐 **English** · [中文](../zh/integration.md)

> You're building the app / SaaS — the **Orchestrator**. You hold no key. Your job:
> issue a pairing token, accept the Executor's dial-out, send work intents, and relay
> the streamed result to your user. New to the model? Read the [docs index](./README.md).

## Today vs. roadmap

**Today, zero code change:** run the **OpenAI-compatible proxy** — `keyward proxy`
(build with `--features proxy`). It exposes an OpenAI-style HTTP endpoint backed by
the paired Executor, so any existing app integrates by pointing its base URL at it:

```sh
keyward proxy   # waits for an executor to pair, then serves http://127.0.0.1:8088
# in your app:  OPENAI_BASE_URL=http://127.0.0.1:8088/v1   OPENAI_API_KEY=anything
```

`/v1/chat/completions`, `/v1/responses`, and `/v1/messages` are routed to the
matching dialect; streaming is relayed verbatim, so your existing OpenAI SDK parses
it natively. The key stays on the Executor; the app's `OPENAI_API_KEY` is ignored.

**Today, embedded:** use the **Orchestrator SDK** to run the client in-process —
[`keyward-sdk`](../../crates/keyward-sdk) for Rust, [`sdk/go`](../../sdk/go) for Go.
Both: bind a listener, `serve_one` / `ServeOne` to pair an Executor, then submit work
intents and stream native events back. (The Go SDK is byte-compatible with the Rust
Executor — cross-verified in CI.)

**Transport — WebSocket or gRPC:** the protocol is transport-agnostic (spec §1). The
Rust SDK serves either — `serve_one` (WebSocket) or `serve_one_grpc` (gRPC, built with
`--features grpc`) — and the Executor picks by URL scheme (`ws://` / `wss://` vs
`grpc://` / `grpcs://`). The Executor stays the dialing side on gRPC too, so it still
needs no inbound ports. Everything above the channel is identical.

**Today, deeper:** integrate the `v0` wire protocol directly over a WebSocket or a gRPC
bidi stream — the whole contract is in [spec.md](../spec.md), and `keyward orchestrator`
is a working reference.

## The message flow

One pairing, then any number of work intents over the same session:

```
Executor (user side)                          Orchestrator (your app)
   │ ── hello (pairing_token, providers) ───────▶ │  verify token
   │ ◀── paired (root_pubkey, op cert, sig) ───── │  prove identity, sign sid
   │  pin root, verify chain                       │
   │ ◀── work (provider, native request) ──────── │  send an LLM call, no key
   │  check policy ✓, inject key, call provider    │
   │ ── work_chunk (seq, native delta) ─────────▶ │  relay to your user
   │ ── work_done (usage) ──────────────────────▶ │
```

- `work.request` is the provider's **native** body, minus any credential — `messages`
  for OpenAI Chat Completions, `input` for the Responses API, the Anthropic Messages
  shape for `anthropic`. The Executor passes it through and you get native chunks
  back, so your existing provider-SDK parsing keeps working.
- The credential lives only in the Executor — you never send one and never receive one.
- A dropped channel **suspends**, it doesn't fail: reconnect and `resume { mid, last_seq }`
  to replay the chunks you missed; send `cancel { mid }` to deliberately abort.

## Pairing UX

Generate a **single-use, short-lived** pairing token and show it to your user the
WalletConnect way — a code to paste or (roadmap) a QR to scan. Authenticate yourself
with a long-lived **root identity key**; the Executor pins it on first contact, so key
rotation / autoscaling across reconnects needs no re-pairing. Show your root key's
fingerprint so the user can confirm it out of band.

## Controlling who can bind (protecting your side)

You can also **authenticate the Executor**, so only your registered users may bind.
Each Executor has a stable identity key; the user runs `keyward identity` to get
their pubkey and registers it with you at sign-up. You then admit only that
allow-list: every `hello` carries the Executor's `pubkey` and a signature over the
pairing token, and you reject any that isn't authorized.

This protects *your* interests (who may use your app) without touching the Owner's —
it's strictly a "who may bind" gate. It does **not** hide prompts or keys from the
user: in BYOK the Owner runs the Executor, so they can always inspect their own
traffic (that's the point), and whoever attaches the credential to the provider call
necessarily sees that call. If you need to hide payloads *from the user*, BYOK is the
wrong model — that requires server-side / TEE execution.

---

Try it locally: [a full walkthrough](./walkthrough.md) · Read the wire format:
[spec.md](../spec.md) · Back to the [docs index](./README.md).
