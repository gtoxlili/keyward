# Keyward 文档

🌐 [English](../en/README.md) · **中文**

> 第一次来？先花一分钟看懂下面这套模型，再按你的角色往下读。协议细节见 [spec.md](../spec.md)，
> 参考实现见 [implementation.md](../implementation.md)。

## 一分钟看懂这套模型

Keyward 把一个 AI 应用拆成「大脑」和「双手」两半：

- **Orchestrator（编排端）**——也就是应用 / SaaS 本身，相当于「大脑」。它负责决定*做什么*
  （写提示词、跑 agent 循环），但**永远不持有你的 key**。
- **Executor（执行器）**——一个*由你*在自己信任边界内运行的小程序，相当于「双手」。key 由它保管，
  你设的限额由它执行，最后也由它去真正调用 Provider。
- **Provider**——OpenAI、Anthropic 这些模型厂商。

应用只把一个**工作意图**（也就是一份不含 key 的请求）发给你的 Executor；Executor 在本地补上 key、
调用 Provider，再把结果流式传回。key 自始至终不会进入应用一侧。如果你用过 WalletConnect，那 Keyward
就是「把同一套思路用在 API key 上」。

## 你是哪一方？

| 你的身份 | 你运行的是 | 从这里开始 |
| --- | --- | --- |
| 手里有 API key 的最终用户 | **Executor** | [自带 Key](./users.md) |
| 做应用 / SaaS 的开发者 | **Orchestrator** | [集成 Keyward](./integration.md) |

## 全部文档

- [自带 Key——写给用户](./users.md)
- [集成 Keyward——写给应用开发者](./integration.md)
- [在本地完整跑一遍](./walkthrough.md)
- [常见问题](./faq.md)
- [协议格式（SPEC）](../spec.md)
- [参考实现（IMPLEMENTATION）](../implementation.md)
- [路线图与现状——已经做了什么、还差什么](../roadmap.md)

## 现状与当前限制

- **`v0`，尚不稳定**——`v1` 之前协议细节随时可能调整。
- **已支持的 Provider：** OpenAI Chat Completions、OpenAI Responses、Anthropic Messages
  （其中 Chat Completions 也顺带覆盖了所有 OpenAI 兼容厂商）；Gemini、工具调用、图片暂未验证。
- **尚未实现：** 预编译二进制、二维码配对、可自部署的 serverless Executor 模板、浏览器 / WASM
  Executor、字节级可复现构建，以及更多 Provider（Gemini 等）与多模态（工具调用 / 图片）支持。

发现了核心承诺上的漏洞——任何能让应用拿到你 key 的途径——请私下报告（[SECURITY.md](../../SECURITY.md)）。
