# Resource Center Top-Down Refactor Plan

## Objective

Refactor Freehand from the top down around a resource center: define resource ownership, allowed and forbidden relations, documentation, manifests, function maps, mainline call maps, and tests before changing runtime business code.

The refactor must make request, response, error, task, session, tool, timer, workspace, metadata, debug, and UI projection flows traceable as resource operations with one owner, one allowed relation path, one documentation truth, and one required verification stack.

## Acceptance Criteria

- `docs/resource-maps/core.json` is the first truth for all core resource ownership and resource relations.
- Each resource has exactly one owner feature and owner crate, declared identity, truth store, operations, and projections.
- Each allowed direct operation is represented by an operation binding, matching relation rule, function map section, mainline call-map row, source-edge registry row when bound, and test-design coverage.
- Each forbidden direct relation declares required via resources, reason, and an active source gate (`checked` or `precise_checked`), not a deferred gap.
- Function maps and mainline call maps can be used to answer "who owns this?", "which resource operation is this?", "which paths are allowed?", and "which tests prove it?" without grep-only archaeology.
- Docs and generated wiki are synchronized from manifests; generated `docs/wiki/**` is not edited by hand.
- Business-code refactor starts only after resource/docs/tests/gate skeleton is parseable, checked, and locally verified.

## Scope

In scope:

- Resource ownership and relation audit.
- `docs/resource-maps/core.json` schema/content completion.
- Feature map resource ownership index alignment.
- Function-map and mainline call-map resource operation binding.
- Test-design matrix for resource operations.
- Gate and red-test coverage for resource map consistency.
- Documentation and local skill updates when truth changes.
- Later code refactor only along documented resource owner and relation paths.

Out of scope until the skeleton is closed:

- Broad runtime/business-code rewrite.
- UI polish unrelated to resource ownership or relation projection.
- Release-profile validation unless runtime behavior changes and promotion is explicitly in scope.
- Fallback or compatibility paths that bypass the unique resource owner.
- Hand-editing generated wiki files.

## Design Principles

- Resource first: every change starts from resource owner and allowed relation lookup.
- Relation first: direct edges are forbidden unless explicitly allowed in the resource map.
- Owner unique: one resource, one owner feature, one owner crate, one truth store.
- Maps before code: resource map, feature map, function map, mainline call map, and test design must be updated before implementation refactor.
- Adjacent edges only: A cannot call B directly if the resource map says A must go through C.
- No fallback: failed relation, owner, or schema checks must fail explicitly.
- Generated docs only through generators: update source manifests, then run the generator/check gates.
- Rust-first gates: governance and resource consistency checks belong in `xtask`/Rust gates, not only prose.

## Source Truths

- `docs/resource-maps/core.json`
- `docs/resource-maps/README.md`
- `docs/architecture/feature-map.md`
- `docs/architecture/function-map-spec.md`
- `docs/function-maps/README.md`
- `docs/function-maps/*.md`
- `docs/mainline-calls/*.json`
- `docs/testing/*.md`
- `docs/architecture/dev-gates.md`
- `.agents/skills/freehand-dev/SKILL.md`
- `xtask/src/main.rs`
- `MEMORY.md`
- `note.md`

## Technical Plan

1. Audit the current resource center.
   - Read `docs/resource-maps/core.json` and list all resources, owners, operations, projections, relation rules, forbidden direct relations, operation bindings, source-edge registry rows, source shortcut gates, and precise source-edge gates.
   - Check for missing resources, duplicate owners, missing projections, missing relation reasons, pending bindings, and deferred or no-op gates.
   - Do not infer ownership from grep alone; use grep only to locate source files, then open files and verify symbols.

2. Close resource ownership.
   - Ensure each core resource has `resource_type`, `owner_feature_id`, `owner_crate`, `identity`, `truth_store`, operations, and projections.
   - Ensure the feature-map Resource Ownership Index links back to every resource exactly once and owner crate seeds match.
   - Add or fix red tests before accepting any new invariant.

3. Close resource relations.
   - For each direct operation, require an `operation_bindings` row and an `allowed_direct=true` relation rule.
   - For each forbidden direct relation, require `required_via`, `reason`, and active source enforcement.
   - For indirect-only paths, ensure the relation map explains the via resources and forbids direct shortcuts.
   - Reject conflicting allowed and forbidden rules for the same source/target pair.

