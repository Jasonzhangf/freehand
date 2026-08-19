#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
fixture_home="$(mktemp -d /tmp/freehand-launchd-guard.XXXXXX)"
runtime_home="$fixture_home/.freehand"
state_dir="$runtime_home/state/launchd"
env_file="$runtime_home/daemon.env"
daemon_bin="$fixture_home/fake-daemon"
label="com.freehand.verify.guard.unit.$$"
state_file="$state_dir/$label.json"
count_file="$fixture_home/count"

cleanup() {
  rm -rf "$fixture_home"
}
trap cleanup EXIT

run_wrapper() {
  HOME="$fixture_home" \
    FREEHAND_DAEMON_ENV_FILE="$env_file" \
    FREEHAND_LAUNCHD_LABEL="$label" \
    FREEHAND_LAUNCHD_STATE_DIR="$state_dir" \
    FREEHAND_LAUNCHD_MAX_RAPID_FAILURES=3 \
    FREEHAND_LAUNCHD_FAILURE_WINDOW_SECONDS=30 \
    FREEHAND_LAUNCHD_STABLE_RUNTIME_SECONDS=10 \
    bash "$repo_root/scripts/freehand-daemon-launchd.sh"
}

run_wrapper_with_invalid_limit() {
  HOME="$fixture_home" \
    FREEHAND_DAEMON_ENV_FILE="$env_file" \
    FREEHAND_LAUNCHD_LABEL="$label" \
    FREEHAND_LAUNCHD_STATE_DIR="$state_dir" \
    FREEHAND_LAUNCHD_MAX_RAPID_FAILURES=invalid \
    bash "$repo_root/scripts/freehand-daemon-launchd.sh"
}

state_value() {
  /usr/bin/plutil -extract "$1" raw -o - "$state_file"
}

mkdir -p "$runtime_home"
run_wrapper
[[ "$(state_value status)" == "blocked" ]]
[[ "$(state_value failure_class)" == "permanent_startup" ]]
[[ "$(state_value reason)" == "missing_env_file" ]]

rm -f "$state_file"
run_wrapper_with_invalid_limit
[[ "$(state_value status)" == "blocked" ]]
[[ "$(state_value failure_class)" == "permanent_startup" ]]
[[ "$(state_value reason)" == "invalid_FREEHAND_LAUNCHD_MAX_RAPID_FAILURES" ]]

cat >"$daemon_bin" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
count=0
if [[ -f "$FREEHAND_GUARD_COUNT_FILE" ]]; then
  count="$(<"$FREEHAND_GUARD_COUNT_FILE")"
fi
printf '%s\n' "$((count + 1))" >"$FREEHAND_GUARD_COUNT_FILE"
exit 75
EOF
chmod 0755 "$daemon_bin"
cat >"$env_file" <<EOF
FREEHAND_DAEMON_AGENT="master"
FREEHAND_DAEMON_WORKDIR="$fixture_home"
FREEHAND_DAEMON_BIN="$daemon_bin"
FREEHAND_GUARD_COUNT_FILE="$count_file"
EOF

run_wrapper
[[ ! -e "$count_file" ]]

rm -f "$state_file"
set +e
run_wrapper
first_status=$?
set -e
[[ "$first_status" == "75" ]]
[[ "$(state_value status)" == "retrying" ]]
[[ "$(state_value consecutive_failures)" == "1" ]]

set +e
run_wrapper
second_status=$?
set -e
[[ "$second_status" == "75" ]]
[[ "$(state_value consecutive_failures)" == "2" ]]

run_wrapper
[[ "$(state_value status)" == "blocked" ]]
[[ "$(state_value failure_class)" == "transient_runtime" ]]
[[ "$(state_value reason)" == "rapid_failure_limit" ]]
[[ "$(state_value consecutive_failures)" == "3" ]]
[[ "$(<"$count_file")" == "3" ]]

echo "launchd_restart_guard_ok permanent_plateau=true rapid_failure_limit=3"
