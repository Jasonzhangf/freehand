# Dev Gates

Freehand uses one gate stack locally and in CI.

## Required Local Gate

Canonical command:

```bash
make ci
```

Expanded command stack:

```bash
cargo build --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

`cargo test --workspace` is the mandatory test umbrella. As modules gain tests, it must cover:

- module white-box tests
- module black-box tests
- project black-box tests

No feature may claim regression-safe completion unless all three mapped layers pass where applicable.

## Commit And Push Rule

- commit requires format and architecture gate
- push requires `make ci`
- CI reruns `make ci`
- release jobs rerun `make ci` before building release binaries
- gate failures block commit and push; no bypass-by-default workflow exists

## Test Taxonomy

- module white-box: internal semantic behavior of the owner crate, including validator/builder/parser/projector edge cases
- module black-box: standard user-visible or caller-visible behavior at the module contract boundary
- project black-box: typical end-to-end application behavior across crate boundaries

Every feature map entry must state its required tests in this taxonomy rather than as an unstructured list.

## Per-Change Expectation

For every feature change:

- identify the owner feature in `docs/architecture/feature-map.md`
- run its mapped white-box tests
- run its mapped module black-box tests
- run its mapped project black-box tests
- run workspace build, lint, and architecture gates

If a layer is intentionally not yet present for a feature, that absence must be explicit in the function map or test strategy docs rather than assumed.

## Architecture Rule

- search existing blocks and owner crates before adding a function
- orchestrator crates are not helper libraries
- reusable or semantic logic must land in `freehand-blocks`
- start development and debug from the resource map before the feature map
- start development and debug from function map and owner
- runtime home is `~/.freehand`
- truth change requires same-task updates to map, docs, skill, and memory
- `AGENTS.md` is router only; detailed truth must live in `docs/`

## Mainline Manifest Gate

`xtask gates check` validates migrated mainline-call sources as deterministic manifests:

- `docs/mainline-calls/<feature_id>.json` path must match its internal `feature_id`
- `function_map_doc`, `test_design_doc`, and `generated_wiki_doc` must point to the canonical feature paths
- function map and test design must contain the same `feature_id`
- function map must reference the same mainline-call source
- feature map must link the mainline-call source and generated wiki path

This keeps generated wiki artifacts as compiled review surfaces over one machine-readable truth instead of independent hand-maintained docs.

## Feature-Map Uniqueness Gate

`xtask gates check` validates that `docs/architecture/feature-map.md` does not carry duplicate seed entries:

- each `### \`<feature_id>\`` seed entry may appear only once
- duplicate owner entries for the same `feature_id` fail the gate even if the duplicated docs are text-identical

This keeps owner routing as one truth instead of allowing silent drift through duplicated seed blocks.

## Mainline Call-Table Binding Gate

`xtask gates check` validates every migrated mainline-call row with `binding_status = "bound"`:

- each `file_path` segment must point to an existing repo file
- each `symbol_path` segment must resolve in one of the listed files
- Rust-style `Type::method` entries may resolve through the method tail, such as `method`, because source files define methods as `fn method`
- `binding_status = "pending"` remains explicit and is not treated as a bound symbol

This keeps mainline call maps code-bound instead of becoming stale review prose.

## Resource Map Gate

`xtask gates check` validates the global resource map:

