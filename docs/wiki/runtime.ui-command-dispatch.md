# Wiki: `runtime.ui-command-dispatch`

Generated from `docs/mainline-calls/runtime.ui-command-dispatch.json`. Do not edit by hand.

- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- function map: `docs/function-maps/runtime.ui-command-dispatch.md`
- generated wiki: `docs/wiki/runtime.ui-command-dispatch.md`
- test design: `docs/testing/runtime.ui-command-dispatch.md`

## Request Mainline

- accepted UI command ingress arrives as a `UiCommandDispatchEnvelope`
- runtime bootstrap may first select one configured agent from `~/.freehand/config.toml`
- config-selected bootstrap consumes local node id, paired node id, paired allowed IP, and paired token env from `config.core`
- config-selected live bootstrap may also seed one shared metadata ledger path for node-owned bootstrap and pairing provenance
- live bootstrap may restore persisted session truth and prior turn projections before the next command runs
- runtime dispatch owner reads the declared owner target from the envelope
- runtime dispatch routes the command into reason, node, or checkpoint owner adapters without letting the app own those semantics
- live submit registers an active turn cancel token before provider execution and releases the runtime mutex before running provider IO
- CancelLatestActiveTurn resolves to the newest active live turn before falling back to latest persisted runtime turn

## Response Mainline

- reason-backed submit/cancel commands return dispatch receipts and update derived UI turn projections, including the original user prompt and explicit cancelled terminal status for public conversation projection
- live provider-backed submit publishes the user prompt into `UiProtocolState` before provider events and incrementally writes reason/debug projection updates while the turn is still running
- live provider-backed multi-round turns keep the original operator prompt in public UI projection instead of exposing internal continuation prompts
- node-backed direct-message commands return dispatch receipts after owner validation
- runtime-backed rewind commands restore checkpointed workspace state without mutating reason/session/UI truth directly
- config-selected runtime bootstrap returns one dispatcher for the requested agent
- config-selected live bootstrap may materialize node-owned bootstrap and pairing metadata into the shared metadata ledger before the first command runs
- live bootstrap rehydrates `UiProtocolState` from persisted turn truth and resumes runtime turn-id allocation from persisted ordinals
- runtime-owned UI state reflects derived projections only, not authoritative turn truth
- active live cancel requests set the active cancel token immediately and publish a cancelled UI projection without waiting for provider completion
- latest-active cancellation supports Esc during the short window before WebUI has received a concrete turn_id

## Error Mainline

- unsupported runtime command paths return explicit dispatch-port failures
- missing turn targets for cancel/resume return explicit dispatch-port failures
- missing checkpoint manifests return explicit dispatch target-not-found failures
- wrong slave target node returns explicit dispatch-port failures
- missing config, invalid agent selection, paired-token mismatch, or slave-mode host selection return explicit bootstrap failures
- unwritable shared node metadata ledgers fail bootstrap explicitly as NodeRuntimeInit and must not materialize a runtime dispatcher
- invalid persisted recovery truth or node-metadata bootstrap failure returns explicit runtime bootstrap failure
- cancelled live turns return explicit cancelled dispatch failure to the original submitter and must not overwrite cancelled UI projection with later provider success
- live provider/tool loops check cancellation at round, stream callback, provider-output, tool-execution, and terminal-write boundaries
- CancelLatestActiveTurn with no active or persisted turn returns explicit target-not-found

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
| 03 | `RuntimeCommandDispatcher::from_default_config` | `crates/freehand-runtime/src/lib.rs` | load default config and bootstrap one runtime dispatcher | agent name | runtime dispatcher | daemon host | config owner plus runtime bootstrap | bound |
| 04 | `RuntimeCommandDispatcher::dispatch` | `crates/freehand-runtime/src/lib.rs` | execute protocol-owned dispatch envelope through the correct owner adapter | dispatch envelope | dispatch receipt or failure | app/daemon runtime boundary | reason/node owner adapter | bound |
| 05 | `RuntimeCommandDispatcher::ui_state` | `crates/freehand-runtime/src/lib.rs` | expose derived UI projection state for runtime-side consumers/tests | runtime dispatcher | shared derived UI state | runtime tests/future daemon | UI protocol state | bound |
| 06 | `run_live_reason_turn_with_hooks` | `crates/freehand-runtime/src/lib.rs` | execute a live provider turn while streaming reason/debug callbacks to runtime-owned consumers | selected live config plus live request plus callbacks | live turn outcome plus incremental callbacks | runtime dispatch/tests | live bridge owner | bound |
| 07 | `RuntimeCommandDispatcher::prepare_live_submit_user_input` | `crates/freehand-runtime/src/lib.rs` | register active live turn cancellation state before provider execution | runtime state plus submitted user text | prepared live submit plus active cancel token | RuntimeCommandDispatcher::dispatch | runtime owner | bound |
| 08 | `RuntimeCommandDispatcher::dispatch_prepared_live_submit` | `crates/freehand-runtime/src/lib.rs` | run provider-backed live turn outside runtime mutex while honoring active cancel token | prepared live submit | live receipt or cancelled dispatch failure | RuntimeCommandDispatcher::dispatch | run_live_reason_turn_with_hooks | bound |
| 09 | `RuntimeCommandDispatcher::dispatch_cancel_turn` | `crates/freehand-runtime/src/lib.rs` | cancel active or persisted turns through reason-owned terminal semantics and UI projection | cancel command turn id | cancel receipt plus cancelled projection | RuntimeCommandDispatcher::dispatch | reason owner / active cancel registry | bound |

## Sync Status Against Mainline Call

- runtime dispatch owner baseline is now bound in code
- provider-backed submit input and cancel dispatch through `reason.turn` and update derived UI turn projections
- live provider submit now streams reason/debug updates into `UiProtocolState` before final receipt is returned
- direct slave message dispatch routes through `node.master-slave`
- explicit checkpoint rewind dispatch now routes through `runtime.checkpoint-rewind`
- missing checkpoint rewind manifests now stay explicit target-not-found dispatch failures instead of being collapsed into generic success or fallback projection
- resume dispatch remains an explicit unsupported runtime path
- config-selected runtime bootstrap is now bound in code
- config-selected runtime bootstrap uses explicit peer-topology config instead of synthetic paired node ids
- config-selected live bootstrap now seeds a shared metadata ledger path into `node.master-slave` before the first command runs
- unwritable shared node metadata ledgers are now regression-locked as explicit bootstrap failures
- config-selected live bootstrap restores persisted turn projection and next runtime turn ordinal when recovery truth exists
- generated wiki must be regenerated from `docs/mainline-calls/runtime.ui-command-dispatch.json` when this function-map truth changes
- live submit now releases the runtime mutex before provider IO so CancelTurn can enter concurrently
- active live cancel now publishes cancelled UI projection immediately and later provider success cannot overwrite it
- runtime dispatch now supports CancelLatestActiveTurn for current-turn stop without requiring the UI to know turn_id
- runtime live bridge cancellation checkpoints now have positive and negative coverage before tool execution and terminal persistence
- missing CancelTurn, empty CancelLatestActiveTurn, and wrong-node direct-message dispatch paths now stay explicit target-not-found failures
