#!/usr/bin/env bash
set -euo pipefail
PREVIEW_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$PREVIEW_ROOT"
if [[ ! -x node_modules/.bin/vite ]]; then
    npm ci --ignore-scripts --no-audit --no-fund
fi
exec npm run dev -- --port "${U60_DESIGN_PORT:-5176}" --strictPort
