# Session Management And History Rollback Plan

## Goal

Close the user-facing session lifecycle loop from function map truth to online WebUI proof:

- WebUI can create, select, rename, archive, restore, and non-destructively delete sessions.
- Archived sessions are visible in a separate UI surface and can be restored.
- Session metadata and transcript truth survive refresh and daemon restart.
- Codex-style double Escape supports session history rollback for the latest completed user turn without physical deletion.
- Rollback affects the next model request history and visible transcript, with audit evidence and online validation.

## Current State

| Area | Current | Gap |
| --- | --- | --- |
| Protocol CRUD | `CreateSession`, `RenameSession`, `ArchiveSession`, `RestoreSession`, `DeleteSession` exist in `ui.protocol` | Protocol exists, but WebUI only exposes create/delete subset |
| Persistence CRUD | `reason.persistence` stores session metadata sidecar and delete-as-archive | No WebUI archived-session restore loop |
| WebUI create/select | `New conversation`, `New task`, selected session transcript, cwd binding exist | Rename/archive/restore controls missing |
| WebUI delete | Bulk `DeleteSession` exists | UI label hides non-destructive archive semantics |
| Refresh/restart restore | Session metadata and transcript restore from reason truth | Needs full CRUD E2E proof after all UI actions |
| Input history | Up/Down local composer recall exists | Not session truth rollback |
| Escape | Esc cancels active turn or clears local input | No double-Esc rollback of latest completed turn |
| Checkpoint rewind | Runtime file checkpoint rewind exists | It is workspace file rollback, not session history rollback |
| SessionHistory rollback | Internal rewrite rollback gate exists | Not exposed as user action and not tied to turn visibility/history filtering |

## Baseline / Target Comparison

This slice is not only a UI polish pass. It closes the full session-management lifecycle by moving every user-visible action through the existing owner chain and proving it online.

| Capability | Baseline | Required Target | Owner Closure |
| --- | --- | --- | --- |
| Session create/select | WebUI can create global/task sessions and select a transcript | Keep behavior, prove refresh and restart persistence after later CRUD operations | `app.webui-smoke` invokes `ui.protocol`; `runtime.ui-command-dispatch` routes; `reason.persistence` persists |
| Session rename | Protocol/runtime/persistence support exists; WebUI surface missing | WebUI can rename selected sessions; empty title fails explicitly; title survives refresh/restart | Add WebUI control only after confirming `ui.protocol` validation and runtime route remain mapped |
| Session archive | Protocol/runtime/persistence support exists; WebUI delete wording is ambiguous | WebUI exposes archive/remove-from-active semantics without implying physical deletion | `DeleteSession` remains non-destructive delete-as-archive unless a later destructive design is approved |
| Archived sessions | Metadata truth exists, but no complete visible archived list/restore loop | WebUI has an archived-session view; archived sessions are excluded from active list, included in archived list, and restorable | Query/projection stays protocol-owned; WebUI must not keep local archived truth |
| History rollback | Internal session-history rewrite helpers exist, but no user command | Double Esc triggers append-only latest-turn rollback through ADP/protocol/runtime/persistence | New protocol command plus persistence rollback marker; no checkpoint-rewind reuse |
| Effective transcript | Visible transcript currently follows restored turn projections | Rolled-back turns are hidden from effective transcript and next model request history, while raw truth remains auditable | `reason.persistence` owns effective transcript helpers; runtime consumes effective history |
| Composer restore | Local Up/Down input recall exists | After rollback, rolled-back user text returns to composer for edit without mutating truth again | WebUI local composer update happens only after protocol receipt |
| Online proof | Prior slices have S-profile evidence, but not for full CRUD + rollback | S-profile WebUI + ADP proof covers create/rename/archive/restore/rollback/replace/restart | `127.0.0.1:4042`, screenshots/JSON under `artifacts/webui-online/` |

## Closure Path From Function Map To Online Test

