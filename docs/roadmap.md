# Roadmap & status

The single source of truth for what's built and what's left. Today Keyward is a `v0`
reference **skeleton that runs end to end** — the gap below is the road to a `v1` that
real apps and users can actually adopt.

Legend: ✅ done · 🟡 partial · ⬜ not started

"Done" for a protocol means roughly: (1) the `v1` wire spec frozen, (2) a reference
Executor verified against real providers, (3) at least one drop-in integration path so
apps adopt it without rewriting, (4) the deployment targets (serverless / binaries) the
non-custodial pitch promises. The two biggest unknowns on that path are **real-provider
verification** and the **Orchestrator SDK / proxy**.

## ✅ Done

- `v0` wire protocol: envelope, pairing, work intents, streaming frames, policy object ([spec](./spec.md))
- Orchestrator auth — root→operational-key chain (Ed25519 SSH-CA: pin root TOFU, rotate op keys across reconnects)
- Executor auth — Orchestrator authenticates the Executor (identity key + pairing-token signature + allow-list)
- Policy engine — provider/model allow-lists (globs), USD budget, rate, expiry, in the §6 order, enforced before the provider is touched
- Provider dialects — OpenAI Chat Completions, OpenAI Responses, Anthropic Messages (Chat-Completions covers OpenAI-compatible providers)
- Session resumption + cancel (§7) — per-intent producer + bounded ring buffer; survives a dropped channel
- Usage metering — vendored LiteLLM prices (`data/model_prices.json` + `scripts/refresh-prices.sh`); real `sha256` policy digest
- Secret storage — OS keychain (native backends, no D-Bus), per provider, env fallback; `SecretString` (redacted, zeroized)
- Transport — outbound WebSocket **and gRPC** reference adapters (scheme-selected `ws://`/`grpc://`, same JSON envelope; gRPC keeps the Executor as the dialing client, cross-verified end to end)
- Engineering — CI (fmt/clippy/test), release workflow with SLSA provenance, dependabot, ~32 tests, bilingual docs

## 🟡 Partial

- **Reference Executor** — runs end to end; missing serverless templates and the bit-for-bit reproducible build
- **Provider coverage** — 3 dialects done; Gemini and others, plus tool-use / images / multimodal, not yet

## ⬜ Not built yet — the gap to "done"

### Security / protocol hardening
- [ ] **Noise inner layer** — E2E crypto for the untrusted-relay case; pick the concrete profile (§9 / spec open question)
- [x] **Single-use pairing tokens** — a token binds to one Executor identity; the same identity may reconnect (resume), a different one is refused
- [~] **Out-of-band fingerprint confirmation** — `KEYWARD_EXPECT_ROOT_FP` lets the Owner pre-state the expected root fingerprint and refuse a mismatch; a passkey / QR gesture is still future
- [ ] **Signature-bound resume** — bind `resume` to a fresh identity signature, not just a re-pair
- [~] **Secret hardening** — `set-key` now reads from a hidden-TTY prompt (no echo) when interactive; `mlock`/`setrlimit` to keep the key out of swap/core dumps still TODO

### Make it usable (productization)
- [x] **Desktop app** — a bilingual (EN/中文) Tauri app for the Executor: pairing, OS-keychain credentials, policy, and a live dashboard with a "Orchestrator → Executor" routing line. Drives the real executor core (the `keyward` crate as a lib). ([apps/executor-desktop](../apps/executor-desktop))
- [x] **Local OpenAI-compatible proxy** — `keyward proxy` (feature `proxy`): an app points `OPENAI_BASE_URL` at it and the proxy relays each request to the paired Executor; streaming SSE is native passthrough, non-streaming is assembled. Verified live against a real backend.
- [x] **Multi-tenant broker / public station** — `keyward broker` (feature `broker`, [spec §10](./spec.md)): a shared/neutral Orchestrator that many Owners' Executors dial into; each request routes to the right Executor by a **routing token in its bearer/API-key slot** (the app stays unaware). The token is a capability, not the provider key — policy still gates. Verified live: two executors, different policies, routed correctly by `Authorization: Bearer`.
- [x] **Orchestrator SDK** — Rust (`keyward-sdk`) and Go (`sdk/go`) clients, both cross-verified against the real executor (the Go orchestrator's Ed25519 chain verifies on the Rust executor — proof the protocol is language-agnostic). The Rust SDK serves either transport (`serve_one` over WebSocket, `serve_one_grpc` over gRPC). (The proxy covers zero-code-change integration; these are for embedding in-process.)
- [ ] **Serverless Executor templates** — Cloudflare Worker / AWS Lambda / Deno Deploy, key as a secret in the user's own account
- [ ] **Browser / WASM Executor** — the ephemeral, in-tab interactive case
- [ ] **Prebuilt binaries / installer** — so users don't have to `cargo build`
- [ ] **QR pairing UX** — the WalletConnect gesture (today the pairing token is passed by env var)
- [x] **Per-Owner policy file** — `KEYWARD_POLICY` points the executor at a JSON policy ([example](./policy.example.json)); falls back to a built-in default

### Coverage / verification
- [~] **Real-provider verification** — the OpenAI Chat Completions adapter is verified end to end against a live streaming API (streaming SSE parse + usage extraction); there's an opt-in `live_*` test (`crates/keyward/src/e2e_tests.rs`, gated on `KEYWARD_LIVE_BASE_URL` + `KEYWARD_LIVE_KEY`). The Responses API and Anthropic Messages are still mock-only.
- [ ] **More providers + multimodal** — Gemini and others; verify tool-use / image / non-text payloads flow through native passthrough
- [ ] **Byte-reproducible build** — pinned-container build so a third party can reproduce the binary bit-for-bit (provenance attestation already ships)

### Spec open questions
- [ ] Concrete **Noise profile** — pattern (`XX` / `KK` / `IKpsk2`), framing, re-handshake schedule (§9)
- [ ] **Binary / CBOR transport profile** (§1)
- [ ] **Multi-key / multi-account Executors** — selection beyond `provider` is under-specified

### Ecosystem
- [ ] **Conformance suite** — once there's a second implementation to keep the spec honest
- [ ] A **second implementation** in another language

---

Design decisions deliberately *out of scope* (not TODOs): hiding prompts/payloads from the Owner,
or stopping the Owner from inspecting their own traffic — both contradict non-custodial BYOK and are
impossible on a machine the Owner controls. Payload-confidentiality-from-the-Owner needs a custodial
/ TEE model, which is the thing Keyward exists to avoid. See [spec §9](./spec.md).
