import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import {fileURLToPath} from "node:url";

export default defineConfig({
  plugins: [react()],
  // Adapter to the pinned dependency's CSP-safe decoder; our worker owns bounds and lifetime.
  resolve:{alias:{"heic-preview-decoder":fileURLToPath(new URL("./node_modules/heic-to/src/lib/libheif-without-unsafe-eval.js",import.meta.url))}},
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_"],
  build: { target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13" }
});
