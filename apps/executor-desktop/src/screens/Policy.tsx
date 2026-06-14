import { clsx } from "clsx";
import { useApp } from "../lib/store";
import { useI18n } from "../i18n";
import * as s from "../styles/ui.css";

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
    <div className={s.reveal}>
      <div className={s.pageHead}>
        <h1>{d.policy.title}</h1>
        <p>{d.policy.subtitle}</p>
      </div>

      <div className={s.grid({ cols: 2 })}>
        <div className={s.card({ pad: "lg" })}>
          <div className={s.cardTitle}>{d.policy.providers}</div>
          <div>
            {KNOWN.map((p) => (
              <label className={s.checkline} key={p}>
                <input
                  type="checkbox"
                  checked={settings.providers.includes(p)}
                  onChange={() => toggle(p)}
                />
                <span className={s.mono}>{p}</span>
              </label>
            ))}
          </div>
          <span className={s.hint} style={{ marginTop: 8, display: "block" }}>
            {d.policy.providersHint}
          </span>
        </div>

        <div className={s.card({ pad: "lg" })}>
          <div className={s.field}>
            <label className={s.fieldLabel}>{d.policy.budget} · USD</label>
            <input
              className={clsx(s.input, s.mono)}
              type="number"
              min="0"
              step="1"
              inputMode="decimal"
              placeholder="—"
              value={settings.budgetUsd ?? ""}
              onChange={(e) => update({ budgetUsd: numOrNull(e.target.value) })}
            />
            <span className={s.hint}>{d.policy.budgetHint}</span>
          </div>

          <div className={s.field}>
            <label className={s.fieldLabel}>{d.policy.rate} · rpm</label>
            <input
              className={clsx(s.input, s.mono)}
              type="number"
              min="0"
              step="1"
              inputMode="numeric"
              placeholder="—"
              value={settings.rpm ?? ""}
              onChange={(e) => update({ rpm: numOrNull(e.target.value) })}
            />
            <span className={s.hint}>{d.policy.rateHint}</span>
          </div>

          <span className={s.hint} style={{ color: "var(--kw-color-brand2)" }}>
            {d.policy.appliesNext}
          </span>
        </div>
      </div>
    </div>
  );
}
