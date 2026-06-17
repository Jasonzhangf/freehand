# Function Map: `reason.persistence`

- feature_id: `reason.persistence`
- owner crate: `crates/freehand-reason`
- owner module: `crates/freehand-reason/src/persistence.rs`
- owner entry symbols:
  - `ReasonPersistence::record_turn_started`
  - `ReasonPersistence::record_provider_output_applied`
  - `ReasonPersistence::record_completion_rejected`
  - `ReasonPersistence::record_turn_closed`
  - `ReasonPersistence::record_rewrite_state_updated`
  - `ReasonPersistence::record_provider_raw_event`
  - `ReasonPersistence::restore`
  - `SessionHistory::persist_json`
  - `SessionHistory::from_persisted_json`
  - `SessionHistory::persist_to_path`
  - `SessionHistory::load_from_path`

## Request Mainline

- runtime opens `~/.freehand/state/turns/<agent_id>/<session_id>/` as the authoritative reason state directory
- session-owned rewrite truth is restored from `SessionHistory` snapshots
- turn execution consumes restored session truth through `ReasonTurnEngine::start_turn`
- runtime persistence appends semantic reason-ledger rows before advancing durable snapshot cursors
- terminal turn close materializes immutable turn truth files and only then updates derived UI and index sidecars
- provider raw ledgers may be appended for debug, but they are not part of the authoritative request or recovery chain

## Response Mainline

- `SessionHistory` JSON/file helpers render and restore authoritative session rewrite truth
- reason persistence appends a reason-ledger row together with current session-history truth, then refreshes authoritative snapshots and derived sidecars
- reason persistence appends provider raw debug-ledger rows under `~/.freehand/ledgers/providers/<family>/<agent>/<session>/<turn>.jsonl` without mutating authoritative session truth
- reason persistence returns deterministic restore state from snapshot plus reason-ledger tail replay, or from reason-ledger-only rebuild when snapshots are missing or invalid
- terminal turn persistence yields immutable per-turn truth plus updated session cursor truth
- derived UI and index sidecars are regenerated from authoritative reason truth after durable writes complete

## Error Mainline

- invalid persisted snapshot JSON is rejected explicitly
- invalid persisted snapshot coherence is rejected explicitly
- reason-ledger sequence gaps or duplicate sequence numbers must block recovery
- provider raw payload availability alone must not mask missing authoritative reason truth
- UI sidecar presence alone must not be treated as session-truth recovery evidence

## Shared Multi-Reference Functions

- `SessionHistory::persist_json`
  - owner: `crates/freehand-reason/src/session_history.rs`
  - purpose: render authoritative session rewrite truth as a persistable JSON snapshot
  - allowed callers: runtime persistence owner, replay/debug tools, owner-crate tests
  - related tests: persisted JSON round-trip, snapshot render/load tests
  - why shared: authoritative session snapshot rendering must stay centralized
- `SessionHistory::from_persisted_json`
  - owner: `crates/freehand-reason/src/session_history.rs`
  - purpose: restore authoritative session rewrite truth from JSON while validating coherence
  - allowed callers: runtime persistence owner, replay/debug tools, owner-crate tests
  - related tests: invalid persisted state rejection, persisted JSON round-trip
  - why shared: restore validation must stay aligned with the authoritative snapshot renderer
- `validate_rewrite_base_segments`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: validate stable rewrite base segments before session snapshots are accepted or restored
  - allowed callers: `freehand-reason`, owner-crate tests
  - related tests: rewrite-base validation, persisted coherence rejection
  - why shared: rewrite-base semantic validation must not be duplicated in persistence coordinators
- `inspect_context_cache_diagnostics`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: compute metadata-side cache diagnostics stored in rewrite records and recovery evidence
  - allowed callers: `freehand-reason`, owner-crate tests, replay/debug tools
  - related tests: rewrite diagnostics snapshot tests, recovery audit tests
  - why shared: cache-shape evidence must stay aligned between planner runtime and persisted rewrite records
