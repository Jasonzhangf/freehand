# Function Maps

This directory is the durable truth for code-bound feature mainlines.

`docs/architecture/feature-map.md` answers:

- who owns the feature
- which resources the feature owns through `Resource Ownership Index`
- where it may change
- how it is validated
- which problem area routes to which `feature_id`

This directory answers:

- which symbols implement the feature
- how request mainline runs
- how response mainline runs
- how error mainline runs
- which shared functions are reused from multiple sites
- how function calls bind back to code
- where data and control state remain type-isolated when a mainline crosses modules
- where metadata writes route through `metadata.core` with writer owner and write-node provenance
- where context segment admission is locked when a feature adds model-visible context
- where the test orchestration document lives through the paired `test_design_doc`
- where the machine-readable mainline call source lives for migrated features
- where the generated wiki artifact lives for migrated features

`docs/resource-maps/` answers the global resource questions that a feature-local function map cannot answer by itself:

- which resources exist
- which feature owns each resource
- how resources may connect directly
- which bound source edge registers each direct operation in code
- which resource relations are indirect and must pass through a required intermediate resource
- which direct relations are forbidden

## Owner Routing Rule

Problem location is not grep-first.

Use this exact chain:

1. `docs/resource-maps/core.json`
2. source resource, target resource, direct-or-indirect relation, and `source_edge_registry` row for bound direct edges
3. `docs/architecture/feature-map.md` `Owner Routing Index`
4. one `feature_id`
5. one owner module/crate
6. one `docs/function-maps/<feature-id>.md`
7. one `docs/testing/<feature-id>.md`
8. mapped white-box, module black-box, and project black-box tests

If a function map cannot identify the owner symbol or mainline, the feature is not ready for implementation or closure.

If the resource map does not allow the relation, the feature is not ready for implementation. A bound operation pair needs both an `allowed_direct=true` relation rule and a `source_edge_registry` row. Add or correct the resource map and gate first instead of connecting modules directly.

For features with resource operations, `## Resource Map Binding` is a gated section, not a prose heading. It must include non-empty `owned resources`, `touched resources`, `resource operations`, and `forbidden shortcuts`, and the section must name the operation id plus the source and target resources from `docs/resource-maps/core.json`.

## Required Per-Feature Sections

Every feature function-map doc must contain:

- feature id
- owner crate
- owner module
- owner entry symbols
- request mainline
- response mainline
- error mainline
- shared multi-reference functions
- function call table
- sync status against code
- data/control isolation notes when feature crosses module boundaries
- metadata owner/write-node notes when feature writes internal control metadata
- paired `test_design_doc` awareness through the feature map
- for migrated features, the machine-readable mainline call source path
- for migrated features, the generated wiki path

## Code Binding Rule

Every call-table row should bind to code with:

- symbol path
- file path
- responsibility
- input semantic
- output semantic
- caller
- callee

If implementation is not landed yet, mark the row as binding pending. Do not invent symbols.

## Shared Multi-Reference Function Rule

If one function is called from multiple places, document it once here with:

- canonical symbol path
- owner
- shared purpose
- allowed callers
- related tests
- why it is shared instead of duplicated

## Update Rule

When code changes:

- update the call table
- update request/response/error mainlines
- update shared-function descriptions
- update sync status
- update data/control isolation notes for cross-module paths
- update metadata owner/write-node notes when metadata write behavior changes
- if the feature is migrated, update the machine-readable mainline call source and regenerate wiki

If these are not updated, the feature is not closed.

## Generated Wiki Rule

For migrated features:

- `docs/mainline-calls/<feature-id>.json` is the machine-readable mainline call source
- `docs/wiki/<feature-id>.md` is the generated wiki artifact
- generate with `cargo run -p xtask -- mainlines generate`
- validate with `cargo run -p xtask -- mainlines check`
- do not edit generated wiki files by hand