4. Bind maps and tests.
   - For each operation binding, update the owning function map and mainline call map with resource operation IDs and source/target resources.
   - Add `Resource Operation Test Coverage` rows to test designs, including white-box, module black-box, and project black-box coverage.
   - Bound operations must have source-edge registry rows. Pending operations must be explicitly pending and must not fake source edges.

5. Strengthen gates.
   - Add red tests in `xtask` for every newly discovered invariant before relying on it.
   - Gate parseability, unique ownership, operation binding completeness, relation consistency, function-map backlinks, mainline source-edge backlinks, test-design coverage, and source shortcut enforcement.
   - Prefer precise source-edge gates when package/import token checks are too coarse.

6. Generate and verify docs.
   - Run mainline generation, then mainline checks and gates.
   - Do not edit `docs/wiki/**` directly.
   - Update `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, and `.agents/skills/freehand-dev/SKILL.md` when workflow truth changes.

7. Only then refactor code.
   - Pick one resource operation at a time.
   - Move behavior to the declared owner crate or shared block layer.
   - Remove duplicate implementations after dependency safety is verified.
   - Keep orchestrator crates as orchestration only.
   - Run the operation's mapped tests and the global map/gate stack before claiming closure.

## Risk And Mitigation

- Risk: resource map becomes documentation-only.
  Mitigation: every invariant must have `xtask` validation or a red test before relying on it.

- Risk: generated docs diverge from source manifests.
  Mitigation: edit manifests/source docs, run `cargo run -p xtask -- mainlines generate`, then checks.

- Risk: code refactor starts before ownership is closed.
  Mitigation: stop any business-code edit if resource owner, relation, function map, or test design is missing.

- Risk: shortcut relations remain hidden in source.
  Mitigation: use source shortcut gates and precise source-edge gates for known forbidden paths; report whole-repo automatic call-graph discovery as a residual gap unless implemented.

- Risk: dirty worktree contains unrelated changes.
  Mitigation: do not clean, revert, or stage unrelated work; touch only files required for this refactor.

## Verification Matrix

- Resource manifest parse:
  - `jq empty docs/resource-maps/core.json`

- Formatting and compile:
  - `cargo fmt --check`
  - `cargo check -p xtask`

- Gate red/green tests:
  - `cargo test -p xtask resource_map_ -- --nocapture`
  - `cargo test -p xtask -- --nocapture`

- Generated docs and registry checks:
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`

- Patch hygiene:
  - `git diff --check`

- Memory closeout:
  - Append verified findings to `note.md`.
  - Promote durable truth to `MEMORY.md`.
  - Run MemoryPalace mine/search if available; if locked, retry and report the gap without repair or process kill.

## Implementation Order

1. Current-state audit against the goal requirements.
2. Resource ownership closure.
3. Resource relation closure.
4. Function-map and mainline call-map binding closure.
5. Test-design coverage closure.
6. Gate and red-test closure.
7. Generated wiki/doc synchronization.
8. Requirement-by-requirement completion audit.
9. Only after the audit, start code refactor by one resource operation at a time.

## Current Resource-Center Audit

Current manifest evidence:

- `docs/resource-maps/core.json` declares the 19 required core resources: `config`, `session`, `turn`, `request_context`, `provider_request`, `provider_response`, `tool_call`, `workspace_path`, `task`, `agent`, `timer`, `error`, `metadata`, `debug_trace`, `ui_projection`, `runtime_command`, `checkpoint`, `node_pairing`, and `instruction_capability`.
- The map currently declares 14 operation bindings, all `bound`.
- `instruction_capability.admit_request_context` is now bound through `ContextSegmentKind::InstructionCapability`; instruction owner output enters request context as a typed context-planner segment, not by provider payload patching.
- The map currently declares 22 relation rules, 7 forbidden direct relations, 5 broad source shortcut gates, and 2 precise source-edge gates.
- Forbidden direct relations currently have no `deferred` source gate status; every forbidden relation is either `checked` or `precise_checked`.

Current gate evidence:

