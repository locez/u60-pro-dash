#!/bin/sh
# Compatibility with deployed agents that still call this filename.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ -f "$script_dir/tailscale-gateway.sh" ]; then
    exec sh "$script_dir/tailscale-gateway.sh" "$@"
fi
exec sh "$script_dir/../tailscale-gateway/tailscale-gateway.sh" "$@"
