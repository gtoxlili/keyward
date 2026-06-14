# Integrate Keyward — for app builders

🌐 **English** · [中文](../zh/integration.md)

> Most apps need **zero integration**: a user just points the app's OpenAI base URL at a
> **Node**, and Keyward does the rest (that's the [user guide](./users.md)). This guide is for
> when you want to *run or embed* a Node yourself — you hold no key either way. New to the
> model? Read the [docs index](./README.md).

## Today vs. roadmap

**Today, zero code change (the common case):** run a **Node** — `keyward node` (build with
`--features node`). It's an OpenAI-style HTTP endpoint backed by the paired Client, so any
existing — and entirely unaware — app integrates by pointing its base URL at it:

```sh
KEYWARD_SINGLE_TENANT=1 keyward node   # one Client, no token; waits for it to pair, serves :8088
# in your app:  OPENAI_BASE_URL=http://127.0.0.1:8088/v1   OPENAI_API_KEY=anything
# (multi-tenant — drop SINGLE_TENANT — and the app's "API key" becomes each user's routing token)
```

`/v1/chat/completions`, `/v1/responses`, and `/v1/messages` are routed to the
matching dialect; streaming is relayed verbatim, so your existing OpenAI SDK parses
it natively. The key stays on the Client; the app's `OPENAI_API_KEY` is ignored.

**Today, embedded:** use the **Node SDK** to embed a Node in-process —
[`keyward-sdk`](../../crates/keyward-sdk) for Rust, [`sdk/go`](../../sdk/go) for Go.
Both: bind a listener, `serve_one` / `ServeOne` to pair a Client, then submit work
intents and stream native events back. (The Go SDK is byte-compatible with the Rust
Client — cross-verified in CI.)

**Transport — WebSocket or gRPC:** the protocol is transport-agnostic (spec §1). The
Rust SDK serves either — `serve_one` (WebSocket) or `serve_one_grpc` (gRPC, built with
`--features grpc`) — and the Client picks by URL scheme (`ws://` / `wss://` vs
`grpc://` / `grpcs://`). The Client stays the dialing side on gRPC too, so it still
needs no inbound ports. Everything above the channel is identical.

**Today, deeper:** integrate the `v0` wire protocol directly over a WebSocket or a gRPC
bidi stream — the whole contract is in [spec.md](../spec.md), and `keyward node`
is a working reference.

## The message flow

One pairing, then any number of work intents over the same session:

```
Client (user side)                          Node (the rendezvous)
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
  shape for `anthropic`. The Client passes it through and you get native chunks
  back, so your existing provider-SDK parsing keeps working.
- The credential lives only in the Client — you never send one and never receive one.
- A dropped channel **suspends**, it doesn't fail: reconnect and `resume { mid, last_seq }`
  to replay the chunks you missed; send `cancel { mid }` to deliberately abort.

## Pairing UX

Generate a **single-use, short-lived** pairing token and show it to your user the
WalletConnect way — a code to paste or (roadmap) a QR to scan. Authenticate yourself
with a long-lived **root identity key**; the Client pins it on first contact, so key
rotation / autoscaling across reconnects needs no re-pairing. Show your root key's
fingerprint so the user can confirm it out of band.

## Controlling who can bind (protecting your side)

You can also **authenticate the Client**, so only your registered users may bind.
Each Client has a stable identity key; the user runs `keyward identity` to get
their pubkey and registers it with you at sign-up. You then admit only that
allow-list: every `hello` carries the Client's `pubkey` and a signature over the
pairing token, and you reject any that isn't authorized.

This protects *your* interests (who may use your app) without touching the Owner's —
it's strictly a "who may bind" gate. It does **not** hide prompts or keys from the
user: in BYOK the Owner runs the Client, so they can always inspect their own
traffic (that's the point), and whoever attaches the credential to the provider call
necessarily sees that call. If you need to hide payloads *from the user*, BYOK is the
wrong model — that requires server-side / TEE execution.

## Deploy with Docker

The repo ships a [`Dockerfile`](../../Dockerfile) — one image, several server roles
(built with the node role, both provider dialects, and gRPC). The default command is `node`:

```sh
docker build -t keyward .
# OpenAI-compatible gateway: :8088 is the HTTP front for your app, :8787 is where the
# Owner's client dials in. Both bind 0.0.0.0 inside the container.
docker run -p 8088:8088 -p 8787:8787 keyward   # multi-tenant by default
#   in your app:  OPENAI_BASE_URL=http://<host>:8088/v1  OPENAI_API_KEY=<the Client's routing token>
#   (or add -e KEYWARD_SINGLE_TENANT=1 for a personal one-Client node, where any key works)
```

Override the command for the Client role — an
always-on Client on the Owner's box with the key as an env secret:

```sh
docker run -e KEYWARD_NODE_URL=grpc://node.example.com:443 \
           -e KEYWARD_PAIRING_TOKEN=pt_... -e OPENAI_API_KEY=sk-... keyward client
```

It runs as a non-root user and needs no build-time secrets. Tune the listen addresses
with `KEYWARD_HTTP_LISTEN` / `KEYWARD_LISTEN`.

---

Try it locally: [a full walkthrough](./walkthrough.md) · Read the wire format:
[spec.md](../spec.md) · Back to the [docs index](./README.md).
