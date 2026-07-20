#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
prefix="${FREEHAND_PREFIX:-"$HOME/.local"}"
bin_dir="$prefix/bin"
runtime_home="${FREEHAND_RUNTIME_HOME:-"$HOME/.freehand"}"
logs_dir="$runtime_home/logs"
command="${1:-installS}"
tool_path="$HOME/.cargo/bin:$bin_dir:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export PATH="$tool_path"

detect_tailscale_ip() {
  if command -v tailscale >/dev/null 2>&1; then
    tailscale ip -4 2>/dev/null | sed -n '1p'
  fi
}

default_relay_bind() {
  local port="$1"
  local tailscale_ip
  tailscale_ip="$(detect_tailscale_ip)"
  if [[ -n "$tailscale_ip" ]]; then
    printf '%s:%s\n' "$tailscale_ip" "$port"
  else
    printf '127.0.0.1:%s\n' "$port"
  fi
}

case "$command" in
  install|restart)
    service_suffix=""
    default_label="com.freehand.relay"
    default_bind_addr="$(default_relay_bind 44041)"
    default_upstream="http://$(default_relay_bind 4041)"
    default_daemon_id="studio"
    daemon_bin="$bin_dir/freehand-daemon"
    install_binary_command=(scripts/install-global.sh)
    ;;
  installS|restartS)
    service_suffix="S"
    default_label="com.freehand.relayS"
    default_bind_addr="$(default_relay_bind 44042)"
    default_upstream="http://127.0.0.1:4042"
    default_daemon_id="studio-s-profile"
    daemon_bin="$bin_dir/freehand-daemonS-bin"
    install_binary_command=(scripts/install-symlink.sh)
    ;;
  *)
    echo "usage: $0 [install|restart|installS|restartS]" >&2
    exit 2
    ;;
esac

label="${FREEHAND_RELAY_LAUNCHD_LABEL:-$default_label}"
bind_addr="${FREEHAND_RELAY_BIND:-$default_bind_addr}"
upstream_base_url="${FREEHAND_RELAY_UPSTREAM_BASE_URL:-$default_upstream}"
account_id="${FREEHAND_RELAY_ACCOUNT_ID:-jason}"
daemon_id="${FREEHAND_RELAY_DAEMON_ID:-$default_daemon_id}"
relay_host_id="${FREEHAND_RELAY_HOST_ID:-studio-host}"
env_file="${FREEHAND_RELAY_ENV_FILE:-"$runtime_home/relay${service_suffix}.env"}"
stdout_log="$logs_dir/relay${service_suffix}.stdout.log"
stderr_log="$logs_dir/relay${service_suffix}.stderr.log"
plist_path="$HOME/Library/LaunchAgents/$label.plist"

if [[ "${FREEHAND_LAUNCHD_PLAN_ONLY:-0}" == "1" ]]; then
  printf 'role=relay\n'
  printf 'label=%s\n' "$label"
  printf 'bind_addr=%s\n' "$bind_addr"
  printf 'upstream_base_url=%s\n' "$upstream_base_url"
  printf 'account_id=%s\n' "$account_id"
  printf 'daemon_id=%s\n' "$daemon_id"
  printf 'relay_host_id=%s\n' "$relay_host_id"
  printf 'env_file=%s\n' "$env_file"
  printf 'stdout_log=%s\n' "$stdout_log"
  printf 'stderr_log=%s\n' "$stderr_log"
  printf 'plist_path=%s\n' "$plist_path"
  exit 0
fi

cd "$repo_root"
mkdir -p "$runtime_home" "$logs_dir" "$HOME/Library/LaunchAgents"

if [[ "${FREEHAND_RELAY_SKIP_BINARY_INSTALL:-0}" != "1" && ( "$command" == install* || "$command" == restart* ) ]]; then
  env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT "${install_binary_command[@]}"
fi

cat >"$env_file" <<EOF
HOME="$HOME"
FREEHAND_RELAY_BIND="$bind_addr"
FREEHAND_RELAY_UPSTREAM_BASE_URL="$upstream_base_url"
FREEHAND_RELAY_ACCOUNT_ID="$account_id"
FREEHAND_RELAY_DAEMON_ID="$daemon_id"
FREEHAND_RELAY_HOST_ID="$relay_host_id"
FREEHAND_DAEMON_BIN="$daemon_bin"
PATH="$tool_path"
EOF
chmod 0600 "$env_file"

