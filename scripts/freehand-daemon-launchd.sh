#!/usr/bin/env bash
set -euo pipefail

env_file="${FREEHAND_DAEMON_ENV_FILE:-"$HOME/.freehand/daemon.env"}"
label="${FREEHAND_LAUNCHD_LABEL:-com.freehand.daemon}"
state_dir="${FREEHAND_LAUNCHD_STATE_DIR:-"$HOME/.freehand/state/launchd"}"
state_file="$state_dir/$label.json"
max_rapid_failures="${FREEHAND_LAUNCHD_MAX_RAPID_FAILURES:-5}"
failure_window_seconds="${FREEHAND_LAUNCHD_FAILURE_WINDOW_SECONDS:-300}"
stable_runtime_seconds="${FREEHAND_LAUNCHD_STABLE_RUNTIME_SECONDS:-60}"
child_pid=""
shutting_down=0

validate_positive_integer() {
  local name="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[1-9][0-9]*$ ]]; then
    block_startup "invalid_${name}"
  fi
}

state_value() {
  local key="$1"
  /usr/bin/plutil -extract "$key" raw -o - "$state_file" 2>/dev/null
}

write_state() {
  local status="$1"
  local failure_class="$2"
  local reason="$3"
  local failures="$4"
  local updated_epoch="$5"
  local daemon_pid="${6:-}"
  local tmp_file
  if ! mkdir -p "$state_dir"; then
    return 1
  fi
  if ! tmp_file="$(mktemp "$state_dir/.${label}.XXXXXX")"; then
    return 1
  fi
  if ! /usr/bin/plutil -create xml1 "$tmp_file" \
    || ! /usr/bin/plutil -insert schema_version -integer 1 "$tmp_file" \
    || ! /usr/bin/plutil -insert label -string "$label" "$tmp_file" \
    || ! /usr/bin/plutil -insert status -string "$status" "$tmp_file" \
    || ! /usr/bin/plutil -insert failure_class -string "$failure_class" "$tmp_file" \
    || ! /usr/bin/plutil -insert reason -string "$reason" "$tmp_file" \
    || ! /usr/bin/plutil -insert consecutive_failures -integer "$failures" "$tmp_file" \
    || ! /usr/bin/plutil -insert updated_epoch -integer "$updated_epoch" "$tmp_file"; then
    rm -f "$tmp_file"
    return 1
  fi
  if [[ -n "$daemon_pid" ]]; then
    if ! /usr/bin/plutil -insert daemon_pid -integer "$daemon_pid" "$tmp_file"; then
      rm -f "$tmp_file"
      return 1
    fi
  fi
  if ! /usr/bin/plutil -convert json "$tmp_file" \
    || ! chmod 0600 "$tmp_file" \
    || ! mv "$tmp_file" "$state_file"; then
    rm -f "$tmp_file"
    return 1
  fi
}

block_startup() {
  local reason="$1"
  echo "freehand launchd startup blocked: $reason" >&2
  if ! write_state "blocked" "permanent_startup" "$reason" 1 "$(date +%s)"; then
    echo "failed to persist launchd blocked state: $state_file" >&2
  fi
  exit 0
}

block_after_exit() {
  local failure_class="$1"
  local reason="$2"
  local failures="$3"
  local updated_epoch="$4"
  echo "freehand launchd retry blocked: $reason" >&2
  if ! write_state "blocked" "$failure_class" "$reason" "$failures" "$updated_epoch"; then
    echo "failed to persist launchd blocked state: $state_file" >&2
  fi
  exit 0
}

forward_shutdown() {
  shutting_down=1
  if [[ -n "$child_pid" ]] && kill -0 "$child_pid" 2>/dev/null; then
    kill -TERM "$child_pid"
  fi
}

trap forward_shutdown TERM INT

