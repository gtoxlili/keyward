# 集成 Keyward —— 面向接入方

🌐 [English](../en/integration.md) · **中文**

> 你在做应用 / SaaS —— 即 **Orchestrator**。你不持有 key。你的职责：签发配对 token、接受 Executor 的拨入、
> 发送工作意图、把流式结果转给你的用户。不熟悉模型？读[文档索引](./README.md)。

## 现状与路线

**现在：** 通过一条双向通道（WebSocket）讲 `v0` 协议来集成。完整契约在 [spec.md](../spec.md)，
`keyward orchestrator` 是一个可读、可跑的参考实现。

**路线图：** 即插即用的 **Orchestrator SDK**（把你的 Provider 客户端换一行）和本地 **OpenAI 兼容代理**
（任何现存应用改个 `OPENAI_BASE_URL` 即可接入），让你几乎零改动集成。

## 消息流

一次配对，之后在同一会话上发任意多个工作意图：

```
Executor（用户侧）                             Orchestrator（你的应用）
   │ ── hello (pairing_token, providers) ───────▶ │  校验 token
   │ ◀── paired (root_pubkey, op cert, sig) ───── │  证明身份、签 sid
   │  钉住 root、验证证书链                          │
   │ ◀── work (provider, native request) ──────── │  发一个 LLM 调用，不带 key
   │  检查策略 ✓、注入 key、调用 Provider            │
   │ ── work_chunk (seq, native delta) ─────────▶ │  转给你的用户
   │ ── work_done (usage) ──────────────────────▶ │
```

- `work.request` 就是 Provider 的**原生** body、去掉凭证——OpenAI Chat Completions 用 `messages`，
  Responses API 用 `input`，`anthropic` 用 Anthropic Messages 形状。Executor 原样转发、你拿回原生 chunk，
  所以你现有的 Provider-SDK 解析照常工作。
- 凭证只活在 Executor 里——你既不发送、也不接收任何凭证。
- 通道掉线是**挂起**而非失败：重连后用 `resume { mid, last_seq }` 补回漏掉的 chunk；用 `cancel { mid }` 主动中止。

## 配对体验

生成一个**单次、短时效**的配对 token，用 WalletConnect 的方式展示给用户——一段可粘贴的码，或（路线图）
一个可扫描的二维码。用一把长期的**根身份钥**证明自己；Executor 首次接触时把它钉住，因此跨重连的密钥轮换 /
自动扩容都无需重新配对。把你根钥的指纹展示出来，让用户带外核对。

---

本地试一下：[完整走一遍](./walkthrough.md) · 读协议格式：[spec.md](../spec.md) · 回到[文档索引](./README.md)。
