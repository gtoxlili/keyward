import { globalStyle, style } from "@vanilla-extract/css";
import { recipe } from "@vanilla-extract/recipes";
import { fadeUp, flow, pulse, vars } from "./theme.css";

const focusRing = {
  outline: "none",
  borderColor: vars.color.brand1,
  boxShadow: "0 0 0 3px rgba(99, 102, 241, 0.18)",
} as const;

// ============================================================ app shell
export const app = style({
  height: "100%",
  display: "grid",
  gridTemplateRows: "52px 1fr",
  background: `radial-gradient(1100px 520px at 78% -8%, rgba(99,102,241,0.1), transparent 60%), radial-gradient(900px 500px at -6% 110%, rgba(34,211,238,0.07), transparent 55%), ${vars.color.bg}`,
});

export const topbar = style({
  display: "flex",
  alignItems: "center",
  gap: "14px",
  padding: "0 16px 0 84px",
  borderBottom: `1px solid ${vars.color.line}`,
  background: `color-mix(in srgb, ${vars.color.bg} 70%, transparent)`,
  backdropFilter: "blur(12px)",
});
export const topbarBrand = style({ display: "flex", alignItems: "center", gap: "10px" });
export const topbarLogo = style({ width: "22px", height: "22px" });
export const topbarName = style({ fontWeight: 700, letterSpacing: "-0.01em" });
export const topbarNameSub = style({ color: vars.color.textFaint, fontWeight: 500 });
export const topbarSpacer = style({ flex: 1, alignSelf: "stretch" });
export const topbarTools = style({ display: "flex", alignItems: "center", gap: "8px" });

export const shell = style({
  display: "grid",
  gridTemplateColumns: "232px 1fr",
  minHeight: 0,
});

export const sidebar = style({
  borderRight: `1px solid ${vars.color.line}`,
  background: `linear-gradient(180deg, ${vars.color.bg2}, ${vars.color.bg})`,
  display: "flex",
  flexDirection: "column",
  padding: "14px 12px",
  minHeight: 0,
});
export const nav = style({ display: "flex", flexDirection: "column", gap: "2px" });

export const navItem = recipe({
  base: {
    display: "flex",
    alignItems: "center",
    gap: "11px",
    padding: "9px 11px",
    borderRadius: vars.radius.md,
    color: vars.color.textDim,
    background: "none",
    border: "none",
    cursor: "pointer",
    width: "100%",
    textAlign: "left",
    position: "relative",
    transition: "color 0.16s, background 0.16s",
    ":hover": { color: vars.color.text, background: vars.color.panel },
  },
  variants: {
    active: {
      true: {
        color: vars.color.text,
        background: `color-mix(in srgb, ${vars.color.brand1} 14%, ${vars.color.panel})`,
        selectors: {
          "&::before": {
            content: '""',
            position: "absolute",
            left: "-12px",
            top: "50%",
            transform: "translateY(-50%)",
            width: "3px",
            height: "20px",
            borderRadius: "0 3px 3px 0",
            background: vars.grad.brand,
          },
        },
      },
    },
  },
});
globalStyle(`${nav} svg`, { width: "18px", height: "18px", flexShrink: 0, opacity: 0.85 });
export const navLabel = style({ fontWeight: 500, fontSize: "13.5px" });

export const sidebarFoot = style({
  marginTop: "auto",
  paddingTop: "12px",
  borderTop: `1px solid ${vars.color.line}`,
});
export const idchip = style({
  display: "flex",
  alignItems: "center",
  gap: "8px",
  padding: "8px 10px",
  borderRadius: vars.radius.md,
  background: vars.color.panel,
  border: `1px solid ${vars.color.line}`,
  cursor: "pointer",
  width: "100%",
  textAlign: "left",
  transition: "border-color 0.16s",
  ":hover": { borderColor: vars.color.line2 },
});
export const idchipDot = style({
  width: "7px",
  height: "7px",
  borderRadius: "50%",
  background: vars.grad.brand,
  boxShadow: vars.shadow.glow,
  flexShrink: 0,
});
export const idchipTxt = style({ minWidth: 0 });
export const idchipLabel = style({
  display: "block",
  fontSize: "10px",
  fontWeight: 600,
  color: vars.color.textFaint,
  textTransform: "uppercase",
  letterSpacing: "0.06em",
});
export const idchipValue = style({ fontSize: "12px", color: vars.color.textDim });

export const content = style({
  minWidth: 0,
  minHeight: 0,
  overflowY: "auto",
  padding: "26px 30px 40px",
});
globalStyle(`${content}::-webkit-scrollbar`, { width: "10px" });
globalStyle(`${content}::-webkit-scrollbar-thumb`, {
  background: vars.color.line2,
  borderRadius: "10px",
  border: "3px solid transparent",
  backgroundClip: "padding-box",
});