cat >"$plist_path" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$label</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/bash</string>
    <string>-lc</string>
    <string>set -a; [ -f "$env_file" ] &amp;&amp; . "$env_file"; set +a; cd "$runtime_home" &amp;&amp; exec "$daemon_bin" remote-relay --bind "$bind_addr"</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>WorkingDirectory</key>
  <string>$HOME</string>
  <key>StandardOutPath</key>
  <string>$stdout_log</string>
  <key>StandardErrorPath</key>
  <string>$stderr_log</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>FREEHAND_RELAY_ENV_FILE</key>
    <string>$env_file</string>
    <key>HOME</key>
    <string>$HOME</string>
    <key>PATH</key>
    <string>$tool_path</string>
    <key>FREEHAND_RELAY_BIND</key>
    <string>$bind_addr</string>
    <key>FREEHAND_RELAY_UPSTREAM_BASE_URL</key>
    <string>$upstream_base_url</string>
    <key>FREEHAND_RELAY_ACCOUNT_ID</key>
    <string>$account_id</string>
    <key>FREEHAND_RELAY_DAEMON_ID</key>
    <string>$daemon_id</string>
    <key>FREEHAND_RELAY_HOST_ID</key>
    <string>$relay_host_id</string>
    <key>FREEHAND_DAEMON_BIN</key>
    <string>$daemon_bin</string>
  </dict>
</dict>
</plist>
EOF

relay_url="http://$bind_addr"

wait_for_relay_health() {
  local attempt=1
  local max_attempts="${FREEHAND_RELAY_HEALTH_WAIT_SECONDS:-60}"
  while [[ $attempt -le $max_attempts ]]; do
    if curl -4fsS "$relay_url/relay/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
    attempt=$((attempt + 1))
  done
  echo "relay did not become healthy at $relay_url/relay/health within ${max_attempts}s" >&2
  tail -n 120 "$stderr_log" >&2 || true
  exit 1
}

register_relay_host() {
  local now_unix
  now_unix="$(date +%s)"
  curl -4fsS \
    -H 'content-type: application/json' \
    -X POST \
    --data "{
      \"accountId\":\"${account_id}\",
      \"daemonId\":\"${daemon_id}\",
      \"relayHostId\":\"${relay_host_id}\",
      \"upstreamBaseUrl\":\"${upstream_base_url}\",
      \"endpoints\":[{
        \"id\":\"relay:${relay_host_id}\",
        \"kind\":\"relay\",
        \"webUrl\":\"/relay/daemon/${relay_host_id}/\",
        \"adpUrl\":\"/relay/daemon/${relay_host_id}/adp\",
        \"relayHostId\":\"${relay_host_id}\",
        \"authRequired\":false,
        \"lastSeenUnix\":${now_unix}
      }]
    }" \
    "$relay_url/relay/hosts" >/dev/null
}

wait_for_registered_host() {
  local host_health="$relay_url/relay/daemon/$relay_host_id/health"
  local attempt=1
  local max_attempts="${FREEHAND_RELAY_UPSTREAM_WAIT_SECONDS:-60}"
  while [[ $attempt -le $max_attempts ]]; do
    if [[ "$(curl -4fsS "$host_health" 2>/dev/null || true)" == "ok" ]]; then
      return 0
    fi
    sleep 1
    attempt=$((attempt + 1))
  done
  echo "relay host $relay_host_id did not proxy upstream health at $host_health within ${max_attempts}s" >&2
  tail -n 120 "$stderr_log" >&2 || true
  exit 1
}

launchctl bootout "gui/$(id -u)" "$plist_path" >/dev/null 2>&1 || true
launchctl bootstrap "gui/$(id -u)" "$plist_path"
launchctl enable "gui/$(id -u)/$label"
wait_for_relay_health
register_relay_host
wait_for_registered_host

service_state="$(launchctl print "gui/$(id -u)/$label" | awk '$1 == "state" && $2 == "=" { print $3; exit }')"
service_pid="$(launchctl print "gui/$(id -u)/$label" | awk '$1 == "pid" && $2 == "=" { print $3; exit }')"

echo "[freehand-relay-launchd] restarted $label"
echo "  relay: $relay_url/relay/daemon/$relay_host_id/"
echo "  upstream: $upstream_base_url"
echo "  env: $env_file"
echo "  logs: $stdout_log"
echo "  logs: $stderr_log"
echo "  state: $service_state"
echo "  pid: $service_pid"
