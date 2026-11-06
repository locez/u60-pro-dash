#!/bin/sh
# Owned, opt-in IPv4 forwarding rules for explicitly configured subnets.
set -eu
GW_CONFIG=${U60_TS_GATEWAY_CONFIG:-/data/local/tmp/tailscale-gateway.conf}
GW_ACTION=${1:-apply}
case "$GW_ACTION" in apply|remove) ;; *) echo 'usage: tailscale-gateway.sh [apply|remove]' >&2; exit 2 ;; esac

LAN_IF=br-lan
LAN_NET=
REMOTE_NETS=
while IFS='=' read -r gw_key gw_value || [ -n "$gw_key" ]; do
    case "$gw_key" in
        ''|'#'*) ;;
        LAN_IF) LAN_IF=$gw_value ;;
        LAN_NET) LAN_NET=$gw_value ;;
        REMOTE_NETS) REMOTE_NETS=$gw_value ;;
        *) echo 'Unknown gateway configuration field.' >&2; exit 1 ;;
    esac
done < "$GW_CONFIG"
case "$LAN_IF" in ''|*[!a-zA-Z0-9_.:-]*) echo 'Invalid LAN interface.' >&2; exit 1 ;; esac
[ "${#LAN_IF}" -le 15 ] || exit 1
[ -n "$LAN_NET" ] && [ -n "$REMOTE_NETS" ] || { echo 'Explicit LAN and remote subnets are required.' >&2; exit 1; }

# Parse data, never source shell code from the configuration. Require canonical
# IPv4 CIDRs and reject overlap with the hotspot LAN before touching rules.
for gw_remote in $REMOTE_NETS; do
    awk -v lan="$LAN_NET" -v remote="$gw_remote" '
    function cidr(s, a, o, n, p, count, i) {
        if (split(s,a,"/") != 2 || a[2] !~ /^[0-9]+$/ || a[2] > 32) return -1;
        p=2^(32-a[2]); count=split(a[1],o,"."); if(count != 4) return -1;
        n=0; for(i=1;i<=4;i++){if(o[i] !~ /^[0-9]+$/ || o[i]>255) return -1; n=n*256+o[i];}
        if(int(n/p)*p != n) return -1; span=p; return n;
    }
    BEGIN { l=cidr(lan); ls=span; r=cidr(remote); rs=span;
        if(l<0 || r<0 || (l<r+rs && r<l+ls)) exit 1;
    }' || { echo 'Invalid or overlapping gateway subnets.' >&2; exit 1; }
done

if [ "$GW_ACTION" = apply ]; then
    [ "$(cat /proc/sys/net/ipv4/ip_forward)" = 1 ] || { echo 'IPv4 forwarding is disabled.' >&2; exit 1; }
    # Derive the actual LAN network, including prefixes other than /24.
    ip -o -4 addr show dev "$LAN_IF" | awk '
    $3=="inet" {split($4,a,"/");split(a[1],o,"."); n=((o[1]*256+o[2])*256+o[3])*256+o[4];
        p=2^(32-a[2]); n=int(n/p)*p; s="";
        for(i=0;i<4;i++){b=n%256;s=b (i ? "." : "") s;n=int(n/256);}
        print s "/" a[2];
    }' | grep -Fx "$LAN_NET" >/dev/null || { echo 'LAN addressing differs from gateway configuration.' >&2; exit 1; }
fi

rule() {
    gw_table=$1; gw_chain=$2; shift 2
    case "$gw_operation" in
        apply)
            iptables -w 5 -t "$gw_table" -C "$gw_chain" "$@" 2>/dev/null ||
                iptables -w 5 -t "$gw_table" -I "$gw_chain" 1 "$@"
            ;;
        remove)
            while iptables -w 5 -t "$gw_table" -C "$gw_chain" "$@" 2>/dev/null; do
                iptables -w 5 -t "$gw_table" -D "$gw_chain" "$@"
            done
            ;;
    esac
}
rules() {
    for gw_remote in $REMOTE_NETS; do
        rule nat POSTROUTING -s "$LAN_NET" -d "$gw_remote" -o tailscale0 -m comment --comment "$gw_tag" -j MASQUERADE
        rule mangle FORWARD -s "$LAN_NET" -d "$gw_remote" -o tailscale0 -p tcp --tcp-flags SYN,RST SYN -m comment --comment "$gw_tag" -j TCPMSS --clamp-mss-to-pmtu
        rule mangle FORWARD -s "$gw_remote" -d "$LAN_NET" -i tailscale0 -p tcp --tcp-flags SYN,RST SYN -m comment --comment "$gw_tag" -j TCPMSS --clamp-mss-to-pmtu
        rule filter FORWARD -i tailscale0 -o "$LAN_IF" -s "$gw_remote" -d "$LAN_NET" -m conntrack --ctstate ESTABLISHED,RELATED -m comment --comment "$gw_tag" -j ACCEPT
        rule filter FORWARD -i "$LAN_IF" -o tailscale0 -s "$LAN_NET" -d "$gw_remote" -m comment --comment "$gw_tag" -j ACCEPT
        rule filter FORWARD -i "$LAN_IF" ! -o tailscale0 -s "$LAN_NET" -d "$gw_remote" -m comment --comment "$gw_tag" -j REJECT
    done
}
gw_tag=u60-tailscale-gateway
gw_operation=$GW_ACTION
rules
# Install new rules before removing legacy rules, matching only owned tuples.
gw_tag=u60-home-gateway
gw_operation=remove
rules