- `write_json_atomic`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: atomically replace authoritative snapshots and derived sidecars after durable ledger append
  - allowed callers: reason persistence owner, owner-crate tests
  - related tests: atomic snapshot replace, save/load smoke
  - why shared: all persistence file writes must use one atomic replacement path instead of ad hoc writes

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `SessionHistory::load_from_path` | `crates/freehand-reason/src/session_history.rs` | restore authoritative session rewrite snapshot from disk | session-history snapshot file | validated session rewrite truth | runtime/bootstrap | session-history owner | bound |
| 02 | `SessionHistory::from_persisted_json` | `crates/freehand-reason/src/session_history.rs` | validate restored session rewrite JSON | session-history JSON payload | validated session rewrite truth | persistence loader/debug tools | session-history owner | bound |
| 03 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | consume restored session truth for turn startup | restored session history + turn input | initialized turn record + provider payload | runtime/live bridge | reason owner | bound |
| 04 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | materialize semantic outputs into turn truth before persistence projection | provider semantic output | updated turn truth | runtime/live bridge | reason owner | bound |
| 05 | `SessionHistory::commit_turn_start` | `crates/freehand-reason/src/session_history.rs` | consume one-shot non-ordinary rewrite state after successful startup | active turn id | updated session rewrite truth | reason owner | session-history owner | bound |
| 06 | `ReasonPersistence::record_turn_started` | `crates/freehand-reason/src/persistence.rs` | append turn-start ledger row, refresh active-turn snapshot, and update cursor/sidecars | session history + started turn truth | durable reason state for running turn | runtime/live bridge/testkit | persistence owner | bound |
| 07 | `ReasonPersistence::record_provider_output_applied` | `crates/freehand-reason/src/persistence.rs` | append provider-output ledger row and refresh active-turn snapshot | session history + updated turn truth + provider semantic output | durable active-turn truth | runtime/live bridge/testkit | persistence owner | bound |
| 08 | `ReasonPersistence::record_completion_rejected` | `crates/freehand-reason/src/persistence.rs` | append schema-rejection ledger row and refresh active-turn rejection counter | session history + updated turn truth + rejection | durable rejection evidence | runtime/live bridge/testkit | persistence owner | bound |
| 09 | `ReasonPersistence::record_turn_closed` | `crates/freehand-reason/src/persistence.rs` | append terminal ledger row, materialize immutable turn truth, clear active-turn snapshot, and update sidecars | session history + terminal turn truth | durable closed-turn truth | runtime/live bridge/testkit | persistence owner | bound |
| 10 | `ReasonPersistence::record_rewrite_state_updated` | `crates/freehand-reason/src/persistence.rs` | append rewrite-state ledger row and refresh session snapshots | updated session-history truth | durable rewrite-state persistence | rewrite runtime / recovery path | persistence owner | bound |
| 11 | `ReasonPersistence::record_provider_raw_event` | `crates/freehand-reason/src/persistence.rs` | append debug-only provider raw ledger rows without mutating authoritative turn/session truth | provider family + session/turn/trace identity + raw wire body + scene provenance | durable provider raw debug evidence | runtime/live bridge | persistence owner | bound |
| 12 | `ReasonPersistence::restore` | `crates/freehand-reason/src/persistence.rs` | rebuild authoritative state from snapshots plus reason-ledger tail, or from ledger alone | snapshot directory + reason ledger | restored in-memory session and turn truth | runtime/bootstrap/testkit/CLI smoke | persistence owner | bound |

## Metadata / Request Isolation Notes

- authoritative snapshots store session and turn truth, not provider wire payloads
- reason-ledger rows may carry metadata-side diagnostics, but request-chain content remains separate from provider raw debug bodies
- provider raw ledgers are separate files under `~/.freehand/ledgers/providers` and must not be reinterpreted as authoritative request or turn truth
- derived UI sidecars and session indexes are downstream projections only and must not participate in recovery decisions

## Sync Status Against Code

- current code baseline now binds session-history JSON/file round-trip, reason-ledger append, provider-raw debug-ledger append, active-turn refresh, terminal turn materialization, derived sidecar writes, and snapshot-plus-tail / ledger-only recovery
- CLI and shared-harness smoke both bind to the persistence owner path without duplicating persistence semantics in the app layer
- live Anthropic runtime path now records provider raw response/error/event bodies through `ReasonPersistence::record_provider_raw_event` while keeping those ledgers outside recovery truth
- explicit owner-bound regression coverage now locks ledger sequence gaps plus provider-raw-only and UI-sidecar-only missing-recovery rejection
- explicit owner-bound regression coverage now also locks invalid persisted snapshot JSON, invalid snapshot coherence, and duplicate-sequence recovery rejection
- migrated mainline-call source now lives at `docs/mainline-calls/reason.persistence.json` and generated wiki lives at `docs/wiki/reason.persistence.md`
