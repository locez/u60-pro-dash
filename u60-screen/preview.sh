#!/usr/bin/env bash
set -euo pipefail
U60_PREVIEW_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
U60_PREVIEW_PORT="${U60_PREVIEW_PORT:-8765}"
printf '打开浏览器访问：http://127.0.0.1:%s\n按 Ctrl+C 停止预览。\n' "$U60_PREVIEW_PORT"
exec python3 -m http.server "$U60_PREVIEW_PORT" --bind 127.0.0.1 --directory "$U60_PREVIEW_DIR/preview"