Implementation must run through this fixed sequence. Skipping any stage means the slice is not closed.

1. Function map routing:
   - Start from `docs/architecture/feature-map.md`.
   - Confirm exactly these owner features before editing: `ui.protocol`, `runtime.ui-command-dispatch`, `reason.persistence`, `app.webui-smoke`, and `app.runtime-daemon`.
   - Add `app.cli-runtime-smoke` only if a CLI no-UI rollback smoke command is implemented.
   - Do not route rollback through `runtime.checkpoint-rewind`; that feature owns workspace file restore, not session transcript restore.
2. Test-design update:
   - Before code edits, update each touched owner test design with lifecycle, white-box, module black-box, project black-box, positive/negative cases, and known gaps.
   - Positive/negative rollback coverage must prove success, no-target, repeated rollback stepping backward through effective turns, active-turn rejection/cancel-first behavior, and restart recovery.
3. Mainline source update:
   - For every migrated touched owner, update `docs/mainline-calls/<feature_id>.json` with real adjacent call edges.
   - Do not invent symbols. If a symbol is pending, mark it pending only until the implementation lands, then bind it.
   - Regenerate generated wiki from JSON truth; do not hand-edit generated wiki.
4. Implementation:
   - Add rollback ingress to `ui.protocol`.
   - Add append-only rollback marker and effective transcript restore/query in `reason.persistence`.
   - Route rollback and refresh projections in `runtime.ui-command-dispatch`.
   - Expose WebUI rename/archive/archived-list/restore/double-Esc controls in `app.webui-smoke`.
   - Add daemon/ADP black-box coverage; add CLI command only if it reduces online test friction without becoming a second truth path.
5. Local verification:
   - Run owner-targeted tests first.
   - Run `cargo fmt --check`, mainline generation/check, gates, workspace tests, clippy, and `make ci`.
6. Online verification:
   - Use only S profile for development validation: `scripts/install-launchd.sh restartS`.
   - Verify served code is current if behavior looks stale; `restartS` refreshes S binaries.
   - Drive real WebUI in a browser against `http://127.0.0.1:4042/`.
   - Compare browser-visible state with ADP session truth for the same session.
   - Save screenshots and JSON under one run directory.
7. Commit:
   - Stage only relevant tracked files and new required docs/artifacts.
   - Leave unrelated untracked artifacts and `.bak` files untouched unless explicitly approved.
   - Commit only after local gates and online S-profile proof are captured.

## Target Semantics

### Session CRUD

- `CreateSession`
  - Global conversation: creates/selects a draft until first submit; no cwd required.
  - Task session: creates metadata session immediately with cwd.
- `RenameSession`
  - UI can rename selected sessions inline or through a compact dialog.
  - Empty title rejected by protocol and shown as explicit UI error.
- `ArchiveSession`
  - Moves active sessions out of the active list.
  - Does not delete turn truth.
- `RestoreSession`
  - Restores archived sessions to active list with transcript and cwd intact.
- `DeleteSession`
  - Remains non-destructive delete-as-archive unless a separate destructive lifecycle is approved.
  - UI copy should say `Archive` or `Remove from active`, not imply physical deletion.

### History Rollback

Use append-only session truth. Do not physically delete existing turn ledgers.

- New protocol command:
  - Preferred name: `RollbackLatestSessionTurn { session_id }`
  - Optional later extension: `RollbackSessionToTurn { session_id, turn_id }`
- Owner:
  - `reason.persistence` owns rollback marker persistence and rebuild/projection truth.
  - `runtime.ui-command-dispatch` only routes command and refreshes protocol state.
  - `ui.protocol` only validates command and projects rollback state.
  - WebUI only invokes command and renders result.
- Persistence truth:
  - append a rollback ledger row with session id, rollback id, target turn id, previous effective head, timestamp, writer owner, and reason.
  - active projection filters rolled-back turns from effective transcript.
  - raw ledger remains inspectable for audit.
