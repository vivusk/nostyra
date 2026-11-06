import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { headersFor } from "./web-headers.mjs";

/** Placeholder in public/licences.html replaced by the release version at build. */
const VERSION_MARK = "__NOSTYRA_VERSION__";

/**
 * Web edition only (`--mode web`):
 * - index.html loads public/browser-gate.js (classic script) before the app module;
 * - `vite preview` sends the production headers per path (web-headers.mjs; Vite's own
 *   `preview.headers` are global and cannot give the player page its own CSP);
 * - the built licences.html names the release version (source link to the tag).
 * @returns {import("vite").Plugin}
 */
export function nostyraWebPlugin() {
  /** @type {import("vite").ResolvedConfig} */
  let config;
  return {
    name: "nostyra-web",
    configResolved(resolved) {
      config = resolved;
    },
    transformIndexHtml: {
      order: "post",
      handler: () => [
        { tag: "script", attrs: { src: `${config.base}browser-gate.js` }, injectTo: "head-prepend" },
      ],
    },
    configurePreviewServer(server) {
      const base = server.config.base.replace(/\/$/, "");
      server.middlewares.use((req, res, next) => {
        const path = decodeURIComponent((req.url ?? "/").split("?")[0]);
        const headers = headersFor(path, base);
        // Set now and again when the static server writes its own (sirv sets
        // Content-Type / Cache-Control itself): ours win, as on the host.
        for (const [key, value] of Object.entries(headers)) res.setHeader(key, value);
        const ours = new Map(Object.entries(headers).map(([key, value]) => [key.toLowerCase(), value]));
        const setHeader = res.setHeader.bind(res);
        res.setHeader = (key, value) => setHeader(key, ours.get(key.toLowerCase()) ?? value);
        next();
      });
    },
    closeBundle() {
      const file = join(resolve(config.root, config.build.outDir), "licences.html");
      const version = JSON.parse(readFileSync(join(config.root, "package.json"), "utf8")).version;
      writeFileSync(file, readFileSync(file, "utf8").replaceAll(VERSION_MARK, version));
    },
  };
}
