# Feature Map

This file is the registry for feature ownership and verification.

Use order:

1. identify target `feature_id`
2. confirm single `owner`
3. obey `allowed_paths` and `forbidden_paths`
4. use `debug_artifacts` as debug entry
5. use `runtime_paths` as scene evidence entry
6. run `required_checks`
7. if truth changed, update this file in same task

## Required Fields

- `feature_id`
- `owner`
- `allowed_paths`
- `forbidden_paths`
- `required_checks`
- `required_white_box_tests`
- `required_module_black_box_tests`
- `required_project_black_box_tests`
- `test_design_doc`
- `function_map_doc`
- `debug_artifacts`
- `runtime_paths`
- `update_triggers`
- `lifecycle_checks`

## Seed Entries

### `foundation.workspace`

- owner: `xtask`, workspace root
- allowed_paths: `Cargo.toml`, `xtask/**`, `docs/architecture/**`
- forbidden_paths: provider and reason implementation crates unless scaffold-related
- required_checks:
  - `cargo test --workspace`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - xtask gate rule tests
- required_module_black_box_tests:
  - xtask gate smoke
- required_project_black_box_tests:
  - workspace harness smoke
- test_design_doc: `docs/testing/foundation.workspace.md`
- function_map_doc: `docs/function-maps/foundation.workspace.md`
- debug_artifacts:
  - none
- runtime_paths:
  - `~/.freehand/logs`
- update_triggers:
  - workspace member changes
  - gate policy changes
  - repo workflow changes
- lifecycle_checks:
  - information sufficient
  - logic closed-loop
  - lifecycle management complete

### `config.core`

- owner: `crates/freehand-config`
- allowed_paths: `crates/freehand-config/**`, `crates/freehand-contracts/**`, `docs/architecture/**`
- forbidden_paths: `apps/**` provider adapter internals
- required_checks:
  - `cargo test -p freehand-config`
- required_white_box_tests:
  - config load/validate tests
  - startup mode config tests
  - slave startup permission config tests
  - multi-agent named-table config tests
  - restart-only config activation tests
- required_module_black_box_tests:
  - config file load smoke
  - named agent selection smoke
- required_project_black_box_tests:
  - CLI agent-start config smoke
- test_design_doc: `docs/testing/config.core.md`
- function_map_doc: `docs/function-maps/config.core.md`
- debug_artifacts:
  - config snapshot path
- runtime_paths:
  - `~/.freehand/state/config`
  - `~/.freehand/logs/config`
- update_triggers:
  - config schema changes
  - config resolution order changes
  - runtime home layout changes
  - startup file contract changes
- lifecycle_checks:
  - multi-agent config ownership remains single-source
  - startup mode lifecycle is fully covered
  - config update path is closed-loop
  - one-process-one-agent startup rule remains explicit

### `provider.semantic`

- owner: `crates/freehand-provider-core`
- allowed_paths: `crates/freehand-provider-core/**`, `crates/freehand-contracts/**`, `crates/freehand-blocks/**`
- forbidden_paths: `apps/**`, `crates/freehand-ui-protocol/**`
- required_checks:
  - `cargo test -p freehand-provider-core`
- required_white_box_tests:
  - semantic request/event mapping tests
  - capability declaration tests
  - periodic recovery classification tests
  - debug raw-event retention policy tests
- required_module_black_box_tests:
  - streaming semantic event smoke
  - single-shot semantic response smoke
- required_project_black_box_tests:
  - provider-to-reason integration smoke
- test_design_doc: `docs/testing/provider.semantic.md`
- function_map_doc: `docs/function-maps/provider.semantic.md`
- debug_artifacts:
  - provider replay fixture path
  - provider raw event fixture path
- runtime_paths:
  - `~/.freehand/ledgers/providers`
  - `~/.freehand/replays/providers`
- update_triggers:
  - request/response semantic changes
  - provider adapter boundary changes
  - debug artifact shape changes
  - capability declaration changes
  - recovery classification changes
- lifecycle_checks:
  - provider semantic path is closed-loop
  - provider failure path is explicit
  - replay/debug lifecycle stays valid
  - debug and non-debug retention policies remain explicit

### `contracts.core`

- owner: `crates/freehand-contracts`
- allowed_paths: `crates/freehand-contracts/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-config/**`, `crates/freehand-ui-protocol/**`, `docs/debug/**` except references
- required_checks:
  - `cargo test -p freehand-contracts`
- required_white_box_tests:
  - shared contract serialization tests
  - shared ID contract tests
  - error contract tests
- required_module_black_box_tests:
  - contract replay fixture decode smoke
- required_project_black_box_tests:
  - cross-crate contract compatibility smoke
- test_design_doc: `docs/testing/contracts.core.md`
- function_map_doc: `docs/function-maps/contracts.core.md`
- debug_artifacts:
  - shared contract replay fixture path
