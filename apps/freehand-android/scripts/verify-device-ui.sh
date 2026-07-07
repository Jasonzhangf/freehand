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
artifact_dir="${FREEHAND_ANDROID_ARTIFACT_DIR:-artifacts/android-device/${stamp}-${safe_serial:-no-serial}-$$}"

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

freehand_fatal_logcat_pattern() {
  printf 'Process: %s|%s.*(Exception|Error)|FATAL EXCEPTION.*%s\n' \
    "$package_name" \
    "${package_name//./\\.}" \
    "${package_name//./\\.}"
}

capture_webui_layout_logcat() {
  adb -s "$serial" logcat -d -s FreehandWebUiLayout:I '*:S' 2>/dev/null || true
}

capture_activity_and_window() {
  adb -s "$serial" shell dumpsys activity activities >"$artifact_dir/dumpsys-activity.txt" 2>&1 || true
  adb -s "$serial" shell dumpsys window >"$artifact_dir/dumpsys-window.txt" 2>&1 || true
}

wait_for_package_available() {
  for _ in $(seq 1 "${FREEHAND_ANDROID_PACKAGE_WAIT_SECONDS:-15}"); do
    if adb -s "$serial" shell pm path "$package_name" 2>/dev/null | grep -F "$package_name" >/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

freehand_is_foreground() {
  grep -Eq "topResumedActivity=.*${package_name//./\\.}|ResumedActivity:.*${package_name//./\\.}|mCurrentFocus=.*${package_name//./\\.}|mFocusedApp=.*${package_name//./\\.}" \
    "$artifact_dir/dumpsys-activity.txt" "$artifact_dir/dumpsys-window.txt" 2>/dev/null
}

system_file_picker_is_foreground() {
  grep -Eq 'topResumedActivity=.*(OPEN_DOCUMENT|filemanager|photopicker|PickerActivity)|mCurrentFocus=.*(filemanager|photopicker|PickerActivity)' \
    "$artifact_dir/dumpsys-activity.txt" "$artifact_dir/dumpsys-window.txt" 2>/dev/null
}

wait_for_webui_layout_probe() {
  for _ in $(seq 1 "${FREEHAND_ANDROID_SETTLE_SECONDS:-12}"); do
    sleep 1
    if capture_webui_layout_logcat | grep -F 'FreehandWebUiLayout' >/dev/null; then
      return 0
    fi
  done
  return 1
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
    if ! wait_for_package_available; then
      write_summary "failed" "installed_package_not_available"
      echo "[freehand-android-device] failed: installed package is not available; see $artifact_dir" >&2
      exit 1
    fi
  fi

  adb -s "$serial" logcat -c || true
  adb -s "$serial" shell am force-stop "$package_name" || true
  adb -s "$serial" shell input keyevent KEYCODE_WAKEUP || true
  adb -s "$serial" shell am start -n "${package_name}/${activity_name}" >"$artifact_dir/am-start.txt" 2>&1 || true
  wait_for_webui_layout_probe || true

  capture_activity_and_window
  if ! capture_webui_layout_logcat | grep -F 'FreehandWebUiLayout' >/dev/null && system_file_picker_is_foreground; then
    adb -s "$serial" shell input keyevent KEYCODE_BACK || true
    adb -s "$serial" shell am start -n "${package_name}/${activity_name}" >>"$artifact_dir/am-start.txt" 2>&1 || true
    wait_for_webui_layout_probe || true
    capture_activity_and_window
  fi

  adb -s "$serial" logcat -d >"$artifact_dir/logcat.txt" 2>&1 || true
  capture_webui_layout_logcat >"$artifact_dir/webui-layout-logcat.txt" || true
  adb -s "$serial" exec-out screencap -p >"$artifact_dir/screenshot.png" 2>"$artifact_dir/screencap.stderr" || true

  local fatal_pattern
  fatal_pattern="$(freehand_fatal_logcat_pattern)"
  if grep -E "$fatal_pattern" "$artifact_dir/logcat.txt" >/dev/null; then
    grep -E "$fatal_pattern" "$artifact_dir/logcat.txt" \
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

  if ! freehand_is_foreground; then
    write_summary "blocked" "freehand_activity_not_foreground"
    echo "[freehand-android-device] blocked: Freehand activity is not foreground; see $artifact_dir" >&2
    exit 2
  fi

  if ! grep -F 'FreehandWebUiLayout' "$artifact_dir/webui-layout-logcat.txt" >/dev/null; then
    write_summary "failed" "missing_webui_layout_probe"
    echo "[freehand-android-device] failed: missing WebUI layout probe; see $artifact_dir" >&2
    exit 1
  fi

  if ! grep -Eq '"shape":"(phone_portrait|tall_phone|tablet_portrait)"' "$artifact_dir/webui-layout-logcat.txt"; then
    write_summary "failed" "webui_layout_not_mobile_conversation"
    echo "[freehand-android-device] failed: WebUI did not report mobile conversation layout; see $artifact_dir" >&2
    exit 1
  fi

  if ! grep -Fq '"sessionDrawerFixed":true' "$artifact_dir/webui-layout-logcat.txt" ||
    ! grep -Fq '"detailDrawerFixed":true' "$artifact_dir/webui-layout-logcat.txt" ||
    ! grep -Fq '"sessionDrawerInViewport":false' "$artifact_dir/webui-layout-logcat.txt" ||
    ! grep -Fq '"detailDrawerInViewport":false' "$artifact_dir/webui-layout-logcat.txt"; then
    write_summary "failed" "webui_drawers_not_offscreen"
    echo "[freehand-android-device] failed: WebUI session/detail surfaces are not hidden as offscreen drawers; see $artifact_dir" >&2
    exit 1
  fi

  write_summary "passed" "freehand_activity_foreground_no_fatal_logcat"
  echo "[freehand-android-device] passed: $artifact_dir"
}

verify_device_ui
