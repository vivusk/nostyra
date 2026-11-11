# Netplay server deployment (M20)

Self-hosted signaling server (`apps/netplay-server`) + coturn (STUN/TURN) for
`netplay.nostyra.com`, behind the VPS's existing nginx. Content is merged into
`docs/deployment-guide.md` ("Netplay server") in phase 06.

## What runs where
| Piece | Where | Ports |
|---|---|---|
| `netplay-server` (docker) | `127.0.0.1:3536` on the host (loopback only) | none public |
| nginx (already there) | TLS for `netplay.nostyra.com`, proxies REST + WebSocket to 3536 | 443 (already open) |
| coturn (docker, host network) | STUN + TURN, plain (no TURNS in v1) | 3478 udp + tcp, relay 49160–49400 udp |

## Steps (manual task M0)
1. DNS: `A` (and `AAAA` if the VPS has IPv6) record `netplay.nostyra.com` → VPS.
2. Firewall: open **3478/tcp**, **3478/udp**, **49160–49400/udp** (443 is already open).
   Check nothing else uses them: `ss -lunp | grep -E ':3478|:49[1-4][0-9]{2}'`.
3. Secrets: `cp .env.example .env`, fill `NETPLAY_SERVER_SECRET` and `TURN_SECRET`
   (`openssl rand -base64 48` each). `.env` is git-ignored and must never be committed.
   Set `TURN_RELAY_IP` to the public IP shown by `ip addr` (add `,<IPv6>` to relay over IPv6);
   if `ip addr` does not show the public IP (1:1 NAT), use the interface IP and set `TURN_EXTERNAL_IP`.
4. Start: from this folder, `docker compose up -d --build` (build context = repo root).
   `docker compose ps` shows `netplay-server` healthy; `curl -s 127.0.0.1:3536/healthz` → `ok`.
5. TLS + vhost: follow the header of `nginx-netplay.conf.example`
   (`certbot certonly --nginx -d netplay.nostyra.com`, copy, enable, `nginx -t`, reload).
   If the other sites do not listen on IPv6, drop the two `listen [::]:…` lines.
6. Check: `scripts/netplay-server-smoke.sh https://netplay.nostyra.com` (curl only);
   with `NETPLAY_SMOKE_TURN=1` and coturn's `turnutils_uclient` installed it also checks
   that coturn accepts the minted TURN credentials. Then make sure the other sites still serve.

## Configuration (`.env`)
| Variable | Default | Meaning |
|---|---|---|
| `NETPLAY_SERVER_SECRET` | required, ≥ 32 bytes | HMAC key of room tokens (30 min TTL) |
| `TURN_SECRET` | required, ≥ 16 bytes | coturn `static-auth-secret`; TURN creds live 2 h |
| `TURN_HOST` | `netplay.nostyra.com` | host in the `stun:` / `turn:` urls |
| `PUBLIC_HOST` | `netplay.nostyra.com` | coturn realm |
| `TRUSTED_PROXY_CIDR` | `127.0.0.1/32,172.16.0.0/12` | socket peers whose `X-Real-IP` is believed (nginx via loopback / docker bridge). The default also trusts other containers on the VPS (any docker network in 172.16/12); narrow it to this compose network's gateway if other stacks are untrusted |
| `MAX_ROOMS` / `MAX_SPECTATORS` | `500` / `8` | global room cap / spectators per room |
| `NETPLAY_BIND` | `127.0.0.1:3536` (`0.0.0.0:3536` in the container) | listen address |
| `TURN_RELAY_IP` | required | coturn `listening-ip` + `relay-ip` (public IPv4 on the interface, optional `,IPv6`); keeps docker bridge IPs out of relay candidates |
| `TURN_EXTERNAL_IP` | unset | coturn `external-ip` behind 1:1 NAT |

## API (v1)
`POST /v1/rooms` · `POST /v1/rooms/{code}/join` · `GET /v1/rooms/{code}` ·
`POST /v1/rooms/{code}/start` (`Authorization: Bearer <host token>`) ·
`GET /v1/ws/{room_id}?token=…` (matchbox signaling) · `GET /healthz`.
Errors are `{"error": "<code>"}`: 400 `bad_*`, 401 `missing_token`, 403 `bad_token` /
`not_host`, 404 `room_not_found`, 409 `room_full` / `room_started` / `spectators_off` /
`already_connected`, 410 `room_expired`, 429 (rate limit), 503 `server_full`.
Not every refusal has a JSON body: malformed / non-JSON requests (400 / 413 / 415 / 422),
a non-WebSocket request on the WS route and 429 answer in plain text. Clients map by
status first and read `error` only when the body parses.

## Limits
Per client IP (IPv6: per /64): 10 room creates / min, 30 joins / min, 20 WebSocket connects / min,
60 status polls + starts / min. Rooms accept joins for 2 h; a room nobody connected to
goes after 5 min, an emptied room 60 s after the last peer left (a running session keeps
its room past 2 h until everyone left). Per socket: 50 signaling messages / s (burst 200,
then closed with 1008) and 64 queued outbound messages (a peer that stops reading is
dropped). The container is capped at 256 MB / 256 pids. Nothing is written to disk.

## Privacy
The server logs an 8-hex hash of the room id per event, never codes, tokens, IPs, ICE
candidates or TURN credentials. nginx `access_log off` for this vhost (URIs carry codes
and the WebSocket token). coturn logs peer addresses to docker logs (bounded to 3 × 10 MB).

## Local run (no Docker)
```sh
NETPLAY_SERVER_SECRET=$(openssl rand -base64 48) TURN_SECRET=$(openssl rand -base64 24) \
TURN_HOST=127.0.0.1 cargo run -p netplay-server
scripts/netplay-server-smoke.sh            # http://127.0.0.1:3536
```
The launcher / runner accept plain `ws://` only for loopback hosts.

## Backlog
TURNS on 5349 (certbot cert mounted into coturn) for networks that block UDP and plain
3478/tcp; spectators introduced to the host only (custom topology) so their IPs reach the
host alone.