- `docs/resource-maps/core.json` must parse as the resource-center manifest
- `resource_map_id=freehand.core-resource-map` must include all required core resources: `config`, `session`, `turn`, `request_context`, `provider_request`, `provider_response`, `tool_call`, `workspace_path`, `task`, `agent`, `timer`, `error`, `metadata`, `debug_trace`, `ui_projection`, `runtime_command`, `checkpoint`, `node_pairing`, and `instruction_capability`
- every `resource_type` is unique
- every resource `owner_feature_id` exists in `docs/architecture/feature-map.md`
- every resource declares at least one non-empty, unique projection
- every resource owner feature is backlinked from `docs/architecture/feature-map.md` `Resource Ownership Index`
- every feature-map resource ownership row points back to `docs/resource-maps/core.json`
- feature-map resource ownership rows may not list unknown resources
- each resource may appear under only one feature-map owner
- feature-map resource ownership must match the resource map's `owner_feature_id`
- feature-map seed `owner` must contain each owned resource's `owner_crate`
- every resource operation name is non-empty and unique within that resource
- every operation binding contract field is non-empty: `operation_id`, `owner_feature_id`, `source_resource`, `target_resource`, `effect`, `mainline_call_doc`, and `binding_status`
- every pending operation binding declares non-empty `pending_reason`, `pending_closure_doc`, and `pending_verification`, and the closure doc exists
- every operation binding references existing source and target resources
- every operation binding id uses `<source_resource>.<operation>` format
- every operation binding operation suffix is listed in the source resource's `operations`
- every operation binding references an existing mainline call source
- every bound operation source/target pair has an `allowed_direct=true` `relation_rules` entry
- direct relation rules keep `via_resources` empty, while indirect relation rules must declare required `via_resources`
- every relation rule declares a non-empty `reason`
- relation rule ids and source/target pairs are unique
- forbidden direct relations may not conflict with an `allowed_direct=true` relation rule for the same source/target pair
- forbidden direct relations must declare a non-empty `reason`
- forbidden direct relations must be backed by a matching `allowed_direct=false` relation rule with identical `via_resources`
- every operation binding is backlinked from that mainline call source's `resource_operations`
- every operation binding is backlinked from the paired function map's `## Resource Map Binding` section, with non-empty owned resources, touched resources, resource operations, forbidden shortcuts, source resource, target resource, and operation id
- every bound operation binding is backlinked from at least one call-table row's `source_resource`, `target_resource`, and `resource_operation`
- every bound call-table row with `resource_operation` is registered in `source_edge_registry`
- every `source_edge_registry` entry references a known bound operation and matches operation source/target/mainline/status exactly
- every `source_edge_registry` entry directly binds to existing source files and resolvable source symbols
- every `source_edge_registry` entry matches an existing mainline call-table row by step, operation, endpoints, file path, and symbol path
- every call-table row `resource_operation` must also be listed in the same mainline source's `resource_operations`
- call-table rows may not declare `source_resource` or `target_resource` without a valid `resource_operation`
- every call-table row resource endpoint pair must match the referenced resource operation binding exactly
- every operation binding is also named in the paired function map and test design
- every operation binding has a `## Resource Operation Test Coverage` row in the paired test design
- each test coverage row must name the operation, match the binding status, and map white-box, module black-box, and project black-box coverage or an explicit pending note
- `bound` resource operation coverage rows may not use pending/future placeholder wording; pending coverage belongs to `binding_status=pending`
- each `bound` coverage cell must include a command-style verification entry; prose can describe the command but cannot replace it
- repo-owned command targets in `bound` coverage cells are validated: cargo `-p/--package` names must exist, `scripts/...` paths must exist, and `make <target>` must exist in `Makefile`
- every relation rule and forbidden direct relation references existing resources
- every forbidden direct relation declares `source_gate_status`
- `source_gate_status=checked` requires a matching `source_shortcut_gates` entry
- `source_gate_status=precise_checked` requires a matching `precise_source_edge_gates` entry
- `source_gate_status=deferred` is rejected; add a precise gate instead of leaving forbidden shortcuts as documented gaps
- every `source_shortcut_gates` pair must be backed by a declared forbidden direct relation
- `source_shortcut_gates` and `precise_source_edge_gates` source/target pairs must be unique
- every `source_shortcut_gates` entry must declare a non-empty reason and at least one actual `forbidden_packages` or `forbidden_import_tokens` check
- `source_shortcut_gates` forbid selected source owner crates from depending on or importing target owner crates when the resource map says direct access is invalid
- `precise_source_edge_gates` check selected file/symbol bodies for required owner-hop tokens and forbidden shortcut tokens when broad crate-level scanning would be too coarse

This is the first gate for the resource-center model. It does not yet prove every code edge is resource-bound, but it makes the resource relation map a required precondition before function-map work.

## CI/CD Command Alignment Gate

`xtask gates check` validates local and remote automation routes through the same full-gate truth:

- `Makefile` must provide a `mainlines` target that runs `cargo run -p xtask -- mainlines check`
- `Makefile` `ci` must include `build fmt clippy test mainlines gates`
- `.githooks/pre-push` must run `make ci`
- GitHub CI must run `make ci`
- release workflow must run `make ci` before release build/publish steps

This prevents pre-push, CI, and release from silently drifting into partial gate stacks.

## Source Search Boundary Gate

`xtask gates check` validates source-only search policy:

- `.ignore` must exclude generated/runtime outputs from default `rg` searches
- `scripts/source-search.sh` must search only source code, tests, maintained scripts, and canonical docs while excluding generated/runtime outputs
- `scripts/source-search.sh` must reject unsafe ignore-bypass options such as `--no-ignore`, `--unrestricted`, and `-u`
- `.agents/skills/freehand-dev/SKILL.md` and debug docs must preserve the source-first search rule
- generated outputs remain excluded from default implementation search and may only be opened as direct verification evidence

This prevents artifacts, generated wiki, build output, or MemoryPalace corpora from becoming accidental implementation truth.

## Metadata/Request Isolation Gate

`xtask gates check` validates one low-noise static boundary for data/control separation:

- request-node contract structs must not introduce metadata/debug/control owner types or obvious metadata/debug/cache/control payload fields
- metadata owner types must stay inside `crates/freehand-metadata`
- metadata owner structs must not introduce request payload fields such as prompt text, message arrays, or context payload content
- metadata owner structs must not introduce control execution payloads such as routing policy, checkpoint payload, cancel token, retry policy, or gate decision

This gate is intentionally narrow. It exists to fail obvious boundary regressions early without inventing fallback or runtime heuristics. Control state must stay in owner modules, ledgers, metadata, or debug channels; it must not be represented by ad hoc request/prompt/provider-payload rewrites.
