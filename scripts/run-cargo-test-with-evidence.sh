#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
timeout_bin="${FREEHAND_CARGO_TEST_TIMEOUT_BIN:-}"
timeout_seconds="${FREEHAND_CARGO_TEST_TIMEOUT_SECONDS:-180}"
log_dir="${FREEHAND_CARGO_TEST_LOG_DIR:-/tmp}"
stamp="$(date +%Y%m%dT%H%M%S)"
stdout_log="$log_dir/freehand-cargo-test-$stamp.out"
stderr_log="$log_dir/freehand-cargo-test-$stamp.err"

usage() {
  cat >&2 <<'EOF'
usage: scripts/run-cargo-test-with-evidence.sh -- <cargo test args...>

Runs `cargo test` with a bounded timeout and prints deterministic evidence:
exit code, stdout log path, stderr log path, and captured output.

Example:
  scripts/run-cargo-test-with-evidence.sh -- -p freehand-task execution_fact_interrupted_marks_task_retryable_without_blocked_truth -- --nocapture
EOF
}

if [[ "${1:-}" != "--" ]]; then
  usage
  exit 2
fi
shift
if [[ "$#" -eq 0 ]]; then
  usage
  exit 2
fi

if [[ -z "$timeout_bin" ]]; then
  if command -v timeout >/dev/null 2>&1; then
    timeout_bin="$(command -v timeout)"
  elif command -v gtimeout >/dev/null 2>&1; then
    timeout_bin="$(command -v gtimeout)"
  else
    echo "missing timeout command; install coreutils or set FREEHAND_CARGO_TEST_TIMEOUT_BIN" >&2
    exit 2
  fi
fi

mkdir -p "$log_dir"
cd "$repo_root"

set +e
"$timeout_bin" "${timeout_seconds}s" cargo test "$@" >"$stdout_log" 2>"$stderr_log"
status="$?"
set -e

echo "freehand_cargo_test_evidence status=$status timeout_seconds=$timeout_seconds stdout=$stdout_log stderr=$stderr_log"
echo "--- stdout ---"
sed -n '1,240p' "$stdout_log"
echo "--- stderr ---"
sed -n '1,240p' "$stderr_log"

if [[ "$status" -eq 124 ]]; then
  echo "freehand_cargo_test_timeout status=124 command=cargo test $*" >&2
fi
exit "$status"
