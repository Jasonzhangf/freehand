#!/usr/bin/env bash
set -euo pipefail

adp_url="${FREEHAND_REAL_PROVIDER_MASTER_WORKER_ADP_URL:-ws://127.0.0.1:4042/adp}"
cli_path="${FREEHAND_REAL_PROVIDER_MASTER_WORKER_CLI:-$HOME/.local/bin/freehand-cliS}"
tasks=()

usage() {
  cat >&2 <<'EOF'
usage: scripts/verify-real-provider-master-worker-history.sh [--url ws://127.0.0.1:4042/adp] --task <task_id> [--task <task_id> ...]
EOF
}

has_post_assignment_event() {
  local event_types="$1"
  case ",$event_types," in
    *",TaskResumed,"*|*",TaskExecutionRecorded,"*|*",TaskBlocked,"*|*",TaskReviewSubmitted,"*|*",TaskReviewRejected,"*|*",TaskReviewApproved,"*|*",TaskClosed,"*|*",TaskPaused,"*|*",TaskCancelled,"*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

run_verify_real_provider_master_worker_history() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --url)
        [[ $# -ge 2 ]] || { usage; exit 2; }
        adp_url="$2"
        shift 2
        ;;
      --task)
        [[ $# -ge 2 ]] || { usage; exit 2; }
        tasks+=("$2")
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        echo "unknown argument: $1" >&2
        usage
        exit 2
        ;;
    esac
  done

  if [[ ${#tasks[@]} -eq 0 ]]; then
    usage
    exit 2
  fi

  if [[ ! -x "$cli_path" ]]; then
    echo "missing executable CLI: $cli_path" >&2
    exit 2
  fi

  failed=0
  for task_id in "${tasks[@]}"; do
    output="$("$cli_path" adp-task-query --url "$adp_url" --history "$task_id")" || {
      status=$?
      echo "real_provider_master_worker_history_failed task=$task_id reason=query_failed status=$status" >&2
      failed=1
      continue
    }

    events="$(sed -n 's/.* events=\([0-9][0-9]*\) .*/\1/p' <<<"$output")"
    event_types="$(sed -n 's/.* event_types=\(.*\)$/\1/p' <<<"$output")"

    if [[ -z "$events" || -z "$event_types" ]]; then
      echo "$output" >&2
      echo "real_provider_master_worker_history_failed task=$task_id reason=unparseable_history" >&2
      failed=1
      continue
    fi

    if [[ "$events" == "0" ]]; then
      echo "real_provider_master_worker_history_failed task=$task_id reason=empty_history event_types=$event_types" >&2
      failed=1
      continue
    fi

    if [[ "$event_types" == "TaskCreated,TaskAssigned" || "$event_types" == "TaskCreated" || "$event_types" == "TaskAssigned" ]]; then
      echo "real_provider_master_worker_history_failed task=$task_id reason=assigned_only event_types=$event_types" >&2
      failed=1
      continue
    fi

    if ! has_post_assignment_event "$event_types"; then
      echo "real_provider_master_worker_history_failed task=$task_id reason=no_worker_lifecycle_event event_types=$event_types" >&2
      failed=1
      continue
    fi

    echo "real_provider_master_worker_history_ok task=$task_id events=$events event_types=$event_types"
  done

  if [[ "$failed" != "0" ]]; then
    exit 1
  fi
}

run_verify_real_provider_master_worker_history "$@"