- Next request history:
  - `ReasonTurnEngine::start_turn` must consume only effective session history, not rolled-back turns.
  - After rollback, the rolled-back user text is returned to composer for editing, but not re-entered into session truth until user submits again.
- Terminal constraints:
  - Running turn: Esc cancels, rollback is not attempted.
  - Completed latest user turn: double Esc can rollback latest logical user turn.
- Already rolled-back same target: the latest-turn command cannot address hidden turns again; repeated double-Esc steps backward through remaining effective visible user turns and returns target-not-found only after no effective target remains.
  - Failed/interrupted latest turn: policy must be explicit in function map; default allow rollback of latest visible user turn if terminal.

### Double Escape UX

- Single Esc:
  - If active turn or submit-in-flight exists: send `CancelTurn` / `CancelLatestActiveTurn`.
  - If composer has text: clear local composer text only.
  - If no active turn and composer empty: arm rollback window and show compact status.
- Double Esc within a short window, e.g. 900 ms:
  - Calls `RollbackLatestSessionTurn` for selected session.
  - Restores latest rolled-back user text into composer.
  - Refreshes sessions/transcript/debug/checkpoint projections.
- The UI must not silently rollback without protocol receipt.
- If no selected session or no rollback target exists, show explicit status and do not mutate local transcript.

## Owners And Files

### Required Feature Map Updates

- `ui.protocol`
  - Add rollback command validation and routing.
  - Add rollback query/projection DTO only if UI needs audit state.
- `runtime.ui-command-dispatch`
  - Route rollback command into `reason.persistence`.
  - Refresh `UiProtocolState` session list and selected transcript after rollback.
- `reason.persistence`
  - Add rollback marker truth, effective transcript restore, and query helpers.
  - Keep raw turn truth immutable.
- `app.webui-smoke`
  - Add UI controls for rename/archive/restore.
  - Add archived sessions view.
  - Add double-Esc rollback interaction.
- `app.runtime-daemon`
  - Add daemon ADP black-box for rollback and complete CRUD.
- Optional if needed:
  - `reason.session-history`
  - only if rollback must update rewrite mode/version beyond persistence projection.

### Expected Code Touch Points

- `crates/freehand-ui-protocol/src/lib.rs`
- `crates/freehand-runtime/src/lib.rs`
- `crates/freehand-reason/src/persistence.rs`
- `apps/freehand-server/src/page.rs`
- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/assets/webui.css`
- `apps/freehand-server/src/lib.rs`
- `apps/freehand-daemon/src/main.rs`
- `apps/freehand-cli/src/main.rs` if no-UI ADP rollback/CRUD smoke is needed

### Required Docs

- `docs/architecture/feature-map.md`
- `docs/function-maps/ui.protocol.md`
- `docs/testing/ui.protocol.md`
- `docs/function-maps/runtime.ui-command-dispatch.md`
- `docs/testing/runtime.ui-command-dispatch.md`
- `docs/function-maps/reason.persistence.md`
- `docs/testing/reason.persistence.md`
- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- `docs/function-maps/app.runtime-daemon.md`
- `docs/testing/app.runtime-daemon.md`
- migrated mainline JSONs for each touched migrated feature
- generated wiki from `cargo run -p xtask -- mainlines generate`
- `note.md`, `MEMORY.md`, and local skill update if new reusable validation or failure rule is learned

## Design Rules

- No physical deletion of session/turn truth in this slice.
- No UI-local session truth. UI may hold selected ids and draft state only.
- No fallback to localStorage for CRUD or rollback truth.
- No silent projection filtering without durable rollback marker.
- Do not reuse checkpoint rewind semantics for session history rollback.
- Keep raw ledger/audit visible to debug paths; user-visible transcript consumes effective history.
- Data/control separation stays intact; rollback control metadata must not be injected into user/provider payload text.

## Test Plan

### White-Box

- `ui.protocol`
  - accepts rollback command with non-empty session id.
  - rejects rollback command with empty session id.
  - command ingress routes rollback to the correct owner.
  - query-route misuse remains rejected.
- `reason.persistence`
  - rollback latest completed user turn writes append-only rollback marker.
  - restore/effective transcript excludes rolled-back turn after restart.
  - raw turn truth remains on disk.
  - repeated rollback steps backward through remaining effective turns, cannot re-target an already-hidden turn, and fails explicitly after no effective target remains.
  - rollback with no eligible target fails explicitly.
  - rollback after multi-round tool turn removes the whole logical user turn from effective next-history.
- `runtime.ui-command-dispatch`
  - ADP rollback command refreshes session transcript projection.
  - next submitted turn after rollback excludes rolled-back history.
  - rollback cannot race active turn; active turn must be cancelled first.

### Module Black-Box

- WebUI server asset tests:
  - rename/archive/restore controls exist.
  - archived session panel exists.
  - double-Esc handler exists and calls rollback command only when armed.
  - single Esc cancel behavior remains.
- Daemon ADP tests:
  - create -> rename -> archive -> query active excludes -> query archived includes -> restore -> query active includes.
  - create session with two turns -> rollback latest -> query transcript excludes latest -> submit next -> query transcript proves inherited history is effective history.

### Project Black-Box / Online

Use S profile only:

```bash
scripts/install-launchd.sh restartS
curl -4fsS http://127.0.0.1:4042/health
~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp
```

Browser proof with Playwright against `http://127.0.0.1:4042/`:

