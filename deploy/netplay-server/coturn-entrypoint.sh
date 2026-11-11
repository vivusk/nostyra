#!/bin/sh
# Fills turnserver.conf.template from the environment (.env via compose) and starts
# coturn. Keeps TURN_SECRET out of the image and out of the process command line.
set -eu
: "${TURN_SECRET:?TURN_SECRET must be set in .env}"
: "${PUBLIC_HOST:?PUBLIC_HOST must be set in .env}"
: "${TURN_RELAY_IP:?TURN_RELAY_IP must be set in .env (public IPv4 on the VPS interface, optional IPv6, comma-separated)}"
case "$TURN_SECRET$PUBLIC_HOST${TURN_EXTERNAL_IP:-}$TURN_RELAY_IP" in
    *'|'* | *'\'* | *'&'*) echo "TURN_SECRET / PUBLIC_HOST / TURN_EXTERNAL_IP / TURN_RELAY_IP: no | \\ & characters" >&2; exit 1 ;;
esac
if [ -n "${TURN_EXTERNAL_IP:-}" ]; then
    external="external-ip=$TURN_EXTERNAL_IP"
else
    external="# external-ip not set"
fi
conf=/tmp/turnserver.conf
umask 077
sed -e "s|@TURN_SECRET@|$TURN_SECRET|" \
    -e "s|@PUBLIC_HOST@|$PUBLIC_HOST|" \
    -e "s|@EXTERNAL_IP_LINE@|$external|" \
    /etc/coturn/turnserver.conf.template > "$conf"
# Pin listen + relay addresses to the public interface IP(s), never the docker bridges.
for ip in $(echo "$TURN_RELAY_IP" | tr ',' ' '); do
    case "$ip" in
        *[!0-9a-fA-F.:]*) echo "TURN_RELAY_IP: invalid address" >&2; exit 1 ;;
    esac
    printf 'listening-ip=%s\nrelay-ip=%s\n' "$ip" "$ip" >> "$conf"
done
exec turnserver -c "$conf"
