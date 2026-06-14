# Keyward Protocol — v0 (Draft)

> **Status: draft, unstable.** Anything here can change until `v1`. This document specifies the
> wire-level contract between a **Client** (holds the key, runs on the Owner's side) and a
> **Node** (the rendezvous; holds no key). The roles, motivation, and threat model live in the
> [README](../README.md) — this is just the bytes on the wire.

The keywords MUST, MUST NOT, SHOULD, MAY are used as in RFC 2119.

## 1. Transport

Keyward is transport-agnostic. It needs exactly one **bidirectional, reliable, ordered,
message-oriented** channel between Client and Node. An outbound WebSocket or a gRPC
bidi stream is the expected shape; an HTTP/2 stream qualifies too.

The dialed channel is an **application-level connection the Node pushes work down**, not a
published port. General-purpose tunnel appliances (`frp`, Cloudflare Tunnel, ngrok, Tailscale
Funnel) are built for the opposite job — exposing an inbound listener to the public — and several
interpose a third-party edge the Owner does not control. They MAY back a transport adapter, but
only if they preserve invariant 1 below and do not require the Client to accept inbound
connections; the protocol MUST NOT assume them.

Two invariants the transport MUST hold:

1. **The Client dials out.** The channel is established from the Owner's side (reverse
   connection). The Node MUST NOT require an inbound connection to the Owner — no open
   ports, no public endpoint there.
2. **In-order, lossless within a session.** On channel loss the session is suspended (§7), not
   silently resumed.

Messages are UTF-8 JSON objects, one per transport frame. A binary/CBOR profile may come later;
out of scope for v0. The channel MUST be encrypted (TLS or equivalent) — Keyward assumes
confidentiality and integrity from the transport.

Both reference transports are implemented and interchangeable: an outbound **WebSocket** (one JSON
object per text frame) and a **gRPC** bidirectional stream (`service Keyward { rpc Open(stream
Frame) returns (stream Frame); }`, where each `Frame { string json = 1; }` wraps one canonical JSON
message). gRPC keeps the Client as the client so invariant 1 still holds — it dials out and opens
the stream. The Client picks the adapter by URL scheme (`ws://` / `wss://` vs `grpc://` /
`grpcs://`); everything above the channel is identical.

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

Out of band, the Owner obtains a one-time **pairing token** from the Node — a QR to scan or
a code to paste, exactly the WalletConnect gesture. The token MUST be short-lived and single-use.

**Client → Node**

```json
{
  "kw": "0", "type": "hello", "mid": "01J...",
  "pairing_token": "pt_...",
  "client":  { "name": "keyward-exec", "version": "0.1.0" },
  "providers": ["openai", "anthropic"],
  "policy_digest": "sha256:9f86d0...",
  "pubkey": "3840db81...",
  "sig": "client signature over pairing_token"
}
```

- `policy_digest` is a hash of the active policy (§6). It lets the Node notice that limits
  changed without the Owner having to reveal them. Sharing the full policy is OPTIONAL via a
  `policy` field — the Owner MAY keep their limits private.
- `pubkey` is the Client's long-term identity key; `sig` is its signature over the `pairing_token`,
  proving possession. They let the Node **authenticate the Client** (§9) — e.g. a SaaS that
  admits only registered users keeps an allow-list of Client `pubkey`s. A Node that
  enforces this MUST reject a `hello` whose `sig` is missing or doesn't verify against `pubkey`, or
  whose `pubkey` is not authorized. This authenticates *who is calling*; it does NOT, and must not,
  limit the Owner's ability to inspect their own key (§9).

**Node → Client**

```json
{ "kw": "0", "type": "paired", "sid": "kw_sess_...", "mid": "01J...",
  "node": { "name": "acme-agent", "id": "node_..." },
  "root_pubkey": "9d8f...",
  "op": { "pubkey": "1a2b...", "not_after": 1779999999, "root_sig": "..." },
  "sig": "operational-key signature over sid" }
```

- `root_pubkey` is the Node's **long-term root identity**. The Client MUST **pin** it on
  first contact (trust-on-first-use) and MUST refuse a later pairing presenting a different root.