1. Create global conversation.
2. Submit first turn and wait terminal.
3. Rename session in WebUI, refresh, confirm title persists.
4. Archive session, confirm active list excludes it and archived list includes it.
5. Restore session, confirm transcript and cwd persist.
6. Submit second turn, wait terminal.
7. Press Esc twice within rollback window.
8. Confirm latest user text is back in composer.
9. Confirm transcript hides rolled-back second turn.
10. Submit edited replacement.
11. Query ADP session transcript and confirm effective order.
12. Restart S daemon, reload browser, confirm title/archive state/effective transcript persist.

Save browser screenshots and JSON under `artifacts/webui-online/<date-session-crud-rollback>/`.

## Required Gates

Run targeted first, then full:

```bash
cargo fmt --check
cargo test -p freehand-ui-protocol -- --nocapture
cargo test -p freehand-reason -- --nocapture
cargo test -p freehand-runtime -- --nocapture
cargo test -p freehand-server -- --nocapture
cargo test -p freehand-daemon -- --nocapture
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
make ci
```

## Implementation Order

1. Update feature map, function maps, test designs, and mainline JSONs for the selected owners.
2. Add protocol command and validation for rollback.
3. Add persistence rollback marker and effective transcript restore.
4. Add runtime dispatch route and UI state refresh.
5. Add ADP/CLI or daemon black-box coverage.
6. Add WebUI CRUD controls and archived view.
7. Add double-Esc rollback UX.
8. Regenerate wiki and run mapped tests.
9. Run S-profile online WebUI and ADP proof.
10. Update memory/skill only with reusable validated lessons.
11. Commit only relevant tracked files.

## Definition Of Done

- Function maps, test designs, mainline JSONs, and generated wiki match implemented code.
- WebUI visibly supports create, rename, archive, restore, and delete-as-archive.
- Archived sessions are inspectable and restorable.
- Double Esc rolls back latest eligible completed user turn through protocol/runtime/persistence truth, not local UI mutation.
- Next model request after rollback uses effective history only.
- Refresh and daemon restart preserve CRUD state and rollback projection.
- S-profile online evidence exists and is reported.
- Full gate passes.

## Gap-Completion Design For The Next Goal

This section is the execution contract for finishing the partially landed session CRUD and rollback slice. It is intentionally comparative: a worker must first prove what is already implemented, then close only the missing gaps from function maps through online S-profile evidence.

