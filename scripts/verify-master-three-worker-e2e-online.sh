#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
source_runtime_home="${FREEHAND_RUNTIME_HOME:-"$HOME/.freehand"}"
isolated_home="${FREEHAND_THREE_WORKER_HOME:-"$(mktemp -d /tmp/freehand-three-worker-home.XXXXXX)"}"
runtime_home="$isolated_home/.freehand"
adp_url="${FREEHAND_THREE_WORKER_ADP_URL:-ws://127.0.0.1:4142/adp}"
health_url="${FREEHAND_THREE_WORKER_HEALTH_URL:-http://127.0.0.1:4142/health}"
cli_path="${FREEHAND_THREE_WORKER_CLI:-$HOME/.local/bin/freehand-cliS}"
daemon_path="${FREEHAND_THREE_WORKER_DAEMON:-$repo_root/target/debug/freehand-daemon}"
port="${FREEHAND_THREE_WORKER_FIXTURE_PORT:-18084}"
session_id="${FREEHAND_THREE_WORKER_SESSION:-online-master-three-worker-evaluation-$(date +%s)}"
fixture_key_name="FREEHAND_THREE_WORKER_FIXTURE_KEY"
fixture_key_value="test-three-worker-key"
config_path="$runtime_home/config.toml"
backup_dir="$runtime_home/tmp/three-worker-e2e-$(date +%Y%m%dT%H%M%S)-$$"
mock_log="$backup_dir/mock-provider.log"
target_cwd="$backup_dir/worker-target"
mock_pid=""
master_pid=""
worker_pid=""

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
  for service_pid in "$worker_pid" "$master_pid"; do
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

start_master() {
  env HOME="$isolated_home" \
    FREEHAND_PAIR_TOKEN_SHARED="$pair_token" \
    FREEHAND_CC_API_KEY="isolated-bootstrap-only" \
    "${fixture_key_name}=${fixture_key_value}" \
    "$daemon_path" serve --agent master --bind 127.0.0.1:4142 \
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
  env HOME="$isolated_home" \
    FREEHAND_PAIR_TOKEN_SHARED="$pair_token" \
    FREEHAND_CC_API_KEY="isolated-bootstrap-only" \
    "${fixture_key_name}=${fixture_key_value}" \
    "$daemon_path" serve --agent worker \
    >"$runtime_home/logs/worker.stdout.log" \
    2>"$runtime_home/logs/worker.stderr.log" &
  worker_pid="$!"
}

