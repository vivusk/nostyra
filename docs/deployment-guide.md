# Deployment guide — releases

Nostyra ships as a Windows x64 portable zip on GitHub Releases: <https://github.com/vivusk/nostyra/releases>.

## Branch model
- **`dev`** — all daily work (local only, never pushed). Full commit history lives here.
- **`main`** — one commit per release, pushed to GitHub together with the release tag (`v<major>.<minor>.<patch>`). Each release commit has exactly dev's tree at release time and the previous release as parent (the first release, v0.1.0, has none).
- Other local branches (`spike/*`, `hd-remaster`) are never pushed.
- CI (`.github/workflows/ci.yml`) runs on pushes to `main` and on pull requests.
- A pull request on GitHub targets `main`; bring its change into `dev` by hand (cherry-pick / re-apply), then it ships with the next release.

## Releasing a version
1. On `dev`, bump the version in all three places (RetroAchievements requires an increasing numeric version in the User-Agent, built from `CARGO_PKG_VERSION`):
   - `Cargo.toml` → `[workspace.package] version`
   - `apps/launcher/src-tauri/tauri.conf.json` → `version`
   - `apps/launcher/package.json` → `version`
2. Write `docs/release-notes-v<version>.md` (features, systems Stable/Experimental, known limitations, privacy, reporting bugs) and add a `docs/project-changelog.md` entry. Commit on `dev`.
3. Run the gates (fmt, clippy `-D warnings`, workspace tests, `npm run typecheck` + `npm run build`).
4. `powershell -File scripts/release.ps1 <version>` — checks the branch, a clean tree, the three versions and the release notes; creates the release commit on `main` with `git commit-tree` (never conflicts; deletions carry over) and the annotated tag `v<version>`. It never pushes.
5. Build the zip: `powershell -File scripts/build-portable.ps1` (needs `cargo-about` installed and `npm ci` in `apps/launcher`; `dev` has the same tree as the release commit, so the zip matches the tagged source as GPL-3.0 requires). The Google Drive OAuth client (`NOSTYRA_GOOGLE_CLIENT_ID` / `NOSTYRA_GOOGLE_CLIENT_SECRET`, a Google "Desktop app" client of the published Nostyra project, scope `drive.file`) must be in the user environment: the script embeds it at build time and warns when it is missing (that build then has no Google Drive); check with `Select-String -Path targetelease
ostyra-launcher.exe -Pattern apps.googleusercontent.com -SimpleMatch -Quiet` style search of the exe. Never commit the client secret (GitHub push protection flags `GOCSPX-`).
6. Push only main and the tag: `git push origin main v<version>`.
7. GitHub → Releases → Draft a new release → tag `v<version>`, paste the release notes, attach `dist/Nostyra-<version>-windows-x64-portable.zip`, publish.

## If something went wrong before pushing
The release commit and tag exist only locally until step 6: delete the tag (`git tag -d v<version>`) and move `main` back (`git update-ref refs/heads/main <previous release commit>`), fix on `dev`, run the script again.

## Web edition (nostyra.com/play)
The web edition is a static build deployed by hand into the **separate nostyra.com website
project on Vercel** — never by Git-triggered builds or GitHub Actions.
1. Release first (above) so the tag `v<version>` holds this source: the web licences page links
   `https://github.com/vivusk/nostyra/tree/v<version>` (GPL-3.0). The build warns when it does not.
2. `powershell -File scripts/build-web.ps1` (or `bash scripts/build-web.sh`; needs wasm-pack
   0.15.0, cargo-about 0.9.2, `npm ci` in `apps/launcher`, network for the first EmulatorJS
   fetch) → `dist/nostyra-web-<version>/` + `.zip`, size per part printed.
3. Optional local check under the production headers: `cd apps/launcher && npx vite preview
   --mode web --port 4174 --strictPort` → `http://localhost:4174/play/`.
4. Hand the package to whoever works in the website repo: its `DEPLOY.md`
   ([web-edition-deploy-guide.md](web-edition-deploy-guide.md)) says where `play/` goes, how to
   merge `vercel.headers.json`, what must not happen (SPA fallback over `/play`, rebuilding or
   minifying the files, CI) and the post-deploy `curl` + browser checklist.
