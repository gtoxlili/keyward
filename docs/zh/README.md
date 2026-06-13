# Keyward 文档

🌐 [English](../en/README.md) · **中文**

> 第一次来？先读**一分钟理解模型**，再按你的角色跳转。协议细节见 [spec.md](../spec.md)，
> 参考实现见 [implementation.md](../implementation.md)。

## 一分钟理解模型

Keyward 把一个 AI 应用拆成**大脑**和一双**手**：

- **Orchestrator（编排端）** —— 应用 / SaaS。决定*做什么*（提示词、agent 循环），**永远不持有你的 key**。
- **Executor（执行器）** —— *你*在自己信任边界内运行的小程序。它持有 key、执行你的限额、真正去调 Provider。
- **Provider** —— OpenAI、Anthropic 等。

应用把一个*工作意图（去掉 key 的请求）*发给你的 Executor；Executor 在本地装上 key、调用 Provider，再把结果
流式传回。Key 从不传到应用侧。用过 WalletConnect 的话，这就是「API key 版」的它。

## 你是谁？

| 你是 | 你运行 | 从这开始 |
| --- | --- | --- |
| 持有 API key 的最终用户 | **Executor** | [自带 Key](./users.md) |
| 应用 · SaaS 开发者 | **Orchestrator** | [集成 Keyward](./integration.md) |

## 全部文档

- [自带 Key —— 面向用户](./users.md)
- [集成 Keyward —— 面向接入方](./integration.md)
- [本地完整走一遍](./walkthrough.md)
- [常见问题](./faq.md)
- [协议格式（SPEC）](../spec.md)
- [参考实现（IMPLEMENTATION）](../implementation.md)
- [路线图与现状 —— 做了什么、还差什么](../roadmap.md)

## 现状与当前限制

- **`v0`、不稳定** —— `v1` 前协议细节可能变。
- **Provider：** OpenAI Chat Completions、OpenAI Responses、Anthropic Messages
  （Chat-Completions 同时覆盖 OpenAI 兼容厂商）；Gemini / 工具调用 / 图片尚未验证。
- **尚未实现：** 预编译二进制、二维码配对、按用户的策略文件、重连时的单次 token 强制、你自己的
  serverless 模板、Orchestrator SDK / OpenAI 兼容代理。

发现核心承诺的漏洞——任何让应用拿到你 key 的方式？请私下报告（[SECURITY.md](../../SECURITY.md)）。
