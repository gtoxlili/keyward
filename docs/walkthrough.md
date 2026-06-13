# A full local walkthrough / 本地完整走一遍

> See the whole chain on one machine. New to the model? Read the
> [docs index](./README.md) first.
>
> 在一台机器上看完整链路。不熟悉模型？先读[文档索引](./README.md)。

## Keyless, no network / 无 key、无网络

```sh
cargo run -- demo
```

Stands up both ends with a mock provider and streams three native dialects (OpenAI
Chat Completions, OpenAI Responses, Anthropic Messages) plus a policy-blocked call —
so you can watch dial-out pairing, the root→operational key chain, policy enforced
before the provider is touched, and metered streaming, all without a key.

用 mock provider 把两端都立起来，流式演示三种原生方言（OpenAI Chat Completions、OpenAI Responses、
Anthropic Messages）外加一个被策略拦下的调用——你能看到拨出配对、根→操作钥链、调用 Provider 前的策略执行、
带计量的流式传输，全程无需 key。

There's also `cargo run -- resume-demo`: it streams an intent, **drops the socket
mid-stream**, then reconnects and resumes from exactly where it left off.

还有 `cargo run -- resume-demo`：它流式发出一个意图、**中途断开 socket**，再重连并从断点精确续传。

## A real provider call / 一次真实的 Provider 调用

Open two terminals.

开两个终端。

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

终端 2 拨出、配对，Executor 钉住根钥并验证操作钥、检查策略、用你的 key 发起**真实**的 OpenAI 调用，
把答案流式传回终端 1。想用 Responses API？把 `KEYWARD_PROVIDER` 设为 `openai-responses`。

---

Bringing your own key for real: [for users](./users.md) · Integrating an app:
[for app builders](./integration.md) · Back to the [docs index](./README.md).

真正自带 key：[面向用户](./users.md) · 集成应用：[面向接入方](./integration.md) · 回到[文档索引](./README.md)。
