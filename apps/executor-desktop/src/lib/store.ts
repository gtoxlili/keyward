import { createContext, useContext } from "react";
import type { ClientEvent, Identity, Settings } from "./api";

export type Status = "idle" | "connecting" | "connected" | "error";
export type LogEntry = ClientEvent & { id: number; at: number };

export type AppState = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  identity: Identity | null;
  status: Status;
  events: LogEntry[];
  /** Start the client with a one-time pairing token (kept out of persisted state). */
  start: (pairingToken: string) => Promise<void>;
  stop: () => Promise<void>;
  toast: (msg: string, tone?: "ok" | "bad") => void;
};

export const AppContext = createContext<AppState | null>(null);

export const useApp = (): AppState => {
  const v = useContext(AppContext);
  if (!v) throw new Error("useApp must be used within AppProvider");
  return v;
};

/** Live stats derived from the recent event log. */
export function deriveStats(events: LogEntry[]) {
  const now = Date.now();
  let spentUsd = 0;
  let node: string | null = null;
  let rootFp: string | null = null;
  const accepted = new Set<string>();
  const terminal = new Set<string>();
  let served = 0;
  let recentAccepts = 0;

  // events are newest-first
  for (const e of events) {
    if (e.kind === "paired" && !node) {
      node = e.node;
      rootFp = e.rootFingerprint;
    }
    if (e.kind === "done") {
      served++;
      if (spentUsd === 0) spentUsd = e.spentUsd;
      terminal.add(e.mid);
    }
    if (e.kind === "workFailed" || e.kind === "denied") terminal.add(e.mid);
    if (e.kind === "accepted") {
      accepted.add(e.mid);
      if (now - e.at < 60_000) recentAccepts++;
    }
  }
  let inFlight = 0;
  for (const mid of accepted) if (!terminal.has(mid)) inFlight++;
  return { spentUsd, node, rootFp, served, inFlight, rpmUsed: recentAccepts };
}
