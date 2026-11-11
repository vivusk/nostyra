#!/usr/bin/env sh
# Smoke test for a running netplay server (M20): health, create a room, join it, poll it,
# start it, check a late join is refused, and (optional) check coturn accepts the TURN
# credentials with `turnutils_uclient` (from the coturn package / container).
#
#   scripts/netplay-server-smoke.sh [base-url]        default http://127.0.0.1:3536
#   NETPLAY_SMOKE_TURN=1 scripts/netplay-server-smoke.sh https://netplay.nostyra.com
#
# Needs curl only (fields are read with sed: every value used is plain ASCII).
# Prints statuses and error codes only: never the room code, tokens or TURN credentials.
set -eu
base="${1:-http://127.0.0.1:3536}"
for tool in curl; do
    command -v "$tool" >/dev/null 2>&1 || { echo "missing tool: $tool" >&2; exit 2; }
done
out=$(mktemp)
trap 'rm -f "$out"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
body() { cat "$out"; }
# field NAME -> first string value of "NAME" in the last body
field() { sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p" "$out"; }

# call METHOD PATH [BODY] [BEARER] -> prints the HTTP status, body lands in $out
call() {
    method=$1 path=$2 data=${3:-} bearer=${4:-}
    set -- -sS -o "$out" -w '%{http_code}' -X "$method"
    [ -n "$bearer" ] && set -- "$@" -H "Authorization: Bearer $bearer"
    [ -n "$data" ] && set -- "$@" -H 'content-type: application/json' -d "$data"
    curl "$@" "$base$path"
}

[ "$(call GET /healthz)" = 200 ] || fail "healthz"
echo "healthz ok"

sha1=$(printf '%040d' 0)
game="{\"content_sha1\":\"$sha1\",\"title\":\"Smoke\",\"system\":\"snes\",\"core_id\":\"snes9x\",\"core_version\":\"0\"}"
create="{\"nickname\":\"smoke-host\",\"game\":$game,\"max_players\":2,\"allow_spectators\":true}"
[ "$(call POST /v1/rooms "$create")" = 200 ] || fail "create room: $(field error)"
code=$(field code)
host_token=$(field token)
turn_user=$(field username)
turn_pass=$(field credential)
turn_host=$(sed -n 's/.*"turn:\([^:"]*\):.*/\1/p' "$out")
[ -n "$code" ] && [ -n "$turn_pass" ] || fail "create response lacks code / TURN credential"
echo "create ok"

join='{"nickname":"smoke-guest","role":"player"}'
[ "$(call POST "/v1/rooms/$code/join" "$join")" = 200 ] || fail "join: $(field error)"
grep -q '"max_players":2' "$out" || fail "join response lacks max_players"
echo "join ok"
[ "$(call POST "/v1/rooms/$code/join" "$join")" = 409 ] || fail "3rd player should be 409"
echo "room full ok (409)"

[ "$(call GET "/v1/rooms/$code")" = 200 ] || fail "status: $(field error)"
echo "status ok: $(body)"
[ "$(call GET /v1/ws/0)" = 401 ] || fail "ws without token should be 401"
echo "ws without token ok (401)"
[ "$(call POST "/v1/rooms/$code/start" "" "$host_token")" = 204 ] || fail "start: $(field error)"
echo "start ok (204)"
spect='{"nickname":"late","role":"spectator"}'
[ "$(call POST "/v1/rooms/$code/join" "$spect")" = 409 ] || fail "late join should be 409"
echo "late join ok (409)"

if [ "${NETPLAY_SMOKE_TURN:-0}" = 1 ]; then
    command -v turnutils_uclient >/dev/null 2>&1 || fail "turnutils_uclient not installed"
    # -y: client-to-client through the relay; an auth error means TURN_SECRET differs
    # between the server's .env and coturn.
    turnutils_uclient -y -n 5 -u "$turn_user" -w "$turn_pass" "$turn_host" >/dev/null \
        || fail "coturn refused the credentials"
    echo "turn ok (coturn accepted the REST credentials)"
fi
echo "netplay server smoke OK"
