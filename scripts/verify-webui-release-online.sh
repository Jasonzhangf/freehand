#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

FREEHAND_WEBUI_BASE_URL="${FREEHAND_WEBUI_BASE_URL:-http://127.0.0.1:4041/}" \
FREEHAND_WEBUI_HEALTH_URL="${FREEHAND_WEBUI_HEALTH_URL:-http://127.0.0.1:4041/health}" \
FREEHAND_WEBUI_ADP_URL="${FREEHAND_WEBUI_ADP_URL:-ws://127.0.0.1:4041/adp}" \
FREEHAND_WEBUI_CLI="${FREEHAND_WEBUI_CLI:-$HOME/.local/bin/freehand-cli}" \
FREEHAND_WEBUI_PROFILE="${FREEHAND_WEBUI_PROFILE:-4041}" \
  scripts/verify-webui-online.sh
