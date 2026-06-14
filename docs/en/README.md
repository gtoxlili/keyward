# Keyward Docs

🌐 **English** · [中文](../zh/README.md)

> New here? Read the model in one minute, then jump to your role. For the protocol
> bytes see [SPEC.md](../spec.md); for the reference code see
> [IMPLEMENTATION.md](../implementation.md).

## The model in one minute

Keyward pulls the key-holding out of an AI app and splits it across two cooperating
pieces — and the app doesn't even have to know:

- **Client** — a small program *you* run, inside your own trust boundary. It holds
  the key, enforces your limits, and makes the actual call to the provider. (The
  "hands" / "wallet".)
- **Node** — a rendezvous the Client dials into and the app reaches. It holds **no
  key**; it just routes each request to the right Client. (The WalletConnect *relay* —
  neutral, and it can be a shared/public station.)
- **App** — decides *what* to do (prompts, the agent loop), holds no key, and can stay
  **completely unaware**: it just points its OpenAI base URL at a Node, with a routing
  token as its "API key".
- **Provider** — OpenAI, Anthropic, etc.

The app makes an ordinary request; the Node relays it to your Client as a *work intent*
(minus any key); the Client attaches the key locally and streams the result back. The
key never travels to the app. If you've used WalletConnect, this is that idea for API keys.

## Who are you?

| You are… | What you do | Start here |
| --- | --- | --- |
| An end user with an API key | run the **Client** that holds your key | [Bring Your Own Key](./users.md) |
| An app / SaaS builder | point your app at a **Node** — or run/embed one | [Integrate Keyward](./integration.md) |

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
- **Not built yet:** prebuilt binaries, QR pairing, your-own serverless Client
  templates, a browser / WASM Client, a byte-reproducible build, and broader
  provider (Gemini, etc.) / multimodal (tool-use, images) coverage.

Found a hole in the core promise — any way the app could get your key? Please report
it privately ([SECURITY.md](../../SECURITY.md)).
