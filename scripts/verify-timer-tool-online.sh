#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
runtime_home="${FREEHAND_RUNTIME_HOME:-"$HOME/.freehand"}"
adp_url="${FREEHAND_TIMER_VERIFY_ADP_URL:-ws://127.0.0.1:4042/adp}"
health_url="${FREEHAND_TIMER_VERIFY_HEALTH_URL:-http://127.0.0.1:4042/health}"
cli_path="${FREEHAND_TIMER_VERIFY_CLI:-$HOME/.local/bin/freehand-cliS}"
port="${FREEHAND_TIMER_VERIFY_FIXTURE_PORT:-18086}"
fixture_key_name="FREEHAND_TIMER_VERIFY_FIXTURE_KEY"
fixture_key_value="test-timer-verify-key"
fixture_provider_id="${FREEHAND_TIMER_VERIFY_PROVIDER:-timer-fixture}"
verify_mode="${FREEHAND_TIMER_VERIFY_MODE:-due}"
config_path="$runtime_home/config.toml"
env_path="$runtime_home/daemonS.env"
stamp="$(date +%s)-$$"
timer_id="timer-online-proof-$stamp"
backup_dir="$runtime_home/tmp/timer-tool-online-$(date +%Y%m%dT%H%M%S)-$$"
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

run_timer_tool_online() {
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

  if [[ "$verify_mode" != "due" && "$verify_mode" != "restart-due" ]]; then
    echo "unsupported FREEHAND_TIMER_VERIFY_MODE: $verify_mode" >&2
    exit 2
  fi

  TIMER_ID="$timer_id" node - "$port" >"$mock_log" 2>&1 <<'NODE' &
const http = require("http");
const port = Number(process.argv[2]);
const timerId = process.env.TIMER_ID;
let count = 0;

function complete(text) {
  const schema = {
    claim: "complete",
    completion_reason: "timer tool online proof complete",
    evidence: "model called timer, received scheduled tool result, and timer due wakeup fired",
    summary: text,
    learned: "timer is an independent internal tool"
  };
  return {
    content: [{
      type: "text",
      text: `${text}\n<freehand_completion>\n${JSON.stringify(schema)}\n</freehand_completion>`
    }],
    usage: { input_tokens: 80, output_tokens: 80 },
    stop_reason: "end_turn"
  };
}

const server = http.createServer((req, res) => {
  let body = "";
  req.on("data", chunk => { body += chunk; });
  req.on("end", () => {
    count += 1;
    const sawToolResult = body.includes("Timer scheduled") && body.includes(timerId);
    const sawTimerWakeup = body.includes("new follow-up turn injected by a due timer")
      && body.includes("scheduled Master timer proof wakeup")
      && body.includes(timerId);
    console.log(JSON.stringify({
      count,
      method: req.method,
      url: req.url,
      hasTimerTool: body.includes('"name":"timer"') || body.includes('"name": "timer"'),
      sawToolResult,
      sawTimerWakeup
    }));
    let response;
    if (count === 1) {
      response = {
        content: [{
          type: "tool_use",
          id: "toolu_timer_online_proof",
          name: "timer",
          input: {
            op: "schedule",
            timer_id: timerId,
            mode: "relative",
            delay_seconds: 3,
            reason: "Online proof that the Master model can call the independent timer tool.",
            prompt: "This is the scheduled Master timer proof wakeup. Read current framework truth before taking any action."
          }
        }],
        usage: { input_tokens: 50, output_tokens: 30 },
        stop_reason: "tool_use"
      };
    } else if (sawToolResult) {
      response = complete("timer online proof completed after receiving scheduled tool result");
    } else if (sawTimerWakeup) {
      response = complete("timer due wakeup injected a new prompt turn into the source session");
    } else {
      res.writeHead(500, { "content-type": "application/json" });
      res.end(JSON.stringify({ type: "error", error: { type: "fixture_error", message: "provider request did not match timer proof sequence" } }));
      return;
    }
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(response));
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
    --provider "$fixture_provider_id" \
    --type anthropic \
    --protocol messages \
    --base-url "http://127.0.0.1:$port" \
    --model MiniMax-M3 \
    --api-key-env "$fixture_key_name" >/dev/null
  scripts/install-launchd.sh restartS >/dev/null
  curl -4fsS "$health_url" >/dev/null
  "$cli_path" adp-smoke --url "$adp_url" >/dev/null

  set +e
  sample_output="$("$cli_path" adp-turn-sample --url "$adp_url" --sample success 2>&1)"
  sample_status="$?"
  set -e
  if [[ "$sample_status" != "0" ]] && ! grep -q "reason_live_turn_completed" <<<"$sample_output"; then
    echo "$sample_output" >&2
    echo "timer online ADP sample failed before live turn completion" >&2
    exit 1
  fi
  if ! grep -q "tool_executions=1" <<<"$sample_output"; then
    echo "$sample_output" >&2
    echo "timer online sample did not record exactly one tool execution" >&2
    exit 1
  fi
  session_id="$(printf '%s\n' "$sample_output" | sed -n 's/.* session=\([^ ]*\) .*/\1/p')"
  if [[ -z "$session_id" ]]; then
    echo "$sample_output" >&2
    echo "timer online sample did not report source session id" >&2
    exit 1
  fi

  mock_count="$(grep -c '"url":"/v1/messages"' "$mock_log" || true)"
  if [[ "$mock_count" -lt "2" ]]; then
    cat "$mock_log" >&2
    echo "expected at least 2 provider attempts before timer due, got $mock_count" >&2
    exit 1
  fi
  if ! grep -q '"sawToolResult":true' "$mock_log"; then
    cat "$mock_log" >&2
    echo "mock provider did not observe scheduled timer tool result" >&2
    exit 1
  fi

  state_path="$runtime_home/state/timers/master.json"
  ledger_path="$runtime_home/ledgers/timers/master.jsonl"
  if [[ ! -f "$state_path" ]]; then
    echo "missing timer state path: $state_path" >&2
    exit 1
  fi
  if [[ ! -f "$ledger_path" ]]; then
    echo "missing timer ledger path: $ledger_path" >&2
    exit 1
  fi
  node - "$state_path" "$ledger_path" "$timer_id" "$session_id" <<'NODE'
const fs = require("fs");
const [statePath, ledgerPath, timerId, sessionId] = process.argv.slice(2);
const schedules = JSON.parse(fs.readFileSync(statePath, "utf8"));
const schedule = schedules.find(item => item.timer_id === timerId);
if (!schedule) throw new Error(`missing schedule ${timerId}`);
if (schedule.status !== "active") throw new Error(`unexpected status ${schedule.status}`);
if (schedule.source_session_id !== sessionId) {
  throw new Error(`timer source session drift before due: ${schedule.source_session_id} !== ${sessionId}`);
}
if (schedule.reason !== "Online proof that the Master model can call the independent timer tool.") {
  throw new Error(`unexpected reason ${schedule.reason}`);
}
if (!schedule.prompt.includes("scheduled Master timer proof wakeup")) {
  throw new Error("persisted prompt missing proof text");
}
const ledgerLines = fs.readFileSync(ledgerPath, "utf8").trim().split(/\n+/).filter(Boolean);
const scheduled = ledgerLines
  .map(line => JSON.parse(line))
  .find(event => event.timer_id === timerId && event.event_type === "TimerScheduled");
if (!scheduled) throw new Error(`missing TimerScheduled ledger event for ${timerId}`);
console.log(`timer_state_verified timer_id=${timerId} status=${schedule.status} next_due_at=${schedule.next_due_at}`);
NODE

  if [[ "$verify_mode" == "restart-due" ]]; then
    scripts/install-launchd.sh restartS >/dev/null
    curl -4fsS "$health_url" >/dev/null
  fi

  due_verified=""
  for _ in $(seq 1 45); do
    if grep -q '"sawTimerWakeup":true' "$mock_log" && node - "$state_path" "$ledger_path" "$timer_id" <<'NODE' >/tmp/freehand-timer-due-check.$$ 2>/tmp/freehand-timer-due-check.err.$$
const fs = require("fs");
const [statePath, ledgerPath, timerId] = process.argv.slice(2);
const schedules = JSON.parse(fs.readFileSync(statePath, "utf8"));
const schedule = schedules.find(item => item.timer_id === timerId);
if (!schedule) throw new Error(`missing schedule ${timerId}`);
if (schedule.status !== "completed") throw new Error(`not completed: ${schedule.status}`);
if (schedule.fired_count !== 1) throw new Error(`unexpected fired_count ${schedule.fired_count}`);
const events = fs.readFileSync(ledgerPath, "utf8").trim().split(/\n+/).filter(Boolean).map(line => JSON.parse(line));
if (!events.some(event => event.timer_id === timerId && event.event_type === "TimerFired")) {
  throw new Error("missing TimerFired");
}
if (!events.some(event => event.timer_id === timerId && event.event_type === "TimerCompleted")) {
  throw new Error("missing TimerCompleted");
}
console.log(`timer_due_verified timer_id=${timerId} status=${schedule.status} fired_count=${schedule.fired_count}`);
NODE
    then
      due_verified="$(cat /tmp/freehand-timer-due-check.$$)"
      rm -f /tmp/freehand-timer-due-check.$$ /tmp/freehand-timer-due-check.err.$$
      break
    fi
    rm -f /tmp/freehand-timer-due-check.$$ /tmp/freehand-timer-due-check.err.$$
    sleep 1
  done
  if [[ -z "$due_verified" ]]; then
    cat "$mock_log" >&2
    echo "timer due wakeup did not fire and complete" >&2
    exit 1
  fi
  session_output="$("$cli_path" adp-session-query --url "$adp_url" --session "$session_id")"
  session_turns="$(printf '%s\n' "$session_output" | sed -n 's/.* selected_session=[^ ]* turns=\([0-9][0-9]*\).*/\1/p')"
  if [[ -z "$session_turns" || "$session_turns" -lt 2 ]]; then
    echo "$session_output" >&2
    echo "timer due wakeup did not inject a visible follow-up turn into the original user session" >&2
    exit 1
  fi

  mock_count="$(grep -c '"url":"/v1/messages"' "$mock_log" || true)"
  if [[ "$mock_count" -lt "3" ]]; then
    cat "$mock_log" >&2
    echo "expected at least 3 provider attempts including timer wakeup, got $mock_count" >&2
    exit 1
  fi

  if [[ "$verify_mode" == "restart-due" ]]; then
    echo "timer_restart_due_online_ok url=$adp_url session=$session_id timer_id=$timer_id session_turns=$session_turns mock_attempts=$mock_count"
  else
    echo "timer_tool_online_ok url=$adp_url session=$session_id timer_id=$timer_id session_turns=$session_turns mock_attempts=$mock_count"
  fi
  echo "$due_verified"
  echo "$sample_output"
  grep '"sawToolResult":true' "$mock_log" | tail -1
  grep '"sawTimerWakeup":true' "$mock_log" | tail -1
}

run_timer_tool_online "$@"
