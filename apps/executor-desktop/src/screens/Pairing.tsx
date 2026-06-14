import { useState } from "react";
import { useApp } from "../lib/store";
import { useI18n } from "../i18n";
import { CopyButton } from "../components/CopyButton";

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
    <div className="reveal">
      <div className="page-head">
        <h1>{d.pairing.title}</h1>
        <p>{d.pairing.subtitle}</p>
      </div>

      <div className="grid grid--2">
        <div className="card card--pad-lg">
          <div className="field">
            <label>{d.pairing.orchUrl}</label>
            <input
              className="input mono"
              placeholder="ws://127.0.0.1:8787"
              value={settings.orchUrl}
              spellCheck={false}
              onChange={(e) => update({ orchUrl: e.target.value })}
            />
            <span className="hint">{d.pairing.orchUrlHint}</span>
          </div>

          <div className="field">
            <label>{d.pairing.token}</label>
            <input
              className="input mono"
              placeholder="pt_…"
              value={token}
              spellCheck={false}
              onChange={(e) => setToken(e.target.value)}
            />
            <span className="hint">{d.pairing.tokenHint}</span>
          </div>

          <div className="field">
            <label>
              {d.pairing.expectedFp}{" "}
              <span style={{ color: "var(--text-faint)", fontWeight: 500 }}>
                · {d.common.optional}
              </span>
            </label>
            <input
              className="input mono"
              placeholder="0000-0000-0000-0000"
              value={settings.expectedRootFp}
              spellCheck={false}
              onChange={(e) => update({ expectedRootFp: e.target.value })}
            />
            <span className="hint">{d.pairing.expectedFpHint}</span>
          </div>

          <div className="field">
            <label>{d.pairing.offer}</label>
            <div>
              {KNOWN.map((p) => (
                <label className="checkline" key={p}>
                  <input
                    type="checkbox"
                    checked={settings.providers.includes(p)}
                    onChange={() => toggleProvider(p)}
                  />
                  <span className="mono">{p}</span>
                </label>
              ))}
            </div>
            <span className="hint">{d.pairing.offerHint}</span>
          </div>

          <div style={{ display: "flex", gap: 10, alignItems: "center", marginTop: 6 }}>
            {running ? (
              <button className="btn btn--danger" onClick={() => stop()}>
                {d.common.stop}
              </button>
            ) : (
              <button
                className="btn btn--primary"
                disabled={!settings.orchUrl || !token || settings.providers.length === 0}
                onClick={() => start(token)}
              >
                {d.common.start}
              </button>
            )}
            {running && (
              <span
                className="pill"
                data-state={status === "connected" ? "connected" : "connecting"}
              >
                <span className="pill__dot" />
                {status === "connected" ? d.status.connected : d.status.connecting}
              </span>
            )}
          </div>
        </div>

        <div className="card card--pad-lg">
          <div className="card__title">{d.pairing.identity}</div>
          <p style={{ color: "var(--text-dim)", fontSize: 13, marginBottom: 18 }}>
            {d.pairing.identityHint}
          </p>
          <div className="field">
            <label>{d.pairing.fingerprint}</label>
            <div className="input-row">
              <input className="input mono" readOnly value={identity?.fingerprint ?? "…"} />
              {identity && <CopyButton value={identity.fingerprint} />}
            </div>
          </div>
          <div className="field">
            <label>{d.pairing.pubkey}</label>
            <div className="input-row">
              <input className="input mono" readOnly value={identity?.pubkey ?? "…"} />
              {identity && <CopyButton value={identity.pubkey} />}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
