#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dist_dir="${FREEHAND_DIST_DIR:-"$repo_root/dist"}"

cd "$repo_root"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 2
  fi
}

find_aapt() {
  if command -v aapt >/dev/null 2>&1; then
    command -v aapt
    return 0
  fi
  if [[ -n "${ANDROID_HOME:-}" ]]; then
    find "$ANDROID_HOME/build-tools" -name aapt -type f 2>/dev/null | sort | tail -n 1
    return 0
  fi
  return 1
}

find_apksigner() {
  if command -v apksigner >/dev/null 2>&1; then
    command -v apksigner
    return 0
  fi
  if [[ -n "${ANDROID_HOME:-}" ]]; then
    find "$ANDROID_HOME/build-tools" -name apksigner -type f 2>/dev/null | sort | tail -n 1
    return 0
  fi
  return 1
}

verify_android_apk_signature() {
  local apk_path="$1"
  local apksigner_bin
  apksigner_bin="$(find_apksigner)"
  if [[ -z "$apksigner_bin" ]]; then
    echo "missing required command: apksigner; set ANDROID_HOME or install Android build-tools" >&2
    exit 2
  fi
  "$apksigner_bin" verify --verbose "$apk_path" >/dev/null
}

write_android_update_manifest() {
  local apk_path="$1"
  local manifest_path="$2"
  local aapt_bin
  aapt_bin="$(find_aapt)"
  if [[ -z "$aapt_bin" ]]; then
    echo "missing required command: aapt; set ANDROID_HOME or install Android build-tools" >&2
    exit 2
  fi

  local package_line version_code version_name
  package_line="$("$aapt_bin" dump badging "$apk_path" | sed -n '1p')"
  version_code="$(printf '%s\n' "$package_line" | sed -n "s/.*versionCode='\([^']*\)'.*/\1/p")"
  version_name="$(printf '%s\n' "$package_line" | sed -n "s/.*versionName='\([^']*\)'.*/\1/p")"
  if [[ ! "$version_code" =~ ^[1-9][0-9]*$ ]]; then
    echo "failed to extract positive Android versionCode from $apk_path" >&2
    exit 2
  fi
  if [[ -z "$version_name" || "$version_name" =~ [^A-Za-z0-9._+-] ]]; then
    echo "failed to extract safe Android versionName from $apk_path" >&2
    exit 2
  fi

  cat >"$manifest_path" <<EOF
{"versionCode":$version_code,"versionName":"$version_name","apkUrl":"/android/freehand-android.apk","releaseNotes":"Freehand Android release artifact served by the current daemon.","required":false}
EOF
  echo "[freehand-release] android update manifest versionCode=$version_code versionName=$version_name"
}

run_release() {
  require_cmd cargo
  require_cmd make

  if [[ -n "${JAVA_HOME:-}" ]]; then
    export PATH="$JAVA_HOME/bin:$PATH"
  fi
  require_cmd java

  echo "[freehand-release] rust full regression"
  make ci

  echo "[freehand-release] android JVM regression"
  (
    cd apps/freehand-android
    ./gradlew testDebugUnitTest --no-daemon
  )

  echo "[freehand-release] rust release binaries"
  cargo build --release -p freehand-cli -p freehand-server -p freehand-daemon

  echo "[freehand-release] android release apk"
  (
    cd apps/freehand-android
    ./gradlew assembleRelease --no-daemon
  )

  rm -rf "$dist_dir"
  mkdir -p "$dist_dir/bin" "$dist_dir/android"

  cp target/release/freehand-cli "$dist_dir/bin/freehand-cli"
  cp target/release/freehand-server "$dist_dir/bin/freehand-server"
  cp target/release/freehand-daemon "$dist_dir/bin/freehand-daemon"
  cp apps/freehand-android/app/build/outputs/apk/release/app-release.apk \
    "$dist_dir/android/freehand-android-release.apk"
  verify_android_apk_signature "$dist_dir/android/freehand-android-release.apk"
  write_android_update_manifest \
    "$dist_dir/android/freehand-android-release.apk" \
    "$dist_dir/android/update.json"

  echo "[freehand-release] artifacts:"
  find "$dist_dir" -type f -maxdepth 3 -print | sort
}

run_release "$@"
