#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
host_home="${FREEHAND_THREE_WORKER_HOST_HOME:-$HOME}"
isolated_home="${FREEHAND_THREE_WORKER_HOME:-"$(mktemp -d /tmp/freehand-three-worker-home.XXXXXX)"}"
runtime_home="$isolated_home/.freehand"
master_bind="${FREEHAND_THREE_WORKER_MASTER_BIND:-127.0.0.1:4142}"
adp_url="${FREEHAND_THREE_WORKER_ADP_URL:-ws://$master_bind/adp}"
health_url="${FREEHAND_THREE_WORKER_HEALTH_URL:-http://$master_bind/health}"
cli_path="${FREEHAND_THREE_WORKER_CLI:-$HOME/.local/bin/freehand-cliS}"
daemon_path="${FREEHAND_THREE_WORKER_DAEMON:-$repo_root/target/debug/freehand-daemon}"
port="${FREEHAND_THREE_WORKER_FIXTURE_PORT:-18084}"
session_id="${FREEHAND_THREE_WORKER_SESSION:-online-master-three-worker-evaluation-$(date +%s)}"
adp_auth_token="${FREEHAND_THREE_WORKER_ADP_AUTH_TOKEN:-three-worker-adp-auth-$$}"
export FREEHAND_THREE_WORKER_ADP_AUTH_TOKEN="$adp_auth_token"
export FREEHAND_ADP_AUTH_TOKEN="$adp_auth_token"
fixture_key_name="FREEHAND_THREE_WORKER_FIXTURE_KEY"
fixture_key_value="test-three-worker-key"
config_path="$runtime_home/config.toml"
backup_dir="$runtime_home/tmp/three-worker-e2e-$(date +%Y%m%dT%H%M%S)-$$"
mock_log="$backup_dir/mock-provider.log"
target_cwd="$backup_dir/worker-target"
worker_start_mode="${FREEHAND_THREE_WORKER_WORKER_START_MODE:-process}"
launchd_label_prefix="${FREEHAND_THREE_WORKER_LAUNCHD_LABEL_PREFIX:-com.freehand.three-worker.$$}"
mock_pid=""
master_pid=""
worker_alpha_pid=""
worker_beta_pid=""
worker_gamma_pid=""

cd "$repo_root"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}

wait_for_health() {
  local label="$1"
  local deadline=$((SECONDS + 90))
  until curl -4fsS "$health_url" >/dev/null 2>&1; do
    if [[ $SECONDS -ge $deadline ]]; then
      echo "three-worker daemon did not become healthy after $label at $health_url" >&2
      launchctl print "gui/$(id -u)/com.freehand.daemonS" 2>&1 | sed -n '1,160p' >&2 || true
      tail -n 120 "$runtime_home/logs/daemonS.stderr.log" >&2 || true
      return 1
    fi
    sleep 1
  done
}

restore_runtime_config() {
  local restore_status=0
  if [[ "$worker_start_mode" == "launchd" ]]; then
    uninstall_launchd_workers || restore_status=$?
  fi
  for service_pid in "$worker_alpha_pid" "$worker_beta_pid" "$worker_gamma_pid" "$master_pid"; do
    if [[ -n "$service_pid" ]] && kill -0 "$service_pid" >/dev/null 2>&1; then
      kill "$service_pid" >/dev/null 2>&1 || restore_status=$?
      wait "$service_pid" >/dev/null 2>&1 || true
    fi
  done
  if [[ -n "$mock_pid" ]]; then
    if kill -0 "$mock_pid" >/dev/null 2>&1; then
      kill "$mock_pid" >/dev/null 2>&1 || restore_status=$?
      wait "$mock_pid" >/dev/null 2>&1 || true
    fi
  fi
  return "$restore_status"
}

write_isolated_config() {
  cat >"$config_path" <<EOF
[providers.minimax]
id = "minimax"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "http://127.0.0.1:$port"
default_model = "MiniMax-M3"

[providers.minimax.auth]
type = "apikey"
api_key_env = "FREEHAND_PAIR_TOKEN_SHARED"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker-alpha", "worker-beta", "worker-gamma"]
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "minimax"
local_web_url = "http://127.0.0.1:4142"

[agents.worker-alpha]
name = "worker-alpha"
mode = "slave"
node_id = "worker-alpha-node"
paired_agents = ["master"]
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "minimax"
local_web_url = "http://127.0.0.1:4143"

[agents.worker-beta]
name = "worker-beta"
mode = "slave"
node_id = "worker-beta-node"
paired_agents = ["master"]
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "minimax"
local_web_url = "http://127.0.0.1:4144"

[agents.worker-gamma]
name = "worker-gamma"
mode = "slave"
node_id = "worker-gamma-node"
paired_agents = ["master"]
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "minimax"
local_web_url = "http://127.0.0.1:4145"
EOF
}

start_master() {
  env HOME="$isolated_home" \
    FREEHAND_PAIR_TOKEN_SHARED="$pair_token" \
    FREEHAND_ADP_AUTH_TOKEN="$adp_auth_token" \
    FREEHAND_CC_API_KEY="isolated-bootstrap-only" \
    FREEHAND_PROVIDER_RETRY_BACKOFF_MS=0 \
    "${fixture_key_name}=${fixture_key_value}" \
    "$daemon_path" serve --agent master --bind "$master_bind" \
    >"$runtime_home/logs/master.stdout.log" \
    2>"$runtime_home/logs/master.stderr.log" &
  master_pid="$!"
  wait_for_health "isolated master start"
}

stop_master() {
  if [[ -n "$master_pid" ]] && kill -0 "$master_pid" >/dev/null 2>&1; then
    kill "$master_pid"
    wait "$master_pid" >/dev/null 2>&1 || true
  fi
  master_pid=""
}

start_worker() {
  local agent_name="$1"
  if [[ "$worker_start_mode" == "launchd" ]]; then
    start_launchd_worker "$agent_name"
    return 0
  fi
  env HOME="$isolated_home" \
    FREEHAND_PAIR_TOKEN_SHARED="$pair_token" \
    FREEHAND_ADP_AUTH_TOKEN="$adp_auth_token" \
    FREEHAND_CC_API_KEY="isolated-bootstrap-only" \
    FREEHAND_PROVIDER_RETRY_BACKOFF_MS=0 \
    "${fixture_key_name}=${fixture_key_value}" \
    "$daemon_path" serve --agent "$agent_name" \
    >"$runtime_home/logs/${agent_name}.stdout.log" \
    2>"$runtime_home/logs/${agent_name}.stderr.log" &
  worker_start_pid="$!"
}

launchd_component_for_agent() {
  printf '%s\n' "$1" | sed 's/[^A-Za-z0-9_.-]/-/g'
}

launchd_label_for_agent() {
  local agent_name="$1"
  printf '%s.%s\n' "$launchd_label_prefix" "$(launchd_component_for_agent "$agent_name")"
}

launchd_plist_for_agent() {
  local agent_name="$1"
  printf '%s/Library/LaunchAgents/%s.plist\n' "$isolated_home" "$(launchd_label_for_agent "$agent_name")"
}

