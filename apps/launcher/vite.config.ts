import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { WEB_BASE_PATH } from "./web-headers.mjs";
import { nostyraWebPlugin } from "./web-vite-plugin.mjs";

// Two editions from one React app (src/backend/backend-contract.ts):
// - default: the desktop launcher inside Tauri (fixed dev port = tauri.conf.json
//   `devUrl`; no reloads when Rust sources under src-tauri change);
// - `--mode web`: nostyra.com/play, served under WEB_BASE_PATH (web-headers.mjs,
//   default /play), built to dist-web/,
//   with the browser backend (never imports @tauri-apps/*).
export default defineConfig(({ mode }) => {
  const web = mode === "web";
  // Root-relative ("/src/…"): Vite resolves it from the project root.
  const backend = web ? "/src/backend/web/index.ts" : "/src/backend/tauri/index.ts";
  return {
    // Web: browser gate, per-path preview headers, licences version (web-vite-plugin.mjs).
    plugins: web ? [react(), tailwindcss(), nostyraWebPlugin()] : [react(), tailwindcss()],
    base: web ? `${WEB_BASE_PATH}/` : "/",
    // public/ = web-only static files (libretro-database copy, EmulatorJS, player,
    // browser gate, licences page);
    // the desktop bundle never ships them.
    publicDir: web ? "public" : false,
    resolve: {
      alias: { "@nostyra/backend": backend },
    },
    build: web ? { outDir: "dist-web", emptyOutDir: true } : undefined,
    // Web: no index.html fallback for missing files (the host has none either: a missing
    // DAT or core must be a 404, never HTML).
    appType: web ? ("mpa" as const) : ("spa" as const),
    // Web workers (SQLite, scan) are ES modules; sqlite-wasm loads its own .wasm.
    worker: { format: "es" as const },
    optimizeDeps: { exclude: ["@sqlite.org/sqlite-wasm"] },
    clearScreen: false,
    server: {
      port: 1420,
      strictPort: true,
      watch: { ignored: ["**/src-tauri/**"] },
      // The web DB reads the desktop migrations from crates/storage-db.
      fs: { allow: ["..", "../.."] },
    },
  };
});
