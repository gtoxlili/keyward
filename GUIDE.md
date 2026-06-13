# Keyward — Usage & Integration Guide / 使用与接入指南

> Bilingual (English first, 中文在后). This guide is for the two people who touch
> Keyward in practice: the **Owner** who brings their own key, and the **app
> builder** who integrates it. For the protocol bytes see [SPEC.md](./SPEC.md); for
> the reference code see [IMPLEMENTATION.md](./IMPLEMENTATION.md).
>
> 中英双语（先英文，后中文）。本指南面向实际接触 Keyward 的两类人：**自带 Key 的用户（Owner）**
> 和**集成它的应用方（接入方）**。协议细节见 [SPEC.md](./SPEC.md)，参考实现见
> [IMPLEMENTATION.md](./IMPLEMENTATION.md)。
>
> **Status / 现状:** `v0`, unstable. The reference Executor runs today; a few
> convenience pieces (one-click installer, QR pairing, a per-Owner policy file,
> a drop-in SDK) are still on the roadmap and called out below.
> `v0`、不稳定。参考 Executor 现在就能跑；少数便利件（一键安装、二维码配对、按用户的策略文件、
> 即插即用 SDK）仍在路线图上，文中会标注。

---

## The model in one minute / 一分钟理解模型

Keyward splits an AI app into a **brain** and a pair of **hands**:

- **Orchestrator** — the app / SaaS. Decides *what* to do (prompts, the agent loop).
  It **never holds your key.**
- **Executor** — a small program *you* run, inside your own trust boundary. It holds
  the key, enforces your limits, and makes the actual call to the provider.
- **Provider** — OpenAI, Anthropic, etc.

The app sends a *work intent* (the request minus any key) to your Executor; your
Executor attaches the key locally and streams the result back. The key never
travels to the app. If you've used WalletConnect, this is that idea for API keys.

Keyward 把一个 AI 应用拆成**大脑**和一双**手**：

- **Orchestrator（编排端）** —— 应用 / SaaS。决定*做什么*（提示词、agent 循环），**永远不持有你的 key**。
- **Executor（执行器）** —— *你*在自己信任边界内运行的小程序。它持有 key、执行你的限额、真正去调 Provider。
- **Provider** —— OpenAI、Anthropic 等。

应用把一个*工作意图（work intent，即去掉 key 的请求）*发给你的 Executor；Executor 在本地把 key 装上、
调用 Provider，再把结果流式传回。Key 从不传到应用侧。用过 WalletConnect 的话，这就是「API key 版」的它。

| You are… / 你是 | You run / 你运行 | This guide / 看本指南 |
| --- | --- | --- |
| An end user with an API key / 持有 API key 的最终用户 | the **Executor** | Part 1 / 第一部分 |
| An app / SaaS builder / 应用 / SaaS 开发者 | the **Orchestrator** | Part 2 / 第二部分 |

---

## Part 1 · For users — Bring Your Own Key / 第一部分 · 面向用户（自带 Key）

### 1. Get Keyward / 获取 Keyward

Prebuilt binaries aren't published yet, so build from source (needs a recent Rust
toolchain). Enable the providers you'll actually use.

预编译二进制还没发布，所以先从源码构建（需要较新的 Rust 工具链）。按你实际要用的 Provider 开启 feature。

```sh
git clone https://github.com/gtoxlili/keyward && cd keyward
cargo build --release --features openai,anthropic
# the binary is now at ./target/release/keyward
```

### 2. Store your provider key / 保存你的 Provider Key

