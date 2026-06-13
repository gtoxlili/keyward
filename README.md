# Keyward

> Route the work to the key, never the key to the work.

Most things that call themselves "BYOK" aren't, really. You paste your API key into someone's
dashboard and from that moment it lives on their server. You're trusting their ops, their logs,
their interns, and every breach they'll ever have — with a credential that spends your money and
acts as you.

Keyward is an attempt to do BYOK the way it should have worked from the start: **the key never
leaves your side.** The app sends work to where your key lives; your key never travels to the app.

If you've used WalletConnect, this will feel familiar. The dApp asks, your wallet signs, the dApp
never sees your private key. Keyward is that idea, for API keys.

**Status:** early, `v0`, unstable — but real enough to run. The wire protocol is in
**[SPEC.md](./docs/spec.md)**, and there's a working Rust reference Executor:
`cargo run -- demo` takes a call end to end (dial-out pairing, a pinned root→operational key chain,
policy enforced before the provider is touched, native streaming relayed and metered), and
`cargo run -- resume-demo` drops the channel mid-stream and resumes. See
**[IMPLEMENTATION.md](./docs/implementation.md)** for what's real and what's still stubbed. Treat every
detail as provisional until `v1`; issues and pushback welcome.

Want to **use** it (bring your own key) or **integrate** it (build on it as an app)? Start with the
**[docs](./docs/)** (English & 中文) — a guide [for users](./docs/en/users.md) and one
[for app builders](./docs/en/integration.md).

## Why bother

The thing that finally convinced me this needs to be a protocol and not just "be careful with keys"
is that the problem is structural, not a matter of anyone being careless:

A provider API key is a static bearer token. To make an authenticated call, *something* has to hold
the plaintext at the moment of the call. There's no clever crypto that gets around that — you can't
present half a bearer token. So the only question that actually matters is **what is that
something, and can you check it yourself.**

Custodial BYOK answers "it's our server, trust us." Every variation — paste-the-key SaaS, a
LiteLLM-style gateway you self-host, a TEE broker — is just a different answer to *who holds it*.
Keyward's answer is: an executor on *your* side holds it, and you can watch it to be sure.

## The shape

Split the system into a brain and a pair of hands.

- **Orchestrator** — the app. It decides *what* to do: drives the agent loop, builds prompts, keeps
  state. It holds no key, ever. (The "brain", or the "dApp" if you like the wallet analogy.)
- **Executor** — a small thing *you* run, inside your own trust boundary. It holds the key, enforces
  your limits, and makes the actual call to the provider. (The "hands", or the "wallet".)
- **Provider** — OpenAI, Anthropic, whoever you're paying.

The brain sends a *work intent* to the hands. The hands attach the key locally, call the provider,
and stream the result back. That's the whole trick.

## One call, end to end

