#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

serial="${1:-${FREEHAND_ANDROID_SERIAL:-}}"
package_name="${FREEHAND_ANDROID_PACKAGE:-com.freehand.android}"
activity_name="${FREEHAND_ANDROID_ACTIVITY:-.ui.MainActivity}"
apk_path="${FREEHAND_ANDROID_APK:-apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk}"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
safe_serial="${serial//[^A-Za-z0-9_.-]/_}"
artifact_dir="${FREEHAND_ANDROID_ARTIFACT_DIR:-artifacts/android-device/${stamp}-${safe_serial:-no-serial}}"

usage() {
  cat >&2 <<'USAGE'
usage: apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>

Requires an explicit serial. Does not restart adb, kill processes, unlock the
device, or switch endpoints. It records blocker evidence when the device is
offline, locked, or not running the Freehand activity in foreground.
USAGE
}

write_summary() {
  local status="$1"
  local reason="$2"
  cat >"$artifact_dir/summary.json" <<JSON
{
  "status": "$status",
  "reason": "$reason",
  "serial": "$serial",
  "package": "$package_name",
  "activity": "$activity_name",
  "apk": "$apk_path",
  "artifact_dir": "$artifact_dir"
}
JSON
}

launcher_activity_class() {
  if [[ "$activity_name" == .* ]]; then
    printf '%s%s\n' "$package_name" "$activity_name"
  else
    printf '%s\n' "$activity_name"
  fi
}

verify_apk_contains_activity() {
  if [[ ! -f "$apk_path" || "${FREEHAND_ANDROID_SKIP_INSTALL:-0}" == "1" ]]; then
    return
  fi

  local apkanalyzer="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}/cmdline-tools/latest/bin/apkanalyzer"
  if [[ ! -x "$apkanalyzer" ]]; then
    return
  fi

  local activity_class
  activity_class="$(launcher_activity_class)"
  if ! "$apkanalyzer" dex packages "$apk_path" | grep -Fq "$activity_class"; then
    write_summary "failed" "apk_missing_launcher_activity_class"
    echo "[freehand-android-device] failed: APK missing $activity_class; see $artifact_dir" >&2
    exit 1
  fi
}

verify_device_ui() {
  if [[ -z "$serial" ]]; then
    usage
    exit 64
  fi

  mkdir -p "$artifact_dir"
  adb devices -l >"$artifact_dir/adb-devices.txt" || true

  device_state="$(adb -s "$serial" get-state 2>"$artifact_dir/adb-get-state.stderr" || true)"
  printf '%s\n' "$device_state" >"$artifact_dir/adb-get-state.txt"
  if [[ "$device_state" != "device" ]]; then
    write_summary "blocked" "adb_state_${device_state:-unavailable}"
    echo "[freehand-android-device] blocked: adb state is ${device_state:-unavailable}; see $artifact_dir" >&2
    exit 2
  fi

  if [[ -f "$apk_path" && "${FREEHAND_ANDROID_SKIP_INSTALL:-0}" != "1" ]]; then
    verify_apk_contains_activity
    adb -s "$serial" install -r "$apk_path" >"$artifact_dir/install.txt" 2>&1
  fi

  adb -s "$serial" logcat -c || true
  adb -s "$serial" shell input keyevent KEYCODE_WAKEUP || true
  adb -s "$serial" shell am start -n "${package_name}/${activity_name}" >"$artifact_dir/am-start.txt" 2>&1 || true
  sleep "${FREEHAND_ANDROID_SETTLE_SECONDS:-3}"

  adb -s "$serial" shell dumpsys activity activities >"$artifact_dir/dumpsys-activity.txt" 2>&1 || true
  adb -s "$serial" shell dumpsys window >"$artifact_dir/dumpsys-window.txt" 2>&1 || true
  adb -s "$serial" logcat -d -t 1500 >"$artifact_dir/logcat.txt" 2>&1 || true
  adb -s "$serial" exec-out screencap -p >"$artifact_dir/screenshot.png" 2>"$artifact_dir/screencap.stderr" || true

  if grep -E "AndroidRuntime|FATAL EXCEPTION|${package_name}.*(Exception|Error)" "$artifact_dir/logcat.txt" >/dev/null; then
    grep -E "AndroidRuntime|FATAL EXCEPTION|${package_name}.*(Exception|Error)" "$artifact_dir/logcat.txt" \
      >"$artifact_dir/fatal-logcat.txt" || true
    write_summary "failed" "fatal_or_exception_logcat"
    echo "[freehand-android-device] failed: fatal/exception logcat found; see $artifact_dir" >&2
    exit 1
  fi

  if grep -Eq 'mDreamingLockscreen=true|mShowingLockscreen=true' "$artifact_dir/dumpsys-window.txt"; then
    write_summary "blocked" "device_locked_or_dreaming"
    echo "[freehand-android-device] blocked: device is locked/dozing; see $artifact_dir" >&2
    exit 2
  fi

  if ! grep -Eq "${package_name}/|${package_name//./\\.}" "$artifact_dir/dumpsys-window.txt" "$artifact_dir/dumpsys-activity.txt"; then
    write_summary "blocked" "freehand_activity_not_foreground"
    echo "[freehand-android-device] blocked: Freehand activity is not foreground; see $artifact_dir" >&2
    exit 2
  fi

  write_summary "passed" "freehand_activity_foreground_no_fatal_logcat"
  echo "[freehand-android-device] passed: $artifact_dir"
}

verify_device_ui
