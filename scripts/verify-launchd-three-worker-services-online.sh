#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
session_suffix="$(date +%s)-$$"

cd "$repo_root"

FREEHAND_THREE_WORKER_WORKER_START_MODE=launchd \
FREEHAND_THREE_WORKER_SESSION="${FREEHAND_THREE_WORKER_SESSION:-online-launchd-three-worker-evaluation-$session_suffix}" \
FREEHAND_THREE_WORKER_FIXTURE_PORT="${FREEHAND_THREE_WORKER_FIXTURE_PORT:-18184}" \
FREEHAND_THREE_WORKER_ADP_URL="${FREEHAND_THREE_WORKER_ADP_URL:-ws://127.0.0.1:4143/adp}" \
FREEHAND_THREE_WORKER_HEALTH_URL="${FREEHAND_THREE_WORKER_HEALTH_URL:-http://127.0.0.1:4143/health}" \
FREEHAND_THREE_WORKER_LAUNCHD_LABEL_PREFIX="${FREEHAND_THREE_WORKER_LAUNCHD_LABEL_PREFIX:-com.freehand.verify.three-worker.$session_suffix}" \
FREEHAND_THREE_WORKER_MASTER_BIND="${FREEHAND_THREE_WORKER_MASTER_BIND:-127.0.0.1:4143}" \
bash scripts/verify-master-three-worker-e2e-online.sh
