# Dev Gates

Freehand uses one gate stack locally, in CI, and in release automation.
The OpenMinis declaration gate requires `swiftc`; every workflow that runs
`make ci` installs it with `swift-actions/setup-swift@v2` before the stack.
Local source-dependent entrypoints first run
`scripts/provision-openminis-source.sh`. It reads the canonical manifest SHA,
creates only a missing sparse `external/OpenMinis` checkout, and rejects
existing origin, HEAD, or dirty-worktree drift.

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

## OpenMinis UI Migration Manifest Gate

`xtask gates check` validates the non-browser OpenMinis UI migration registry:

- the machine manifest and human migration tree contain the exact same required node ids
- the human registry and machine manifest have identical entrypoint, forward edge id/from/to/semantic, and return from/to/semantic sets; Mermaid node/forward pairs also remain exact, and every manifest node must be forward-reachable from `foundation.root`
- `.github/workflows/ci.yml` has one `actions/checkout` step whose `with.repository`, `with.ref`, and `with.path` jointly bind `OpenMinis/OpenMinis`, the manifest SHA, and `external/OpenMinis`; values in unrelated steps do not satisfy the gate
- the local/CI `external/OpenMinis` checkout exists, its HEAD equals the manifest SHA, the object is a commit, every node source path resolves recursively at that commit, and every source symbol resolves as exactly one declaration in those pinned blobs
- `make test`, `make gates`, `make ci`, and pre-commit invoke the deterministic local source provisioner before source-dependent validation; the provisioner itself clears hook-exported repository-local Git variables, validates the exact checkout root through Git so normal/worktree/submodule layouts are accepted, serializes concurrent first-run installation through an atomic owner-bearing lock, reclaims it only when the recorded same-host PID is absent, and cleans only its own staging/lock artifacts on every exit; waiting contenders verify the winner's exact checkout, and the Rust gate never fetches or mutates source
- repository evidence is produced by `cargo run -p xtask -- openminis-ui verify-node <node_id>`, which validates a source-bound projection without admitting evidence and therefore cannot recursively certify `xtask gates check`
- every verifier report binds the exact `node_id` and `migration_unit_id`; a generic report cannot promote multiple nodes
- every direct production call edge inside `xtask/src/openminis_ui_migration/`, including recursive self-edges, is derived without a function-name whitelist; production and test cfg projections are discovered independently, each processes only file modules reachable through active declarations, then they are merged by retaining all production truth plus test-only definitions/callers, so cfg-exclusive external modules, imports, and same-name definitions never share one false scope; migration-owned callees retain legitimate non-test inbound callers, external callees retain only migration-owned direct callers, and outside-to-outside edges are excluded; imported aliases and module/callable/nested lexical glob imports resolve single- and multi-segment paths before bare-name filtering, callable-local associated paths and module re-exports resolve instead of disappearing, reassigned local receiver truth is refreshed instead of staying stale, inline `Result<Local, E>` / `Option<Local>` `unwrap`/`expect` chains preserve their following local receiver edge, cfg-disabled statements/expressions/match arms contribute no edges, and callable parameters plus local values/`const`/`static` items shadow same-named module functions; block-local const/static initializers receive independent callers, while active nested impl/trait items, nested function items, local macro definitions, and `include!` are rejected because unexpanded or nested code cannot supply an independently registered module-qualified caller identity
- BrowserUse, Cookie, Profile, and Takeover source paths/symbols are rejected without treating `FileBrowserView` as browser-tool scope; exclusion is checked on every blob recursively resolved from a declared ancestor, not only on the ancestor string
- every migration unit has one owner feature and separates touched features
- lifecycle fields are validated per state: `owner_mapped` does not require target symbols; contract states require protocol/surface fields; source-bound and later states require target symbols; blocked states retain their named pending boundary
- manifest phase is consistent: `design_baseline` has only inventory/blocked nodes, `migration_in_progress` has promoted nodes without complete retirement, and `migration_complete` requires every included node to be `legacy_retired`
- map, mainline, test-design, target-path, resource-operation, and gate references exist
- a `syn`-parsed Rust module/import graph automatically derives the complete multi-reference production-plus-test-only shared-function set from separate cfg projections; external module test identity is resolved from parsed module ancestry before callable indexing, current target predicates come from `rustc --print cfg`, `test` is explicit, test-only identity requires absence from the production projection, inactive-in-both/unsupported cfg items fail, and `shared_functions`, exact module-qualified direct `allowed_callers`, module-qualified direct `related_tests`, and one-edge-per-row `openminis_ui_migration` mainline calls must match, with no tracked-symbol whitelist, bare-name merging, or prose/grouped aliases
- every lifecycle state validates function/mainline/test paths as canonical repository-relative files in their unique map directories; the three sets must carry identical feature ids, equal `touched_feature_ids`, include the owner feature, and prove Markdown/JSON path and self identity
- every promoted node must match one canonical resource-map operation exactly: `binding_status=bound`, one source resource, target resource, operation owner in touched features, and an `allowed_direct=true` relation; it must also bind the exact non-empty set of incident manifest edge ids in `route_edge_ids`; `source_bound` and later additionally accept only canonical repository-relative target/mainline paths, reject symlink traversal, select only supported declaration-source languages from broad target directories, and require every target symbol to resolve as exactly one language-aware declaration—not a comment, literal, call, longer identifier, or binary/XML/JSON asset—and equal an exact mainline symbol segment on a bound row with the same operation
- evidence covers its declared gates exactly and points to JSON attestations under `docs/migrations/openminis-ui/evidence/` whose node, gate, code-locked canonical command, proof kind, verifier identity, passed result, and run id match the evidence record; each attestation binds a distinct report under that root using schema `freehand.verifier-report.v1`, canonical path, and SHA-256. The report must carry the gate-specific passed assertions and attest the pre-promotion source commit/tree, so post-proof drift admits only the exact attestation/report paths plus top-level `status` and per-node `status`, `evidence`, and `legacy_retirement` changes in the canonical `docs/migrations/openminis-ui/ui-tree.manifest.json`; after removing those lifecycle fields, the attested and current manifests must be identical. The same verifier-report admission path handles repository, WebUI, Android-device, and legacy no-touch gates; required gate ids are never rejected categorically. The current manifest still passes the complete lifecycle/binding/evidence/topology gate, and command/proof/verifier/assertion/source/registry/manifest-contract drift fails
- `legacy_retired` independently requires exactly one `legacy_scan_roots` row in the node owner feature mainline, exact registry/manifest node-owner/path/removed-identity agreement, canonical repository-relative non-symlink directory roots covering every bound target and removed path, reject nested scan symlinks and treat broken legacy symlinks as present, absent registered legacy paths, no registered removed symbol/import/caller under those roots, the dedicated `openminis_ui_legacy_online_no_touch` gate, and an artifact proving `legacy_touched=false`; manifest-selected fabricated identities, arbitrary in-repository empty roots, wrong owners, path drift, uncovered identities, and absolute/non-canonical/out-of-repository paths fail
- every node is bound to this gate through `verification_gates`