- Resource owners are backlinked through the feature-map Resource Ownership Index.
- Bound operation bindings are backlinked from mainline call sources, function maps, test designs, and `source_edge_registry`.
- `source_edge_registry` rows are checked against real file paths and symbol paths.
- Function-map `## Resource Map Binding` sections are required for resource-bound features.
- Test-design `## Resource Operation Test Coverage` rows are required for resource-bound operations.
- Bound coverage rows require command-style entries and repo-owned command targets where statically checkable.
- Pending operation bindings require an explicit closeout contract instead of vague placeholder text; the current production map has no pending operation bindings.

Known residual gaps:

- The gate validates declared source edges plus configured shortcut gates; it does not yet automatically discover every undeclared Rust resource edge across the whole repository.
- The gate validates coverage command presence and repo-owned targets; it does not execute every command listed in every coverage row.
- Business-code refactor is not globally complete. Any code refactor must start from one selected resource operation after checking this map, its owner function map, and its test-design coverage.

## Requirement Completion Audit

| requirement | current evidence | status |
| --- | --- | --- |
| `docs/resource-maps/core.json` is the first resource ownership and relation truth | production map declares 19 required resources, owner feature/crate, identity, truth store, operations, projections, operation bindings, relation rules, forbidden direct relations, source gates, and source-edge registry | complete |
| required core resources are covered | resource map contains `config`, `session`, `turn`, `request_context`, `provider_request`, `provider_response`, `tool_call`, `workspace_path`, `task`, `agent`, `timer`, `error`, `metadata`, `debug_trace`, `ui_projection`, `runtime_command`, `checkpoint`, `node_pairing`, and `instruction_capability` | complete |
| each key resource has unique owner, identity, truth store, operations, and projections | `xtask gates check` validates unique `resource_type`, non-empty owner/identity/truth-store, operation/projection presence and uniqueness, feature-map owner backlink, and owner-crate seed match | complete |
| direct, indirect, and forbidden relations are explicit | resource map has 22 relation rules and 7 forbidden direct relations; forbidden pairs require `required_via`, reason, matching indirect rule, and active `checked` or `precise_checked` source gate | complete |
| operation bindings are complete and resource-typed | 14 operation bindings are all `bound`; gate validates operation id format, source/target resources, source operation membership, owner feature, mainline doc, and direct relation rule | complete |
| function maps and mainline call maps backlink resource operations | resource-bound function maps require `## Resource Map Binding`; mainline rows with resource endpoints require matching resource operation; every bound operation has a source-edge registry row | complete |
| test designs explain verification entry points | resource-bound test designs require `## Resource Operation Test Coverage`; bound rows require white-box, module black-box, and project black-box cells with command-style entries and repo-owned targets where statically checkable | complete |
| generated wiki is synchronized from source manifests | `cargo run -p xtask -- mainlines generate` regenerates wiki from `docs/mainline-calls/*.json`; `cargo run -p xtask -- mainlines check` validates freshness | complete |
| gate blocks missing owner, unregistered direct edge, missing source-edge registry, conflicting relation, no-op source gate, duplicate source gate, and deferred source gate | `cargo test -p xtask resource_map_ -- --nocapture` covers the resource-map red-test set; `cargo run -p xtask -- gates check` passes on production docs | complete |
| code refactor only after resource/docs/tests/gate skeleton | business-code change happened only for selected operation `instruction_capability.admit_request_context` after resource map, closure doc, function map, mainline call map, and test design existed; it is now bound with source edge and S-profile proof | complete for selected code slice |
| final report can list residual risks | residual risks are scoped to stronger future automation: whole-repo undeclared Rust edge discovery and executing every coverage-row command inside gates | complete with residual risk |

## Definition Of Done

- The resource center can answer ownership, relation path, operation binding, source edge, docs, and required tests for every in-scope resource operation.
- `xtask` gates fail on missing owner, missing projection, unregistered direct edge, missing source-edge registry, conflicting relation, no-op source gate, duplicate source gate, or deferred source gate.
- All verification commands in the matrix pass in the current worktree.
- `note.md`, `MEMORY.md`, and local Freehand skill are updated for reusable workflow truth.
- Final report lists changed files, verification evidence, residual gaps, and the next concrete resource operation for code refactor.
