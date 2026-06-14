import { useApp } from "../lib/store";
import { useI18n } from "../i18n";

const KNOWN = ["mock", "openai", "openai-responses", "anthropic"];

export function Policy() {
  const { d } = useI18n();
  const { settings, update } = useApp();

  const toggle = (p: string) => {
    const has = settings.providers.includes(p);
    update({
      providers: has ? settings.providers.filter((x) => x !== p) : [...settings.providers, p],
    });
  };

  const numOrNull = (v: string): number | null => {
    const n = Number(v);
    return v.trim() === "" || Number.isNaN(n) ? null : n;
  };

  return (
    <div className="reveal">
      <div className="page-head">
        <h1>{d.policy.title}</h1>
        <p>{d.policy.subtitle}</p>
      </div>

      <div className="grid grid--2">
        <div className="card card--pad-lg">
          <div className="card__title">{d.policy.providers}</div>
          <div>
            {KNOWN.map((p) => (
              <label className="checkline" key={p}>
                <input
                  type="checkbox"
                  checked={settings.providers.includes(p)}
                  onChange={() => toggle(p)}
                />
                <span className="mono">{p}</span>
              </label>
            ))}
          </div>
          <span className="hint" style={{ marginTop: 8, display: "block" }}>
            {d.policy.providersHint}
          </span>
        </div>

        <div className="card card--pad-lg">
          <div className="field">
            <label>{d.policy.budget} · USD</label>
            <input
              className="input mono"
              type="number"
              min="0"
              step="1"
              inputMode="decimal"
              placeholder="—"
              value={settings.budgetUsd ?? ""}
              onChange={(e) => update({ budgetUsd: numOrNull(e.target.value) })}
            />
            <span className="hint">{d.policy.budgetHint}</span>
          </div>

          <div className="field">
            <label>{d.policy.rate} · rpm</label>
            <input
              className="input mono"
              type="number"
              min="0"
              step="1"
              inputMode="numeric"
              placeholder="—"
              value={settings.rpm ?? ""}
              onChange={(e) => update({ rpm: numOrNull(e.target.value) })}
            />
            <span className="hint">{d.policy.rateHint}</span>
          </div>

          <span className="hint" style={{ color: "var(--brand-2)" }}>
            {d.policy.appliesNext}
          </span>
        </div>
      </div>
    </div>
  );
}
