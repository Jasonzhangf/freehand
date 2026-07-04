#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
prefix="${FREEHAND_PREFIX:-"$HOME/.local"}"
bin_dir="$prefix/bin"

cd "$repo_root"

run_install_symlink() {
  cargo build -p freehand-cli -p freehand-server -p freehand-daemon

  mkdir -p "$bin_dir"
  ln -sfn "$repo_root/target/debug/freehand-cli" "$bin_dir/freehand-cliS"
  ln -sfn "$repo_root/target/debug/freehand-server" "$bin_dir/freehand-serverS"
  ln -sfn "$repo_root/target/debug/freehand-daemon" "$bin_dir/freehand-daemonS"
  install -m 0755 "$repo_root/target/debug/freehand-daemon" "$bin_dir/freehand-daemonS-bin"
  if [[ -L "$bin_dir/freehand-daemon-launchdS" ]]; then
    rm "$bin_dir/freehand-daemon-launchdS"
  fi
  install -m 0755 "$repo_root/scripts/freehand-daemon-launchd.sh" "$bin_dir/freehand-daemon-launchdS"

  echo "[freehand-symlink] installed:"
  printf '  %s -> %s\n' "$bin_dir/freehand-cliS" "$repo_root/target/debug/freehand-cli"
  printf '  %s -> %s\n' "$bin_dir/freehand-serverS" "$repo_root/target/debug/freehand-server"
  printf '  %s -> %s\n' "$bin_dir/freehand-daemonS" "$repo_root/target/debug/freehand-daemon"
  printf '  %s (launchd debug binary copy)\n' "$bin_dir/freehand-daemonS-bin"
  printf '  %s (wrapper copy)\n' "$bin_dir/freehand-daemon-launchdS"
  echo "[freehand-symlink] ensure PATH contains: $bin_dir"
  echo "[freehand-symlink] daemon start: freehand-daemonS serve --agent master --bind 127.0.0.1:4042"
}

run_install_symlink "$@"
