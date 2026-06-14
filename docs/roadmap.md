# Roadmap & status

The single source of truth for what's built and what's left. Today Keyward is a `v0`
reference **skeleton that runs end to end** — the gap below is the road to a `v1` that
real apps and users can actually adopt.

Legend: ✅ done · 🟡 partial · ⬜ not started

"Done" for a protocol means roughly: (1) the `v1` wire spec frozen, (2) a reference
Client verified against real providers, (3) at least one drop-in integration path so
apps adopt it without rewriting, (4) the deployment targets (serverless / binaries) the
non-custodial pitch promises. The two biggest unknowns on that path are **real-provider
verification** and the **Node SDK / standalone node**.

## ✅ Done

- `v0` wire protocol: envelope, pairing, work intents, streaming frames, policy object ([spec](./spec.md))
- Node auth — root→operational-key chain (Ed25519 SSH-CA: pin root TOFU, rotate op keys across reconnects)
- Client auth — Node authenticates the Client (identity key + pairing-token signature + allow-list)
- Policy engine — provider/model allow-lists (globs), USD budget, rate, expiry, in the §6 order, enforced before the provider is touched
- Provider dialects — OpenAI Chat Completions, OpenAI Responses, Anthropic Messages (Chat-Completions covers OpenAI-compatible providers)
- Session resumption + cancel (§7) — per-intent producer + bounded ring buffer; survives a dropped channel
- Usage metering — vendored LiteLLM prices (`data/model_prices.json` + `scripts/refresh-prices.sh`); real `sha256` policy digest
- Secret storage — OS keychain (native backends, no D-Bus), per provider, env fallback; `SecretString` (redacted, zeroized)
- Transport — outbound WebSocket **and gRPC** reference adapters (scheme-selected `ws://`/`grpc://`, same JSON envelope; gRPC keeps the Client as the dialing client, cross-verified end to end)
- Engineering — CI (fmt/clippy/test), release workflow with SLSA provenance, dependabot, ~32 tests, bilingual docs

## 🟡 Partial

- **Reference Client** — runs end to end; missing serverless templates and the bit-for-bit reproducible build
- **Provider coverage** — 3 dialects done; Gemini and others, plus tool-use / images / multimodal, not yet

## ⬜ Not built yet — the gap to "done"

### Security / protocol hardening
- [x] **Inner seal layer** — E2E crypto for the untrusted-relay / blind-node case (§9/§10): `keyward shim` seals each request to the Client's identity key (non-interactive ECIES — X25519 from the Ed25519 identities + ChaCha20-Poly1305), the node forwards only ciphertext. Verified live: a secret in the prompt never reached the node. (Interactive Noise XX, for per-message forward secrecy, remains the open upgrade.)
- [x] **Single-use pairing tokens** — a token binds to one Client identity; the same identity may reconnect (resume), a different one is refused
- [~] **Out-of-band fingerprint confirmation** — `KEYWARD_EXPECT_ROOT_FP` lets the Owner pre-state the expected root fingerprint and refuse a mismatch; a passkey / QR gesture is still future
- [ ] **Signature-bound resume** — bind `resume` to a fresh identity signature, not just a re-pair
- [~] **Secret hardening** — `set-key` now reads from a hidden-TTY prompt (no echo) when interactive; `mlock`/`setrlimit` to keep the key out of swap/core dumps still TODO

### Make it usable (productization)
- [x] **Desktop app** — a bilingual (EN/中文) Tauri app for the Client: pairing, OS-keychain credentials, policy, and a live dashboard with a "Node → Client" routing line. Drives the real client core (the `keyward` crate as a lib). ([apps/executor-desktop](../apps/executor-desktop))
- [x] **Local OpenAI-compatible node** — `keyward node` (feature `node`): an app points `OPENAI_BASE_URL` at it and the node relays each request to the paired Client; streaming SSE is native passthrough, non-streaming is assembled. Verified live against a real backend.
- [x] **Multi-tenant node / public station** — `keyward node` (feature `node`, [spec §10](./spec.md)): a shared/neutral Node that many Owners' Clients dial into; each request routes to the right Client by a **routing token in its bearer/API-key slot** (the app stays unaware). The token is a capability, not the provider key — policy still gates. Verified live: two clients, different policies, routed correctly by `Authorization: Bearer`.
- [x] **Node SDK** — Rust (`keyward-sdk`) and Go (`sdk/go`) clients, both cross-verified against the real client (the Go node's Ed25519 chain verifies on the Rust client — proof the protocol is language-agnostic). The Rust SDK serves either transport (`serve_one` over WebSocket, `serve_one_grpc` over gRPC). (A standalone node covers zero-code-change integration; these are for embedding in-process.)
- [ ] **Serverless Client templates** — Cloudflare Worker / AWS Lambda / Deno Deploy, key as a secret in the user's own account
- [ ] **Browser / WASM Client** — the ephemeral, in-tab interactive case
- [ ] **Prebuilt binaries / installer** — so users don't have to `cargo build`
- [ ] **QR pairing UX** — the WalletConnect gesture (today the pairing token is passed by env var)
- [x] **Per-Owner policy file** — `KEYWARD_POLICY` points the client at a JSON policy ([example](./policy.example.json)); falls back to a built-in default

### Coverage / verification
- [~] **Real-provider verification** — the OpenAI Chat Completions adapter is verified end to end against a live streaming API (streaming SSE parse + usage extraction); there's an opt-in `live_*` test (`crates/keyward/src/e2e_tests.rs`, gated on `KEYWARD_LIVE_BASE_URL` + `KEYWARD_LIVE_KEY`). The Responses API and Anthropic Messages are still mock-only.
- [ ] **More providers + multimodal** — Gemini and others; verify tool-use / image / non-text payloads flow through native passthrough
- [ ] **Byte-reproducible build** — pinned-container build so a third party can reproduce the binary bit-for-bit (provenance attestation already ships)

### Spec open questions
- [ ] **Noise profile for forward secrecy** — the shipped seal layer is non-interactive ECIES (no FS); an interactive Noise `XX`/`KK`/`IKpsk2` profile (framing, re-handshake schedule) is the FS upgrade (§9)
- [ ] **Binary / CBOR transport profile** (§1)
- [ ] **Multi-key / multi-account Clients** — selection beyond `provider` is under-specified

### Ecosystem
- [ ] **Conformance suite** — once there's a second implementation to keep the spec honest
- [ ] A **second implementation** in another language

---

Design decisions deliberately *out of scope* (not TODOs): hiding prompts/payloads from the Owner,
or stopping the Owner from inspecting their own traffic — both contradict non-custodial BYOK and are
impossible on a machine the Owner controls. Payload-confidentiality-from-the-Owner needs a custodial
/ TEE model, which is the thing Keyward exists to avoid. See [spec §9](./spec.md).
