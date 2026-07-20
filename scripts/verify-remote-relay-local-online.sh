#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/freehand-remote-relay-online.XXXXXX")"
upstream_pid=""
relay_pid=""

cleanup() {
  local status=$?
  if [[ -n "$relay_pid" ]]; then
    kill "$relay_pid" 2>/dev/null || true
    wait "$relay_pid" 2>/dev/null || true
  fi
  if [[ -n "$upstream_pid" ]]; then
    kill "$upstream_pid" 2>/dev/null || true
    wait "$upstream_pid" 2>/dev/null || true
  fi
  rm -rf "$tmp_dir"
  exit "$status"
}
trap cleanup EXIT INT TERM

read -r upstream_port relay_port < <(python3 - <<'PY'
import socket

sockets = []
ports = []
for _ in range(2):
    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    sockets.append(sock)
    ports.append(sock.getsockname()[1])
print(*ports)
for sock in sockets:
    sock.close()
PY
)

cargo build -p freehand-server -p freehand-daemon -p freehand-cli >/dev/null

upstream_url="http://127.0.0.1:${upstream_port}"
relay_url="http://127.0.0.1:${relay_port}"
relay_adp_url="ws://127.0.0.1:${relay_port}/relay/daemon/studio-host/adp"

target/debug/freehand-server webui-serve-smoke --bind "127.0.0.1:${upstream_port}" \
  >"${tmp_dir}/upstream.stdout.log" 2>"${tmp_dir}/upstream.stderr.log" &
upstream_pid="$!"

target/debug/freehand-daemon remote-relay --bind "127.0.0.1:${relay_port}" \
  >"${tmp_dir}/relay.stdout.log" 2>"${tmp_dir}/relay.stderr.log" &
relay_pid="$!"

wait_until_ok() {
  local url="$1"
  for _ in {1..200}; do
    if [[ "$(curl -fsS "$url" 2>/dev/null || true)" == "ok" ]]; then
      return 0
    fi
    sleep 0.15
  done
  echo "health check failed for ${url}" >&2
  echo "upstream stdout:" >&2
  sed -n '1,80p' "${tmp_dir}/upstream.stdout.log" >&2 || true
  echo "upstream stderr:" >&2
  sed -n '1,80p' "${tmp_dir}/upstream.stderr.log" >&2 || true
  echo "relay stdout:" >&2
  sed -n '1,80p' "${tmp_dir}/relay.stdout.log" >&2 || true
  echo "relay stderr:" >&2
  sed -n '1,80p' "${tmp_dir}/relay.stderr.log" >&2 || true
  return 1
}

wait_until_ok "${upstream_url}/health"
wait_until_ok "${relay_url}/relay/health"

curl -fsS \
  -H 'content-type: application/json' \
  -X POST \
  --data "{
    \"accountId\":\"jason\",
    \"daemonId\":\"studio\",
    \"relayHostId\":\"studio-host\",
    \"upstreamBaseUrl\":\"${upstream_url}\",
    \"endpoints\":[{
      \"id\":\"relay:studio-host\",
      \"kind\":\"relay\",
      \"webUrl\":\"/relay/daemon/studio-host/\",
      \"adpUrl\":\"/relay/daemon/studio-host/adp\",
      \"relayHostId\":\"studio-host\",
      \"authRequired\":true,
      \"lastSeenUnix\":10
    }]
  }" \
  "${relay_url}/relay/hosts" >"${tmp_dir}/register.json"

python3 - "${tmp_dir}/register.json" <<'PY'
import json, sys

payload = json.load(open(sys.argv[1]))
assert payload["accountId"] == "jason", payload
assert payload["daemonId"] == "studio", payload
assert payload["relayHostId"] == "studio-host", payload
PY

curl -fsS "${relay_url}/relay/directory/jason" >"${tmp_dir}/directory.json"
python3 - "${tmp_dir}/directory.json" <<'PY'
import json, sys

payload = json.load(open(sys.argv[1]))
assert payload["schemaVersion"] == 1, payload
assert payload["accountId"] == "jason", payload
hosts = payload["daemons"]
assert len(hosts) == 1, payload
assert hosts[0]["relayHostId"] == "studio-host", payload
PY

health_body="$(curl -fsS "${relay_url}/relay/daemon/studio-host/health")"
if [[ "$health_body" != "ok" ]]; then
  echo "unexpected relay health body: ${health_body}" >&2
  exit 1
fi

curl -fsS "${relay_url}/relay/daemon/studio-host/?client=android-webview" \
  >"${tmp_dir}/relay-root.html"
grep -F 'data-webui-shell="true"' "${tmp_dir}/relay-root.html" >/dev/null
grep -F 'href="/relay/daemon/studio-host/assets/theme.css?v=' "${tmp_dir}/relay-root.html" >/dev/null
grep -F 'src="/relay/daemon/studio-host/assets/webui.js?v=' "${tmp_dir}/relay-root.html" >/dev/null
grep -F 'data-adp-endpoint="/relay/daemon/studio-host/adp"' "${tmp_dir}/relay-root.html" >/dev/null
grep -F 'data-turn-subscribe="/relay/daemon/studio-host/ui/subscribe/turn/latest"' "${tmp_dir}/relay-root.html" >/dev/null
if grep -F 'href="/assets/theme.css' "${tmp_dir}/relay-root.html" >/dev/null; then
  echo "relay root leaked daemon-root asset path" >&2
  exit 1
fi

curl -fsS "${relay_url}/relay/daemon/studio-host/assets/webui.css?v=relay-test" \
  >"${tmp_dir}/relay-webui.css"
grep -F '.app-shell' "${tmp_dir}/relay-webui.css" >/dev/null

curl -fsS "${relay_url}/relay/daemon/studio-host/assets/webui.js?v=relay-test" \
  >"${tmp_dir}/relay-webui.js"
grep -F 'from "/relay/daemon/studio-host/assets/theme.js?v=' "${tmp_dir}/relay-webui.js" >/dev/null
if grep -F 'from "/assets/theme.js' "${tmp_dir}/relay-webui.js" >/dev/null; then
  echo "relay webui js leaked daemon-root theme import" >&2
  exit 1
fi

curl -fsS "${relay_url}/relay/daemon/studio-host/ui/query/latest-active-turn" \
  >"${tmp_dir}/relay-latest-turn.json"
grep -F '"turn_id":"turn-webui-smoke"' "${tmp_dir}/relay-latest-turn.json" >/dev/null

adp_output="$(target/debug/freehand-cli adp-smoke --url "$relay_adp_url")"
if [[ "$adp_output" != adp_smoke_ok* ]]; then
  echo "unexpected relay ADP smoke output: ${adp_output}" >&2
  exit 1
fi

missing_status="$(
  curl -sS -o "${tmp_dir}/missing.json" -w '%{http_code}' \
    "${relay_url}/relay/daemon/missing-host/health"
)"
if [[ "$missing_status" != "404" ]]; then
  echo "expected missing relay host 404, got ${missing_status}" >&2
  exit 1
fi
python3 - "${tmp_dir}/missing.json" <<'PY'
import json, sys

payload = json.load(open(sys.argv[1]))
assert payload["code"] == "relay_host_not_found", payload
PY

echo "remote_relay_local_online_ok upstream_url=${upstream_url} relay_url=${relay_url} relay_host=studio-host adp=${relay_adp_url}"