run_launchd_wrapper() {
  validate_positive_integer "FREEHAND_LAUNCHD_MAX_RAPID_FAILURES" "$max_rapid_failures"
  validate_positive_integer "FREEHAND_LAUNCHD_FAILURE_WINDOW_SECONDS" "$failure_window_seconds"
  validate_positive_integer "FREEHAND_LAUNCHD_STABLE_RUNTIME_SECONDS" "$stable_runtime_seconds"

  if [[ -f "$state_file" ]]; then
    local existing_status
    if ! existing_status="$(state_value status)"; then
      echo "invalid launchd guard state; refusing daemon spawn: $state_file" >&2
      block_after_exit "guard_state" "invalid_state" 1 "$(date +%s)"
    fi
    if [[ "$existing_status" == "blocked" ]]; then
      echo "launchd guard is blocked; run the explicit installer restart after fixing configuration: $state_file" >&2
      exit 0
    fi
  fi

  if [[ ! -f "$env_file" ]]; then
    block_startup "missing_env_file"
  fi

  set +e
  set -a
  # shellcheck disable=SC1090
  . "$env_file"
  local source_status=$?
  set +a
  set -e
  if [[ "$source_status" != "0" ]]; then
    block_startup "invalid_env_file"
  fi

  if [[ -z "${FREEHAND_DAEMON_AGENT:-}" ]]; then
    block_startup "missing_agent"
  fi
  if [[ -z "${FREEHAND_DAEMON_WORKDIR:-}" ]]; then
    block_startup "missing_workdir"
  fi
  if [[ -z "${FREEHAND_DAEMON_BIN:-}" ]]; then
    block_startup "missing_binary_path"
  fi

  if [[ ! -d "$FREEHAND_DAEMON_WORKDIR" ]]; then
    block_startup "workdir_not_found"
  fi
  if [[ ! -x "$FREEHAND_DAEMON_BIN" ]]; then
    block_startup "binary_not_executable"
  fi

  cd "$FREEHAND_DAEMON_WORKDIR"

  local started_epoch ended_epoch runtime_seconds child_status
  local previous_failures=0 previous_updated=0 failures=1
  local daemon_args=(serve --agent "$FREEHAND_DAEMON_AGENT")
  if [[ -n "${FREEHAND_DAEMON_BIND:-}" ]]; then
    daemon_args+=(--bind "$FREEHAND_DAEMON_BIND")
  fi

  if [[ -f "$state_file" ]]; then
    previous_failures="$(state_value consecutive_failures 2>/dev/null || printf '0')"
    previous_updated="$(state_value updated_epoch 2>/dev/null || printf '0')"
    if [[ ! "$previous_failures" =~ ^[0-9]+$ || ! "$previous_updated" =~ ^[0-9]+$ ]]; then
      echo "invalid launchd retry state; refusing daemon spawn: $state_file" >&2
      block_after_exit "guard_state" "invalid_retry_state" 1 "$(date +%s)"
    fi
  fi

  started_epoch="$(date +%s)"
  set +e
  "$FREEHAND_DAEMON_BIN" "${daemon_args[@]}" &
  child_pid=$!
  if ! write_state "running" "none" "daemon_started" "$previous_failures" "$previous_updated" "$child_pid"; then
    echo "failed to persist running launchd state; stopping daemon child: $state_file" >&2
    forward_shutdown
    wait "$child_pid" >/dev/null 2>&1 || true
    child_pid=""
    exit 0
  fi
  wait "$child_pid"
  child_status=$?
  if [[ "$shutting_down" == "1" ]] && kill -0 "$child_pid" 2>/dev/null; then
    wait "$child_pid"
    child_status=$?
  fi
  set -e
  child_pid=""
  ended_epoch="$(date +%s)"
  runtime_seconds=$((ended_epoch - started_epoch))

  if [[ "$shutting_down" == "1" || "$child_status" == "0" ]]; then
    rm -f "$state_file"
    exit 0
  fi

  if [[ "$child_status" == "78" ]]; then
    block_after_exit "permanent_startup" "daemon_exit_78" 1 "$ended_epoch"
  fi

  if [[ "$runtime_seconds" -lt "$stable_runtime_seconds" ]]; then
    if ((ended_epoch - previous_updated <= failure_window_seconds)); then
      failures=$((previous_failures + 1))
    fi
  fi

  if ((failures >= max_rapid_failures)); then
    block_after_exit "transient_runtime" "rapid_failure_limit" "$failures" "$ended_epoch"
  fi

  if ! write_state "retrying" "transient_runtime" "daemon_exit_nonzero" "$failures" "$ended_epoch"; then
    echo "failed to persist launchd retry state; refusing another automatic restart: $state_file" >&2
    exit 0
  fi
  exit "$child_status"
}

run_launchd_wrapper "$@"
