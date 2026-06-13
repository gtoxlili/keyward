# Integrate Keyward — for app builders / 面向接入方 · 集成 Keyward

> You're building the app / SaaS — the **Orchestrator**. You hold no key. Your job:
> issue a pairing token, accept the Executor's dial-out, send work intents, and relay
> the streamed result to your user. New to the model? Read the [docs index](./README.md).
>
> 你在做应用 / SaaS —— 即 **Orchestrator**。你不持有 key。你的职责：签发配对 token、接受 Executor 的拨入、
> 发送工作意图、把流式结果转给你的用户。不熟悉模型？读[文档索引](./README.md)。

## Today vs. roadmap / 现状与路线

**Today / 现在:** integrate by speaking the `v0` wire protocol over a bidirectional
channel (a WebSocket). The whole contract is in [SPEC.md](../SPEC.md), and
`keyward orchestrator` is a working reference you can read and run.

**现在:** 通过一条双向通道（WebSocket）讲 `v0` 协议来集成。完整契约在 [SPEC.md](../SPEC.md)，
`keyward orchestrator` 是一个可读、可跑的参考实现。

**Roadmap / 路线图:** a drop-in **Orchestrator SDK** (swap your provider client for
one line) and a local **OpenAI-compatible proxy** (point any existing app at it by
changing `OPENAI_BASE_URL`) so you integrate with near-zero code change.

**路线图:** 即插即用的 **Orchestrator SDK**（把你的 Provider 客户端换一行）和本地 **OpenAI 兼容代理**
（任何现存应用改个 `OPENAI_BASE_URL` 即可接入），让你几乎零改动集成。

## The message flow / 消息流

One pairing, then any number of work intents over the same session:

一次配对，之后在同一会话上发任意多个工作意图：

```
Executor (user side)                          Orchestrator (your app)
   │ ── hello (pairing_token, providers) ───────▶ │  verify token
   │ ◀── paired (root_pubkey, op cert, sig) ───── │  prove identity, sign sid
   │  pin root, verify chain                       │
   │ ◀── work (provider, native request) ──────── │  send an LLM call, no key
   │  check policy ✓, inject key, call provider    │
   │ ── work_chunk (seq, native delta) ─────────▶ │  relay to your user
   │ ── work_done (usage) ──────────────────────▶ │
```

- `work.request` is the provider's **native** body, minus any credential — `messages`
  for OpenAI Chat Completions, `input` for the Responses API, the Anthropic Messages
  shape for `anthropic`. The Executor passes it through and you get native chunks
  back, so your existing provider-SDK parsing keeps working.
- The credential lives only in the Executor — you never send one and never receive one.
- A dropped channel **suspends**, it doesn't fail: reconnect and `resume { mid, last_seq }`
  to replay the chunks you missed; send `cancel { mid }` to deliberately abort.

- `work.request` 就是 Provider 的**原生** body、去掉凭证——OpenAI Chat Completions 用 `messages`，
  Responses API 用 `input`，`anthropic` 用 Anthropic Messages 形状。Executor 原样转发、你拿回原生 chunk，
  所以你现有的 Provider-SDK 解析照常工作。
- 凭证只活在 Executor 里——你既不发送、也不接收任何凭证。
- 通道掉线是**挂起**而非失败：重连后用 `resume { mid, last_seq }` 补回漏掉的 chunk；用 `cancel { mid }` 主动中止。

## Pairing UX / 配对体验

Generate a **single-use, short-lived** pairing token and show it to your user the
WalletConnect way — a code to paste or (roadmap) a QR to scan. Authenticate yourself
with a long-lived **root identity key**; the Executor pins it on first contact, so key
rotation / autoscaling across reconnects needs no re-pairing. Show your root key's
fingerprint so the user can confirm it out of band.

生成一个**单次、短时效**的配对 token，用 WalletConnect 的方式展示给用户——一段可粘贴的码，或（路线图）
一个可扫描的二维码。用一把长期的**根身份钥**证明自己；Executor 首次接触时把它钉住，因此跨重连的密钥轮换 /
自动扩容都无需重新配对。把你根钥的指纹展示出来，让用户带外核对。

---

Try it locally: [a full walkthrough](./walkthrough.md) · Read the wire format:
[SPEC.md](../SPEC.md) · Back to the [docs index](./README.md).

本地试一下：[完整走一遍](./walkthrough.md) · 读协议格式：[SPEC.md](../SPEC.md) · 回到[文档索引](./README.md)。
