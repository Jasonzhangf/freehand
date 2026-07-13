# Worker/Master Observability Fixed-Session Closeout Plan

## Goal And Acceptance

Close the current Freehand Worker/Master failure-observability and fixed online verification slice.

Acceptance means:

- A user `SubmitUserInput` never collapses into an empty selected session while live execution is pending or failing.
- Worker/provider failures are visible as owner truth through session, TaskBoard, AgentBoard, and task/worker lifecycle evidence.
- The online verifier uses a fixed session and does not keep creating new random sessions.
- Install or service restart runs the macOS file-permission preflight before launchd start/restart and records status.
- S-profile online proof is run only against `127.0.0.1:4042`; release `4041` is not touched.
- No commit is made unless Jason explicitly asks.

## Scope

In scope:

- Runtime selected-session live-submit failure materialization and projection.
- Fixed-session ADP online observability script.
- Worker/subagent state observation through owner truth, not receipt-only waiting.
- First-launch/restart file-permission preflight wiring.
- Function map, mainline call map, testing docs, wiki, local skill, `note.md`, and `MEMORY.md` updates for changed truth.
- Current S-profile online verification.

Out of scope:

- Release `4041`, Android, broad WebUI browser matrix, or screenshots unless needed for the current proof.
- Cleaning or deleting `output/`.
- Fixing unrelated broad-test failures unless they block the stated closeout.
- Changing provider credentials or silently falling back to another provider.

## Design Rules

- No fallback: if selected-session/cwd/persistence/provider truth fails, expose the error or persist a failed selected-session turn; never silently switch to a non-live path.
- Session truth first: command receipts are not enough; every online proof must query selected session truth plus TaskBoard and AgentBoard.
- Worker failure is lifecycle truth: Worker errors should become interrupted/blocked/error facts that Master can observe and continue from.
- Fixed validation identity: use stable session ids for repeatable online checks.
- Service-scoped operations only: no `pkill`, `killall`, `kill $(...)`, or `xargs kill`.
- Preserve dirty worktree and unrelated untracked `output/`.

## Technical Plan

Runtime dispatch:

- Ensure `prepare_live_submit_user_input` returns explicit errors for selected-session/cwd preparation failures.
- Ensure live-submit finish restores existing persisted failed turn truth when available.
- Ensure provider/protocol failures before persisted truth materialize a failed turn into the selected session and update `UiProtocolState`.
- Ensure corrupt persistence fails explicitly instead of inventing fake success or fallback truth.

Online verifier:

- Provide a standard script that sends internally tagged ADP envelopes with `kind=command` and `kind=query`.
- Submit to a caller-provided fixed session id.
- Query pending selected-session turns after a short delay.
- Wait for command receipt or timeout without treating receipt as the only proof.
- Query final selected-session turns, TaskBoard, and AgentBoard.
- Emit one JSON evidence object with pending/final turn state, receipt/timeout, worker state, task id, execution id, activity reason, and blocked-task summaries.

Permission preflight:

- Add a macOS shell preflight that checks runtime home, launchd workdir, `Documents`, `Desktop`, `Downloads`, and optional extra paths.
- Write `~/.freehand/state/file-permission-preflight.json`.
- Open Full Disk Access settings on denial.
- Fail by default on denial; allow explicit warn mode with `FREEHAND_FILE_PERMISSION_PREFLIGHT=warn`.
- Wire install/restart paths for S and non-S daemon/worker profiles through the preflight.

Docs and maps:

- Update `runtime.ui-command-dispatch` function map, testing doc, mainline JSON, and wiki for selected-session failure materialization.
- Update `foundation.workspace` function map, testing doc, mainline JSON, and wiki for permission preflight and fixed-session online verifier.
- Update local Freehand skill with the fixed-session observability rule.
- Append only confirmed results to `note.md` and `MEMORY.md`.

## File Checklist

Expected touched files for this slice include:

