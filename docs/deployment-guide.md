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

## Netplay server
Online co-op (M20) needs one small self-hosted service: the signaling server (`apps/netplay-server`)
plus coturn (STUN / TURN), deployed with Docker on the VPS that already serves other sites through
nginx. The desktop app defaults to `wss://netplay.nostyra.com` (`NETPLAY_DEFAULT_SERVER_URL`; a build
can override it with the build-time env `NOSTYRA_NETPLAY_SERVER`, a user under Settings → Netplay →
Advanced). Files: `deploy/netplay-server/` (its `README.md` is the short version of this section);
first-time VPS preparation (Docker, DNS, firewall, certbot placeholder vhost) is the plan's
`vps-prep-guide.md` (local, under `plans/`). It is never part of the portable zip and has nothing
to do with the release flow above.

### What runs where
| Piece | Where | Ports |
|---|---|---|
| `netplay-server` (container `nostyra/netplay-server:local`, built from the repo) | host loopback `127.0.0.1:3536` only | none public |
| nginx (already on the VPS) | vhost `netplay.nostyra.com`, certbot TLS, proxies REST + WebSocket to 3536 | 80 / 443 (already open) |
| coturn (`coturn/coturn:4.6.3`, `network_mode: host`) | STUN + TURN, plain (no TURNS in v1) | **3478/udp + 3478/tcp**, relay **49160–49400/udp** |

Do **not** open 3536 (loopback only) or 5349 (no TURNS). WebRTC data is DTLS-encrypted end to end
whether relayed or not.

### First deploy
```bash
# DNS: A (and AAAA if the VPS has IPv6) record netplay.nostyra.com -> VPS
sudo ufw allow 3478/tcp && sudo ufw allow 3478/udp && sudo ufw allow 49160:49400/udp
sudo ss -lunp | grep -E ':3478|:49[1-4][0-9]{2}'   # must print nothing before coturn starts
git clone https://github.com/vivusk/nostyra.git ~/nostyra   # build context = repo root
cd ~/nostyra/deploy/netplay-server
cp .env.example .env && chmod 600 .env
openssl rand -base64 48   # -> NETPLAY_SERVER_SECRET
openssl rand -base64 48   # -> TURN_SECRET
nano .env                 # paste both; TURN_RELAY_IP=<interface IP>[,<IPv6>]; TURN_EXTERNAL_IP=<public> only behind 1:1 NAT
docker compose up -d --build
docker compose ps         # netplay-server "healthy", coturn "running"
curl -s 127.0.0.1:3536/healthz   # -> ok
# TLS + vhost (see the header of nginx-netplay.conf.example):
sudo certbot certonly --nginx -d netplay.nostyra.com
sudo cp nginx-netplay.conf.example /etc/nginx/sites-available/netplay.nostyra.com
sudo ln -s /etc/nginx/sites-available/netplay.nostyra.com /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
# check (from any machine with curl; turnutils_uclient for the TURN part):
sh ~/nostyra/scripts/netplay-server-smoke.sh https://netplay.nostyra.com
NETPLAY_SMOKE_TURN=1 sh ~/nostyra/scripts/netplay-server-smoke.sh https://netplay.nostyra.com
```
If the other vhosts are IPv4 only, drop the two `listen [::]:…` lines of the snippet. After the
deploy, open the other sites once to make sure they still serve. Until a release carries
`deploy/netplay-server/` on `main`, copy the repo to the VPS instead of cloning.

