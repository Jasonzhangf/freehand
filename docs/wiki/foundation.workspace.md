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
- gate runner verifies feature-map seed entries stay unique per feature_id
- gate runner verifies migrated mainline manifest cross-links between JSON truth, feature map, function map, test design, and generated wiki path
- gate runner verifies migrated mainline call-table `bound` rows still point to existing files and discoverable source symbols
- gate runner verifies `make ci`, pre-push, CI, and release paths include the canonical full gate with mainline freshness
- release script runs full regression, release binary builds, Android JVM regression, Android release APK packaging with Android release lint disabled in Gradle config, and deterministic artifact staging
- WebUI online verification wrapper checks fixed S-profile 127.0.0.1:4042 daemon health and invokes the real browser WebUI plus ADP proof for alpha promotion; release-profile 127.0.0.1:4041 proof is a separate explicit target
- global install script installs the staged host binaries into one explicit prefix without inventing runtime config truth
- symlink install script builds debug host binaries, exposes S-suffixed development commands, and installs a prefix-local launchd wrapper without replacing global release commands
- launchd install script installs host binaries, writes the LaunchAgent plist, writes daemon environment truth with explicit daemon binary path, starts the user service, and exposes fixed WebUI/log paths
- launchd install script supports a coexisting symlink profile through installS and restartS with a separate label, env file, bind, binary, and logs; restartS refreshes the debug daemon binary copy before service kickstart so launchd does not run stale code
- gate runner verifies static data/control boundary rules on source-owned request and metadata types
- mainline generator loads machine-readable feature sources from `docs/mainline-calls/*.json`
- framework loop governance starts in L1 report-only under docs/loops/freehand-framework-loop and must not automate code/config changes until signal quality and checker gates are proven

## Response Mainline

- gate returns success when required repo truth, workspace structure, and generated wiki freshness are present
- gate returns success when feature-map seed entries stay unique and owner routing has one seed entry per feature_id
- gate returns success when migrated mainline manifests are deterministically linked to their owner docs
- gate returns success when migrated mainline call-table bindings resolve to source files and source symbols
- gate returns success when local and remote automation routes through the same full gate stack
- S-profile WebUI online verification writes screenshots and summary.json under artifacts/webui-online/<run-id>/, proving composer clear, visible submitted input, multi-round failed-tool continuation, no stale historical animation, refresh persistence, and ADP session truth alignment
- global install exposes freehand-cli, freehand-server, freehand-daemon, and freehand-daemon-launchd on the chosen install prefix
- symlink install exposes freehand-cliS, freehand-serverS, freehand-daemonS, and freehand-daemon-launchdS on the chosen install prefix, with host commands pointing at repo debug binaries and the launchd wrapper installed as a prefix-local file
- launchd install exposes com.freehand.daemon as a user LaunchAgent with RunAtLoad, KeepAlive, explicit FREEHAND_DAEMON_BIN, fixed 127.0.0.1:4041 WebUI, and logs under ~/.freehand/logs
- launchd symlink install exposes com.freehand.daemonS as a separate user LaunchAgent with explicit FREEHAND_DAEMON_BIN, fixed 127.0.0.1:4042 WebUI, and daemonS logs under ~/.freehand/logs; symlink profile restart refreshes the debug daemon binary copy before restarting
- gate returns success when request-node contracts remain free of metadata/debug/control types and metadata owner types remain free of request/control payload fields
- gate returns explicit failure with missing path, missing policy snippet, or stale generated wiki
- mainline generation writes `docs/wiki/*.md` and `docs/wiki/README.md` from JSON truth
- mainline freshness check returns success only when current generated wiki matches current JSON source
- loop initialization returns an inspectable report-only control surface with purpose, state, constraints, budget, run log, kill switch path, and owner binding

## Error Mainline

- missing file or missing required snippet surfaces as gate failure
- duplicate feature-map seed entries for one feature_id surface as gate failure
- mismatched mainline manifest source path, function map path, test design path, generated wiki path, or feature-map link surfaces as gate failure
- missing source file or missing source symbol in a migrated `bound` call-table row surfaces as gate failure
- missing `mainlines check` in `make ci` or CI/CD full-gate wiring surfaces as gate failure
- missing fixed-port daemon health, Chrome/CDP availability, WebUI browser failures, or ADP mismatch surfaces as online verification failure before alpha success is claimed
- launchd bootstrap, kickstart, or daemon binary prefix mismatch failure surfaces as script failure before background service success is claimed
- symlink install failure surfaces before launchd symlink service success is claimed
- request-node structs that introduce metadata/debug/cache/control payload fields or types surface as gate failure
- ad hoc metadata owner types outside `freehand-metadata` or metadata owner structs that introduce request or control payload fields surface as gate failure
- invalid JSON mainline source surfaces as generation/check failure
- stale generated wiki surfaces as explicit freshness failure
- no fallback path exists
- loop overreach, missing owner mapping, active kill switch, or exhausted budget surfaces as escalation/no-op instead of automated action

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
| 16 | `verify_data_control_boundaries` | `xtask/src/main.rs` | validate static data/control isolation rules on source-owned request and metadata types | Rust source files for contracts and metadata owners | pass/fail | run_gates_check | source scanners | bound |
| 17 | `verify_feature_map_unique_entries` | `xtask/src/main.rs` | validate that docs/architecture/feature-map.md keeps one seed entry per feature_id | feature-map markdown | pass/fail | run_gates_check | feature-map scanner | bound |
| 18 | `run_release` | `scripts/release.sh` | run release regressions and build/stage host + Android artifacts | repo root state | dist/ artifacts | operator / GitHub release workflow | make ci, Cargo, Gradle | bound |
| 19 | `run_verify_webui_online` | `scripts/verify-webui-online.sh` | run fixed-port S-profile real browser WebUI plus ADP alpha proof | running S-profile daemon on 127.0.0.1:4042 | screenshots, summary JSON, ADP session alignment | operator / make verify-webui-online | curl, scripts/webui_verify_online.mjs, Chrome CDP, WebUI, freehand-cliS adp-session-query | bound |
| 20 | `run_install_global` | `scripts/install-global.sh` | run release script and install host binaries to a global prefix | release artifacts | installed commands | operator | scripts/release.sh, install tool | bound |
| 21 | `run_install_launchd` | `scripts/install-launchd.sh` | install global binaries or refresh S debug binaries and bootstrap/restart the macOS user LaunchAgent | repo root plus runtime env | running launchd service | operator | scripts/install-global.sh, launchctl | bound |
| 22 | `run_uninstall_launchd` | `scripts/uninstall-launchd.sh` | stop and remove the macOS user LaunchAgent plist | launchd label | service removed | operator | launchctl | bound |
| 23 | `run_install_symlink` | `scripts/install-symlink.sh` | build debug host binaries and expose S-suffixed symlinks for development | repo root state | installed symlink commands | operator | Cargo, symlink creation | bound |

## Sync Status Against Mainline Call

- workspace gate orchestration, generated-wiki freshness checks, and wiki generation pipeline are bound in code
- current gate baseline enforces required files, policy docs, generated wiki freshness, feature-map seed-entry uniqueness, migrated mainline manifest cross-links, migrated mainline call-table bindings, CI/CD full-gate command alignment, and static data/control boundary checks
- generated wiki must be regenerated from `docs/mainline-calls/foundation.workspace.json` when this function-map truth changes
- initial framework loop governance docs are bound under docs/loops/freehand-framework-loop in L1 report-only mode
- alpha WebUI online verification defaults to S-profile 4042 and is documented in docs/release.md; release 4041 proof is explicit via make verify-webui-release-online
