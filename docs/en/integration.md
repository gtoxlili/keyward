# Integrate Keyward — for app builders

🌐 **English** · [中文](../zh/integration.md)

> You're building the app / SaaS — the **Orchestrator**. You hold no key. Your job:
> issue a pairing token, accept the Executor's dial-out, send work intents, and relay
> the streamed result to your user. New to the model? Read the [docs index](./README.md).

## Today vs. roadmap

**Today:** integrate by speaking the `v0` wire protocol over a bidirectional channel
(a WebSocket). The whole contract is in [spec.md](../spec.md), and
`keyward orchestrator` is a working reference you can read and run.

**Roadmap:** a drop-in **Orchestrator SDK** (swap your provider client for one line)
and a local **OpenAI-compatible proxy** (point any existing app at it by changing
`OPENAI_BASE_URL`) so you integrate with near-zero code change.

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
