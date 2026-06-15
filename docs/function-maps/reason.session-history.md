# Function Map: `reason.session-history`

- feature_id: `reason.session-history`
- owner crate: `crates/freehand-reason`
- owner module: `crates/freehand-reason/src/session_history.rs`
- owner entry symbols:
  - `SessionHistory::new`
  - `SessionHistory::stage_compaction`
  - `SessionHistory::stage_rollback`
  - `SessionHistory::stage_resume_rebuild`
  - `SessionHistory::commit_turn_start`
  - `SessionHistory::persist_json`
  - `SessionHistory::from_persisted_json`
  - `SessionHistory::persist_to_path`
  - `SessionHistory::load_from_path`

## Request Mainline

- runtime/session truth is created or restored as `SessionHistory`
- stable/session-stable base context lives in `SessionHistory.base_context_segments`
- `reason.rewrite-policy` decides whether runtime should stay append-only, compact, rollback, rebuild, or block
- `ReasonRewriteRuntime` is the baseline consumer that applies policy decisions before calling any `SessionHistory::stage_*` method
- explicit rewrite gate methods are the only owner path that may bump `rewrite_version` and switch non-ordinary `rewrite_mode`
- `ReasonTurnEngine::start_turn` reads `SessionHistory` for base context, `rewrite_mode`, and `rewrite_version`
- after successful turn startup, `SessionHistory::commit_turn_start` returns the session to ordinary-turn mode while preserving the bumped version

## Response Mainline

- rewrite gate methods return updated session truth plus a rewrite-ledger record
- persistence methods return JSON or filesystem snapshots for later reload
- turn startup consumes session history state and projects rewrite metadata into planner diagnostics, not request text

## Error Mainline

- empty rewrite reason is rejected
- volatile or forbidden rewrite base segments are rejected
- invalid persisted json is rejected
- file IO failure is explicit
- mismatched session id between turn input and session history is rejected by `ReasonTurnEngine::start_turn`

## Shared Multi-Reference Functions

- `validate_rewrite_base_segments`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: validate and order stable/session-stable rewrite base segments before session truth mutates
  - allowed callers: `freehand-reason`, owner-crate tests
  - related tests: rewrite base rejection, persisted session truth validation
  - why shared: stable-prefix semantic validation must not be duplicated in orchestrator code
- `inspect_context_cache_diagnostics`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: compute metadata-side cache diagnostics for rewrite ledger evidence
  - allowed callers: `freehand-reason`, owner-crate tests, replay/debug tools
  - related tests: rewrite diagnostics snapshot tests
  - why shared: cache-shape semantics must stay aligned between planner turns and rewrite ledger

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `SessionHistory::new` | `crates/freehand-reason/src/session_history.rs` | create session truth with validated base context and ordinary rewrite state | session id + stable/session-stable base segments | initialized session history | runtime/bootstrap | session-history owner | bound |
| 02 | `SessionHistory::stage_compaction` | `crates/freehand-reason/src/session_history.rs` | stage compaction rewrite and bump rewrite version | compacted base context + reason | updated session truth + rewrite ledger record | runtime/orchestrator | rewrite gate | bound |
| 03 | `SessionHistory::stage_rollback` | `crates/freehand-reason/src/session_history.rs` | stage rollback rewrite and bump rewrite version | rollback base context + reason + reference turn id | updated session truth + rewrite ledger record | runtime/orchestrator | rewrite gate | bound |
| 04 | `SessionHistory::stage_resume_rebuild` | `crates/freehand-reason/src/session_history.rs` | stage resume rebuild rewrite and bump rewrite version | rebuilt base context + reason + resume source | updated session truth + rewrite ledger record | runtime/orchestrator | rewrite gate | bound |
| 05 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | consume session rewrite state for turn startup | session history + turn input | planned turn request + provider payload | CLI/server/node | turn orchestrator | bound |
| 06 | `SessionHistory::commit_turn_start` | `crates/freehand-reason/src/session_history.rs` | clear one-shot non-ordinary rewrite mode after successful startup and stamp applied turn id | turn id | updated session truth | turn orchestrator | session-history owner | bound |
| 07 | `SessionHistory::persist_json` | `crates/freehand-reason/src/session_history.rs` | render persistable session truth snapshot | session history | JSON snapshot | runtime/debug/replay | persistence helper | bound |
| 08 | `SessionHistory::from_persisted_json` | `crates/freehand-reason/src/session_history.rs` | restore session truth from persisted JSON | JSON snapshot | session history | runtime/debug/replay | persistence helper | bound |

## Metadata / Request Isolation Notes

- `SessionHistory` owns rewrite mode/version, rewrite reason, reference ids, and rewrite diagnostics on the metadata/session-truth side
- planner request content is built later from `base_context_segments` plus turn-scoped additions; rewrite reasons and diagnostics never enter request text
- persistence methods serialize session truth, not provider request payloads

## Sync Status Against Code

- session-history baseline is landed in `freehand-reason`
- rewrite mode and rewrite version are now sourced from `SessionHistory` instead of turn-local constants
- compaction, rollback, and resume rebuild each have explicit owner methods
- persisted json/file round-trip is implemented for session truth baseline
- `ReasonRewriteRuntime` now consumes `reason.rewrite-policy` decisions before calling each rewrite gate
- remaining gap: final CLI/server runtime loop must supply real usage metrics and persisted recovery payloads