The gate is part of `xtask gates check`, which is invoked by `make ci`, CI,
release, pre-commit, and pre-push. Missing pinned source or evidence is an
explicit failure; the gate never fetches, falls back, or treats design/pending
records as implementation truth.


## ADP Protocol Artifact Gate

`xtask gates check` validates generated ADP protocol artifacts:

- `crates/freehand-ui-protocol/generated/adp-protocol.schema.json` must exist and match a fresh `export-adp-protocol --json` run
- `apps/freehand-server/assets/webui/generated/adp-protocol.js` must exist and match a fresh `export-adp-protocol --js` run
- the served WebUI asset table must include the generated module
- WebUI ADP client/legacy shell must import and use generated constructors instead of maintaining a handwritten command-frame mirror

This keeps ADP command frame class, version, handshake capability, and owner routing single-sourced from Rust protocol truth while Gap 6 continues toward full payload schema/types, auth, and command-surface contraction.

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
- every present operation-owned `ui_contract` has non-empty `projection_or_query`, `generated_command`, and a normalized repository-relative `surface_path` that canonically resolves to an in-repository file; OpenMinis migration nodes at `contract_ready` or later must match those values exactly
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
- `.githooks/pre-commit` must clear `git rev-parse --local-env-vars` before running gates so nested pinned-source Git commands cannot inherit the outer commit index/worktree
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