### Current Implemented Baseline To Verify First

| Layer | Expected landed state | Verification before further edits |
| --- | --- | --- |
| `ui.protocol` | `RollbackLatestSessionTurn { session_id }` command, validation, owner routing, and session projection replacement helper exist | Read `crates/freehand-ui-protocol/src/lib.rs`; run `cargo test -p freehand-ui-protocol -- --nocapture` |
| `reason.persistence` | append-only rollback marker exists; effective restore filters rolled-back logical turns; raw turn files remain retained | Read `crates/freehand-reason/src/persistence.rs`; run `cargo test -p freehand-reason -- --nocapture --test-threads=1` |
| `runtime.ui-command-dispatch` | rollback dispatch calls persistence, refreshes in-memory state, refreshes `UiProtocolState`, and returns explicit status | Read `crates/freehand-runtime/src/lib.rs`; run `cargo test -p freehand-runtime -- --nocapture --test-threads=1` |
| `app.webui-smoke` | WebUI has rename, archive, archived-list, restore, and double-Esc rollback handlers | Read `apps/freehand-server/src/page.rs`, `apps/freehand-server/assets/webui.js`, and `apps/freehand-server/assets/webui.css`; run `node --check apps/freehand-server/assets/webui.js` plus `cargo test -p freehand-server -- --nocapture` |
| `app.cli-runtime-smoke` | CLI rollback action may exist for no-UI ADP testing | Read `apps/freehand-cli/src/main.rs`; run `cargo test -p freehand-cli -- --nocapture` |

Do not assume the baseline is correct from source presence alone. Treat it as unverified until the mapped tests and source review agree.

### Known Gaps To Close

| Gap | Required closure | Owner docs to update |
| --- | --- | --- |
| Function map and test design drift | Every touched feature must bind the new rollback/session symbols, mainline paths, lifecycle checks, and tests | `docs/function-maps/ui.protocol.md`, `docs/testing/ui.protocol.md`, `docs/function-maps/reason.persistence.md`, `docs/testing/reason.persistence.md`, `docs/function-maps/runtime.ui-command-dispatch.md`, `docs/testing/runtime.ui-command-dispatch.md`, `docs/function-maps/app.webui-smoke.md`, `docs/testing/app.webui-smoke.md`, `docs/function-maps/app.runtime-daemon.md`, `docs/testing/app.runtime-daemon.md`, plus CLI docs if CLI rollback remains |
| Mainline JSON not fully synced | Migrated features need machine-readable adjacent call edges with real symbols, then regenerated wiki | `docs/mainline-calls/*.json`, regenerated `docs/wiki/*.md` |
| Rollback negative cases incomplete | Add/verify no-target, active-turn rejection, repeated rollback policy, restart recovery, raw-truth-retained checks | `reason.persistence`, `runtime.ui-command-dispatch`, daemon ADP tests |
| Repeated rollback semantics | Repeated double-Esc steps backward through effective visible user turns; rollback of an already-hidden same target cannot be addressed again and must not resurrect or silently no-op | `reason.persistence` function map/test design and persistence tests |
| Daemon ADP black-box gap | Add a real ADP test for create/rename/archive/archived-list/restore and rollback effective transcript | `app.runtime-daemon` |
| Online WebUI evidence missing | Run S-profile browser proof covering CRUD, rollback, replacement submit, ADP comparison, and restart restore | artifact directory under `artifacts/webui-online/` |
| Commit boundary not closed | Stage only relevant tracked code/docs and required new docs/artifacts; leave unrelated untracked files untouched | git status review before commit |

### Required Closed Loop

