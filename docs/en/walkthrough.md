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

The Node speaks OpenAI over HTTP, so any OpenAI client drives it. Open two terminals,
then `curl` it like you would OpenAI.

```sh
# Terminal 1 — the Node (rendezvous). Holds no key. Listens for a Client on :8787 and
# serves an OpenAI-compatible front on :8088.
cargo run --features node -- node
# prints: clients dial in on ws://127.0.0.1:8787  (pairing_token=pt_dev_token)

# Terminal 2 — the Client (you). Build with the openai adapter; key from keychain/env.
OPENAI_API_KEY=sk-... \
KEYWARD_NODE_URL=ws://127.0.0.1:8787 KEYWARD_PAIRING_TOKEN=pt_dev_token \
  cargo run --features openai -- client
```

Now drive it through the Node exactly as an unaware OpenAI app would — its base URL is just
the Node, and any bearer routes to your single paired Client:

```sh
# Terminal 3 — the unaware "app": an ordinary OpenAI call.
curl http://127.0.0.1:8088/v1/chat/completions \
  -H 'authorization: Bearer anything' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Say hi in 5 words."}]}'
```

The Client dials out, pairs, pins the root and verifies the op key, checks policy, makes the
**real** OpenAI call with your key, and streams the answer back through the Node to your curl.
Want the Responses API? Hit `/v1/responses`; for Anthropic, `/v1/messages`.

---

Bringing your own key for real: [for users](./users.md) · Integrating an app:
[for app builders](./integration.md) · Back to the [docs index](./README.md).
