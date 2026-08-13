#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
binary="$target_dir/debug/freehand-relay-server"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/freehand-account-config-smoke.XXXXXX")"
store_path="$work_dir/relay-store.json"
config_dir="$work_dir/account-config"
log_path="$work_dir/relay.log"
port="$(
  ruby -rsocket -e 'server = TCPServer.new("127.0.0.1", 0); puts server.addr[1]; server.close'
)"
base_url="http://127.0.0.1:$port"
relay_pid=""

cleanup() {
  if [[ -n "$relay_pid" ]] && kill -0 "$relay_pid" 2>/dev/null; then
    kill "$relay_pid"
    wait "$relay_pid" 2>/dev/null || true
  fi
  rm -rf -- "$work_dir"
}
trap cleanup EXIT

cargo build -p freehand-relay-server >/dev/null

export FREEHAND_RELAY_BIND="127.0.0.1:$port"
export FREEHAND_RELAY_STORE="$store_path"
export FREEHAND_RELAY_PRESENCE_LEASE_SECONDS=45
export FREEHAND_RELAY_SECURE_COOKIE=false
export FREEHAND_RELAY_ACCOUNT_CONFIG_DIR="$config_dir"
unset FREEHAND_RELAY_UPDATES_DIR

"$binary" init-store
"$binary" serve >"$log_path" 2>&1 &
relay_pid=$!

for _ in 1 2 3 4 5 6 7 8 9 10; do
  if curl --fail --silent "$base_url/relay/health" >/dev/null; then
    break
  fi
  sleep 0.2
done
curl --fail --silent "$base_url/relay/health" >/dev/null

auth_json="$(
  curl --fail --silent --show-error \
    -H 'content-type: application/json' \
    -d '{"username":"config-smoke","password":"relay-password-123"}' \
    "$base_url/relay/api/auth/register"
)"
token="$(printf '%s' "$auth_json" | jq -er '.accessToken')"

first_json="$(
  curl --fail --silent --show-error \
    -X PUT \
    -H "authorization: Bearer $token" \
    -H 'content-type: application/json' \
    -d '{"schemaVersion":1,"document":{"providerRegistry":[{"id":"primary","label":"Primary","providerType":"openai-compatible","protocol":"chat","baseUrl":"https://api.example.com/v1","auth":{"authType":"env","authSource":"FREEHAND_PRIMARY_API_KEY"},"model":"gpt-5.6"}],"modelGroups":[],"relayEndpointCandidates":[],"remoteDaemonRegistry":[]}}' \
    "$base_url/relay/api/config"
)"
etag="$(printf '%s' "$first_json" | jq -er '.etag')"
[[ "$(printf '%s' "$first_json" | jq -er '.revision')" == "1" ]]

kill "$relay_pid"
wait "$relay_pid" || true
relay_pid=""
"$binary" serve >"$log_path" 2>&1 &
relay_pid=$!
for _ in 1 2 3 4 5 6 7 8 9 10; do
  if curl --fail --silent "$base_url/relay/health" >/dev/null; then
    break
  fi
  sleep 0.2
done

restored_json="$(
  curl --fail --silent --show-error \
    -H "authorization: Bearer $token" \
    "$base_url/relay/api/config"
)"
[[ "$(printf '%s' "$restored_json" | jq -er '.revision')" == "1" ]]
[[ "$(printf '%s' "$restored_json" | jq -er '.etag')" == "$etag" ]]
[[ "$(printf '%s' "$restored_json" | jq -er '.document.providerRegistry[0].auth.authSource')" == "FREEHAND_PRIMARY_API_KEY" ]]
if printf '%s' "$restored_json" | grep -Eq 'sk-|password|tokenValue'; then
  echo "account config response contains forbidden secret material" >&2
  exit 1
fi

stale_status="$(
  curl --silent --output "$work_dir/stale.json" --write-out '%{http_code}' \
    -X PUT \
    -H "authorization: Bearer $token" \
    -H 'if-match: "stale"' \
    -H 'content-type: application/json' \
    -d '{"schemaVersion":1,"document":{"providerRegistry":[],"modelGroups":[],"relayEndpointCandidates":[],"remoteDaemonRegistry":[]}}' \
    "$base_url/relay/api/config"
)"
[[ "$stale_status" == "409" ]]
[[ "$(jq -er '.serverDocument.revision' "$work_dir/stale.json")" == "1" ]]

secret_status="$(
  curl --silent --output "$work_dir/secret.json" --write-out '%{http_code}' \
    -X PUT \
    -H "authorization: Bearer $token" \
    -H "if-match: $etag" \
    -H 'content-type: application/json' \
    -d '{"schemaVersion":1,"document":{"providerRegistry":[{"id":"primary","label":"Primary","providerType":"openai-compatible","protocol":"chat","baseUrl":"https://api.example.com/v1","auth":{"authType":"inline","authSource":"sk-live-secret"},"model":"gpt-5.6"}],"modelGroups":[],"relayEndpointCandidates":[],"remoteDaemonRegistry":[]}}' \
    "$base_url/relay/api/config"
)"
[[ "$secret_status" == "400" ]]

printf 'relay_account_config_smoke_ok revision=1 etag=%s\n' "$etag"
