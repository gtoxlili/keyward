import { useMemo } from "react";
import { useApp, deriveStats, type LogEntry } from "../lib/store";
import { useI18n, fill, type Dict } from "../i18n";
import { IconArrow } from "../components/icons";

function eventText(e: LogEntry, d: Dict): { msg: string; meta: string } {
  switch (e.kind) {
    case "connecting":
      return { msg: e.url, meta: "" };
    case "paired":
      return { msg: e.orchestrator, meta: e.rootFingerprint };
    case "accepted":
      return { msg: `${e.provider} · ${e.model}`, meta: "" };
    case "done":
      return {
        msg: `${e.provider} · ${e.model}`,
        meta: `${e.inputTokens}/${e.outputTokens} · $${e.costUsd.toFixed(4)}`,
      };
    case "denied":
      return { msg: `${e.provider} · ${e.model}`, meta: e.code };
    case "workFailed":
      return { msg: e.message, meta: e.code };
    case "connectionLost":
      return { msg: fill(d.dashboard.inFlight, { n: e.pending }), meta: "" };
    case "reconnecting":
      return { msg: `#${e.attempt}`, meta: "" };
    case "stopped":
      return { msg: e.reason, meta: "" };
  }
}

export function Dashboard() {
  const { d } = useI18n();
  const { status, events, settings } = useApp();
  const stats = useMemo(() => deriveStats(events), [events]);
  const live = status === "connected";

  const budget = settings.budgetUsd;
  const rpm = settings.rpm;
  const spendPct = budget ? Math.min(100, (stats.spentUsd / budget) * 100) : 0;
  const ratePct = rpm ? Math.min(100, (stats.rpmUsed / rpm) * 100) : 0;

  return (
    <div className="reveal">
      <div className="page-head">
        <h1>{d.dashboard.title}</h1>
        <p>{d.dashboard.subtitle}</p>
      </div>

      <div className="routing" data-live={live} style={{ marginBottom: 16 }}>
        <div className="routing__node">
          <b>{d.dashboard.orchestrator}</b>
          <small>{stats.orchestrator ?? d.dashboard.orchestratorSub}</small>
          <span className="node-badge">
            <i className="dot" />
            {stats.orchestrator ? d.status.connected : d.dashboard.notPaired}
          </span>
        </div>
        <div className="routing__wire">
          <span className="glyph">
            <IconArrow size={18} />
          </span>
        </div>
        <div className="routing__node right">
          <b>{d.dashboard.you}</b>
          <small>{d.dashboard.youSub}</small>
          <span className="node-badge you">
            <i className="dot" />
            {d.dashboard.routes}
          </span>
        </div>
      </div>

      <div className="grid grid--3" style={{ marginBottom: 16 }}>
        <div className="card">
          <div className="stat">
            <span className="stat__label">{d.dashboard.sessionSpend}</span>
            <span className="stat__value mono">
              <span className="unit">$</span>
              {stats.spentUsd.toFixed(4)}
            </span>
            <div className="bar">
              <i style={{ width: `${spendPct}%` }} />
            </div>
            <span className="stat__sub">
              {budget
                ? fill(d.dashboard.perBudget, { budget: `$${budget}` })
                : d.dashboard.noBudget}
            </span>
          </div>
        </div>

        <div className="card">
          <div className="stat">
            <span className="stat__label">{d.dashboard.requests}</span>
            <span className="stat__value mono">{stats.served}</span>
            <div className="bar">
              <i style={{ width: stats.inFlight ? "100%" : "0%", opacity: 0.5 }} />
            </div>
            <span className="stat__sub">{fill(d.dashboard.inFlight, { n: stats.inFlight })}</span>
          </div>
        </div>

        <div className="card">
          <div className="stat">
            <span className="stat__label">{d.dashboard.rate}</span>
            <span className="stat__value mono">
              {stats.rpmUsed}
              <span className="unit">rpm</span>
            </span>
            <div className="bar">
              <i style={{ width: `${ratePct}%` }} />
            </div>
            <span className="stat__sub">
              {rpm ? fill(d.dashboard.rpmCap, { rpm }) : d.dashboard.noRate}
            </span>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card__title">{d.dashboard.activity}</div>
        {events.length === 0 ? (
          <div className="empty">{d.dashboard.noActivity}</div>
        ) : (
          <div className="log">
            {events.slice(0, 40).map((e) => {
              const { msg, meta } = eventText(e, d);
              return (
                <div className="log__row" key={e.id}>
                  <span className="log__time">
                    {new Date(e.at).toLocaleTimeString([], { hour12: false })}
                  </span>
                  <span className="log__kind" data-k={e.kind}>
                    {d.events[e.kind]}
                  </span>
                  <span className="log__msg">{msg}</span>
                  <span className="log__meta mono">{meta}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
