# Test Design: `reason.context-planner`

- feature_id: `reason.context-planner`
- owner: `crates/freehand-blocks`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `turn.plan_request_context`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `turn.plan_request_context` | bound | `cargo test -p freehand-blocks -- --nocapture` covers segment ordering, cache shape, rewrite-version, subagent conclusion, instruction capability, task contract, task-space snapshot, current-time, and turn-volatile AttentionResolution planner tests | `cargo test -p freehand-runtime live_bridge_admits_instruction_capability_manifest_as_typed_context -- --nocapture`, `cargo test -p freehand-runtime live_bridge_admits_long_operator_task_without_semantic_truncation -- --nocapture`, and `cargo test -p freehand-runtime live_master_attention -- --nocapture` cover provider-neutral request content build, typed instruction/attention admission, metadata isolation, and cache diagnostics boundary tests | `cargo test -p freehand-runtime effective_context_uses_last_repaired_round_without_raw_failed_attempt -- --nocapture` and `cargo test -p freehand-runtime live_master_attention -- --nocapture` cover reason-to-provider stable cache head, repaired-failure context economy, and stale-output-free attention continuation |

- lifecycle path under test:
  - stable prefix is classified and held stable
  - volatile tail is appended without mutating stable prefix
  - subagent search returns one final conclusion admitted as parent context
  - metadata and request payload stay hard-isolated
  - explicit rewrite events are the only path that changes rewrite version and rewrite mode in planner diagnostics
  - restored same-session context supplied by runtime restore has already excluded superseded repaired-failure rounds from default prompt context
  - task contracts are stable/cacheable context while task-space snapshots are volatile/no-cache context
  - current framework time is volatile/no-cache context and is excluded from stable
    rewrite/cache truth
  - attention resolutions are volatile/no-cache typed developer context ordered
    after the refreshed task-space snapshot and rejected from rewrite base
  - instruction capability content is session-stable/cacheable typed context and must enter through instruction owner output, not provider payload patching
- white-box plan:
  - segment classification tests
  - segment ordering tests
  - task contract and task-space snapshot cache-shape tests
  - instruction capability typed segment admission tests
  - attention resolution typed segment admission and rewrite-base rejection tests
  - segment token-cap rejection tests
  - subagent conclusion admission tests
  - raw subagent transcript rejection tests
  - cache-shape hash drift tests
  - tool-schema hash drift tests
  - rewrite-version bump tests
  - rewrite-base validation tests
- module black-box plan:
  - planner builds provider-neutral request content from stable + volatile inputs
  - planner keeps task-space snapshots out of stable-prefix diagnostics while task contract changes drift the stable prefix
  - planner admits instruction capability segments into stable-prefix diagnostics with explicit `instruction_capability` kind
  - planner admits AttentionResolution only as volatile/no-cache context after
    TaskSpaceSnapshot
  - planner rejects metadata/request mixed inputs
  - planner emits cache diagnostics without changing request content
- project black-box impact:
  - reason-to-provider request build keeps stable cache head across ordinary turns
  - repaired failed attempts are not re-admitted into future default prompt context after runtime restore has selected the latest repaired round
  - subagent search enriches parent turn by final report only
  - compaction/rollback remain the only context rewrite gates
- fixtures / replay inputs / runtime evidence paths:
  - context planner replay fixtures
  - subagent final-report fixtures
  - `~/.freehand/ledgers/context`
  - `~/.freehand/replays/context`
- known gaps:
  - exact compaction summary payload format is still pending implementation binding
- sync status between design and implementation:
  - design locked
  - planner baseline implemented in `freehand-blocks`
  - session-history rewrite-mode/version wiring is landed
  - runtime tool-schema fingerprint wiring is landed through reason turn input and the runtime live bridge
  - repaired-failure context economy is locked at the runtime restore boundary before segments enter planner admission
  - task contract and task-space snapshot segment contracts are locked in planner admission
  - instruction capability segment contract is locked in planner admission
  - attention resolution segment contract is locked in planner admission and
    live Master continuation tests
  - migrated mainline-call source and generated wiki are kept in sync with this test design
