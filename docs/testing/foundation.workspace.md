# Test Design: `foundation.workspace`

- feature_id: `foundation.workspace`
- owner: `xtask`, workspace root
- lifecycle path under test:
  - workspace scaffold exists
  - required architecture docs exist
  - required hooks and CI files exist
  - `make ci` is the canonical full local gate and includes mainline freshness before architecture gates
  - pre-push, CI, and release paths consume the same full gate instead of drifting into partial gate stacks
  - release script runs full regression, Rust release build, Android JVM regression, Android release build, extracts APK version truth, and stages APK plus `update.json` artifacts
  - Android release artifact packaging disables Android release lint checks in Gradle config; release regression truth is `make ci` plus Android JVM tests, not the failing Android Lint Vital task
  - global install script installs release host binaries into the configured prefix and Android update artifacts into runtime-home distribution truth
  - symlink install script builds debug host binaries, exposes S-suffixed development commands, and installs a prefix-local launchd wrapper without replacing global release commands
  - launchd install script installs release host binaries, stages Android update artifacts, writes `~/Library/LaunchAgents/com.freehand.daemon.plist`, writes `~/.freehand/daemon.env` with explicit daemon binary and Android update paths, starts the user service, and exposes fixed logs/WebUI
  - launchd symlink profile writes `~/Library/LaunchAgents/com.freehand.daemonS.plist`, writes `~/.freehand/daemonS.env`, uses `freehand-daemonS-bin` for launchd execution, refreshes that debug binary copy during `restartS`, rewrites the plist, reloads only the service-scoped launchd label so new ProgramArguments/env sourcing are active, health-checks the env-backed bind, and exposes `127.0.0.1:4042` plus separate `daemonS.*.log` files
  - launchd Worker profiles write `com.freehand.worker` / `com.freehand.workerS`,
    source the matching master profile pair token, omit `--bind`, use
    `RunAtLoad=true` and `KeepAlive=true`, and prove the worker PID remains
    stable before reporting startup success
  - launchd install script does not leak daemon workspace root overrides into release/global-install regression subprocesses
  - launchd release and S profiles default `FREEHAND_DAEMON_WORKDIR` to
    `$HOME/.freehand`, create it before service bootstrap, and never default the
    master daemon to the repository root
  - gate command can validate policy locks
  - gate command validates resource-map ownership, operation binding, direct/indirect/forbidden relation, source-edge registry, function-map backlink, and test-design coverage consistency before code refactor
  - source-only search policy keeps implementation search out of generated/runtime outputs and rejects unsafe `rg` ignore-bypass options
  - gate command can reject data/control boundary leaks at the repo source level
  - mainline generation command can render wiki from JSON truth
  - mainline check command rejects stale wiki
  - framework loop governance docs initialize in L1 report-only mode with state, constraints, budget, run log, owner binding, and kill switch path
- white-box plan:
  - xtask rule-check logic
  - mainline JSON parse/render logic
  - generated-wiki freshness logic
  - feature-map duplicate seed-entry detection
  - resource-map parser and consistency checks, including required core resources, unique owner backlinks, operation binding completeness, source-edge registry backlinks, forbidden/direct relation conflict rejection, forbidden direct relations backed by matching indirect relation rules, source shortcut gates, and precise source-edge gates
  - mainline manifest cross-link logic between JSON, feature map, function map, test design, and generated wiki path
  - mainline call-table file and symbol binding logic for migrated `bound` rows
  - CI/CD and local hook command-alignment logic
  - release script prerequisite, APK version extraction, sidecar generation, and artifact path logic
  - WebUI online verification wrapper checks fixed-port health before invoking the real browser verifier
  - Android release packaging config disables Android release lint checks explicitly
  - global install prefix and runtime Android update artifact staging logic
  - symlink install S-suffix command, symlink target logic, and prefix-local launchd wrapper copy
  - launchd daemon binary prefix mismatch rejection
  - launchd S-profile label/env/bin/bind/log/Android-update-artifact separation, including fixed loopback `127.0.0.1:4042` defaults and env-backed health checks on restart
  - launchd `restartS` debug daemon binary refresh, plist refresh, service-scoped launchd reload, and env-file sourcing before health check
  - launchd Worker profile command, shared-token, no-bind, stable-PID, and
    separate env/log path checks
  - launchd release subprocess daemon-workdir env isolation
  - launchd default master workdir is `$HOME/.freehand`
  - source-only search policy checks `.ignore`, `scripts/source-search.sh`, debug docs, local skill snippets, and unsafe-argument rejection
  - data/control boundary leak logic for request-node contracts and metadata-owner uniqueness
  - loop governance docs include required L1 report-only files and deny automated action until explicit L2 approval
