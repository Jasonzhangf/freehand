#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
prefix="${FREEHAND_PREFIX:-"$HOME/.local"}"
bin_dir="$prefix/bin"
runtime_home="${FREEHAND_RUNTIME_HOME:-"$HOME/.freehand"}"
logs_dir="$runtime_home/logs"
pair_token="${FREEHAND_PAIR_TOKEN_SHARED:-}"
command="${1:-install}"

sanitize_launchd_component() {
  printf '%s\n' "$1" | sed 's/[^A-Za-z0-9_.-]/-/g'
}

detect_tailscale_ip() {
  if command -v tailscale >/dev/null 2>&1; then
    tailscale ip -4 2>/dev/null | sed -n '1p'
  fi
}

default_daemon_bind() {
  local port="$1"
  local profile_suffix="${2:-}"
  if [[ "$profile_suffix" == "S" ]]; then
    printf '127.0.0.1:%s\n' "$port"
    return 0
  fi
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
    service_role="master"
    service_suffix=""
    default_label="com.freehand.daemon"
    default_bind_addr="$(default_daemon_bind 4041 "$service_suffix")"
    ;;
  installS|restartS)
    service_role="master"
    service_suffix="S"
    default_label="com.freehand.daemonS"
    default_bind_addr="$(default_daemon_bind 4042 "$service_suffix")"
    ;;
  installWorker|restartWorker)
    service_role="worker"
    service_suffix=""
    default_label="com.freehand.worker"
    default_bind_addr=""
    ;;
  installWorkerS|restartWorkerS)
    service_role="worker"
    service_suffix="S"
    default_label="com.freehand.workerS"
    default_bind_addr=""
    ;;
  *)
    echo "usage: $0 [install|restart|installS|restartS|installWorker|restartWorker|installWorkerS|restartWorkerS]" >&2
    exit 2
    ;;
esac

if [[ "$service_role" == "worker" ]]; then
  agent="${FREEHAND_WORKER_AGENT:-worker}"
  agent_label_component="$(sanitize_launchd_component "$agent")"
  default_label="${default_label}.${agent_label_component}"
  workdir="${FREEHAND_WORKER_WORKDIR:-"$runtime_home"}"
  env_file="${FREEHAND_DAEMON_ENV_FILE:-"$runtime_home/worker${service_suffix}.${agent_label_component}.env"}"
  stdout_log="$logs_dir/worker${service_suffix}.${agent_label_component}.stdout.log"
  stderr_log="$logs_dir/worker${service_suffix}.${agent_label_component}.stderr.log"
else
  agent="${FREEHAND_DAEMON_AGENT:-master}"
  workdir="${FREEHAND_DAEMON_WORKDIR:-"$runtime_home"}"
  env_file="${FREEHAND_DAEMON_ENV_FILE:-"$runtime_home/daemon${service_suffix}.env"}"
  stdout_log="$logs_dir/daemon${service_suffix}.stdout.log"
  stderr_log="$logs_dir/daemon${service_suffix}.stderr.log"
fi
label="${FREEHAND_LAUNCHD_LABEL:-$default_label}"
plist_path="$HOME/Library/LaunchAgents/$label.plist"
daemon_bin="$bin_dir/freehand-daemon${service_suffix}"
if [[ "$service_suffix" == "S" ]]; then
  daemon_bin="$bin_dir/freehand-daemonS-bin"
fi
launchd_wrapper="$bin_dir/freehand-daemon-launchd${service_suffix}"

if [[ "${FREEHAND_LAUNCHD_PLAN_ONLY:-0}" == "1" ]]; then
  printf 'role=%s\n' "$service_role"
  printf 'agent=%s\n' "$agent"
  printf 'label=%s\n' "$label"
  printf 'env_file=%s\n' "$env_file"
  printf 'stdout_log=%s\n' "$stdout_log"
  printf 'stderr_log=%s\n' "$stderr_log"
  printf 'plist_path=%s\n' "$plist_path"
  exit 0
fi