- `op` delegates a short-lived **operational key**: the root signs `op.pubkey ‖ op.not_after`
  (`root_sig`). The Client MUST verify `root_sig` against the pinned root and reject an expired
  `op`. `sig` is the operational key's signature over the assigned `sid`, which the Client MUST
  verify before treating the session as open. This is the SSH-CA pattern: a stolen pairing token
  alone can't bind (§9) — it can't produce a key chaining to the pinned root — yet the Node
  can **rotate operational keys / autoscale across reconnects without the Owner re-pairing**, since
  the Client pins only the root. Pinning a single bare operational key is NOT RECOMMENDED.
- The Client MUST reject `work` / `resume` / `cancel` that arrive before a verified `paired` on the
  connection.

After `paired` the session is open. TOFU's one weakness is the first contact; to close it the
Client SHOULD confirm the `root_pubkey` fingerprint out of band (a short string shown in the
Client UI/terminal, or a passkey/WebAuthn approval whose challenge embeds the fingerprint for
hosted Nodes).

## 4. Work intent

A request from the Node to perform **one** provider call.

**Node → Client**

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
  `Authorization` / `api_key` field — the Node has none to send.
- The Client selects the endpoint and attaches the credential for `provider`. A `provider` value
  names a specific **API surface**, which may map to a shared account credential: e.g. `openai`
  (Chat Completions) and `openai-responses` (the Responses API) are distinct providers — different
  endpoints and different streaming events — that resolve to the same OpenAI key.
- Streaming is requested through the provider-native field (`"stream": true` for OpenAI-shaped
  providers); the Client reflects that mode onto the wire (§5).

The Client MUST validate the intent against policy (§6) **before** contacting the provider. A
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

The Client MUST NOT place the raw credential, the provider auth header, or anything from which
the credential can be derived, into any frame. On a provider auth failure it returns
`code: "provider_auth"` with a sanitized message — never the key, never the full upstream body if
it might echo the key.

## 6. Policy object

Owner-defined, enforced at the Client, **not changeable by the Node**. All fields are
optional; absence means unrestricted for that dimension, though implementations SHOULD default-deny
on budget.

```json
{
  "providers": ["openai", "anthropic"],
  "models": ["gpt-4o", "claude-sonnet-*"],
  "nodes": ["acme-agent"],
  "budget": { "limit_usd": 20, "window": "month", "spent_usd": 7.40 },
  "rate": { "rpm": 60, "tpm": 200000 },
  "expires_at": "2026-12-31T00:00:00Z"
}
```

