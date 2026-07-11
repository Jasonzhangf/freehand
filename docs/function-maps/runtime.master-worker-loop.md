# Function Map: `runtime.master-worker-loop`

- feature_id: `runtime.master-worker-loop`
- owner crate: `crates/freehand-runtime`
- owner modules:
  - `crates/freehand-runtime/src/master_runner.rs`
  - `crates/freehand-runtime/src/master_runner/tests.rs`
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
- Master mode starts the runtime/UI command path and a background lifecycle
  runner over Task Center EventInbox truth
- the Master lifecycle runner keeps a durable event cursor and invokes the
  Master model only for current review-ready, blocked, or interrupted truth
- each actionable event attempt uses the task-scoped internal
  `master-lifecycle-<task>` reason session, plus deterministic
  event-and-attempt-isolated turn and trace ids, and an explicit target-task
  decision boundary
- Slave mode constructs a production Worker runner instead of a Master UI dispatcher
- Worker opens the paired Master's Task Center namespace and uses its own configured agent id as execution identity
- each worker tick queries the highest-priority Assigned task for that worker
- a selected task is claimed with one execution id and lease heartbeat
- the task target cwd expands a leading `~`, canonicalizes through symlinks, and becomes the worker's locked execution root
- worker live reasoning receives task goal, content, deliverables, acceptance criteria, the requested `target_cwd`, the canonical locked workspace, and path-preflight instructions
- worker provider requests expose governed workspace tools but exclude recursive `task` and unrestricted shell tools
- Master provider guidance binds dispatch to the configured paired Worker id,
  rejects historical AgentBoard entries as production dispatch targets, and
  forbids putting `task(...)` lifecycle instructions into Worker task content
- Master provider guidance tells the model to preserve user-supplied paths,
  avoid repeated Master-side probes outside runtime home, require Worker
  symlink/canonical-path evidence, and never invent `/workspace`, `/tmp`, or
  sibling output dirs when the user supplied a repository path
- Master task-tool execution independently enforces the configured paired
  Worker id; a non-configured assignment becomes a paired failed tool result
  and cannot mutate Task Center truth
- Master task creation independently rejects omitted, auto, self, or
  non-configured-agent dispatch so persisted historical agents cannot be
  selected before the explicit assignment gate

## Response Mainline

- no Assigned task returns an explicit idle outcome without task mutation
- Master review-ready handling must move the task to rejected or
  approved/closed before its event cursor advances
- once the event-specific target-task decision boundary is reached, the reason
  turn closes in the same tool-result round and control returns to EventInbox
  polling; the model never waits for a future Worker event inside that turn
- lifecycle decision rounds are finite; exhaustion closes blocked and leaves
  the event cursor retryable
- retryable lifecycle executor and missing-decision failures keep the durable
  event cursor unchanged, persist the next attempt id, apply bounded
  exponential backoff, and do not stop the daemon
- approved review-ready truth remains retryable until the Master closes it
- a blocked task may remain blocked only when the Master records that outside
  action is still required through `task(op="append")`, or explicitly
  reassigns it
- interrupted truth remains retryable until the task leaves `Interrupted`
- Worker startup/ticks requeue only retryable `Interrupted` and
  `Rejected` tasks previously bound to that Worker
- claim persists `TaskResumed` and `TaskHeartbeat` before provider execution
- successful worker completion writes one `ExecutionFactKind::ReviewReady`
- provider/runtime/tool-loop terminal failure writes one `ExecutionFactKind::Blocked`
- blocked execution releases the Worker resource to `Available`; task blockage
  and Worker resource pause are distinct owner truths
- Worker startup relies on Task Center boot reconciliation to repair historical
  paused snapshots only when no assigned task remains explicitly paused
- blocked tasks remain Master decisions and are never silently retried by the
  Worker runner
- worker reason/session truth remains persisted under worker agent identity
- task/execution/lease/agent truth remains persisted under the paired Master's Task Center namespace
- periodic runner ticks continue after idle, success, or blocked outcomes
- Worker heartbeat and result reporting are rejected if Task Center truth has
  externally terminalized the task; stale Worker runtime state cannot overwrite
  cancel/close/fail truth