launchd_pid_for_agent() {
  local agent_name="$1"
  local label state_file
  label="$(launchd_label_for_agent "$agent_name")"
  state_file="$runtime_home/state/launchd/$label.json"
  /usr/bin/plutil -extract daemon_pid raw -o - "$state_file" 2>/dev/null
}

wait_for_launchd_worker_pid() {
  local agent_name="$1"
  local old_pid="${2:-}"
  local deadline=$((SECONDS + 90))
  local service_pid=""
  while [[ $SECONDS -lt $deadline ]]; do
    service_pid="$(launchd_pid_for_agent "$agent_name")"
    if [[ -n "$service_pid" && "$service_pid" != "$old_pid" ]] && kill -0 "$service_pid" >/dev/null 2>&1; then
      worker_start_pid="$service_pid"
      return 0
    fi
    sleep 1
  done
  echo "launchd Worker $agent_name did not expose a fresh running PID" >&2
  launchctl print "gui/$(id -u)/$(launchd_label_for_agent "$agent_name")" 2>&1 | sed -n '1,160p' >&2 || true
  tail -n 120 "$runtime_home/logs/workerS.$(launchd_component_for_agent "$agent_name").stderr.log" >&2 || true
  return 1
}

start_launchd_worker() {
  local agent_name="$1"
  local label
  label="$(launchd_label_for_agent "$agent_name")"
  HOME="$isolated_home" \
    CARGO_HOME="${CARGO_HOME:-$host_home/.cargo}" \
    RUSTUP_HOME="${RUSTUP_HOME:-$host_home/.rustup}" \
    FREEHAND_PREFIX="$isolated_home/.local" \
    FREEHAND_RUNTIME_HOME="$runtime_home" \
    FREEHAND_WORKER_AGENT="$agent_name" \
    FREEHAND_WORKER_WORKDIR="$runtime_home" \
    FREEHAND_LAUNCHD_LABEL="$label" \
    FREEHAND_PAIR_TOKEN_SHARED="$pair_token" \
    FREEHAND_PROVIDER_RETRY_BACKOFF_MS=0 \
    FREEHAND_LAUNCHD_SKIP_ENABLE=1 \
    FREEHAND_LAUNCHD_HEALTH_WAIT_SECONDS=90 \
    bash scripts/install-launchd.sh restartWorkerS >/dev/null
  wait_for_launchd_worker_pid "$agent_name"
}

uninstall_launchd_worker() {
  local agent_name="$1"
  local label
  label="$(launchd_label_for_agent "$agent_name")"
  HOME="$isolated_home" \
    FREEHAND_LAUNCHD_LABEL="$label" \
    bash scripts/uninstall-launchd.sh uninstallWorkerS >/dev/null 2>&1 || true
  rm -f "$(launchd_plist_for_agent "$agent_name")"
}

uninstall_launchd_workers() {
  uninstall_launchd_worker worker-alpha
  uninstall_launchd_worker worker-beta
  uninstall_launchd_worker worker-gamma
}

start_workers() {
  start_worker worker-alpha
  worker_alpha_pid="$worker_start_pid"
  start_worker worker-beta
  worker_beta_pid="$worker_start_pid"
  start_worker worker-gamma
  worker_gamma_pid="$worker_start_pid"
}

verify_worker_processes() {
  local duplicate=""
  if [[ "$worker_alpha_pid" == "$worker_beta_pid" || "$worker_alpha_pid" == "$worker_gamma_pid" || "$worker_beta_pid" == "$worker_gamma_pid" ]]; then
    duplicate="yes"
  fi
  if [[ -n "$duplicate" ]]; then
    echo "three-worker verifier expected distinct worker PIDs, got alpha=$worker_alpha_pid beta=$worker_beta_pid gamma=$worker_gamma_pid" >&2
    exit 2
  fi
  for service_pid in "$worker_alpha_pid" "$worker_beta_pid" "$worker_gamma_pid"; do
    if ! kill -0 "$service_pid" >/dev/null 2>&1; then
      echo "three-worker verifier worker PID is not alive: $service_pid" >&2
      exit 2
    fi
  done
}

