# Resource Maps

This directory is the durable truth for global resource ownership and allowed resource relations.

Function maps remain feature-local code binding documents. Resource maps sit above them and answer:

- what resources exist
- which feature owns each resource
- how each resource is identified
- where resource truth is stored
- which operations may mutate or project it
- which projections expose it to callers, UI, ledgers, or runtime surfaces
- which resources may connect directly
- which resources may connect only through an intermediate resource
- which direct resource edges are forbidden

## Required Read Order

Before non-trivial implementation, debugging, or architecture work:

1. read `docs/resource-maps/core.json`
2. identify the source resource and target resource
3. confirm whether the relation is direct or indirect
4. if indirect, follow the required `via_resources`
5. then open the owner feature map, function map, mainline call source, and test design

If a relation is not in the resource map, do not implement it by directly connecting modules. Add or correct the resource map first, then update the bound feature function maps and gates.

## Direct vs Indirect Relations

A direct relation means the source resource may call or write the target resource through the named owner operation.

An indirect relation means the source and target must be connected through one or more required resources. For example, if `ui_session` reaches `task` only through `ui_projection`, UI code must not read or mutate task truth directly.

Every bound operation pair must have an `allowed_direct=true` `relation_rules` entry. Operation bindings describe a concrete owner operation; relation rules describe whether the resource pair is architecturally allowed to connect directly. Do not rely on operation bindings alone as implicit direct relation truth.

Direct relation rules must keep `via_resources` empty. Indirect relation rules must set `allowed_direct=false` and list every required `via_resources` hop.

Forbidden direct relations are explicit. They document paths that may look convenient but are architecturally invalid because they bypass the required owner resource.

A forbidden direct relation may not conflict with an `allowed_direct=true` relation rule for the same source/target pair. If the pair is forbidden, model it as an indirect relation with required `via_resources` and keep it out of direct operation bindings.

Every relation rule must declare a non-empty `reason`. For direct rules, the reason explains why the source resource is allowed to operate on the target directly. For indirect rules, the reason explains why the path must route through the listed `via_resources`.

Every forbidden direct relation must declare a non-empty `reason`. The reason explains why direct access is invalid, while `required_via` names the resource path that must be used instead.

Every forbidden direct relation must be backed by a matching indirect `relation_rules` entry with the same `source_resource`, `target_resource`, and `via_resources`. `forbidden_direct_relations.required_via` is not a standalone relation truth; it must agree with the indirect relation rule that explains how the source may reach the target without a direct shortcut.

Every forbidden direct relation must also declare source-gate status:

- `checked`: a matching `source_shortcut_gates` entry enforces a static owner-crate dependency/token boundary
- `precise_checked`: a matching `precise_source_edge_gates` entry checks a specific file/symbol body for required owner-hop tokens and forbidden direct-call tokens

`deferred` is intentionally not accepted by `xtask gates check`. If a relation is real but cannot be gate-checked yet, add the precise gate design before marking it as a forbidden direct relation.

## Current Manifest

- core map: `docs/resource-maps/core.json`
- gate: `cargo run -p xtask -- gates check`

## Required Core Resources

The `freehand.core-resource-map` manifest must include these resource types:

- `config`
- `session`
- `turn`
- `request_context`
- `provider_request`
- `provider_response`
- `tool_call`
- `workspace_path`
- `task`
- `agent`
- `timer`
- `error`
- `metadata`
- `debug_trace`
- `ui_projection`
- `runtime_command`
- `checkpoint`
- `node_pairing`
- `instruction_capability`

`xtask gates check` rejects the core map if any required resource is missing. Smaller test fixtures may use another `resource_map_id` and are not required to duplicate the production resource set.

## Resource Projection Rule

Every resource must declare at least one projection. A projection is the named surface where the resource becomes observable outside its owner, such as a UI protocol frame, tool result, debug/query row, runtime receipt, ledger view, or internal owner wakeup.

`xtask gates check` validates that:

- each resource has a non-empty `projections` list
- projection strings are non-empty
- projection strings are unique within that resource

Do not add a resource that has only ownership/truth-store prose but no declared projection. If a resource is intentionally internal, name the internal owner-facing projection explicitly so readers can still tell how the resource is observed and tested.

## Feature-Map Backlink

Every resource owner must be listed in `docs/architecture/feature-map.md` under `## Resource Ownership Index`.

`xtask gates check` validates:

- each resource `owner_feature_id` has a feature-map ownership row
- the row lists the owned `resource_type`
- the row links back to `docs/resource-maps/core.json`
- ownership rows may not list unknown resources
- a resource may appear under only one owner feature
- feature-map ownership must match the resource map's `owner_feature_id`
- feature-map seed `owner` must contain the resource map `owner_crate`
- duplicate feature ownership rows fail

This keeps ownership discoverable from both directions: resource map -> owner feature, and feature map -> owned resources.

## Backlink Rule

Every operation in `operation_bindings` must be backlinked from:

- `source_edge_registry` when the operation has a bound mainline call-table row
- the referenced `docs/mainline-calls/<feature-id>.json` `resource_operations` list
- at least one referenced mainline call-table row with matching `source_resource`, `target_resource`, and `resource_operation` when the operation is `bound`
- the paired `docs/function-maps/<feature-id>.md`
- the paired `docs/testing/<feature-id>.md`

This makes the resource center the top-level truth while keeping function maps, mainline call maps, and test designs bound to the same resource operation id.

Function-map backlinks must live under `## Resource Map Binding`. That section must declare non-empty `owned resources`, `touched resources`, `resource operations`, and `forbidden shortcuts`, and the section must name each bound operation id plus its source and target resources. Do not satisfy the backlink by mentioning an operation id elsewhere in prose while leaving the resource binding list empty.

