import { execSync } from "node:child_process";
import react from "@vitejs/plugin-react";
import { vanillaExtractPlugin } from "@vanilla-extract/vite-plugin";
import { defineConfig } from "vite";

/** Short build stamp shown in Settings. Falls back to the CI-provided SHA, then "dev". */
function buildStamp(): string {
  try {
    return execSync("git rev-parse --short=8 HEAD", { encoding: "utf-8" }).trim();
  } catch {
    return (process.env.GIT_SHA || "dev").slice(0, 8);
  }
}

// Tauri serves the frontend on a fixed port in dev and bundles `dist/` for release.
export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(buildStamp()),
  },

  plugins: [
    // vanilla-extract — zero-runtime CSS-in-TypeScript. Compiles every `*.css.ts`
    // module to static CSS extracted at build time (no runtime style injection).
    vanillaExtractPlugin(),

    // React + the React Compiler (automatic memoization). target "19" — React 19
    // ships the compiler runtime (`react/compiler-runtime`) natively, no polyfill.
    react({
      babel: {
        plugins: [["babel-plugin-react-compiler", { target: "19" }]],
      },
    }),
  ],

  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },

  build: {
    // The Tauri webview is always modern (WKWebView / WebView2 / webkitgtk) — no
    // need to down-level for legacy browsers.
    target: "es2022",
    // lightningcss dedups selectors + compresses colors ~20% better than esbuild's
    // CSS minifier — a good match for vanilla-extract's generated styles.
    cssMinify: "lightningcss",
    sourcemap: false,
    assetsInlineLimit: 4096,
  },

  resolve: {
    // One copy of React even if a transitive dep pulls its own (two Reacts break
    // hook identity).
    dedupe: ["react", "react-dom"],
  },
});
