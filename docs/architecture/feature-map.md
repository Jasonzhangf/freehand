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
- `mainline_call_doc` for migrated features
- `generated_wiki_doc` for migrated features
- `debug_artifacts`
- `runtime_paths`
- `update_triggers`
- `lifecycle_checks`

## Owner Routing Index

Use this table before grep or implementation. Every bug or feature request must first map to one `feature_id`, then follow that feature's `function_map_doc` and `test_design_doc`.

| problem area | feature_id | owner module / crate | function map | test orchestration |
| --- | --- | --- | --- | --- |
| workspace gates, CI/CD, repo rules | `foundation.workspace` | `xtask`, workspace root | `docs/function-maps/foundation.workspace.md` | `docs/testing/foundation.workspace.md` |
| config load, agents, providers, startup selection | `config.core` | `crates/freehand-config` | `docs/function-maps/config.core.md` | `docs/testing/config.core.md` |
| shared IDs, cross-module contracts, request/response/error contracts | `contracts.core` | `crates/freehand-contracts` | `docs/function-maps/contracts.core.md` | `docs/testing/contracts.core.md` |
| provider-neutral semantics and recovery policy | `provider.semantic` | `crates/freehand-provider-core` | `docs/function-maps/provider.semantic.md` | `docs/testing/provider.semantic.md` |
| OpenAI-compatible wire rendering/parsing | `provider.openai-adapter` | `crates/freehand-provider-openai` | `docs/function-maps/provider.openai-adapter.md` | `docs/testing/provider.openai-adapter.md` |
| Anthropic Messages wire rendering/parsing/executor | `provider.anthropic-adapter` | `crates/freehand-provider-anthropic` | `docs/function-maps/provider.anthropic-adapter.md` | `docs/testing/provider.anthropic-adapter.md` |
| provider-selected live bridge into runtime-owned live reason turn | `provider.reason-live-bridge` | `crates/freehand-runtime` | `docs/function-maps/provider.reason-live-bridge.md` | `docs/testing/provider.reason-live-bridge.md` |
| built-in tool registry, Reasonix-aligned tool schemas, tool execution ownership | `tool.registry` | `crates/freehand-tools` | `docs/function-maps/tool.registry.md` | `docs/testing/tool.registry.md` |
| turn truth, provider-output application, terminal schema | `reason.turn` | `crates/freehand-reason` | `docs/function-maps/reason.turn.md` | `docs/testing/reason.turn.md` |
| session-history rewrite state and rewrite gates | `reason.session-history` | `crates/freehand-reason` | `docs/function-maps/reason.session-history.md` | `docs/testing/reason.session-history.md` |
| reason persistence, ledgers, restore, derived sidecars | `reason.persistence` | `crates/freehand-reason` | `docs/function-maps/reason.persistence.md` | `docs/testing/reason.persistence.md` |
| context planning, cache shape, segment admission | `reason.context-planner` | `crates/freehand-blocks` | `docs/function-maps/reason.context-planner.md` | `docs/testing/reason.context-planner.md` |
| compaction/rewrite/recovery trigger policy | `reason.rewrite-policy` | `crates/freehand-blocks` | `docs/function-maps/reason.rewrite-policy.md` | `docs/testing/reason.rewrite-policy.md` |
| independent debug/trace contracts, snapshots, hub/sinks | `debug.core` | `crates/freehand-debug` | `docs/function-maps/debug.core.md` | `docs/testing/debug.core.md` |
| master/slave pairing, node status, delegation, slave turn publication | `node.master-slave` | `crates/freehand-node` | `docs/function-maps/node.master-slave.md` | `docs/testing/node.master-slave.md` |
| UI commands, query/subscribe, UI projections | `ui.protocol` | `crates/freehand-ui-protocol` | `docs/function-maps/ui.protocol.md` | `docs/testing/ui.protocol.md` |
| runtime wiring for UI command dispatch into owner modules | `runtime.ui-command-dispatch` | `crates/freehand-runtime` | `docs/function-maps/runtime.ui-command-dispatch.md` | `docs/testing/runtime.ui-command-dispatch.md` |
| CLI reason smoke and config-selected runtime harness | `app.cli-runtime-smoke` | `apps/freehand-cli` | `docs/function-maps/app.cli-runtime-smoke.md` | `docs/testing/app.cli-runtime-smoke.md` |
| CLI live provider turn and completion loop smoke | `app.cli-live-turn` | `apps/freehand-cli` | `docs/function-maps/app.cli-live-turn.md` | `docs/testing/app.cli-live-turn.md` |
| WebUI/protocol-only app boundary smoke | `app.webui-smoke` | `apps/freehand-server` | `docs/function-maps/app.webui-smoke.md` | `docs/testing/app.webui-smoke.md` |
| runtime-backed HTTP/SSE UI daemon host | `app.runtime-daemon` | `apps/freehand-daemon` | `docs/function-maps/app.runtime-daemon.md` | `docs/testing/app.runtime-daemon.md` |

