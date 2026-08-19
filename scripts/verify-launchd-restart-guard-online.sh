#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
fixture_home="$(mktemp -d /tmp/freehand-launchd-guard-online.XXXXXX)"
runtime_home="$fixture_home/.freehand"
state_dir="$runtime_home/state/launchd"
launchd_wrapper="$fixture_home/freehand-daemon-launchd"
domain="gui/$(id -u)"
suffix="$(date +%s)-$$"
permanent_label="com.freehand.verify.guard.permanent.$suffix"
transient_label="com.freehand.verify.guard.transient.$suffix"
rapid_label="com.freehand.verify.guard.rapid.$suffix"

bootout_label() {
  local label="$1"
  local plist="$2"
  launchctl bootout "$domain" "$plist" >/dev/null 2>&1 || true
  launchctl disable "$domain/$label" >/dev/null 2>&1 || true
}

cleanup() {
  bootout_label "$permanent_label" "$fixture_home/$permanent_label.plist"
  bootout_label "$transient_label" "$fixture_home/$transient_label.plist"
  bootout_label "$rapid_label" "$fixture_home/$rapid_label.plist"
  rm -rf "$fixture_home"
}
trap cleanup EXIT

write_env() {
  local env_file="$1"
  local daemon_bin="$2"
  local count_file="$3"
  cat >"$env_file" <<EOF
FREEHAND_DAEMON_AGENT="master"
FREEHAND_DAEMON_WORKDIR="$fixture_home"
FREEHAND_DAEMON_BIN="$daemon_bin"
FREEHAND_GUARD_COUNT_FILE="$count_file"
EOF
}

write_plist() {
  local label="$1"
  local env_file="$2"
  local max_failures="$3"
  local plist="$fixture_home/$label.plist"
  cat >"$plist" <<EOF
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
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>ThrottleInterval</key>
  <integer>1</integer>
  <key>StandardOutPath</key>
  <string>$fixture_home/$label.stdout.log</string>
  <key>StandardErrorPath</key>
  <string>$fixture_home/$label.stderr.log</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key>
    <string>$fixture_home</string>
    <key>PATH</key>
    <string>/usr/bin:/bin:/usr/sbin:/sbin</string>
    <key>FREEHAND_DAEMON_ENV_FILE</key>
    <string>$env_file</string>
    <key>FREEHAND_LAUNCHD_LABEL</key>
    <string>$label</string>
    <key>FREEHAND_LAUNCHD_STATE_DIR</key>
    <string>$state_dir</string>
    <key>FREEHAND_LAUNCHD_MAX_RAPID_FAILURES</key>
    <string>$max_failures</string>
    <key>FREEHAND_LAUNCHD_FAILURE_WINDOW_SECONDS</key>
    <string>30</string>
    <key>FREEHAND_LAUNCHD_STABLE_RUNTIME_SECONDS</key>
    <string>10</string>
  </dict>
</dict>
</plist>
EOF
  plutil -lint "$plist" >/dev/null
}

service_value() {
  local label="$1"
  local key="$2"
  launchctl print "$domain/$label" 2>/dev/null | awk -v key="$key" '$1 == key && $2 == "=" { print $3; exit }'
}

wait_for_state() {
  local state_file="$1"
  local expected="$2"
  local attempt=1
  while [[ $attempt -le 30 ]]; do
    if [[ -f "$state_file" ]] && [[ "$(plutil -extract status raw -o - "$state_file" 2>/dev/null || true)" == "$expected" ]]; then
      return 0
    fi
    sleep 1
    attempt=$((attempt + 1))
  done
  echo "timed out waiting for $state_file status=$expected" >&2
  return 1
}

mkdir -p "$runtime_home" "$state_dir"
install -m 0755 "$repo_root/scripts/freehand-daemon-launchd.sh" "$launchd_wrapper"

