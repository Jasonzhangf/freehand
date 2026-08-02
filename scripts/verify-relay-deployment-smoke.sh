#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/freehand-relay-deploy.XXXXXX")"
RELAY_PID=""
UPSTREAM_PID=""
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
  for _ in $(seq 1 80); do
    if [[ "$(curl -fsS "$url" 2>/dev/null || true)" == "ok" ]]; then
      return 0
    fi
    sleep 0.1
  done
  echo "relay health did not become ready: $url" >&2
  return 1
}

RELAY_PORT="$(free_port)"
UPSTREAM_PORT="$(free_port)"
STORE_PATH="$TMP_DIR/store.json"
mkdir -p "$TMP_DIR/upstream"
printf '%s\n' '<script src="/assets/app.js"></script><script>new WebSocket("/adp")</script>' > "$TMP_DIR/upstream/index.html"
mkdir -p "$TMP_DIR/upstream/assets"
printf '%s\n' 'window.freehandRelayDeploySmoke = true;' > "$TMP_DIR/upstream/assets/app.js"

cargo build -p freehand-relay-server --manifest-path "$ROOT_DIR/Cargo.toml"
if FREEHAND_RELAY_BIND="127.0.0.1:$RELAY_PORT" \
  FREEHAND_RELAY_STORE="$STORE_PATH" \
  FREEHAND_RELAY_PRESENCE_LEASE_SECONDS=45 \
  env -u FREEHAND_RELAY_SECURE_COOKIE \
  "$TARGET_DIR/debug/freehand-relay-server" serve >"$TMP_DIR/missing-cookie-mode.log" 2>&1; then
  echo "Relay unexpectedly started without FREEHAND_RELAY_SECURE_COOKIE" >&2
  exit 1
fi
grep -q 'FREEHAND_RELAY_SECURE_COOKIE is required' "$TMP_DIR/missing-cookie-mode.log"
FREEHAND_RELAY_STORE="$STORE_PATH" \
FREEHAND_RELAY_PRESENCE_LEASE_SECONDS=45 \
  "$TARGET_DIR/debug/freehand-relay-server" init-store
ruby -run -e httpd "$TMP_DIR/upstream" -b 127.0.0.1 -p "$UPSTREAM_PORT" >"$TMP_DIR/upstream.log" 2>&1 &
UPSTREAM_PID=$!

start_relay() {
  FREEHAND_RELAY_BIND="127.0.0.1:$RELAY_PORT" \
  FREEHAND_RELAY_STORE="$STORE_PATH" \
  FREEHAND_RELAY_PRESENCE_LEASE_SECONDS=45 \
  FREEHAND_RELAY_SECURE_COOKIE=false \
    "$TARGET_DIR/debug/freehand-relay-server" serve >"$TMP_DIR/relay.log" 2>&1 &
  RELAY_PID=$!
  wait_health "http://127.0.0.1:$RELAY_PORT/relay/health"
}

start_relay
REGISTER_JSON="$(curl -fsS -X POST "http://127.0.0.1:$RELAY_PORT/relay/api/auth/register" \
  -H 'content-type: application/json' \
  --data '{"username":"deploy-smoke","password":"relay-password-123"}')"
TOKEN="$(jq -er '.accessToken' <<<"$REGISTER_JSON")"
ACCOUNT_ID="$(jq -er '.accountId' <<<"$REGISTER_JSON")"

FREEHAND_RELAY_AGENT_URL="http://127.0.0.1:$RELAY_PORT" \
FREEHAND_RELAY_AGENT_TOKEN="$TOKEN" \
FREEHAND_RELAY_AGENT_ID="studio" \
FREEHAND_RELAY_AGENT_DISPLAY_NAME="Studio Master" \
FREEHAND_RELAY_AGENT_NODE_ID="node-studio" \
FREEHAND_RELAY_AGENT_ROLE="master" \
FREEHAND_RELAY_AGENT_STATUS="running" \
FREEHAND_RELAY_AGENT_ACTIVE_SESSION_COUNT=2 \
FREEHAND_RELAY_AGENT_LOCAL_ADDR="127.0.0.1:$UPSTREAM_PORT" \
  "$TARGET_DIR/debug/freehand-relay-server" agent-tunnel >"$TMP_DIR/agent.log" 2>&1 &
AGENT_PID=$!
for _ in $(seq 1 80); do
  if curl -fsS "http://127.0.0.1:$RELAY_PORT/relay/api/agents" -H "authorization: Bearer $TOKEN" \
      | jq -e '.agents[0].agentId == "studio" and .agents[0].online == true' >/dev/null; then
    break
  fi
  sleep 0.1
done

curl -fsS "http://127.0.0.1:$RELAY_PORT/relay/api/agents" \
  -H "authorization: Bearer $TOKEN" \
  | jq -e --arg account "$ACCOUNT_ID" '.accountId == $account and .agents[0].agentId == "studio" and .agents[0].activeSessionCount == 2' >/dev/null

curl -fsS "http://127.0.0.1:$RELAY_PORT/relay/agents/studio/" \
  -H "authorization: Bearer $TOKEN" \
  | grep -F '<script src="/assets/app.js"></script>' >/dev/null

if grep -F "$TOKEN" "$STORE_PATH" >/dev/null; then
  echo "raw Relay token was persisted" >&2
  exit 1
fi
if grep -F 'relay-password-123' "$STORE_PATH" >/dev/null; then
  echo "raw Relay password was persisted" >&2
  exit 1
fi

stop_pid "$AGENT_PID"
AGENT_PID=""
stop_pid "$RELAY_PID"
RELAY_PID=""
start_relay
LOGIN_JSON="$(curl -fsS -X POST "http://127.0.0.1:$RELAY_PORT/relay/api/auth/login" \
  -H 'content-type: application/json' \
  --data '{"username":"deploy-smoke","password":"relay-password-123"}')"
RESTART_TOKEN="$(jq -er '.accessToken' <<<"$LOGIN_JSON")"
curl -fsS "http://127.0.0.1:$RELAY_PORT/relay/api/agents" \
  -H "authorization: Bearer $RESTART_TOKEN" \
  | jq -e '.agents[0].agentId == "studio"' >/dev/null

test -f "$ROOT_DIR/apps/freehand-relay-server/deploy/freehand-relay.service"
test -f "$ROOT_DIR/apps/freehand-relay-server/deploy/relay.env.example"
if rg -n -i '(password|token|secret)=' "$ROOT_DIR/apps/freehand-relay-server/deploy"; then
  echo "Relay deployment manifest contains a secret field" >&2
  exit 1
fi

echo "relay_deployment_smoke_ok"