If a problem does not fit this table, update this routing index before making code changes. Do not create a second owner by patching an adjacent module.

## Seed Entries

### `foundation.workspace`

- owner: `xtask`, workspace root
- allowed_paths: `Cargo.toml`, `xtask/**`, `docs/architecture/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`, `docs/goals/**`, `CACHE.md`, `MEMORY.md`, `note.md`
- forbidden_paths: provider and reason implementation crates unless scaffold-related
- required_checks:
  - `cargo test --workspace`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - xtask gate rule tests
  - xtask mainline render/generation tests
- required_module_black_box_tests:
  - xtask gate smoke
  - xtask mainlines check smoke
- required_project_black_box_tests:
  - workspace harness smoke
- test_design_doc: `docs/testing/foundation.workspace.md`
- function_map_doc: `docs/function-maps/foundation.workspace.md`
- mainline_call_doc: `docs/mainline-calls/foundation.workspace.json`
- generated_wiki_doc: `docs/wiki/foundation.workspace.md`
- debug_artifacts:
  - none
- runtime_paths:
  - `~/.freehand/logs`
- update_triggers:
  - workspace member changes
  - gate policy changes
  - repo workflow changes
  - mainline generation shape changes
  - generated wiki freshness policy changes
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
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - config load/validate tests
  - startup mode config tests
  - slave startup permission config tests
  - multi-agent named-table config tests
  - reciprocal peer-topology config tests
  - multi-provider named-table config tests
  - provider auth source resolution tests
  - provider protocol declaration tests
  - provider unknown-field rejection tests
  - restart-only config activation tests
- required_module_black_box_tests:
  - config file load smoke
  - named agent selection smoke
  - named provider selection smoke
- required_project_black_box_tests:
  - CLI agent-start config + provider projection smoke
- test_design_doc: `docs/testing/config.core.md`
- function_map_doc: `docs/function-maps/config.core.md`
- mainline_call_doc: `docs/mainline-calls/config.core.json`
- generated_wiki_doc: `docs/wiki/config.core.md`
- debug_artifacts:
  - config snapshot path
- runtime_paths:
  - `~/.freehand/state/config`
  - `~/.freehand/logs/config`
- update_triggers:
  - config schema changes
  - provider registry schema changes
  - provider selection rules change
  - config resolution order changes
  - runtime home layout changes
  - startup file contract changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - multi-agent config ownership remains single-source
  - multi-provider config ownership remains single-source
  - startup mode lifecycle is fully covered
  - provider selection lifecycle is fully covered
  - config update path is closed-loop
  - one-process-one-agent startup rule remains explicit
  - paired node topology remains config-owned and reciprocal
  - migrated mainline call source and generated wiki stay in sync with the function map

### `app.cli-runtime-smoke`

- owner: `apps/freehand-cli`
- allowed_paths: `apps/freehand-cli/**`, `crates/freehand-testkit/**`, `crates/freehand-reason/**`, `crates/freehand-config/**`, `docs/architecture/**`, `docs/function-maps/**`, `docs/testing/**`
- forbidden_paths: `crates/freehand-provider-*/**` except consumed semantic outputs only
- required_checks:
  - `cargo test -p freehand-cli`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - none beyond dispatch helper coverage
- required_module_black_box_tests:
  - CLI startup config smoke
  - CLI reason compaction smoke
  - CLI recovery block smoke
- required_project_black_box_tests:
  - app boundary config -> harness-backed reason E2E smoke
- test_design_doc: `docs/testing/app.cli-runtime-smoke.md`
- function_map_doc: `docs/function-maps/app.cli-runtime-smoke.md`
- mainline_call_doc: `docs/mainline-calls/app.cli-runtime-smoke.json`
- generated_wiki_doc: `docs/wiki/app.cli-runtime-smoke.md`
- debug_artifacts:
  - CLI smoke stdout fixtures
- runtime_paths:
  - `~/.freehand/state/config`
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
- update_triggers:
  - CLI command shape changes
  - smoke scenario changes
  - harness boundary changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - CLI remains a true app boundary, not direct crate test glue
  - config selection still has one-process-one-agent truth
  - reason smoke path still routes through shared harness and rewrite policy owner paths
  - migrated mainline call source and generated wiki stay in sync with the function map

### `app.cli-live-turn`

- owner: `apps/freehand-cli`
- allowed_paths: `apps/freehand-cli/**`, `crates/freehand-runtime/**`, `crates/freehand-config/**`, `crates/freehand-provider-anthropic/**`, `crates/freehand-provider-core/**`, `crates/freehand-reason/**`, `docs/function-maps/**`, `docs/testing/**`
- forbidden_paths: `crates/freehand-reason/**` semantic-owner changes unrelated to provider-neutral consumption
- required_checks:
  - `cargo test -p freehand-cli`
- required_white_box_tests:
  - none beyond argument dispatch helpers
- required_module_black_box_tests:
  - CLI live-turn single-shot mock smoke
  - CLI live-turn stream mock smoke
  - CLI live-turn invalid-schema retry smoke
  - CLI live-turn unsupported-provider smoke
