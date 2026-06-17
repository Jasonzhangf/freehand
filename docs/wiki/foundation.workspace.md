# Wiki: `foundation.workspace`

Generated from `docs/mainline-calls/foundation.workspace.json`. Do not edit by hand.

- owner crate: `xtask`
- owner module: `xtask/src/main.rs`
- function map: `docs/function-maps/foundation.workspace.md`
- generated wiki: `docs/wiki/foundation.workspace.md`
- test design: `docs/testing/foundation.workspace.md`

## Request Mainline

- repo root may invoke `xtask gates check`
- repo root may invoke `xtask mainlines generate`
- repo root may invoke `xtask mainlines check`
- gate runner verifies required files, workspace members, policy doc snippets, and generated wiki freshness
- gate runner verifies migrated mainline manifest cross-links between JSON truth, feature map, function map, test design, and generated wiki path
- gate runner verifies migrated mainline call-table `bound` rows still point to existing files and discoverable source symbols
- gate runner verifies `make ci`, pre-push, CI, and release paths include the canonical full gate with mainline freshness
- gate runner verifies static metadata/request boundary rules on source-owned request and metadata types
- mainline generator loads machine-readable feature sources from `docs/mainline-calls/*.json`

## Response Mainline

- gate returns success when required repo truth, workspace structure, and generated wiki freshness are present
- gate returns success when migrated mainline manifests are deterministically linked to their owner docs
- gate returns success when migrated mainline call-table bindings resolve to source files and source symbols
- gate returns success when local and remote automation routes through the same full gate stack
- gate returns success when request-node contracts remain free of metadata/debug types and metadata owner types remain free of request payload fields
- gate returns explicit failure with missing path, missing policy snippet, or stale generated wiki
- mainline generation writes `docs/wiki/*.md` and `docs/wiki/README.md` from JSON truth
- mainline freshness check returns success only when current generated wiki matches current JSON source

## Error Mainline

- missing file or missing required snippet surfaces as gate failure
- mismatched mainline manifest source path, function map path, test design path, generated wiki path, or feature-map link surfaces as gate failure
- missing source file or missing source symbol in a migrated `bound` call-table row surfaces as gate failure
- missing `mainlines check` in `make ci` or CI/CD full-gate wiring surfaces as gate failure
- request-node structs that introduce metadata/debug/cache payload fields or types surface as gate failure
- ad hoc metadata owner types outside `freehand-metadata` or metadata owner structs that introduce request payload fields surface as gate failure
- invalid JSON mainline source surfaces as generation/check failure
- stale generated wiki surfaces as explicit freshness failure
- no fallback path exists

## Shared Multi-Reference Functions


## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run_gates_check` | `xtask/src/main.rs` | workspace gate orchestrator | repo root state | gate result | CLI `main` | helper verifiers | bound |
| 02 | `require_files` | `xtask/src/main.rs` | required-file presence check | repo file list | pass/fail | run_gates_check | filesystem | bound |
| 03 | `verify_workspace_members` | `xtask/src/main.rs` | workspace member cargo check | workspace member list | pass/fail | run_gates_check | filesystem | bound |
| 04 | `verify_skill_rules` | `xtask/src/main.rs` | skill rule snippet check | skill text | pass/fail | run_gates_check | file reader | bound |
| 05 | `verify_orchestrator_policy_docs` | `xtask/src/main.rs` | policy doc snippet check | docs text | pass/fail | run_gates_check | file reader | bound |
| 06 | `verify_generated_wiki` | `xtask/src/main.rs` | generated wiki freshness check | JSON mainline truth plus current wiki files | pass/fail | run_gates_check | wiki renderer/checker | bound |
| 07 | `run_mainlines_generate` | `xtask/src/main.rs` | mainline wiki generation command | repo root state | generated wiki refresh result | CLI `main` | mainline generator | bound |
| 08 | `run_mainlines_check` | `xtask/src/main.rs` | mainline wiki freshness command | repo root state | freshness check result | CLI `main` | mainline checker | bound |
| 09 | `generate_mainline_wikis` | `xtask/src/main.rs` | write or verify generated wiki artifacts from JSON sources | repo root plus write flag | wiki generation/check result | mainline commands | renderer pipeline | bound |
| 10 | `render_all_mainline_wikis` | `xtask/src/main.rs` | enumerate JSON sources and derive all wiki outputs including README index | `docs/mainline-calls/*.json` | expected wiki path/content pairs | mainline commands plus gate | renderer pipeline | bound |
| 11 | `load_mainline_doc` | `xtask/src/main.rs` | parse one machine-readable mainline source | JSON source file | typed mainline document | renderer pipeline | serde loader | bound |
| 12 | `render_mainline_wiki` | `xtask/src/main.rs` | render one human-readable wiki artifact from one typed mainline document | typed mainline document | wiki markdown | renderer pipeline | markdown renderer | bound |
| 13 | `verify_mainline_manifest_links` | `xtask/src/main.rs` | validate migrated mainline manifest cross-links | JSON mainline truth plus feature/function/testing docs | pass/fail | run_gates_check | filesystem and mainline loader | bound |
| 14 | `verify_mainline_call_table_bindings` | `xtask/src/main.rs` | validate migrated mainline call-table file and symbol bindings | JSON mainline truth plus source files | pass/fail | run_gates_check | filesystem and symbol resolver | bound |
| 15 | `verify_ci_cd_gate_commands` | `xtask/src/main.rs` | validate local hook, Makefile, CI, and release full-gate command alignment | automation config files | pass/fail | run_gates_check | filesystem and policy snippets | bound |
| 16 | `verify_metadata_request_boundaries` | `xtask/src/main.rs` | validate static metadata/request isolation rules on source-owned request and metadata types | Rust source files for contracts and metadata owners | pass/fail | run_gates_check | source scanners | bound |

## Sync Status Against Mainline Call

- workspace gate orchestration, generated-wiki freshness checks, and wiki generation pipeline are bound in code
- current gate baseline enforces required files, policy docs, generated wiki freshness, migrated mainline manifest cross-links, migrated mainline call-table bindings, CI/CD full-gate command alignment, and static metadata/request boundary checks
- generated wiki must be regenerated from `docs/mainline-calls/foundation.workspace.json` when this function-map truth changes