Mainline call-table rows may not declare `source_resource` or `target_resource` unless they also declare a valid `resource_operation`. This prevents a direct resource edge from appearing in a machine-readable mainline without being registered in `operation_bindings`.

Operation ids must use `<source_resource>.<operation>` format. The source prefix must match `source_resource`, and the operation suffix must be listed in that source resource's `operations` array. This keeps resource-level allowed operations authoritative instead of letting operation bindings create unlisted capabilities.

Every operation binding must declare all contract fields with non-empty values:

- `operation_id`
- `owner_feature_id`
- `source_resource`
- `target_resource`
- `effect`
- `mainline_call_doc`
- `binding_status`

`binding_status` must be `bound` or `pending`. Use `pending` for a real planned resource operation that has no code-bound source edge yet. Do not leave `effect` empty; it is the human-readable reason the source resource is allowed to operate on the target resource.

Pending operations must also declare closure truth:

- `pending_reason`: why the operation is not code-bound yet
- `pending_closure_doc`: the design or planning doc that owns the closeout path
- `pending_verification`: the verification entrance required before changing `binding_status` to `bound`

`xtask gates check` validates those fields and requires `pending_closure_doc` to point to an existing repo file. Pending is an explicit tracked gap, not a vague placeholder.

## Source Edge Registry

`source_edge_registry` is the resource-center-level index for code-bound direct resource edges.

Each row identifies one mainline call-table source edge by:

- `edge_id`
- `operation_id`
- `source_resource`
- `target_resource`
- `mainline_call_doc`
- `call_table_step`
- `file_path`
- `symbol_path`
- `binding_status`

`xtask gates check` validates the registry in both directions:

- every registry entry references a known bound resource operation
- registry source/target/mainline/status match the operation binding exactly
- registry `file_path` entries exist in the repo
- registry `symbol_path` entries resolve in the listed files
- the referenced mainline call-table row exists
- row step, operation, endpoints, file path, and symbol path match the registry entry
- every bound mainline row with a resource operation appears in the registry
- pending operations do not need a registry entry and must not fake a source edge

This keeps source-edge truth from living only in scattered feature-local call tables or from being mutually backfilled by a stale mainline row and stale registry row. The registry is a code-bound resource edge index, not only a doc cross-reference.

## Test Coverage Binding

Every test design that backs a resource operation must include `## Resource Operation Test Coverage`.

Each row is checked by `xtask gates check` and must include:

- the exact resource operation id
- the current `binding_status`
- white-box coverage
- module black-box coverage
- project black-box coverage

If a resource operation is intentionally not implemented yet, keep it in the table with `pending` status and explicit pending coverage notes. Do not invent tests or fake bound code symbols to satisfy the map.

For `bound` operations, coverage cells must describe current verification entrances. They may not use pending or future-placeholder wording such as `pending`, `future`, `not claimed`, `not yet`, `TODO`, or `TBD`. If coverage is not real yet, keep the operation pending or document the uncovered scope outside the bound coverage row.

Each `bound` coverage cell must include a command-style verification entry, such as a `cargo test ...`, `cargo run -p xtask ...`, `make ...`, `scripts/...`, `node ...`, `bash ...`, `jq ...`, `grep ...`, `freehand-cli...`, or `./gradlew ...` command. Natural-language descriptions may explain the command, but they do not replace it.

`xtask gates check` also validates command entry targets where they are repo-owned:

- `cargo ... -p <package>` / `cargo ... --package <package>` package names must exist in repo Cargo manifests
- `scripts/...` entries must point to an existing script file
- `make <target>` entries must point to a target in the repo `Makefile`

The gate does not execute every command in the table; it proves the mapped verification entrance is syntactically command-like and points at an existing repo entry. Run the mapped commands before claiming a feature-level code refactor is closed.

## Source Shortcut Gate

`source_shortcut_gates` records forbidden direct resource relations that can be checked against source crates without full call-graph analysis.

For each entry, `xtask gates check` validates:

- the source and target resources exist
- the gate declares a non-empty `reason`
- the gate declares at least one real check through `forbidden_packages` or `forbidden_import_tokens`
- the source resource owner crate does not depend on any `forbidden_packages`
- Rust files under the source resource owner crate do not contain any `forbidden_import_tokens`

Use this only for static owner-boundary shortcuts that are safe to scan directly. Runtime orchestrator crates may legitimately depend on multiple owners, so those cases require a more precise future call-edge gate rather than a broad dependency ban.

`xtask gates check` also validates that:

- every `source_shortcut_gates` pair is declared in `forbidden_direct_relations`
- each `source_shortcut_gates` source/target pair is unique
- every forbidden relation marked `checked` has a matching source shortcut gate
- every forbidden relation marked `precise_checked` has a matching precise source edge gate
- `deferred` source-gate status is rejected so forbidden shortcuts cannot become permanent documented gaps

## Precise Source Edge Gate

`precise_source_edge_gates` exists for orchestrator crates that legitimately depend on multiple owner crates, where broad dependency scanning would create false positives.

For each entry, `xtask gates check` validates:

- the source and target resources exist
- each `precise_source_edge_gates` source/target pair is unique
- the pair is declared in `forbidden_direct_relations`
- the declared file exists
- the declared symbol resolves in that file
- the function body contains each `required_tokens` entry
- the function body does not contain any `forbidden_tokens`

Use this only for small, code-bound owner-hop functions. It is not a replacement for full source call-graph analysis across a whole crate.
