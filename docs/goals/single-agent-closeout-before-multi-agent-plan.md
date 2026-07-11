# Single-Agent Closeout Before Multi-Agent Plan

## Goal

Close the single-model, single-agent lifecycle before starting multi-agent
dispatch work.

The single agent must reliably complete one workspace-bound task from user input
through multi-round reasoning, tool execution, repair, final answer, optional
task review/close, persistence, UI projection, ADP/headless validation, and
restart recovery.

## Acceptance Criteria

- One workspace/session can survive refresh and daemon restart.
- Same-session follow-up provider request includes prior effective turns.
- Multi-round tool execution succeeds when an earlier tool call fails and is
  returned to the model as a paired tool result.
- Completion schema mismatch is handled as response polishing, not task/turn
  failure.
- Recoverable provider errors retry exactly ten attempts with exponential
  backoff starting at one second before terminal provider failure.
- UI shows every lifecycle phase from submit through terminal state and never
  leaves blank/fake running rows.
- Historical turns are static after terminal state.
- Tool display uses protocol-owned semantic display, not WebUI guessing.
- Minimal single-agent task lifecycle exists and closes with accepted summary.
- Future prompt context prefers success path after repaired failures while
  debug/replay/task/error ledgers keep raw failure evidence.
- Headless ADP/CLI samples prove the same flows without UI.
- WebUI online proof compares visible UI with ADP/session truth.

## Scope

In scope:

- session durability and same-session continuation
- single-agent reasoning turn lifecycle
- tool call/result pairing and repair loop
- schema polishing
- provider retry/backoff distinction
- WebUI lifecycle observability
- minimal single-agent task model
- accepted summary admission
- context-economy rule for repaired failures
- ADP/CLI black-box samples
- restart recovery for session/task/turn truth
- docs/function-map/test-design updates for touched owners

Out of scope:

- worker pool
- master/worker topology
- subagent spawn/send/wait/close implementation
- parallel scheduling
- worker turn subscription
- parent/child task graph beyond minimal single-agent task references
- worker lease/reassign/resource pool recovery
- Android-only feature work unless WebUI proof requires release/phone validation

## Design Principles

- Single source of truth first: UI renders protocol/session/task truth and does
  not infer from raw ids or local browser state.
- No fallback: every error must be typed and visible through the owning pipeline.
- Control/data split: schema polishing, tool result failure, provider failure,
  permission failure, and persistence failure stay distinct.
- Repair loops are normal: tool execution error and schema mismatch are not
  terminal failures by themselves.
- Prompt context stays economical: raw failed attempts are kept in ledgers, not
  future default prompt context after successful repair.
- UI observability starts at client acceptance: submitted user text appears
  immediately and every wait phase has real state, timer, and terminal stop.
- Tests must include positive and negative locks for terminal/non-terminal state.

## Technical Plan

### Owner Areas

- `reason.turn`: turn state, provider-output application, terminal schema
- `reason.persistence`: authoritative session/turn persistence and restore
- `reason.session-history`: same-session context rebuild and accepted history
- `reason.context-planner`: prompt context admission and repaired-failure economy
- `reason.rewrite-policy`: superseded failed-attempt pruning decision
- `provider.reason-live-bridge`: live provider/tool loop and provider retry
- `tool.registry`: tool schema/execution ownership
- `tool.display`: semantic tool projection
- `ui.protocol`: ADP commands/query/subscribe and UI projections
- `runtime.ui-command-dispatch`: runtime dispatch into owner modules
- `app.webui-smoke`: WebUI rendering, online verifier, smoke checks
- `task.orchestration`: minimal single-agent durable task lifecycle

### Required Behaviors

1. Session truth:
   - create clean session
   - append turns in order
   - refresh/restart restore
   - continuation includes effective history

2. Reasoning truth:
   - user submit accepted
   - model request sent
   - model waiting/streaming
   - tool call ready
   - tool running
   - tool result returned to model
   - final received
   - terminal success/failure

3. Error handling:
   - schema mismatch -> polishing retry
   - tool failed -> paired tool result -> model continuation
   - provider recoverable error -> 5 retries starting at 1s
   - provider exhausted -> terminal provider failure with concrete code
   - permission/precondition error -> visible user/action-needed state
   - persistence error -> explicit system failure, no silent UI disappearance

4. UI projection:
   - one chronological card/row per real turn phase
   - live animation only for current non-terminal phase
   - terminal turns static
   - submitted text remains visible after send and refresh
   - final summary preserves actual response format
   - tool cards update semantically in place

5. Minimal single-agent task:
   - task belongs to workspace/session
   - task can be created/running/review_ready/approved/closed/blocked/failed
   - accepted summary can be admitted to workspace session context
   - task truth persists across restart

6. Context economy:
   - failed attempts visible only while needed for repair
   - later success supersedes failed attempt in future prompt history
   - raw failure evidence remains in debug/replay/task/error truth

## File Checklist

