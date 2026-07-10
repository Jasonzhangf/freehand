# Wiki: `runtime.master-worker-loop`

Generated from `docs/mainline-calls/runtime.master-worker-loop.json`. Do not edit by hand.

- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/worker_runner.rs`
- function map: `docs/function-maps/runtime.master-worker-loop.md`
- generated wiki: `docs/wiki/runtime.master-worker-loop.md`
- test design: `docs/testing/runtime.master-worker-loop.md`

## Request Mainline

- one daemon process selects one configured agent
- Slave mode constructs a production Worker runner instead of a Master UI dispatcher
- Worker opens the paired Master's Task Center namespace and uses its own configured agent id as execution identity
- each Worker tick claims the highest-priority Assigned task for that Worker
- claim persists one execution id and lease heartbeat
- task target cwd becomes the canonical locked Worker execution root
- Worker provider requests expose implemented workspace/shell tools but exclude recursive `task`

## Response Mainline

- no Assigned task returns an explicit idle outcome without task mutation
- successful Worker completion writes one review-ready execution fact
- provider/runtime failure writes one blocked execution fact
- Worker reason/session truth persists under Worker agent identity
- task/execution/lease/agent truth persists under the paired Master's Task Center namespace
- periodic Worker ticks continue after idle, success, or blocked outcomes

## Error Mainline

- Master-selected config is rejected by the Worker runner
- missing or invalid target cwd records blocked truth before model execution
- claim or heartbeat persistence failure stops before provider execution
- failure to persist a blocked execution fact remains an explicit runner error
- Worker cannot call recursive `task` through schema or execution policy
- Worker failure never becomes review-ready, approved, closed, or successful UI truth

## Shared Multi-Reference Functions

- `run_live_reason_turn_with_policy`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: share provider/reason loop mechanics while applying explicit Master or Worker tool/workspace policy
  - allowed callers: run_live_reason_turn, run_worker_live_reason_turn, runtime tests
  - related tests: Master boundary tests, Worker tool-policy tests
  - why shared: provider/reason lifecycle must not be copied for Worker execution
- `BuiltinToolRegistry::worker_implemented_definitions`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: expose implemented Worker tools while excluding recursive `task`
  - allowed callers: runtime live bridge, tool-registry tests
  - related tests: Worker schema inclusion/exclusion tests
  - why shared: Worker capability policy must have one registry owner

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ProductionWorkerRunner::from_default_config` | `crates/freehand-runtime/src/worker_runner.rs` | load selected Slave config and bind paired Master Task Center namespace | configured agent name | Worker runner | daemon Slave startup | config and runtime owner | bound |
| 02 | `ProductionWorkerRunner::run` | `crates/freehand-runtime/src/worker_runner.rs` | run periodic Worker ticks with explicit cadence | runner and interval | long-running Worker service | daemon Slave mode | ProductionWorkerRunner::run_once | bound |
| 03 | `ProductionWorkerRunner::run_once` | `crates/freehand-runtime/src/worker_runner.rs` | claim one Assigned task, heartbeat, execute, and report | Task Center and Worker identity | idle, review-ready, or blocked outcome | Worker service loop and tests | task owner and live bridge | bound |
| 04 | `TaskRuntime::claim_next_task` | `crates/freehand-task/src/lib.rs` | choose and claim the highest-priority Assigned task for Worker | Worker id, execution id, and lease TTL | claimed task plus TaskResumed and heartbeat truth | ProductionWorkerRunner::run_once | task owner | bound |
| 05 | `WorkerHeartbeat::start` | `crates/freehand-runtime/src/worker_runner/heartbeat.rs` | renew the claimed task lease while provider execution remains active | claimed task, execution, and Worker identity | periodic TaskHeartbeat truth or explicit heartbeat error | ProductionWorkerRunner::run_once | task owner | bound |
| 06 | `run_worker_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | execute one Worker task in task cwd with Worker tool policy | selected Worker config and live request | closed live reason outcome | ProductionWorkerRunner::run_once | provider/reason live bridge | bound |
| 07 | `TaskRuntime::apply_execution_fact` | `crates/freehand-task/src/lib.rs` | persist review-ready or blocked result for the same execution | typed execution fact | terminal task mutation | ProductionWorkerRunner::run_once | task owner | bound |
| 08 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | select Slave host path without constructing Master UI dispatcher | daemon agent selection | Worker service process | daemon CLI | ProductionWorkerRunner::run | bound |

## Sync Status Against Mainline Call

- Task Center claim, heartbeat, execution fact, persistence, and recovery APIs are already bound
- Master workspace boundary and external-cwd delegation are already bound
- production Worker runner, Worker-specific live tool policy, periodic heartbeat, and Slave daemon startup are code-bound
- deterministic positive and negative tests cover idle, review-ready, blocked, missing workspace, role mismatch, and Worker tool capability boundaries
- generated wiki must be regenerated whenever this mainline changes
