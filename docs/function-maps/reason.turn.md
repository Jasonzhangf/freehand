# Function Map: `reason.turn`

- feature_id: `reason.turn`
- owner crate: `crates/freehand-reason`
- owner module: `crates/freehand-reason/src/lib.rs`
- mainline call source: `docs/mainline-calls/reason.turn.json`
- generated wiki: `docs/wiki/reason.turn.md`
- owner entry symbols:
  - `ReasonTurnEngine::start_turn`
  - `ReasonTurnEngine::apply_provider_output`
  - `ReasonTurnEngine::submit_completion`
- `ReasonTurnEngine::project_session`

## Request Mainline

- user input and context material enter the turn orchestration path
- `reason.session-history` provides stable base context plus the current turn's `rewrite_mode` and `rewrite_version`
- `reason.rewrite-policy` owns the decision of whether runtime should stay append-only or call a rewrite gate before turn startup
- provider `TokenUsage` is converted into compaction prompt pressure through `prompt_tokens_from_usage`
- turn orchestration manages tool re-entry and delegates context planning to `reason.context-planner`
- current code baseline now calls `plan_context`, carries typed `context_segments`, stores planner diagnostics separately, and derives provider payload from `input_segments`
- stable-prefix and volatile-tail separation now exists at planner baseline level, and rewrite sourcing now comes from `SessionHistory`
- metadata and request-chain content must stay separate types; `freehand-reason` owns request-content composition truth
- turn startup writes internal control metadata through `metadata.core` after request/provider payload construction and before session-history commit

## Response Mainline

- provider semantic events become turn truth updates
- provider semantic outputs write owner/node-provenance metadata before mutating turn truth
- turn truth broadcasts semantic events for reasoning, text, tool, usage, terminal, and error
- turn lifecycle and provider-output milestones may emit debug events into `debug.core`
- terminal result is projected from validated completion schema, not raw provider finish reason
- cancel requests become explicit cancelled terminal events through the reason owner rather than failed terminal events
- completion schema is extracted from `<freehand_completion>...</freehand_completion>` tagged JSON before validation
- invalid completion schema feedback identifies concrete invalid schema entries
- provider metadata signals may influence orchestration decisions only through explicit typed fields, never by hidden prompt mutation

## Error Mainline

- invalid completion schema is rejected and reprompted with field-level feedback
- invalid completion schema retries are capped at 3 before a failed terminal outcome is written
- provider `finish_reason=stop` or `finish_reason=end_turn` does not end the turn by itself
- UI/runtime cancellation is represented as `TerminalStatus::Cancelled`, not as a failed or successful terminal outcome
- raw provider events go to debug ledger, not session truth
- debug emission is observation-only and must not mutate turn/session truth
- metadata/request boundary violations must be treated as architecture errors, not silently tolerated
- metadata write failures are explicit `ReasonTurnError::MetadataWriteFailed` errors and must stop the affected mutation instead of being swallowed

## Shared Multi-Reference Functions

- `validate_completion_submission`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: shared pure completion-schema validator for terminal acceptance
  - allowed callers: reason orchestrator, tests
  - related tests: completion acceptance, invalid schema rejection, blocked terminal tests
  - why shared: keeps terminal validation semantics out of orchestrator glue
- `parse_completion_submission_block`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: extract and parse tagged completion JSON into a typed submission or itemized schema errors
  - allowed callers: reason orchestrator, live bridge, tests
  - related tests: tagged schema extraction, missing tag rejection, invalid JSON rejection, invalid claim rejection
  - why shared: keeps completion schema parsing out of live/provider/app orchestration
- `DebugHub::emit`
  - owner: `crates/freehand-debug/src/lib.rs`
  - purpose: receive reason-turn debug emissions without making `freehand-reason` the observation delivery owner
  - allowed callers: reason orchestrator, future runtime bridges
  - related tests: reason debug emission smoke
  - why shared: keeps debug distribution separate from turn truth ownership
- `DebugHub::subscribe_failures`
  - owner: `crates/freehand-debug/src/lib.rs`
  - purpose: surface observation-only sink-dispatch failures without rewriting turn truth
  - allowed callers: reason tests, future runtime/provider/node observers
  - related tests: reason debug sink failure surfacing smoke
  - why shared: keeps debug failure observability outside session/turn business truth
