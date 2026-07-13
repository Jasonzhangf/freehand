#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
runtime_home="${FREEHAND_RUNTIME_HOME:-"$HOME/.freehand"}"
adp_url="${FREEHAND_PROVIDER_RETRY_ADP_URL:-ws://127.0.0.1:4042/adp}"
health_url="${FREEHAND_PROVIDER_RETRY_HEALTH_URL:-http://127.0.0.1:4042/health}"
cli_path="${FREEHAND_PROVIDER_RETRY_CLI:-$HOME/.local/bin/freehand-cliS}"
port="${FREEHAND_PROVIDER_RETRY_FIXTURE_PORT:-18081}"
fixture_key_name="FREEHAND_PROVIDER_RETRY_FIXTURE_KEY"
fixture_key_value="test-provider-retry-key"
fixture_backoff_name="FREEHAND_PROVIDER_RETRY_BACKOFF_MS"
config_path="$runtime_home/config.toml"
env_path="$runtime_home/daemonS.env"
backup_dir="$runtime_home/tmp/provider-retry-online-$(date +%Y%m%dT%H%M%S)-$$"
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

run_verify_provider_retry_online() {
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
let count = 0;
const server = http.createServer((req, res) => {
  let body = "";
  req.on("data", chunk => { body += chunk; });
  req.on("end", () => {
    count += 1;
    console.log(JSON.stringify({ count, method: req.method, url: req.url, bodyLength: body.length }));
    res.writeHead(500, { "content-type": "application/json" });
    res.end(JSON.stringify({
      type: "error",
      error: { type: "api_error", message: `fixture upstream failure ${count}` },
    }));
  });
});
server.on("error", err => {
  console.error(`mock-provider-error ${err.message}`);
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

  printf '\n%s="%s"\n%s="0"\n' \
    "$fixture_key_name" "$fixture_key_value" \
    "$fixture_backoff_name" >>"$env_path"
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

  sample_output="$("$cli_path" adp-turn-sample --url "$adp_url" --sample provider-retry)"
  session_id="$(printf '%s\n' "$sample_output" | sed -n 's/.* session=\([^ ]*\) .*/\1/p')"
  if [[ -z "$session_id" ]]; then
    echo "$sample_output" >&2
    echo "provider retry sample did not report session id" >&2
    exit 1
  fi

  error_output="$("$cli_path" adp-error-query --url "$adp_url" --session "$session_id" --domain provider)"
  session_output="$("$cli_path" adp-session-query --url "$adp_url" --session "$session_id")"
  session_status="$(python3 - "$adp_url" "$session_id" <<'PY'
import asyncio
import json
import sys

import websockets


async def main():
    url = sys.argv[1]
    session_id = sys.argv[2]
    async with websockets.connect(url) as ws:
        await ws.send(json.dumps({
            "kind": "query",
            "request_id": "provider-retry-session-truth",
            "query": {"QuerySessionTurns": {"session_id": session_id}},
        }))
        while True:
            msg = json.loads(await ws.recv())
            if msg.get("request_id") != "provider-retry-session-truth":
                continue
            turns = (
                msg.get("result", {})
                .get("SessionTurns", {})
                .get("turns", [])
            )
            if not turns:
                print("missing")
                return
            print(turns[-1].get("terminal_status") or "non_terminal")
            return


asyncio.run(main())
PY
)"
  mock_count="$(grep -c '"url":"/v1/messages"' "$mock_log" || true)"

  if [[ "$mock_count" != "10" ]]; then
    cat "$mock_log" >&2
    echo "expected 10 provider attempts, got $mock_count" >&2
    exit 1
  fi
  if ! grep -q "provider_retries=1" <<<"$sample_output"; then
    echo "$sample_output" >&2
    echo "provider retry sample did not report provider retry evidence" >&2
    exit 1
  fi
  if ! grep -q "count=10" <<<"$error_output"; then
    echo "$error_output" >&2
    echo "error-center query did not return ten provider rows" >&2
    exit 1
  fi
  if ! grep -q "retry_same_step" <<<"$error_output" || ! grep -q "fail_turn" <<<"$error_output"; then
    echo "$error_output" >&2
    echo "error-center query missing retry/fail recovery actions" >&2
    exit 1
  fi
  if ! grep -q "anthropic_http_status_500" <<<"$error_output"; then
    echo "$error_output" >&2
    echo "error-center query missing provider HTTP status code" >&2
    exit 1
  fi
  if ! grep -q "selected_session=$session_id turns=1" <<<"$session_output"; then
    echo "$session_output" >&2
    echo "session truth did not report one provider retry turn" >&2
    exit 1
  fi
  if [[ "$session_status" != "Failed" ]]; then
    echo "$session_output" >&2
    echo "expected provider retry session terminal_status=Failed, got $session_status" >&2
    exit 1
  fi

  echo "provider_retry_online_ok session=$session_id mock_attempts=$mock_count session_status=$session_status"
  echo "$sample_output"
  echo "$error_output"
}

run_verify_provider_retry_online "$@"