- required_project_black_box_tests:
  - app boundary config-selected anthropic provider drives one real turn through live bridge
- test_design_doc: `docs/testing/app.cli-live-turn.md`
- function_map_doc: `docs/function-maps/app.cli-live-turn.md`
- debug_artifacts:
  - CLI live-turn stdout fixtures
- runtime_paths:
  - `~/.freehand/state/config`
  - `~/.freehand/ledgers/providers/anthropic`
  - `~/.freehand/ledgers/reason`
- update_triggers:
  - CLI live-turn command shape changes
  - live bridge summary projection changes
  - config-selected anthropic path changes
- lifecycle_checks:
  - CLI remains app boundary only
  - live turn still routes through runtime-owned live bridge instead of duplicating provider/runtime semantics
  - config-selected anthropic path remains closed-loop
  - completion loop projections stay on the app boundary and do not leak tagged schema text

### `app.webui-smoke`

- owner: `apps/freehand-server`
- allowed_paths: `apps/freehand-server/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-runtime/**`, `crates/freehand-reason/**`, `crates/freehand-node/**`, `crates/freehand-config/**`, `crates/freehand-provider-*/**`
- required_checks:
  - `cargo test -p freehand-server`
- required_white_box_tests:
  - render helper coverage
  - protocol-only router coverage
  - dependency boundary scan
- required_module_black_box_tests:
  - WebUI command ingress dispatch receipt smoke
  - WebUI command ingress query-route-misuse rejection smoke
  - WebUI query projection smoke
  - WebUI debug query projection smoke
  - WebUI latest-turn SSE subscribe smoke
  - WebUI debug SSE subscribe smoke
  - WebUI slave-card render smoke
  - CLI/WebUI divergence smoke via protocol projection
- required_project_black_box_tests:
  - protocol-only app boundary smoke for HTTP query/SSE/command ingress
- test_design_doc: `docs/testing/app.webui-smoke.md`
- function_map_doc: `docs/function-maps/app.webui-smoke.md`
- debug_artifacts:
  - WebUI smoke stdout fixture
- runtime_paths:
  - `~/.freehand/state/ui`
  - `~/.freehand/replays/ui`
- update_triggers:
  - HTTP/SSE route shape changes
  - static smoke dispatch behavior changes
  - protocol-only app boundary changes
- lifecycle_checks:
  - app remains protocol-only
  - shared transport helpers remain protocol-only
  - runtime owner injection stays outside this app crate

### `app.runtime-daemon`

- owner: `apps/freehand-daemon`
- allowed_paths: `apps/freehand-daemon/**`, `crates/freehand-runtime/**`, `apps/freehand-server/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-reason/**`, `crates/freehand-node/**`, `crates/freehand-config/**`, `crates/freehand-provider-*/**` except through `crates/freehand-runtime`
- required_checks:
  - `cargo test -p freehand-daemon`
- required_white_box_tests:
  - daemon bootstrap config coverage
  - config-selected bootstrap coverage
  - runtime-dispatcher wiring coverage
  - dependency boundary scan
- required_module_black_box_tests:
  - daemon submit-user-input HTTP smoke
  - daemon latest-turn query smoke
  - daemon restart latest-turn query/SSE restore smoke
  - daemon restart next-turn-id continuation smoke
  - daemon provider failure HTTP smoke
  - daemon direct-message dispatch smoke
  - daemon slave-mode startup rejection smoke
- required_project_black_box_tests:
  - real runtime owner injection over shared HTTP/SSE/command transport without app-owned business logic
- test_design_doc: `docs/testing/app.runtime-daemon.md`
- function_map_doc: `docs/function-maps/app.runtime-daemon.md`
- debug_artifacts:
  - daemon stdout fixture
- runtime_paths:
  - `~/.freehand/state/ui`
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
- update_triggers:
  - runtime transport injection changes
  - daemon bootstrap contract changes
  - shared app transport injection shape changes
- lifecycle_checks:
  - daemon depends on `freehand-runtime`, not directly on reason/node/provider/config owners
  - app transport remains shared and protocol-only
  - runtime dispatch and UI projection stay closed-loop through one shared state handle
  - config-selected bootstrap remains one-process-one-agent and rejects slave-mode UI host startup explicitly

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
- mainline_call_doc: `docs/mainline-calls/provider.semantic.json`
- generated_wiki_doc: `docs/wiki/provider.semantic.md`
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
  - provider semantic layer remains independent from `freehand-reason`
  - metadata and request-chain data remain type-isolated
  - migrated mainline call source and generated wiki stay in sync with the function map

### `provider.openai-adapter`

- owner: `crates/freehand-provider-openai`
- allowed_paths: `crates/freehand-provider-openai/**`, `crates/freehand-provider-core/**`, `crates/freehand-blocks/**`, `docs/design/**`, `docs/references/provider-protocols/**`
- forbidden_paths: `crates/freehand-ui-protocol/**`, `apps/**` except wiring-only integration tests
- required_checks:
  - `cargo test -p freehand-provider-openai`
