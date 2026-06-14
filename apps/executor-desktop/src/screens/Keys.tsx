import { useEffect, useMemo, useState } from "react";
import { clsx } from "clsx";
import { api } from "../lib/api";
import { useApp } from "../lib/store";
import { useI18n, fill } from "../i18n";
import * as s from "../styles/ui.css";

export function Keys() {
  const { d } = useI18n();
  const { toast } = useApp();
  const [extra, setExtra] = useState<string[]>([]);
  const [custom, setCustom] = useState("");
  const providers = useMemo(() => ["openai", "anthropic", ...extra], [extra]);
  const [present, setPresent] = useState<Record<string, boolean>>({});
  const [drafts, setDrafts] = useState<Record<string, string>>({});

  const refresh = async (list: string[]) => {
    try {
      const st = await api.keyStatus(list);
      setPresent((prev) => ({
        ...prev,
        ...Object.fromEntries(st.map((x) => [x.provider, x.present])),
      }));
    } catch {
      /* keychain unavailable — leave as unknown */
    }
  };

  useEffect(() => {
    void refresh(providers);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providers.length]);

  const store = async (p: string) => {
    const key = (drafts[p] ?? "").trim();
    if (!key) return;
    try {
      await api.setKey(p, key);
      setDrafts((prev) => ({ ...prev, [p]: "" }));
      toast(d.keys.storedToast, "ok");
      void refresh([p]);
    } catch (e) {
      toast(String(e), "bad");
    }
  };

  const remove = async (p: string) => {
    try {
      await api.deleteKey(p);
      toast(d.keys.removedToast);
      void refresh([p]);
    } catch (e) {
      toast(String(e), "bad");
    }
  };

  const addCustom = () => {
    const id = custom.trim().toLowerCase();
    if (id && !providers.includes(id)) {
      setExtra((x) => [...x, id]);
      setCustom("");
    }
  };

  return (
    <div className={s.reveal}>
      <div className={s.pageHead}>
        <h1>{d.keys.title}</h1>
        <p>{d.keys.subtitle}</p>
      </div>

      <div className={s.card({})}>
        {providers.map((p) => (
          <div className={s.row} key={p}>
            <div className={s.rowMain}>
              <b className={s.mono}>{p}</b>
              <small>{present[p] ? d.keys.stored : d.keys.notStored}</small>
            </div>
            {present[p] ? (
              <>
                <span className={s.tag({ tone: "ok" })}>{d.keys.stored}</span>
                <button className={s.btn({ tone: "danger", size: "sm" })} onClick={() => remove(p)}>
                  {d.common.remove}
                </button>
              </>
            ) : (
              <div className={s.inputRow} style={{ flex: 1, maxWidth: 440 }}>
                <input
                  className={clsx(s.input, s.mono)}
                  type="password"
                  placeholder={fill(d.keys.placeholder, { provider: p })}
                  value={drafts[p] ?? ""}
                  onChange={(e) => setDrafts((prev) => ({ ...prev, [p]: e.target.value }))}
                  onKeyDown={(e) => e.key === "Enter" && store(p)}
                />
                <button
                  className={s.btn({ size: "sm" })}
                  disabled={!(drafts[p] ?? "").trim()}
                  onClick={() => store(p)}
                >
                  {d.common.store}
                </button>
              </div>
            )}
          </div>
        ))}

        <div className={s.row}>
          <div className={s.inputRow} style={{ flex: 1, maxWidth: 440 }}>
            <input
              className={clsx(s.input, s.mono)}
              placeholder={d.keys.customName}
              value={custom}
              onChange={(e) => setCustom(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addCustom()}
            />
            <button className={s.btn({ tone: "ghost", size: "sm" })} onClick={addCustom}>
              {d.keys.custom}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