## Error Mainline

- Master-selected config is rejected by the Worker runner
- Slave-selected config is rejected by the Master lifecycle runner
- Master review prose without a Task Center mutation is an explicit
  `MissingReviewDecision` error and the event cursor does not advance
- Master approval without close is an explicit `IncompleteReviewDecision`
  error and the event cursor does not advance
- nullable unused response-status fields do not stop the Master lifecycle turn;
  non-null response-status type mismatches are polished in another model round
- Master blocked prose without a persisted `task(op="append")`
  `TaskProgressed` decision is an
  explicit `MissingBlockedDecision` error
- Master interrupted prose without reassignment is an explicit
  `MissingInterruptedDecision` error
- unrelated task mutation cannot satisfy the current event decision boundary
- assignment to a historical or non-configured Worker returns a paired failed
  tool result and writes no `TaskAssigned` event
- task creation with omitted/auto/self dispatch, or explicit dispatch to a
  historical/non-configured Worker, returns a paired failed tool result and
  writes no task truth
- lifecycle decision round-budget exhaustion is explicit blocked reason truth,
  never an indefinitely active turn
- lifecycle cursor parse/write failures stop the Master loop explicitly
- Task Center and lifecycle-state failures are fatal owner-truth failures;
  lifecycle executor and missing/incomplete decision failures are retryable
- missing paired Master identity or invalid provider config blocks runner bootstrap
- missing or non-canonicalizable target cwd records blocked task truth before model execution, with classified wording for missing parent, likely output-directory misuse, permission denial, and generic canonicalization failure
- claim/heartbeat persistence failure returns an explicit runner error and does not start the model
- heartbeat or result reporting after external cancel returns explicit Task
  Center failure and does not append Worker lifecycle truth
- provider/runtime failure is paired to the claimed task/execution as blocked truth
- failure to persist the blocked fact is returned as a combined explicit runner error
- worker cannot call `task`; schema excludes it and execution policy rejects it if received
- Worker failure never becomes `review_ready`, approved, closed, or successful UI truth
- retryable task recovery must allocate a new execution id; old execution
  history remains immutable

## Shared Multi-Reference Functions

- `run_live_reason_turn_with_policy`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: share provider/reason loop mechanics while applying an explicit Master or Worker tool/workspace policy
  - allowed callers: `run_live_reason_turn`, `run_worker_live_reason_turn`, runtime tests
  - related tests: Master boundary tests, Worker tool-policy tests
  - why shared: provider/reason lifecycle must not be copied for Worker execution