query_worker_health() {
  local phase="$1"
  local alpha_pid="$2"
  local beta_pid="$3"
  local gamma_pid="$4"
  local previous_gamma_instance="${5:-}"
  local previous_gamma_task="${6:-}"
  local previous_gamma_execution="${7:-}"
  python3 - "$adp_url" "$phase" "$alpha_pid" "$beta_pid" "$gamma_pid" "$previous_gamma_instance" "$previous_gamma_task" "$previous_gamma_execution" <<'PY'
import asyncio
import json
import os
import sys
import websockets

(
    url,
    phase,
    alpha_pid,
    beta_pid,
    gamma_pid,
    previous_gamma_instance,
    previous_gamma_task,
    previous_gamma_execution,
) = sys.argv[1:9]
previous_gamma_task = previous_gamma_task or None
previous_gamma_execution = previous_gamma_execution or None
expected_pids = {
    "worker-alpha": int(alpha_pid),
    "worker-beta": int(beta_pid),
    "worker-gamma": int(gamma_pid),
}
headers = {
    "Authorization": f"Bearer {os.environ['FREEHAND_THREE_WORKER_ADP_AUTH_TOKEN']}"
}

async def query_board():
    async with websockets.connect(url, additional_headers=headers) as ws:
        await ws.send(json.dumps({
            "protocol_version": 4,
            "kind": "handshake",
            "request_id": f"worker-health-handshake-{phase}",
            "client_name": "three-worker-verifier",
            "capabilities": ["adp.v4.handshake"],
        }))
        handshake = json.loads(await asyncio.wait_for(ws.recv(), timeout=20))
        if handshake.get("request_id") != f"worker-health-handshake-{phase}":
            raise RuntimeError(f"ADP worker health handshake failed: {handshake}")
        await ws.send(json.dumps({
            "protocol_version": 4,
            "kind": "query",
            "request_id": f"worker-health-{phase}",
            "query": {"QueryAgentBoard": {}},
        }))
        while True:
            message = json.loads(await asyncio.wait_for(ws.recv(), timeout=20))
            if message.get("request_id") == f"worker-health-{phase}":
                return message

def validate(message):
    agents = (
        message.get("result", {})
        .get("AgentBoard", {})
        .get("agents", [])
    )
    workers = {
        agent.get("agent_id"): agent
        for agent in agents
        if agent.get("agent_id") in expected_pids
    }
    if set(workers) != set(expected_pids):
        return None, f"missing configured Workers: {workers}"
    processes = {}
    for worker_id, expected_pid in expected_pids.items():
        worker = workers[worker_id]
        process = worker.get("process") or {}
        processes[worker_id] = process
        if process.get("process_id") != expected_pid:
            return None, (
                f"{worker_id} process_id={process.get('process_id')} "
                f"expected={expected_pid}"
            )
        if not process.get("process_instance_id"):
            return None, f"{worker_id} missing process_instance_id"
        if not process.get("started_at") or not process.get("heartbeat_at"):
            return None, f"{worker_id} missing process timestamps"
    gamma = workers["worker-gamma"]
    gamma_process = processes["worker-gamma"]
    if phase == "fresh":
        if not all(worker.get("alive") for worker in workers.values()):
            return None, f"fresh Worker not alive: {workers}"
        if any(process.get("restart_count") != 0 for process in processes.values()):
            return None, f"fresh Worker restart_count mismatch: {workers}"
        if gamma.get("state") != "idle":
            return None, f"released gamma is not idle after takeover: {gamma}"
        if gamma.get("current_task_id") is not None:
            return None, f"released gamma retained current task after takeover: {gamma}"
        if gamma.get("current_execution_id") is not None:
            return None, f"released gamma retained current execution after takeover: {gamma}"
        if (gamma.get("last_activity") or {}).get("kind") != "interrupted":
            return None, f"released gamma lost interrupted last activity: {gamma}"
    elif phase == "offline":
        if not workers["worker-alpha"].get("alive") or not workers["worker-beta"].get("alive"):
            return None, f"unaffected Worker became offline: {workers}"
        if gamma.get("alive"):
            return None, f"stopped gamma still alive before TTL projection: {gamma}"
        if gamma_process.get("process_instance_id") != previous_gamma_instance:
            return None, f"offline gamma instance changed: {gamma}"
        if gamma_process.get("restart_count") != 0:
            return None, f"offline gamma restart_count changed: {gamma}"
        if gamma.get("current_task_id") != previous_gamma_task:
            return None, f"offline gamma task identity changed: {gamma}"
        if gamma.get("current_execution_id") != previous_gamma_execution:
            return None, f"offline gamma execution identity changed: {gamma}"
    elif phase == "restarted":
        if not all(worker.get("alive") for worker in workers.values()):
            return None, f"restarted Worker not alive: {workers}"
        if gamma_process.get("process_instance_id") == previous_gamma_instance:
            return None, f"restarted gamma reused process instance: {gamma}"
        if gamma_process.get("restart_count") != 1:
            return None, f"restarted gamma restart_count mismatch: {gamma}"
        if gamma.get("current_task_id") != previous_gamma_task:
            return None, f"restarted gamma task identity changed: {gamma}"
        if gamma.get("current_execution_id") != previous_gamma_execution:
            return None, f"restarted gamma execution identity changed: {gamma}"
    else:
        return None, f"unknown phase {phase}"
    return workers, None

async def main():
    deadline = asyncio.get_event_loop().time() + 30
    last_error = "no query"
    while asyncio.get_event_loop().time() < deadline:
        message = await query_board()
        workers, error = validate(message)
        if workers is not None:
            print(json.dumps({
                "phase": phase,
                "workers": workers,
            }, ensure_ascii=False, sort_keys=True))
            return
        last_error = error
        await asyncio.sleep(1)
    raise RuntimeError(f"worker health phase {phase} did not converge: {last_error}")

asyncio.run(main())
PY
}

verify_worker_health_restart() {
  local initial_health old_gamma_pid gamma_instance gamma_task gamma_execution
  local offline_health restarted_health
  initial_health="$(query_worker_health fresh "$worker_alpha_pid" "$worker_beta_pid" "$worker_gamma_pid")"
  gamma_instance="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["workers"]["worker-gamma"]["process"]["process_instance_id"])' "$initial_health")"
  gamma_task="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["workers"]["worker-gamma"].get("current_task_id") or "")' "$initial_health")"
  gamma_execution="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["workers"]["worker-gamma"].get("current_execution_id") or "")' "$initial_health")"

  old_gamma_pid="$worker_gamma_pid"
  kill "$old_gamma_pid"
  if [[ "$worker_start_mode" == "launchd" ]]; then
    wait_for_launchd_worker_pid worker-gamma "$old_gamma_pid"
    worker_gamma_pid="$worker_start_pid"
    if [[ "$worker_gamma_pid" == "$old_gamma_pid" ]]; then
      echo "launchd worker-gamma restart reused PID $old_gamma_pid" >&2
      return 1
    fi
    verify_worker_processes
    restarted_health="$(query_worker_health restarted "$worker_alpha_pid" "$worker_beta_pid" "$worker_gamma_pid" "$gamma_instance" "$gamma_task" "$gamma_execution")"
    health_proof="$(python3 - "$initial_health" "$restarted_health" "$old_gamma_pid" "$worker_gamma_pid" "$(launchd_label_for_agent worker-gamma)" <<'PY'
import json
import sys

initial, restarted = (json.loads(value) for value in sys.argv[1:3])
print(json.dumps({
    "ok": True,
    "mode": "launchd_keepalive",
    "initial": initial,
    "restarted": restarted,
    "old_gamma_pid": int(sys.argv[3]),
    "new_gamma_pid": int(sys.argv[4]),
    "gamma_launchd_label": sys.argv[5],
}, ensure_ascii=False, sort_keys=True))
PY
)"
    return 0
  fi
  wait "$old_gamma_pid" >/dev/null 2>&1 || true
  worker_gamma_pid=""
  offline_health="$(query_worker_health offline "$worker_alpha_pid" "$worker_beta_pid" "$old_gamma_pid" "$gamma_instance" "$gamma_task" "$gamma_execution")"

  start_worker worker-gamma
  worker_gamma_pid="$worker_start_pid"
  if [[ "$worker_gamma_pid" == "$old_gamma_pid" ]]; then
    echo "worker-gamma restart reused PID $old_gamma_pid" >&2
    return 1
  fi
  verify_worker_processes
  restarted_health="$(query_worker_health restarted "$worker_alpha_pid" "$worker_beta_pid" "$worker_gamma_pid" "$gamma_instance" "$gamma_task" "$gamma_execution")"

  health_proof="$(python3 - "$initial_health" "$offline_health" "$restarted_health" "$old_gamma_pid" "$worker_gamma_pid" <<'PY'
import json
import sys

initial, offline, restarted = (json.loads(value) for value in sys.argv[1:4])
print(json.dumps({
    "ok": True,
    "initial": initial,
    "offline_after_ttl": offline,
    "restarted": restarted,
    "old_gamma_pid": int(sys.argv[4]),
    "new_gamma_pid": int(sys.argv[5]),
}, ensure_ascii=False, sort_keys=True))
PY
)"
}

