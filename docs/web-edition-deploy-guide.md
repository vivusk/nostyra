# Nostyra Online — deploy guide (for the nostyra.com website project)

This package holds a finished static build of **Nostyra Online**, the browser edition of
Nostyra, to be served at **`https://nostyra.com/online/`**. It is written for whoever (a
person or a coding agent) works in the **nostyra.com website repository**, without access
to the Nostyra source repository. Nothing in it is built, bundled or minified again.

Package `nostyra-web-<version>/` (also as `nostyra-web-<version>.zip`):

| Path | What |
|---|---|
| `online/` | Every file served under `/online/` (≈ 32 MB, ~125 files; zip ≈ 13 MB). Copy as is. |
| `vercel.headers.json` | `{ "headers": [...] }` — response headers to merge into the site's `vercel.json`. |
| `DEPLOY.md` | This guide. |
| `build-info.json` | Version, Nostyra commit, `sourceTagMatches`, bytes per part. |

## 0. Before you start
- Read `build-info.json`. If `sourceTagMatches` is `false`, the build is not the published
  source of tag `v<version>` that the licences page links (GPL-3.0). Ask the owner to
  release the source first and send a new package; a private preview deploy is fine.
- Find out how the site deploys: the output folder Vercel serves (e.g. `public/`, `dist/`,
  `out/`, or the repo root for a plain static site) and whether the site is deployed by
  hand (`vercel deploy --prod` / `--prebuilt`) or by Git pushes. **Nostyra Online must be
  deployed by hand** (see § 4); if the site only deploys through Git, ask the owner.

## 1. Copy the files
Copy the package's `online/` folder so that it becomes `<site static output>/online/`:
`online/index.html` must be served at `https://nostyra.com/online/` and
`online/emulatorjs/4.2.3/loader.js` at `https://nostyra.com/online/emulatorjs/4.2.3/loader.js`.
- Replace an older `online/` folder completely (delete it first): file names are hashed.
- Keep every file byte for byte: no build step, no minifier, no image/asset optimiser, no
  formatter, no "clean URLs" rename. `.data`, `.wasm`, `.dat` and `.txt` files are binary or
  data files the app fetches by exact name.
- If the site has a framework build (Next.js, Astro, Vite, …), put `online/` in the folder
  it copies verbatim (`public/` in most frameworks) and check the built output contains
  `online/emulatorjs/4.2.3/cores/*.data` unchanged.
- If the site's repo ignores large or binary files (`.gitignore`, Git LFS rules), make sure
  `online/**` is kept; total ≈ 32 MB, largest file ≈ 4 MB (`libretro-database/Sony - PlayStation.dat`).

## 2. Merge the headers into `vercel.json`
Append every object of `vercel.headers.json` → `headers` to the `headers` array of the
site's `vercel.json` (create the array if missing). Keep the order. Do not edit the values:
- Two Content-Security-Policies: the launcher pages (no `'unsafe-eval'`) and
  `/online/player.html` (needs `'unsafe-eval'` for the EmulatorJS 7z/zip extractors). The
  rules never set the same header twice for one path.
- `Cache-Control`: pages `no-cache`; `online/assets/*` (hashed) and
  `online/emulatorjs/4.2.3/*` (versioned) one year `immutable`; `online/libretro-database/*`
  one day. `Content-Type` for `.wasm` and `.data`; `X-Content-Type-Options: nosniff`;
  `Referrer-Policy: no-referrer`.
- If an existing site rule for `/(.*)` also sets `Content-Security-Policy`,
  `Cache-Control` or `Referrer-Policy`, exclude `/online` from it (e.g. change its source
  to `/:path((?!online(?:/|$)).*)`), or the two policies combine and break the player.
- Do **not** add `Cross-Origin-Opener-Policy` / `Cross-Origin-Embedder-Policy` to
  `/online` (not needed; COEP would block the GitHub artwork).
- Do **not** add `X-Frame-Options: DENY` to `/online/player.html`: the launcher shows it in
  a same-origin iframe (`SAMEORIGIN` is fine; the CSP already has `frame-ancestors 'self'`).

## 3. What must NOT happen
- **No rewrite or SPA fallback may catch `/online/**`.** A rule like
  `{"source": "/(.*)", "destination": "/index.html"}` turns a missing or mistyped file into
  HTML with status 200: DAT files then parse as empty (games are not identified), cores fail
  to start ("Failed to start game"), wasm fails to compile. Exclude `/online` from such
  rewrites (`/:path((?!online(?:/|$)).*)`). The web edition has no client-side routes:
  every URL under `/online/` is a real file.