- `BuiltinToolRegistry::worker_implemented_definitions`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: expose implemented Worker tools while physically excluding recursive `task` and unrestricted shell tools
  - allowed callers: runtime live bridge and tool-registry tests
  - related tests: Worker schema inclusion/exclusion tests and shell rejection test
  - why shared: Worker capability and write-boundary policy must have one registry owner

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ProductionWorkerRunner::from_default_config` | `crates/freehand-runtime/src/worker_runner.rs` | load selected Slave config and bind paired Master Task Center namespace | configured agent name | Worker runner | daemon Slave startup | config + runtime owner | bound |
| 02 | `ProductionWorkerRunner::run` | `crates/freehand-runtime/src/worker_runner.rs` | run periodic Worker ticks with explicit cadence | runner + interval | long-running Worker service | daemon Slave mode | `run_once` | bound |
| 03 | `ProductionWorkerRunner::run_once` | `crates/freehand-runtime/src/worker_runner.rs` | claim one Assigned task, canonicalize target cwd with `~` expansion and symlink resolution, heartbeat, execute, and report | Task Center + Worker identity | idle/review-ready/blocked outcome | Worker service loop/tests | task owner + live bridge | bound |
| 04 | `TaskRuntime::claim_next_task` | `crates/freehand-task/src/lib.rs` | choose and claim highest-priority Assigned task for Worker | worker id + execution id + lease TTL | claimed task + TaskResumed/heartbeat truth | Worker runner | task owner | bound |
| 05 | `WorkerHeartbeat::start` | `crates/freehand-runtime/src/worker_runner/heartbeat.rs` | renew the claimed task lease while provider execution remains active | claimed task/execution/worker identity | periodic TaskHeartbeat truth or explicit heartbeat error | `ProductionWorkerRunner::run_once` | task owner | bound |
| 06 | `run_worker_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | execute one worker task in canonical task cwd with Worker tool policy and path-preflight prompt contract | selected Worker config + live request | closed live reason outcome | Worker runner | provider/reason live bridge | bound |
| 07 | `TaskRuntime::apply_execution_fact` | `crates/freehand-task/src/lib.rs` | persist review-ready or blocked result for same execution | typed execution fact | terminal task mutation | Worker runner | task owner | bound |
| 08 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | select Slave host path without constructing Master UI dispatcher | daemon agent selection | Worker service process | daemon CLI | runtime Worker runner | bound |
| 09 | `ProductionMasterRunner::from_default_config` | `crates/freehand-runtime/src/master_runner.rs` | load selected Master config and bind the Master Task Center namespace | configured agent name | Master lifecycle runner | daemon Master startup | config + runtime owner | bound |
| 10 | `ProductionMasterRunner::run_until` | `crates/freehand-runtime/src/master_runner.rs` | poll Task Center lifecycle events, retry model/provider decision failures with bounded backoff, and stop on owner-truth failure or daemon shutdown | runner + cancellation signal | long-running Master lifecycle service | daemon Master mode | `ProductionMasterRunner::run_once` | bound |
| 11 | `ProductionMasterRunner::run_once` | `crates/freehand-runtime/src/master_runner.rs` | drain new Task Center events from the durable lifecycle cursor | Task Center + cursor | idle or task decision outcome | Master lifecycle service/tests | EventInbox + `handle_event` | bound |
| 12 | `ProductionMasterRunner::handle_event` | `crates/freehand-runtime/src/master_runner.rs` | invoke Master decision only for current review-ready, blocked, or interrupted truth | task snapshot + trigger event | task advanced, blocked observed, or explicit error | `run_once` | Master live reason turn + task owner | bound |
| 13 | `run_master_lifecycle_reason_turn` | `crates/freehand-runtime/src/lib.rs` | execute one event-isolated lifecycle decision with a target-task boundary and finite round budget | selected Master config + typed lifecycle prompt + decision boundary | closed Master turn and Task Center mutation | `ProductionMasterRunner::handle_event` | provider/reason live bridge | bound |
| 14 | `configured_worker_task_boundary_failure` | `crates/freehand-runtime/src/lib.rs` | validate Master task create/assign routing against the configured Worker topology | task tool call + configured Worker id | explicit topology failure or allowed mutation path | `execute_registry_tool_call_with_workspace` | pure boundary validator | bound |
| 15 | `execute_registry_tool_call_with_workspace` | `crates/freehand-runtime/src/lib.rs` | enforce configured Worker task routing before task-tool mutation | Master task create/assign call + configured Worker id | paired failed result or owner-routed task mutation | provider/reason live bridge | topology validator + task tool owner | bound |
| 16 | `run_master_mode` | `apps/freehand-daemon/src/main.rs` | run WebUI/ADP host and Master lifecycle runner under one daemon lifetime | Master bootstrap + bind | supervised Master daemon | daemon CLI | server host + `ProductionMasterRunner::run_until` | bound |

## Sync Status Against Code

- Task Center claim, heartbeat, execution fact, persistence, and recovery APIs are already bound
- Master workspace boundary, external-cwd delegation, and path/symlink dispatch guidance are already bound
- production Worker runner, Worker-specific live tool policy, periodic heartbeat, and Slave daemon startup are code-bound
- deterministic positive/negative tests cover idle, review-ready, blocked, missing workspace, role mismatch, and Worker tool capability boundaries
- generated wiki must be regenerated whenever this mainline changes
