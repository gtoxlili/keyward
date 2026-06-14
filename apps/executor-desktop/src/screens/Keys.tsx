import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { useApp } from "../lib/store";
import { useI18n, fill } from "../i18n";

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
      const s = await api.keyStatus(list);
      setPresent((prev) => ({
        ...prev,
        ...Object.fromEntries(s.map((x) => [x.provider, x.present])),
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
    <div className="reveal">
      <div className="page-head">
        <h1>{d.keys.title}</h1>
        <p>{d.keys.subtitle}</p>
      </div>

      <div className="card">
        {providers.map((p) => (
          <div className="row" key={p}>
            <div className="row__main">
              <b className="mono">{p}</b>
              <small>{present[p] ? d.keys.stored : d.keys.notStored}</small>
            </div>
            {present[p] ? (
              <>
                <span className="tag tag--ok">{d.keys.stored}</span>
                <button className="btn btn--ghost btn--sm btn--danger" onClick={() => remove(p)}>
                  {d.common.remove}
                </button>
              </>
            ) : (
              <div className="input-row" style={{ flex: 1, maxWidth: 440 }}>
                <input
                  className="input mono"
                  type="password"
                  placeholder={fill(d.keys.placeholder, { provider: p })}
                  value={drafts[p] ?? ""}
                  onChange={(e) => setDrafts((prev) => ({ ...prev, [p]: e.target.value }))}
                  onKeyDown={(e) => e.key === "Enter" && store(p)}
                />
                <button
                  className="btn btn--sm"
                  disabled={!(drafts[p] ?? "").trim()}
                  onClick={() => store(p)}
                >
                  {d.common.store}
                </button>
              </div>
            )}
          </div>
        ))}

        <div className="row">
          <div className="input-row" style={{ flex: 1, maxWidth: 440 }}>
            <input
              className="input mono"
              placeholder={d.keys.customName}
              value={custom}
              onChange={(e) => setCustom(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addCustom()}
            />
            <button className="btn btn--sm btn--ghost" onClick={addCustom}>
              {d.keys.custom}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