bind_addr="$default_bind_addr"
if [[ "$service_role" == "master" ]]; then
  if [[ -n "${FREEHAND_DAEMON_BIND:-}" ]]; then
    bind_addr="$FREEHAND_DAEMON_BIND"
  elif [[ -f "$env_file" ]]; then
    env_bind="$(awk -F= '$1 == "FREEHAND_DAEMON_BIND" { gsub(/^"/, "", $2); gsub(/"$/, "", $2); print $2; exit }' "$env_file")"
    if [[ -n "$env_bind" ]]; then
      bind_addr="$env_bind"
    fi
  fi
fi

cd "$repo_root"
mkdir -p "$runtime_home" "$logs_dir" "$workdir"

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

remove_env_var() {
  local key="$1"
  local tmp_env
  tmp_env="$(mktemp)"
  awk -v key="$key" '$0 !~ "^" key "=" { print }' "$env_file" >"$tmp_env"
  cat "$tmp_env" >"$env_file"
  rm -f "$tmp_env"
}

copy_worker_provider_env_from_master() {
  if [[ "$service_role" != "worker" ]]; then
    return 0
  fi
  local master_env_file="$runtime_home/daemon${service_suffix}.env"
  if [[ ! -f "$master_env_file" ]]; then
    return 0
  fi
  while IFS='=' read -r key raw_value; do
    if [[ "$key" =~ ^FREEHAND_.*(_KEY|CREDENTIAL|SECRET)$ ]]; then
      local value="$raw_value"
      value="${value%\"}"
      value="${value#\"}"
      upsert_env_var "$key" "$value"
    fi
  done <"$master_env_file"
}

write_launchd_env() {
  if [[ -z "$pair_token" ]]; then
    if [[ "$service_role" == "worker" ]]; then
      master_env_file="$runtime_home/daemon${service_suffix}.env"
      if [[ ! -f "$master_env_file" ]]; then
        echo "worker requires existing master env: $master_env_file" >&2
        exit 2
      fi
      pair_token="$(awk -F= '$1 == "FREEHAND_PAIR_TOKEN_SHARED" { gsub(/^"/, "", $2); gsub(/"$/, "", $2); print $2; exit }' "$master_env_file")"
      if [[ -z "$pair_token" ]]; then
        echo "worker requires FREEHAND_PAIR_TOKEN_SHARED from $master_env_file" >&2
        exit 2
      fi
    elif command -v uuidgen >/dev/null 2>&1; then
      pair_token="$(uuidgen | tr '[:upper:]' '[:lower:]')"
    else
      pair_token="freehand-$(date +%s)-$$"
    fi
  fi

  if [[ ! -f "$env_file" ]]; then
    cat >"$env_file" <<EOF
HOME="$HOME"
FREEHAND_DAEMON_AGENT="$agent"
FREEHAND_DAEMON_WORKDIR="$workdir"
FREEHAND_DAEMON_BIN="$daemon_bin"
FREEHAND_PAIR_TOKEN_SHARED="$pair_token"
PATH="$bin_dir:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
EOF
    if [[ -n "${FREEHAND_PROVIDER_RETRY_BACKOFF_MS:-}" ]]; then
      printf 'FREEHAND_PROVIDER_RETRY_BACKOFF_MS="%s"\n' "$FREEHAND_PROVIDER_RETRY_BACKOFF_MS" >>"$env_file"
    fi
    if [[ "$service_role" == "master" ]]; then
      printf 'FREEHAND_DAEMON_BIND="%s"\n' "$bind_addr" >>"$env_file"
    fi
    chmod 0600 "$env_file"
  else
    required_pair_token="$pair_token"
    # shellcheck disable=SC1090
    . "$env_file"
    if [[ "$service_role" == "worker" ]]; then
      pair_token="$required_pair_token"
    else
      pair_token="${FREEHAND_PAIR_TOKEN_SHARED:-$pair_token}"
    fi
    if [[ -n "${FREEHAND_DAEMON_BIN:-}" && "$FREEHAND_DAEMON_BIN" != "$daemon_bin" && "$service_suffix" != "S" ]]; then
      echo "daemon env uses a different binary path: $FREEHAND_DAEMON_BIN" >&2
      echo "expected: $daemon_bin" >&2
      exit 2
    fi
    if [[ -z "${FREEHAND_DAEMON_BIN:-}" ]]; then
      printf '\nFREEHAND_DAEMON_BIN="%s"\n' "$daemon_bin" >>"$env_file"
    fi
    upsert_env_var "FREEHAND_DAEMON_AGENT" "$agent"
    upsert_env_var "FREEHAND_DAEMON_WORKDIR" "$workdir"
    upsert_env_var "FREEHAND_DAEMON_BIN" "$daemon_bin"
    upsert_env_var "FREEHAND_PAIR_TOKEN_SHARED" "$pair_token"
    if [[ -n "${FREEHAND_PROVIDER_RETRY_BACKOFF_MS:-}" ]]; then
      upsert_env_var "FREEHAND_PROVIDER_RETRY_BACKOFF_MS" "$FREEHAND_PROVIDER_RETRY_BACKOFF_MS"
    else
      remove_env_var "FREEHAND_PROVIDER_RETRY_BACKOFF_MS"
    fi
    if [[ "$service_role" == "master" ]]; then
      upsert_env_var "FREEHAND_DAEMON_BIND" "$bind_addr"
    else
      remove_env_var "FREEHAND_DAEMON_BIND"
    fi
    if [[ -z "${HOME:-}" ]]; then
      printf '\nHOME="%s"\n' "$HOME" >>"$env_file"
    elif ! rg -q '^HOME=' "$env_file"; then
      printf '\nHOME="%s"\n' "$HOME" >>"$env_file"
    fi
    echo "[freehand-launchd] keeping existing env file: $env_file"
  fi
  copy_worker_provider_env_from_master
}

write_launchd_plist() {
  mkdir -p "$runtime_home" "$logs_dir" "$HOME/Library/LaunchAgents"

  if [[ "$service_role" == "worker" ]]; then
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
    <string>set -a; [ -f "$env_file" ] &amp;&amp; . "$env_file"; set +a; cd "$workdir" &amp;&amp; exec "$daemon_bin" serve --agent "$agent"</string>
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
    <key>FREEHAND_DAEMON_ENV_FILE</key>
    <string>$env_file</string>
    <key>HOME</key>
    <string>$HOME</string>
    <key>PATH</key>
    <string>$bin_dir:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    <key>FREEHAND_DAEMON_AGENT</key>
    <string>$agent</string>
    <key>FREEHAND_DAEMON_WORKDIR</key>
    <string>$workdir</string>
    <key>FREEHAND_DAEMON_BIN</key>
    <string>$daemon_bin</string>
    <key>FREEHAND_PAIR_TOKEN_SHARED</key>
    <string>$pair_token</string>
  </dict>
  </dict>
</plist>
EOF
    return 0
  fi

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
    <string>set -a; [ -f "$env_file" ] &amp;&amp; . "$env_file"; set +a; cd "$workdir" &amp;&amp; exec "$daemon_bin" serve --agent "$agent" --bind "$bind_addr"</string>
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
    <key>FREEHAND_DAEMON_ENV_FILE</key>
    <string>$env_file</string>
    <key>HOME</key>
    <string>$HOME</string>
    <key>PATH</key>
    <string>$bin_dir:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    <key>FREEHAND_DAEMON_AGENT</key>
    <string>$agent</string>
    <key>FREEHAND_DAEMON_BIND</key>
    <string>$bind_addr</string>
    <key>FREEHAND_DAEMON_WORKDIR</key>
    <string>$workdir</string>
    <key>FREEHAND_DAEMON_BIN</key>
    <string>$daemon_bin</string>
    <key>FREEHAND_PAIR_TOKEN_SHARED</key>
    <string>$pair_token</string>
  </dict>
  </dict>
</plist>
EOF
}

run_install_launchd() {
  launchctl bootout "gui/$(id -u)" "$plist_path" >/dev/null 2>&1 || true
  launchctl bootstrap "gui/$(id -u)" "$plist_path"
  enable_launchd_service
  wait_for_service

  echo "[freehand-launchd] installed:"
  echo "  label: $label"
  echo "  plist: $plist_path"
  echo "  env: $env_file"
  echo "  logs: $stdout_log"
  echo "  logs: $stderr_log"
  if [[ "$service_role" == "master" ]]; then
    echo "  webui: http://$bind_addr/"
  fi
  echo "[freehand-launchd] status:"
  service_state="$(launchctl print "gui/$(id -u)/$label" | awk '$1 == "state" && $2 == "=" { print $3; exit }')"
  service_pid="$(launchctl print "gui/$(id -u)/$label" | awk '$1 == "pid" && $2 == "=" { print $3; exit }')"
  echo "  state: $service_state"
  echo "  pid: $service_pid"
}

restart_launchd() {
  launchctl bootout "gui/$(id -u)" "$plist_path" >/dev/null 2>&1 || true
  launchctl bootstrap "gui/$(id -u)" "$plist_path"
  enable_launchd_service
  wait_for_service
  echo "[freehand-launchd] restarted $label"
}

enable_launchd_service() {
  if [[ "${FREEHAND_LAUNCHD_SKIP_ENABLE:-0}" == "1" ]]; then
    return 0
  fi
  launchctl enable "gui/$(id -u)/$label"
}

wait_for_service() {
  if [[ "$service_role" == "worker" ]]; then
    wait_for_worker_service
    return 0
  fi

  local health_url="http://$bind_addr/health"
  local attempt=1
  local max_attempts="${FREEHAND_LAUNCHD_HEALTH_WAIT_SECONDS:-60}"

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

wait_for_worker_service() {
  local attempt=1
  local max_attempts="${FREEHAND_LAUNCHD_HEALTH_WAIT_SECONDS:-60}"

  while [[ $attempt -le $max_attempts ]]; do
    service_pid="$(launchctl print "gui/$(id -u)/$label" 2>/dev/null | awk '$1 == "pid" && $2 == "=" { print $3; exit }')"
    if [[ -n "$service_pid" ]] && kill -0 "$service_pid" 2>/dev/null; then
      sleep 1
      stable_pid="$(launchctl print "gui/$(id -u)/$label" 2>/dev/null | awk '$1 == "pid" && $2 == "=" { print $3; exit }')"
      if [[ "$stable_pid" == "$service_pid" ]] && kill -0 "$stable_pid" 2>/dev/null; then
        return 0
      fi
    fi
    sleep 1
    attempt=$((attempt + 1))
  done

  echo "worker service $label did not remain running within ${max_attempts}s" >&2
  exit 1
}

run_file_permission_preflight() {
  FREEHAND_PERMISSION_PREFLIGHT_WORKDIR="$workdir" \
    FREEHAND_RUNTIME_HOME="$runtime_home" \
    scripts/freehand-file-permission-preflight.sh
}

restart_s_profile_relay_if_enabled() {
  if [[ "$service_role" != "master" || "$service_suffix" != "S" ]]; then
    return 0
  fi
  if [[ "${FREEHAND_SKIP_RELAY_S_RESTART:-0}" == "1" ]]; then
    return 0
  fi
  FREEHAND_RELAY_SKIP_BINARY_INSTALL=1 scripts/install-relay-launchd.sh restartS
}

case "$command" in
  install)
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-global.sh
    run_file_permission_preflight
    write_launchd_env
    write_launchd_plist
    run_install_launchd
    ;;
  installS)
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-symlink.sh
    run_file_permission_preflight
    write_launchd_env
    write_launchd_plist
    run_install_launchd
    restart_s_profile_relay_if_enabled
    ;;
  restart)
    run_file_permission_preflight
    write_launchd_env
    write_launchd_plist
    restart_launchd
    ;;
  restartS)
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-symlink.sh
    run_file_permission_preflight
    write_launchd_env
    write_launchd_plist
    restart_launchd
    restart_s_profile_relay_if_enabled
    ;;
  installWorker)
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-global.sh
    run_file_permission_preflight
    write_launchd_env
    write_launchd_plist
    run_install_launchd
    ;;
  installWorkerS)
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-symlink.sh
    run_file_permission_preflight
    write_launchd_env
    write_launchd_plist
    run_install_launchd
    ;;
  restartWorker)
    run_file_permission_preflight
    write_launchd_env
    write_launchd_plist
    restart_launchd
    ;;
  restartWorkerS)
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-symlink.sh
    run_file_permission_preflight
    write_launchd_env
    write_launchd_plist
    restart_launchd
    ;;
esac
