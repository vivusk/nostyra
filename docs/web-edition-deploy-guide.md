# Nostyra Online — deploy guide (for the nostyra.com website project)

This package holds a finished static build of **Nostyra Online**, the browser edition of
Nostyra, to be served at **`https://nostyra.com/play/`**. It is written for whoever (a
person or a coding agent) works in the **nostyra.com website repository**, without access
to the Nostyra source repository. Nothing in it is built, bundled or minified again.

Package `nostyra-web-<version>/` (also as `nostyra-web-<version>.zip`):

| Path | What |
|---|---|
| `play/` | Every file served under `/play/` (≈ 35 MB, ~150 files with the Atari cores; the build prints the exact numbers). Copy as is. |
| `vercel.headers.json` | `{ "headers": [...] }` — response headers to merge into the site's `vercel.json`. |
| `DEPLOY.md` | This guide. |
| `build-info.json` | Version, Nostyra commit, `basePath` (`/play/`), `sourceTagMatches`, bytes per part. |

## 0. Before you start
- Read `build-info.json`. If `sourceTagMatches` is `false`, the build is not the published
  source of tag `v<version>` that the licences page links (GPL-3.0). Ask the owner to
  release the source first and send a new package; a private preview deploy is fine.
- Find out how the site deploys: the output folder Vercel serves (e.g. `public/`, `dist/`,
  `out/`, or the repo root for a plain static site) and whether the site is deployed by
  hand (`vercel deploy --prod` / `--prebuilt`) or by Git pushes. **Nostyra Online must be
  deployed by hand** (see § 4); if the site only deploys through Git, ask the owner.

## Migrating from /online/
Packages up to 0.3.0 were served at `/online/`; from 0.3.1 Nostyra Online lives at **`/play/`**
(the name "Nostyra Online" is unchanged).
- Copy the new package's `play/` folder (§ 1) and replace the old `/online` header rules with the
  new `vercel.headers.json` (§ 2). Remove the old `online/` folder afterwards.
- Add one **permanent (308) redirect** so old bookmarks keep working, keeping the sub-path:
  ```json
  { "redirects": [ { "source": "/online/:path*", "destination": "/play/:path*", "permanent": true } ] }
  ```
  Also send the bare `/online` there (the rule above covers `/online/…`; add
  `{ "source": "/online", "destination": "/play/", "permanent": true }` if the site has none).
  No rewrite and no SPA fallback for `/online/**` — a redirect only.
- **Players keep their data.** Everything the page stores — the library database (OPFS, SQLite
  `opfs-sahpool` VFS), browser-storage saves, the remembered ROM / Data folder handles and their
  "Allow on every visit" permission (IndexedDB `nostyra-online`), settings, and the
  "already open in another tab" lock (Web Lock `nostyra-online`) — belongs to the **origin**
  `https://nostyra.com`, not to the URL path, so it is found again at `/play/`. These names keep
  the word "online" on purpose: renaming them would lose that data. There is no service worker
  and no Cache Storage. Only the browser's HTTP cache of old `/online/` files becomes unused.
- A tab still open on `/online/` during the switch holds the tab lock: `/play/` then says
  "already open in another tab" until that old tab is closed or reloaded (it redirects).

## 1. Copy the files
Copy the package's `play/` folder so that it becomes `<site static output>/play/`:
`play/index.html` must be served at `https://nostyra.com/play/` and
`play/emulatorjs/4.2.3/loader.js` at `https://nostyra.com/play/emulatorjs/4.2.3/loader.js`.
- Replace an older `play/` folder completely (delete it first): file names are hashed.
- Keep every file byte for byte: no build step, no minifier, no image/asset optimiser, no
  formatter, no "clean URLs" rename. `.data`, `.wasm`, `.dat` and `.txt` files are binary or
  data files the app fetches by exact name.
- If the site has a framework build (Next.js, Astro, Vite, …), put `play/` in the folder
  it copies verbatim (`public/` in most frameworks) and check the built output contains
  `play/emulatorjs/4.2.3/cores/*.data` unchanged (eight cores, incl. `stella2014-`, `prosystem-` and
  `handy-legacy-wasm.data` for the Atari systems) and `play/notices/` (licence texts the licences page links,
  e.g. `GPL-2.0.txt`, `Handy-zlib.txt`). There is no PlayStation 2 core: PS2 (M18) is desktop-only (EmulatorJS has
  none), so nothing PS2-related is deployed and PS2 discs in a folder only count towards "N games need the
  desktop app".
