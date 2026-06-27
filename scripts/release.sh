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
  cp apps/freehand-android/app/build/outputs/apk/release/app-release-unsigned.apk \
    "$dist_dir/android/freehand-android-release-unsigned.apk"

  echo "[freehand-release] artifacts:"
  find "$dist_dir" -type f -maxdepth 3 -print | sort
}

run_release "$@"