- required_white_box_tests:
  - responses request renderer tests
  - chat-completions request renderer tests
  - responses single-shot parser tests
  - chat-completions stream parser tests
  - partial tool-call accumulation tests
- required_module_black_box_tests:
  - openai adapter emits unified semantic outputs for responses
  - openai adapter emits unified semantic outputs for chat completions
- required_project_black_box_tests:
  - openai adapter to reason integration smoke
- test_design_doc: `docs/testing/provider.openai-adapter.md`
- function_map_doc: `docs/function-maps/provider.openai-adapter.md`
- mainline_call_doc: `docs/mainline-calls/provider.openai-adapter.json`
- generated_wiki_doc: `docs/wiki/provider.openai-adapter.md`
- debug_artifacts:
  - openai raw payload fixtures
  - openai stream replay fixtures
- runtime_paths:
  - `~/.freehand/ledgers/providers/openai`
  - `~/.freehand/replays/providers/openai`
- update_triggers:
  - openai protocol support changes
  - responses/chat-completions render rules change
  - stream chunk accumulation changes
  - tool argument mapping changes
- lifecycle_checks:
  - responses and chat-completions boundaries remain explicit
  - partial tool-call lifecycle is explicit
  - adapter-private DTO boundary remains intact
  - adapter does not depend on `freehand-reason`
  - metadata does not become prompt/request content implicitly
  - migrated mainline call source and generated wiki stay in sync with the function map

### `provider.anthropic-adapter`

- owner: `crates/freehand-provider-anthropic`
- allowed_paths: `crates/freehand-provider-anthropic/**`, `crates/freehand-provider-core/**`, `crates/freehand-blocks/**`, `docs/design/**`, `docs/references/provider-protocols/**`
- forbidden_paths: `crates/freehand-ui-protocol/**`, `apps/**` except wiring-only integration tests
- required_checks:
  - `cargo test -p freehand-provider-anthropic`
- required_white_box_tests:
  - messages request renderer tests
  - messages single-shot parser tests
  - SSE stream parser tests
  - messages HTTP executor tests
  - incremental SSE callback delivery tests
  - tool-use and fine-grained tool-stream accumulation tests
  - stop-reason mapping tests
- required_module_black_box_tests:
  - anthropic adapter emits unified semantic outputs for messages
  - anthropic executor emits unified semantic outputs for local single-shot and SSE mock servers
- required_project_black_box_tests:
  - anthropic adapter to reason integration smoke
- test_design_doc: `docs/testing/provider.anthropic-adapter.md`
- function_map_doc: `docs/function-maps/provider.anthropic-adapter.md`
- mainline_call_doc: `docs/mainline-calls/provider.anthropic-adapter.json`
- generated_wiki_doc: `docs/wiki/provider.anthropic-adapter.md`
- debug_artifacts:
  - anthropic raw payload fixtures
  - anthropic stream replay fixtures
- runtime_paths:
  - `~/.freehand/ledgers/providers/anthropic`
  - `~/.freehand/replays/providers/anthropic`
- update_triggers:
  - anthropic messages render rules change
  - tool-use stream accumulation changes
  - stop-reason mapping changes
- lifecycle_checks:
  - messages stateless request boundary remains explicit
  - partial tool-call lifecycle is explicit
  - adapter-private DTO boundary remains intact
  - adapter does not depend on `freehand-reason`
  - metadata does not become prompt/request content implicitly
  - migrated mainline call source and generated wiki stay in sync with the function map

### `provider.reason-live-bridge`

- owner: `crates/freehand-runtime`
- allowed_paths: `crates/freehand-runtime/**`, `crates/freehand-config/**`, `crates/freehand-provider-core/**`, `crates/freehand-provider-anthropic/**`, `crates/freehand-reason/**`, `crates/freehand-blocks/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-reason/**` semantic-owner changes unrelated to provider-neutral consumption, `crates/freehand-provider-openai/**`, `apps/freehand-daemon/**`
- required_checks:
  - `cargo test -p freehand-runtime`
- required_white_box_tests:
  - live bridge request build tests
  - live bridge anthropic single-shot mock tests
  - live bridge anthropic SSE mock tests
  - live bridge broadcast capture tests
  - live bridge incremental stream broadcast tests
  - live bridge invalid-schema retry tests
  - live bridge continue-next-round tests
  - live bridge retry-exhausted failed-terminal tests
  - unsupported provider selection tests
  - provider HTTP failure tests
  - persistence restore/write tests on runtime-owned live bridge
  - tool-result re-entry into second provider request tests
- required_module_black_box_tests:
  - config-selected anthropic provider can drive one runtime-owned live turn with persistence and UI projection updates
  - config-selected restart can restore prior closed turns and continue ordinal allocation without turn-id reuse
- required_project_black_box_tests:
  - CLI live-turn smoke against local anthropic-compatible mock server
  - daemon submit-user-input HTTP smoke against local anthropic-compatible mock server
