# 常见问题

🌐 [English](../en/faq.md) · **中文**

> 回到[文档索引](./README.md)。

**应用能读到我的原始 key 吗？**

不能，永远不能。它发出去的只是工作意图；key 是在你的 Executor 内部才补上的。这正是你可以亲自验证的那个承诺
（见[自己验证](./users.md#5-自己验证)）。

**应用能看到我的提示词吗？**

能。Keyward 保护的是*凭证*，不是*内容*——提示词本来就是应用自己构建、自己读取的。对应用隐藏内容是另一个问题，
不在 Keyward 的范围之内。

**恶意应用会把我的预算烧光吗？**

只能在你设定的限额之内折腾。「保管」不等于「控制」——正因如此，模型白名单、预算上限、速率限制、有效期才全都
放在 Executor 上，而且每次调用前都会执行一遍。

**这不就是个代理 / LiteLLM 吗？**

不是。网关是*持有*你的 key、替你转发请求——那是托管式的，你又回到了「信任一台服务器」。而 Keyward 的
Orchestrator 什么都不持有，只要你这边没有一个运行中的 Executor，它就根本发不出任何调用。

**我关掉标签页 / 停掉 Executor 会怎样？**

所有活儿立刻全停——没有运行中的 Executor，Orchestrator 就调不动 Provider。要跑自治任务，就把 Executor
常驻在你自己的机器上。

**现在支持哪些 Provider？**

OpenAI Chat Completions、OpenAI Responses、Anthropic Messages（其中 Chat Completions 也覆盖了 OpenAI
兼容厂商）。Gemini、工具调用、图片暂未验证。详见[现状](./README.md#现状与当前限制)。

---

发现了核心承诺上的漏洞？请私下报告（[SECURITY.md](../../SECURITY.md)）。
