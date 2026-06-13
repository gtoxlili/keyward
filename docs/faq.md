# FAQ / 常见问题

> Back to the [docs index](./README.md).

**Can the app read my raw key? / 应用能读到我的原始 key 吗？**

No — never. It only ever sends a work intent; the key is attached inside your
Executor. That's the one promise you can verify yourself (see
[Verify it yourself](./users.md#5-verify-it-yourself--自己验证)).

不能——永远不能。它只发工作意图；key 在你的 Executor 内部装上。这是你能亲自验证的那个承诺
（见[自己验证](./users.md#5-verify-it-yourself--自己验证)）。

**Can the app see my prompts? / 应用能看到我的提示词吗？**

Yes. Keyward protects the *credential*, not the *content* — the app builds and reads
the prompts by construction. Hiding content from the app is a different, out-of-scope
problem.

能。Keyward 保护的是*凭证*，不是*内容*——提示词本就是应用构建和读取的。对应用隐藏内容是另一个、不在本范围内的问题。

**Can a malicious app burn my budget? / 恶意应用会烧光我的预算吗？**

Only within your limits. Custody isn't control — that's exactly why model allow-lists,
budget caps, rate limits and expiry live in the Executor and run before every call.

只能在你的限额内。「保管」不等于「控制」——这正是模型白名单、预算上限、速率限制、过期都放在 Executor、
且每次调用前都执行的原因。

**Isn't this just a proxy / LiteLLM? / 这不就是个代理 / LiteLLM 吗？**

No. A gateway *holds* your key and forwards calls — custodial, you're trusting a
server. Keyward's Orchestrator holds nothing and literally cannot make a call without
a live Executor on your side.

不是。网关*持有*你的 key 并替你转发——这是托管式的，你在信任一台服务器。Keyward 的 Orchestrator 什么都不持有，
没有你这边活着的 Executor 它根本调不了。

**What if I close the tab / stop the Executor? / 我关掉标签页 / 停掉 Executor 会怎样？**

All work stops immediately — the Orchestrator can't call the provider without a live
Executor. For autonomous runs, keep the Executor running on a box you own.

所有工作立刻停止——没有活着的 Executor，Orchestrator 调不了 Provider。要做自治任务，就把 Executor 跑在你自己的机器上常驻。

**Which providers work today? / 现在支持哪些 Provider？**

OpenAI Chat Completions, the OpenAI Responses API, and Anthropic Messages
(Chat-Completions also covers OpenAI-compatible providers). Gemini / tool-use /
images aren't verified yet. See [status](./README.md#status--current-limits--现状与当前限制).

OpenAI Chat Completions、OpenAI Responses、Anthropic Messages（Chat-Completions 同时覆盖 OpenAI 兼容厂商）。
Gemini / 工具调用 / 图片尚未验证。见[现状](./README.md#status--current-limits--现状与当前限制)。

---

Found a hole in the core promise? Report it privately ([SECURITY.md](../SECURITY.md)).
发现核心承诺的漏洞？请私下报告（[SECURITY.md](../SECURITY.md)）。