- No `trailingSlash: false` redirect loop: `/online` and `/online/` must both end at
  `online/index.html` (Vercel's default does). With `"trailingSlash": false` site-wide, add
  a redirect `/online` → `/online/` only if the page does not load at `/online`.
- No `cleanUrls` effect on `/online/player.html`, `/online/licences.html`: the app loads
  `player.html` by that exact name (with `cleanUrls: true` Vercel redirects `.html` URLs;
  check § 5 still gives 200, else exclude `/online`).
- No GitHub Actions, CI job or Git-triggered build for this. Nostyra's GitHub Actions are
  disabled on purpose; do not add or trigger any.
- No middleware / edge function on `/online/**`, no analytics or third-party script
  injected into these pages (the CSP blocks them and the page promises nothing leaves the
  computer).

## 4. Deploy (by hand)
From the site project (Vercel CLI logged in, project linked):
- Static site without a framework build: `vercel deploy --prod`.
- Prebuilt flow (the site is built locally): build the site as usual (`vercel build --prod`),
  confirm `.vercel/output/static/online/index.html` exists, then `vercel deploy --prebuilt --prod`.
- Try `vercel deploy` (preview, no `--prod`) first and run § 5 against the preview URL.

Size limits relied on (https://vercel.com/docs/limits, read 2026-11-06): CLI "static file
uploads" up to 100 MB on Hobby / 1 GB on Pro, at most 15,000 files per deployment. The page
does not say whether 100 MB is per file or per upload; this package is ≈ 32 MB in ~125
files with the largest ≈ 4 MB, so it fits either reading — but together with the rest of
the site a Hobby deployment may come close to 100 MB.

## 5. Post-deploy checklist
Replace `$SITE` with `https://nostyra.com` (or the preview URL).
```sh
SITE=https://nostyra.com
curl -sI "$SITE/online/" | grep -iE '^(HTTP|content-type|content-security-policy|cache-control)'
#   200, text/html, CSP without 'unsafe-eval', Cache-Control: no-cache
curl -sI "$SITE/online/player.html" | grep -i content-security-policy
#   script-src 'self' 'wasm-unsafe-eval' blob: 'unsafe-eval'
curl -sI "$SITE/online/$(curl -s "$SITE/online/" | grep -o 'assets/index-[^"]*\.js' | head -1)" | grep -iE '^(HTTP|cache-control)'
#   200, public, max-age=31536000, immutable
curl -sI "$SITE/online/emulatorjs/4.2.3/cores/snes9x-legacy-wasm.data" | grep -iE '^(HTTP|content-type|content-length)'
#   200, application/octet-stream, ~1.1 MB (not text/html)
curl -s "$SITE/online/libretro-database/Nintendo%20-%20Game%20Boy.dat" | head -c 200
#   clrmamepro ( name "Nintendo - Game Boy" ...  — NOT "<!doctype html>"
curl -s -o /dev/null -w '%{http_code} %{content_type}\n' "$SITE/online/libretro-database/does-not-exist.dat"
#   404 (a 200 text/html here means an SPA fallback is catching /online, see § 3)
curl -sI "$SITE/online/licences.html" | head -1
#   200
```
For the `.wasm` MIME, open DevTools → Network on `/online/`, filter `wasm`: both
`assets/nostyra_web_bg-*.wasm` and `assets/sqlite3-*.wasm` must be `200`,
`application/wasm` (or take a name from the folder: `ls online/assets/*.wasm` and
`curl -sI "$SITE/online/assets/<name>.wasm"`).

Browser check (the owner, or an agent with a real browser):
1. Desktop **Chrome or Edge**: open `https://nostyra.com/online/` → "Set up your library",
   footer "Your ROMs and saves never leave your computer." + "Licences and source code".
   DevTools console: no `Content-Security-Policy` errors.
2. "Choose folder" → pick a ROM folder → allow → the library fills (scan notice, then games).
3. Step 2 "Where to keep saves and settings" → pick a folder (e.g. `Documents\Nostyra`) or
   browser storage → Library. Play a game: it starts in the page; quit with "Exit game".
4. Open `/online/` in a **second tab** → "Nostyra Online is already open".
5. **Firefox** (or Safari, or a phone) → "Nostyra Online needs Chrome or Edge on a
   computer" with a link to nostyra.com. (Brave with default settings shows the same page
   plus a hint about `brave://flags/#file-system-access-api`.)
6. `https://nostyra.com/online/licences.html` lists Nostyra, EmulatorJS 4.2.3, the five
   cores, libretro-database and the notices links (each link opens).

## 6. Troubleshooting
| Symptom | Cause / fix |
|---|---|
| Console: "Refused to evaluate a string as JavaScript" in the player, game never starts | `/online/player.html` did not get its own CSP (rule order changed, or a site-wide CSP is merged in). § 2. |
| Games stay "needs review", DAT errors | `libretro-database/*.dat` answered with HTML (SPA fallback) or were modified. § 3. |
| "Failed to start game" for every system | Core `.data` files rewritten, compressed again or answered with HTML. Check § 5 `.data` curl. |
| Page loads an old version | Browser cached `index.html`: check `Cache-Control: no-cache` on `/online/`. |
| A PlayStation pad does nothing | Steam Input (Steam running with PlayStation controller support) or DS4Windows / HidHide hides the pad from the browser: exit Steam or turn off Steam Input for PlayStation controllers, reconnect the pad, press a button on the page. |
| "Can't open this folder … contains system files" | Chrome refuses folders under `AppData`; pick e.g. `Documents\Nostyra`. |

## 7. Updating later
A new package replaces `online/` completely (§ 1) and its `vercel.headers.json` replaces
the previous `/online` header rules (§ 2); then § 4 and § 5 again.
