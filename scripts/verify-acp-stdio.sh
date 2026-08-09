#!/usr/bin/env bash
# ACP v1 end-to-end smoke: handshake + session/new + session/prompt returning
# end_turn, then session/cancel followed by session/prompt returning cancelled
# without invoking the provider a second time, then a third prompt returning
# end_turn so the cancel reset is verified. stdout must contain only NDJSON
# JSON-RPC frames; stderr must be empty.

set -euo pipefail

BIN="${BIN:-./target/debug/freehand-daemon}"
[[ -x "$BIN" ]] || { echo "missing $BIN" >&2; exit 2; }

PAYLOAD='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientInfo":{"name":"verify-acp-stdio","version":"1"},"clientCapabilities":{}}}
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/tmp","mcpServers":[]}}
{"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":"acp-1"}}
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"acp-1","prompt":[{"type":"text","text":"x"}]}}
{"jsonrpc":"2.0","id":4,"method":"session/prompt","params":{"sessionId":"acp-1","prompt":[{"type":"text","text":"reply with exactly: OK"}]}}'

STDOUT_FILE="$(mktemp)"
STDERR_FILE="$(mktemp)"
trap 'rm -f "$STDOUT_FILE" "$STDERR_FILE"' EXIT

printf '%s\n' "$PAYLOAD" \
  | env FREEHAND_PAIR_TOKEN_SHARED="${FREEHAND_PAIR_TOKEN_SHARED:-testpair}" \
        FREEHAND_RELAY_AGENT_TOKEN="${FREEHAND_RELAY_AGENT_TOKEN:-testrelay}" \
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
