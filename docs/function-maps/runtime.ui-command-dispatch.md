# Function Map: `runtime.ui-command-dispatch`

- feature_id: `runtime.ui-command-dispatch`
- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- owner entry symbols:
  - `RuntimeCommandDispatcher::new`
  - `RuntimeCommandDispatcher::from_selected_agent`
  - `RuntimeCommandDispatcher::from_default_config`
  - `RuntimeCommandDispatcher::dispatch`
  - `RuntimeCommandDispatcher::ui_state`
  - `run_live_reason_turn_with_hooks`

## Request Mainline

- accepted UI command ingress arrives as a `UiCommandDispatchEnvelope`
- runtime bootstrap may first select one configured agent from `~/.freehand/config.toml`
- config-selected bootstrap consumes local node id, paired node id, paired allowed IP, and paired token env from `config.core`
- config-selected live bootstrap may also seed one shared metadata ledger path for node-owned bootstrap and pairing provenance
- submit commands may carry an optional selected session id so a draft or explicitly chosen session can receive the new turn instead of always using the default session
- live bootstrap may restore persisted session truth and prior turn projections before the next command runs
- runtime dispatch owner reads the declared owner target from the envelope
- live submit registers active turn cancellation state before provider execution and releases the runtime mutex before provider IO
- `CancelLatestActiveTurn` resolves to the newest active live turn before falling back to latest persisted runtime turn
- runtime dispatch routes the command into reason, node, or checkpoint owner adapters without letting the app own those semantics

## Response Mainline

- reason-backed submit/cancel commands return dispatch receipts and update derived UI turn projections, including the original user prompt and explicit cancelled terminal status for public conversation projection
- live provider-backed submit incrementally writes reason/debug projection updates into `UiProtocolState` while the turn is still running
- live provider-backed submit maps `ReasonBroadcastEvent::ToolResult` into `UiProtocolState::apply_tool_result` so tool lifecycle completion is visible over turn SSE
- live provider-backed submit publishes the user prompt into `UiProtocolState` before provider events so blank UI subscriptions can render a complete public conversation stream
- live provider-backed submit honors a selected session id when present and keeps the derived UI state pinned to that session instead of the latest global session
- active live cancel requests set the active cancel token immediately and publish a cancelled UI projection without waiting for provider completion
- latest-active cancellation supports Esc during the short window before WebUI has received a concrete `turn_id`
- live provider-backed multi-round turns keep the original operator prompt in public UI projection instead of exposing internal continuation prompts
- final live multi-round UI projection preserves final-round visible text, usage, errors, and terminal status while aggregating tool-call/tool-result lifecycle activity across runtime rounds
- live bootstrap restores multi-round UI projections from reason-ledger snapshots so earlier-round tool activity remains visible after daemon restart
- live provider-backed submit refreshes `UiProtocolState` from authoritative persistence before returning dispatch failure when the live bridge materializes failed turn truth
- node-backed direct-message commands return dispatch receipts after owner validation
- runtime-backed rewind commands restore checkpointed workspace state without mutating reason/session/UI truth directly
- config-selected runtime bootstrap returns one dispatcher for the requested agent
- config-selected live bootstrap may materialize node-owned bootstrap and pairing metadata into the shared metadata ledger before the first command runs
- live bootstrap rehydrates `UiProtocolState` from persisted turn truth and resumes runtime turn-id allocation from persisted ordinals
- live bootstrap groups persisted `runtime-turn-N` round snapshots into one UI projection when restoring session transcripts, while keeping authoritative closed-turn recovery unchanged
- runtime-owned UI state reflects derived projections only, not authoritative turn truth

## Error Mainline

- unsupported runtime command paths return explicit dispatch-port failures
- missing turn targets for cancel/resume return explicit dispatch-port failures
- cancelled live turns return explicit cancelled dispatch failure to the original submitter and must not overwrite cancelled UI projection with later provider success
- live provider/tool loops check cancellation at round, stream callback, provider-output, tool-execution, and terminal-write boundaries
- live provider/tool failures that materialize failed turn truth are returned as explicit dispatch failures only after the failed projection has been refreshed into `UiProtocolState`
- `CancelLatestActiveTurn` with no active or persisted turn returns explicit target-not-found
- missing checkpoint manifests return explicit dispatch target-not-found failures
- wrong slave target node returns explicit dispatch-port failures
- missing config, invalid agent selection, paired-token mismatch, or slave-mode host selection return explicit bootstrap failures
- invalid persisted recovery truth or node-metadata bootstrap failure returns explicit runtime bootstrap failure
- unwritable shared node metadata ledgers fail bootstrap explicitly as `NodeRuntimeInit` and must not materialize a runtime dispatcher

## Shared Multi-Reference Functions

- `build_command_dispatch_envelope`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: declare command owner routing before runtime dispatch
  - allowed callers: runtime dispatch ports, transport adapters
  - related tests: command dispatch envelope owner-routing smoke
  - why shared: keeps command-to-owner routing out of app/runtime glue duplication
