#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
prefix="${FREEHAND_PREFIX:-"$HOME/.local"}"
bin_dir="$prefix/bin"
runtime_home="${FREEHAND_RUNTIME_HOME:-"$HOME/.freehand"}"
logs_dir="$runtime_home/logs"
agent="${FREEHAND_DAEMON_AGENT:-master}"
workdir="${FREEHAND_DAEMON_WORKDIR:-"$repo_root"}"
pair_token="${FREEHAND_PAIR_TOKEN_SHARED:-}"
command="${1:-install}"

case "$command" in
  install|restart)
    service_suffix=""
    default_label="com.freehand.daemon"
    default_bind_addr="127.0.0.1:4041"
    ;;
  installS|restartS)
    service_suffix="S"
    default_label="com.freehand.daemonS"
    default_bind_addr="127.0.0.1:4042"
    ;;
  *)
    echo "usage: $0 [install|restart|installS|restartS]" >&2
    exit 2
    ;;
esac

env_file="${FREEHAND_DAEMON_ENV_FILE:-"$runtime_home/daemon${service_suffix}.env"}"
label="${FREEHAND_LAUNCHD_LABEL:-$default_label}"
plist_path="$HOME/Library/LaunchAgents/$label.plist"
bind_addr="${FREEHAND_DAEMON_BIND:-$default_bind_addr}"
daemon_bin="$bin_dir/freehand-daemon${service_suffix}"
launchd_wrapper="$bin_dir/freehand-daemon-launchd${service_suffix}"
stdout_log="$logs_dir/daemon${service_suffix}.stdout.log"
stderr_log="$logs_dir/daemon${service_suffix}.stderr.log"

cd "$repo_root"

upsert_env_var() {
  local key="$1"
  local value="$2"
  local tmp_env
  tmp_env="$(mktemp)"
  awk -v key="$key" -v value="$value" '
    BEGIN { updated = 0 }
    $0 ~ "^" key "=" {
      printf "%s=\"%s\"\n", key, value
      updated = 1
      next
    }
    { print }
    END {
      if (updated == 0) {
        printf "%s=\"%s\"\n", key, value
      }
    }
  ' "$env_file" >"$tmp_env"
  cat "$tmp_env" >"$env_file"
  rm -f "$tmp_env"
}

write_launchd_env() {
  if [[ -z "$pair_token" ]]; then
    if command -v uuidgen >/dev/null 2>&1; then
      pair_token="$(uuidgen | tr '[:upper:]' '[:lower:]')"
    else
      pair_token="freehand-$(date +%s)-$$"
    fi
  fi

  if [[ ! -f "$env_file" ]]; then
    cat >"$env_file" <<EOF
HOME="$HOME"
FREEHAND_DAEMON_AGENT="$agent"
FREEHAND_DAEMON_BIND="$bind_addr"
FREEHAND_DAEMON_WORKDIR="$workdir"
FREEHAND_DAEMON_BIN="$daemon_bin"
FREEHAND_PAIR_TOKEN_SHARED="$pair_token"
PATH="$bin_dir:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
EOF
    chmod 0600 "$env_file"
  else
    # shellcheck disable=SC1090
    . "$env_file"
    if [[ -n "${FREEHAND_DAEMON_BIN:-}" && "$FREEHAND_DAEMON_BIN" != "$daemon_bin" ]]; then
      echo "daemon env uses a different binary path: $FREEHAND_DAEMON_BIN" >&2
      echo "expected: $daemon_bin" >&2
      exit 2
    fi
    if [[ -z "${FREEHAND_DAEMON_BIN:-}" ]]; then
      printf '\nFREEHAND_DAEMON_BIN="%s"\n' "$daemon_bin" >>"$env_file"
    fi
    upsert_env_var "FREEHAND_DAEMON_AGENT" "$agent"
    upsert_env_var "FREEHAND_DAEMON_BIND" "$bind_addr"
    upsert_env_var "FREEHAND_DAEMON_WORKDIR" "$workdir"
    upsert_env_var "FREEHAND_DAEMON_BIN" "$daemon_bin"
    if [[ -z "${HOME:-}" ]]; then
      printf '\nHOME="%s"\n' "$HOME" >>"$env_file"
    elif ! rg -q '^HOME=' "$env_file"; then
      printf '\nHOME="%s"\n' "$HOME" >>"$env_file"
    fi
    echo "[freehand-launchd] keeping existing env file: $env_file"
  fi
}

write_launchd_plist() {
  mkdir -p "$runtime_home" "$logs_dir" "$HOME/Library/LaunchAgents"

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
    <string>$launchd_wrapper</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>WorkingDirectory</key>
  <string>$workdir</string>
  <key>StandardOutPath</key>
  <string>$stdout_log</string>
  <key>StandardErrorPath</key>
  <string>$stderr_log</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>FREEHAND_DAEMON_ENV_FILE</key>
    <string>$env_file</string>
    <key>HOME</key>
    <string>$HOME</string>
    <key>PATH</key>
    <string>$bin_dir:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
  </dict>
  </dict>
</plist>
EOF
}

run_install_launchd() {
  launchctl bootout "gui/$(id -u)" "$plist_path" >/dev/null 2>&1 || true
  launchctl bootstrap "gui/$(id -u)" "$plist_path"
  launchctl enable "gui/$(id -u)/$label"
  wait_for_health

  echo "[freehand-launchd] installed:"
  echo "  label: $label"
  echo "  plist: $plist_path"
  echo "  env: $env_file"
  echo "  logs: $stdout_log"
  echo "  logs: $stderr_log"
  echo "  webui: http://$bind_addr/"
  echo "[freehand-launchd] status:"
  launchctl print "gui/$(id -u)/$label" | sed -n '1,80p'
}

restart_launchd() {
  launchctl kickstart -k "gui/$(id -u)/$label"
  wait_for_health
  echo "[freehand-launchd] restarted $label"
}

wait_for_health() {
  local health_url="http://$bind_addr/health"
  local attempt=1
  local max_attempts=30

  while [[ $attempt -le $max_attempts ]]; do
    if curl -4fsS "$health_url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
    attempt=$((attempt + 1))
  done

  echo "daemon did not become healthy at $health_url within ${max_attempts}s" >&2
  exit 1
}

case "$command" in
  install)
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-global.sh
    write_launchd_env
    write_launchd_plist
    run_install_launchd
    ;;
  installS)
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-symlink.sh
    write_launchd_env
    write_launchd_plist
    run_install_launchd
    ;;
  restart)
    restart_launchd
    ;;
  restartS)
    restart_launchd
    ;;
esac
