# Keyward Executor — desktop app

A modern, bilingual (English / 中文) desktop app for the Keyward **Executor** — the
non-custodial side that runs on your machine, holds your API keys, enforces your
policy, and routes work from an Orchestrator to your keys. **The key never leaves.**

It drives the *real* executor core (the `keyward` crate) — no reimplementation. Status
streams back over a Tauri IPC channel into a live dashboard.

## Screens

- **Dashboard** — connection status, a live "Orchestrator → Executor" routing line,
  session spend / requests / rate, and an activity log.
- **Pairing** — dial out to an orchestrator (URL + one-time token), with your
  allow-listable identity fingerprint + pubkey. Optional out-of-band root-fingerprint pin.
- **Keys** — store / remove provider credentials in the OS keychain. They never cross
  the wire or reach the UI.
- **Policy** — allowed providers, monthly USD budget, requests-per-minute.
- **Settings** — language, theme.

## Develop

```sh
npm install
npm run tauri dev      # hot-reloading dev app
```

## Build

```sh
npm run tauri build    # produces a native bundle in src-tauri/target/release/bundle/
```

`protoc` is **not** required (the desktop build excludes the gRPC transport; ws:// /
wss:// orchestrators are supported).

## Stack

Tauri 2 (Rust backend) · React 19 + TypeScript + Vite · **vanilla-extract** (zero-runtime
CSS-in-TS) + the **React Compiler** · lightningcss minification · CSS-only motion · fonts
vendored via `@fontsource` (offline). The backend depends on the workspace `keyward` crate
at `../../crates/keyward`.