- runtime_paths:
  - `~/.freehand/replays/contracts`
  - `~/.freehand/state/contracts`
- update_triggers:
  - shared chain node changes
  - shared ID changes
  - error contract policy changes
  - serialization boundary changes
- lifecycle_checks:
  - shared semantic ownership remains centralized
  - request/response/error contract paths are closed-loop
  - persistence and replay guarantees remain explicit

### `reason.turn`

- owner: `crates/freehand-reason`
- allowed_paths: `crates/freehand-reason/**`, `crates/freehand-contracts/**`, `crates/freehand-blocks/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-node/**` except runtime wiring boundaries, `crates/freehand-provider-*/**` except adapter interfaces
- required_checks:
  - `cargo test -p freehand-reason`
- required_white_box_tests:
  - per-turn truth projection tests
  - tool re-entry ownership tests
  - terminal schema validation tests
  - invalid completion schema rejection tests
  - slow subscriber non-blocking tests
- required_module_black_box_tests:
  - turn semantic stream smoke
  - completion rejection/retry smoke
- required_project_black_box_tests:
  - reason-to-ui terminal projection smoke
- test_design_doc: `docs/testing/reason.turn.md`
- function_map_doc: `docs/function-maps/reason.turn.md`
- debug_artifacts:
  - turn replay fixture path
  - completion schema rejection fixture path
- runtime_paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/replays/reason`
- update_triggers:
  - turn truth granularity changes
  - terminal schema changes
  - subscriber delivery policy changes
  - raw-event retention policy changes
- lifecycle_checks:
  - turn truth write path remains single-owner
  - terminal decision path is closed-loop
  - schema rejection and retry path are explicit
  - debug ledger and session truth boundaries remain explicit

### `ui.protocol`

- owner: `crates/freehand-ui-protocol`
- allowed_paths: `crates/freehand-ui-protocol/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-provider-*/**`, `crates/freehand-config/**` except imported config selections, UI rendering app internals except wiring
- required_checks:
  - `cargo test -p freehand-ui-protocol`
- required_white_box_tests:
  - command -> projection smoke
  - slave turn subscription smoke
  - node status query smoke
  - terminal result projection smoke
- required_module_black_box_tests:
  - latest-active-turn subscribe smoke
  - specific-turn snapshot/query smoke
  - stream-kind routing smoke
- required_project_black_box_tests:
  - CLI command/query projection smoke
  - WebUI slave-card subscription smoke
- test_design_doc: `docs/testing/ui.protocol.md`
- function_map_doc: `docs/function-maps/ui.protocol.md`
- debug_artifacts:
  - ui protocol stream fixture path
  - node status snapshot fixture path
- runtime_paths:
  - `~/.freehand/replays/ui`
  - `~/.freehand/state/ui`
- update_triggers:
  - command surface changes
  - projection surface changes
  - subscription model changes
  - source identity field changes
  - slave turn presentation changes
- lifecycle_checks:
  - query and subscribe boundaries remain explicit
  - source identity remains traceable
  - CLI/WebUI divergence remains protocol-safe
  - terminal text projection remains closed-loop

### `node.master-slave`

- owner: `crates/freehand-node`
- allowed_paths: `crates/freehand-node/**`, `crates/freehand-contracts/**`, `crates/freehand-ui-protocol/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-provider-*/**`, `apps/**` except wiring-only entrypoint glue
- required_checks:
  - `cargo test -p freehand-node`
- required_white_box_tests:
  - master/slave pairing tests
  - paired slave input-restriction tests
  - slave startup config permission tests
  - local websocket handshake tests
  - pairing-loss relisten tests
  - slave turn subscription tests
  - status query and health-check tests
- required_module_black_box_tests:
  - node status snapshot smoke
  - slave progress query smoke
- required_project_black_box_tests:
  - master-delegate/slave-progress smoke
  - master-subscribe-slave-turn smoke
- test_design_doc: `docs/testing/node.master-slave.md`
- function_map_doc: `docs/function-maps/node.master-slave.md`
- debug_artifacts:
  - pairing ledger path
  - slave mode transition replay path
  - websocket handshake replay path
  - node status snapshot path
- runtime_paths:
  - `~/.freehand/state/nodes`
  - `~/.freehand/state/config`
  - `~/.freehand/ledgers/nodes`
  - `~/.freehand/replays/nodes`
- update_triggers:
  - pairing semantics changes
  - input-permission semantics changes
  - slave input restrictions change
  - node mode lifecycle changes
  - slave startup config changes
  - websocket pairing changes
  - turn subscription changes
- lifecycle_checks:
  - input-permission lifecycle is complete
  - pairing path and rejection path are both explicit
  - runtime evidence remains traceable
  - local one-master one-slave scope remains explicit
  - pairing-loss and re-listen path remain explicit
