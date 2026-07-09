#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
runtime_home="${FREEHAND_RUNTIME_HOME:-"$HOME/.freehand"}"
adp_url="${FREEHAND_MASTER_AUTONOMY_ADP_URL:-ws://127.0.0.1:4042/adp}"
health_url="${FREEHAND_MASTER_AUTONOMY_HEALTH_URL:-http://127.0.0.1:4042/health}"
cli_path="${FREEHAND_MASTER_AUTONOMY_CLI:-$HOME/.local/bin/freehand-cliS}"
port="${FREEHAND_MASTER_AUTONOMY_FIXTURE_PORT:-18082}"
fixture_key_name="FREEHAND_MASTER_AUTONOMY_FIXTURE_KEY"
fixture_key_value="test-master-autonomy-key"
config_path="$runtime_home/config.toml"
env_path="$runtime_home/daemonS.env"
backup_dir="$runtime_home/tmp/master-autonomy-online-$(date +%Y%m%dT%H%M%S)-$$"
mock_log="$backup_dir/mock-provider.log"
mock_pid=""

cd "$repo_root"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}

restore_runtime_config() {
  local restore_status=0
  if [[ -n "$mock_pid" ]]; then
    if kill -0 "$mock_pid" >/dev/null 2>&1; then
      kill "$mock_pid" >/dev/null 2>&1 || restore_status=$?
      wait "$mock_pid" >/dev/null 2>&1 || true
    fi
  fi
  if [[ -f "$backup_dir/config.toml" ]]; then
    cp "$backup_dir/config.toml" "$config_path" || restore_status=$?
  fi
  if [[ -f "$backup_dir/daemonS.env" ]]; then
    cp "$backup_dir/daemonS.env" "$env_path" || restore_status=$?
  fi
  if [[ -f "$config_path" && -f "$env_path" ]]; then
    scripts/install-launchd.sh restartS >/dev/null || restore_status=$?
  fi
  return "$restore_status"
}

extract_field() {
  local line="$1"
  local field="$2"
  sed -n "s/.* ${field}=\\([^ ]*\\) .*/\\1/p" <<<"$line"
}

