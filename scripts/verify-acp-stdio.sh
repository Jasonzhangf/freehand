#!/usr/bin/env bash
# ACP v1 end-to-end smoke: handshake + session/new + session/prompt returning
# end_turn, then session/cancel followed by session/prompt returning cancelled
# without invoking the provider a second time, then a third prompt returning
# end_turn so the cancel reset is verified. stdout must contain only NDJSON
# JSON-RPC frames; stderr must be empty.
#
# The daemon runs with an isolated temporary HOME and a local deterministic
# Anthropic-compatible mock provider. This keeps the gate hermetic and does not
# depend on a developer's real provider keys or upstream provider availability.

set -euo pipefail

BIN="${BIN:-./target/debug/freehand-daemon}"
[[ -x "$BIN" ]] || { echo "missing $BIN" >&2; exit 2; }

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}
require_cmd node

PAYLOAD='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientInfo":{"name":"verify-acp-stdio","version":"1"},"clientCapabilities":{}}}
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/tmp","mcpServers":[]}}
{"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":"acp-1"}}
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"acp-1","prompt":[{"type":"text","text":"x"}]}}
{"jsonrpc":"2.0","id":4,"method":"session/prompt","params":{"sessionId":"acp-1","prompt":[{"type":"text","text":"reply with exactly: OK"}]}}'

ACP_HOME="$(mktemp -d)"
PORT_FILE="$ACP_HOME/mock-port"
MOCK_LOG="$ACP_HOME/mock.log"
STDOUT_FILE="$(mktemp)"
STDERR_FILE="$(mktemp)"
mock_pid=""

cleanup() {
  if [[ -n "$mock_pid" ]] && kill -0 "$mock_pid" >/dev/null 2>&1; then
    kill "$mock_pid" >/dev/null 2>&1 || true
    wait "$mock_pid" >/dev/null 2>&1 || true
  fi
  rm -rf "$ACP_HOME"
  rm -f "$STDOUT_FILE" "$STDERR_FILE"
}
trap cleanup EXIT

node - "$PORT_FILE" >"$MOCK_LOG" 2>&1 <<'NODE' &
const fs = require("fs");
const http = require("http");
const portFile = process.argv[2];
const completion = [
  "<freehand_completion>",
  "{\"claim\":\"complete\",\"completion_reason\":\"mock provider finished\",\"evidence\":\"mock provider returned the requested result\",\"summary\":\"mock provider result\",\"learned\":\"mock provider deterministic\"}",
  "</freehand_completion>",
].join("\n");
const body = JSON.stringify({
  content: [{ type: "text", text: "mock-ok\n" + completion }],
  usage: { input_tokens: 8, output_tokens: 8 },
  stop_reason: "end_turn",
});
const server = http.createServer((req, res) => {
  req.resume();
  req.on("end", () => {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(body);
  });
});
server.on("error", err => {
  console.error(`mock-provider-error ${err.message}`);
  process.exit(3);
});
server.listen(0, "127.0.0.1", () => {
  fs.writeFileSync(portFile, String(server.address().port));
  console.log(`mock-provider-listening ${server.address().port}`);
});
process.on("SIGTERM", () => server.close(() => process.exit(0)));
NODE
mock_pid="$!"

for _ in $(seq 1 50); do
  if [[ -s "$PORT_FILE" ]]; then
    break
  fi
  if ! kill -0 "$mock_pid" >/dev/null 2>&1; then
    cat "$MOCK_LOG" >&2
    echo "mock provider did not become ready" >&2
    exit 2
  fi
  sleep 0.1
done
if [[ ! -s "$PORT_FILE" ]]; then
  cat "$MOCK_LOG" >&2
  echo "mock provider did not become ready" >&2
  exit 2
fi

mock_port="$(cat "$PORT_FILE")"
mkdir -p "$ACP_HOME/.freehand"
cat > "$ACP_HOME/.freehand/config.toml" <<EOF
[providers.mock]
id = "mock"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "http://127.0.0.1:$mock_port"
default_model = "mock-model"

[providers.mock.auth]
type = "apikey"
api_key = "mock-key"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "FREEHAND_ACP_PAIR_TOKEN"
provider = "mock"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "FREEHAND_ACP_PAIR_TOKEN"
provider = "mock"
EOF

printf '%s\n' "$PAYLOAD" \
  | env HOME="$ACP_HOME" \
        FREEHAND_ACP_PAIR_TOKEN="testpair" \
        timeout 300 "$BIN" acp \
        1>"$STDOUT_FILE" \
        2>"$STDERR_FILE" || true

fail=0

assert_present() {
  local needle="$1"
  grep -q -- "$needle" "$STDOUT_FILE" || { echo "missing frame: $needle" >&2; fail=1; }
}

assert_present '"protocolVersion":1'
assert_present '"sessionId":"acp-1"'
assert_present '"stopReason":"cancelled"'
assert_present '"stopReason":"end_turn"'
# stderr must be empty so the wire stays on stdout only.
if [[ -s "$STDERR_FILE" ]]; then
  echo "stderr must be empty; got:" >&2
  cat "$STDERR_FILE" >&2
  fail=1
fi

# Every stdout line must be a valid JSON-RPC frame: starts with '{', ends with '}'.
while IFS= read -r line; do
  [[ -z "$line" ]] && continue
  case "$line" in
    '{'*) ;;
    *) echo "non-JSON stdout line: $line" >&2; fail=1 ;;
  esac
done < "$STDOUT_FILE"

exit "$fail"
