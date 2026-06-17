# Function Map: `foundation.workspace`

- feature_id: `foundation.workspace`
- owner crate: `xtask`
- owner module: `xtask/src/main.rs`
- owner entry symbols:
  - `run_gates_check`
  - `run_mainlines_generate`
  - `run_mainlines_check`
  - `generate_mainline_wikis`
  - `render_all_mainline_wikis`
  - `verify_mainline_manifest_links`
  - `verify_mainline_call_table_bindings`
  - `load_mainline_doc`
  - `render_mainline_wiki`
  - `verify_generated_wiki`

## Request Mainline

- repo root invokes `xtask gates check`
- repo root may invoke `xtask mainlines generate`
- repo root may invoke `xtask mainlines check`
- gate runner verifies required files, workspace members, and policy doc snippets
- gate runner verifies migrated mainline JSON cross-links back to feature map, function map, test design, and generated wiki path
- gate runner verifies migrated mainline call-table `bound` rows still point to existing files and discoverable source symbols
- mainline generator loads machine-readable feature sources from `docs/mainline-calls/*.json`
- generated wiki writer materializes `docs/wiki/*.md` and `docs/wiki/README.md` from the JSON truth

## Response Mainline

- gate returns success when required repo truth and workspace structure are present
- gate returns success when migrated mainline manifests are deterministic and cross-linked to their owner docs
- gate returns success when migrated mainline call-table bindings resolve to source files and symbols
- gate returns explicit failure with missing path or missing policy snippet
- mainline generation returns fresh wiki artifacts derived from machine-readable source
- mainline freshness check returns explicit failure when any generated wiki is stale against current JSON truth

## Error Mainline

- missing file or missing required snippet surfaces as gate failure
- mismatched mainline manifest path, generated wiki path, function map, test design, or feature map link surfaces as gate failure
- missing source file or missing source symbol in a migrated `bound` call-table row surfaces as gate failure
- invalid JSON mainline source surfaces as generation/check failure
- stale generated wiki surfaces as explicit freshness failure
- no fallback path exists

## Shared Multi-Reference Functions

- none at current scaffold stage

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run_gates_check` | `xtask/src/main.rs` | workspace gate orchestrator | repo root state | gate result | CLI `main` | helper verifiers | bound |
| 02 | `require_files` | `xtask/src/main.rs` | required-file presence check | repo file list | pass/fail | `run_gates_check` | filesystem | bound |
| 03 | `verify_workspace_members` | `xtask/src/main.rs` | workspace member cargo check | workspace member list | pass/fail | `run_gates_check` | filesystem | bound |
| 04 | `verify_skill_rules` | `xtask/src/main.rs` | skill rule snippet check | skill text | pass/fail | `run_gates_check` | file reader | bound |
| 05 | `verify_orchestrator_policy_docs` | `xtask/src/main.rs` | policy doc snippet check | docs text | pass/fail | `run_gates_check` | file reader | bound |
| 06 | `verify_generated_wiki` | `xtask/src/main.rs` | generated wiki freshness check | JSON mainline truth + current wiki files | pass/fail | `run_gates_check` | wiki renderer/checker | bound |
| 07 | `run_mainlines_generate` | `xtask/src/main.rs` | mainline wiki generation command | repo root state | generated wiki refresh result | CLI `main` | mainline generator | bound |
| 08 | `run_mainlines_check` | `xtask/src/main.rs` | mainline wiki freshness command | repo root state | freshness check result | CLI `main` | mainline checker | bound |
| 09 | `generate_mainline_wikis` | `xtask/src/main.rs` | write or verify generated wiki artifacts from JSON sources | repo root + write flag | wiki generation/check result | mainline commands | renderer pipeline | bound |
| 10 | `render_all_mainline_wikis` | `xtask/src/main.rs` | enumerate JSON sources and derive all wiki outputs including README index | `docs/mainline-calls/*.json` | expected wiki path/content pairs | mainline commands + gate | renderer pipeline | bound |
| 11 | `load_mainline_doc` | `xtask/src/main.rs` | parse one machine-readable mainline source | JSON source file | typed mainline document | renderer pipeline | serde loader | bound |
| 12 | `render_mainline_wiki` | `xtask/src/main.rs` | render one human-readable wiki artifact from one typed mainline document | typed mainline document | wiki markdown | renderer pipeline | markdown renderer | bound |
| 13 | `verify_mainline_manifest_links` | `xtask/src/main.rs` | validate migrated mainline manifest cross-links | JSON mainline truth plus feature/function/testing docs | pass/fail | `run_gates_check` | filesystem and mainline loader | bound |
| 14 | `verify_mainline_call_table_bindings` | `xtask/src/main.rs` | validate migrated mainline call-table file and symbol bindings | JSON mainline truth plus source files | pass/fail | `run_gates_check` | filesystem and symbol resolver | bound |

## Sync Status Against Code

- workspace gate orchestration, generated-wiki freshness checks, and wiki generation pipeline are bound in code
- current gate baseline enforces required files, policy docs, generated wiki freshness, migrated mainline manifest cross-links, and migrated mainline call-table bindings
- generated wiki must be regenerated from `docs/mainline-calls/foundation.workspace.json` when this function-map truth changes
