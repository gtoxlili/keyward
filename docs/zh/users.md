# 自带 Key——写给用户

🌐 [English](../en/users.md) · **中文**

> 你手里有一把 API key，想让某个应用拿它干活，却**不想让应用看到这把 key**。做法是：你跑一个小小的
> **Client**，应用把活儿发给它，key 始终留在你这边。还不熟悉这套模型？先看[文档索引](./README.md)。

## 1. 获取 Keyward

预编译二进制还没发布，所以目前要从源码构建（需要一个较新的 Rust 工具链）。按你实际要用的 Provider
开启对应 feature 即可。

```sh
git clone https://github.com/gtoxlili/keyward && cd keyward
cargo build --release --features openai,anthropic
# 构建好的二进制在 ./target/release/keyward
```

## 2. 存好你的 Provider Key

把 key 交给操作系统的钥匙串保管，而不是写进 `.env` 文件。它从 **stdin** 读取，绝不经过命令行参数
（所以不会落进 shell 历史，也不会出现在 `ps` 里）。

```sh
keyward set-key openai        # 回车后粘贴 key，再回车
# 或者用管道喂进去：
echo "sk-..." | keyward set-key openai
echo "sk-ant-..." | keyward set-key anthropic
```

- 一把 OpenAI 凭证可以同时用于 `openai` 和 `openai-responses` 两个接口。
- 没有钥匙串（比如无头服务器）？退回到环境变量：`OPENAI_API_KEY`、`ANTHROPIC_API_KEY`。
- 想删掉某个：`keyward delete-key <provider>`。

## 3. 与应用配对

支持 Keyward 的应用会给你一个**配对 token**（目前是一串码，可扫描的二维码还在路线图上）和一个连接地址。
让你的 Client 连过去就行——它是**主动拨出**的，所以你这边不用开任何入站端口。

```sh
KEYWARD_NODE_URL="wss://the-app.example.com/keyward" \
KEYWARD_PAIRING_TOKEN="pt_xxx_from_the_app" \
keyward client
```

配对时，Client 会**钉住（pin）应用的根身份钥**（首次信任，即 TOFU），并打印出它的指纹。把这个指纹和
应用界面上展示的核对一下——正是这一步保证了「就算配对 token 泄露，冒充者也绑不上来」。

有些应用只接受注册过的用户。遇到这种情况，运行 `keyward identity` 打印出你 Client 的公钥，注册时交给应用；
之后连接时，你的 Client 会证明自己确实握有这把钥。

## 4. Client 替你把关

应用偷不走你的 key，但仍然可能*花掉*它。所以每次调用之前，Client 都会执行**你定下的策略（Owner policy）**，
在请求*真正发往 Provider 之前*就把越界的挡下来：

- **Provider / 模型白名单**（模型名支持结尾的 `*` 通配）
- **预算**——按时间窗口设定的美元上限
- **速率**——每分钟的请求数 / token 数
- **有效期**——到期后自动停止

> **怎么配：** 把 `KEYWARD_POLICY` 指向一个 JSON 策略文件（见
> [policy.example.json](../policy.example.json)），即可设定 Provider / 模型白名单、美元预算、速率和有效期。
> 不配的话，CLI 会用一套内置默认值（`KEYWARD_PROVIDERS` 指定的 Provider、任意模型、约 $5/月、60 rpm）。

## 5. 自己验证

别光听我们说——把 Client 发往 Provider 的调用穿过一个抓包代理，亲眼确认你的 key**只**出现在发往
Provider 的请求里，从不出现在通往节点的那条通道上。

```sh
# 1. 在 :8080 起一个抓包代理（mitmproxy，或任意会记录请求的代理）
mitmproxy -p 8080 &
# 2. 让 Client 的 Provider 调用经过这个代理
OPENAI_BASE_URL="http://127.0.0.1:8080/v1" \
KEYWARD_NODE_URL="wss://…" KEYWARD_PAIRING_TOKEN="pt_…" \
keyward client
# 在代理里你会看到，`Authorization: Bearer sk-…` 只出现在发往 Provider 的请求上。
```

## 6. 停止与吊销

Client 只在运行时才有效。把它关掉（`Ctrl-C`），所有活儿立刻全停——交互式使用时，这通常正合你意。
如果是自治 / 常驻场景，就把它作为长驻进程跑在你自己的机器上（可自部署的 serverless 模板还在路线图上）。

---

看完整链路：[在本地完整跑一遍](./walkthrough.md) · 有疑问：[常见问题](./faq.md) · 回到[文档索引](./README.md)。
