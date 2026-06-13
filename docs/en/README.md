# Keyward Docs

🌐 **English** · [中文](../zh/README.md)

> New here? Read the model in one minute, then jump to your role. For the protocol
> bytes see [SPEC.md](../spec.md); for the reference code see
> [IMPLEMENTATION.md](../implementation.md).

## The model in one minute

Keyward splits an AI app into a **brain** and a pair of **hands**:

- **Orchestrator** — the app / SaaS. Decides *what* to do (prompts, the agent loop).
  It **never holds your key.**
- **Executor** — a small program *you* run, inside your own trust boundary. It holds
  the key, enforces your limits, and makes the actual call to the provider.
- **Provider** — OpenAI, Anthropic, etc.

The app sends a *work intent* (the request minus any key) to your Executor; your
Executor attaches the key locally and streams the result back. The key never travels
to the app. If you've used WalletConnect, this is that idea for API keys.

## Who are you?

| You are… | You run | Start here |
| --- | --- | --- |
| An end user with an API key | the **Executor** | [Bring Your Own Key](./users.md) |
| An app / SaaS builder | the **Orchestrator** | [Integrate Keyward](./integration.md) |

## All docs

- [Bring Your Own Key — for users](./users.md)
- [Integrate Keyward — for app builders](./integration.md)
- [A full local walkthrough](./walkthrough.md)
- [FAQ](./faq.md)
- [Protocol wire format (SPEC)](../spec.md)
- [Reference implementation (IMPLEMENTATION)](../implementation.md)
- [Roadmap & status — what's built and what's left](../roadmap.md)

## Status & current limits

- **`v0`, unstable** — wire details may change before `v1`.
- **Providers:** OpenAI Chat Completions, OpenAI Responses, Anthropic Messages
  (Chat-Completions also covers OpenAI-compatible providers). Gemini / tool-use /
  images are not verified yet.
- **Not built yet:** prebuilt binaries, QR pairing, your-own serverless Executor
  templates, a browser / WASM Executor, a byte-reproducible build, and broader
  provider (Gemini, etc.) / multimodal (tool-use, images) coverage.

Found a hole in the core promise — any way the app could get your key? Please report
it privately ([SECURITY.md](../../SECURITY.md)).
