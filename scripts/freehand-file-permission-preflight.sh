#!/usr/bin/env bash
set -euo pipefail

runtime_home="${FREEHAND_RUNTIME_HOME:-"$HOME/.freehand"}"
workdir="${FREEHAND_PERMISSION_PREFLIGHT_WORKDIR:-${FREEHAND_DAEMON_WORKDIR:-$runtime_home}}"
mode="${FREEHAND_FILE_PERMISSION_PREFLIGHT:-enforce}"
status_dir="$runtime_home/state"
status_path="$status_dir/file-permission-preflight.json"

json_escape() {
  printf '%s' "$1" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'
}

write_status() {
  local status="$1"
  local message="$2"
  mkdir -p "$status_dir"
  {
    printf '{\n'
    printf '  "status": %s,\n' "$(json_escape "$status")"
    printf '  "message": %s,\n' "$(json_escape "$message")"
    printf '  "runtime_home": %s,\n' "$(json_escape "$runtime_home")"
    printf '  "workdir": %s,\n' "$(json_escape "$workdir")"
    printf '  "checked_at_unix": %s\n' "$(date +%s)"
    printf '}\n'
  } >"$status_path"
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  write_status "skipped" "macOS file permission preflight skipped on non-Darwin host"
  exit 0
fi

declare -a candidate_paths=(
  "$runtime_home"
  "$workdir"
  "$HOME/Documents"
  "$HOME/Desktop"
  "$HOME/Downloads"
)

if [[ -n "${FREEHAND_PERMISSION_PREFLIGHT_PATHS:-}" ]]; then
  IFS=':' read -r -a extra_paths <<<"$FREEHAND_PERMISSION_PREFLIGHT_PATHS"
  for extra_path in "${extra_paths[@]}"; do
    if [[ -n "$extra_path" ]]; then
      candidate_paths+=("$extra_path")
    fi
  done
fi

mkdir -p "$runtime_home" "$status_dir"

declare -a denied=()
declare -a missing=()
for path in "${candidate_paths[@]}"; do
  if [[ -z "$path" ]]; then
    continue
  fi
  if [[ ! -e "$path" ]]; then
    missing+=("$path")
    continue
  fi
  if ! /bin/ls -ld "$path" >/dev/null 2>&1; then
    denied+=("$path")
    continue
  fi
  if [[ -d "$path" && -w "$path" ]]; then
    probe="$path/.freehand-permission-preflight.$$"
    if ! (: >"$probe") >/dev/null 2>&1; then
      denied+=("$path")
    else
      rm -f "$probe"
    fi
  fi
done

if [[ ${#denied[@]} -gt 0 ]]; then
  message="Freehand cannot access required macOS file locations. Grant Full Disk Access to the terminal/launchd host, then rerun install/restart. Denied: ${denied[*]}"
  write_status "denied" "$message"
  if command -v open >/dev/null 2>&1; then
    open "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles" >/dev/null 2>&1 || true
  fi
  echo "[freehand-permission] $message" >&2
  if [[ "$mode" == "warn" ]]; then
    exit 0
  fi
  exit 1
fi

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "[freehand-permission] skipped missing optional paths: ${missing[*]}" >&2
fi

write_status "ok" "macOS file permission preflight passed"
echo "[freehand-permission] preflight ok; status=$status_path"
