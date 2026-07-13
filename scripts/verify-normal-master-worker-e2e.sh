#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
adp_url="${FREEHAND_NORMAL_MASTER_WORKER_ADP_URL:-ws://127.0.0.1:4042/adp}"
health_url="${FREEHAND_NORMAL_MASTER_WORKER_HEALTH_URL:-http://127.0.0.1:4042/health}"
cli_path="${FREEHAND_NORMAL_MASTER_WORKER_CLI:-$HOME/.local/bin/freehand-cliS}"
master_plist="$HOME/Library/LaunchAgents/com.freehand.daemonS.plist"
worker_plist="$HOME/Library/LaunchAgents/com.freehand.workerS.plist"
runtime_home="${FREEHAND_RUNTIME_HOME:-$HOME/.freehand}"
daemon_env="$runtime_home/daemonS.env"

cd "$repo_root"

require_executable() {
  local path="$1"
  if [[ ! -x "$path" ]]; then
    echo "missing executable: $path" >&2
    exit 2
  fi
}

shared_pair_token() {
  if [[ ! -f "$daemon_env" ]]; then
    echo "missing daemon env: $daemon_env" >&2
    exit 2
  fi
  awk -F= '$1 == "FREEHAND_PAIR_TOKEN_SHARED" { gsub(/^"/, "", $2); gsub(/"$/, "", $2); print $2; exit }' "$daemon_env"
}

task_history() {
  "$cli_path" adp-task-query --url "$adp_url" --history "$1"
}

observe_agent_board() {
  python3 - "$adp_url" <<'PY' 2>/dev/null || true
import asyncio, json, sys, websockets

async def main():
    async with websockets.connect(sys.argv[1]) as ws:
        await ws.send(json.dumps({
            "kind": "query",
            "request_id": "agent",
            "query": {"QueryAgentBoard": {}},
        }))
        while True:
            msg = json.loads(await ws.recv())
            if msg.get("request_id") == "agent":
                agents = msg.get("result", {}).get("AgentBoard", {}).get("agents", [])
                rows = []
                for agent in agents:
                    if agent.get("agent_id") in ("master", "worker") or agent.get("state") == "running":
                        rows.append(
                            f"{agent.get('agent_id')}:{agent.get('state')}:"
                            f"{agent.get('current_task_id')}:{agent.get('current_execution_id')}"
                        )
                print("agent_board_observed " + ",".join(rows))
                return

asyncio.run(main())
PY
}

wait_history_contains() {
  local task_id="$1"
  local timeout_seconds="$2"
  shift 2
  local deadline=$((SECONDS + timeout_seconds))
  local output=""
  local last_observed=0
  while [[ $SECONDS -le $deadline ]]; do
    output="$(task_history "$task_id" 2>/dev/null || true)"
    local ok=1
    for expected in "$@"; do
      if ! grep -q "$expected" <<<"$output"; then
        ok=0
        break
      fi
    done
    if [[ "$ok" == "1" ]]; then
      echo "$output"
      return 0
    fi
    if (( SECONDS - last_observed >= 15 )); then
      echo "normal_master_worker_observe task=$task_id waiting_for=$*"
      echo "$output"
      observe_agent_board
      last_observed=$SECONDS
    fi
    sleep 1
  done
  echo "normal_master_worker_e2e_timeout task=$task_id expected=$*" >&2
  task_history "$task_id" >&2 || true
  return 1
}

stop_service_if_loaded() {
  local plist="$1"
  launchctl bootout "gui/$(id -u)" "$plist" >/dev/null 2>&1 || true
}

restart_master() {
  scripts/install-launchd.sh restartS >/dev/null
  wait_for_health "restart_master"
}

restart_worker() {
  scripts/install-launchd.sh restartWorkerS >/dev/null
}

wait_for_health() {
  local label="$1"
  local deadline=$((SECONDS + 90))
  until curl -4fsS "$health_url" >/dev/null 2>&1; do
    if [[ $SECONDS -ge $deadline ]]; then
      echo "daemon did not become healthy after $label at $health_url" >&2
      launchctl print "gui/$(id -u)/com.freehand.daemonS" 2>&1 | sed -n '1,160p' >&2 || true
      tail -n 120 "$runtime_home/logs/daemonS.stderr.log" >&2 || true
      return 1
    fi
    sleep 1
  done
}

seed_with_token() {
  local token="$1"
  shift
  (
    set -a
    . "$daemon_env"
    set +a
    env FREEHAND_PAIR_TOKEN_SHARED="$token" PATH="$PATH" HOME="$HOME" "$cli_path" "$@"
  )
}

