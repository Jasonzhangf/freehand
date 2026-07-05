#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
health_url="${FREEHAND_WEBUI_HEALTH_URL:-http://127.0.0.1:4041/health}"

cd "$repo_root"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}

run_verify_webui_online() {
  require_cmd node
  require_cmd curl

  echo "[freehand-webui-online] health: $health_url"
  curl -4fsS "$health_url" >/dev/null

  echo "[freehand-webui-online] running real browser WebUI + ADP verification"
  node scripts/webui_verify_4041.mjs
}

run_verify_webui_online "$@"
