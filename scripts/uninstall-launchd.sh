#!/usr/bin/env bash
set -euo pipefail

label="${FREEHAND_LAUNCHD_LABEL:-com.freehand.daemon}"
plist_path="$HOME/Library/LaunchAgents/$label.plist"

run_uninstall_launchd() {
  launchctl bootout "gui/$(id -u)" "$plist_path" >/dev/null 2>&1 || true
  rm -f "$plist_path"

  echo "[freehand-launchd] uninstalled $label"
}

run_uninstall_launchd "$@"
