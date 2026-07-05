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
  - `verify_ci_cd_gate_commands`
  - `verify_data_control_boundaries`
  - `verify_feature_map_unique_entries`
  - `docs/loops/freehand-framework-loop/LOOP.md`

## Request Mainline

- repo root invokes `xtask gates check`
- repo root may invoke `xtask mainlines generate`
- repo root may invoke `xtask mainlines check`
- gate runner verifies required files, workspace members, and policy doc snippets
- gate runner verifies migrated mainline JSON cross-links back to feature map, function map, test design, and generated wiki path
- gate runner verifies feature-map seed entries stay unique per `feature_id`
- gate runner verifies migrated mainline call-table `bound` rows still point to existing files and discoverable source symbols
- gate runner verifies `make ci`, pre-push, CI, and release paths include the canonical full gate with mainline freshness
- gate runner verifies source-only search policy so implementation searches use source code, tests, maintained scripts, and canonical docs rather than generated/runtime output; the wrapper rejects unsafe `rg` options such as `--no-ignore` / `-u` and keeps hard generated-output exclusions after caller args
- release script runs full regression, release binary builds, Android JVM regression, Android release APK packaging with Android release lint disabled in Gradle config, and deterministic artifact staging
- WebUI online verification wrapper checks fixed S-profile `127.0.0.1:4042` daemon health and invokes the real browser WebUI + ADP proof for alpha promotion; release-profile `127.0.0.1:4041` proof is a separate explicit target
- global install script installs the staged host binaries into one explicit prefix without inventing runtime config truth
- symlink install script builds debug host binaries, exposes `freehand-cliS`, `freehand-serverS`, and `freehand-daemonS` as symlinks, and installs a copied `freehand-daemon-launchdS` wrapper for development without replacing global release commands
- launchd install script installs host binaries, writes the LaunchAgent plist, writes daemon environment truth with explicit daemon binary path, starts the user service, and exposes fixed WebUI/log paths
- launchd install script supports a coexisting symlink profile through `installS` / `restartS`, using `com.freehand.daemonS`, `~/.freehand/daemonS.env`, `127.0.0.1:4042`, and `daemonS.*.log`; `restartS` refreshes the debug daemon binary copy before service kickstart so launchd does not run stale code
- launchd install script keeps daemon workspace env out of the release/global-install regression subprocess so daemon runtime path selection cannot pollute workspace tests
- gate runner verifies static data/control boundary rules on source-owned request and metadata types
- mainline generator loads machine-readable feature sources from `docs/mainline-calls/*.json`
- generated wiki writer materializes `docs/wiki/*.md` and `docs/wiki/README.md` from the JSON truth
- framework loop governance starts in `L1 report-only` under `docs/loops/freehand-framework-loop` and must not automate code/config changes until signal quality and checker gates are proven

## Response Mainline

- gate returns success when required repo truth and workspace structure are present
- gate returns success when migrated mainline manifests are deterministic and cross-linked to their owner docs
- gate returns success when feature-map seed entries stay unique and owner routing has one seed entry per `feature_id`
- gate returns success when migrated mainline call-table bindings resolve to source files and symbols
- gate returns success when local and remote automation routes through the same full gate stack
- gate returns success when `.ignore`, `scripts/source-search.sh`, debug docs, and local skill keep generated/runtime output excluded from implementation search, including wrapper-level rejection of unsafe `rg` ignore bypass options
- release artifacts include `freehand-cli`, `freehand-server`, `freehand-daemon`, and the Android release APK under `dist/`
- S-profile WebUI online verification writes screenshots and `summary.json` under `artifacts/webui-online/<run-id>/`, proving composer clear, visible submitted input, multi-round failed-tool continuation, no stale historical animation, refresh persistence, and ADP session truth alignment
- global install exposes `freehand-cli`, `freehand-server`, and `freehand-daemon` on the chosen install prefix
- symlink install exposes `freehand-cliS`, `freehand-serverS`, `freehand-daemonS`, and `freehand-daemon-launchdS` on the chosen prefix, pointing host commands at repo debug binaries while keeping the launchd wrapper executable as a prefix-local file
- launchd install exposes `com.freehand.daemon` as a user LaunchAgent with `RunAtLoad`, `KeepAlive`, explicit `FREEHAND_DAEMON_BIN`, fixed `127.0.0.1:4041` WebUI, and logs under `~/.freehand/logs`
- launchd symlink install exposes `com.freehand.daemonS` as a separate user LaunchAgent with explicit `FREEHAND_DAEMON_BIN`, fixed `127.0.0.1:4042` WebUI, and separate `daemonS.*.log` files; symlink profile restart refreshes the debug daemon binary copy before restarting
- release/global-install regressions run without inherited daemon workspace root overrides while the final LaunchAgent still receives the configured daemon workdir
- gate returns success when request-node contracts remain free of metadata/debug/control types and metadata owner types remain free of request/control payload fields
- gate returns explicit failure with missing path or missing policy snippet
- mainline generation returns fresh wiki artifacts derived from machine-readable source
- mainline freshness check returns explicit failure when any generated wiki is stale against current JSON truth
- loop initialization returns an inspectable report-only control surface with purpose, state, constraints, budget, run log, kill switch path, and owner binding