start_fixture() {
  node - "$port" >"$mock_log" 2>&1 <<'NODE' &
const http = require("http");
const port = Number(process.argv[2]);
const sessions = new Map();
const rejectedTasks = new Set();
const workerRuns = new Map();
const gammaFailureAttempts = new Map();
let count = 0;

function textResponse(text) {
  return {
    content: [{ type: "text", text }],
    usage: { input_tokens: 80, output_tokens: 80 },
    stop_reason: "end_turn",
  };
}

function completion(summary, evidence, learned = "fixture-driven proof") {
  return `${summary}\n<freehand_completion>\n${JSON.stringify({
    claim: "complete",
    completion_reason: "three worker e2e fixture completed",
    evidence,
    summary,
    learned,
  })}\n</freehand_completion>`;
}

function waiting(nextStep) {
  return `Parent goal evaluation created required next-round work.\n<freehand_completion>\n${JSON.stringify({
    claim: "waiting",
    next_step: nextStep,
  })}\n</freehand_completion>`;
}

function toolUse(id, input) {
  return {
    content: [{ type: "tool_use", id, name: "task", input }],
    usage: { input_tokens: 80, output_tokens: 40 },
    stop_reason: "tool_use",
  };
}

function match(body, key) {
  const escaped = new RegExp(`${key}=([^\\\\\\n"]+)`).exec(body);
  if (escaped) return escaped[1];
  const plain = new RegExp(`${key}=([^\\n"]+)`).exec(body);
  return plain ? plain[1] : null;
}

function idsFromBody(body) {
  const taskIdMatch = /task-three-worker-([0-9]+)-(?:alpha|beta|gamma|integration)/.exec(body);
  const stamp = match(body, "FH3_STAMP") || taskIdMatch?.[1] || "missing-stamp";
  const parentSessionMatch = /"session_id"\s*:\s*"(online-master-three-worker-[^"]+)"/.exec(body)
    || /(online-master-three-worker-evaluation-[0-9]+)/.exec(body);
  const workerFor = (name) => match(body, `FH3_WORKER_${name.toUpperCase()}`) || `worker-${name}`;
  return {
    stamp,
    session: match(body, "FH3_SESSION") || parentSessionMatch?.[1] || "online-master-three-worker-e2e",
    targetCwd: match(body, "FH3_TARGET_CWD") || process.env.FH3_TARGET_CWD,
    tasks: ["alpha", "beta", "gamma"].map((name) => ({
      name,
      id: match(body, `FH3_TASK_${name.toUpperCase()}`) || `task-three-worker-${stamp}-${name}`,
      result: `worker_result_${name}=${stamp}`,
      worker: workerFor(name),
    })),
    integration: {
      name: "integration",
      id: match(body, "FH3_TASK_INTEGRATION") || `task-three-worker-${stamp}-integration`,
      result: `worker_result_integration=${stamp}`,
      worker: match(body, "FH3_WORKER_INTEGRATION") || "worker-alpha",
    },
  };
}

function isWorkerRequest(body) {
  return body.includes("Worker execution policy") || body.includes("Role: you are a Worker");
}

function workerResponse(body) {
  const ids = idsFromBody(body);
  const allTasks = [...ids.tasks, ids.integration];
  const task = allTasks.find((candidate) => body.includes(candidate.id)) || ids.tasks[0];
  const run = (workerRuns.get(task.id) || 0) + 1;
  workerRuns.set(task.id, run);
  const result = task.name === "beta" && run === 1
    ? `worker_result_beta_draft=${ids.stamp}`
    : task.result;
  const summary = `Worker ${task.name} completed ${result}`;
  return textResponse(completion(summary, `task_id=${task.id}; ${result}`));
}

function hasTaskEvent(body, task, eventType) {
  const escaped = task.id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`task_id=${escaped}[^\\n]*${eventType}`).test(body)
    || new RegExp(`"task_id"\\s*:\\s*"${escaped}"[\\s\\S]{0,2400}${eventType}`).test(body);
}

function allHaveEvent(body, ids, eventType) {
  return ids.tasks.every((task) => hasTaskEvent(body, task, eventType));
}

function stateFor(ids) {
  if (!sessions.has(ids.session)) {
    sessions.set(ids.session, { stage: 0, historyPolls: 0 });
  }
  return sessions.get(ids.session);
}

function lifecycleTaskId(body) {
  const snapshotStart = body.indexOf("Task snapshot:");
  const triggerStart = body.indexOf("Trigger event:");
  const scopedBody = snapshotStart >= 0
    ? body.slice(snapshotStart, triggerStart >= 0 ? triggerStart : undefined)
    : body;
  const matches = [...scopedBody.matchAll(/task-three-worker-[0-9]+-(?:alpha|beta|gamma|integration)/g)]
    .map((match) => match[0]);
  return matches.length > 0
    ? matches[matches.length - 1]
    : null;
}

function lifecycleEventKind(body) {
  const triggerStart = body.indexOf("Trigger event:");
  const scopedBody = triggerStart >= 0 ? body.slice(triggerStart) : body;
  const quoted = /"kind"\s*:\s*"(review_ready|execution_interrupted|execution_blocked)"/.exec(scopedBody);
  if (quoted) return quoted[1];
  const escaped = /\\\"kind\\\"\s*:\s*\\\"(review_ready|execution_interrupted|execution_blocked)\\\"/.exec(scopedBody);
  return escaped ? escaped[1] : null;
}

