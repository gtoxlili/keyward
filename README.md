<div align="center">

<img src="./assets/logo.png" alt="Keyward" width="140" />

# Keyward

**Route the work to the key, never the key to the work.**

A non-custodial **BYOK** protocol — your API key never leaves your side.

[![CI](https://github.com/gtoxlili/keyward/actions/workflows/ci.yml/badge.svg)](https://github.com/gtoxlili/keyward/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](./LICENSE)
[![Status](https://img.shields.io/badge/status-incubating%20·%20v0-orange.svg)](#roadmap)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust-dea584.svg?logo=rust&logoColor=white)

[Docs](./docs/) · [Spec](./docs/spec.md) · [For users](./docs/en/users.md) · [For app builders](./docs/en/integration.md)

</div>

---

Most things that call themselves "BYOK" aren't, really. You paste your API key into someone's
dashboard and from that moment it lives on their server. You're trusting their ops, their logs,
their interns, and every breach they'll ever have — with a credential that spends your money and
acts as you.

Keyward is an attempt to do BYOK the way it should have worked from the start: **the key never
leaves your side.** The app sends work to where your key lives; your key never travels to the app.

If you've used WalletConnect, this will feel familiar. The dApp asks, your wallet signs, the dApp
never sees your private key. Keyward is that idea, for API keys.

> **Status:** `v0`, unstable, and incubating — but real enough to run. The wire protocol is drafted
> ([spec](./docs/spec.md)) and there's a working Rust reference Client; see
> [what's real and what's still stubbed](./docs/implementation.md). Treat every detail as provisional
> until `v1`. Issues and pushback welcome.

## Quick start

```sh
git clone https://github.com/gtoxlili/keyward && cd keyward
cargo run -- demo          # watch the whole chain run with a mock provider — no key, no network
cargo run -- resume-demo   # drop the channel mid-stream and resume from where it left off
```

Then bring your own key or integrate it into an app — start with the **[docs](./docs/)** (English &
中文): [for users](./docs/en/users.md) · [for app builders](./docs/en/integration.md).

## Why bother

The thing that finally convinced me this needs to be a protocol and not just "be careful with keys"
is that the problem is structural, not a matter of anyone being careless:

A provider API key is a static bearer token. To make an authenticated call, *something* has to hold
the plaintext at the moment of the call. There's no clever crypto that gets around that — you can't
present half a bearer token. So the only question that actually matters is **what is that
something, and can you check it yourself.**

Custodial BYOK answers "it's our server, trust us." Every variation — paste-the-key SaaS, a
LiteLLM-style gateway you self-host, a TEE broker — is just a different answer to *who holds it*.
Keyward's answer is: a Client on *your* side holds it, and you can watch it to be sure.

## The shape

Keyward pulls the key-holding out of the app and splits it across two cooperating pieces — and
the app doesn't even have to know it happened.

- **Client** — a small thing *you* run, inside your own trust boundary. It holds the key, enforces
  your limits, and makes the actual call to the provider. (The "hands", or the "wallet".)
- **Node** — a rendezvous the Client dials into and the app reaches. It holds no key, ever; it
  just routes each request to the right Client. (The WalletConnect *relay*, if you like the
  analogy — neutral, and it can even be a shared/public station.)
- **App** — whatever's driving: it decides *what* to do (the agent loop, the prompts) and holds no
  key. It can stay **completely unaware** of Keyward — it just points its OpenAI base URL at a Node
  and uses a routing token as its "API key". (Building the app yourself? Embed the Node in-process
  with the SDK instead.)
- **Provider** — OpenAI, Anthropic, whoever you're paying.

The app makes an ordinary request; the Node relays it to your Client as a *work intent* (no key);
the Client attaches the key locally, calls the provider, and streams the result back. That's the
whole trick — and the app is none the wiser.

## One call, end to end

```
Owner                Client                 Node              Provider
 │  start, bind key      │                          │                       │
 │ ───────────────────▶  │                          │                       │
 │                       │  dial out + pair          │                       │
 │                       │ ───────────────────────▶  │  reverse connection:  │
 │                       │                          │  no inbound port on    │
 │                       │                          │  your side, NAT-fine   │
 │                       │   ◀──── work intent ──────│  model, messages,     │
 │                       │         (no key)          │  tools, params        │
 │                       │  check policy ✓            │                       │
 │                       │  inject key locally        │                       │
 │                       │ ─────────────────────────────────────────────▶   │
 │                       │  ◀───────────── stream ───────────────────────────│
 │                       │   ──── relayed back ────▶ │                       │
 │  revoke / inspect     │                          │                       │
 │ ◀───────────────────▶ │                          │                       │
```

1. You run a Client and give it your key as a local secret. It **dials out** to the Node
   and pairs — think scanning a WalletConnect QR. Dialing out means no open ports and nothing
   public on your side: it's an outbound connection the Node pushes work down (a WebSocket
   or a gRPC stream), not a published port. The protocol is transport-agnostic, but it's a
   *dial-out app connection*, not a tunnel appliance — `frp`/ngrok/Cloudflare Tunnel are built to
   expose an inbound listener, which is the opposite shape (see [SPEC §1](./docs/spec.md)).
2. The (unaware) app makes an ordinary request to the Node; the Node relays it down that session
   as a work intent: model, messages, tools, params. No key — the Node doesn't have one to send.
3. The Client checks the intent against the limits *you* set (allowed models, budget, rate, which
   app), injects the key, and calls the provider directly.
4. It streams the response back over the session.
5. You can inspect, throttle, or kill the connection at the Client whenever you want. The key's
   bytes never pass through the Node.

## What you can actually verify

The claim is deliberately narrow: **the Node never has your key.** Not in memory, not in a
log, not in transit, and not sitting in a database waiting to leak.

The reason I care about the word *verify* is that you don't have to take my word for any of it:

- The key shows up in exactly one place — the Client's outbound call to the provider. Point a
  proxy at the Client and confirm the key never appears in anything going to the Node.
- The Client is meant to be open source and reproducibly built, so "this binary is that source"
  is something you check, not assume.
- It runs on a box you own, so nobody else can read its memory or its secret store.

A custodial proxy can only ever promise "we don't log it." A TEE broker can offer "trust the silicon
and the attestation paperwork." Keyward's pitch is just: look — it never left your side.

## Running the Client

The only real requirement is that the Client is reachable while there's work to do.

For interactive use — you're sitting there watching the agent — a local process or even something
running in the browser tab is fine. When you close the tab, the work stops, and that's usually what
you want anyway. There's also a **[desktop app](apps/executor-desktop)** (Tauri, bilingual EN/中文):
pair with a node, keep provider keys in your OS keychain, set policy, and watch a live
dashboard — driving the same client core.

For autonomous use — the agent grinds away while you're asleep — it needs to be always-on. The part
people miss is that **"always-on" doesn't have to mean "on the app's servers."** Deploy the Client
to your *own* serverless (a Cloudflare Worker, a Lambda, Deno Deploy) with the key as a secret in
*your* account, or to a cheap VPS you own. Always available, but the key sits in infrastructure the
app provably can't read.

## Things people will (rightly) ask

**Isn't this just LiteLLM / a proxy with extra steps?**
No, and this is the one distinction worth being pedantic about. A gateway *holds* your key and
forwards calls for you — it's custodial, you're back to trusting a server. Keyward's Node
holds nothing and literally cannot make a call without a live Client on your side. Different
category, not a nicer proxy.

**Why not just use a TEE / enclave?**
A TEE keeps the key on the *operator's* hardware and asks you to trust attestation and the absence
of side-channels. That's strong, but it's not something a normal user can check, and SGX has been
broken more than once. Keyward keeps the key on *your* hardware so there's nothing to attest.

**Does the app still see my prompts?**
Yes. Keyward protects the *credential*, not the *payload* — the app is the thing building and
reading the prompts, so of course it sees them. (A *blind* Node that relays only ciphertext is a
separate mode, [SPEC §10](./docs/spec.md).) Hiding content from the app itself is a different,
harder problem and explicitly out of scope here. I'd rather do one thing honestly.

**A malicious app can still burn my budget within the limits, right?**
Right. Custody isn't the same as control. That's exactly why limits — model allowlists, budget
caps, rate limits, per-app scoping, an audit log — live in the Client and are part of the
protocol, not an afterthought. Custody stops the key from leaking; policy stops it from being
abused.

## How it compares

|                                   | Paste-the-key SaaS | Self-hosted gateway | TEE broker        | Keyward         |
| --------------------------------- | ------------------ | ------------------- | ----------------- | --------------- |
| App can read the raw key          | yes                | n/a (you're host)   | no, if HW holds   | **never**       |
| What you're trusting              | a privacy policy   | yourself            | CPU vendor + attestation | **code you can read** |
| You can verify it yourself        | no                 | —                   | hard              | **yes**         |
| Works for offline/autonomous runs | yes                | yes                 | yes               | yes, on your own always-on box |
| Infra you have to run             | none               | the whole gateway   | none              | a small Client |

## Roadmap

The single source of truth for what's built and what's left is **[docs/roadmap.md](./docs/roadmap.md)**.
The high-level shape:

- [x] `v0` spec — wire format for pairing, the work intent, streaming frames, and the policy object.
      Drafted in [SPEC.md](./docs/spec.md); pairing auth, resumption, and budget pricing are now resolved.
- [~] Reference Client — open source, reproducible; a local binary plus one-click serverless
      templates. A Rust **walking skeleton** runs end to end today — see
      [IMPLEMENTATION.md](./docs/implementation.md). Serverless templates and reproducible-build pipeline next.
- [x] Node SDK — Rust (`keyward-sdk`) and Go (`sdk/go`) clients, plus a zero-code-change
      OpenAI-compatible node (`keyward node`) so an app integrates by pointing `OPENAI_BASE_URL` at it.
- [x] Transport adapters — outbound **WebSocket** and a **gRPC** bidi stream (scheme-selected, same
      envelope, the Client stays the dialing client); the protocol is transport-agnostic. (Tunnel
      appliances are the wrong shape — see SPEC §1.)
- [~] Provider adapters — OpenAI Chat-Completions, the OpenAI Responses API, and Anthropic Messages
      all land in the skeleton (Chat-Completions covers OpenAI-compatible providers for free);
      Gemini and so on next.
- [ ] Channel E2E crypto — a Noise inner layer for the untrusted-relay case (SPEC §9).
- [ ] A conformance suite, once there's more than one implementation to keep honest.

## Contributing

It's early, so the most useful thing right now is to argue with [SPEC.md](./docs/spec.md) — especially
its open questions. Prose issues are fine; you don't need a PR. See [CONTRIBUTING.md](./CONTRIBUTING.md).

If you find a hole in the core promise — any way the app could still get hold of the key — please
treat it as a security issue and report it privately ([SECURITY.md](./SECURITY.md)).

## License

[Apache-2.0](./LICENSE). The patent grant matters for a protocol you actually want people to adopt.
