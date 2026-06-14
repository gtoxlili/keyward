import { useApp } from "../lib/store";
import { useI18n } from "../i18n";
import * as s from "../styles/ui.css";

export function Settings() {
  const { d, lang, setLang } = useI18n();
  const { settings, update } = useApp();

  return (
    <div className={s.reveal}>
      <div className={s.pageHead}>
        <h1>{d.settings.title}</h1>
        <p>{d.settings.subtitle}</p>
      </div>

      <div className={s.grid({ cols: 2 })}>
        <div className={s.card({ pad: "lg" })}>
          <div className={s.field}>
            <label className={s.fieldLabel}>{d.settings.language}</label>
            <div className={s.seg}>
              <button className={s.segBtn({ on: lang === "en" })} onClick={() => setLang("en")}>
                English
              </button>
              <button className={s.segBtn({ on: lang === "zh" })} onClick={() => setLang("zh")}>
                中文
              </button>
            </div>
          </div>

          <div className={s.field} style={{ marginBottom: 0 }}>
            <label className={s.fieldLabel}>{d.settings.theme}</label>
            <div className={s.seg}>
              <button
                className={s.segBtn({ on: settings.theme === "dark" })}
                onClick={() => update({ theme: "dark" })}
              >
                {d.settings.dark}
              </button>
              <button
                className={s.segBtn({ on: settings.theme === "light" })}
                onClick={() => update({ theme: "light" })}
              >
                {d.settings.light}
              </button>
            </div>
          </div>
        </div>

        <div className={s.card({ pad: "lg" })}>
          <div className={s.cardTitle}>{d.settings.about}</div>
          <p style={{ color: "var(--kw-color-textDim)", fontSize: 13.5, lineHeight: 1.65 }}>
            {d.settings.aboutBody}
          </p>
          <div style={{ marginTop: 18, display: "flex", gap: 10, alignItems: "center" }}>
            <span className={s.tag({})}>Keyward</span>
            <span className={s.hint}>
              {d.settings.version} 0.0.0 · {__APP_VERSION__}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