function masterResponse(body) {
  if (
    body.includes("production Master lifecycle coordinator")
    || (body.includes("Task snapshot:") && body.includes("Trigger event:"))
    || (body.includes("review_ready") && body.includes("Task snapshot"))
    || (body.includes("execution_interrupted") && body.includes("Task snapshot"))
  ) {
    const taskId = lifecycleTaskId(body);
    if (!taskId) {
      throw new Error("master lifecycle request missing three-worker task_id");
    }
    const ids = idsFromBody(body);
    const eventKind = lifecycleEventKind(body);
    if (taskId.endsWith("-gamma") && eventKind === "execution_interrupted") {
      return toolUse(`fh3_lifecycle_takeover_${taskId}`, {
        op: "assign",
        task_id: taskId,
        agent_id: ids.integration.worker,
      });
    }
    if (taskId.endsWith("-beta") && !rejectedTasks.has(taskId)) {
      rejectedTasks.add(taskId);
      return toolUse(`fh3_lifecycle_reject_${taskId}`, {
        op: "reject",
        task_id: taskId,
        reject_reason: "beta draft does not satisfy integration-ready quality",
        next_requirements: [
          `resubmit exact worker_result_beta=${ids.stamp}`,
          "provide integration-ready evidence",
        ],
      });
    }
    if (body.includes("Task approved:")) {
      return toolUse(`fh3_lifecycle_close_${taskId}`, {
        op: "close",
        task_id: taskId,
      });
    }
    return toolUse(`fh3_lifecycle_approve_${taskId}`, {
      op: "approve",
      task_id: taskId,
    });
  }

  if (body.includes("<freehand_parent_evaluation")) {
    const ids = idsFromBody(body);
    const results = {};
    for (const match of body.matchAll(
      /review_summary[\s\S]{0,1200}?worker_result_(alpha|beta|gamma|integration)=([0-9]+)/g,
    )) {
      results[match[1]] = `worker_result_${match[1]}=${match[2]}`;
    }
    const missing = ["alpha", "beta", "gamma"].filter((name) => !results[name]);
    if (missing.length > 0) {
      throw new Error(`parent evaluation missing accepted worker results: ${missing.join(",")}`);
    }
    if (!body.includes(ids.integration.id)) {
      throw new Error("parent evaluation missing original integration task objective");
    }
    if (results.integration) {
      const summary = `Overall goal complete after evaluation and improvement: ${["alpha", "beta", "gamma", "integration"].map((name) => results[name]).join("; ")}`;
      return textResponse(completion(
        summary,
        "beta was rejected and redone; integration task was created after first-set evaluation; all accepted child evidence now satisfies the overall goal",
        "Parent evaluation created next-round work before verified final completion.",
      ));
    }
    if (!body.includes(`Task created: task_id=${ids.integration.id}`)
        && !body.includes(`"task_id":"${ids.integration.id}"`)
        && !body.includes(`"task_id": "${ids.integration.id}"`)) {
      return toolUse("fh3_create_integration", {
        op: "create",
        task_id: ids.integration.id,
        title: "Integrate accepted alpha beta gamma results",
        content: `Evaluate and integrate alpha, beta, and gamma into ${ids.integration.result}.`,
        goal: "Close the overall-goal integration gap left after the first three accepted subtasks.",
        deliverables: ["integration result token"],
        acceptance: [
          `return exact ${ids.integration.result}`,
          "prove alpha beta gamma results were evaluated together",
        ],
        target_cwd: ids.targetCwd,
        dispatch: { mode: "none" },
        priority: 99,
      });
    }
    if (!body.includes(`Task assigned: task_id=${ids.integration.id}`)) {
      return toolUse("fh3_assign_integration", {
        op: "assign",
        task_id: ids.integration.id,
        agent_id: ids.integration.worker,
      });
    }
    return textResponse(waiting(
      `Wait for ${ids.integration.id}, review it against the overall objective, then reevaluate the parent goal.`,
    ));
  }

  const ids = idsFromBody(body);
  const state = stateFor(ids);
  const tasks = ids.tasks;
  const targetCwd = ids.targetCwd;
  if (!targetCwd) throw new Error("missing FH3_TARGET_CWD");

  const createTask = (task, priority) => toolUse(`fh3_create_${task.name}`, {
    op: "create",
    task_id: task.id,
    title: `Three worker E2E ${task.name}`,
    content: `Worker ${task.name} must return ${task.result}. No file writes are required.`,
    goal: `Return exact result token ${task.result} for the three-worker E2E proof.`,
    deliverables: [`${task.name} result token`],
    acceptance: [`final summary includes ${task.result}`],
    target_cwd: targetCwd,
    dispatch: { mode: "none" },
    priority,
  });
  const assignTask = (task) => toolUse(`fh3_assign_${task.name}`, {
    op: "assign",
    task_id: task.id,
    agent_id: task.worker,
  });
  const historyTask = (task) => toolUse(`fh3_history_${task.name}_${state.historyPolls}`, {
    op: "history",
    task_id: task.id,
  });
  const approveTask = (task) => toolUse(`fh3_approve_${task.name}`, {
    op: "approve",
    task_id: task.id,
  });
  const closeTask = (task) => toolUse(`fh3_close_${task.name}`, {
    op: "close",
    task_id: task.id,
  });

  const planned = [
    () => createTask(tasks[0], 93),
    () => assignTask(tasks[0]),
    () => createTask(tasks[1], 92),
    () => assignTask(tasks[1]),
    () => createTask(tasks[2], 91),
    () => assignTask(tasks[2]),
  ];
  if (state.stage < planned.length) {
    const response = planned[state.stage]();
    state.stage += 1;
    return response;
  }

  if (!allHaveEvent(body, ids, "TaskReviewSubmitted")) {
    return textResponse(waiting(
      "The first three Worker tasks have been dispatched. Wait for the production Worker and Master lifecycle runners to execute, review, and close them before parent-goal evaluation.",
    ));
  }

  for (const task of tasks) {
    if (!hasTaskEvent(body, task, "TaskReviewApproved")) {
      return approveTask(task);
    }
    if (!hasTaskEvent(body, task, "TaskClosed")) {
      return closeTask(task);
    }
  }

  if (!allHaveEvent(body, ids, "TaskClosed")) {
    const task = tasks[state.historyPolls % tasks.length];
    state.historyPolls += 1;
    return historyTask(task);
  }

  return textResponse(waiting(
    "The first three required Worker tasks are closed. Wait for the production parent-goal evaluation to compare their accepted review truth with the overall objective before any final completion claim.",
  ));
}

const server = http.createServer((req, res) => {
  let body = "";
  req.on("data", chunk => { body += chunk; });
  req.on("end", () => {
    const workerRequest = isWorkerRequest(body);
    const respond = () => {
      count += 1;
      try {
        const ids = idsFromBody(body);
        const gammaTask = ids.tasks.find((task) => task.name === "gamma");
        const gammaExecution = body.includes("exec-worker-worker-gamma-");
        if (workerRequest && gammaExecution && body.includes(gammaTask.id)) {
          const failureAttempt = (gammaFailureAttempts.get(gammaTask.id) || 0) + 1;
          gammaFailureAttempts.set(gammaTask.id, failureAttempt);
          if (failureAttempt <= 10) {
            console.log(JSON.stringify({
              count,
              url: req.url,
              role: "worker",
              session: ids.session,
              forced_gamma_interruption_attempt: failureAttempt,
            }));
            res.writeHead(500, { "content-type": "application/json" });
            res.end(JSON.stringify({
              type: "error",
              error: {
                type: "fixture_gamma_route_failure",
                message: "worker-gamma route unavailable; force same-task takeover",
              },
            }));
            return;
          }
        }
        const response = workerRequest ? workerResponse(body) : masterResponse(body);
        console.log(JSON.stringify({
          count,
          url: req.url,
          role: workerRequest ? "worker" : "master",
          session: ids.session,
          stage: sessions.get(ids.session)?.stage ?? null,
        }));
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify(response));
      } catch (error) {
        console.error(`three-worker-fixture-error ${error.message}`);
        res.writeHead(500, { "content-type": "application/json" });
        res.end(JSON.stringify({ type: "error", error: { type: "fixture_error", message: error.message } }));
      }
    };
    if (workerRequest) {
      setTimeout(respond, 12000);
    } else {
      respond();
    }
  });
});
server.on("error", err => {
  console.error(`three-worker-fixture-listen-error ${err.message}`);
  process.exit(3);
});
server.listen(port, "127.0.0.1", () => {
  console.log(`three-worker-fixture-listening http://127.0.0.1:${port}`);
});
process.on("SIGTERM", () => server.close(() => process.exit(0)));
NODE
  mock_pid="$!"
  for _ in $(seq 1 30); do
    if grep -q "three-worker-fixture-listening" "$mock_log"; then
      return 0
    fi
    if ! kill -0 "$mock_pid" >/dev/null 2>&1; then
      cat "$mock_log" >&2
      exit 2
    fi
    sleep 1
  done
  cat "$mock_log" >&2
  echo "three-worker fixture did not become ready" >&2
  exit 2
}

