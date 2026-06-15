# Function Map: `reason.persistence`

- feature_id: `reason.persistence`
- owner crate: `crates/freehand-reason`
- owner module: current baseline `crates/freehand-reason/src/session_history.rs`; dedicated runtime persistence module binding remains pending inside `crates/freehand-reason`
- owner entry symbols:
  - `SessionHistory::persist_json`
  - `SessionHistory::from_persisted_json`
  - `SessionHistory::persist_to_path`
  - `SessionHistory::load_from_path`
  - runtime snapshot and ledger entry symbols: binding pending

## Request Mainline

- runtime opens `~/.freehand/state/turns/<agent_id>/<session_id>/` as the authoritative reason state directory
- session-owned rewrite truth is restored from `SessionHistory` snapshots
- turn execution consumes restored session truth through `ReasonTurnEngine::start_turn`
- runtime persistence appends semantic reason-ledger rows before advancing durable snapshot cursors
- terminal turn close materializes immutable turn truth files and only then updates derived UI and index sidecars
- provider raw ledgers may be appended for debug, but they are not part of the authoritative request or recovery chain

## Response Mainline

- `SessionHistory` JSON/file helpers render and restore authoritative session rewrite truth
- reason persistence returns deterministic restore state from snapshot plus reason-ledger tail replay, or from reason-ledger-only rebuild when snapshots are missing or invalid
- terminal turn persistence yields immutable per-turn truth plus updated session cursor truth
- derived UI and index sidecars are regenerated from authoritative reason truth after durable writes complete

## Error Mainline

- invalid persisted snapshot JSON is rejected explicitly
- invalid persisted snapshot coherence is rejected explicitly
- reason-ledger sequence gaps or duplicate sequence numbers must block recovery
- provider raw payload availability must not mask missing authoritative reason truth
- UI sidecar presence must not be treated as session-truth recovery evidence

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

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `SessionHistory::load_from_path` | `crates/freehand-reason/src/session_history.rs` | restore authoritative session rewrite snapshot from disk | session-history snapshot file | validated session rewrite truth | runtime/bootstrap | session-history owner | bound |
| 02 | `SessionHistory::from_persisted_json` | `crates/freehand-reason/src/session_history.rs` | validate restored session rewrite JSON | session-history JSON payload | validated session rewrite truth | persistence loader/debug tools | session-history owner | bound |
| 03 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | consume restored session truth for turn startup | restored session history + turn input | initialized turn record + provider payload | runtime/live bridge | reason owner | bound |
| 04 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | materialize semantic outputs into turn truth before persistence projection | provider semantic output | updated turn truth | runtime/live bridge | reason owner | bound |
| 05 | `SessionHistory::commit_turn_start` | `crates/freehand-reason/src/session_history.rs` | consume one-shot non-ordinary rewrite state after successful startup | active turn id | updated session rewrite truth | reason owner | session-history owner | bound |
| 06 | `SessionHistory::persist_json` | `crates/freehand-reason/src/session_history.rs` | render authoritative session rewrite snapshot | session history | JSON snapshot payload | persistence writer/debug tools | session-history owner | bound |
| 07 | `SessionHistory::persist_to_path` | `crates/freehand-reason/src/session_history.rs` | persist authoritative session rewrite snapshot to filesystem | session history + target path | updated snapshot file | persistence writer/debug tools | session-history owner | bound |
| 08 | `binding pending: reason-ledger append writer` | `crates/freehand-reason/**` | append monotonic semantic and rewrite evidence before snapshot cursor advancement | turn event or rewrite evidence | durable reason-ledger row | runtime persistence owner | filesystem append path | binding pending |
| 09 | `binding pending: active-turn snapshot writer` | `crates/freehand-reason/**` | atomically refresh active-turn truth after durable ledger append | in-memory active turn truth + cursor state | refreshed `active-turn.json` | runtime persistence owner | filesystem snapshot path | binding pending |
| 10 | `binding pending: terminal turn materializer` | `crates/freehand-reason/**` | close a turn into immutable turn truth and update session cursor | terminal turn truth + latest durable sequence | `turns/<turn_id>.json` + refreshed cursor snapshot | runtime persistence owner | filesystem snapshot path | binding pending |
| 11 | `binding pending: snapshot-plus-ledger recovery coordinator` | `crates/freehand-reason/**` | rebuild authoritative state from snapshots plus reason-ledger tail, or from ledger alone | snapshot directory + reason ledger | restored in-memory session and turn truth | runtime/bootstrap | persistence owner | binding pending |

## Metadata / Request Isolation Notes

- authoritative snapshots store session and turn truth, not provider wire payloads
- reason-ledger rows may carry metadata-side diagnostics, but request-chain content remains separate from provider raw debug bodies
- provider raw ledgers are separate files under `~/.freehand/ledgers/providers` and must not be reinterpreted as authoritative request or turn truth
- derived UI sidecars and session indexes are downstream projections only and must not participate in recovery decisions

## Sync Status Against Code

- current code baseline already binds session-history JSON and file round-trip helpers
- current code baseline already binds turn startup and provider-output materialization against restored session history
- runtime snapshot coordinator, reason-ledger append writer, terminal turn materializer, and recovery coordinator are not yet implemented
- authoritative three-layer persistence split is design-locked even though several binding rows remain pending