permanent_bin="$fixture_home/permanent-daemon"
cat >"$permanent_bin" <<'EOF'
#!/usr/bin/env bash
exit 78
EOF
chmod 0755 "$permanent_bin"
permanent_env="$runtime_home/permanent.env"
write_env "$permanent_env" "$permanent_bin" "$fixture_home/permanent.count"
write_plist "$permanent_label" "$permanent_env" 3
launchctl bootstrap "$domain" "$fixture_home/$permanent_label.plist"
wait_for_state "$state_dir/$permanent_label.json" blocked
permanent_runs_before="$(service_value "$permanent_label" runs)"
sleep 3
permanent_runs_after="$(service_value "$permanent_label" runs)"
[[ "$permanent_runs_before" == "$permanent_runs_after" ]]
[[ "$permanent_runs_after" == "1" ]]

transient_count="$fixture_home/transient.count"
transient_bin="$fixture_home/transient-daemon"
cat >"$transient_bin" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
count=0
if [[ -f "$FREEHAND_GUARD_COUNT_FILE" ]]; then
  count="$(<"$FREEHAND_GUARD_COUNT_FILE")"
fi
count=$((count + 1))
printf '%s\n' "$count" >"$FREEHAND_GUARD_COUNT_FILE"
if [[ "$count" == "1" ]]; then
  exit 75
fi
exec sleep 30
EOF
chmod 0755 "$transient_bin"
transient_env="$runtime_home/transient.env"
write_env "$transient_env" "$transient_bin" "$transient_count"
write_plist "$transient_label" "$transient_env" 3
launchctl bootstrap "$domain" "$fixture_home/$transient_label.plist"
attempt=1
while [[ $attempt -le 30 ]]; do
  transient_runs="$(service_value "$transient_label" runs)"
  transient_pid="$(service_value "$transient_label" pid)"
  if [[ "${transient_runs:-0}" -ge 2 && -n "$transient_pid" ]] && kill -0 "$transient_pid" 2>/dev/null; then
    break
  fi
  sleep 1
  attempt=$((attempt + 1))
done
[[ "${transient_runs:-0}" -ge 2 ]]
[[ "$(<"$transient_count")" == "2" ]]
[[ "$(plutil -extract status raw -o - "$state_dir/$transient_label.json")" == "running" ]]
[[ "$(plutil -extract consecutive_failures raw -o - "$state_dir/$transient_label.json")" == "1" ]]
transient_daemon_pid="$(plutil -extract daemon_pid raw -o - "$state_dir/$transient_label.json")"
[[ "$transient_daemon_pid" != "$transient_pid" ]]
kill -0 "$transient_daemon_pid" 2>/dev/null

rapid_count="$fixture_home/rapid.count"
rapid_bin="$fixture_home/rapid-daemon"
cat >"$rapid_bin" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
count=0
if [[ -f "$FREEHAND_GUARD_COUNT_FILE" ]]; then
  count="$(<"$FREEHAND_GUARD_COUNT_FILE")"
fi
printf '%s\n' "$((count + 1))" >"$FREEHAND_GUARD_COUNT_FILE"
exit 75
EOF
chmod 0755 "$rapid_bin"
rapid_env="$runtime_home/rapid.env"
write_env "$rapid_env" "$rapid_bin" "$rapid_count"
write_plist "$rapid_label" "$rapid_env" 3
launchctl bootstrap "$domain" "$fixture_home/$rapid_label.plist"
wait_for_state "$state_dir/$rapid_label.json" blocked
rapid_runs_before="$(service_value "$rapid_label" runs)"
sleep 3
rapid_runs_after="$(service_value "$rapid_label" runs)"
[[ "$rapid_runs_before" == "$rapid_runs_after" ]]
[[ "$rapid_runs_after" == "3" ]]
[[ "$(<"$rapid_count")" == "3" ]]

echo "launchd_restart_guard_online_ok permanent_runs=$permanent_runs_after transient_runs=$transient_runs rapid_runs=$rapid_runs_after"
