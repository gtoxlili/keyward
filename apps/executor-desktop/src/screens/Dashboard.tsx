import { useMemo } from "react";
import { useApp, deriveStats, type LogEntry } from "../lib/store";
import { useI18n, fill, type Dict } from "../i18n";
import { IconArrow } from "../components/icons";
import * as s from "../styles/ui.css";

function eventText(e: LogEntry, d: Dict): { msg: string; meta: string } {
  switch (e.kind) {
    case "connecting":
      return { msg: e.url, meta: "" };
    case "paired":
      return { msg: e.node, meta: e.rootFingerprint };
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
    <div className={s.reveal}>
      <div className={s.pageHead}>
        <h1>{d.dashboard.title}</h1>
        <p>{d.dashboard.subtitle}</p>
      </div>

      <div className={s.routing} data-live={live} style={{ marginBottom: 16 }}>
        <div className={s.routingNode({})}>
          <b>{d.dashboard.node}</b>
          <small>{stats.node ?? d.dashboard.nodeSub}</small>
          <span className={s.nodeBadge({})}>
            <i className={s.nodeBadgeDot} />
            {stats.node ? d.status.connected : d.dashboard.notPaired}
          </span>
        </div>
        <div className={s.routingWire}>
          <span className={s.routingGlyph}>
            <IconArrow size={18} />
          </span>
        </div>
        <div className={s.routingNode({ right: true })}>
          <b>{d.dashboard.you}</b>
          <small>{d.dashboard.youSub}</small>
          <span className={s.nodeBadge({ you: true })}>
            <i className={s.nodeBadgeDot} />
            {d.dashboard.routes}
          </span>
        </div>
      </div>

      <div className={s.grid({ cols: 3 })} style={{ marginBottom: 16 }}>
        <div className={s.card({})}>
          <div className={s.stat}>
            <span className={s.statLabel}>{d.dashboard.sessionSpend}</span>
            <span className={s.statValue}>
              <span className={s.statUnit}>$</span>
              {stats.spentUsd.toFixed(4)}
            </span>
            <div className={s.bar}>
              <i className={s.barFill} style={{ width: `${spendPct}%` }} />
            </div>
            <span className={s.statSub}>
              {budget
                ? fill(d.dashboard.perBudget, { budget: `$${budget}` })
                : d.dashboard.noBudget}
            </span>
          </div>
        </div>

        <div className={s.card({})}>
          <div className={s.stat}>
            <span className={s.statLabel}>{d.dashboard.requests}</span>
            <span className={s.statValue}>{stats.served}</span>
            <div className={s.bar}>
              <i className={s.barFill} style={{ width: stats.inFlight ? "100%" : "0%", opacity: 0.5 }} />
            </div>
            <span className={s.statSub}>{fill(d.dashboard.inFlight, { n: stats.inFlight })}</span>
          </div>
        </div>

        <div className={s.card({})}>
          <div className={s.stat}>
            <span className={s.statLabel}>{d.dashboard.rate}</span>
            <span className={s.statValue}>
              {stats.rpmUsed}
              <span className={s.statUnit}>rpm</span>
            </span>
            <div className={s.bar}>
              <i className={s.barFill} style={{ width: `${ratePct}%` }} />
            </div>
            <span className={s.statSub}>
              {rpm ? fill(d.dashboard.rpmCap, { rpm }) : d.dashboard.noRate}
            </span>
          </div>
        </div>
      </div>

      <div className={s.card({})}>
        <div className={s.cardTitle}>{d.dashboard.activity}</div>
        {events.length === 0 ? (
          <div className={s.empty}>{d.dashboard.noActivity}</div>
        ) : (
          <div className={s.log}>
            {events.slice(0, 40).map((e) => {
              const { msg, meta } = eventText(e, d);
              return (
                <div className={s.logRow} key={e.id}>
                  <span className={s.logTime}>
                    {new Date(e.at).toLocaleTimeString([], { hour12: false })}
                  </span>
                  <span className={s.logKind} data-k={e.kind}>
                    {d.events[e.kind]}
                  </span>
                  <span className={s.logMsg}>{msg}</span>
                  <span className={s.logMeta}>{meta}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
