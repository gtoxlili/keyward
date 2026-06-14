import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri serves the frontend on a fixed port in dev and bundles `dist/` for release.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2021",
    sourcemap: false,
  },
});
