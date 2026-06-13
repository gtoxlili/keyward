# Keyward Docs / 文档

> Bilingual (English first, 中文在后). New here? Read **the model in one minute**,
> then jump to your role. For the protocol bytes see [SPEC.md](../SPEC.md); for the
> reference code see [IMPLEMENTATION.md](../IMPLEMENTATION.md).
>
> 中英双语（先英文，后中文）。第一次来？先读**一分钟理解模型**，再按你的角色跳转。协议细节见
> [SPEC.md](../SPEC.md)，参考实现见 [IMPLEMENTATION.md](../IMPLEMENTATION.md)。

## The model in one minute / 一分钟理解模型

Keyward splits an AI app into a **brain** and a pair of **hands**:

- **Orchestrator** — the app / SaaS. Decides *what* to do (prompts, the agent loop).
  It **never holds your key.**
- **Executor** — a small program *you* run, inside your own trust boundary. It holds
  the key, enforces your limits, and makes the actual call to the provider.
- **Provider** — OpenAI, Anthropic, etc.

The app sends a *work intent* (the request minus any key) to your Executor; your
Executor attaches the key locally and streams the result back. The key never travels
to the app. If you've used WalletConnect, this is that idea for API keys.

Keyward 把一个 AI 应用拆成**大脑**和一双**手**：

- **Orchestrator（编排端）** —— 应用 / SaaS。决定*做什么*（提示词、agent 循环），**永远不持有你的 key**。
- **Executor（执行器）** —— *你*在自己信任边界内运行的小程序。它持有 key、执行你的限额、真正去调 Provider。
- **Provider** —— OpenAI、Anthropic 等。

应用把一个*工作意图（去掉 key 的请求）*发给你的 Executor；Executor 在本地装上 key、调用 Provider，再把结果
流式传回。Key 从不传到应用侧。用过 WalletConnect 的话，这就是「API key 版」的它。

## Who are you? / 你是谁？

| You are… / 你是 | You run / 你运行 | Start here / 从这开始 |
| --- | --- | --- |
| An end user with an API key / 持有 API key 的最终用户 | the **Executor** | [Bring Your Own Key / 自带 Key](./users.md) |
| An app / SaaS builder / 应用 · SaaS 开发者 | the **Orchestrator** | [Integrate Keyward / 集成](./integration.md) |

## All docs / 全部文档

- [Bring Your Own Key — for users / 面向用户](./users.md)
- [Integrate Keyward — for app builders / 面向接入方](./integration.md)
- [A full local walkthrough / 本地完整走一遍](./walkthrough.md)
- [FAQ / 常见问题](./faq.md)
- [Protocol wire format / 协议格式 (SPEC)](../SPEC.md)
- [Reference implementation / 参考实现 (IMPLEMENTATION)](../IMPLEMENTATION.md)

## Status & current limits / 现状与当前限制

- **`v0`, unstable** — wire details may change before `v1`. / `v0`、不稳定——`v1` 前协议细节可能变。
- **Providers:** OpenAI Chat Completions, OpenAI Responses, Anthropic Messages
  (Chat-Completions also covers OpenAI-compatible providers). Gemini / tool-use /
  images are not verified yet. / **Provider：** OpenAI Chat Completions、OpenAI
  Responses、Anthropic Messages（Chat-Completions 同时覆盖 OpenAI 兼容厂商）；Gemini /
  工具调用 / 图片尚未验证。
- **Not built yet:** prebuilt binaries, QR pairing, a per-Owner policy file,
  single-use-token enforcement on reconnect, your-own serverless templates, the
  Orchestrator SDK / OpenAI-compatible proxy. / **尚未实现：** 预编译二进制、二维码配对、
  按用户的策略文件、重连时的单次 token 强制、你自己的 serverless 模板、Orchestrator SDK / OpenAI 兼容代理。

Found a hole in the core promise — any way the app could get your key? Please report
it privately ([SECURITY.md](../SECURITY.md)).
发现核心承诺的漏洞——任何让应用拿到你 key 的方式？请私下报告（[SECURITY.md](../SECURITY.md)）。