export const pageHead = style({ marginBottom: "22px" });
globalStyle(`${pageHead} h1`, {
  fontSize: "22px",
  fontWeight: 700,
  letterSpacing: "-0.02em",
});
globalStyle(`${pageHead} p`, {
  color: vars.color.textDim,
  marginTop: "3px",
  maxWidth: "64ch",
});

// ============================================================ primitives
export const card = recipe({
  base: {
    background: vars.color.panel,
    border: `1px solid ${vars.color.line}`,
    borderRadius: vars.radius.lg,
    padding: "20px",
  },
  variants: { pad: { lg: { padding: "24px" } } },
});
export const cardTitle = style({
  fontSize: "12px",
  fontWeight: 600,
  letterSpacing: "0.07em",
  textTransform: "uppercase",
  color: vars.color.textFaint,
  marginBottom: "14px",
});

export const grid = recipe({
  base: { display: "grid", gap: "16px" },
  variants: {
    cols: {
      2: { gridTemplateColumns: "repeat(2, 1fr)" },
      3: {
        gridTemplateColumns: "repeat(3, 1fr)",
        "@media": { "screen and (max-width: 920px)": { gridTemplateColumns: "1fr 1fr" } },
      },
    },
  },
});

export const btn = recipe({
  base: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "8px",
    padding: "9px 16px",
    borderRadius: vars.radius.md,
    border: `1px solid ${vars.color.line2}`,
    background: vars.color.panel2,
    color: vars.color.text,
    fontWeight: 600,
    fontSize: "13.5px",
    cursor: "pointer",
    transition: "transform 0.12s, border-color 0.16s, background 0.16s, opacity 0.16s",
    ":hover": { background: vars.color.panelHi },
    ":active": { transform: "translateY(1px)" },
    ":disabled": { opacity: 0.45, cursor: "not-allowed" },
  },
  variants: {
    tone: {
      primary: {
        border: "none",
        background: vars.grad.brand,
        color: "#fff",
        boxShadow: "0 8px 24px -10px rgba(99,102,241,0.7)",
        ":hover": { filter: "brightness(1.07)", background: vars.grad.brand },
      },
      danger: {
        color: vars.color.bad,
        borderColor: `color-mix(in srgb, ${vars.color.bad} 40%, transparent)`,
      },
      ghost: { background: "transparent", borderColor: vars.color.line },
    },
    size: { sm: { padding: "6px 11px", fontSize: "12.5px" } },
  },
});

export const field = style({
  display: "flex",
  flexDirection: "column",
  gap: "7px",
  marginBottom: "16px",
});
export const fieldLabel = style({ fontSize: "12.5px", fontWeight: 600, color: vars.color.textDim });
export const hint = style({ fontSize: "12px", color: vars.color.textFaint });
export const input = style({
  width: "100%",
  padding: "10px 13px",
  background: vars.color.bg2,
  border: `1px solid ${vars.color.line2}`,
  borderRadius: vars.radius.md,
  color: vars.color.text,
  transition: "border-color 0.16s, box-shadow 0.16s",
  userSelect: "text",
  "::placeholder": { color: vars.color.textFaint },
  ":focus": focusRing,
});
export const inputRow = style({ display: "flex", gap: "8px" });

export const pill = style({
  display: "inline-flex",
  alignItems: "center",
  gap: "7px",
  padding: "5px 11px 5px 9px",
  borderRadius: "999px",
  fontSize: "12.5px",
  fontWeight: 600,
  border: `1px solid ${vars.color.line2}`,
  background: vars.color.panel,
});
export const pillDot = style({
  width: "8px",
  height: "8px",
  borderRadius: "50%",
  background: vars.color.textFaint,
});
globalStyle(`${pill}[data-state="connected"]`, {
  color: vars.color.ok,
  borderColor: `color-mix(in srgb, ${vars.color.ok} 35%, transparent)`,
});
globalStyle(`${pill}[data-state="connected"] ${pillDot}`, {
  background: vars.color.ok,
  boxShadow: `0 0 0 4px color-mix(in srgb, ${vars.color.ok} 18%, transparent)`,
});
globalStyle(`${pill}[data-state="connecting"]`, {
  color: vars.color.warn,
  borderColor: `color-mix(in srgb, ${vars.color.warn} 35%, transparent)`,
});
globalStyle(`${pill}[data-state="connecting"] ${pillDot}`, {
  background: vars.color.warn,
  animation: `${pulse} 1.1s ease-in-out infinite`,
});
globalStyle(`${pill}[data-state="error"]`, {
  color: vars.color.bad,
  borderColor: `color-mix(in srgb, ${vars.color.bad} 35%, transparent)`,
});
globalStyle(`${pill}[data-state="error"] ${pillDot}`, { background: vars.color.bad });