- `models` MAY use a trailing-`*` glob.
- `budget.spent_usd` is tracked by the Client from provider-reported `usage` (the billing source
  of truth) and per-model pricing. Pricing SHOULD be sourced from a machine-readable registry such
  as LiteLLM's `model_prices_and_context_window.json`, refreshed on a schedule with a pinned
  fallback copy; Clients MUST NOT scrape provider HTML. Client-side token counting
  (tiktoken / the provider's token-count endpoint) MAY be used as a pre-flight admission estimate
  and as a fallback when a stream is interrupted before `usage` arrives.
- **Enforcement order:** provider → model → node → expiry → rate → budget. The first failing
  check produces the matching `policy_*` error and aborts.
- **Budget vs. streaming.** Final cost is not known until the stream ends, and an interrupted
  stream may report no `usage` at all — and closing the channel does **not** stop the provider from
  billing the generation (§7). A hard cap is therefore enforced at **admission** (a pre-flight
  estimate MAY reject an obviously-over-budget intent) and reconciled **post-hoc** from reported
  `usage`; a Client MAY additionally meter mid-stream and abort at the cap.

Custody stops the key from leaking; **policy stops it from being abused.** Both are the point.

## 7. Session lifecycle

- A session lives as long as the channel. On channel loss the session is **suspended**, not failed:
  the Node MAY re-establish through the transport and resume an in-flight intent.
- **Resumption.** Each intent's `work_chunk`s carry a monotonic `seq` from 0 (§5). The transport
  gives ordering; the `seq` is the resumable cursor — do not reuse a transport stream id for it. A
  Client SHOULD retain recently-sent chunks for an in-flight `mid` in a bounded per-intent buffer.
  On reconnect the Node MAY send

  ```json
  { "type": "resume", "sid": "kw_sess_...", "mid": "01J...", "last_seq": 41 }
  ```

  and the Client replays `seq > last_seq` from its buffer, then continues live. If `last_seq`
  predates the buffer, the Client returns a `work_error` with `code: "unrecoverable"` and the
  Node MAY re-issue under a new `mid`. The Node detects gaps when `seq != expected`
  and dedupes on `mid`+`seq` (delivery is at-least-once; rendering is exactly-once).
- **A dropped channel is a suspend, not a cancel.** To stop work deliberately the Node sends
  `{ "type": "cancel", "mid": "..." }`. Closing the channel MUST NOT be relied on to halt a
  generation: most provider streaming endpoints have no server-side cancel, so the provider keeps
  generating — and **billing** — regardless. A Client that keeps a generation alive for resume is
  therefore mutually exclusive with one that aborts to save tokens; which it does is policy.
- An in-flight `mid` that can be neither resumed nor cancelled cleanly is **failed**; re-issuing is
  the Node's call, and idempotency is its problem — provider calls are not idempotent.
- Either side MAY send `{ "type": "close", "reason": "…" }`. An Owner revoking at the Client
  surfaces as a `close` with `reason: "revoked"`, or simply as a dropped channel.

## 8. Error codes

`work_error.code` (and `error.code` for envelope-level faults):

| code | meaning | provider contacted? |
| --- | --- | --- |
| `policy_provider` | provider not in allowlist | no |
| `policy_model` | model not in allowlist | no |
| `policy_node` | this Node not allowed | no |
| `policy_expired` | policy past `expires_at` | no |
| `policy_rate` | rate limit exceeded | no |
| `policy_budget` | budget exhausted | no |
| `provider_auth` | provider rejected the credential (sanitized) | yes |
| `provider_status` | other non-success from provider (`provider_status` = HTTP code) | yes |
| `provider_network` | provider unreachable | attempted |
| `bad_request` | malformed intent | no |
| `unsupported_type` | unknown message type | no |
| `unsupported_provider` | Client cannot serve this `provider` | no |
| `unrecoverable` | `resume` requested past the retained buffer (§7) | n/a |

## 9. Security considerations

- **Credential confinement (the whole point).** The credential MUST exist only inside the
  Client's process/host and MUST NOT appear in any Keyward message, log, or error. An
  implementation that violates this is non-conformant — there is no "convenience" exception.
- **Pairing tokens** MUST be single-use and short-lived. A leaked token lets an attacker pair
  *their* Node to the Owner's Client; they still cannot extract the key, and policy still
  bounds the spend. Implementations SHOULD show the Owner every paired Node and let them
  revoke one.
- **Authenticate the Node.** The Client spends the Owner's money on the Node's
  say-so, so it MUST authenticate the Node rather than trust the channel alone. The
  mechanism (pinned Ed25519 identity, signature over `sid`, root-key→operational-key chaining,
  out-of-band fingerprint confirmation) is specified in §3.
- **Authenticating the Client (for the Node's benefit).** Symmetrically, a Node
  MAY authenticate the Client: `hello` carries the Client's identity `pubkey` and a `sig` over
  the pairing token (§3). This lets a SaaS admit only registered Clients (an allow-list of
  `pubkey`s) — protecting *its* side without weakening the Owner's. It is strictly a "who may bind"
  control: it MUST NOT be used to stop the Owner from inspecting the credential on their own host
  (that verification is the whole point of Keyward), and it cannot — the Owner runs the Client.
- **Payloads cannot be hidden from the Owner.** A corollary of the above: whoever attaches the
  bearer credential to the provider request necessarily sees that request, and in Keyward that is
  the Owner's Client. So a Node cannot hide prompts/payloads from the Owner; non-custodial
  BYOK and payload-confidentiality-from-the-Owner are mutually exclusive (the latter needs a
  custodial/TEE model, out of scope here).
- **Channel encryption vs. a relay.** A direct dial-out to the Node over TLS gets
  confidentiality and integrity from the transport (§1). If an **untrusted relay** is interposed —
  one that only stores and forwards opaque frames, e.g. the blind node of §10 — TLS to the relay
  is not enough; the Client and the requester run an inner encrypted layer **keyed by the §3
  identities** so the relay sees only ciphertext. The implemented profile is non-interactive ECIES:
  a fresh ephemeral X25519 ↔ the Client's static X25519 (both mapped from the Ed25519 identities)
  feeding ChaCha20-Poly1305; the request carries the ephemeral pubkey and the derived box encrypts
  the response stream too (the `Sealed` frame; the relay forwards it by `mid`). It fits the
  request/response shape without relaying a multi-message handshake, but gives no per-message
  forward secrecy — an interactive Noise XX profile is the FS upgrade (below).
- **Payloads are not protected.** Prompts and completions are visible to the Node by
  construction. Keyward is about custody of the *credential*, not confidentiality of the *content*.
  Do not rely on it for the latter.

## 10. Shared nodes (public stations)

The Node role is not necessarily the SaaS. It decomposes into two jobs that the SaaS
model happens to fuse: **the requester** (who submits work) and **the router** (who matches work
to a Client and relays it back). When they are fused, "which user?" is answered by the SaaS's
own session. When they are not — a **shared/neutral node** (a public-good "station") that many
unrelated Owners point their apps and Clients at — the node must be *told* which Client a
request is for. The protocol already permits this: the Client dials out to any Node URL;
the only missing piece is multi-tenant addressing.

**Routing token.** Each Client is addressed by a **routing token**, advertised in `hello`
(`route_token`) or, absent that, derived by the node from the Client's identity `pubkey`. The
node keeps a `route_token → connection` registry. A request names its token **in the credential
slot the app already sends** — `Authorization: Bearer <token>` (or `x-api-key: <token>`). So an
OpenAI-compatible app stays unaware: to it this is just a base URL and an API key. (This is the
WalletConnect pattern — a public relay routing by session topic — with the topic riding in the
bearer header.) Responses are demultiplexed back to the waiting requester by `mid` (§5), exactly
as a single-tenant Node.

**The token is a capability, not a key.** It is *not* the provider credential — that never leaves
the Client, and the node never sees it. And the Client's policy (§6) still gates every call,
so a leaked routing token can do no more than the policy already allows. A routing token is
therefore far safer to hand an app than a real provider key, even though it sits in the same slot.

**Pairing.** A node admits many Clients under one shared join-token, so it does NOT enforce
the single-use, one-identity-per-token binding a SaaS uses (§9); per-Owner isolation comes from
the routing token plus each Client's own policy. It MAY still authenticate Client identities
(§9) and allow-list them.

**Addressing MUST be by token, never by count.** A multi-tenant node MUST refuse a request whose
token names no connected Client; it MUST NOT fall back to "the only Client currently connected,"
because the connected set is transient — doing so would hand one requester's prompt to an
arbitrary Owner and spend *their* key. A node MAY offer a single-tenant convenience mode (one
Client, token optional) as an explicit operator opt-in, distinct from the default.

**Trust spectrum.** A fully-app-unaware node terminates TLS, so it sees prompts (never keys) —
a "trust the operator" model, acceptable for many public stations and still fully non-custodial of
the credential. To make the node *blind* to content, move the small amount of awareness off the
app and into a **local shim** the unaware app talks to (`keyward shim`): the shim runs the inner
seal layer (§9) end-to-end to the Client — it seals each request to the Client's identity key,
the node forwards ciphertext by token without decrypting (`POST /kw/sealed`, `Sealed` frames),
and only metadata terminals (done/error, usage) stay cleartext. That is the untrusted-relay case of
§9 plus token routing. You can have *app-unaware*, *blind-node*, and
*routable* pairwise, but not all three at once without that shim.

The reference node is `keyward node` (feature `node`): many Clients dial in, each
registering its routing token; an OpenAI-compatible HTTP front routes each request by its bearer
token to the matching Client.

## 11. Versioning

`kw` carries the major version. Within a major version, additions — new optional fields, new
message types — are backward-compatible and receivers MUST ignore what they do not understand.
Breaking changes bump `kw`. **v0 is explicitly unstable; treat every detail as provisional until
v1.**

## Open questions

These are unresolved on purpose, and feedback on them is the most useful thing an issue can carry:

- **Forward secrecy for the inner layer (§9).** The shipped seal layer is non-interactive ECIES
  (no per-message FS). The open upgrade is an interactive **Noise** profile: pattern (`XX`
  first-contact, `KK` vs `IKpsk2` steady-state), framing (Noise caps messages at 65535 bytes), and
  the re-handshake schedule — at the cost of relaying a multi-message handshake through the node.
- A binary/CBOR transport profile (§1).
- Multi-key / multi-account Clients: a single Client fronting several providers or several
  accounts of one provider — assumed reachable via `provider`, but selection beyond that is
  under-specified.

Resolved since the first draft (mechanisms now normative above): Node authentication
(§3/§9), session resumption across channel loss (§7), and the budget-pricing data source (§6).
