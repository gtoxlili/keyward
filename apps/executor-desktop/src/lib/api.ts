import { invoke, Channel } from "@tauri-apps/api/core";
import type { Lang } from "../i18n";

/** Mirrors `keyward::executor::ExecutorEvent` (internally tagged on `kind`). */
export type ExecutorEvent =
  | { kind: "connecting"; url: string }
  | { kind: "paired"; orchestrator: string; rootFingerprint: string; sid: string }
  | { kind: "accepted"; mid: string; provider: string; model: string }
  | {
      kind: "done";
      mid: string;
      provider: string;
      model: string;
      inputTokens: number;
      outputTokens: number;
      costUsd: number;
      spentUsd: number;
    }
  | { kind: "denied"; mid: string; provider: string; model: string; code: string }
  | { kind: "workFailed"; mid: string; code: string; message: string }
  | { kind: "connectionLost"; pending: number }
  | { kind: "reconnecting"; attempt: number }
  | { kind: "stopped"; reason: string };

export type ExecutorEventKind = ExecutorEvent["kind"];

export type StartConfig = {
  orchUrl: string;
  pairingToken: string;
  providers: string[];
  budgetUsd?: number | null;
  rpm?: number | null;
  expectedRootFp?: string | null;
};

export type Identity = { pubkey: string; fingerprint: string };
export type KeyStatus = { provider: string; present: boolean };

/** Persisted UI settings (the backend stores this blob opaquely). */
export type Settings = {
  lang: Lang;
  theme: "dark" | "light";
  orchUrl: string;
  providers: string[];
  budgetUsd: number | null;
  rpm: number | null;
  expectedRootFp: string;
};

export const api = {
  getIdentity: () => invoke<Identity>("get_identity"),
  setKey: (provider: string, key: string) => invoke<void>("set_key", { provider, key }),
  deleteKey: (provider: string) => invoke<void>("delete_key", { provider }),
  keyStatus: (providers: string[]) => invoke<KeyStatus[]>("key_status", { providers }),
  startExecutor(config: StartConfig, onEvent: (e: ExecutorEvent) => void) {
    const channel = new Channel<ExecutorEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("start_executor", { config, onEvent: channel });
  },
  stopExecutor: () => invoke<void>("stop_executor"),
  loadSettings: () => invoke<Partial<Settings> | null>("load_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
};
