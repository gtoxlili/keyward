# 本地完整跑一遍

🌐 [English](../en/walkthrough.md) · **中文**

> 在一台机器上看完整链路是怎么跑起来的。还不熟悉这套模型？先看[文档索引](./README.md)。

## 不用 key，也不联网

```sh
cargo run -- demo
```

它用一个 mock provider 把两端都跑起来，流式演示三种原生请求格式（OpenAI Chat Completions、OpenAI
Responses、Anthropic Messages），外加一个被策略拦下的调用——你能亲眼看到拨出配对、根→操作钥的证书链、
请求发往 Provider 之前的策略执行，以及带用量计量的流式传输，全程不需要任何 key。

还有一个 `cargo run -- resume-demo`：它先流式发出一个意图，**中途把 socket 断开**，然后重连，并从断点处
精确续传。

## 一次真实的 Provider 调用

开两个终端。

```sh
# 终端 1 —— 应用（Orchestrator），不持有 key。这里我们请求一次真实的 OpenAI 调用。
KEYWARD_PROVIDER=openai KEYWARD_MODEL=gpt-4o KEYWARD_PROMPT="Say hi in 5 words." \
  cargo run -- orchestrator
# 它会打印出一个 ws:// 地址和一个配对 token，然后等待。

# 终端 2 —— Executor（你）。构建时带上 openai 适配器；key 来自钥匙串 / 环境变量。
OPENAI_API_KEY=sk-... \
KEYWARD_ORCH_URL=ws://127.0.0.1:8787 KEYWARD_PAIRING_TOKEN=pt_dev_token \
  cargo run --features openai -- executor
```

终端 2 拨出并配对，Executor 钉住根钥、验证操作钥、检查策略，然后用你的 key 发起一次**真实**的 OpenAI 调用，
再把答案流式传回终端 1。想改用 Responses API？把 `KEYWARD_PROVIDER` 设成 `openai-responses` 即可。

---

真正自带 key 上手：[写给用户](./users.md) · 把它集成进应用：[写给应用开发者](./integration.md) · 回到[文档索引](./README.md)。
