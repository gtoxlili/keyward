import {
  createGlobalTheme,
  createGlobalThemeContract,
  globalStyle,
  keyframes,
} from "@vanilla-extract/css";

/**
 * Design tokens as a typed contract. `vars.color.bg` compiles to `var(--kw-color-bg)`.
 * "A calm instrument panel for your keys" — deep-ink canvas; the indigo→cyan gradient
 * is reserved for signal (live connection, the key motif, primary actions).
 */
export const vars = createGlobalThemeContract(
  {
    color: {
      bg: null,
      bg2: null,
      panel: null,
      panel2: null,
      panelHi: null,
      line: null,
      line2: null,
      text: null,
      textDim: null,
      textFaint: null,
      brand1: null,
      brand2: null,
      ok: null,
      warn: null,
      bad: null,
    },
    grad: { brand: null },
    shadow: { card: null, glow: null },
    font: { sans: null, mono: null },
    radius: { sm: null, md: null, lg: null },
  },
  (_value, path) => `kw-${path.join("-")}`,
);

// Invariant across themes.
const brand = {
  brand1: "#6366f1",
  brand2: "#22d3ee",
  ok: "#34d399",
  warn: "#f59e0b",
  bad: "#fb7185",
};
const grad = { brand: "linear-gradient(118deg, #6366f1, #22d3ee)" };
const glow = "0 0 28px -6px rgba(99, 102, 241, 0.5)";
const font = {
  sans: '"Hanken Grotesk", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Noto Sans SC", system-ui, sans-serif',
  mono: '"JetBrains Mono", ui-monospace, "SFMono-Regular", monospace',
};
const radius = { sm: "8px", md: "12px", lg: "18px" };

// Dark (hero).
createGlobalTheme(":root", vars, {
  color: {
    bg: "#090b11",
    bg2: "#0c0f18",
    panel: "#12151f",
    panel2: "#171b28",
    panelHi: "#1d2233",
    line: "rgba(255, 255, 255, 0.07)",
    line2: "rgba(255, 255, 255, 0.12)",
    text: "#e8eaf2",
    textDim: "#9aa0b6",
    textFaint: "#5e6479",
    ...brand,
  },
  grad,
  shadow: { card: "0 18px 50px -22px rgba(0, 0, 0, 0.75)", glow },
  font,
  radius,
});

// Light.
createGlobalTheme('[data-theme="light"]', vars, {
  color: {
    bg: "#f5f6fb",
    bg2: "#eef0f7",
    panel: "#ffffff",
    panel2: "#f4f5fa",
    panelHi: "#eceef6",
    line: "rgba(12, 16, 34, 0.09)",
    line2: "rgba(12, 16, 34, 0.16)",
    text: "#14182a",
    textDim: "#545b73",
    textFaint: "#8b91a7",
    ...brand,
  },
  grad,
  shadow: { card: "0 16px 40px -20px rgba(24, 30, 60, 0.28)", glow },
  font,
  radius,
});

// --- shared keyframes ---
export const fadeUp = keyframes({
  from: { opacity: 0, transform: "translateY(8px)" },
  to: { opacity: 1, transform: "none" },
});
export const flow = keyframes({
  from: { left: "-44px" },
  to: { left: "100%" },
});
export const pulse = keyframes({
  "0%, 100%": { opacity: 1 },
  "50%": { opacity: 0.35 },
});

// --- global reset / base ---
globalStyle("*", { boxSizing: "border-box", margin: 0, padding: 0 });
globalStyle("html, body, #root", { height: "100%" });
globalStyle("body", {
  fontFamily: vars.font.sans,
  background: vars.color.bg,
  color: vars.color.text,
  fontSize: "14px",
  lineHeight: 1.5,
  WebkitFontSmoothing: "antialiased",
  textRendering: "optimizeLegibility",
  overflow: "hidden",
  userSelect: "none",
});
globalStyle("::selection", { background: "rgba(99, 102, 241, 0.35)" });
globalStyle("button, input, select", {
  fontFamily: "inherit",
  fontSize: "inherit",
  color: "inherit",
});
globalStyle("*", {
  "@media": {
    "(prefers-reduced-motion: reduce)": { animationDuration: "0.001ms !important" },
  },
});
