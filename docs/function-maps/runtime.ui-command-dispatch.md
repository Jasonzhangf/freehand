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
- live bootstrap may restore persisted session truth and prior turn projections before the next command runs
- runtime dispatch owner reads the declared owner target from the envelope
- runtime dispatch routes the command into reason, node, or checkpoint owner adapters without letting the app own those semantics

## Response Mainline

- reason-backed submit/cancel commands return dispatch receipts and update derived UI turn projections, including the original user prompt for public conversation projection
- live provider-backed submit incrementally writes reason/debug projection updates into `UiProtocolState` while the turn is still running
- live provider-backed submit publishes the user prompt into `UiProtocolState` before provider events so blank UI subscriptions can render a complete public conversation stream
- live provider-backed multi-round turns keep the original operator prompt in public UI projection instead of exposing internal continuation prompts
- node-backed direct-message commands return dispatch receipts after owner validation
- runtime-backed rewind commands restore checkpointed workspace state without mutating reason/session/UI truth directly
- config-selected runtime bootstrap returns one dispatcher for the requested agent
- live bootstrap rehydrates `UiProtocolState` from persisted turn truth and resumes runtime turn-id allocation from persisted ordinals
- runtime-owned UI state reflects derived projections only, not authoritative turn truth

## Error Mainline

- unsupported runtime command paths return explicit dispatch-port failures
- missing turn targets for cancel/resume return explicit dispatch-port failures
- missing checkpoint manifests return explicit dispatch target-not-found failures
- wrong slave target node returns explicit dispatch-port failures
- missing config, invalid agent selection, paired-token mismatch, or slave-mode host selection return explicit bootstrap failures
- invalid persisted recovery truth returns explicit runtime bootstrap failure

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
| 01 | `RuntimeCommandDispatcher::new` | `crates/freehand-runtime/src/lib.rs` | compose first runtime owner wiring for reason/node command dispatch | runtime config | runtime dispatcher | runtime bootstrap/tests | runtime owner | bound |
| 02 | `RuntimeCommandDispatcher::from_selected_agent` | `crates/freehand-runtime/src/lib.rs` | derive runtime bootstrap config from one selected agent config | selected agent config | runtime dispatcher | daemon/bootstrap tests | runtime bootstrap | bound |
| 03 | `RuntimeCommandDispatcher::from_default_config` | `crates/freehand-runtime/src/lib.rs` | load default config and bootstrap one runtime dispatcher | agent name | runtime dispatcher | daemon host | config owner + runtime bootstrap | bound |
| 04 | `RuntimeCommandDispatcher::dispatch` | `crates/freehand-runtime/src/lib.rs` | execute protocol-owned dispatch envelope through the correct owner adapter | dispatch envelope | dispatch receipt or failure | app/daemon runtime boundary | reason/node owner adapter | bound |
| 05 | `RuntimeCommandDispatcher::ui_state` | `crates/freehand-runtime/src/lib.rs` | expose derived UI projection state for runtime-side consumers/tests | runtime dispatcher | shared derived UI state | runtime tests/future daemon | UI protocol state | bound |
| 06 | `run_live_reason_turn_with_hooks` | `crates/freehand-runtime/src/lib.rs` | execute a live provider turn while streaming reason/debug callbacks to runtime-owned consumers | selected live config + live request + callbacks | live turn outcome plus incremental callbacks | runtime dispatch/tests | live bridge owner | bound |

## Sync Status Against Code

- runtime dispatch owner baseline is now bound in code
- provider-backed submit input and cancel dispatch through `reason.turn` and update derived UI turn projections
- live provider submit now streams reason/debug updates into `UiProtocolState` before final receipt is returned
- direct slave message dispatch routes through `node.master-slave`
- explicit checkpoint rewind dispatch now routes through `runtime.checkpoint-rewind`
- resume dispatch remains an explicit unsupported runtime path
- config-selected runtime bootstrap is now bound in code
- config-selected runtime bootstrap uses explicit peer-topology config instead of synthetic paired node ids
- config-selected live bootstrap restores persisted turn projection and next runtime turn ordinal when recovery truth exists
- migrated mainline-call source now lives at `docs/mainline-calls/runtime.ui-command-dispatch.json` and generated wiki lives at `docs/wiki/runtime.ui-command-dispatch.md`