- module black-box plan:
  - `xtask gates check` smoke from repo root
  - `xtask mainlines check` smoke from repo root
  - `cargo test -p xtask` manifest-link positive and negative tests
  - `cargo test -p xtask` call-table binding positive and negative tests
  - `cargo test -p xtask` CI/CD command-alignment positive and negative tests
  - `cargo test -p xtask` source-search boundary positive and negative tests, including missing generated-output exclusion and missing unsafe-argument guard
  - `bash -n scripts/release.sh`
  - `bash -n scripts/source-search.sh`
  - `bash -n scripts/verify-webui-online.sh`
  - `node --check scripts/verify-mobile-ui-tree-goal-audit.mjs`
  - `python3 -m py_compile scripts/verify-adp-fixed-session-observability-online.py`
  - `bash -n scripts/install-global.sh`
  - `bash -n scripts/install-symlink.sh`
  - `bash -n scripts/freehand-file-permission-preflight.sh`
  - `bash -n scripts/freehand-daemon-launchd.sh`
  - `bash -n scripts/install-launchd.sh`
  - `bash -n scripts/uninstall-launchd.sh`
  - `cargo test -p xtask` data/control leak-gate positive and negative tests
  - `cargo test -p xtask` feature-map uniqueness positive and negative tests
  - `cargo test -p xtask resource_map_ -- --nocapture` resource-map positive and negative gate tests
  - loop governance doc smoke validates required files are present and owner-bound through `foundation.workspace`
- project black-box impact:
  - full workspace `make ci` gate smoke, including `cargo run -p xtask -- mainlines check`
  - `scripts/release.sh` stages host and Android release artifacts plus `dist/android/update.json` under `dist/`
  - `scripts/install-global.sh` installs `freehand-cli`, `freehand-server`, `freehand-daemon`, and runtime-home Android update artifacts
  - `scripts/install-symlink.sh` installs `freehand-cliS`, `freehand-serverS`, `freehand-daemonS`, and `freehand-daemon-launchdS` as symlinks
  - `scripts/install-launchd.sh` starts `com.freehand.daemon` with `RunAtLoad`, `KeepAlive`, explicit daemon binary path, explicit Android update manifest/APK paths, fixed `127.0.0.1:4041`, and logs under `~/.freehand/logs`
  - `scripts/install-launchd.sh installS` starts `com.freehand.daemonS` without replacing the global service, fixed at `127.0.0.1:4042`, with Android update env paths pointing at runtime-home staged artifacts
  - `scripts/install-launchd.sh restartS` refreshes S debug binaries, stages current repo Android update artifacts when present, rewrites the env-sourcing plist, reloads only `com.freehand.daemonS`, reads the existing env bind for health checks, and restarts only that label
  - `scripts/freehand-file-permission-preflight.sh` records macOS runtime/workdir/protected-folder permission preflight status under `~/.freehand/state/file-permission-preflight.json`; denial opens Full Disk Access settings and fails install/restart unless explicitly run with `FREEHAND_FILE_PERMISSION_PREFLIGHT=warn`
  - `scripts/install-launchd.sh installWorkerS` starts
    `com.freehand.workerS` from the same debug binary and pair token without a
    WebUI bind; `restartWorkerS` preserves the same service contract and copies
    credential-style `FREEHAND_*_KEY`, `FREEHAND_*_CREDENTIAL`, and
    `FREEHAND_*_SECRET` provider env keys from the matching Master profile
  - `make verify-webui-online` runs the fixed S-profile `127.0.0.1:4042` real-browser WebUI + ADP proof after symlink install/restartS, injects the verifier-only provider credential env required for Settings valid-save proof, restores S-profile config/env afterward, and saves screenshots plus `summary.json` under `artifacts/webui-online/`; `make verify-webui-release-online` is the explicit release-profile `127.0.0.1:4041` proof
  - `scripts/verify-adp-fixed-session-observability-online.py --url ws://127.0.0.1:4042/adp --session <fixed-id>` proves fixed-session submit observability with pending/final selected-session turns plus TaskBoard/AgentBoard owner truth, without creating random sessions
  - `node scripts/verify-mobile-ui-tree-goal-audit.mjs` reads the accepted mobile UI tree artifact summaries, checks live S-profile config/env restoration, records ADB lockscreen/device state, and produces a JSON/Markdown pass/block/missing/fail report without creating test sessions
  - machine-readable mainline truth remains the only source for generated wiki artifacts
  - loop governance starts as report-only project control, not unattended automation
- fixtures / replay inputs / runtime evidence paths:
  - repo filesystem layout
- known gaps:
  - no diff-aware gate yet for changed-feature-only optimization
- sync status between design and implementation:
  - baseline aligned with current harness
  - mainline generation and freshness checks are implemented in `xtask`
  - feature-map duplicate seed-entry checks are implemented in `xtask`
  - mainline manifest cross-link checks are implemented in `xtask`
  - mainline call-table binding checks are implemented in `xtask`
  - CI/CD command-alignment checks are implemented in `xtask`
  - source-search boundary checks and unsafe-argument guard checks are implemented in `xtask`
  - release/global-install operator docs live in `docs/release.md`
  - data/control leak gate must stay implemented in `xtask`
  - resource-map gate must stay implemented in `xtask`
  - initial loop governance docs are landed under `docs/loops/freehand-framework-loop`
  - migrated mainline-call source and generated wiki are kept in sync with this test design
