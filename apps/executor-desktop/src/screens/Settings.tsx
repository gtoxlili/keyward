import { useApp } from "../lib/store";
import { useI18n } from "../i18n";

const VERSION = "0.0.0";

export function Settings() {
  const { d, lang, setLang } = useI18n();
  const { settings, update } = useApp();

  return (
    <div className="reveal">
      <div className="page-head">
        <h1>{d.settings.title}</h1>
        <p>{d.settings.subtitle}</p>
      </div>

      <div className="grid grid--2">
        <div className="card card--pad-lg">
          <div className="field">
            <label>{d.settings.language}</label>
            <div className="seg">
              <button className={lang === "en" ? "on" : ""} onClick={() => setLang("en")}>
                English
              </button>
              <button className={lang === "zh" ? "on" : ""} onClick={() => setLang("zh")}>
                中文
              </button>
            </div>
          </div>

          <div className="field" style={{ marginBottom: 0 }}>
            <label>{d.settings.theme}</label>
            <div className="seg">
              <button
                className={settings.theme === "dark" ? "on" : ""}
                onClick={() => update({ theme: "dark" })}
              >
                {d.settings.dark}
              </button>
              <button
                className={settings.theme === "light" ? "on" : ""}
                onClick={() => update({ theme: "light" })}
              >
                {d.settings.light}
              </button>
            </div>
          </div>
        </div>

        <div className="card card--pad-lg">
          <div className="card__title">{d.settings.about}</div>
          <p style={{ color: "var(--text-dim)", fontSize: 13.5, lineHeight: 1.65 }}>
            {d.settings.aboutBody}
          </p>
          <div style={{ marginTop: 18, display: "flex", gap: 10, alignItems: "center" }}>
            <span className="tag">Keyward</span>
            <span className="hint">
              {d.settings.version} {VERSION}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