- If the site's repo ignores large or binary files (`.gitignore`, Git LFS rules), make sure
  `play/**` is kept; total ≈ 35 MB, largest file ≈ 4 MB (`libretro-database/Sony - PlayStation.dat`).

## 2. Merge the headers into `vercel.json`
Append every object of `vercel.headers.json` → `headers` to the `headers` array of the
site's `vercel.json` (create the array if missing). Keep the order. Do not edit the values:
- Two Content-Security-Policies: the launcher pages (no `'unsafe-eval'`) and
  `/play/player.html` (needs `'unsafe-eval'` for the EmulatorJS 7z/zip extractors). The
  rules never set the same header twice for one path.
- `Cache-Control`: pages `no-cache`; `play/assets/*` (hashed) and
  `play/emulatorjs/4.2.3/*` (versioned) one year `immutable`; `play/libretro-database/*`
  one day. `Content-Type` for `.wasm` and `.data`; `X-Content-Type-Options: nosniff`;
  `Referrer-Policy: no-referrer`.
- If an existing site rule for `/(.*)` also sets `Content-Security-Policy`,
  `Cache-Control` or `Referrer-Policy`, exclude `/play` from it (e.g. change its source
  to `/:path((?!play(?:/|$)).*)`), or the two policies combine and break the player.
- Do **not** add `Cross-Origin-Opener-Policy` / `Cross-Origin-Embedder-Policy` to
  `/play` (not needed; COEP would block the GitHub artwork).
- Do **not** add `X-Frame-Options: DENY` to `/play/player.html`: the launcher shows it in
  a same-origin iframe (`SAMEORIGIN` is fine; the CSP already has `frame-ancestors 'self'`).

## 3. What must NOT happen
- **No rewrite or SPA fallback may catch `/play/**`.** A rule like
  `{"source": "/(.*)", "destination": "/index.html"}` turns a missing or mistyped file into
  HTML with status 200: DAT files then parse as empty (games are not identified), cores fail
  to start ("Failed to start game"), wasm fails to compile. Exclude `/play` from such
  rewrites (`/:path((?!play(?:/|$)).*)`). The web edition has no client-side routes:
  every URL under `/play/` is a real file.
