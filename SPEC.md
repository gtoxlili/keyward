# Keyward Protocol — v0 (Draft)

> **Status: draft, unstable.** Anything here can change until `v1`. This document specifies the
> wire-level contract between an **Executor** (holds the key, runs on the Owner's side) and an
> **Orchestrator** (the app; holds no key). The roles, motivation, and threat model live in the
> [README](./README.md) — this is just the bytes on the wire.

The keywords MUST, MUST NOT, SHOULD, MAY are used as in RFC 2119.

## 1. Transport

Keyward is transport-agnostic. It needs exactly one **bidirectional, reliable, ordered,
message-oriented** channel between Executor and Orchestrator. A WebSocket, a gRPC bidi stream, an
HTTP/2 stream, or a tunnel stood up with `frp` all qualify.

Two invariants the transport MUST hold:

1. **The Executor dials out.** The channel is established from the Owner's side (reverse
   connection). The Orchestrator MUST NOT require an inbound connection to the Owner — no open
   ports, no public endpoint there.
2. **In-order, lossless within a session.** On channel loss the session is suspended (§7), not
   silently resumed.

Messages are UTF-8 JSON objects, one per transport frame. A binary/CBOR profile may come later;
out of scope for v0. The channel MUST be encrypted (TLS or equivalent) — Keyward assumes
confidentiality and integrity from the transport.

## 2. Message envelope

Every message carries:

```json
{ "kw": "0", "type": "<message-type>", "sid": "<session-id>", "mid": "<message-id>" }
```

- `kw` — protocol major version, the string `"0"` for this draft.
- `type` — message type (§3–§7).
- `sid` — session id, assigned at pairing. Absent only on `hello`.
- `mid` — unique per message within a session (ULID/UUID). Response frames echo the `mid` of the
  intent they answer, so concurrent intents can be demultiplexed.

Receivers MUST ignore unknown fields (forward-compat) and MUST answer an unknown `type` with an
`error` of code `unsupported_type` rather than closing the channel.

## 3. Pairing

Out of band, the Owner obtains a one-time **pairing token** from the Orchestrator — a QR to scan or
a code to paste, exactly the WalletConnect gesture. The token MUST be short-lived and single-use.

**Executor → Orchestrator**

```json
{
  "kw": "0", "type": "hello", "mid": "01J...",
  "pairing_token": "pt_...",
  "executor":  { "name": "keyward-exec", "version": "0.1.0" },
  "providers": ["openai", "anthropic"],
  "policy_digest": "sha256:9f86d0..."
}
```

- `policy_digest` is a hash of the active policy (§6). It lets the Orchestrator notice that limits
  changed without the Owner having to reveal them. Sharing the full policy is OPTIONAL via a
  `policy` field — the Owner MAY keep their limits private.

**Orchestrator → Executor**

```json
{ "kw": "0", "type": "paired", "sid": "kw_sess_...", "mid": "01J...",
  "orchestrator": { "name": "acme-agent", "id": "orch_..." } }
```

After `paired` the session is open. Mutual authentication beyond the pairing token (e.g. the
Executor pinning an Orchestrator public key) is RECOMMENDED; its mechanism is deferred past v0
(§9).

## 4. Work intent

A request from the Orchestrator to perform **one** provider call.

**Orchestrator → Executor**

```json
{
  "kw": "0", "type": "work", "sid": "kw_sess_...", "mid": "01J...",
  "provider": "openai",
  "request": {
    "model": "gpt-4o",
    "messages": [{ "role": "user", "content": "…" }],
    "tools": [],
    "stream": true
  }
}
```

- `request` is the provider's **native** request body, minus any credential. There is no
  `Authorization` / `api_key` field — the Orchestrator has none to send.
- The Executor selects the endpoint and attaches the credential for `provider`.
- Streaming is requested through the provider-native field (`"stream": true` for OpenAI-shaped
  providers); the Executor reflects that mode onto the wire (§5).

The Executor MUST validate the intent against policy (§6) **before** contacting the provider. A
rejected intent yields a `work_error` with a `policy_*` code and the provider is never called.

## 5. Response frames

All response frames echo the originating `mid`.

- **`work_accepted`** *(optional, once)* — intent passed policy, provider call started.
  ```json
  { "type": "work_accepted", "mid": "01J..." }
  ```
- **`work_chunk`** *(zero or more, streaming only)*
  ```json
  { "type": "work_chunk", "mid": "01J...", "seq": 0, "delta": { /* provider-native chunk */ } }
  ```
  `seq` is monotonic per `mid`, for gap detection.
- **`work_done`** *(exactly one, terminal success)*
  ```json
  { "type": "work_done", "mid": "01J...",
    "result": { /* full provider response; MAY be omitted when already streamed */ },
    "usage":  { "input_tokens": 812, "output_tokens": 240 } }
  ```