run_adp_submit_and_verify() {
  local stamp="$1"
  local task_alpha="$2"
  local task_beta="$3"
  local task_gamma="$4"
  local task_integration="$5"
  local worker_alpha="$6"
  local worker_beta="$7"
  local worker_gamma="$8"
  local worker_integration="$9"
  local prompt="${10}"
  python3 - "$adp_url" "$session_id" "$task_alpha" "$task_beta" "$task_gamma" "$task_integration" "$worker_alpha" "$worker_beta" "$worker_gamma" "$worker_integration" "$prompt" <<'PY'
import asyncio
import json
import os
import sys
import websockets

(
    url,
    session_id,
    task_alpha,
    task_beta,
    task_gamma,
    task_integration,
    worker_alpha,
    worker_beta,
    worker_gamma,
    worker_integration,
    prompt,
) = sys.argv[1:12]
task_ids = [task_alpha, task_beta, task_gamma, task_integration]
expected_workers = {
    task_alpha: worker_alpha,
    task_beta: worker_beta,
    task_gamma: worker_alpha,
    task_integration: worker_integration,
}
initial_workers = {
    task_alpha: worker_alpha,
    task_beta: worker_beta,
    task_gamma: worker_gamma,
    task_integration: worker_integration,
}
headers = {
    "Authorization": f"Bearer {os.environ['FREEHAND_THREE_WORKER_ADP_AUTH_TOKEN']}"
}

def adp_command(request_id, command):
    return {"protocol_version": 4, "kind": "command", "request_id": request_id, "command": command}

def adp_query(request_id, query):
    return {"protocol_version": 4, "kind": "query", "request_id": request_id, "query": query}

async def handshake(ws, request_id):
    await ws.send(json.dumps({
        "protocol_version": 4,
        "kind": "handshake",
        "request_id": request_id,
        "client_name": "three-worker-verifier",
        "capabilities": ["adp.v4.handshake"],
    }))
    response = json.loads(await asyncio.wait_for(ws.recv(), timeout=20))
    if response.get("request_id") != request_id or response.get("kind") != "handshake_accepted":
        raise RuntimeError(f"ADP handshake failed: {response}")

async def recv_until(ws, request_id, timeout_seconds):
    deadline = asyncio.get_event_loop().time() + timeout_seconds
    seen = []
    while True:
        remaining = deadline - asyncio.get_event_loop().time()
        if remaining <= 0:
            raise RuntimeError(f"timeout waiting for {request_id}; seen={seen}")
        msg = json.loads(await asyncio.wait_for(ws.recv(), timeout=remaining))
        if msg.get("request_id") == request_id:
            return msg
        seen.append(f"{msg.get('kind')}:{msg.get('request_id')}")

async def query_all():
    async with websockets.connect(url, additional_headers=headers) as ws:
        await handshake(ws, "three-worker-query-handshake")
        requests = [
            adp_query("turns", {"QuerySessionTurns": {"session_id": session_id}}),
            adp_query("tasks", {"QueryTaskBoard": {"include_terminal": True}}),
            adp_query("agents", {"QueryAgentBoard": {}}),
        ]
        for task_id in task_ids:
            requests.append(adp_query(f"history-{task_id}", {"QueryTaskHistory": {"task_id": task_id}}))
        responses = {}
        for request in requests:
            await ws.send(json.dumps(request))
            responses[request["request_id"]] = await recv_until(
                ws,
                request["request_id"],
                20,
            )
        return responses

async def main():
    async with websockets.connect(url, additional_headers=headers) as ws:
        await handshake(ws, "three-worker-submit-handshake")
        await ws.send(json.dumps(adp_command("three-worker-submit", {
            "SubmitUserInput": {"text": prompt, "session_id": session_id}
        })))
        receipt = await recv_until(ws, "three-worker-submit", 420)
    def collect_dispatch_status(value):
        statuses = []
        if isinstance(value, dict):
            for key, nested in value.items():
                if key == "dispatch_status" and isinstance(nested, str):
                    statuses.append(nested)
                else:
                    statuses.extend(collect_dispatch_status(nested))
        elif isinstance(value, list):
            for nested in value:
                statuses.extend(collect_dispatch_status(nested))
        return statuses
    receipt_statuses = collect_dispatch_status(receipt)
    if receipt.get("error"):
        raise RuntimeError(f"SubmitUserInput receipt failed: {receipt}")
    if not any(status.startswith("reason_live_turn_completed") for status in receipt_statuses):
        raise RuntimeError(
            f"SubmitUserInput receipt did not complete the foreground waiting turn: {receipt}"
        )
    required_results = [
        "worker_result_alpha",
        "worker_result_beta",
        "worker_result_gamma",
        "worker_result_integration",
    ]
    deadline = asyncio.get_event_loop().time() + 420
    while True:
        responses = await query_all()
        transcript = responses["turns"].get("result", {}).get("SessionTurns", {})
        turns = transcript.get("turns", [])
        if turns:
            last_turn = turns[-1]
            terminal = last_turn.get("terminal_text") or ""
            missing_results = [item for item in required_results if item not in terminal]
            if last_turn.get("terminal_status") == "Success" and not missing_results:
                break
        if asyncio.get_event_loop().time() >= deadline:
            raise RuntimeError(
                f"timed out waiting for final parent evaluation; turns={turns}"
            )
        await asyncio.sleep(2)

    if len(turns) < 2:
        raise RuntimeError(f"expected original plus parent evaluation turns, got {len(turns)}")
    if not any(turn.get("user_text") == prompt for turn in turns[:-1]):
        raise RuntimeError("original submitted prompt is missing from prior session turns")
    if last_turn.get("user_text") not in (None, ""):
        raise RuntimeError(
            f"parent evaluation exposed synthetic user text: {last_turn.get('user_text')}"
        )
    if "<freehand_parent_evaluation" in json.dumps(last_turn):
        raise RuntimeError("parent evaluation marker leaked into final UI projection")

    prior_waiting = [
        turn for turn in turns[:-1]
        if turn.get("terminal_status") == "ToolPending"
        and task_integration in (turn.get("terminal_text") or "")
    ]
    if len(prior_waiting) != 1:
        raise RuntimeError(
            f"expected one waiting parent evaluation that created integration work, got {prior_waiting}"
        )

    board_tasks = responses["tasks"].get("result", {}).get("TaskBoard", {}).get("tasks", [])
    matching_tasks = [task for task in board_tasks if task.get("task_id") in task_ids]
    if len(matching_tasks) != 4:
        raise RuntimeError(f"expected 4 matching tasks after next-round evaluation, got {len(matching_tasks)}")
    bad_parent = [task for task in matching_tasks if task.get("parent_session_id") != session_id]
    if bad_parent:
        raise RuntimeError(f"tasks not parented to session {session_id}: {bad_parent}")
    not_closed = [task for task in matching_tasks if task.get("status") != "closed"]
    if not_closed:
        raise RuntimeError(f"tasks not closed: {not_closed}")
    wrong_assignees = [
        task for task in matching_tasks
        if task.get("assignee_agent_id") != expected_workers[task.get("task_id")]
    ]
    if wrong_assignees:
        raise RuntimeError(f"tasks crossed configured Worker assignment boundaries: {wrong_assignees}")

    premature_success = [
        turn for turn in turns[:-1]
        if turn.get("terminal_status") == "Success"
    ]
    if premature_success:
        raise RuntimeError(
            f"parent session exposed success before overall-goal evaluation closed next-round work: {premature_success}"
        )

    required_events = [
        "TaskCreated",
        "TaskAssigned",
        "TaskResumed",
        "TaskHeartbeat",
        "TaskReviewSubmitted",
        "TaskReviewApproved",
        "TaskClosed",
    ]
    histories = {}
    execution_histories = {}
    for task_id in task_ids:
        events = (
            responses[f"history-{task_id}"]
            .get("result", {})
            .get("TaskHistory", {})
            .get("events", [])
        )
        event_types = [event.get("event_type") for event in events]
        histories[task_id] = event_types
        missing = [event for event in required_events if event not in event_types]
        if missing:
            raise RuntimeError(f"{task_id} missing events {missing}: {event_types}")
        expected_worker = expected_workers[task_id]
        initial_worker = initial_workers[task_id]
        first_assignment = next(
            event for event in events if event.get("event_type") == "TaskAssigned"
        )
        if first_assignment.get("actor_agent_id") != "master":
            raise RuntimeError(
                f"{task_id} first assignment was not authored by master: {first_assignment}"
            )
        assignment_workers = [
            (event.get("payload") or {}).get("agent_id")
            for event in events
            if event.get("event_type") == "TaskAssigned"
        ]
        if task_id == task_gamma:
            expected_assignment_workers = [worker_gamma, worker_alpha]
        elif task_id == task_beta:
            expected_assignment_workers = [worker_beta, worker_beta]
        else:
            expected_assignment_workers = [initial_worker]
        if assignment_workers != expected_assignment_workers:
            raise RuntimeError(
                f"{task_id} assignment history {assignment_workers} "
                f"expected {expected_assignment_workers}"
            )
        allowed_execution_workers = set(expected_assignment_workers)
        for event in events:
            event_type = event.get("event_type")
            actor = event.get("actor_agent_id")
            payload = event.get("payload") or {}
            if event_type in {"TaskResumed", "TaskHeartbeat", "TaskReviewSubmitted"}:
                if actor not in allowed_execution_workers:
                    raise RuntimeError(
                        f"{task_id} {event_type} actor {actor} escaped "
                        f"{sorted(allowed_execution_workers)}"
                    )
                payload_worker = payload.get("agent_id") or payload.get("claim_agent_id")
                if payload_worker not in allowed_execution_workers:
                    raise RuntimeError(
                        f"{task_id} {event_type} payload worker {payload_worker} escaped "
                        f"{sorted(allowed_execution_workers)}"
                    )
            if event_type in {"TaskReviewRejected", "TaskReviewApproved", "TaskClosed"}:
                if actor != "master":
                    raise RuntimeError(
                        f"{task_id} {event_type} actor {actor} was not master"
                    )
        execution_ids = {
            (event.get("payload") or {}).get("execution_id")
            for event in events
            if event.get("event_type") in {"TaskResumed", "TaskReviewSubmitted"}
            and (event.get("payload") or {}).get("execution_id")
        }
        if not execution_ids:
            raise RuntimeError(f"{task_id} has no worker execution identity")
        expected_execution_prefixes = tuple(
            f"exec-worker-{assignment_worker}-"
            for assignment_worker in allowed_execution_workers
        )
        if any(not execution_id.startswith(expected_execution_prefixes) for execution_id in execution_ids):
            raise RuntimeError(
                f"{task_id} execution ids do not belong to "
                f"{sorted(allowed_execution_workers)}: {sorted(execution_ids)}"
            )
        execution_histories[task_id] = sorted(execution_ids)
    beta_events = histories[task_beta]
    if "TaskReviewRejected" not in beta_events:
        raise RuntimeError(f"beta task was never rejected for rework: {beta_events}")
    if beta_events.count("TaskReviewSubmitted") < 2:
        raise RuntimeError(f"beta task did not resubmit after rejection: {beta_events}")
    if len(execution_histories[task_beta]) < 2:
        raise RuntimeError(
            f"beta task did not use a new execution for rework: {execution_histories[task_beta]}"
        )
    gamma_events = histories[task_gamma]
    if "TaskInterrupted" not in gamma_events:
        raise RuntimeError(f"gamma task never entered interrupted truth: {gamma_events}")
    if len(execution_histories[task_gamma]) < 2:
        raise RuntimeError(
            f"gamma task did not use distinct gamma and alpha executions: "
            f"{execution_histories[task_gamma]}"
        )
    if not any(
        execution_id.startswith(f"exec-worker-{worker_gamma}-")
        for execution_id in execution_histories[task_gamma]
    ) or not any(
        execution_id.startswith(f"exec-worker-{worker_alpha}-")
        for execution_id in execution_histories[task_gamma]
    ):
        raise RuntimeError(
            f"gamma same-task takeover lacks both worker histories: "
            f"{execution_histories[task_gamma]}"
        )
    first_round_execution_ids = {
        execution_histories[task_id][0]
        for task_id in [task_alpha, task_beta, task_gamma]
    }
    if len(first_round_execution_ids) != 3:
        raise RuntimeError(
            f"initial workers did not produce three distinct execution histories: {execution_histories}"
        )

    agents = responses["agents"].get("result", {}).get("AgentBoard", {}).get("agents", [])
    worker_agents = {
        agent.get("agent_id"): agent
        for agent in agents
        if agent.get("agent_id") in {worker_alpha, worker_beta, worker_gamma}
    }
    if set(worker_agents) != {worker_alpha, worker_beta, worker_gamma}:
        raise RuntimeError(
            f"AgentBoard did not expose all three configured Worker identities: {worker_agents}"
        )
    released_gamma = worker_agents[worker_gamma]
    if released_gamma.get("state") != "idle":
        raise RuntimeError(f"released gamma is not idle after takeover: {released_gamma}")
    if released_gamma.get("current_task_id") is not None:
        raise RuntimeError(
            f"released gamma retained current task after takeover: {released_gamma}"
        )
    if released_gamma.get("current_execution_id") is not None:
        raise RuntimeError(
            f"released gamma retained current execution after takeover: {released_gamma}"
        )
    if (released_gamma.get("last_activity") or {}).get("kind") != "interrupted":
        raise RuntimeError(
            f"released gamma lost interrupted last activity: {released_gamma}"
        )
    result = {
        "ok": True,
        "session_id": session_id,
        "receipt": receipt,
        "turn_count": len(turns),
        "turn_id": last_turn.get("turn_id"),
        "terminal_status": last_turn.get("terminal_status"),
        "terminal_text": terminal,
        "tasks": [
            {
                "task_id": task.get("task_id"),
                "status": task.get("status"),
                "parent_session_id": task.get("parent_session_id"),
                "assignee_agent_id": task.get("assignee_agent_id"),
                "last_event_seq": task.get("last_event_seq"),
                "events": histories[task.get("task_id")],
                "execution_ids": execution_histories[task.get("task_id")],
            }
            for task in matching_tasks
        ],
        "workers": {
            worker_id: {
                "state": worker.get("state"),
                "current_task_id": worker.get("current_task_id"),
                "current_execution_id": worker.get("current_execution_id"),
            }
            for worker_id, worker in sorted(worker_agents.items())
        },
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))