```
Owner                Executor                 Orchestrator              Provider
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

1. You run an Executor and give it your key as a local secret. It **dials out** to the Orchestrator
   and pairs — think scanning a WalletConnect QR. Dialing out means no open ports and nothing
   public on your side: it's an outbound connection the Orchestrator pushes work down (a WebSocket
   or a gRPC stream), not a published port. The protocol is transport-agnostic, but it's a
   *dial-out app connection*, not a tunnel appliance — `frp`/ngrok/Cloudflare Tunnel are built to
   expose an inbound listener, which is the opposite shape (see [SPEC §1](./docs/spec.md)).
2. When the Orchestrator wants an LLM call, it sends a work intent over that session: model,
   messages, tools, params. No key — it doesn't have one to send.
3. The Executor checks the intent against the limits *you* set (allowed models, budget, rate, which
   app), injects the key, and calls the provider directly.
4. It streams the response back over the session.
5. You can inspect, throttle, or kill the connection at the Executor whenever you want. The key's
   bytes never pass through the Orchestrator.

## What you can actually verify

The claim is deliberately narrow: **the Orchestrator never has your key.** Not in memory, not in a
log, not in transit, and not sitting in a database waiting to leak.

The reason I care about the word *verify* is that you don't have to take my word for any of it:

- The key shows up in exactly one place — the Executor's outbound call to the provider. Point a
  proxy at the Executor and confirm the key never appears in anything going to the Orchestrator.
- The Executor is meant to be open source and reproducibly built, so "this binary is that source"
  is something you check, not assume.
- It runs on a box you own, so nobody else can read its memory or its secret store.

A custodial proxy can only ever promise "we don't log it." A TEE broker can offer "trust the silicon
and the attestation paperwork." Keyward's pitch is just: look — it never left your side.

## Running the Executor

The only real requirement is that the Executor is reachable while there's work to do.

For interactive use — you're sitting there watching the agent — a local process or even something
running in the browser tab is fine. When you close the tab, the work stops, and that's usually what
you want anyway.

For autonomous use — the agent grinds away while you're asleep — it needs to be always-on. The part
people miss is that **"always-on" doesn't have to mean "on the app's servers."** Deploy the Executor
to your *own* serverless (a Cloudflare Worker, a Lambda, Deno Deploy) with the key as a secret in
*your* account, or to a cheap VPS you own. Always available, but the key sits in infrastructure the
app provably can't read.

## Things people will (rightly) ask

**Isn't this just LiteLLM / a proxy with extra steps?**
No, and this is the one distinction worth being pedantic about. A gateway *holds* your key and
forwards calls for you — it's custodial, you're back to trusting a server. Keyward's Orchestrator
holds nothing and literally cannot make a call without a live Executor on your side. Different
category, not a nicer proxy.

**Why not just use a TEE / enclave?**
A TEE keeps the key on the *operator's* hardware and asks you to trust attestation and the absence
of side-channels. That's strong, but it's not something a normal user can check, and SGX has been
broken more than once. Keyward keeps the key on *your* hardware so there's nothing to attest.

**Does the app still see my prompts?**
Yes. Keyward protects the *credential*, not the *payload* — the Orchestrator is the thing building
and reading the prompts, so of course it sees them. Hiding content from the app too is a different,
harder problem and explicitly out of scope here. I'd rather do one thing honestly.

**A malicious app can still burn my budget within the limits, right?**
Right. Custody isn't the same as control. That's exactly why limits — model allowlists, budget
caps, rate limits, per-app scoping, an audit log — live in the Executor and are part of the
protocol, not an afterthought. Custody stops the key from leaking; policy stops it from being
abused.

## How it compares

|                                   | Paste-the-key SaaS | Self-hosted gateway | TEE broker        | Keyward         |
| --------------------------------- | ------------------ | ------------------- | ----------------- | --------------- |
| App can read the raw key          | yes                | n/a (you're host)   | no, if HW holds   | **never**       |
| What you're trusting              | a privacy policy   | yourself            | CPU vendor + attestation | **code you can read** |
| You can verify it yourself        | no                 | —                   | hard              | **yes**         |
| Works for offline/autonomous runs | yes                | yes                 | yes               | yes, on your own always-on box |
| Infra you have to run             | none               | the whole gateway   | none              | a small Executor |

## Roadmap

- [x] `v0` spec — wire format for pairing, the work intent, streaming frames, and the policy object.
      Drafted in [SPEC.md](./docs/spec.md); pairing auth, resumption, and budget pricing are now resolved.
- [~] Reference Executor — open source, reproducible; a local binary plus one-click serverless
      templates. A Rust **walking skeleton** runs end to end today — see
      [IMPLEMENTATION.md](./docs/implementation.md). Serverless templates and reproducible-build pipeline next.
- [ ] Orchestrator SDK — ideally you integrate by swapping your provider client for one line.
- [~] Transport adapters — outbound **WebSocket** first (done in the skeleton), then a gRPC bidi
      stream; the protocol is transport-agnostic. (Tunnel appliances are the wrong shape — see SPEC §1.)
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
