# Function Map: `runtime.master-worker-loop`

- feature_id: `runtime.master-worker-loop`
- owner crate: `crates/freehand-runtime`
- owner modules:
  - `crates/freehand-runtime/src/worker_runner.rs`
  - `crates/freehand-runtime/src/worker_runner/heartbeat.rs`
  - `crates/freehand-runtime/src/worker_runner/tests.rs`
  - `crates/freehand-runtime/src/lib.rs`
- host wiring: `apps/freehand-daemon/src/main.rs`
- task truth dependency: `crates/freehand-task`
- config truth dependency: `crates/freehand-config`
- tool policy dependency: `crates/freehand-tools`
- mainline call source: `docs/mainline-calls/runtime.master-worker-loop.json`
- generated wiki: `docs/wiki/runtime.master-worker-loop.md`
- owner entry symbols:
  - `ProductionWorkerRunner::from_default_config`
  - `ProductionWorkerRunner::run_once`
  - `ProductionWorkerRunner::run`
  - `run_worker_live_reason_turn`

## Request Mainline

- one daemon process selects one configured agent
- Master mode continues to create and assign tasks through the existing runtime/UI command path
- Slave mode constructs a production Worker runner instead of a Master UI dispatcher
- Worker opens the paired Master's Task Center namespace and uses its own configured agent id as execution identity
- each worker tick queries the highest-priority Assigned task for that worker
- a selected task is claimed with one execution id and lease heartbeat
- the task target cwd is canonicalized and becomes the worker's locked execution root
- worker live reasoning receives task goal, content, deliverables, and acceptance criteria
- worker provider requests expose implemented workspace/shell tools but exclude recursive `task`

## Response Mainline

- no Assigned task returns an explicit idle outcome without task mutation
- claim persists `TaskResumed` and `TaskHeartbeat` before provider execution
- successful worker completion writes one `ExecutionFactKind::ReviewReady`
- provider/runtime/tool-loop terminal failure writes one `ExecutionFactKind::Blocked`
- worker reason/session truth remains persisted under worker agent identity
- task/execution/lease/agent truth remains persisted under the paired Master's Task Center namespace
- periodic runner ticks continue after idle, success, or blocked outcomes

## Error Mainline

- Master-selected config is rejected by the Worker runner
- missing paired Master identity or invalid provider config blocks runner bootstrap
- missing or non-canonicalizable target cwd records blocked task truth before model execution
- claim/heartbeat persistence failure returns an explicit runner error and does not start the model
- provider/runtime failure is paired to the claimed task/execution as blocked truth
- failure to persist the blocked fact is returned as a combined explicit runner error
- worker cannot call `task`; schema excludes it and execution policy rejects it if received
- Worker failure never becomes `review_ready`, approved, closed, or successful UI truth

## Shared Multi-Reference Functions

- `run_live_reason_turn_with_policy`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: share provider/reason loop mechanics while applying an explicit Master or Worker tool/workspace policy
  - allowed callers: `run_live_reason_turn`, `run_worker_live_reason_turn`, runtime tests
  - related tests: Master boundary tests, Worker tool-policy tests
  - why shared: provider/reason lifecycle must not be copied for Worker execution
- `BuiltinToolRegistry::worker_implemented_definitions`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: expose implemented Worker tools while physically excluding recursive `task`
  - allowed callers: runtime live bridge and tool-registry tests
  - related tests: Worker schema inclusion/exclusion tests
  - why shared: Worker capability policy must have one registry owner

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ProductionWorkerRunner::from_default_config` | `crates/freehand-runtime/src/worker_runner.rs` | load selected Slave config and bind paired Master Task Center namespace | configured agent name | Worker runner | daemon Slave startup | config + runtime owner | bound |
| 02 | `ProductionWorkerRunner::run` | `crates/freehand-runtime/src/worker_runner.rs` | run periodic Worker ticks with explicit cadence | runner + interval | long-running Worker service | daemon Slave mode | `run_once` | bound |
| 03 | `ProductionWorkerRunner::run_once` | `crates/freehand-runtime/src/worker_runner.rs` | claim one Assigned task, heartbeat, execute, and report | Task Center + Worker identity | idle/review-ready/blocked outcome | Worker service loop/tests | task owner + live bridge | bound |
| 04 | `TaskRuntime::claim_next_task` | `crates/freehand-task/src/lib.rs` | choose and claim highest-priority Assigned task for Worker | worker id + execution id + lease TTL | claimed task + TaskResumed/heartbeat truth | Worker runner | task owner | bound |
| 05 | `WorkerHeartbeat::start` | `crates/freehand-runtime/src/worker_runner/heartbeat.rs` | renew the claimed task lease while provider execution remains active | claimed task/execution/worker identity | periodic TaskHeartbeat truth or explicit heartbeat error | `ProductionWorkerRunner::run_once` | task owner | bound |
| 06 | `run_worker_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | execute one worker task in task cwd with Worker tool policy | selected Worker config + live request | closed live reason outcome | Worker runner | provider/reason live bridge | bound |
| 07 | `TaskRuntime::apply_execution_fact` | `crates/freehand-task/src/lib.rs` | persist review-ready or blocked result for same execution | typed execution fact | terminal task mutation | Worker runner | task owner | bound |
| 08 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | select Slave host path without constructing Master UI dispatcher | daemon agent selection | Worker service process | daemon CLI | runtime Worker runner | bound |

## Sync Status Against Code

- Task Center claim, heartbeat, execution fact, persistence, and recovery APIs are already bound
- Master workspace boundary and external-cwd delegation are already bound
- production Worker runner, Worker-specific live tool policy, periodic heartbeat, and Slave daemon startup are code-bound
- deterministic positive/negative tests cover idle, review-ready, blocked, missing workspace, role mismatch, and Worker tool capability boundaries
- generated wiki must be regenerated whenever this mainline changes
