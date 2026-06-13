# 集成 Keyward —— 面向接入方

🌐 [English](../en/integration.md) · **中文**

> 你在做应用 / SaaS —— 即 **Orchestrator**。你不持有 key。你的职责：签发配对 token、接受 Executor 的拨入、
> 发送工作意图、把流式结果转给你的用户。不熟悉模型？读[文档索引](./README.md)。

## 现状与路线

**现在 · 零改动：** 跑 **OpenAI 兼容代理** —— `keyward proxy`（`--features proxy` 构建）。它暴露一个
OpenAI 风格的 HTTP 端点、背后接已配对的 Executor，任何现存应用把 base URL 指过来即可接入：

```sh
keyward proxy   # 等一个 executor 配对，然后服务 http://127.0.0.1:8088
# 你的 app 里：  OPENAI_BASE_URL=http://127.0.0.1:8088/v1   OPENAI_API_KEY=anything
```

`/v1/chat/completions`、`/v1/responses`、`/v1/messages` 按路径路由到对应方言；流式原样转发，所以你
现有的 OpenAI SDK 直接能解析。key 始终留在 Executor 上，app 的 `OPENAI_API_KEY` 被忽略。

**现在 · 进程内嵌：** 用 **Orchestrator SDK** 把客户端嵌进进程——Rust 用
[`keyward-sdk`](../../crates/keyward-sdk)，Go 用 [`sdk/go`](../../sdk/go)。两者都是：绑一个监听、
`serve_one` / `ServeOne` 配对一个 Executor，然后发 work intent、流式拿回原生事件。（Go SDK 与 Rust
Executor 字节级兼容，CI 里跨语言验证过。）

**现在 · 更底层：** 直接对着 WebSocket 实现 `v0` 协议——完整契约在 [spec.md](../spec.md)，
`keyward orchestrator` 是可读可跑的参考。

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

## 控制谁能绑定（保护你这一侧）

你也可以**反向认证 Executor**，只允许你注册过的用户绑定。每个 Executor 有一把稳定的身份钥；用户跑
`keyward identity` 拿到自己的 pubkey，在注册时登记给你。之后你只放行这个白名单：每个 `hello` 都带着
Executor 的 `pubkey` 和对配对 token 的签名，没在白名单里的一律拒绝。

这保护的是**你的**利益（谁能用你的 app），不碰用户那一侧——它纯粹是个「谁能绑定」的门禁。它**不会**对用户
隐藏 prompt 或 key：BYOK 下 Executor 是用户在跑，他们永远能检查自己的流量（这正是 Keyward 的意义），
而且谁给 Provider 请求装上凭证、谁就必然看得见这个请求。如果你需要对**用户**隐藏 payload，那 BYOK 就是错的
模型——那需要服务端 / TEE 执行。

---

本地试一下：[完整走一遍](./walkthrough.md) · 读协议格式：[spec.md](../spec.md) · 回到[文档索引](./README.md)。
