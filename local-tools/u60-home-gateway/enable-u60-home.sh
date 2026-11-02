#!/usr/bin/env bash
# Compatibility entry point for existing local commands.
set -euo pipefail
script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export U60_GATEWAY_ARTIFACT_DIR=${U60_GATEWAY_ARTIFACT_DIR:-$script_dir}
exec bash "$script_dir/../tailscale-gateway/manage-gateway.sh" "$@"