- `load_default_config`
  - owner: `crates/freehand-config/src/lib.rs`
  - purpose: load the default config file before runtime host bootstrap
  - allowed callers: runtime bootstrap helpers, CLI/startup tests
  - related tests: config load smoke, runtime bootstrap smoke
  - why shared: keeps config loading and selection in the config owner

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `RuntimeCommandDispatcher::new` | `crates/freehand-runtime/src/lib.rs` | compose first runtime owner wiring for reason/node command dispatch and live node metadata bootstrap | runtime config | runtime dispatcher | runtime bootstrap/tests | runtime owner | bound |
| 02 | `RuntimeCommandDispatcher::from_selected_agent` | `crates/freehand-runtime/src/lib.rs` | derive runtime bootstrap config from one selected agent config | selected agent config | runtime dispatcher | daemon/bootstrap tests | runtime bootstrap | bound |
| 03 | `RuntimeCommandDispatcher::from_default_config` | `crates/freehand-runtime/src/lib.rs` | load default config and bootstrap one runtime dispatcher | agent name | runtime dispatcher | daemon host | config owner + runtime bootstrap | bound |
| 04 | `RuntimeCommandDispatcher::dispatch` | `crates/freehand-runtime/src/lib.rs` | execute protocol-owned dispatch envelope through the correct owner adapter | dispatch envelope | dispatch receipt or failure | app/daemon runtime boundary | reason/node owner adapter | bound |
| 05 | `RuntimeCommandDispatcher::ui_state` | `crates/freehand-runtime/src/lib.rs` | expose derived UI projection state for runtime-side consumers/tests | runtime dispatcher | shared derived UI state | runtime tests/future daemon | UI protocol state | bound |
| 06 | `run_live_reason_turn_with_hooks` | `crates/freehand-runtime/src/lib.rs` | execute a live provider turn while streaming reason/debug/tool-result callbacks to runtime-owned consumers | selected live config + live request + callbacks | live turn outcome plus incremental callbacks | runtime dispatch/tests | live bridge owner | bound |
| 07 | `RuntimeCommandDispatcher::prepare_live_submit_user_input` | `crates/freehand-runtime/src/lib.rs` | register active live turn cancellation state before provider execution and bind an optional requested session id | runtime state + submitted user text + optional requested session id | prepared live submit + active cancel token | `RuntimeCommandDispatcher::dispatch` | runtime owner | bound |
| 08 | `RuntimeCommandDispatcher::dispatch_prepared_live_submit` | `crates/freehand-runtime/src/lib.rs` | run provider-backed live turn outside runtime mutex while honoring active cancel token | prepared live submit | live receipt or cancelled dispatch failure | `RuntimeCommandDispatcher::dispatch` | `run_live_reason_turn_with_hooks` | bound |
| 09 | `RuntimeCommandDispatcher::dispatch_cancel_turn` | `crates/freehand-runtime/src/lib.rs` | cancel active or persisted turns through reason-owned terminal semantics and UI projection | cancel command turn id | cancel receipt + cancelled projection | `RuntimeCommandDispatcher::dispatch` | reason owner / active cancel registry | bound |
| 10 | `restore_all_persisted_sessions_into_ui` | `crates/freehand-runtime/src/lib.rs` | rehydrate UI protocol state from reason-ledger turn snapshots and group same-ordinal runtime rounds for transcript projection | persisted reason sessions | derived UI session list/transcripts with retained tool activity | runtime bootstrap | reason persistence + UI protocol | bound |

## Sync Status Against Code

- runtime dispatch owner baseline is now bound in code
- provider-backed submit input and cancel dispatch through `reason.turn` and update derived UI turn projections
- live provider submit now streams reason/debug updates into `UiProtocolState` before final receipt is returned
- live provider submit now streams tool-result updates into `UiProtocolState` so tool activities can transition to completed before final receipt
- live submit now releases the runtime mutex before provider IO so `CancelTurn` can enter concurrently
- active live cancel now publishes cancelled UI projection immediately and later provider success cannot overwrite it
- runtime dispatch now supports `CancelLatestActiveTurn` for current-turn stop without requiring the UI to know `turn_id`
- runtime live bridge cancellation checkpoints now have positive and negative coverage before tool execution and terminal persistence
- missing `CancelTurn`, empty `CancelLatestActiveTurn`, and wrong-node direct-message dispatch paths now stay explicit target-not-found failures
- direct slave message dispatch routes through `node.master-slave`
- explicit checkpoint rewind dispatch now routes through `runtime.checkpoint-rewind`
- missing checkpoint rewind manifests now stay explicit target-not-found dispatch failures instead of being collapsed into generic success or fallback projection
- resume dispatch remains an explicit unsupported runtime path
- config-selected runtime bootstrap is now bound in code
- config-selected runtime bootstrap uses explicit peer-topology config instead of synthetic paired node ids
- config-selected live bootstrap now seeds a shared metadata ledger path into `node.master-slave` before the first command runs
- unwritable shared node metadata ledgers are now regression-locked as explicit bootstrap failures
- config-selected live bootstrap restores persisted turn projection and next runtime turn ordinal when recovery truth exists
- config-selected live bootstrap now restores multi-round tool activity into UI session transcripts after daemon restart
- final live projection now aggregates only cross-round tool lifecycle state so earlier-round tool activity remains visible without leaking intermediate continuation text into the final latest turn
- failed live bridge tool execution now refreshes runtime UI state from persisted failed turn truth before returning the dispatch error, so WebUI query/SSE can observe failure instead of waiting forever
- migrated mainline-call source now lives at `docs/mainline-calls/runtime.ui-command-dispatch.json` and generated wiki lives at `docs/wiki/runtime.ui-command-dispatch.md`
