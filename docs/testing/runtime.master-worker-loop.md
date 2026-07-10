# Test Design: `runtime.master-worker-loop`

- feature_id: `runtime.master-worker-loop`
- owner: `crates/freehand-runtime`
- host: `apps/freehand-daemon`

## Lifecycle Under Test

1. Master Task Center has a registered Worker and an Assigned task.
2. Slave daemon selects the configured Worker and starts the production runner.
3. Runner opens the paired Master's Task Center namespace.
4. Runner claims one task for its Worker identity and persists lease heartbeat.
5. Runner canonicalizes `task.target_cwd`.
6. Runner executes one provider/reason turn under Worker identity and Worker tool policy.
7. Runner writes `review_ready` on successful completion or `blocked` on provider/runtime failure.
8. Runner returns to polling without inventing task truth while idle.
9. Restart reads the same task/execution/agent/history truth.

## White-Box Coverage

- runner lifecycle tests live in `crates/freehand-runtime/src/worker_runner/tests.rs`
- lease renewal ownership lives in `crates/freehand-runtime/src/worker_runner/heartbeat.rs`

### Positive

- Worker tool definitions include `bash`, workspace read/write/search tools, and local planning tools.
- Worker tool definitions exclude `task`.
- Assigned task is claimed once with one execution id.
- Claim writes `TaskResumed` and `TaskHeartbeat`.
- Successful model completion writes `TaskReviewSubmitted` with matching task/execution/agent ids.
- Worker session id and turn id are deterministic from task/execution identity.
- no-task tick returns `Idle` and leaves task history unchanged.

### Negative

- Master config cannot construct `ProductionWorkerRunner`.
- Worker cannot execute `task` even if a malformed/provider-injected call bypasses schema exposure.
- missing `target_cwd` records `Blocked`; model execution does not start.
- non-canonicalizable `target_cwd` records `Blocked`; model execution does not start.
- provider/runtime failure records `Blocked`; it never writes `ReviewReady`.
- claim or heartbeat failure stops before provider execution.
- failure to persist the blocked fact remains an explicit runner error.
- existing Master shell denial and runtime-home boundary tests remain green.

## Module Black-Box Coverage

- deterministic fake executor drives `run_once` through:
  - idle
  - successful completion
  - provider/runtime failure
- real live bridge test proves Worker schema excludes `task` and workspace root is task cwd.
- daemon test proves:
  - Master mode creates UI host
  - Slave mode creates Worker runner
  - Slave mode does not bind WebUI/ADP transport
- restart test opens a new runner against the same runtime home and verifies same task/execution/agent/history ids.

## Project Black-Box Coverage

- start S-profile Master daemon on fixed port 4042
- start a separate configured Slave daemon process
- submit a real Master request that creates and assigns work outside `~/.freehand`
- verify Worker TaskHistory contains:
  - `TaskAssigned`
  - `TaskResumed`
  - `TaskHeartbeat`
  - `TaskReviewSubmitted` or `TaskBlocked`
- verify the same task/execution/agent ids after Worker restart
- only claim production closure when the Worker produced a real deliverable or an explicit real-provider blocked result

## Runtime Evidence

- `~/.freehand/state/tasks`
- `~/.freehand/state/agents`
- `~/.freehand/state/turns`
- `~/.freehand/ledgers/tasks`
- `~/.freehand/ledgers/reason`
- `~/.freehand/logs`

## Known Non-Goals

- multi-task context switching inside one Worker process
- recursive Worker-created subagents
- automatic task approval/close by the Worker
- UI projection changes
- remote node transport; first production slice uses the shared local Task Center runtime home

## Definition Of Done

- positive and negative tests above are green
- function map and mainline manifest bind real symbols
- `cargo test -p freehand-tools worker_implemented -- --nocapture`
- `cargo test -p freehand-runtime production_worker_runner -- --nocapture`
- `cargo test -p freehand-daemon worker_mode -- --nocapture`
- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- S-profile online TaskHistory proves claim + heartbeat + terminal execution fact