export const tag = recipe({
  base: {
    display: "inline-flex",
    padding: "2px 8px",
    borderRadius: "6px",
    fontSize: "11px",
    fontWeight: 600,
    fontFamily: vars.font.mono,
    background: vars.color.panel2,
    color: vars.color.textDim,
    border: `1px solid ${vars.color.line}`,
  },
  variants: {
    tone: {
      ok: { color: vars.color.ok, borderColor: `color-mix(in srgb, ${vars.color.ok} 30%, transparent)` },
      bad: { color: vars.color.bad, borderColor: `color-mix(in srgb, ${vars.color.bad} 30%, transparent)` },
    },
  },
});

export const iconbtn = style({
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  height: "32px",
  minWidth: "32px",
  padding: "0 9px",
  gap: "5px",
  borderRadius: vars.radius.md,
  border: `1px solid ${vars.color.line}`,
  background: vars.color.panel,
  color: vars.color.textDim,
  cursor: "pointer",
  fontWeight: 600,
  fontSize: "12.5px",
  transition: "color 0.16s, border-color 0.16s, background 0.16s",
  ":hover": { color: vars.color.text, borderColor: vars.color.line2 },
});
globalStyle(`${iconbtn} svg`, { width: "16px", height: "16px" });

export const seg = style({
  display: "inline-flex",
  padding: "2px",
  borderRadius: vars.radius.md,
  background: vars.color.panel,
  border: `1px solid ${vars.color.line}`,
});
export const segBtn = recipe({
  base: {
    border: "none",
    background: "none",
    color: vars.color.textDim,
    padding: "4px 10px",
    borderRadius: "7px",
    cursor: "pointer",
    fontWeight: 600,
    fontSize: "12.5px",
    transition: "color 0.15s, background 0.15s",
  },
  variants: { on: { true: { color: vars.color.text, background: vars.color.panelHi } } },
});

export const checkline = style({ display: "flex", alignItems: "center", gap: "10px", padding: "8px 0" });
globalStyle(`${checkline} input`, { width: "17px", height: "17px", accentColor: vars.color.brand1, cursor: "pointer" });

// ============================================================ dashboard
export const routing = style({
  position: "relative",
  display: "grid",
  gridTemplateColumns: "1fr auto 1fr",
  alignItems: "center",
  gap: "18px",
  padding: "26px 28px",
  borderRadius: vars.radius.lg,
  border: `1px solid ${vars.color.line}`,
  background: `radial-gradient(120% 140% at 50% 0%, rgba(99,102,241,0.08), transparent 60%), ${vars.color.panel}`,
  overflow: "hidden",
});
export const routingNode = recipe({
  base: { display: "flex", flexDirection: "column", gap: "4px" },
  variants: { right: { true: { textAlign: "right", alignItems: "flex-end" } } },
});
globalStyle(`${routing} b`, { fontSize: "15px", fontWeight: 700 });
globalStyle(`${routing} small`, { color: vars.color.textFaint, fontSize: "12px" });
export const routingWire = style({
  position: "relative",
  width: "100%",
  minWidth: "120px",
  height: "2px",
  borderRadius: "2px",
  background: vars.color.line2,
});
export const routingGlyph = style({
  position: "absolute",
  top: "50%",
  left: "50%",
  transform: "translate(-50%, -50%)",
  width: "38px",
  height: "38px",
  borderRadius: "50%",
  display: "grid",
  placeItems: "center",
  background: vars.color.panel,
  border: `1px solid ${vars.color.line2}`,
  color: vars.color.textFaint,
  zIndex: 2,
});
globalStyle(`${routingGlyph} svg`, { width: "18px", height: "18px" });
globalStyle(`${routing}[data-live="true"] ${routingWire}`, {
  background: `linear-gradient(90deg, color-mix(in srgb, ${vars.color.brand1} 60%, transparent), color-mix(in srgb, ${vars.color.brand2} 60%, transparent))`,
});
globalStyle(`${routing}[data-live="true"] ${routingWire}::after`, {
  content: '""',
  position: "absolute",
  top: "-1px",
  left: 0,
  height: "4px",
  width: "44px",
  borderRadius: "4px",
  background: `linear-gradient(90deg, transparent, ${vars.color.brand2}, transparent)`,
  animation: `${flow} 2.1s linear infinite`,
});
globalStyle(`${routing}[data-live="true"] ${routingGlyph}`, {
  color: vars.color.brand2,
  borderColor: `color-mix(in srgb, ${vars.color.brand2} 50%, transparent)`,
  boxShadow: vars.shadow.glow,
});
export const nodeBadge = recipe({
  base: {
    display: "inline-flex",
    alignItems: "center",
    gap: "7px",
    padding: "4px 10px 4px 8px",
    borderRadius: "999px",
    background: vars.color.bg2,
    border: `1px solid ${vars.color.line}`,
    fontSize: "12px",
    color: vars.color.textDim,
    marginTop: "7px",
  },
  variants: { you: { true: {} } },
});
export const nodeBadgeDot = style({ width: "7px", height: "7px", borderRadius: "50%", background: vars.color.textFaint });
globalStyle(`${nodeBadge({ you: true })} ${nodeBadgeDot}`, {
  background: vars.grad.brand,
  boxShadow: vars.shadow.glow,
});

