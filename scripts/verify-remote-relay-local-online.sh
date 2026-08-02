#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/freehand-remote-relay-online.XXXXXX")"
UPSTREAM_PID=""
RELAY_PID=""
AGENT_PID=""

stop_pid() {
  local pid="$1"
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid"
    wait "$pid" 2>/dev/null || true
  fi
}

cleanup() {
  stop_pid "$RELAY_PID"
  stop_pid "$UPSTREAM_PID"
  stop_pid "$AGENT_PID"
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

free_port() {
  ruby -rsocket -e 'server = TCPServer.new("127.0.0.1", 0); puts server.addr[1]; server.close'
}

wait_health() {
  local url="$1"
  local pid="$2"
  local label="$3"
  for _ in $(seq 1 300); do
    if [[ "$(curl -fsS "$url" 2>/dev/null || true)" == "ok" ]]; then
      return 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "$label process exited before health admission: pid=$pid url=$url" >&2
      sed -n '1,120p' "$TMP_DIR/$label.stdout.log" >&2 || true
      sed -n '1,120p' "$TMP_DIR/$label.stderr.log" >&2 || true
      return 1
    fi
    sleep 0.1
  done
  echo "health check failed: $url" >&2
  sed -n '1,120p' "$TMP_DIR/upstream.stdout.log" >&2 || true
  sed -n '1,120p' "$TMP_DIR/upstream.stderr.log" >&2 || true
  sed -n '1,120p' "$TMP_DIR/relay.stdout.log" >&2 || true
  sed -n '1,120p' "$TMP_DIR/relay.stderr.log" >&2 || true
  return 1
}

UPSTREAM_PORT="$(free_port)"
RELAY_PORT="$(free_port)"
while [[ "$RELAY_PORT" == "$UPSTREAM_PORT" ]]; do
  RELAY_PORT="$(free_port)"
done
UPSTREAM_URL="http://127.0.0.1:$UPSTREAM_PORT"
RELAY_URL="http://127.0.0.1:$RELAY_PORT"
STORE_PATH="$TMP_DIR/relay-store.json"
COOKIE_JAR="$TMP_DIR/cookies.txt"

cargo build -p freehand-server -p freehand-daemon -p freehand-relay-server --manifest-path "$ROOT_DIR/Cargo.toml"

FREEHAND_RELAY_STORE="$STORE_PATH" \
FREEHAND_RELAY_PRESENCE_LEASE_SECONDS=45 \
  "$TARGET_DIR/debug/freehand-relay-server" init-store

FREEHAND_ADP_AUTH_TOKEN=upstream-token "$TARGET_DIR/debug/freehand-server" webui-serve-smoke --bind "127.0.0.1:$UPSTREAM_PORT" \
  >"$TMP_DIR/upstream.stdout.log" 2>"$TMP_DIR/upstream.stderr.log" &
UPSTREAM_PID=$!

wait_health "$UPSTREAM_URL/health" "$UPSTREAM_PID" upstream

FREEHAND_RELAY_BIND="127.0.0.1:$RELAY_PORT" \
FREEHAND_RELAY_STORE="$STORE_PATH" \
FREEHAND_RELAY_PRESENCE_LEASE_SECONDS=45 \
FREEHAND_RELAY_SECURE_COOKIE=false \
  "$TARGET_DIR/debug/freehand-daemon" remote-relay \
  >"$TMP_DIR/relay.stdout.log" 2>"$TMP_DIR/relay.stderr.log" &
RELAY_PID=$!

wait_health "$RELAY_URL/relay/health" "$RELAY_PID" relay

REGISTER_JSON="$(curl -fsS -X POST "$RELAY_URL/relay/api/auth/register" \
  -H 'content-type: application/json' \
  --data '{"username":"local-online","password":"relay-password-123"}')"
TOKEN="$(jq -er '.accessToken' <<<"$REGISTER_JSON")"
ACCOUNT_ID="$(jq -er '.accountId' <<<"$REGISTER_JSON")"

FREEHAND_RELAY_AGENT_URL="$RELAY_URL" \
FREEHAND_RELAY_AGENT_TOKEN="$TOKEN" \
FREEHAND_RELAY_AGENT_ID="studio" \
FREEHAND_RELAY_AGENT_DISPLAY_NAME="Studio Master" \
FREEHAND_RELAY_AGENT_NODE_ID="node-studio" \
FREEHAND_RELAY_AGENT_ROLE="master" \
FREEHAND_RELAY_AGENT_STATUS="running" \
FREEHAND_RELAY_AGENT_ACTIVE_SESSION_COUNT=1 \
FREEHAND_RELAY_AGENT_LOCAL_ADDR="127.0.0.1:$UPSTREAM_PORT" \
FREEHAND_RELAY_AGENT_LOCAL_ADP_TOKEN=upstream-token \
  "$TARGET_DIR/debug/freehand-relay-server" agent-tunnel \
  >"$TMP_DIR/agent.stdout.log" 2>"$TMP_DIR/agent.stderr.log" &
AGENT_PID=$!
for _ in $(seq 1 80); do
  if curl -fsS "$RELAY_URL/relay/api/agents" -H "authorization: Bearer $TOKEN" \
      | jq -e '.agents[0].agentId == "studio" and .agents[0].online == true' >/dev/null; then
    break
  fi
  sleep 0.1
done

curl -fsS "$RELAY_URL/relay/api/agents" \
  -H "authorization: Bearer $TOKEN" \
  | jq -e --arg account "$ACCOUNT_ID" '.accountId == $account and .agents[0].agentId == "studio" and .agents[0].role == "master" and .agents[0].status == "running"' >/dev/null

curl -fsS -c "$COOKIE_JAR" \
  -H "authorization: Bearer $TOKEN" \
  "$RELAY_URL/relay/agents/studio/?client=android-webview" \
  >"$TMP_DIR/relay-root.html"
grep -F 'data-webui-shell="true"' "$TMP_DIR/relay-root.html" >/dev/null
grep -F 'data-adp-endpoint="/adp"' "$TMP_DIR/relay-root.html" >/dev/null

curl -fsS -b "$COOKIE_JAR" \
  -H "authorization: Bearer $TOKEN" \
  "$RELAY_URL/relay/agents/studio/ui/query/latest-active-turn" \
  | grep -F '"turn_id":"turn-webui-smoke"' >/dev/null

SECOND_JSON="$(curl -fsS -X POST "$RELAY_URL/relay/api/auth/register" \
  -H 'content-type: application/json' \
  --data '{"username":"second-account","password":"relay-password-456"}')"
SECOND_TOKEN="$(jq -er '.accessToken' <<<"$SECOND_JSON")"
CROSS_STATUS="$(curl -sS -o "$TMP_DIR/cross.json" -w '%{http_code}' \
  -H "authorization: Bearer $SECOND_TOKEN" \
  "$RELAY_URL/relay/agents/studio/health")"
if [[ "$CROSS_STATUS" != "404" ]]; then
  echo "expected cross-account Agent route 404, got $CROSS_STATUS" >&2
  exit 1
fi
jq -e '.code == "relay_agent_not_found"' "$TMP_DIR/cross.json" >/dev/null

echo "remote_relay_local_online_ok upstream=$UPSTREAM_URL relay=$RELAY_URL agent=studio outbound_tunnel=verified"
