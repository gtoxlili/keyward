# 本地完整走一遍

🌐 [English](../en/walkthrough.md) · **中文**

> 在一台机器上看完整链路。不熟悉模型？先读[文档索引](./README.md)。

## 无 key、无网络

```sh
cargo run -- demo
```

用 mock provider 把两端都立起来，流式演示三种原生方言（OpenAI Chat Completions、OpenAI Responses、
Anthropic Messages）外加一个被策略拦下的调用——你能看到拨出配对、根→操作钥链、调用 Provider 前的策略执行、
带计量的流式传输，全程无需 key。

还有 `cargo run -- resume-demo`：它流式发出一个意图、**中途断开 socket**，再重连并从断点精确续传。

## 一次真实的 Provider 调用

开两个终端。

```sh
# 终端 1 —— 应用（Orchestrator），不持有 key。这里请求一次真实的 OpenAI 调用。
KEYWARD_PROVIDER=openai KEYWARD_MODEL=gpt-4o KEYWARD_PROMPT="Say hi in 5 words." \
  cargo run -- orchestrator
# 它会打印一个 ws:// 地址和一个配对 token，然后等待。

# 终端 2 —— Executor（你）。带 openai 适配器构建；key 来自钥匙串 / 环境变量。
OPENAI_API_KEY=sk-... \
KEYWARD_ORCH_URL=ws://127.0.0.1:8787 KEYWARD_PAIRING_TOKEN=pt_dev_token \
  cargo run --features openai -- executor
```

终端 2 拨出、配对，Executor 钉住根钥并验证操作钥、检查策略、用你的 key 发起**真实**的 OpenAI 调用，
把答案流式传回终端 1。想用 Responses API？把 `KEYWARD_PROVIDER` 设为 `openai-responses`。

---

真正自带 key：[面向用户](./users.md) · 集成应用：[面向接入方](./integration.md) · 回到[文档索引](./README.md)。
