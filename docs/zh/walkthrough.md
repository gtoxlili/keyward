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

Node 用 HTTP 说 OpenAI 协议，所以任何 OpenAI 客户端都能驱动它。开两个终端，再像调用 OpenAI 那样 `curl` 它。

```sh
# 终端 1 —— Node（会合点），不持有 key。在 :8787 等 Client 拨入，并在 :8088 提供 OpenAI 兼容入口。
cargo run --features node -- node
# 打印：clients dial in on ws://127.0.0.1:8787  (pairing_token=pt_dev_token)

# 终端 2 —— Client（你）。构建时带上 openai 适配器；key 来自钥匙串 / 环境变量。
OPENAI_API_KEY=sk-... \
KEYWARD_NODE_URL=ws://127.0.0.1:8787 KEYWARD_PAIRING_TOKEN=pt_dev_token \
  cargo run --features openai -- client
```

现在像一个无感知的 OpenAI 应用那样，通过 Node 发起调用——它的 base URL 就是这个 Node，随便什么 bearer 都会
路由到你这唯一一个已配对的 Client：

```sh
# 终端 3 —— 无感知的「应用」：一次普通的 OpenAI 调用。
curl http://127.0.0.1:8088/v1/chat/completions \
  -H 'authorization: Bearer anything' \
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Say hi in 5 words."}]}'
```

Client 拨出并配对，钉住根钥、验证操作钥、检查策略，然后用你的 key 发起一次**真实**的 OpenAI 调用，再把
答案经由 Node 流式传回你的 curl。想用 Responses API？打 `/v1/responses`；Anthropic 则是 `/v1/messages`。

---

真正自带 key 上手：[写给用户](./users.md) · 把它集成进应用：[写给应用开发者](./integration.md) · 回到[文档索引](./README.md)。