Put your key in the OS keychain — not a `.env` file. The key is read from **stdin**,
never the command line (so it won't land in your shell history or `ps`).

把 key 放进操作系统钥匙串，而不是 `.env` 文件。Key 从 **stdin** 读取、绝不走命令行参数（因此不会进入
shell 历史或 `ps`）。

```sh
keyward set-key openai        # then paste the key and press Enter
# or pipe it:
echo "sk-..." | keyward set-key openai
echo "sk-ant-..." | keyward set-key anthropic
```

- One credential serves both OpenAI surfaces (`openai` and `openai-responses`).
- No keychain (e.g. a headless server)? Fall back to env vars: `OPENAI_API_KEY`,
  `ANTHROPIC_API_KEY`.
- 一把 OpenAI 凭证同时服务 `openai` 和 `openai-responses` 两个面。
- 没有钥匙串（比如无头服务器）？退回环境变量：`OPENAI_API_KEY`、`ANTHROPIC_API_KEY`。

### 3. Pair with an app / 与应用配对

A Keyward-enabled app gives you a **pairing token** (today a code; a scannable QR is
on the roadmap) and a connection URL. You point your Executor at them — the Executor
**dials out**, so you never open an inbound port.

支持 Keyward 的应用会给你一个**配对 token**（目前是一段码；可扫描的二维码在路线图上）和一个连接 URL。
你把 Executor 指过去——Executor **主动拨出**，所以你不用开任何入站端口。

```sh
KEYWARD_ORCH_URL="wss://the-app.example.com/keyward" \
KEYWARD_PAIRING_TOKEN="pt_xxx_from_the_app" \
keyward executor
```

On pairing the Executor pins the app's **root identity key** (trust-on-first-use)
and prints its fingerprint. Compare that fingerprint against the one the app shows —
that's what stops an impostor from binding even if your pairing token leaks.

配对时 Executor 会**钉住（pin）应用的根身份钥**（首次信任 TOFU）并打印其指纹。把这个指纹和应用展示的对一下
——这正是「即使配对 token 泄露，冒充者也绑不上」的关键。

### 4. What the Executor enforces for you / Executor 替你把的关

Even though the app can't *steal* your key, it could still *spend* it. So the
Executor enforces an **Owner policy** before every call — and rejects anything
outside it *before* the provider is ever contacted:

应用虽然*偷不走*你的 key，却仍可能*花掉*它。所以 Executor 在每次调用前执行**你的策略（Owner policy）**，
并在*接触 Provider 之前*就拒绝越界请求：

- **provider / model allow-lists** (model supports a trailing-`*` glob) / Provider 与模型白名单（模型支持尾部 `*` 通配）
- **budget** — a USD cap per window / 预算 —— 按窗口的美元上限
- **rate** — requests- and tokens-per-minute / 速率 —— 每分钟请求数 / token 数
- **expiry** — auto-stop after a date / 过期 —— 到期自动停止

> **v0 note / v0 说明:** the CLI currently ships a built-in default policy (the
> providers from `KEYWARD_PROVIDERS`, any model, ~$5/month, 60 rpm). A per-Owner
> policy file is on the roadmap. / 目前 CLI 内置一套默认策略（来自 `KEYWARD_PROVIDERS`
> 的 Provider、任意模型、约 $5/月、60 rpm）。按用户自定义的策略文件在路线图上。

### 5. Verify it yourself / 自己验证

Don't take our word for it — point a proxy at the Executor and confirm your key
appears **only** on the call to the provider, never on the channel to the app.

别只信我们——拿个代理对着 Executor，确认你的 key**只**出现在发往 Provider 的请求里，从不出现在通往应用的通道上。

```sh
# run the Executor with provider traffic going through a logging proxy
OPENAI_BASE_URL="http://127.0.0.1:8080/v1" \
KEYWARD_ORCH_URL="wss://…" KEYWARD_PAIRING_TOKEN="pt_…" \
keyward executor
# in the proxy you'll see `Authorization: Bearer sk-…` ONLY on the provider request.
```

### 6. Stop or revoke / 停止与吊销

The Executor only works while it's running. Close it (`Ctrl-C`) and all work stops
immediately — for interactive use, that's usually what you want. For autonomous /
always-on use, run it as a long-lived process on a box you own (your own serverless
templates are on the roadmap).

Executor 只在运行时有效。关掉它（`Ctrl-C`），所有工作立刻停止——交互式使用时这通常正合你意。
对于自治 / 常驻使用，把它作为长驻进程跑在你自己的机器上（你自己的 serverless 模板在路线图上）。

---

## Part 2 · For app builders — Integrate Keyward / 第二部分 · 面向接入方（集成 Keyward）

You are the **Orchestrator**. You hold no key. Your job: issue a pairing token,
accept the Executor's dial-out, send work intents, and relay the streamed result to
your user.

你是 **Orchestrator**。你不持有 key。你的职责：签发配对 token、接受 Executor 的拨入、发送工作意图、
把流式结果转给你的用户。

### Today vs. roadmap / 现状与路线

**Today / 现在:** you integrate by speaking the `v0` wire protocol over a
bidirectional channel (a WebSocket). The whole contract is in [SPEC.md](./SPEC.md),
and `keyward orchestrator` is a working reference you can read and run.

**现在:** 通过一条双向通道（WebSocket）讲 `v0` 协议来集成。完整契约在 [SPEC.md](./SPEC.md)，
`keyward orchestrator` 是一个可读、可跑的参考实现。

**Roadmap / 路线图:** a drop-in **Orchestrator SDK** (swap your provider client for
one line) and a local **OpenAI-compatible proxy** (point any existing app at it by
changing `OPENAI_BASE_URL`) so you integrate with near-zero code change.

**路线图:** 即插即用的 **Orchestrator SDK**（把你的 Provider 客户端换一行）和本地
**OpenAI 兼容代理**（任何现存应用改个 `OPENAI_BASE_URL` 即可接入），让你几乎零改动集成。

### The message flow / 消息流

One pairing, then any number of work intents over the same session:

一次配对，之后在同一会话上发任意多个工作意图：

```
Executor (user side)                          Orchestrator (your app)
   │ ── hello (pairing_token, providers) ───────▶ │  verify token
   │ ◀── paired (root_pubkey, op cert, sig) ───── │  prove identity, sign sid
   │  pin root, verify chain                       │
   │ ◀── work (provider, native request) ──────── │  send an LLM call, no key
   │  check policy ✓, inject key, call provider    │
   │ ── work_chunk (seq, native delta) ─────────▶ │  relay to your user
   │ ── work_done (usage) ──────────────────────▶ │
```

- `work.request` is the provider's **native** body, minus any credential
  (`messages` for OpenAI Chat Completions, `input` for the Responses API, the
  Anthropic Messages shape for `anthropic`). The Executor passes it through; you get
  native chunks back, so your existing provider-SDK parsing keeps working.
- The credential lives only in the Executor — you never send one and never receive one.

- `work.request` 就是 Provider 的**原生** body、去掉凭证（OpenAI Chat Completions 用 `messages`，
  Responses API 用 `input`，`anthropic` 用 Anthropic Messages 形状）。Executor 原样转发；
  你拿回的是原生 chunk，所以你现有的 Provider-SDK 解析照常工作。
- 凭证只活在 Executor 里——你既不发送、也不接收任何凭证。

### Pairing UX / 配对体验

Generate a **single-use, short-lived** pairing token and show it to your user the
WalletConnect way — a code to paste or (roadmap) a QR to scan. Authenticate yourself
with a long-lived **root identity key**; the Executor pins it on first contact, so
key rotation / autoscaling across reconnects needs no re-pairing.

生成一个**单次、短时效**的配对 token，用 WalletConnect 的方式展示给用户——一段可粘贴的码，或（路线图）
一个可扫描的二维码。用一把长期的**根身份钥**证明自己；Executor 首次接触时把它钉住，因此跨重连的密钥轮换 /
自动扩容都无需重新配对。

---

## Part 3 · A full local walkthrough / 第三部分 · 本地完整走一遍

See the whole chain on one machine. Open two terminals.

在一台机器上看完整链路。开两个终端。

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
back to Terminal 1.

终端 2 拨出、配对，Executor 钉住根钥并验证操作钥、检查策略、用你的 key 发起**真实**的 OpenAI 调用，
把答案流式传回终端 1。

No key and no network? Just run `cargo run -- demo` — it stands up both ends with a
mock provider and streams three native dialects (OpenAI Chat Completions, OpenAI
Responses, Anthropic Messages) plus a policy-blocked call.

没有 key、没有网络？直接 `cargo run -- demo`——它用 mock provider 把两端都立起来，流式演示三种原生方言
（OpenAI Chat Completions、OpenAI Responses、Anthropic Messages）外加一个被策略拦下的调用。

---

## FAQ / 常见问题

**Can the app read my raw key? / 应用能读到我的原始 key 吗？**
No — never. It only ever sends a work intent; the key is attached inside your
Executor. That's the one promise you can verify yourself (Part 1 §5).
不能——永远不能。它只发工作意图；key 在你的 Executor 内部装上。这是你能亲自验证的那个承诺（第一部分 §5）。

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

---

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
it privately ([SECURITY.md](./SECURITY.md)).
发现核心承诺的漏洞——任何让应用拿到你 key 的方式？请私下报告（[SECURITY.md](./SECURITY.md)）。