Update only owners required by actual findings. Before code edits, inspect
feature map, function map, test design, and mainline call map for each owner.

Likely docs to update:

- `docs/design/reason-turn-design.md`
- `docs/design/reason-persistence-design.md`
- `docs/design/reason-context-planner-design.md`
- `docs/design/reason-rewrite-policy-design.md`
- `docs/design/task-orchestration-design.md`
- `docs/function-maps/*.md` for touched owners
- `docs/testing/*.md` for touched owners
- `docs/mainline-calls/*.json` when migrated mainline edges change
- `docs/wiki/*.md` generated only through xtask
- `MEMORY.md`, `CACHE.md`, `note.md`

Likely code areas:

- `crates/freehand-reason/**`
- `crates/freehand-runtime/**`
- `crates/freehand-ui-protocol/**`
- `crates/freehand-tools/**`
- `crates/freehand-blocks/**`
- `apps/freehand-server/**`
- `apps/freehand-cli/**`
- `scripts/webui_verify_online.mjs`

## Risks And Mitigations

- Risk: mixing multi-agent work into single-agent closeout.
  - Mitigation: keep worker pool/subagent/parallel scheduling out of scope.

- Risk: UI proves only DOM state, not truth.
  - Mitigation: every online proof must compare ADP/session truth with visible UI.

- Risk: tool failure becomes terminal failure again.
  - Mitigation: positive/negative tests for paired failed tool result returning
    to model.

- Risk: schema mismatch confused with provider failure.
  - Mitigation: tests for finish-reason-gated schema polishing and separate
    provider retry branch.

- Risk: restart recovery loses session/task state.
  - Mitigation: daemon restart scenario in CLI/WebUI online verifier.

- Risk: prompt context bloats from failed attempts.
  - Mitigation: context planner/rewrite-policy tests plus provider request
    inspection.

## Verification Matrix

Local gates:

```bash
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

Targeted owner tests:

- `reason.turn` positive/negative terminal tests
- `reason.persistence` session restore/restart tests
- `reason.session-history` continuation request-history tests
- `reason.context-planner` repaired-failure context admission tests
- `provider.reason-live-bridge` multi-round failed-tool repair and provider retry tests
- `ui.protocol` projection tests for waiting/tool/final/error states
- `app.webui-smoke` asset/render smoke tests
- `task.orchestration` minimal single-agent lifecycle tests

Headless black-box samples:

```bash
freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp
freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample success
freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample failure
freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample schema-mismatch
freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample provider-retry
scripts/verify-provider-retry-online.sh
freehand-cliS session-continue-sample --url ws://127.0.0.1:4042/adp
freehand-cliS task-lifecycle-sample --url ws://127.0.0.1:4042/adp
```

If a listed CLI sample does not exist yet, implement it before claiming the
corresponding behavior closed.

Online WebUI proof:

- restart S profile on fixed `127.0.0.1:4042`
- health check
- ADP smoke
- run real browser/CDP WebUI verifier
- submit real requests for success, failed-tool repair, schema mismatch,
  provider retry/failure if credentials/fixture allow
- verify submitted text visible immediately and after refresh
- verify terminal state stops all live animation
- verify ADP/session truth matches visible UI
- save artifacts under `artifacts/webui-online/`

Release/Android proof:

- only required if this closeout promotes to release/phone surface
- release fixed port remains `4041`
- S/dev fixed port remains `4042`

## Implementation Steps

1. Read `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`, feature map, and local
   Freehand skill.
2. Search MemoryPalace/source-only docs for existing single-agent lifecycle
   truth.
3. For each target owner, read function map and test design before edits.
4. Update test design first for any changed behavior.
5. Add/repair headless ADP samples before UI-only work.
6. Fix session/history/continuation if still incomplete.
7. Fix reasoning/tool/schema/provider lifecycle issues.
8. Fix UI projection only after protocol truth is correct.
9. Add minimal single-agent task lifecycle if missing.
10. Add context-economy tests for repaired failure admission/pruning.
11. Run targeted tests.
12. Run full gates as needed.
13. Restart S profile and run online WebUI proof.
14. Update docs/function maps/mainline maps/wiki/memory.
15. Commit only related changes after verification.

## Definition Of Done

- Single-agent task can be completed end-to-end through CLI/ADP and WebUI.
- Same-session continuation is proven after refresh and daemon restart.
- Multi-round failed-tool repair reaches terminal success.
- Schema mismatch and provider failure branches are distinct and tested.
- UI lifecycle is observable from submit to terminal with no fake animation.
- Minimal task lifecycle persists and closes with accepted summary.
- Future prompt context does not carry superseded raw failed attempts by default.
- Required docs, function maps, test designs, and memory are updated.
- Verification evidence is recorded with exact commands and artifact paths.

## Non-Goal Lock

Do not implement or start multi-agent dispatch in this goal. No worker pool,
subagent spawn/send/wait/close, parallel scheduling, worker turn subscription, or
master/worker topology work belongs in this closeout.