- test_design_doc: `docs/testing/provider.reason-live-bridge.md`
- function_map_doc: `docs/function-maps/provider.reason-live-bridge.md`
- mainline_call_doc: `docs/mainline-calls/provider.reason-live-bridge.json`
- generated_wiki_doc: `docs/wiki/provider.reason-live-bridge.md`
- debug_artifacts:
  - live bridge replay fixture path
  - local mock transcript fixtures
- runtime_paths:
  - `~/.freehand/ledgers/providers/anthropic`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/replays/providers/anthropic`
- update_triggers:
  - config-to-provider bridge rules change
  - anthropic executor boundary changes
  - reason turn live ingestion path changes
  - CLI live-turn command shape changes
- lifecycle_checks:
  - reason remains provider-implementation independent
  - live bridge owns runtime composition without duplicating adapter semantics
  - anthropic live path is closed-loop from config selection to turn truth, persistence, and UI projection
  - completion schema loop remains bridge composition, not provider or app semantics
  - migrated mainline call source and generated wiki stay in sync with the function map

### `tool.registry`

- owner: `crates/freehand-tools`
- allowed_paths: `crates/freehand-tools/**`, `crates/freehand-provider-core/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `apps/**`, `crates/freehand-provider-openai/**`, `crates/freehand-provider-anthropic/**`, `crates/freehand-reason/**`
- required_checks:
  - `cargo test -p freehand-tools`
- required_white_box_tests:
  - registry schema export tests
  - read-only / implemented metadata tests
  - implemented tool execution tests
  - unknown/unimplemented tool rejection tests
- required_module_black_box_tests:
  - runtime live bridge tool-schema export smoke
  - runtime live bridge implemented tool execution smoke
- required_project_black_box_tests:
  - CLI live provider tool-loop smoke
  - daemon live provider tool-loop smoke
- test_design_doc: `docs/testing/tool.registry.md`
- function_map_doc: `docs/function-maps/tool.registry.md`
- mainline_call_doc: `docs/mainline-calls/tool.registry.json`
- generated_wiki_doc: `docs/wiki/tool.registry.md`
- debug_artifacts:
  - tool registry spec fixture path
- runtime_paths:
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/replays/providers`
- update_triggers:
  - tool registry surface changes
  - tool schema changes
  - implemented tool execution behavior changes
  - runtime live bridge tool ownership changes
- lifecycle_checks:
  - tool schema ownership remains outside runtime orchestration
  - registered but unimplemented tools fail explicitly
  - first-version path tools remain locked to one workspace-root policy
  - implemented tool execution path is closed-loop into provider tool-result re-entry

### `ui.protocol`

- owner: `crates/freehand-ui-protocol`
- allowed_paths: `crates/freehand-ui-protocol/**`, `crates/freehand-debug/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-reason/**`, `crates/freehand-provider-*/**`, `apps/**` except transport-only adapters
- required_checks:
  - `cargo test -p freehand-ui-protocol`
- required_white_box_tests:
  - command/projection mapping tests
  - ingress acceptance/rejection tests
  - command dispatch routing tests
  - subscription selector and match tests
  - public turn projection tests
  - client-specific projection gating tests
  - debug-state projection and receiver-drain tests
- required_module_black_box_tests:
  - command ingress accept/reject smoke
  - command dispatch envelope owner-routing smoke
  - latest-turn subscribe and specific-turn query smoke
  - debug-state snapshot/query/subscribe smoke
  - CLI/WebUI divergence smoke via protocol projection
  - public conversation projection smoke
- required_project_black_box_tests:
  - protocol truth can back HTTP query and SSE subscribe adapters without app-owned projection duplication
- test_design_doc: `docs/testing/ui.protocol.md`
- function_map_doc: `docs/function-maps/ui.protocol.md`
- mainline_call_doc: `docs/mainline-calls/ui.protocol.json`
- generated_wiki_doc: `docs/wiki/ui.protocol.md`
- debug_artifacts:
  - UI protocol stream fixtures
  - node status snapshots
- runtime_paths:
  - `~/.freehand/state/ui`
  - `~/.freehand/replays/ui`
- update_triggers:
  - UI command shape changes
  - query/subscribe routing changes
  - public projection rules change
  - debug snapshot bridge changes
  - client-specific projection gating changes
- lifecycle_checks:
  - UI remains ingress plus read-only projection boundary
  - command ingress stays separate from query/subscribe
  - projection ownership stays in `freehand-ui-protocol`
  - client-specific projection gating stays protocol-owned
  - UI does not become reason/debug/session truth writer

### `contracts.core`

- owner: `crates/freehand-contracts`
- allowed_paths: `crates/freehand-contracts/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-config/**`, `crates/freehand-ui-protocol/**`, `docs/debug/**` except references
- required_checks:
  - `cargo test -p freehand-contracts`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
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
- mainline_call_doc: `docs/mainline-calls/contracts.core.json`
- generated_wiki_doc: `docs/wiki/contracts.core.md`
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
  - generated wiki freshness policy changes
- lifecycle_checks:
  - shared semantic ownership remains centralized
  - request/response/error contract paths are closed-loop
  - persistence and replay guarantees remain explicit
  - migrated mainline call source and generated wiki stay in sync with the function map

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
  - tagged completion parser integration tests
  - invalid completion schema rejection tests
  - failed terminal write tests
  - slow subscriber non-blocking tests
- required_module_black_box_tests:
  - turn semantic stream smoke
  - completion rejection/retry smoke
- required_project_black_box_tests:
  - reason-to-ui terminal projection smoke
- test_design_doc: `docs/testing/reason.turn.md`
- function_map_doc: `docs/function-maps/reason.turn.md`
- mainline_call_doc: `docs/mainline-calls/reason.turn.json`
- generated_wiki_doc: `docs/wiki/reason.turn.md`
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
  - session-history ownership changes
- lifecycle_checks:
  - turn truth write path remains single-owner
  - terminal decision path is closed-loop
  - schema rejection and retry path are explicit
  - debug ledger and session truth boundaries remain explicit
  - context orchestration truth remains inside `freehand-reason`
  - turn startup rewrite state remains sourced from `reason.session-history`
  - provider adapter crates remain independent from `freehand-reason`
  - metadata and request-chain data remain type-isolated
  - migrated mainline call source and generated wiki stay in sync with the function map

### `reason.session-history`

- owner: `crates/freehand-reason`
- allowed_paths: `crates/freehand-reason/**`, `crates/freehand-contracts/**`, `crates/freehand-blocks/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-provider-*/**`, `crates/freehand-ui-protocol/**` except projection-only consumers
- required_checks:
  - `cargo test -p freehand-reason`
- required_white_box_tests:
  - rewrite version persistence tests
  - explicit compaction rewrite tests
  - explicit rollback rewrite tests
  - explicit resume-rebuild rewrite tests
  - persisted json/file round-trip tests
  - ordinary-turn no-rewrite-version-bump tests
- required_module_black_box_tests:
  - session-history to start-turn rewrite propagation smoke
  - rewrite-gate consumption smoke
- required_project_black_box_tests:
  - reason-to-provider rewrite-version propagation smoke
- test_design_doc: `docs/testing/reason.session-history.md`
- function_map_doc: `docs/function-maps/reason.session-history.md`
- mainline_call_doc: `docs/mainline-calls/reason.session-history.json`
- generated_wiki_doc: `docs/wiki/reason.session-history.md`
- debug_artifacts:
  - session-history persisted fixture path
  - rewrite-ledger fixture path
- runtime_paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/replays/reason`
- update_triggers:
  - rewrite gate semantics change
  - persisted session truth format changes
  - turn-start rewrite sourcing changes
  - compaction/rollback/resume lifecycle changes
- lifecycle_checks:
  - rewrite version is single-owned by `freehand-reason`
  - non-ordinary rewrite modes enter planner only through explicit session-history gate methods
  - ordinary turns do not bump rewrite version
  - rewrite ledger retains diagnostics and applied-turn evidence
- persisted session truth remains serializable and reloadable

### `reason.persistence`

- owner: `crates/freehand-reason`
- allowed_paths: `crates/freehand-reason/**`, `crates/freehand-testkit/**`, `apps/freehand-cli/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/runtime/**`, `docs/debug/**`
- forbidden_paths: `crates/freehand-provider-*/**` except debug raw-ledger consumers at adapter boundaries, `crates/freehand-ui-protocol/**` except projection-only consumers
- required_checks:
  - `cargo test -p freehand-reason`
  - `cargo test -p freehand-testkit`
  - `cargo test -p freehand-cli`
- required_white_box_tests:
  - session snapshot render/load tests
  - persistence cursor serialization tests
  - reason-ledger sequence ordering tests
  - snapshot-plus-tail recovery tests
  - ledger-only rebuild tests
  - provider-raw-ledger exclusion-from-session-truth tests
  - atomic snapshot replace tests
- required_module_black_box_tests:
  - persistence save/reload smoke
  - terminal turn materialization smoke
  - recovery from snapshot-plus-ledger-tail smoke
  - derived-sidecar rebuild smoke
- required_project_black_box_tests:
  - CLI persistence restore smoke
- test_design_doc: `docs/testing/reason.persistence.md`
- function_map_doc: `docs/function-maps/reason.persistence.md`
- mainline_call_doc: `docs/mainline-calls/reason.persistence.json`
- generated_wiki_doc: `docs/wiki/reason.persistence.md`
- debug_artifacts:
  - persisted session snapshot fixture path
  - reason ledger fixture path
  - corrupted persistence fixture path
  - provider raw debug fixture path
- runtime_paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/state/ui`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/ledgers/providers`
  - `~/.freehand/cache/session-index`
  - `~/.freehand/replays/reason`
- update_triggers:
  - snapshot file shape changes
  - reason ledger schema changes
  - recovery ordering changes
  - derived sidecar boundaries change
  - restart/resume flow changes
  - runtime home subdirectory changes
- lifecycle_checks:
  - only `freehand-reason` writes authoritative session/turn persistence
  - snapshot and reason-ledger ordering remains explicit and recoverable
  - provider raw debug data never becomes session truth
  - UI and index sidecars remain derived and rebuildable
  - recovery never depends on UI projections or provider raw payloads
  - metadata and request-chain data remain type-isolated across persisted artifacts

### `reason.rewrite-policy`

- owner: `crates/freehand-blocks`
- allowed_paths: `crates/freehand-blocks/**`, `crates/freehand-contracts/**`, `crates/freehand-reason/**`, `crates/freehand-testkit/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-provider-*/**`, `crates/freehand-ui-protocol/**`
- required_checks:
  - `cargo test -p freehand-blocks`
- required_white_box_tests:
  - compaction threshold tests
  - stale-prune-preferred tests
  - paused-auto-compaction tests
  - rollback-vs-rebuild recovery tests
  - insufficient-recovery-truth block tests
- required_module_black_box_tests:
  - rewrite policy decision smoke
  - restore recovery decision smoke
- required_project_black_box_tests:
  - provider usage event reaches rewrite policy through runtime harness
  - reason runtime reaches session-history rewrite gates only through policy-approved paths
  - missing recovery source blocks without mutating session truth
- test_design_doc: `docs/testing/reason.rewrite-policy.md`
- function_map_doc: `docs/function-maps/reason.rewrite-policy.md`
- mainline_call_doc: `docs/mainline-calls/reason.rewrite-policy.json`
- generated_wiki_doc: `docs/wiki/reason.rewrite-policy.md`
- debug_artifacts:
  - rewrite-policy replay fixture path
- runtime_paths:
  - `~/.freehand/ledgers/context`
  - `~/.freehand/replays/context`
  - `~/.freehand/state/turns`
- update_triggers:
  - compaction threshold changes
  - rewrite recovery classification changes
  - rollback or rebuild trigger policy changes
  - auto-compaction pause policy changes
- lifecycle_checks:
  - rewrite trigger policy remains separate from session-history mutation
  - missing runtime truth does not silently compact or recover
  - rollback, resume rebuild, and explicit block all remain distinct outcomes
  - runtime still may not invent rewrite modes outside the policy owner

### `reason.context-planner`

- owner: `crates/freehand-blocks`
- allowed_paths: `crates/freehand-blocks/**`, `crates/freehand-contracts/**`, `crates/freehand-reason/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-provider-*/**` except explicit request-consumer interfaces, `crates/freehand-ui-protocol/**`
- required_checks:
  - `cargo test -p freehand-blocks`
- required_white_box_tests:
  - context segment classification tests
  - context segment ordering tests
  - token-cap admission tests
  - subagent conclusion admission tests
  - raw subagent transcript rejection tests
  - cache-shape drift tests
  - rewrite-version bump tests
- required_module_black_box_tests:
  - planned request-content build smoke
  - metadata/request isolation smoke
  - subagent final-report enrichment smoke
- required_project_black_box_tests:
  - reason-to-provider stable-prefix smoke
  - compaction/rollback-only rewrite smoke
- test_design_doc: `docs/testing/reason.context-planner.md`
- function_map_doc: `docs/function-maps/reason.context-planner.md`
- mainline_call_doc: `docs/mainline-calls/reason.context-planner.json`
- generated_wiki_doc: `docs/wiki/reason.context-planner.md`
- debug_artifacts:
  - context planner replay fixture path
  - cache-shape drift fixture path
  - subagent final-report fixture path
- runtime_paths:
  - `~/.freehand/ledgers/context`
  - `~/.freehand/replays/context`
  - `~/.freehand/state/turns`
- update_triggers:
  - context segment class changes
  - context ordering changes
  - cache-shape policy changes
  - subagent context-admission changes
  - metadata/request boundary changes
- lifecycle_checks:
  - stable-prefix lock remains explicit
  - append-only tail lock remains explicit
  - rewrite-gate lock remains explicit
  - subagent conclusion-only admission remains explicit
  - provider renderers still do not own context planning
  - metadata and request-chain data remain type-isolated

### `debug.core`

- owner: `crates/freehand-debug`
- allowed_paths: `crates/freehand-debug/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-reason/**`, `crates/freehand-provider-*/**`, `crates/freehand-node/**`, `apps/**`
- required_checks:
  - `cargo test -p freehand-debug`
- required_white_box_tests:
  - debug snapshot builder tests
  - trace envelope serialization tests
  - semantic/scene coordinate preservation tests
- required_module_black_box_tests:
  - debug snapshot caller-visible smoke
  - trace envelope JSON round-trip smoke
- required_project_black_box_tests:
  - UI debug-state projection consumes `freehand-debug` snapshot truth
- test_design_doc: `docs/testing/debug.core.md`
- function_map_doc: `docs/function-maps/debug.core.md`
- mainline_call_doc: `docs/mainline-calls/debug.core.json`
- generated_wiki_doc: `docs/wiki/debug.core.md`
- debug_artifacts:
  - debug trace envelope fixture path
  - debug snapshot fixture path
- runtime_paths:
  - `~/.freehand/ledgers`
  - `~/.freehand/replays`
  - `~/.freehand/logs`
- update_triggers:
  - trace envelope fields change
  - debug snapshot fields change
  - debug module dependency direction changes
  - debug ledger/replay ownership changes
- lifecycle_checks:
  - debug remains observation-only
  - debug does not become request/session/reason truth
  - semantic and scene positions remain paired
  - UI consumes debug projections without owning debug truth

### `runtime.ui-command-dispatch`

- owner: `crates/freehand-runtime`
- allowed_paths: `crates/freehand-runtime/**`, `crates/freehand-reason/**`, `crates/freehand-node/**`, `crates/freehand-ui-protocol/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/design/**`, `docs/architecture/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `apps/freehand-server/**` except protocol-only transport injection, `crates/freehand-provider-*/**`
- required_checks:
  - `cargo test -p freehand-runtime`
- required_white_box_tests:
  - config-selected runtime bootstrap tests
  - submit-input dispatch routing tests
  - cancel-turn dispatch tests
  - direct-message dispatch tests
  - resume-turn unsupported dispatch tests
  - runtime ui-state projection update tests
- required_module_black_box_tests:
  - command dispatch receipt smoke
  - command dispatch owner-routing smoke
  - reason-backed turn projection smoke
  - node-backed direct-message smoke
  - config-selected runtime bootstrap smoke
- required_project_black_box_tests:
  - runtime dispatch owner stays outside app boundary smoke
- test_design_doc: `docs/testing/runtime.ui-command-dispatch.md`
- function_map_doc: `docs/function-maps/runtime.ui-command-dispatch.md`
- mainline_call_doc: `docs/mainline-calls/runtime.ui-command-dispatch.json`
- generated_wiki_doc: `docs/wiki/runtime.ui-command-dispatch.md`
- debug_artifacts:
  - runtime dispatch smoke fixtures
- runtime_paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/state/ui`
  - `~/.freehand/ledgers/reason`