asyncio.run(main())
PY
}

verify_restart_idempotency() {
  local expected_stamp="$1"
  python3 - "$adp_url" "$session_id" "$expected_stamp" <<'PY'
import asyncio
import json
import os
import sys
import websockets

url, session_id, stamp = sys.argv[1:4]
headers = {
    "Authorization": f"Bearer {os.environ['FREEHAND_THREE_WORKER_ADP_AUTH_TOKEN']}"
}

async def main():
    async with websockets.connect(url, additional_headers=headers) as ws:
        await ws.send(json.dumps({
            "protocol_version": 4,
            "kind": "handshake",
            "request_id": "restart-handshake",
            "client_name": "three-worker-verifier",
            "capabilities": ["adp.v4.handshake"],
        }))
        handshake = json.loads(await asyncio.wait_for(ws.recv(), timeout=20))
        if handshake.get("request_id") != "restart-handshake" or handshake.get("kind") != "handshake_accepted":
            raise RuntimeError(f"ADP restart handshake failed: {handshake}")
        await ws.send(json.dumps({
            "protocol_version": 4,
            "kind": "query",
            "request_id": "restart-turns",
            "query": {"QuerySessionTurns": {"session_id": session_id}},
        }))
        while True:
            message = json.loads(await ws.recv())
            if message.get("request_id") == "restart-turns":
                break
    turns = (
        message.get("result", {})
        .get("SessionTurns", {})
        .get("turns", [])
    )
    result_tokens = [
        f"worker_result_alpha={stamp}",
        f"worker_result_beta={stamp}",
        f"worker_result_gamma={stamp}",
        f"worker_result_integration={stamp}",
    ]
    final_evaluations = [
        turn for turn in turns
        if turn.get("terminal_status") == "Success"
        and all(token in (turn.get("terminal_text") or "") for token in result_tokens)
    ]
    if len(final_evaluations) != 1:
        raise RuntimeError(
            f"expected exactly one persisted final parent evaluation after restart, got {len(final_evaluations)}"
        )
    evaluation = final_evaluations[0]
    if evaluation.get("user_text") not in (None, ""):
        raise RuntimeError("restarted parent evaluation projection exposed synthetic user text")
    print(json.dumps({
        "restart_idempotent": True,
        "session_id": session_id,
        "final_evaluation_turn_id": evaluation.get("turn_id"),
        "final_evaluation_count": len(final_evaluations),
        "turn_count": len(turns),
    }, sort_keys=True))

asyncio.run(main())
PY
}

