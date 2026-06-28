#!/usr/bin/env bash
set -euo pipefail

env_file="${FREEHAND_DAEMON_ENV_FILE:-"$HOME/.freehand/daemon.env"}"

run_launchd_wrapper() {
  if [[ ! -f "$env_file" ]]; then
    echo "missing daemon env file: $env_file" >&2
    exit 2
  fi

  set -a
  # shellcheck disable=SC1090
  . "$env_file"
  set +a

  : "${FREEHAND_DAEMON_AGENT:?missing FREEHAND_DAEMON_AGENT in $env_file}"
  : "${FREEHAND_DAEMON_BIND:?missing FREEHAND_DAEMON_BIND in $env_file}"
  : "${FREEHAND_DAEMON_WORKDIR:?missing FREEHAND_DAEMON_WORKDIR in $env_file}"
  : "${FREEHAND_DAEMON_BIN:?missing FREEHAND_DAEMON_BIN in $env_file}"

  if [[ ! -d "$FREEHAND_DAEMON_WORKDIR" ]]; then
    echo "missing FREEHAND_DAEMON_WORKDIR: $FREEHAND_DAEMON_WORKDIR" >&2
    exit 2
  fi
  if [[ ! -x "$FREEHAND_DAEMON_BIN" ]]; then
    echo "missing executable FREEHAND_DAEMON_BIN: $FREEHAND_DAEMON_BIN" >&2
    exit 2
  fi

  cd "$FREEHAND_DAEMON_WORKDIR"

  exec "$FREEHAND_DAEMON_BIN" serve --agent "$FREEHAND_DAEMON_AGENT" --bind "$FREEHAND_DAEMON_BIND"
}

run_launchd_wrapper "$@"