- update_triggers:
  - command-to-owner routing changes
  - runtime dispatch receipt/failure contract changes
  - runtime reason/node adapter behavior changes
  - app/runtime injection boundary changes
- lifecycle_checks:
  - apps remain protocol-only and do not become runtime owners
  - command dispatch owner routing remains explicit and single-sourced
  - reason turn truth mutation still stays inside `freehand-reason`
  - node direct-message/task semantics still stay inside `freehand-node`

### `app.webui-smoke`

- owner: `apps/freehand-server`
- allowed_paths: `apps/freehand-server/**`, `crates/freehand-ui-protocol/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/goals/**`
- forbidden_paths: `crates/freehand-provider-*/**`, `crates/freehand-reason/**`, `crates/freehand-node/**`, `crates/freehand-config/**` except consuming already-owned UI protocol projections
- required_checks:
  - `cargo test -p freehand-server`
- required_white_box_tests:
  - none beyond app boundary rendering helpers
- required_module_black_box_tests:
  - WebUI command ingress accept smoke
  - WebUI command ingress query-route-misuse rejection smoke
  - WebUI query projection smoke
  - WebUI debug query projection smoke
  - WebUI latest-turn SSE subscribe smoke
  - WebUI debug SSE subscribe smoke
  - WebUI slave-card render smoke
  - CLI/WebUI divergence smoke via protocol projection
