// Response headers of the web edition (nostyra.com/online) — the single source for
// `vite preview --mode web` (web-vite-plugin.mjs) and for the `vercel.json` `headers`
// block written by scripts/build-web.mjs. Rules never overlap for one header key on
// one path, so the result does not depend on how the host merges matching rules.
// `source` = Vercel pattern (path-to-regexp: plain regex groups, an optional `:name`
// before a group); `headersFor` runs the same patterns as regular expressions.

/** CSP directives measured in the phase 00 spike (row 1c). */
function csp(extra) {
  const directives = {
    "default-src": "'self'",
    "script-src": "'self' 'wasm-unsafe-eval' blob:",
    "worker-src": "'self' blob:",
    "connect-src": "'self' blob:",
    "img-src": "'self' data: blob:",
    "font-src": "'self' data:",
    "style-src": "'self'",
    "style-src-attr": "'unsafe-inline'",
    "media-src": "'self' blob:",
    "frame-src": "'self'",
    "object-src": "'none'",
    "base-uri": "'self'",
    "form-action": "'none'",
    "frame-ancestors": "'self'",
  };
  for (const [name, value] of Object.entries(extra)) directives[name] = `${directives[name]} ${value}`;
  return Object.entries(directives)
    .map(([name, value]) => `${name} ${value}`)
    .join("; ");
}

/** Launcher pages: artwork index (api.github.com) and images (raw.githubusercontent.com)
 *  of libretro-thumbnails; no eval. */
export const LAUNCHER_CSP = csp({
  "connect-src": "https://raw.githubusercontent.com https://api.github.com",
  "img-src": "https://raw.githubusercontent.com",
});
/** Player iframe: EmulatorJS's asm.js 7z/zip extractors need eval (phase 00 D3). */
export const PLAYER_CSP = csp({ "script-src": "'unsafe-eval'" });

const NO_CACHE = "no-cache";
const IMMUTABLE = "public, max-age=31536000, immutable";
// libretro-database/ paths are not versioned: one day, then revalidate (ETag).
const ONE_DAY = "public, max-age=86400, must-revalidate";

/** Header rules for one base path (no trailing slash), e.g. "/online". */
export function headerRules(base = "/online") {
  const p = (path) => `${base}${path}`;
  return [
    { source: p("/(.*)"), headers: { "X-Content-Type-Options": "nosniff", "Referrer-Policy": "no-referrer" } },
    {
      source: base,
      headers: {
        "X-Content-Type-Options": "nosniff",
        "Referrer-Policy": "no-referrer",
        "Content-Security-Policy": LAUNCHER_CSP,
        "Cache-Control": NO_CACHE,
      },
    },
    { source: p("/:page((?!player\\.html$).*)"), headers: { "Content-Security-Policy": LAUNCHER_CSP } },
    { source: p("/player\\.html"), headers: { "Content-Security-Policy": PLAYER_CSP } },
    { source: p("/:file((?!assets/|emulatorjs/|libretro-database/).*)"), headers: { "Cache-Control": NO_CACHE } },
    { source: p("/(assets|emulatorjs)/(.*)"), headers: { "Cache-Control": IMMUTABLE } },
    { source: p("/libretro-database/(.*)"), headers: { "Cache-Control": ONE_DAY } },
    { source: p("/(.*)\\.wasm"), headers: { "Content-Type": "application/wasm" } },
    { source: p("/(.*)\\.data"), headers: { "Content-Type": "application/octet-stream" } },
  ];
}

/** The `headers` array for the site's vercel.json. */
export function vercelHeaders(base = "/online") {
  return headerRules(base).map(({ source, headers }) => ({
    source,
    headers: Object.entries(headers).map(([key, value]) => ({ key, value })),
  }));
}

const patternOf = (source) => new RegExp(`^${source.replace(/:[A-Za-z]\w*(?=\()/g, "")}$`);

/** Headers for one request path (no query), in rule order. */
export function headersFor(pathname, base = "/online") {
  const result = {};
  for (const { source, headers } of headerRules(base)) {
    if (patternOf(source).test(pathname)) Object.assign(result, headers);
  }
  return result;
}