- No `trailingSlash: false` redirect loop: `/play` and `/play/` must both end at
  `play/index.html` (Vercel's default does). With `"trailingSlash": false` site-wide, add
  a redirect `/play` → `/play/` only if the page does not load at `/play`.
- No `cleanUrls` effect on `/play/player.html`, `/play/licences.html`: the app loads
  `player.html` by that exact name (with `cleanUrls: true` Vercel redirects `.html` URLs;
  check § 5 still gives 200, else exclude `/play`).
- No GitHub Actions, CI job or Git-triggered build for this. Nostyra's GitHub Actions are
  disabled on purpose; do not add or trigger any.
- No middleware / edge function on `/play/**`, no analytics or third-party script
  injected into these pages (the CSP blocks them and the page promises nothing leaves the
  computer).

## 4. Deploy (by hand)
From the site project (Vercel CLI logged in, project linked):
- Static site without a framework build: `vercel deploy --prod`.
- Prebuilt flow (the site is built locally): build the site as usual (`vercel build --prod`),
  confirm `.vercel/output/static/play/index.html` exists, then `vercel deploy --prebuilt --prod`.
- Try `vercel deploy` (preview, no `--prod`) first and run § 5 against the preview URL.

Size limits relied on (https://vercel.com/docs/limits, read 2026-11-06): CLI "static file
uploads" up to 100 MB on Hobby / 1 GB on Pro, at most 15,000 files per deployment. The page
does not say whether 100 MB is per file or per upload; this package is ≈ 35 MB in ~150
files with the largest ≈ 4 MB, so it fits either reading — but together with the rest of
the site a Hobby deployment may come close to 100 MB.

## 5. Post-deploy checklist
Replace `$SITE` with `https://nostyra.com` (or the preview URL).
```sh
SITE=https://nostyra.com
curl -sI "$SITE/play/" | grep -iE '^(HTTP|content-type|content-security-policy|cache-control)'
#   200, text/html, CSP without 'unsafe-eval', Cache-Control: no-cache
curl -sI "$SITE/play/player.html" | grep -i content-security-policy
#   script-src 'self' 'wasm-unsafe-eval' blob: 'unsafe-eval'
curl -sI "$SITE/play/$(curl -s "$SITE/play/" | grep -o 'assets/index-[^"]*\.js' | head -1)" | grep -iE '^(HTTP|cache-control)'
#   200, public, max-age=31536000, immutable
curl -sI "$SITE/play/emulatorjs/4.2.3/cores/snes9x-legacy-wasm.data" | grep -iE '^(HTTP|content-type|content-length)'
#   200, application/octet-stream, ~1.1 MB (not text/html)
curl -s "$SITE/play/libretro-database/Nintendo%20-%20Game%20Boy.dat" | head -c 200
#   clrmamepro ( name "Nintendo - Game Boy" ...  — NOT "<!doctype html>"
curl -s -o /dev/null -w '%{http_code} %{content_type}\n' "$SITE/play/libretro-database/does-not-exist.dat"
#   404 (a 200 text/html here means an SPA fallback is catching /play, see § 3)
curl -sI "$SITE/play/licences.html" | head -1
#   200
```
For the `.wasm` MIME, open DevTools → Network on `/play/`, filter `wasm`: both
`assets/nostyra_web_bg-*.wasm` and `assets/sqlite3-*.wasm` must be `200`,
`application/wasm` (or take a name from the folder: `ls play/assets/*.wasm` and
`curl -sI "$SITE/play/assets/<name>.wasm"`).

Browser check (the owner, or an agent with a real browser):
1. Desktop **Chrome or Edge**: open `https://nostyra.com/play/` → "Set up your library",
   footer "Your ROMs and saves never leave your computer." + "Licences and source code".
   DevTools console: no `Content-Security-Policy` errors.
2. "Choose folder" → pick a ROM folder → allow → the library fills (scan notice, then games).
3. Step 2 "Where to keep saves and settings" → pick a folder (e.g. `Documents\Nostyra`) or
   browser storage → Library. Play a game: it starts in the page; quit with "Exit game".
4. Open `/play/` in a **second tab** → "Nostyra Online is already open".
5. **Firefox** (or Safari, or a phone) → "Nostyra Online needs Chrome or Edge on a
   computer" with a link to nostyra.com. (Brave with default settings shows the same page
   plus a hint about `brave://flags/#file-system-access-api`.)
6. `https://nostyra.com/play/licences.html` lists Nostyra, EmulatorJS 4.2.3, the eight
   cores (incl. Stella 2014, ProSystem, Handy), libretro-database and the notices links (each link opens).

## 6. Troubleshooting
| Symptom | Cause / fix |
|---|---|
| Console: "Refused to evaluate a string as JavaScript" in the player, game never starts | `/play/player.html` did not get its own CSP (rule order changed, or a site-wide CSP is merged in). § 2. |
| Games stay "needs review", DAT errors | `libretro-database/*.dat` answered with HTML (SPA fallback) or were modified. § 3. |
| "Failed to start game" for every system | Core `.data` files rewritten, compressed again or answered with HTML. Check § 5 `.data` curl. |
| Page loads an old version | Browser cached `index.html`: check `Cache-Control: no-cache` on `/play/`. |
| A PlayStation pad does nothing | Steam Input (Steam running with PlayStation controller support) or DS4Windows / HidHide hides the pad from the browser: exit Steam or turn off Steam Input for PlayStation controllers, reconnect the pad, press a button on the page. |
| "Can't open this folder … contains system files" | Chrome refuses folders under `AppData`; pick e.g. `Documents\Nostyra`. |

## 7. Updating later
A new package replaces `play/` completely (§ 1) and its `vercel.headers.json` replaces
the previous `/play` header rules (§ 2); then § 4 and § 5 again.