run_three_worker_e2e() {
  trap restore_runtime_config EXIT

  require_cmd node
  require_cmd python3
  require_cmd curl
  if [[ ! -x "$cli_path" ]]; then
    echo "missing executable CLI: $cli_path" >&2
    exit 2
  fi
  if [[ ! -x "$daemon_path" ]]; then
    echo "missing executable daemon: $daemon_path" >&2
    exit 2
  fi

  mkdir -p "$backup_dir" "$target_cwd" "$runtime_home/logs"
  pair_token="three-worker-pair-token-$$"
  write_isolated_config
  export FH3_TARGET_CWD="$target_cwd"
  printf 'fixture workspace\n' >"$target_cwd/README.txt"

  local stamp task_alpha task_beta task_gamma task_integration
  local worker_alpha worker_beta worker_gamma worker_integration
  local prompt proof health_proof restart_proof
  stamp="178$(date +%s)"
  task_alpha="task-three-worker-$stamp-alpha"
  task_beta="task-three-worker-$stamp-beta"
  task_gamma="task-three-worker-$stamp-gamma"
  task_integration="task-three-worker-$stamp-integration"
  worker_alpha="worker-alpha"
  worker_beta="worker-beta"
  worker_gamma="worker-gamma"
  worker_integration="worker-alpha"
  prompt=$'Three independent Worker iterative goal proof.\n'"FH3_STAMP=$stamp"$'\n'"FH3_SESSION=$session_id"$'\n'"FH3_WORKER_ALPHA=$worker_alpha"$'\n'"FH3_WORKER_BETA=$worker_beta"$'\n'"FH3_WORKER_GAMMA=$worker_gamma"$'\n'"FH3_WORKER_INTEGRATION=$worker_integration"$'\n'"FH3_TARGET_CWD=$target_cwd"$'\n'"FH3_TASK_ALPHA=$task_alpha"$'\n'"FH3_TASK_BETA=$task_beta"$'\n'"FH3_TASK_GAMMA=$task_gamma"$'\n'"FH3_TASK_INTEGRATION=$task_integration"$'\nOverall goal: produce verified alpha, beta, and gamma results, reject and redo any result that is not integration-ready, then evaluate whether additional integration work is required. Do not finish merely by summarizing the first three results. Create exactly three initial Worker tasks: assign alpha to worker-alpha, beta to worker-beta, and gamma to worker-gamma. Each independent Worker must execute only its assigned task. The parent evaluation must compare accepted review truth with the overall goal and create the integration task if the first accepted set still leaves the overall integration goal incomplete. Final completion requires the integration task to close and all four exact worker_result_* tokens to be verified.'

  start_fixture

  start_master
  start_workers
  verify_worker_processes

  FREEHAND_ADP_AUTH_TOKEN="$adp_auth_token" "$cli_path" adp-session-manage --url "$adp_url" --action delete --session "$session_id" >/dev/null 2>&1 || true
  FREEHAND_ADP_AUTH_TOKEN="$adp_auth_token" "$cli_path" adp-session-manage --url "$adp_url" --action create --session "$session_id" --title "Three worker E2E" --cwd "$repo_root" >/dev/null

  proof="$(run_adp_submit_and_verify \
    "$stamp" \
    "$task_alpha" \
    "$task_beta" \
    "$task_gamma" \
    "$task_integration" \
    "$worker_alpha" \
    "$worker_beta" \
    "$worker_gamma" \
    "$worker_integration" \
    "$prompt")"
  verify_worker_processes
  health_proof=""
  verify_worker_health_restart
  verify_worker_processes
  stop_master
  start_master
  restart_proof="$(verify_restart_idempotency "$stamp")"
  printf '%s\n' "$proof" >"$backup_dir/three-worker-proof.json"
  printf '%s\n' "$health_proof" >"$backup_dir/worker-health-proof.json"
  printf '%s\n' "$restart_proof" >"$backup_dir/restart-proof.json"
  echo "master_three_worker_e2e_ok url=$adp_url session=$session_id initial_tasks=$task_alpha:$worker_alpha,$task_beta:$worker_beta,$task_gamma:$worker_gamma next_task=$task_integration:$worker_integration worker_pids=$worker_alpha_pid,$worker_beta_pid,$worker_gamma_pid evidence_dir=$backup_dir"
  echo "$proof"
  echo "$health_proof"
  echo "$restart_proof"
}

run_three_worker_e2e "$@"
