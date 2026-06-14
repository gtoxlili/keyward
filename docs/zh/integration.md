# 集成 Keyward——写给应用开发者

🌐 [English](../en/integration.md) · **中文**

> 大多数应用**根本不用集成**：用户只要把应用的 OpenAI base URL 指向一个 **Node**，剩下的交给 Keyward
> 就行（那是[用户指南](./users.md)讲的）。这篇是写给那些想*自己跑或内嵌*一个 Node 的人——无论哪种，你都不持有
> key。还不熟悉这套模型？先看[文档索引](./README.md)。

## 现在能用什么、还有什么在路上

**最省事，零改动（最常见的情形）：** 直接跑一个 **Node**——`keyward node`（用 `--features node` 构建）。
它就是一个 OpenAI 风格的 HTTP 端点，背后接着已经配对好的 Client；任何现成的、甚至完全无感知的应用，只要把
base URL 指过来就能接入：

```sh
keyward node   # 等一个 Client 配对完成，然后在 http://127.0.0.1:8088 提供服务
# 在你的应用里：  OPENAI_BASE_URL=http://127.0.0.1:8088/v1   OPENAI_API_KEY=anything
```

`/v1/chat/completions`、`/v1/responses`、`/v1/messages` 会按路径分发到对应的请求格式；流式响应原样转发，
所以你现有的 OpenAI SDK 不用改就能解析。key 始终留在 Client 上，应用里填的 `OPENAI_API_KEY` 会被忽略。

**想嵌进进程：** 用 **Node SDK** 把一个 Node 直接嵌入你的程序——Rust 用
[`keyward-sdk`](../../crates/keyward-sdk)，Go 用 [`sdk/go`](../../sdk/go)。两者用法一致：绑定一个监听端口，
用 `serve_one` / `ServeOne` 配对一个 Client，然后提交工作意图、流式拿回原生事件。（Go SDK 与 Rust
Client 在字节层面完全兼容，这一点在 CI 里做了跨语言验证。）

**传输层，WebSocket 或 gRPC 都行：** 协议本身与传输无关（spec §1）。Rust SDK 两种都能起——`serve_one`
走 WebSocket，`serve_one_grpc` 走 gRPC（用 `--features grpc` 构建）——而 Client 按 URL scheme 自动选择
（`ws://` / `wss://` 对应 WebSocket，`grpc://` / `grpcs://` 对应 gRPC）。即便走 gRPC，Client 依然是
主动拨出的一方，照样不需要开入站端口；通道之上的逻辑完全一样。

**想做到更底层：** 也可以直接基于 WebSocket 或 gRPC 双向流自己实现 `v0` 协议——完整契约见 [spec.md](../spec.md)，
而 `keyward node` 就是一份可读、可跑的参考实现。

## 消息流

配对一次，之后就能在同一个会话上发任意多个工作意图：

```
Client（用户侧）                             Node（会合点）
   │ ── hello (pairing_token, providers) ───────▶ │  校验 token
   │ ◀── paired (root_pubkey, op cert, sig) ───── │  证明身份、为 sid 签名
   │  钉住 root、验证证书链                          │
   │ ◀── work (provider, native request) ──────── │  发起一次 LLM 调用，不带 key
   │  校验策略 ✓、注入 key、调用 Provider            │
   │ ── work_chunk (seq, native delta) ─────────▶ │  转发给你的用户
   │ ── work_done (usage) ──────────────────────▶ │
```

- `work.request` 就是 Provider 的**原生**请求体，只是抹掉了凭证——OpenAI Chat Completions 用 `messages`，
  Responses API 用 `input`，`anthropic` 则是 Anthropic Messages 的结构。Client 原样转发，你拿回的也是
  原生 chunk，所以你现有的 Provider SDK 解析逻辑照常工作。
- 凭证只存在于 Client 之中——你既不会发出凭证，也不会收到凭证。
- 通道断开只是**挂起**，并不是失败：重连后用 `resume { mid, last_seq }` 把漏掉的 chunk 补回来，或用
  `cancel { mid }` 主动中止。

## 配对体验

生成一个**单次有效、短时效**的配对 token，像 WalletConnect 那样展示给用户——一串可粘贴的码，或者（路线图上的）
一个可扫描的二维码。Node 这边用一把长期的**根身份钥**来证明自己的身份；Client 首次接触时会把它钉住，因此之后
跨重连的密钥轮换、自动扩容都无需重新配对。记得把根钥的指纹展示出来，好让用户带外核对。

## 控制谁能绑定（保护你这一侧）

你也可以**反过来认证 Client**，只允许你注册过的用户绑定。每个 Client 都有一把固定的身份钥；用户运行
`keyward identity` 拿到自己的公钥，注册时登记给你。之后你只放行白名单内的：每个 `hello` 都带着 Client 的
`pubkey` 以及对配对 token 的签名，不在白名单里的一律拒绝。

这道门禁保护的是**你**的利益（谁能用你的应用），并不触及用户那一侧——它纯粹是「谁能绑定」的关卡。它**不会**替你
对用户隐藏提示词或 key：在 BYOK 模式下，Client 是用户自己在跑，他们随时都能检查自己的流量（这本就是 Keyward
的意义所在）；而且谁给 Provider 请求装上凭证，谁就必然看得见这个请求。如果你的需求是对**用户**隐藏 payload，
那 BYOK 根本就是错的模型——那得靠服务端 / TEE 来执行。

## 用 Docker 部署

仓库里自带了一个 [`Dockerfile`](../../Dockerfile)——同一个镜像，可以扮演多种服务角色（构建时已包含 node 角色、
两种 Provider 请求格式和 gRPC）。默认命令就是 `node`：

```sh
docker build -t keyward .
# OpenAI 兼容网关：:8088 是给你应用用的 HTTP 前端，:8787 是 Owner 的 Client 拨入的端口。
# 容器内这两个都绑在 0.0.0.0 上。
docker run -p 8088:8088 -p 8787:8787 keyward
#   在你的应用里：  OPENAI_BASE_URL=http://<host>:8088/v1   OPENAI_API_KEY=anything
```

换个命令就能切到 Client 角色——在 Owner 的机器上跑一个常驻 Client，把 key 以环境变量机密的形式传进去：

```sh
docker run -e KEYWARD_NODE_URL=grpc://node.example.com:443 \
           -e KEYWARD_PAIRING_TOKEN=pt_... -e OPENAI_API_KEY=sk-... keyward client
```

镜像以非 root 用户运行，也不需要任何构建期的机密。监听地址可以用 `KEYWARD_HTTP_LISTEN` / `KEYWARD_LISTEN`
调整。

---

本地试一下：[完整跑一遍](./walkthrough.md) · 读协议格式：[spec.md](../spec.md) · 回到[文档索引](./README.md)。
