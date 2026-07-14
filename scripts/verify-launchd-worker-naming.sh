#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
fixture_home="$(mktemp -d /tmp/freehand-launchd-worker-naming.XXXXXX)"

cleanup() {
  rm -rf "$fixture_home"
}
trap cleanup EXIT

cd "$repo_root"

plan_worker() {
  local agent="$1"
  HOME="$fixture_home" \
    FREEHAND_PREFIX="$fixture_home/.local" \
    FREEHAND_RUNTIME_HOME="$fixture_home/.freehand" \
    FREEHAND_WORKER_AGENT="$agent" \
    FREEHAND_LAUNCHD_PLAN_ONLY=1 \
    bash scripts/install-launchd.sh installWorkerS
}

plan_value() {
  local plan="$1"
  local key="$2"
  printf '%s\n' "$plan" | awk -F= -v key="$key" '$1 == key { sub($1 "=", ""); print; exit }'
}

assert_expected_plan() {
  local agent="$1"
  local plan="$2"
  local expected_label="com.freehand.workerS.${agent}"
  local expected_env="$fixture_home/.freehand/workerS.${agent}.env"
  local expected_stdout="$fixture_home/.freehand/logs/workerS.${agent}.stdout.log"
  local expected_stderr="$fixture_home/.freehand/logs/workerS.${agent}.stderr.log"

  [[ "$(plan_value "$plan" role)" == "worker" ]]
  [[ "$(plan_value "$plan" agent)" == "$agent" ]]
  [[ "$(plan_value "$plan" label)" == "$expected_label" ]]
  [[ "$(plan_value "$plan" env_file)" == "$expected_env" ]]
  [[ "$(plan_value "$plan" stdout_log)" == "$expected_stdout" ]]
  [[ "$(plan_value "$plan" stderr_log)" == "$expected_stderr" ]]
}

alpha_plan="$(plan_worker worker-alpha)"
beta_plan="$(plan_worker worker-beta)"
gamma_plan="$(plan_worker worker-gamma)"

assert_expected_plan worker-alpha "$alpha_plan"
assert_expected_plan worker-beta "$beta_plan"
assert_expected_plan worker-gamma "$gamma_plan"

for key in label env_file stdout_log stderr_log; do
  alpha_value="$(plan_value "$alpha_plan" "$key")"
  beta_value="$(plan_value "$beta_plan" "$key")"
  gamma_value="$(plan_value "$gamma_plan" "$key")"
  if [[ "$alpha_value" == "$beta_value" || "$alpha_value" == "$gamma_value" || "$beta_value" == "$gamma_value" ]]; then
    echo "launchd Worker plans share $key: alpha=$alpha_value beta=$beta_value gamma=$gamma_value" >&2
    exit 1
  fi
done

echo "launchd_worker_naming_ok agents=worker-alpha,worker-beta,worker-gamma labels=com.freehand.workerS.worker-alpha,com.freehand.workerS.worker-beta,com.freehand.workerS.worker-gamma"
