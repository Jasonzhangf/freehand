#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${FREEHAND_PREFIX:-"$HOME/.local"}"
bin_dir="$prefix/bin"
runtime_home="${FREEHAND_RUNTIME_HOME:-"$HOME/.freehand"}"
logs_dir="$runtime_home/logs"
env_file="$runtime_home/daemon.env"
label="${FREEHAND_LAUNCHD_LABEL:-com.freehand.daemon}"
plist_path="$HOME/Library/LaunchAgents/$label.plist"
agent="${FREEHAND_DAEMON_AGENT:-master}"
bind_addr="${FREEHAND_DAEMON_BIND:-127.0.0.1:4041}"
workdir="${FREEHAND_DAEMON_WORKDIR:-"$repo_root"}"
pair_token="${FREEHAND_PAIR_TOKEN_SHARED:-}"

cd "$repo_root"

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
FREEHAND_DAEMON_AGENT="$agent"
FREEHAND_DAEMON_BIND="$bind_addr"
FREEHAND_DAEMON_WORKDIR="$workdir"
FREEHAND_PAIR_TOKEN_SHARED="$pair_token"
PATH="$bin_dir:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
EOF
    chmod 0600 "$env_file"
  else
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
    <string>$bin_dir/freehand-daemon-launchd</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>WorkingDirectory</key>
  <string>$workdir</string>
  <key>StandardOutPath</key>
  <string>$logs_dir/daemon.stdout.log</string>
  <key>StandardErrorPath</key>
  <string>$logs_dir/daemon.stderr.log</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>FREEHAND_DAEMON_ENV_FILE</key>
    <string>$env_file</string>
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

  echo "[freehand-launchd] installed:"
  echo "  label: $label"
  echo "  plist: $plist_path"
  echo "  env: $env_file"
  echo "  logs: $logs_dir/daemon.stdout.log"
  echo "  logs: $logs_dir/daemon.stderr.log"
  echo "  webui: http://$bind_addr/"
  echo "[freehand-launchd] status:"
  launchctl print "gui/$(id -u)/$label" | sed -n '1,80p'
}

restart_launchd() {
  launchctl kickstart -k "gui/$(id -u)/$label"
  echo "[freehand-launchd] restarted $label"
}

case "${1:-install}" in
  install)
    scripts/install-global.sh
    write_launchd_env
    write_launchd_plist
    run_install_launchd
    ;;
  restart)
    restart_launchd
    ;;
  *)
    echo "usage: $0 [install|restart]" >&2
    exit 2
    ;;
esac