run_master_worker_autonomy_online() {
  trap restore_runtime_config EXIT

  require_cmd node
  require_cmd curl
  if [[ ! -x "$cli_path" ]]; then
    echo "missing executable CLI: $cli_path" >&2
    exit 2
  fi
  if [[ ! -f "$config_path" ]]; then
    echo "missing config: $config_path" >&2
    exit 2
  fi
  if [[ ! -f "$env_path" ]]; then
    echo "missing S env: $env_path" >&2
    exit 2
  fi

  mkdir -p "$backup_dir"
  cp "$config_path" "$backup_dir/config.toml"
  cp "$env_path" "$backup_dir/daemonS.env"

  node - "$port" >"$mock_log" 2>&1 <<'NODE' &
const http = require("http");
const port = Number(process.argv[2]);
const states = new Map();
let count = 0;

function matchValue(body, key) {
  const escaped = new RegExp(`${key}=([^\\\\\\n"]+)`).exec(body);
  if (escaped) return escaped[1];
  const plain = new RegExp(`${key}=([^\\n"]+)`).exec(body);
  return plain ? plain[1] : null;
}

function inferIds(body) {
  const scenario = matchValue(body, "FHMA_SCENARIO")
    || (body.includes("execution-error") ? "execution-error" : body.includes("reject-retry") ? "reject-retry" : "success");
  const taskId = matchValue(body, "FHMA_TASK_ID")
    || (body.match(/task-cli-master-autonomy-[A-Za-z0-9-]+/) || [])[0];
  const workerId = matchValue(body, "FHMA_WORKER_ID")
    || (body.match(/worker-cli-master-autonomy-[A-Za-z0-9-]+/) || [])[0];
  const executionId = matchValue(body, "FHMA_EXECUTION_ID")
    || (body.match(/exec-cli-master-autonomy-[A-Za-z0-9-]+/) || [])[0];
  if (!taskId || !workerId || !executionId) {
    throw new Error(`missing ids in provider request bodyLength=${body.length}`);
  }
  return { scenario, taskId, workerId, executionId };
}

function toolUse(id, input) {
  return {
    content: [{ type: "tool_use", id, name: "task", input }],
    usage: { input_tokens: 50, output_tokens: 20 },
    stop_reason: "tool_use",
  };
}

function complete(text) {
  const schema = {
    claim: "complete",
    completion_reason: "done",
    evidence: "master worker autonomy fixture reached expected task truth",
    summary: text,
    learned: "fixture only drives deterministic online proof",
  };
  return {
    content: [{
      type: "text",
      text: `${text}\n<freehand_completion>\n${JSON.stringify(schema)}\n</freehand_completion>`,
    }],
    usage: { input_tokens: 80, output_tokens: 80 },
    stop_reason: "end_turn",
  };
}

function sequenceFor(ids) {
  const { scenario, taskId, workerId, executionId } = ids;
  const common = [
    toolUse(`toolu_${scenario}_agent`, { op: "create_agent", agent_id: workerId, capabilities: ["code_edit", "test_run"] }),
    toolUse(`toolu_${scenario}_create`, {
      op: "create",
      task_id: taskId,
      title: `Autonomy ${scenario} task`,
      content: `Worker task for ${scenario}`,
      goal: `Prove master worker autonomy ${scenario}`,
      deliverables: ["worker result"],
      acceptance: ["owner task truth matches scenario"],
      dispatch: { mode: "none" },
      priority: scenario === "success" ? 90 : scenario === "execution-error" ? 80 : 85,
    }),
    toolUse(`toolu_${scenario}_assign`, { op: "assign", task_id: taskId, agent_id: workerId }),
    toolUse(`toolu_${scenario}_claim`, { op: "claim_next", agent_id: workerId, execution_id: executionId, ttl_seconds: 600 }),
  ];
  if (scenario === "success") {
    return [
      ...common,
      toolUse("toolu_success_running", {
        op: "record_execution", status: "running", task_id: taskId, agent_id: workerId, execution_id: executionId,
        phase: "implementation", summary: "worker implemented requested change", evidence: ["changed files inspected"],
      }),
      toolUse("toolu_success_review_ready", {
        op: "record_execution", status: "review_ready", task_id: taskId, agent_id: workerId, execution_id: executionId,
        phase: "review", summary: "worker completed all acceptance checks", deliverables: ["success report"], evidence: ["unit test passed"],
      }),
      toolUse("toolu_success_approve", { op: "approve", task_id: taskId }),
      toolUse("toolu_success_close", { op: "close", task_id: taskId }),
      complete("master closed successful worker task"),
    ];
  }
  if (scenario === "execution-error") {
    return [
      ...common,
      toolUse("toolu_error_running", {
        op: "record_execution", status: "running", task_id: taskId, agent_id: workerId, execution_id: executionId,
        phase: "implementation", summary: "worker started execution", evidence: ["worker heartbeat observed"],
      }),
      toolUse("toolu_error_blocked", {
        op: "record_execution", status: "blocked", task_id: taskId, agent_id: workerId, execution_id: executionId,
        phase: "execution_error", summary: "worker hit provider_error_500", evidence: ["provider_error_500", "no deliverable produced"],
      }),
      complete("master left errored worker task blocked"),
    ];
  }
  return [
    ...common,
    toolUse("toolu_retry_incomplete_review", {
      op: "record_execution", status: "review_ready", task_id: taskId, agent_id: workerId, execution_id: executionId,
      phase: "review", summary: "worker submitted partial implementation", deliverables: ["partial report"], evidence: ["no regression evidence"],
    }),
    toolUse("toolu_retry_reject", {
      op: "reject", task_id: taskId, reject_reason: "missing regression proof",
      next_requirements: ["run regression evidence", "resubmit complete deliverable"],
    }),
    toolUse("toolu_retry_recovering", {
      op: "record_execution", status: "recovering", task_id: taskId, agent_id: workerId, execution_id: executionId,
      phase: "retry", summary: "worker is fixing rejected submission", evidence: ["rejection reason acknowledged"], retry_count: 1,
    }),
    toolUse("toolu_retry_complete_review", {
      op: "record_execution", status: "review_ready", task_id: taskId, agent_id: workerId, execution_id: executionId,
      phase: "review", summary: "worker resubmitted complete implementation", deliverables: ["complete report"], evidence: ["regression passed"],
    }),
    toolUse("toolu_retry_approve", { op: "approve", task_id: taskId }),
    toolUse("toolu_retry_close", { op: "close", task_id: taskId }),
    complete("master closed retried worker task"),
  ];
}

const server = http.createServer((req, res) => {
  let body = "";
  req.on("data", chunk => { body += chunk; });
  req.on("end", () => {
    count += 1;
    try {
      const ids = inferIds(body);
      const key = ids.taskId;
      if (!states.has(key)) states.set(key, { ids, index: 0, sequence: sequenceFor(ids) });
      const state = states.get(key);
      const response = state.sequence[state.index];
      if (!response) throw new Error(`sequence exhausted for ${ids.scenario} ${ids.taskId}`);
      state.index += 1;
      console.log(JSON.stringify({ count, method: req.method, url: req.url, scenario: ids.scenario, taskId: ids.taskId, step: state.index }));
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify(response));
    } catch (error) {
      console.error(`mock-provider-error ${error.message}`);
      res.writeHead(500, { "content-type": "application/json" });
      res.end(JSON.stringify({ type: "error", error: { type: "fixture_error", message: error.message } }));
    }
  });
});
server.on("error", err => {
  console.error(`mock-provider-listen-error ${err.message}`);
  process.exit(3);
});
server.listen(port, "127.0.0.1", () => {
  console.log(`mock-provider-listening http://127.0.0.1:${port}`);
});
process.on("SIGTERM", () => server.close(() => process.exit(0)));
NODE
  mock_pid="$!"

  for _ in $(seq 1 30); do
    if grep -q "mock-provider-listening" "$mock_log"; then
      break
    fi
    if ! kill -0 "$mock_pid" >/dev/null 2>&1; then
      cat "$mock_log" >&2
      exit 2
    fi
    sleep 1
  done
  if ! grep -q "mock-provider-listening" "$mock_log"; then
    cat "$mock_log" >&2
    echo "mock provider did not become ready" >&2
    exit 2
  fi

  printf '\n%s="%s"\n' "$fixture_key_name" "$fixture_key_value" >>"$env_path"
  scripts/install-launchd.sh restartS >/dev/null
  curl -4fsS "$health_url" >/dev/null

  "$cli_path" adp-config-update \
    --url "$adp_url" \
    --agent master \
    --provider minimax \
    --type anthropic \
    --protocol messages \
    --base-url "http://127.0.0.1:$port" \
    --model MiniMax-M3 \
    --api-key-env "$fixture_key_name" >/dev/null
  scripts/install-launchd.sh restartS >/dev/null
  curl -4fsS "$health_url" >/dev/null
  "$cli_path" adp-smoke --url "$adp_url" >/dev/null

  sample_output="$("$cli_path" master-worker-autonomy-sample --url "$adp_url" --scenario all)"
  if ! grep -q "master_worker_autonomy_sample_ok" <<<"$sample_output"; then
    echo "$sample_output" >&2
    echo "master autonomy sample did not report success" >&2
    exit 1
  fi
  for scenario in success execution-error reject-retry; do
    if ! grep -q "scenario=$scenario" <<<"$sample_output"; then
      echo "$sample_output" >&2
      echo "missing scenario output: $scenario" >&2
      exit 1
    fi
  done
  for expected in "tool_executions=8" "tool_executions=6" "tool_executions=10" "status=blocked" "TaskReviewRejected" "TaskExecutionRecovering"; do
    if ! grep -q "$expected" <<<"$sample_output"; then
      echo "$sample_output" >&2
      echo "sample output missing expected evidence: $expected" >&2
      exit 1
    fi
  done
  mock_count="$(grep -c '"url":"/v1/messages"' "$mock_log" || true)"
  if [[ "$mock_count" != "27" ]]; then
    cat "$mock_log" >&2
    echo "expected 27 provider attempts, got $mock_count" >&2
    exit 1
  fi

  scripts/install-launchd.sh restartS >/dev/null
  curl -4fsS "$health_url" >/dev/null

  verify_lines=()
  for scenario in success execution-error reject-retry; do
    line="$(grep "scenario=$scenario " <<<"$sample_output" | tail -1)"
    task_id="$(extract_field "$line" "task")"
    execution_id="$(extract_field "$line" "execution")"
    agent_id="$(extract_field "$line" "agent")"
    if [[ -z "$task_id" || -z "$execution_id" || -z "$agent_id" ]]; then
      echo "$sample_output" >&2
      echo "failed to parse ids for scenario $scenario" >&2
      exit 1
    fi
    verify_output="$("$cli_path" master-worker-autonomy-sample --url "$adp_url" --scenario "$scenario" --verify-task "$task_id" --execution "$execution_id" --agent "$agent_id")"
    if ! grep -q "master_worker_autonomy_verify_ok" <<<"$verify_output"; then
      echo "$verify_output" >&2
      echo "verify failed for scenario $scenario" >&2
      exit 1
    fi
    verify_lines+=("$verify_output")
  done

  echo "master_worker_autonomy_online_ok mock_attempts=$mock_count"
  echo "$sample_output"
  printf '%s\n' "${verify_lines[@]}"
}

run_master_worker_autonomy_online "$@"