1. Function map and test-design loop:
   - Start from `docs/architecture/feature-map.md`.
   - Confirm owners: `ui.protocol`, `reason.persistence`, `runtime.ui-command-dispatch`, `app.webui-smoke`, `app.runtime-daemon`, and `app.cli-runtime-smoke` only if CLI rollback remains.
   - Update function maps before or with implementation so another worker can locate owner, symbols, request path, response path, error path, and tests without grep.
   - Update test designs with positive and negative coverage before claiming the tests close the lifecycle.
2. Mainline and wiki loop:
   - Update migrated `docs/mainline-calls/<feature_id>.json` files with adjacent caller/callee edges only.
   - Use real code symbols after implementation lands; do not invent symbols.
   - Run `cargo run -p xtask -- mainlines generate` and treat generated wiki as derived truth.
   - Run `cargo run -p xtask -- mainlines check` and `cargo run -p xtask -- gates check`.
3. Code and persistence loop:
   - Protocol accepts only valid rollback commands and routes them to the persistence owner.
   - Persistence writes append-only rollback truth and computes effective transcript from authoritative truth.
   - Runtime refreshes both session turn state and shared UI projection after rollback.
   - WebUI invokes protocol commands only; it may restore composer text after receipt but must not mutate transcript truth locally.
4. Black-box loop:
   - Daemon ADP test proves CRUD state and effective transcript without browser DOM dependency.
   - CLI no-UI command may be used as test driver, but it must not become a second source of session truth.
5. Online loop:
   - Restart the S profile with `scripts/install-launchd.sh restartS`.
   - Prove `127.0.0.1:4042` health and ADP smoke.
   - Drive real WebUI in a browser.
   - Compare visible transcript/session UI with ADP session truth for the same session.
   - Restart S daemon and reload browser to prove durable recovery.

### Online Acceptance Scenario

Use a fresh session marker and save all evidence under one directory:

`artifacts/webui-online/<yyyymmdd-session-crud-rollback-4042>/`

Required browser and ADP scenario:

1. Open `http://127.0.0.1:4042/`.
2. Create a new global conversation.
3. Submit first prompt with a unique marker and wait for terminal state.
4. Rename the session in WebUI; refresh the page and confirm the title persists.
5. Archive the session; confirm active list excludes it and archived list includes it.
6. Restore the session; confirm active list includes it and transcript is intact.
7. Submit second prompt; wait for terminal state.
8. Press Esc twice inside the rollback window.
9. Confirm the second prompt returns to composer, visible transcript hides that latest user turn, and ADP transcript matches effective history.
10. Submit edited replacement text.
11. Confirm visible transcript order and ADP `turn_ids` show the replacement as the next effective turn.
12. Run `scripts/install-launchd.sh restartS`, reload browser, and confirm title, active/archive state, composer state, and effective transcript remain correct.

Evidence must include screenshots, browser console/page-error summary, ADP query summary, served-code freshness check if behavior looks stale, and exact commands used.

### Minimum Verification Matrix

Run targeted gates first:

```bash
node --check apps/freehand-server/assets/webui.js
cargo test -p freehand-ui-protocol -- --nocapture
cargo test -p freehand-reason -- --nocapture --test-threads=1
cargo test -p freehand-runtime -- --nocapture --test-threads=1
cargo test -p freehand-server -- --nocapture
cargo test -p freehand-daemon -- --nocapture
cargo test -p freehand-cli -- --nocapture
```

Then run registry and full gates:

```bash
cargo fmt --check
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
make ci
```

Then run online S-profile proof:

```bash
scripts/install-launchd.sh restartS
curl -4fsS http://127.0.0.1:4042/health
~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp
```

### Commit Criteria

Commit only after all of the following are true:

- function maps, test designs, mainline JSON, generated wiki, and implemented symbols agree;
- targeted tests, mainline checks, gates, workspace tests, clippy, and `make ci` pass;
- online S-profile WebUI and ADP evidence is captured;
- `git status --short` is reviewed and only relevant tracked changes plus required new goal/docs/artifacts are staged;
- unrelated untracked artifacts, `.bak` files, and user/other-worker changes are left untouched.
