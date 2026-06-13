# 自带 Key —— 面向用户

🌐 [English](../en/users.md) · **中文**

> 你持有一把 API key，想让某个应用用它、却**永远看不到它**。你运行一个小小的 **Executor**；应用把工作发给它；
> key 始终留在你这边。不熟悉模型？先读[文档索引](./README.md)。

## 1. 获取 Keyward

预编译二进制还没发布，所以先从源码构建（需要较新的 Rust 工具链）。按你实际要用的 Provider 开启 feature。

```sh
git clone https://github.com/gtoxlili/keyward && cd keyward
cargo build --release --features openai,anthropic
# 二进制现在位于 ./target/release/keyward
```

## 2. 保存你的 Provider Key

把 key 放进操作系统钥匙串，而不是 `.env` 文件。它从 **stdin** 读取、绝不走命令行参数（因此不会进入
shell 历史或 `ps`）。

```sh
keyward set-key openai        # 然后粘贴 key、回车
# 或用管道：
echo "sk-..." | keyward set-key openai
echo "sk-ant-..." | keyward set-key anthropic
```

- 一把 OpenAI 凭证同时服务 `openai` 和 `openai-responses` 两个面。
- 没有钥匙串（比如无头服务器）？退回环境变量：`OPENAI_API_KEY`、`ANTHROPIC_API_KEY`。
- 删除某个：`keyward delete-key <provider>`。

## 3. 与应用配对

支持 Keyward 的应用会给你一个**配对 token**（目前是一段码；可扫描的二维码在路线图上）和一个连接 URL。
把 Executor 指过去——它**主动拨出**，所以你不用开任何入站端口。

```sh
KEYWARD_ORCH_URL="wss://the-app.example.com/keyward" \
KEYWARD_PAIRING_TOKEN="pt_xxx_from_the_app" \
keyward executor
```

配对时 Executor 会**钉住（pin）应用的根身份钥**（首次信任 TOFU）并打印其指纹。把它和应用展示的对一下
——这正是「即使配对 token 泄露，冒充者也绑不上」的关键。

有些应用只接受注册过的用户。如果是这样，跑 `keyward identity` 打印你 Executor 的 pubkey，在注册时交给应用
——连接时你的 Executor 会证明自己握有这把钥。

## 4. Executor 替你把的关

应用偷不走你的 key，却仍可能*花掉*它。所以 Executor 在每次调用前执行**你的策略（Owner policy）**，
并在*接触 Provider 之前*就拒绝越界请求：

- **Provider / 模型白名单**（模型支持尾部 `*` 通配）
- **预算** —— 按窗口的美元上限
- **速率** —— 每分钟请求数 / token 数
- **过期** —— 到期自动停止

> **v0 说明：** 目前 CLI 内置一套默认策略（来自 `KEYWARD_PROVIDERS` 的 Provider、任意模型、约 $5/月、
> 60 rpm）。按用户自定义的策略文件在路线图上。

## 5. 自己验证

别只信我们——拿个代理对着 Executor，确认你的 key**只**出现在发往 Provider 的请求里，从不出现在通往应用的通道上。

```sh
OPENAI_BASE_URL="http://127.0.0.1:8080/v1" \
KEYWARD_ORCH_URL="wss://…" KEYWARD_PAIRING_TOKEN="pt_…" \
keyward executor
# 在代理里你会看到 `Authorization: Bearer sk-…` 只出现在发往 Provider 的请求上。
```

## 6. 停止与吊销

Executor 只在运行时有效。关掉它（`Ctrl-C`），所有工作立刻停止——交互式使用时这通常正合你意。对于自治 /
常驻使用，把它作为长驻进程跑在你自己的机器上（你自己的 serverless 模板在路线图上）。

---

看完整链路：[本地完整走一遍](./walkthrough.md) · 疑问：[常见问题](./faq.md) · 回到[文档索引](./README.md)。
