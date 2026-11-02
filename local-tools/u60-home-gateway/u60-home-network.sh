#!/bin/sh
# Installed on the U60; called by the opt-in agent after Tailscale starts
# and after a cellular reconnect. Only the specified home subnet is forwarded.
set -eu

LAN_IF=br-lan
TS_IF=tailscale0
LAN_NET=192.168.0.0/24
REMOTE_NET=192.168.1.0/24
TAG=u60-home-gateway

rule() {
    table=$1
    chain=$2
    shift 2
    case "$action" in
        apply)
            iptables -w 5 -t "$table" -C "$chain" "$@" 2>/dev/null ||
                iptables -w 5 -t "$table" -I "$chain" 1 "$@"
            ;;
        remove)
            if iptables -w 5 -t "$table" -C "$chain" "$@" 2>/dev/null; then
                iptables -w 5 -t "$table" -D "$chain" "$@"
            fi
            ;;
    esac
}

action=${1:-apply}
case "$action" in
    apply|remove) ;;
    *) echo 'usage: u60-home-network.sh [apply|remove]' >&2; exit 2 ;;
esac

if [ "$action" = apply ]; then
    [ "$(cat /proc/sys/net/ipv4/ip_forward)" = 1 ] ||
        { echo 'IPv4 forwarding is disabled; leaving the firewall unchanged.' >&2; exit 1; }
    ip -4 addr show dev "$LAN_IF" | grep -q 'inet 192\.168\.0\.1/24' ||
        { echo 'LAN addressing changed; refusing to use stale subnet rules.' >&2; exit 1; }
fi

# Reply packets are de-NATed by conntrack back to the hotspot client.
rule nat POSTROUTING -s "$LAN_NET" -d "$REMOTE_NET" -o "$TS_IF" \
    -m comment --comment "$TAG" -j MASQUERADE
rule mangle FORWARD -s "$LAN_NET" -d "$REMOTE_NET" -o "$TS_IF" \
    -p tcp --tcp-flags SYN,RST SYN -m comment --comment "$TAG" \
    -j TCPMSS --clamp-mss-to-pmtu
rule mangle FORWARD -s "$REMOTE_NET" -d "$LAN_NET" -i "$TS_IF" \
    -p tcp --tcp-flags SYN,RST SYN -m comment --comment "$TAG" \
    -j TCPMSS --clamp-mss-to-pmtu
rule filter FORWARD -i "$TS_IF" -o "$LAN_IF" -s "$REMOTE_NET" -d "$LAN_NET" \
    -m conntrack --ctstate ESTABLISHED,RELATED -m comment --comment "$TAG" -j ACCEPT
rule filter FORWARD -i "$LAN_IF" -o "$TS_IF" -s "$LAN_NET" -d "$REMOTE_NET" \
    -m comment --comment "$TAG" -j ACCEPT
# If the tunnel loses its route, do not send the home subnet through cellular WAN.
rule filter FORWARD -i "$LAN_IF" ! -o "$TS_IF" -s "$LAN_NET" -d "$REMOTE_NET" \
    -m comment --comment "$TAG" -j REJECT