- `crates/freehand-runtime/src/lib.rs`
- `scripts/freehand-file-permission-preflight.sh`
- `scripts/install-launchd.sh`
- `scripts/verify-adp-fixed-session-observability-online.py`
- `.agents/skills/freehand-dev/SKILL.md`
- `docs/function-maps/runtime.ui-command-dispatch.md`
- `docs/testing/runtime.ui-command-dispatch.md`
- `docs/mainline-calls/runtime.ui-command-dispatch.json`
- `docs/wiki/runtime.ui-command-dispatch.md`
- `docs/function-maps/foundation.workspace.md`
- `docs/testing/foundation.workspace.md`
- `docs/mainline-calls/foundation.workspace.json`
- `docs/wiki/foundation.workspace.md`
- `note.md`
- `MEMORY.md`

Do not assume every dirty file belongs to this slice; inspect before staging or reporting.

## Verification Matrix

Static and local verification:

- `bash -n scripts/freehand-file-permission-preflight.sh`
- `bash -n scripts/install-launchd.sh`
- `python3 -m py_compile scripts/verify-adp-fixed-session-observability-online.py`
- `FREEHAND_FILE_PERMISSION_PREFLIGHT=warn scripts/freehand-file-permission-preflight.sh`
- `jq empty docs/mainline-calls/runtime.ui-command-dispatch.json docs/mainline-calls/foundation.workspace.json`
- `cargo test -p freehand-runtime live_dispatch_materializes_failed_turn_when_provider_fails_before_persistence -- --nocapture`
- `cargo test -p freehand-runtime live_dispatch_failure_preserves_other_session_transcripts -- --nocapture`
- `cargo fmt --check`
- `cargo check -p freehand-runtime`
- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- `git diff --check`

S-profile online verification:

- `scripts/install-launchd.sh restartS`
- `curl -4fsS http://127.0.0.1:4042/health`
- `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
- `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp`
- `cat ~/.freehand/state/file-permission-preflight.json`
- `scripts/verify-adp-fixed-session-observability-online.py --url ws://127.0.0.1:4042/adp --session online-fixed-observability-standard`
- `grep -n "FREEHAND_PROVIDER_RETRY_FIXTURE_KEY\|FREEHAND_PROVIDER_RETRY_BACKOFF_MS\|FREEHAND_MASTER_AUTONOMY_FIXTURE_KEY\|FREEHAND_MASTER_AUTONOMY_TARGET_CWD" ~/.freehand/daemonS.env || true`

Optional diagnostic verification:

- If using a broad `live_dispatch` filter, report any unrelated red test separately and do not hide it.
- If Worker is blocked by provider/network failure, report task id, execution id, current activity reason, and whether the failure is now observable.

## Evidence To Report

Final report must include:

- What changed at behavior level.
- Local verification commands and pass/fail results.
- S-profile endpoint, config summary, and fixture-env restoration result.
- Fixed session id and turn id from the online verifier.
- Pending selected-session evidence: turn exists, original user text exists, terminal state.
- Final selected-session evidence: same turn or expected repaired turn, terminal state, original user text still present.
- Worker/AgentBoard evidence: state, task id, execution id, activity reason.
- Permission preflight JSON status.
- Known remaining risks, including provider-blocked Worker tasks or unrelated red tests.
- Explicitly state that `output/` remains unrelated and untouched.

## Implementation Order

1. Re-read current docs/maps and source before editing.
2. Confirm owner from function map and mainline call map.
3. Apply runtime and script changes surgically.
4. Update docs/maps/wiki/skill/memory for changed truth.
5. Run local verification matrix.
6. Run service-scoped S-profile online verification.
7. Summarize evidence and risks without claiming unverified completion.

## Done Definition

This closeout is done only when:

- Fixed selected-session online proof shows no empty-session collapse.
- Worker failure or blocked state is visible through owner truth.
- Permission preflight is wired and recorded.
- Function maps/mainline maps/docs are synchronized.
- Required local and S-profile gates have current evidence.
- No broad kill, release `4041`, random-session churn, fallback, or unapproved commit occurred.