export const stat = style({ display: "flex", flexDirection: "column", gap: "6px" });
export const statLabel = style({
  fontSize: "11.5px",
  fontWeight: 600,
  letterSpacing: "0.05em",
  textTransform: "uppercase",
  color: vars.color.textFaint,
});
export const statValue = style({
  fontFamily: vars.font.mono,
  fontSize: "28px",
  fontWeight: 600,
  letterSpacing: "-0.02em",
});
export const statUnit = style({ fontSize: "15px", color: vars.color.textFaint, marginLeft: "3px" });
export const statSub = style({ fontSize: "12px", color: vars.color.textDim });
export const bar = style({
  height: "6px",
  borderRadius: "6px",
  background: vars.color.bg2,
  overflow: "hidden",
  marginTop: "2px",
});
export const barFill = style({
  display: "block",
  height: "100%",
  borderRadius: "6px",
  background: vars.grad.brand,
  transition: "width 0.5s cubic-bezier(0.2,0.8,0.2,1)",
});

export const log = style({ display: "flex", flexDirection: "column" });
export const logRow = style({
  display: "grid",
  gridTemplateColumns: "64px 92px 1fr auto",
  gap: "12px",
  alignItems: "center",
  padding: "9px 4px",
  borderTop: `1px solid ${vars.color.line}`,
  fontSize: "13px",
  animation: `${fadeUp} 0.3s ease both`,
  selectors: { "&:first-child": { borderTop: "none" } },
});
export const logTime = style({ fontFamily: vars.font.mono, fontSize: "11.5px", color: vars.color.textFaint });
export const logKind = style({
  fontWeight: 600,
  fontSize: "11.5px",
  letterSpacing: "0.03em",
  textTransform: "uppercase",
});
globalStyle(`${logKind}[data-k="paired"], ${logKind}[data-k="done"]`, { color: vars.color.ok });
globalStyle(`${logKind}[data-k="accepted"]`, { color: vars.color.brand2 });
globalStyle(
  `${logKind}[data-k="denied"], ${logKind}[data-k="workFailed"], ${logKind}[data-k="stopped"]`,
  { color: vars.color.bad },
);
globalStyle(
  `${logKind}[data-k="connecting"], ${logKind}[data-k="reconnecting"], ${logKind}[data-k="connectionLost"]`,
  { color: vars.color.warn },
);
export const logMsg = style({
  color: vars.color.textDim,
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
});
export const logMeta = style({ fontFamily: vars.font.mono, fontSize: "11.5px", color: vars.color.textFaint });
export const empty = style({ padding: "30px", textAlign: "center", color: vars.color.textFaint, fontSize: "13px" });

export const row = style({
  display: "flex",
  alignItems: "center",
  gap: "12px",
  padding: "13px 4px",
  borderTop: `1px solid ${vars.color.line}`,
  selectors: { "&:first-child": { borderTop: "none" } },
});
export const rowMain = style({ flex: 1, minWidth: 0 });
globalStyle(`${rowMain} b`, { fontWeight: 600 });
globalStyle(`${rowMain} small`, { display: "block", color: vars.color.textFaint, fontSize: "12px" });

// ============================================================ toasts
export const toasts = style({
  position: "fixed",
  bottom: "20px",
  left: "50%",
  transform: "translateX(-50%)",
  display: "flex",
  flexDirection: "column",
  gap: "8px",
  zIndex: 50,
});
export const toast = recipe({
  base: {
    padding: "10px 16px",
    borderRadius: vars.radius.md,
    background: vars.color.panelHi,
    border: `1px solid ${vars.color.line2}`,
    boxShadow: vars.shadow.card,
    fontSize: "13px",
    fontWeight: 500,
    animation: `${fadeUp} 0.25s ease both`,
  },
  variants: {
    tone: {
      ok: { color: vars.color.ok },
      bad: { color: vars.color.bad, borderColor: `color-mix(in srgb, ${vars.color.bad} 40%, transparent)` },
    },
  },
});

// ============================================================ reveal (stagger)
export const reveal = style({});
globalStyle(`${reveal} > *`, { animation: `${fadeUp} 0.4s ease both` });
for (let i = 1; i <= 5; i++) {
  globalStyle(`${reveal} > *:nth-child(${i})`, { animationDelay: `${0.02 + (i - 1) * 0.05}s` });
}

// the mono utility
export const mono = style({ fontFamily: vars.font.mono });