- `MetadataCenter::write`
  - owner: `crates/freehand-metadata/src/lib.rs`
  - purpose: receive reason-turn internal control/provenance metadata with writer owner and write-node provenance
  - allowed callers: reason producer integration, future runtime/provider/node/debug producers
  - related tests: reason producer metadata provenance, request-text isolation, metadata-write failure tests
  - why shared: keeps metadata admission and request-data-key rejection in `metadata.core` instead of local reason maps

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | create per-turn truth container and provider payload from session-owned rewrite state | session history + user input + turn-scoped additions | initialized turn record | CLI/server/node | reason orchestrator | bound |
| 01 note | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | current startup path reads rewrite mode/version and base context from `SessionHistory`, invokes planner-owned segment admission, and stores planner diagnostics while keeping them off request content | request-chain content + session metadata inputs | provider-ready typed request content + metadata-side cache diagnostics | reason orchestrator | `plan_context` | bound |
| 01 metadata | `ReasonTurnEngine::write_metadata` | `crates/freehand-reason/src/lib.rs` | write start-turn control metadata with owner/node provenance after payload construction and before history commit | turn record + rewrite/model diagnostics | validated metadata envelope in metadata center or explicit metadata error | `ReasonTurnEngine::start_turn` | `MetadataCenter::write` | bound |
| 02 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | materialize provider semantic output into turn truth | provider semantic output | updated turn state | provider boundary | turn state writer | bound |
| 02 metadata | `ReasonTurnEngine::write_provider_output_metadata` | `crates/freehand-reason/src/lib.rs` | classify provider output control metadata before turn mutation | provider semantic output + turn identity | metadata entries for output kind, tool/usage/error/provider terminal control facts | `ReasonTurnEngine::apply_provider_output` | `ReasonTurnEngine::write_metadata` | bound |
| 03 | `parse_completion_submission_block` | `crates/freehand-blocks/src/lib.rs` | parse tagged completion schema from model text | model text with tagged JSON | typed completion submission or itemized parse errors | turn/live runtime | completion parser | bound |
| 04 | `validate_completion_submission` | `crates/freehand-blocks/src/lib.rs` | validate completion schema | completion submission | completion decision or rejection | turn state writer | terminal validator | bound |
| 05 | `ReasonTurnEngine::submit_completion` | `crates/freehand-reason/src/lib.rs` | accept or reject terminal outcome | candidate completion payload | terminal event or rejection | turn state writer | terminal validator | bound |
| 06 | `ReasonTurnEngine::fail_turn` | `crates/freehand-reason/src/lib.rs` | write explicit failed terminal outcome after retry exhaustion | failure reason | failed terminal event | turn/live runtime | turn state writer | bound |
| 06a | `ReasonTurnEngine::cancel_turn` | `crates/freehand-reason/src/lib.rs` | write explicit cancelled terminal outcome for user/runtime cancellation | cancellation reason | cancelled terminal event | runtime cancel dispatch | turn state writer | bound |
| 07 | `ReasonTurnEngine::project_session` | `crates/freehand-reason/src/lib.rs` | project conversation view from turns | turn records | projected session view | UI/session consumers | projector | bound |
| 08 | `ReasonTurnEngine::emit_debug` | `crates/freehand-reason/src/lib.rs` | emit observation-only debug event for turn lifecycle or provider-output milestones | turn truth + scene metadata + status/detail text | debug event fanout | reason orchestrator | `DebugHub::emit` | bound |

## Sync Status Against Code

- turn startup, provider-output materialization, completion parsing/validation, failed terminal writing, and session projection are bound in code
- explicit cancelled terminal writing via `ReasonTurnEngine::cancel_turn` is bound in code
- planner baseline is implemented and called from turn startup
- `reason.session-history` now owns rewrite version and explicit rewrite-gate orchestration for turn startup
- `ReasonRewriteRuntime` now provides the baseline consumer path for calling `reason.rewrite-policy` and then triggering compaction/rollback/resume gates
- provider usage conversion into rewrite policy is bound
- debug emission into `debug.core` is bound for start-turn, provider-output application, completion acceptance/rejection, and explicit failed terminal write
- current debug emission remains observation-only; sink-dispatch failures surface through `DebugHub::subscribe_failures` and are not promoted into turn truth or reason error events
- metadata emission into `metadata.core` is bound for start-turn and provider-output application
- metadata write failures are explicit and are tested to prevent start-turn history commit or provider-output turn mutation
- remaining gap is final CLI/server runtime loop integration with real provider usage events and persisted recovery payloads
- metadata/request hard isolation is reflected in request content vs planner diagnostics split and now enforced for reason-turn producer metadata through `metadata.core` key validation and producer tests; static repo-wide metadata leak gate remains pending
- the generated wiki must be regenerated from `docs/mainline-calls/reason.turn.json` when this function-map truth changes
