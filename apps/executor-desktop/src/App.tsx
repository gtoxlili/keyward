import { useEffect, useRef, useState, type ReactNode } from "react";
import { api, type ExecutorEvent, type Identity, type Settings } from "./lib/api";
import { AppContext, type AppState, type LogEntry, type Status } from "./lib/store";
import { I18nContext, dicts, type Lang } from "./i18n";
import {
  IconDashboard,
  IconPair,
  IconKey,
  IconShield,
  IconGear,
  IconSun,
  IconMoon,
} from "./components/icons";
import { Dashboard } from "./screens/Dashboard";
import { Pairing } from "./screens/Pairing";
import { Keys } from "./screens/Keys";
import { Policy } from "./screens/Policy";
import { Settings as SettingsScreen } from "./screens/Settings";
import logo from "./assets/logo.png";

type ScreenKey = "dashboard" | "pairing" | "keys" | "policy" | "settings";

const DEFAULTS: Settings = {
  lang: "en",
  theme: "dark",
  orchUrl: "ws://127.0.0.1:8787",
  providers: ["mock"],
  budgetUsd: 5,
  rpm: 60,
  expectedRootFp: "",
};

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [identity, setIdentity] = useState<Identity | null>(null);
  const [status, setStatus] = useState<Status>("idle");
  const [events, setEvents] = useState<LogEntry[]>([]);
  const [toasts, setToasts] = useState<{ id: number; msg: string; tone?: "ok" | "bad" }[]>([]);
  const [screen, setScreen] = useState<ScreenKey>("dashboard");
  const everPaired = useRef(false);
  const counter = useRef(0);

  // First run: load persisted settings + identity, default language to the OS locale.
  useEffect(() => {
    (async () => {
      const saved = await api.loadSettings().catch(() => null);
      const sysLang: Lang = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
      setSettings({ ...DEFAULTS, lang: sysLang, ...(saved ?? {}) });
      api.getIdentity().then(setIdentity).catch(() => undefined);
    })();
  }, []);

  // Persist + apply theme whenever settings change.
  useEffect(() => {
    if (!settings) return;
    void api.saveSettings(settings).catch(() => undefined);
    document.documentElement.dataset.theme = settings.theme;
  }, [settings]);

  const update = (patch: Partial<Settings>) =>
    setSettings((s) => (s ? { ...s, ...patch } : s));

  const toast = (msg: string, tone?: "ok" | "bad") => {
    const id = ++counter.current;
    setToasts((t) => [...t, { id, msg, tone }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 2600);
  };

  const onEvent = (e: ExecutorEvent) => {
    setEvents((prev) => [{ ...e, id: ++counter.current, at: Date.now() }, ...prev].slice(0, 250));
    switch (e.kind) {
      case "connecting":
        setStatus("connecting");
        break;
      case "paired":
        everPaired.current = true;
        setStatus("connected");
        break;
      case "connectionLost":
      case "reconnecting":
        setStatus("connecting");
        break;
      case "stopped":
        setStatus(everPaired.current ? "idle" : "error");
        break;
    }
  };

  const start = async (token: string) => {
    if (!settings) return;
    everPaired.current = false;
    setEvents([]);
    setStatus("connecting");
    try {
      await api.startExecutor(
        {
          orchUrl: settings.orchUrl.trim(),
          pairingToken: token.trim(),
          providers: settings.providers,
          budgetUsd: settings.budgetUsd,
          rpm: settings.rpm,
          expectedRootFp: settings.expectedRootFp.trim() || null,
        },
        onEvent,
      );
    } catch (e) {
      setStatus("error");
      toast(String(e), "bad");
    }
  };

  const stop = async () => {
    try {
      await api.stopExecutor();
    } catch {
      /* ignore */
    }
    setStatus("idle");
  };

  if (!settings) return <div className="app" />;

  const d = dicts[settings.lang];
  const i18n = { lang: settings.lang, setLang: (l: Lang) => update({ lang: l }), d };
  const appState: AppState = { settings, update, identity, status, events, start, stop, toast };

  const nav: { key: ScreenKey; label: string; icon: ReactNode }[] = [
    { key: "dashboard", label: d.nav.dashboard, icon: <IconDashboard /> },
    { key: "pairing", label: d.nav.pairing, icon: <IconPair /> },
    { key: "keys", label: d.nav.keys, icon: <IconKey /> },
    { key: "policy", label: d.nav.policy, icon: <IconShield /> },
    { key: "settings", label: d.nav.settings, icon: <IconGear /> },
  ];

  const pillState = status === "idle" ? undefined : status;
  const pillText =
    status === "idle" ? d.status.disconnected : d.status[status as "connecting" | "connected" | "error"];

  return (
    <I18nContext.Provider value={i18n}>
      <AppContext.Provider value={appState}>
        <div className="app">
          <header className="topbar" data-tauri-drag-region>
            <div className="topbar__brand">
              <img src={logo} alt="Keyward" />
              <span className="topbar__name">
                Keyward <span>{d.brand.tag}</span>
              </span>
            </div>
            <div className="topbar__spacer" data-tauri-drag-region />
            <div className="topbar__tools">
              <span className="pill" data-state={pillState}>
                <span className="pill__dot" />
                {pillText}
              </span>
              <button
                className="iconbtn"
                title={settings.lang === "en" ? "中文" : "English"}
                onClick={() => update({ lang: settings.lang === "en" ? "zh" : "en" })}
              >
                {settings.lang === "en" ? "中" : "EN"}
              </button>
              <button
                className="iconbtn"
                title={d.settings.theme}
                onClick={() => update({ theme: settings.theme === "dark" ? "light" : "dark" })}
              >
                {settings.theme === "dark" ? <IconSun /> : <IconMoon />}
              </button>
            </div>
          </header>

          <div className="shell">
            <aside className="sidebar">
              <nav className="nav">
                {nav.map((n) => (
                  <button
                    key={n.key}
                    className={`nav__item${screen === n.key ? " active" : ""}`}
                    onClick={() => setScreen(n.key)}
                  >
                    {n.icon}
                    <span className="nav__label">{n.label}</span>
                  </button>
                ))}
              </nav>
              <div className="sidebar__foot">
                <div
                  className="idchip"
                  title={d.pairing.fingerprint}
                  onClick={() => {
                    if (identity) {
                      navigator.clipboard.writeText(identity.pubkey).catch(() => undefined);
                      toast(d.common.copied, "ok");
                    }
                  }}
                >
                  <span className="idchip__dot" />
                  <span className="idchip__txt">
                    <b>{d.pairing.identity}</b>
                    <span className="mono">{identity?.fingerprint ?? "…"}</span>
                  </span>
                </div>
              </div>
            </aside>

            <main className="content">
              {screen === "dashboard" && <Dashboard />}
              {screen === "pairing" && <Pairing />}
              {screen === "keys" && <Keys />}
              {screen === "policy" && <Policy />}
              {screen === "settings" && <SettingsScreen />}
            </main>
          </div>

          <div className="toasts">
            {toasts.map((t) => (
              <div className="toast" data-tone={t.tone} key={t.id}>
                {t.msg}
              </div>
            ))}
          </div>
        </div>
      </AppContext.Provider>
    </I18nContext.Provider>
  );
}
