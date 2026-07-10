#!/usr/bin/env bash
set -euo pipefail

command="${1:-uninstall}"

case "$command" in
  uninstall)
    default_label="com.freehand.daemon"
    ;;
  uninstallS)
    default_label="com.freehand.daemonS"
    ;;
  uninstallWorker)
    default_label="com.freehand.worker"
    ;;
  uninstallWorkerS)
    default_label="com.freehand.workerS"
    ;;
  *)
    echo "usage: $0 [uninstall|uninstallS|uninstallWorker|uninstallWorkerS]" >&2
    exit 2
    ;;
esac

label="${FREEHAND_LAUNCHD_LABEL:-$default_label}"
plist_path="$HOME/Library/LaunchAgents/$label.plist"

run_uninstall_launchd() {
  launchctl bootout "gui/$(id -u)" "$plist_path" >/dev/null 2>&1 || true
  rm -f "$plist_path"

  echo "[freehand-launchd] uninstalled $label"
}

run_uninstall_launchd "$@"
