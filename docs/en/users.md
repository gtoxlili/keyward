# Bring Your Own Key — for users

🌐 **English** · [中文](../zh/users.md)

> You hold an API key and want an app to use it **without ever seeing it.** You run a
> small **Client**; the app sends work to it; the key stays on your side. New to the
> model? Read the [docs index](./README.md) first.

## 1. Get Keyward

Prebuilt binaries aren't published yet, so build from source (needs a recent Rust
toolchain). Enable the providers you'll actually use.

```sh
git clone https://github.com/gtoxlili/keyward && cd keyward
cargo build --release --features openai,anthropic
# the binary is now at ./target/release/keyward
```

## 2. Store your provider key

Put your key in the OS keychain — not a `.env` file. It's read from **stdin**, never
the command line (so it won't land in your shell history or `ps`).

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

## 3. Pair with an app

A Keyward-enabled app gives you a **pairing token** (today a code; a scannable QR is
on the roadmap) and a connection URL. Point your Client at them — it **dials out**,
so you never open an inbound port.

```sh
KEYWARD_NODE_URL="wss://the-app.example.com/keyward" \
KEYWARD_PAIRING_TOKEN="pt_xxx_from_the_app" \
keyward client
```

On pairing the Client pins the app's **root identity key** (trust-on-first-use) and
prints its fingerprint. Compare it against the one the app shows — that's what stops
an impostor from binding even if your pairing token leaks.

Some apps only admit registered users. If so, run `keyward identity` to print your
Client's pubkey and give it to the app at sign-up — your Client proves it holds
that key when it connects.

## 4. What the Client enforces for you

The app can't *steal* your key, but it could still *spend* it. So the Client
enforces an **Owner policy** before every call, rejecting anything outside it *before*
the provider is contacted:

- **provider / model allow-lists** (model supports a trailing-`*` glob)
- **budget** — a USD cap per window
- **rate** — requests / tokens per minute
- **expiry** — auto-stop after a date

> **Configure it:** point `KEYWARD_POLICY` at a JSON policy file (see
> [policy.example.json](../policy.example.json)) — provider/model allow-lists, USD
> budget, rate, expiry. Without it, the CLI uses a built-in default (the
> `KEYWARD_PROVIDERS`, any model, ~$5/month, 60 rpm).

## 5. Verify it yourself

Don't take our word for it — route the Client's provider calls through an intercepting
proxy and confirm your key appears **only** on the call to the provider, never on the
channel to the node.

```sh
# 1. start an intercepting proxy on :8080 (mitmproxy, or any logging proxy)
mitmproxy -p 8080 &
# 2. run the Client with its provider calls pointed through that proxy
OPENAI_BASE_URL="http://127.0.0.1:8080/v1" \
KEYWARD_NODE_URL="wss://…" KEYWARD_PAIRING_TOKEN="pt_…" \
keyward client
# in the proxy you'll see `Authorization: Bearer sk-…` ONLY on the provider request.
```

## 6. Stop or revoke

The Client only works while it's running. Close it (`Ctrl-C`) and all work stops
immediately — for interactive use, usually what you want. For autonomous / always-on
use, run it as a long-lived process on a box you own (your-own serverless templates
are on the roadmap).

---

See it all run: [a full local walkthrough](./walkthrough.md) · Questions: [FAQ](./faq.md)
· Back to the [docs index](./README.md).
