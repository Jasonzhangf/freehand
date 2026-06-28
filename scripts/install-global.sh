#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${FREEHAND_PREFIX:-"$HOME/.local"}"
bin_dir="$prefix/bin"

cd "$repo_root"

run_install_global() {
  scripts/release.sh

  mkdir -p "$bin_dir"
  install -m 0755 dist/bin/freehand-cli "$bin_dir/freehand-cli"
  install -m 0755 dist/bin/freehand-server "$bin_dir/freehand-server"
  install -m 0755 dist/bin/freehand-daemon "$bin_dir/freehand-daemon"
  install -m 0755 scripts/freehand-daemon-launchd.sh "$bin_dir/freehand-daemon-launchd"

  echo "[freehand-install] installed:"
  printf '  %s\n' "$bin_dir/freehand-cli" "$bin_dir/freehand-server" "$bin_dir/freehand-daemon" "$bin_dir/freehand-daemon-launchd"
  echo "[freehand-install] ensure PATH contains: $bin_dir"
  echo "[freehand-install] daemon start: freehand-daemon serve --agent master --bind 127.0.0.1:4041"
}

run_install_global "$@"
