# A full local walkthrough

🌐 **English** · [中文](../zh/walkthrough.md)

> See the whole chain on one machine. New to the model? Read the
> [docs index](./README.md) first.

## Keyless, no network

```sh
cargo run -- demo
```

Stands up both ends with a mock provider and streams three native dialects (OpenAI
Chat Completions, OpenAI Responses, Anthropic Messages) plus a policy-blocked call —
so you can watch dial-out pairing, the root→operational key chain, policy enforced
before the provider is touched, and metered streaming, all without a key.

There's also `cargo run -- resume-demo`: it streams an intent, **drops the socket
mid-stream**, then reconnects and resumes from exactly where it left off.

## A real provider call

Open two terminals.

```sh
# Terminal 1 — the app (Orchestrator). Holds no key. Here we ask for a real OpenAI call.
KEYWARD_PROVIDER=openai KEYWARD_MODEL=gpt-4o KEYWARD_PROMPT="Say hi in 5 words." \
  cargo run -- orchestrator
# It prints a ws:// address and a pairing token, then waits.

# Terminal 2 — the Executor (you). Build with the openai adapter; key from keychain/env.
OPENAI_API_KEY=sk-... \
KEYWARD_ORCH_URL=ws://127.0.0.1:8787 KEYWARD_PAIRING_TOKEN=pt_dev_token \
  cargo run --features openai -- executor
```

Terminal 2 dials out, pairs, the Executor pins the root and verifies the op key,
checks policy, makes the **real** OpenAI call with your key, and streams the answer
back to Terminal 1. Want the Responses API instead? Set `KEYWARD_PROVIDER=openai-responses`.

---

Bringing your own key for real: [for users](./users.md) · Integrating an app:
[for app builders](./integration.md) · Back to the [docs index](./README.md).