- **`work_error`** *(terminal failure)*
  ```json
  { "type": "work_error", "mid": "01J...", "code": "provider_status",
    "message": "rate limited", "provider_status": 429 }
  ```

Exactly one of `work_done` / `work_error` terminates a given `mid`.

The Executor MUST NOT place the raw credential, the provider auth header, or anything from which
the credential can be derived, into any frame. On a provider auth failure it returns
`code: "provider_auth"` with a sanitized message — never the key, never the full upstream body if
it might echo the key.

## 6. Policy object

Owner-defined, enforced at the Executor, **not changeable by the Orchestrator**. All fields are
optional; absence means unrestricted for that dimension, though implementations SHOULD default-deny
on budget.

```json
{
  "providers": ["openai", "anthropic"],
  "models": ["gpt-4o", "claude-3-5-sonnet-*"],
  "orchestrators": ["acme-agent"],
  "budget": { "limit_usd": 20, "window": "month", "spent_usd": 7.40 },
  "rate": { "rpm": 60, "tpm": 200000 },
  "expires_at": "2026-12-31T00:00:00Z"
}
```

- `models` MAY use a trailing-`*` glob.
- `budget.spent_usd` is tracked by the Executor from reported `usage` and provider pricing; how
  pricing is sourced is implementation-defined in v0.
- **Enforcement order:** provider → model → orchestrator → expiry → rate → budget. The first failing
  check produces the matching `policy_*` error and aborts.

Custody stops the key from leaking; **policy stops it from being abused.** Both are the point.

## 7. Session lifecycle

- A session lives as long as the channel. On channel loss the Orchestrator MAY re-establish through
  the transport; whether the same `sid` resumes or a fresh pairing is required is
  transport/implementation-defined in v0.
- An in-flight `mid` whose channel dropped before its terminal frame is **failed**. The Orchestrator
  MAY re-issue under a new `mid`, but idempotency is the Orchestrator's problem — provider calls are
  not idempotent.
- Either side MAY send `{ "type": "close", "reason": "…" }`. An Owner revoking at the Executor
  surfaces as a `close` with `reason: "revoked"`, or simply as a dropped channel.

## 8. Error codes

`work_error.code` (and `error.code` for envelope-level faults):

| code | meaning | provider contacted? |
| --- | --- | --- |
| `policy_provider` | provider not in allowlist | no |
| `policy_model` | model not in allowlist | no |
| `policy_orchestrator` | this Orchestrator not allowed | no |
| `policy_expired` | policy past `expires_at` | no |
| `policy_rate` | rate limit exceeded | no |
| `policy_budget` | budget exhausted | no |
| `provider_auth` | provider rejected the credential (sanitized) | yes |
| `provider_status` | other non-success from provider (`provider_status` = HTTP code) | yes |
| `provider_network` | provider unreachable | attempted |
| `bad_request` | malformed intent | no |
| `unsupported_type` | unknown message type | no |
| `unsupported_provider` | Executor cannot serve this `provider` | no |

## 9. Security considerations

- **Credential confinement (the whole point).** The credential MUST exist only inside the
  Executor's process/host and MUST NOT appear in any Keyward message, log, or error. An
  implementation that violates this is non-conformant — there is no "convenience" exception.
- **Pairing tokens** MUST be single-use and short-lived. A leaked token lets an attacker pair
  *their* Orchestrator to the Owner's Executor; they still cannot extract the key, and policy still
  bounds the spend. Implementations SHOULD show the Owner every paired Orchestrator and let them
  revoke one.
- **Authenticate the Orchestrator.** The Executor spends the Owner's money on the Orchestrator's
  say-so, so it SHOULD authenticate the Orchestrator (e.g. a public key pinned at pairing) rather
  than trust the channel alone. Concrete mechanism deferred past v0.
- **Payloads are not protected.** Prompts and completions are visible to the Orchestrator by
  construction. Keyward is about custody of the *credential*, not confidentiality of the *content*.
  Do not rely on it for the latter.

## 10. Versioning

`kw` carries the major version. Within a major version, additions — new optional fields, new
message types — are backward-compatible and receivers MUST ignore what they do not understand.
Breaking changes bump `kw`. **v0 is explicitly unstable; treat every detail as provisional until
v1.**

## Open questions

These are unresolved on purpose, and feedback on them is the most useful thing an issue can carry:

- The Orchestrator-authentication mechanism (§3, §9).
- Session resumption semantics across channel loss (§7).
- Where budget pricing data comes from (§6).
- A binary/CBOR transport profile (§1).
- Multi-key / multi-account Executors: a single Executor fronting several providers or several
  accounts of one provider — assumed reachable via `provider`, but selection beyond that is
  under-specified.