start_fixture() {
  node - "$port" >"$mock_log" 2>&1 <<'NODE' &
const http = require("http");
const port = Number(process.argv[2]);
const sessions = new Map();
const rejectedTasks = new Set();
const workerRuns = new Map();
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
  const parentSessionMatch = /"session_id"\s*:\s*"(online-master-three-worker-[^"]+)"/.exec(body);
  return {
    stamp,
    session: match(body, "FH3_SESSION") || parentSessionMatch?.[1] || "online-master-three-worker-e2e",
    targetCwd: match(body, "FH3_TARGET_CWD") || process.env.FH3_TARGET_CWD,
    worker: match(body, "FH3_WORKER") || "worker",
    tasks: ["alpha", "beta", "gamma"].map((name) => ({
      name,
      id: match(body, `FH3_TASK_${name.toUpperCase()}`) || `task-three-worker-${stamp}-${name}`,
      result: `worker_result_${name}=${stamp}`,
    })),
    integration: {
      name: "integration",
      id: match(body, "FH3_TASK_INTEGRATION") || `task-three-worker-${stamp}-integration`,
      result: `worker_result_integration=${stamp}`,
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

function masterResponse(body) {
  if (body.includes("production Master lifecycle coordinator")) {
    const taskMatch = /task-three-worker-[0-9]+-(?:alpha|beta|gamma|integration)/.exec(body);
    if (!taskMatch) {
      throw new Error("master lifecycle request missing three-worker task_id");
    }
    const taskId = taskMatch[0];
    if (taskId.endsWith("-beta") && !rejectedTasks.has(taskId)) {
      rejectedTasks.add(taskId);
      return toolUse(`fh3_lifecycle_reject_${taskId}`, {
        op: "reject",
        task_id: taskId,
        reject_reason: "beta draft does not satisfy integration-ready quality",
        next_requirements: [
          `resubmit exact worker_result_beta=${idsFromBody(body).stamp}`,
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
        agent_id: ids.worker,
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
    agent_id: ids.worker,
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
    const task = tasks[state.historyPolls % tasks.length];
    state.historyPolls += 1;
    if (state.historyPolls > 45) {
      throw new Error("worker reviews did not appear within fixture poll budget");
    }
    return historyTask(task);
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

  const summary = `Three worker E2E complete: ${tasks.map((task) => task.result).join("; ")}`;
  const evidence = tasks.map((task) => `${task.id}=approved_and_closed`).join("; ");
  return textResponse(completion(summary, evidence, "Master created three Worker tasks, reviewed all results, and returned one user-visible summary."));
}

const server = http.createServer((req, res) => {
  let body = "";
  req.on("data", chunk => { body += chunk; });
  req.on("end", () => {
    count += 1;
    try {
      const response = isWorkerRequest(body) ? workerResponse(body) : masterResponse(body);
      const ids = idsFromBody(body);
      console.log(JSON.stringify({
        count,
        url: req.url,
        role: isWorkerRequest(body) ? "worker" : "master",
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
  local prompt="$6"
  python3 - "$adp_url" "$session_id" "$task_alpha" "$task_beta" "$task_gamma" "$task_integration" "$prompt" <<'PY'
import asyncio
import json
import sys
import websockets

url, session_id, task_alpha, task_beta, task_gamma, task_integration, prompt = sys.argv[1:8]
task_ids = [task_alpha, task_beta, task_gamma, task_integration]

def adp_command(request_id, command):
    return {"kind": "command", "request_id": request_id, "command": command}

def adp_query(request_id, query):
    return {"kind": "query", "request_id": request_id, "query": query}

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
    async with websockets.connect(url) as ws:
        requests = [
            adp_query("turns", {"QuerySessionTurns": {"session_id": session_id}}),
            adp_query("tasks", {"QueryTaskBoard": {"include_terminal": True}}),
            adp_query("agents", {"QueryAgentBoard": {}}),
        ]
        for task_id in task_ids:
            requests.append(adp_query(f"history-{task_id}", {"QueryTaskHistory": {"task_id": task_id}}))
        for request in requests:
            await ws.send(json.dumps(request))
        return {request["request_id"]: await recv_until(ws, request["request_id"], 20) for request in requests}

async def main():
    async with websockets.connect(url) as ws:
        await ws.send(json.dumps(adp_command("three-worker-submit", {
            "SubmitUserInput": {"text": prompt, "session_id": session_id}
        })))
        receipt = await recv_until(ws, "three-worker-submit", 240)
    required_results = [
        "worker_result_alpha",
        "worker_result_beta",
        "worker_result_gamma",
        "worker_result_integration",
    ]
    deadline = asyncio.get_event_loop().time() + 240
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

    required_events = ["TaskCreated", "TaskAssigned", "TaskResumed", "TaskReviewSubmitted", "TaskReviewApproved", "TaskClosed"]
    histories = {}
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
    beta_events = histories[task_beta]
    if "TaskReviewRejected" not in beta_events:
        raise RuntimeError(f"beta task was never rejected for rework: {beta_events}")
    if beta_events.count("TaskReviewSubmitted") < 2:
        raise RuntimeError(f"beta task did not resubmit after rejection: {beta_events}")

    agents = responses["agents"].get("result", {}).get("AgentBoard", {}).get("agents", [])
    worker = next((agent for agent in agents if agent.get("agent_id") == "worker"), None)
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
                "last_event_seq": task.get("last_event_seq"),
                "events": histories[task.get("task_id")],
            }
            for task in matching_tasks
        ],
        "worker": None if worker is None else {
            "state": worker.get("state"),
            "current_task_id": worker.get("current_task_id"),
            "current_execution_id": worker.get("current_execution_id"),
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
import sys
import websockets

url, session_id, stamp = sys.argv[1:4]

async def main():
    async with websockets.connect(url) as ws:
        await ws.send(json.dumps({
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
  if [[ ! -f "$source_runtime_home/config.toml" || ! -f "$source_runtime_home/daemonS.env" ]]; then
    echo "missing source S config/env under $source_runtime_home" >&2
    exit 2
  fi

  mkdir -p "$backup_dir" "$target_cwd" "$runtime_home/logs"
  cp "$source_runtime_home/config.toml" "$config_path"
  pair_token="$(awk -F= '$1 == "FREEHAND_PAIR_TOKEN_SHARED" { gsub(/^"/, "", $2); gsub(/"$/, "", $2); print $2; exit }' "$source_runtime_home/daemonS.env")"
  if [[ -z "$pair_token" ]]; then
    echo "source S env has no FREEHAND_PAIR_TOKEN_SHARED" >&2
    exit 2
  fi
  export FH3_TARGET_CWD="$target_cwd"
  printf 'fixture workspace\n' >"$target_cwd/README.txt"

  local stamp task_alpha task_beta task_gamma task_integration prompt proof restart_proof
  stamp="178$(date +%s)"
  task_alpha="task-three-worker-$stamp-alpha"
  task_beta="task-three-worker-$stamp-beta"
  task_gamma="task-three-worker-$stamp-gamma"
  task_integration="task-three-worker-$stamp-integration"
  prompt=$'Three worker iterative goal proof.\n'"FH3_STAMP=$stamp"$'\n'"FH3_SESSION=$session_id"$'\n'"FH3_WORKER=worker"$'\n'"FH3_TARGET_CWD=$target_cwd"$'\n'"FH3_TASK_ALPHA=$task_alpha"$'\n'"FH3_TASK_BETA=$task_beta"$'\n'"FH3_TASK_GAMMA=$task_gamma"$'\n'"FH3_TASK_INTEGRATION=$task_integration"$'\nOverall goal: produce verified alpha, beta, and gamma results, reject and redo any result that is not integration-ready, then evaluate whether additional integration work is required. Do not finish merely by summarizing the first three results. Create exactly three initial Worker tasks for alpha, beta, and gamma and assign them to worker. The parent evaluation must create the integration task if the first accepted set still leaves the overall integration goal incomplete. Final completion requires all four exact worker_result_* tokens.'

  start_fixture

  start_master
  "$cli_path" adp-config-update \
    --url "$adp_url" \
    --agent master \
    --provider minimax \
    --type anthropic \
    --protocol messages \
    --base-url "http://127.0.0.1:$port" \
    --model MiniMax-M3 \
    --api-key-env "$fixture_key_name" >/dev/null
  "$cli_path" adp-config-update \
    --url "$adp_url" \
    --agent worker \
    --provider minimax \
    --type anthropic \
    --protocol messages \
    --base-url "http://127.0.0.1:$port" \
    --model MiniMax-M3 \
    --api-key-env "$fixture_key_name" >/dev/null
  stop_master
  start_master
  start_worker

  "$cli_path" adp-session-manage --url "$adp_url" --action delete --session "$session_id" >/dev/null 2>&1 || true
  "$cli_path" adp-session-manage --url "$adp_url" --action create --session "$session_id" --title "Three worker E2E" --cwd "$repo_root" >/dev/null

  proof="$(run_adp_submit_and_verify "$stamp" "$task_alpha" "$task_beta" "$task_gamma" "$task_integration" "$prompt")"
  stop_master
  start_master
  restart_proof="$(verify_restart_idempotency "$stamp")"
  echo "master_three_worker_e2e_ok url=$adp_url session=$session_id initial_tasks=$task_alpha,$task_beta,$task_gamma next_task=$task_integration"
  echo "$proof"
  echo "$restart_proof"
}

run_three_worker_e2e "$@"
