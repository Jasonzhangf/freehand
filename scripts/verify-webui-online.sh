#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
base_url="${FREEHAND_WEBUI_BASE_URL:-http://127.0.0.1:4042/}"
health_url="${FREEHAND_WEBUI_HEALTH_URL:-http://127.0.0.1:4042/health}"
adp_url="${FREEHAND_WEBUI_ADP_URL:-ws://127.0.0.1:4042/adp}"
cli_path="${FREEHAND_WEBUI_CLI:-$HOME/.local/bin/freehand-cliS}"
profile="${FREEHAND_WEBUI_PROFILE:-4042}"

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
  if [[ ! -x "$cli_path" ]]; then
    echo "missing executable CLI for WebUI online verification: $cli_path" >&2
    exit 2
  fi

  echo "[freehand-webui-online] health: $health_url"
  curl -4fsS "$health_url" >/dev/null

  echo "[freehand-webui-online] running real browser WebUI + ADP verification: $base_url"
  FREEHAND_WEBUI_BASE_URL="$base_url" \
    FREEHAND_WEBUI_ADP_URL="$adp_url" \
    FREEHAND_WEBUI_CLI="$cli_path" \
    FREEHAND_WEBUI_PROFILE="$profile" \
    node scripts/webui_verify_online.mjs
}

run_verify_webui_online "$@"
