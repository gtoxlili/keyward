# 常见问题

🌐 [English](../en/faq.md) · **中文**

> 回到[文档索引](./README.md)。

**应用能读到我的原始 key 吗？**

不能——永远不能。它只发工作意图；key 在你的 Executor 内部装上。这是你能亲自验证的那个承诺
（见[自己验证](./users.md#5-自己验证)）。

**应用能看到我的提示词吗？**

能。Keyward 保护的是*凭证*，不是*内容*——提示词本就是应用构建和读取的。对应用隐藏内容是另一个、不在本范围内的问题。

**恶意应用会烧光我的预算吗？**

只能在你的限额内。「保管」不等于「控制」——这正是模型白名单、预算上限、速率限制、过期都放在 Executor、
且每次调用前都执行的原因。

**这不就是个代理 / LiteLLM 吗？**

不是。网关*持有*你的 key 并替你转发——这是托管式的，你在信任一台服务器。Keyward 的 Orchestrator 什么都不持有，
没有你这边活着的 Executor 它根本调不了。

**我关掉标签页 / 停掉 Executor 会怎样？**

所有工作立刻停止——没有活着的 Executor，Orchestrator 调不了 Provider。要做自治任务，就把 Executor 跑在你自己的机器上常驻。

**现在支持哪些 Provider？**

OpenAI Chat Completions、OpenAI Responses、Anthropic Messages（Chat-Completions 同时覆盖 OpenAI 兼容厂商）。
Gemini / 工具调用 / 图片尚未验证。见[现状](./README.md#现状与当前限制)。

---

发现核心承诺的漏洞？请私下报告（[SECURITY.md](../../SECURITY.md)）。
