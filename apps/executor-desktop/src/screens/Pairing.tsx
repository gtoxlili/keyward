import { useState } from "react";
import { clsx } from "clsx";
import { useApp } from "../lib/store";
import { useI18n } from "../i18n";
import { CopyButton } from "../components/CopyButton";
import * as s from "../styles/ui.css";

const KNOWN = ["mock", "openai", "openai-responses", "anthropic"];

export function Pairing() {
  const { d } = useI18n();
  const { settings, update, identity, status, start, stop } = useApp();
  const [token, setToken] = useState("");
  const running = status === "connecting" || status === "connected";

  const toggleProvider = (p: string) => {
    const has = settings.providers.includes(p);
    update({
      providers: has ? settings.providers.filter((x) => x !== p) : [...settings.providers, p],
    });
  };

  return (
    <div className={s.reveal}>
      <div className={s.pageHead}>
        <h1>{d.pairing.title}</h1>
        <p>{d.pairing.subtitle}</p>
      </div>

      <div className={s.grid({ cols: 2 })}>
        <div className={s.card({ pad: "lg" })}>
          <div className={s.field}>
            <label className={s.fieldLabel}>{d.pairing.orchUrl}</label>
            <input
              className={clsx(s.input, s.mono)}
              placeholder="ws://127.0.0.1:8787"
              value={settings.orchUrl}
              spellCheck={false}
              onChange={(e) => update({ orchUrl: e.target.value })}
            />
            <span className={s.hint}>{d.pairing.orchUrlHint}</span>
          </div>

          <div className={s.field}>
            <label className={s.fieldLabel}>{d.pairing.token}</label>
            <input
              className={clsx(s.input, s.mono)}
              placeholder="pt_…"
              value={token}
              spellCheck={false}
              onChange={(e) => setToken(e.target.value)}
            />
            <span className={s.hint}>{d.pairing.tokenHint}</span>
          </div>

          <div className={s.field}>
            <label className={s.fieldLabel}>
              {d.pairing.expectedFp}{" "}
              <span style={{ color: "var(--kw-color-textFaint)", fontWeight: 500 }}>
                · {d.common.optional}
              </span>
            </label>
            <input
              className={clsx(s.input, s.mono)}
              placeholder="0000-0000-0000-0000"
              value={settings.expectedRootFp}
              spellCheck={false}
              onChange={(e) => update({ expectedRootFp: e.target.value })}
            />
            <span className={s.hint}>{d.pairing.expectedFpHint}</span>
          </div>

          <div className={s.field}>
            <label className={s.fieldLabel}>{d.pairing.offer}</label>
            <div>
              {KNOWN.map((p) => (
                <label className={s.checkline} key={p}>
                  <input
                    type="checkbox"
                    checked={settings.providers.includes(p)}
                    onChange={() => toggleProvider(p)}
                  />
                  <span className={s.mono}>{p}</span>
                </label>
              ))}
            </div>
            <span className={s.hint}>{d.pairing.offerHint}</span>
          </div>

          <div style={{ display: "flex", gap: 10, alignItems: "center", marginTop: 6 }}>
            {running ? (
              <button className={s.btn({ tone: "danger" })} onClick={() => stop()}>
                {d.common.stop}
              </button>
            ) : (
              <button
                className={s.btn({ tone: "primary" })}
                disabled={!settings.orchUrl || !token || settings.providers.length === 0}
                onClick={() => start(token)}
              >
                {d.common.start}
              </button>
            )}
            {running && (
              <span
                className={s.pill}
                data-state={status === "connected" ? "connected" : "connecting"}
              >
                <span className={s.pillDot} />
                {status === "connected" ? d.status.connected : d.status.connecting}
              </span>
            )}
          </div>
        </div>

        <div className={s.card({ pad: "lg" })}>
          <div className={s.cardTitle}>{d.pairing.identity}</div>
          <p style={{ color: "var(--kw-color-textDim)", fontSize: 13, marginBottom: 18 }}>
            {d.pairing.identityHint}
          </p>
          <div className={s.field}>
            <label className={s.fieldLabel}>{d.pairing.fingerprint}</label>
            <div className={s.inputRow}>
              <input className={clsx(s.input, s.mono)} readOnly value={identity?.fingerprint ?? "…"} />
              {identity && <CopyButton value={identity.fingerprint} />}
            </div>
          </div>
          <div className={s.field}>
            <label className={s.fieldLabel}>{d.pairing.pubkey}</label>
            <div className={s.inputRow}>
              <input className={clsx(s.input, s.mono)} readOnly value={identity?.pubkey ?? "…"} />
              {identity && <CopyButton value={identity.pubkey} />}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
