# Function Map: `reason.turn`

- feature_id: `reason.turn`
- owner crate: `crates/freehand-reason`
- owner module: `crates/freehand-reason/src/lib.rs`
- owner entry symbols:
  - `ReasonTurnEngine::start_turn`
  - `ReasonTurnEngine::apply_provider_output`
  - `ReasonTurnEngine::submit_completion`
  - `ReasonTurnEngine::project_session`

## Request Mainline

- user input and context material enter the turn orchestration path
- turn orchestration renders provider-ready input and manages tool re-entry

## Response Mainline

- provider semantic events become turn truth updates
- turn truth broadcasts semantic events for reasoning, text, tool, usage, terminal, and error
- terminal result is projected from validated completion schema, not raw provider finish reason

## Error Mainline

- invalid completion schema is rejected and reprompted
- provider `finish_reason=stop` or `finish_reason=end_turn` does not end the turn by itself
- raw provider events go to debug ledger, not session truth

## Shared Multi-Reference Functions

- `validate_completion_submission`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: shared pure completion-schema validator for terminal acceptance
  - allowed callers: reason orchestrator, tests
  - related tests: completion acceptance, invalid schema rejection, blocked terminal tests
  - why shared: keeps terminal validation semantics out of orchestrator glue

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | create per-turn truth container and provider payload | user input + session state | initialized turn record | CLI/server/node | reason orchestrator | bound |
| 02 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | materialize provider semantic output into turn truth | provider semantic output | updated turn state | provider boundary | turn state writer | bound |
| 03 | `validate_completion_submission` | `crates/freehand-blocks/src/lib.rs` | validate completion schema | completion submission | completion decision or rejection | turn state writer | terminal validator | bound |
| 04 | `ReasonTurnEngine::submit_completion` | `crates/freehand-reason/src/lib.rs` | accept or reject terminal outcome | candidate completion payload | terminal event or rejection | turn state writer | terminal validator | bound |
| 05 | `ReasonTurnEngine::project_session` | `crates/freehand-reason/src/lib.rs` | project conversation view from turns | turn records | projected session view | UI/session consumers | projector | bound |

## Sync Status Against Code

- turn startup, provider-output materialization, completion validation, and session projection are bound in code