### Environment (`.env`, git-ignored, never committed)
| Variable | Default | Meaning |
|---|---|---|
| `NETPLAY_SERVER_SECRET` | required, ≥ 32 bytes | HMAC key of room tokens (30 min TTL) |
| `TURN_SECRET` | required, ≥ 16 bytes | coturn `static-auth-secret`; minted TURN credentials live 2 h |
| `TURN_HOST` / `PUBLIC_HOST` | `netplay.nostyra.com` | host in the `stun:` / `turn:` urls / coturn realm |
| `TRUSTED_PROXY_CIDR` | `127.0.0.1/32,172.16.0.0/12` | socket peers whose `X-Real-IP` is believed (nginx via loopback / docker bridge); narrow it to this compose network's gateway if other containers on the VPS are untrusted |
| `MAX_ROOMS` / `MAX_SPECTATORS` | `500` / `8` | global room cap / spectators per room |
| `TURN_RELAY_IP` | required | coturn `listening-ip` + `relay-ip` (public IPv4 on the interface, optional `,IPv6`); keeps docker bridge IPs out of relay candidates |
| `TURN_EXTERNAL_IP` | unset | coturn `external-ip` behind 1:1 NAT |
| `RUST_LOG` | `info` | server log level |

### Secret rotation
- **`NETPLAY_SERVER_SECRET`**: edit `.env`, `docker compose up -d`. Rooms live in memory, so a restart
  drops open lobbies and closes the signaling socket of running sessions (they may end); rotate when
  nobody plays.
- **`TURN_SECRET`**: the server and coturn read the same value from `.env`; `docker compose up -d`
  restarts both. Credentials minted with the old secret stop working for new relay allocations.
- A leaked secret: rotate it at once, then check `docker compose logs` for unusual volume.

### Quotas and limits (built in)
Per client IP (IPv6: per /64): 10 room creates / min, 30 joins / min, 20 WebSocket connects / min,
60 status polls + starts / min (429 above). Rooms accept joins for 2 h; an unused room goes after
5 min, an emptied room 60 s after the last peer left. Per socket: 50 signaling messages / s (burst
200, then closed with 1008), 64 queued outbound messages, 64 KiB per message. Container: 256 MB
memory, 256 pids. coturn: `user-quota=12`, `total-quota=1200`, relay range 241 ports (about 20 fully
relayed 4-player rooms; raise `max-port` and the firewall range together if rooms hit the quota),
no TCP relay, private / loopback / CGNAT / link-local peers denied.

### Logs
- `docker compose logs -f --tail=100 netplay-server`: one line per event with an 8-hex hash of the
  room id; never room codes, tokens, IP addresses, ICE candidates or TURN credentials.
- `docker compose logs coturn`: coturn logs peer addresses; docker keeps 3 × 10 MB per container
  (bounded, rotated). nginx has `access_log off` for this vhost (URIs carry codes and the WebSocket token).
- Nothing is written to disk by the server.

### Upgrade
```bash
cd ~/nostyra && git pull
cd deploy/netplay-server && docker compose up -d --build
docker compose ps && curl -s 127.0.0.1:3536/healthz
sh ~/nostyra/scripts/netplay-server-smoke.sh https://netplay.nostyra.com
docker image prune -f      # drop the old image layers
```
A protocol change (`/v1` → `/v2`, lobby wire version) must stay compatible with the desktop
versions in use or ship together with a release that names the minimum version. Certificates renew
through certbot's timer (nginx reload is automatic). coturn image upgrades: bump the tag in
`docker-compose.yml`, `docker compose pull coturn && docker compose up -d`.

### Local run (development, no Docker)
```sh
NETPLAY_SERVER_SECRET=$(openssl rand -base64 48) TURN_SECRET=$(openssl rand -base64 24) \
TURN_HOST=127.0.0.1 cargo run -p netplay-server
sh scripts/netplay-server-smoke.sh            # http://127.0.0.1:3536
```
The launcher's room client is HTTPS-only, so a launcher-driven test needs a TLS endpoint; the runner
loopback test (`NOSTYRA_NETPLAY_LOOPBACK=1 NOSTYRA_NETPLAY_SERVER_URL=http://127.0.0.1:3536 cargo test
-p nostyra-runner --test netplay_loopback_tests`) talks to the plain local server directly.