## Error Mainline

- missing file or missing required snippet surfaces as gate failure
- mismatched mainline manifest path, generated wiki path, function map, test design, or feature map link surfaces as gate failure
- missing source file or missing source symbol in a migrated `bound` call-table row surfaces as gate failure
- duplicate feature-map seed entries for one `feature_id` surface as gate failure
- missing `mainlines check` in `make ci` or CI/CD full-gate wiring surfaces as gate failure
- missing source-only search exclusions, missing unsafe-argument guards, or missing `scripts/source-search.sh` policy snippets surface as gate failure
- missing release prerequisites such as Java or Cargo surface as script failure before artifacts are claimed
- missing fixed-port daemon health, Chrome/CDP availability, WebUI browser failures, or ADP mismatch surface as online verification failure before alpha success is claimed
- launchd bootstrap or kickstart failure surfaces as script failure before background service success is claimed
- mismatched launchd daemon binary prefix surfaces as install script failure before service success is claimed
- symlink install failure surfaces before launchd symlink service success is claimed
- request-node structs that introduce metadata/debug/cache/control payload fields or types surface as gate failure
- ad hoc metadata owner types outside `freehand-metadata` or metadata owner structs that introduce request or control payload fields surface as gate failure
- invalid JSON mainline source surfaces as generation/check failure
- stale generated wiki surfaces as explicit freshness failure
- no fallback path exists
- loop overreach, missing owner mapping, active kill switch, or exhausted budget surfaces as escalation/no-op instead of automated action

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
| 15 | `verify_ci_cd_gate_commands` | `xtask/src/main.rs` | validate local hook, Makefile, CI, and release full-gate command alignment | automation config files | pass/fail | `run_gates_check` | filesystem and policy snippets | bound |
| 16 | `verify_source_search_policy` | `xtask/src/main.rs` | validate source-only implementation search boundaries, including unsafe `rg` option rejection | ignore file, source search script, debug docs, and local skill | pass/fail | `run_gates_check` | filesystem and policy snippets | bound |
| 17 | `verify_data_control_boundaries` | `xtask/src/main.rs` | validate static data/control isolation rules on source-owned request and metadata types | Rust source files for contracts and metadata owners | pass/fail | `run_gates_check` | source scanners | bound |
| 18 | `verify_feature_map_unique_entries` | `xtask/src/main.rs` | validate that `docs/architecture/feature-map.md` keeps one seed entry per `feature_id` | feature-map markdown | pass/fail | `run_gates_check` | feature-map scanner | bound |
| 19 | `run_release` | `scripts/release.sh` | run release regressions and build/stage host + Android artifacts | repo root state | `dist/` artifacts | operator / GitHub release workflow | `make ci`, Cargo, Gradle | bound |
| 20 | `run_verify_webui_online` | `scripts/verify-webui-online.sh` | run fixed-port S-profile real browser WebUI + ADP alpha proof | running S-profile daemon on `127.0.0.1:4042` | screenshots, summary JSON, ADP session alignment | operator / `make verify-webui-online` | curl, `scripts/webui_verify_online.mjs`, Chrome CDP, WebUI, `freehand-cliS adp-session-query` | bound |
| 21 | `run_install_global` | `scripts/install-global.sh` | run release script and install host binaries to a global prefix | release artifacts | installed commands | operator | `scripts/release.sh`, install tool | bound |
| 22 | `run_install_launchd` | `scripts/install-launchd.sh` | install global binaries or refresh S debug binaries and bootstrap/restart the macOS user LaunchAgent | repo root + runtime env | running launchd service | operator | `scripts/install-global.sh`, launchctl | bound |
| 23 | `run_uninstall_launchd` | `scripts/uninstall-launchd.sh` | stop and remove the macOS user LaunchAgent plist | launchd label | service removed | operator | launchctl | bound |
| 24 | `run_install_symlink` | `scripts/install-symlink.sh` | build debug host binaries and expose S-suffixed symlinks for development | repo root state | installed symlink commands | operator | Cargo, symlink creation | bound |

## Sync Status Against Code

- workspace gate orchestration, generated-wiki freshness checks, and wiki generation pipeline are bound in code
- current gate baseline enforces required files, policy docs, generated wiki freshness, feature-map seed-entry uniqueness, migrated mainline manifest cross-links, migrated mainline call-table bindings, CI/CD full-gate command alignment, source-search boundaries including unsafe-argument rejection, and static data/control boundary checks
- release and global-install scripts are documented in `docs/release.md`
- alpha WebUI online verification defaults to S-profile 4042 and is documented in `docs/release.md`; release 4041 proof is explicit via `make verify-webui-release-online`
- initial framework loop governance docs are bound under `docs/loops/freehand-framework-loop` in L1 report-only mode
- generated wiki must be regenerated from `docs/mainline-calls/foundation.workspace.json` when this function-map truth changes
