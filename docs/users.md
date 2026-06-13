# Bring Your Own Key — for users / 面向用户 · 自带 Key

> You hold an API key and want an app to use it **without ever seeing it.** You run
> a small **Executor**; the app sends work to it; the key stays on your side.
> New to the model? Read the [docs index](./README.md) first.
>
> 你持有一把 API key，想让某个应用用它、却**永远看不到它**。你运行一个小小的 **Executor**；
> 应用把工作发给它；key 始终留在你这边。不熟悉模型？先读[文档索引](./README.md)。

## 1. Get Keyward / 获取 Keyward

Prebuilt binaries aren't published yet, so build from source (needs a recent Rust
toolchain). Enable the providers you'll actually use.

预编译二进制还没发布，所以先从源码构建（需要较新的 Rust 工具链）。按你实际要用的 Provider 开启 feature。

```sh
git clone https://github.com/gtoxlili/keyward && cd keyward
cargo build --release --features openai,anthropic
# the binary is now at ./target/release/keyward
```

## 2. Store your provider key / 保存你的 Provider Key

Put your key in the OS keychain — not a `.env` file. It's read from **stdin**, never
the command line (so it won't land in your shell history or `ps`).

把 key 放进操作系统钥匙串，而不是 `.env` 文件。它从 **stdin** 读取、绝不走命令行参数（因此不会进入
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
- Remove one with `keyward delete-key <provider>`.
- 一把 OpenAI 凭证同时服务 `openai` 和 `openai-responses` 两个面。
- 没有钥匙串（比如无头服务器）？退回环境变量：`OPENAI_API_KEY`、`ANTHROPIC_API_KEY`。
- 删除某个：`keyward delete-key <provider>`。

## 3. Pair with an app / 与应用配对

A Keyward-enabled app gives you a **pairing token** (today a code; a scannable QR is
on the roadmap) and a connection URL. Point your Executor at them — it **dials out**,
so you never open an inbound port.

支持 Keyward 的应用会给你一个**配对 token**（目前是一段码；可扫描的二维码在路线图上）和一个连接 URL。
把 Executor 指过去——它**主动拨出**，所以你不用开任何入站端口。

```sh
KEYWARD_ORCH_URL="wss://the-app.example.com/keyward" \
KEYWARD_PAIRING_TOKEN="pt_xxx_from_the_app" \
keyward executor
```

On pairing the Executor pins the app's **root identity key** (trust-on-first-use) and
prints its fingerprint. Compare it against the one the app shows — that's what stops
an impostor from binding even if your pairing token leaks.

配对时 Executor 会**钉住（pin）应用的根身份钥**（首次信任 TOFU）并打印其指纹。把它和应用展示的对一下
——这正是「即使配对 token 泄露，冒充者也绑不上」的关键。

## 4. What the Executor enforces for you / Executor 替你把的关

The app can't *steal* your key, but it could still *spend* it. So the Executor
enforces an **Owner policy** before every call, rejecting anything outside it *before*
the provider is contacted:

应用偷不走你的 key，却仍可能*花掉*它。所以 Executor 在每次调用前执行**你的策略**，并在*接触 Provider 之前*
就拒绝越界请求：

- **provider / model allow-lists** (model supports a trailing-`*` glob) / Provider 与模型白名单（模型支持尾部 `*` 通配）
- **budget** — a USD cap per window / 预算 —— 按窗口的美元上限
- **rate** — requests / tokens per minute / 速率 —— 每分钟请求数 / token 数
- **expiry** — auto-stop after a date / 过期 —— 到期自动停止

> **v0 note / v0 说明:** the CLI currently ships a built-in default policy (the
> providers from `KEYWARD_PROVIDERS`, any model, ~$5/month, 60 rpm). A per-Owner
> policy file is on the roadmap. / 目前 CLI 内置一套默认策略（来自 `KEYWARD_PROVIDERS`
> 的 Provider、任意模型、约 $5/月、60 rpm）。按用户自定义的策略文件在路线图上。

## 5. Verify it yourself / 自己验证

Don't take our word for it — point a proxy at the Executor and confirm your key
appears **only** on the call to the provider, never on the channel to the app.

别只信我们——拿个代理对着 Executor，确认你的 key**只**出现在发往 Provider 的请求里，从不出现在通往应用的通道上。

```sh
OPENAI_BASE_URL="http://127.0.0.1:8080/v1" \
KEYWARD_ORCH_URL="wss://…" KEYWARD_PAIRING_TOKEN="pt_…" \
keyward executor
# in the proxy you'll see `Authorization: Bearer sk-…` ONLY on the provider request.
```

## 6. Stop or revoke / 停止与吊销

The Executor only works while it's running. Close it (`Ctrl-C`) and all work stops
immediately — for interactive use, usually what you want. For autonomous / always-on
use, run it as a long-lived process on a box you own (your-own serverless templates
are on the roadmap).

Executor 只在运行时有效。关掉它（`Ctrl-C`），所有工作立刻停止——交互式使用时这通常正合你意。
对于自治 / 常驻使用，把它作为长驻进程跑在你自己的机器上（你自己的 serverless 模板在路线图上）。

---

See it all run: [a full local walkthrough](./walkthrough.md) · Questions: [FAQ](./faq.md)
· Back to the [docs index](./README.md).

看完整链路：[本地完整走一遍](./walkthrough.md) · 疑问：[常见问题](./faq.md) · 回到[文档索引](./README.md)。