- required_project_black_box_tests:
  - app boundary WebUI consumes `freehand-ui-protocol` projection truth without provider/reason imports
- test_design_doc: `docs/testing/app.webui-smoke.md`
- function_map_doc: `docs/function-maps/app.webui-smoke.md`
- debug_artifacts:
  - WebUI smoke stdout fixture
- runtime_paths:
  - `~/.freehand/state/ui`
  - `~/.freehand/replays/ui`
- update_triggers:
  - WebUI command shape changes
  - WebUI projection shape changes
  - UI protocol projection rules change
- lifecycle_checks:
  - WebUI remains app/render boundary only
  - WebUI consumes `freehand-ui-protocol` truth
  - query and subscribe remain protocol-owned
  - slave-card divergence remains protocol-safe

### `node.master-slave`

- owner: `crates/freehand-node`
- allowed_paths: `crates/freehand-node/**`, `crates/freehand-contracts/**`, `crates/freehand-ui-protocol/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-provider-*/**`, `apps/**` except wiring-only entrypoint glue
- required_checks:
  - `cargo test -p freehand-node`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
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
- mainline_call_doc: `docs/mainline-calls/node.master-slave.json`
- generated_wiki_doc: `docs/wiki/node.master-slave.md`
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
  - generated wiki freshness policy changes
- lifecycle_checks:
  - input-permission lifecycle is complete
  - pairing path and rejection path are both explicit
  - runtime evidence remains traceable
  - local one-master one-slave scope remains explicit
  - pairing-loss and re-listen path remain explicit
  - migrated mainline call source and generated wiki stay in sync with the function map