run_rejected_retry_branch() {
  local stamp task_id execution_id target token history
  stamp="178$(date +%s)"
  task_id="task-normal-rejected-$stamp"
  execution_id="exec-normal-rejected-$stamp"
  target="/tmp/normal-rejected-$stamp"
  token="$(shared_pair_token)"
  mkdir -p "$target"
  cat >"$target/instructions.txt" <<EOF
task_id=$task_id
required_result=normal_rejected_retry_ok=$stamp
required_file=result.md
instructions=After the seeded rejection, create result.md containing exactly the required_result line, verify it by reading it back, then submit review_ready.
EOF

  stop_service_if_loaded "$master_plist"
  stop_service_if_loaded "$worker_plist"
  if curl -4fsS --max-time 2 "$health_url" >/dev/null 2>&1; then
    echo "daemon still online after bootout" >&2
    exit 1
  fi
  seed_with_token "$token" task-restart-seed-rejected \
    --agent master \
    --task "$task_id" \
    --worker worker \
    --execution "$execution_id" \
    --target-cwd "$target" \
    --summary "Read instructions.txt, create result.md containing exactly normal_rejected_retry_ok=$stamp, verify by reading it back, then submit review_ready." >/dev/null
  restart_master
  restart_worker
  history="$(wait_history_contains "$task_id" 180 TaskReviewRejected TaskClosed)"
  echo "normal_master_worker_branch_ok branch=rejected_retry task=$task_id execution=$execution_id"
  echo "$history"
}

run_blocked_branch() {
  local stamp task_id execution_id target token history
  stamp="178$(date +%s)"
  task_id="task-normal-blocked-$stamp"
  execution_id="exec-normal-blocked-$stamp"
  target="/tmp/normal-blocked-$stamp"
  token="$(shared_pair_token)"
  mkdir -p "$target"

  stop_service_if_loaded "$master_plist"
  if curl -4fsS --max-time 2 "$health_url" >/dev/null 2>&1; then
    echo "daemon still online after bootout" >&2
    exit 1
  fi
  seed_with_token "$token" task-restart-seed-blocked \
    --agent master \
    --task "$task_id" \
    --worker worker \
    --execution "$execution_id" \
    --target-cwd "$target" \
    --summary "normal blocked decision proof $stamp" >/dev/null
  restart_master
  history="$(wait_history_contains "$task_id" 120 TaskBlocked TaskProgressed)"
  echo "normal_master_worker_branch_ok branch=blocked_decision task=$task_id execution=$execution_id"
  echo "$history"
}

run_crash_recovery_branch() {
  local stamp task_id execution_id target token history
  stamp="178$(date +%s)"
  task_id="task-normal-crash-$stamp"
  execution_id="exec-normal-crash-$stamp"
  target="/tmp/normal-crash-$stamp"
  token="$(shared_pair_token)"
  mkdir -p "$target"
  cat >"$target/instructions.txt" <<EOF
task_id=$task_id
required_result=normal_crash_recovered=$stamp
required_file=result.md
instructions=After restart recovery, create result.md containing exactly the required_result line, verify it by reading it back, then stop and submit review_ready.
EOF

  stop_service_if_loaded "$master_plist"
  stop_service_if_loaded "$worker_plist"
  if curl -4fsS --max-time 2 "$health_url" >/dev/null 2>&1; then
    echo "daemon still online after bootout" >&2
    exit 1
  fi
  seed_with_token "$token" task-restart-seed-running \
    --agent master \
    --task "$task_id" \
    --worker worker \
    --execution "$execution_id" \
    --target-cwd "$target" \
    --summary "Read instructions.txt after restart recovery, create result.md containing exactly normal_crash_recovered=$stamp, verify by reading it back, then stop and submit review_ready." \
    --ttl-seconds 1 >/dev/null
  sleep 2
  restart_master
  restart_worker
  history="$(wait_history_contains "$task_id" 180 TaskInterrupted TaskClosed)"
  echo "normal_master_worker_branch_ok branch=worker_crash_recovery task=$task_id execution=$execution_id"
  echo "$history"
}

run_normal_master_worker_e2e() {
  require_executable "$cli_path"
  restart_master
  restart_worker
  "$cli_path" adp-smoke --url "$adp_url" >/dev/null

  autonomy_output="$(FREEHAND_MASTER_AUTONOMY_LEAVE_SERVICES_STOPPED=1 scripts/verify-master-worker-autonomy-online.sh)"
  if ! grep -q "master_worker_autonomy_online_ok" <<<"$autonomy_output"; then
    echo "$autonomy_output" >&2
    echo "normal master-worker SubmitUserInput autonomy gate failed" >&2
    exit 1
  fi
  echo "$autonomy_output"

  run_rejected_retry_branch
  run_blocked_branch
  run_crash_recovery_branch

  restart_master
  restart_worker
  wait_for_health "final restart"
  echo "normal_master_worker_e2e_ok url=$adp_url"
}

run_normal_master_worker_e2e "$@"
