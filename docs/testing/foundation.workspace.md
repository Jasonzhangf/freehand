# Test Design: `foundation.workspace`

- feature_id: `foundation.workspace`
- owner: `xtask`, workspace root
- module_registry: `docs/module-registry/foundation.workspace.json`
- verification_map: `docs/verification-maps/foundation.workspace.json`
- lifecycle path under test:
  - workspace scaffold exists
  - required architecture docs exist
  - required hooks and CI files exist
- `make ci` is the canonical full local gate and includes mainline freshness before architecture gates
- `make dev` and `make pre-push-fast` are manual inner-loop tiers that never replace `make ci`; `make nightly` adds webui online verifiers, and `make release` reruns `make ci` before staging artifacts
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
  - gate command validates generated ADP protocol manifest/constructor artifacts are deterministic against the Rust exporter and consumed by WebUI assets
  - gate command executes the WebUI foundation contract verifier against the same runtime-owned surface registry consumed by bootstrap
  - gate command validates the OpenMinis non-browser UI migration design baseline, including exact required nodes/statuses, owner and map references, Browser/Cookie/Profile/Takeover exclusion, human/machine entrypoint and forward topology parity, advanced-state target bindings, and structured online-state evidence coverage
  - source-only search policy keeps implementation search out of generated/runtime outputs and rejects unsafe `rg` ignore-bypass options
  - gate command can reject data/control boundary leaks at the repo source level
  - mainline generation command can render wiki from JSON truth
  - mainline check command rejects stale wiki
  - launchd control mainline rows are split into adjacent single-symbol bindings; the architecture gate rejects compound `symbol_path` rows for the daemon launchd edges
  - framework loop governance docs initialize in L1 report-only mode with state, constraints, budget, run log, owner binding, and kill switch path
- white-box plan:
  - xtask rule-check logic
  - mainline JSON parse/render logic
  - generated-wiki freshness logic
  - feature-map duplicate seed-entry detection
  - resource-map parser and consistency checks, including required core resources, unique owner backlinks, operation binding completeness, source-edge registry backlinks, forbidden/direct relation conflict rejection, forbidden direct relations backed by matching indirect relation rules, source shortcut gates, and precise source-edge gates
  - ADP protocol artifact gate regenerates JSON and JS outputs through `export-adp-protocol`, diffs committed artifacts, and scans WebUI consumers for generated constructor usage
  - WebUI foundation gate imports the runtime surface registry, validates non-empty typed fields, immutable string arrays, unique surface/DOM-root identities, and rejects empty or key-mismatched registries plus invalid shared-state/action contracts
  - OpenMinis migration manifest loader/value validator checks the lifecycle envelope, owner/touched-feature references, source scope exclusions, required per-node gates, and every lifecycle state's canonical repository-relative function/mainline/test map paths; the three map sets must name the same feature ids, include the owner feature, stay in their unique map directories, and prove each document's path/self/feature identity
  - OpenMinis migration lifecycle validator has a separate contract per status: `owner_mapped` requires owner/resource/operation truth without target symbols; `contract_ready` and `implementation_in_progress` require complete protocol/surface fields; `source_bound` and later require real target symbols; blocked states retain their named pending boundary and cannot masquerade as implemented
  - promoted OpenMinis nodes bind their source resource, target resource, operation owner, bound status, allowed direct relation, and exact non-empty incident `route_edge_ids` set to canonical resource/topology registries; invented endpoints, missing/unrelated route edges, and owner/touched-feature drift fail
  - every node's `touched_feature_ids` set equals the feature ids derived independently from its function/mainline/test map sets; map-only and touched-only features fail
  - aggregate `migration_complete` requires every included node to reach `legacy_retired`; an all-`online_verified` manifest remains incomplete
  - OpenMinis pinned-source validator requires the deterministic `external/OpenMinis` checkout at the exact manifest commit, recursively opens every declared path from that commit, rechecks every resolved blob path against the BrowserUse/Cookie/Profile/Takeover exclusion boundary, resolves every declared source symbol, and parses workflow YAML so every `make ci` job jointly contains the pinned checkout and Swift setup before the full gate; `scripts/provision-openminis-source.sh` reads the canonical manifest commit, creates only a missing sparse checkout from the canonical upstream, and rejects an existing checkout with origin, HEAD, or dirty-state drift
  - OpenMinis migration topology helper parses the human Mermaid tree plus explicit entrypoint/forward-edge/return-path tables, proves every manifest node is forward-reachable from `foundation.root`, and compares entrypoint, edge id/from/to/semantic, and return from/to/semantic sets bidirectionally with the manifest; every table row in the registered section must match its active table schema and malformed/unknown/unreachable rows fail
  - OpenMinis migration target-binding helper recursively selects only supported Rust/Swift/JS/TS/Kotlin declaration sources from broad target directories, resolves every source-bound target symbol, retains its unique declaration file, requires an exact mainline symbol segment whose row `file_path` equals that declaration's canonical repository-relative file on a `binding_status=bound` row carrying the same operation, and requires that resource operation binding itself to be `bound`; Android PNG/XML/JSON assets cannot make source binding fail before Kotlin declarations are reached
  - OpenMinis migration evidence helper requires JSON artifacts whose `node_id`, `gate_id`, exact `command`, `result=passed`, and `online_run_id` exactly match the manifest evidence record; repository evidence uses the non-recursive node verifier rather than the aggregate gate, raw reports bind exact `node_id`/`migration_unit_id`, and incomplete or cross-node gate coverage fails
  - each evidence gate uses one code-locked canonical command and proof kind; repository, WebUI, and Android artifacts must additionally carry their required passed boolean assertions, so copied generic `passed` fields cannot promote a node
  - repository evidence binds an attestation to a distinct raw report under `docs/migrations/openminis-ui/evidence/` by canonical path and SHA-256 and validates its schema, run id, code-locked command/proof/verifier identity, terminal process result, gate-specific assertions, exact attested commit/tree, and worktree drift; WebUI, Android-device, and legacy no-touch reports additionally require an Ed25519 signature from the external runner key pinned in the verifier, while missing/forged signatures, forged commands, generic passed fields, report drift, assertion drift, source drift, or incomplete coverage fail
  - pinned Swift source symbols and promoted target symbols resolve as unique language-parser declaration occurrences: Swift compiler parse AST for Swift, `syn` for Rust, and language tree-sitter ASTs for JS/TS/Kotlin; syntax-error trees, comments, string/regex literals, call sites, longer identifiers, cross-file duplicates, and same-file duplicates all fail, and every workflow full gate must install the Swift parser consumed by this gate
  - OpenMinis legacy-retirement helper rejects absolute, non-canonical, backslash, dot-segment, and repository-escaping scan/evidence/removed paths; it requires one machine `legacy_scan_roots` row in the owning feature's mainline map, exact owner/node/scan-root/removed-identity agreement, directory roots that cover every bound target path and registered removed legacy path, independently proves registered legacy paths are absent, registered legacy symbols/imports/callers do not occur under those owner-bound roots, and requires a dedicated online no-touch artifact with `legacy_touched=false`
  - mainline manifest cross-link logic between JSON, feature map, function map, test design, and generated wiki path
  - mainline call-table file and symbol binding logic for migrated `bound` rows
  - CI/CD and local hook command-alignment logic, including pre-commit clearing outer Git local environment variables before nested pinned-source inspection
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
  - search-evidence conformance manifest, fixture coverage, exact rejection category/path, and repeated provider/domain scenario tests
  - loop governance docs include required L1 report-only files and deny automated action until explicit L2 approval
- module black-box plan:
  - `xtask gates check` smoke from repo root, including ADP generated artifact freshness
  - `node scripts/verify-webui-foundation-contracts.mjs` positive runtime registry/shared-state proof plus built-in negative schema mutations
  - `xtask mainlines check` smoke from repo root
  - `cargo test -p xtask` manifest-link positive and negative tests
  - `cargo test -p xtask` call-table binding positive and negative tests
  - `cargo test -p xtask daemon_launchd_mainline_edges_ -- --nocapture`
  - `cargo test -p xtask` CI/CD command-alignment positive and negative tests
    lock launchd plist wrapper execution, env-file side-channel wiring,
    `KeepAlive.SuccessfulExit=false`, `ThrottleInterval`, label-scoped guard
    state, and guarded child-PID readiness while rejecting the retired inline
    daemon exec path
  - `cargo test -p xtask` source-search boundary positive and negative tests, including missing generated-output exclusion and missing unsafe-argument guard
  - `cargo run -p xtask -- search-schema check`
  - `bash -n scripts/release.sh`
  - `bash -n scripts/source-search.sh`
  - `bash -n scripts/provision-openminis-source.sh`
  - `bash -n scripts/verify-webui-online.sh`
  - `node --check scripts/verify-mobile-ui-tree-goal-audit.mjs`
  - `python3 -m py_compile scripts/verify-adp-fixed-session-observability-online.py`
  - `bash -n scripts/install-global.sh`
  - `bash -n scripts/install-symlink.sh`
  - `bash -n scripts/freehand-file-permission-preflight.sh`
  - `bash -n scripts/freehand-daemon-launchd.sh`
  - `bash -n scripts/install-launchd.sh`
  - `bash -n scripts/uninstall-launchd.sh`
  - `bash scripts/verify-launchd-restart-guard.sh`
  - `bash scripts/verify-launchd-restart-guard-online.sh`
  - `make ci` includes both launchd guard targets; non-Darwin full builds leave
    the real online execution to the required macOS CI/release job rather than
    claiming Linux can host a LaunchAgent
  - `cargo test -p xtask` data/control leak-gate positive and negative tests
  - `cargo test -p xtask` feature-map uniqueness positive and negative tests
  - `cargo test -p xtask resource_map_ -- --nocapture` resource-map positive and negative gate tests
  - `cargo test -p xtask openminis_ui_migration_manifest_ -- --nocapture` locks the current design baseline positively and rejects unbound advanced status, missing machine nodes, excluded Browser symbols, unknown lifecycle status, human/machine edge drift, forged target symbols, and unstructured online evidence
  - loop governance doc smoke validates required files are present and owner-bound through `foundation.workspace`
- project black-box impact:
  - full workspace `make ci` gate smoke, including `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check` invokes `verify_webui_foundation_contracts`, so WebUI foundation schema drift blocks the canonical project gate
  - `scripts/release.sh` stages host and Android release artifacts plus `dist/android/update.json` under `dist/`; the manifest carries the APK signer certificate SHA-256 digest and Relay staging preserves it
  - `scripts/verify-dual-path-update.sh` validates manifest and downloaded APK version, SHA-256, byte size, and signer certificate identity on both explicit endpoints
  - `bash -n apps/freehand-relay-server/deploy/claw-deploy.sh` validates the Claw deployment evidence script before any external deployment
  - `scripts/install-global.sh` installs `freehand-cli`, `freehand-server`, `freehand-daemon`, and runtime-home Android update artifacts
  - `scripts/install-symlink.sh` installs `freehand-cliS`, `freehand-serverS`, `freehand-daemonS`, and `freehand-daemon-launchdS` as symlinks
  - `scripts/install-launchd.sh` starts `com.freehand.daemon` with `RunAtLoad`, `KeepAlive.SuccessfulExit=false`, `ThrottleInterval`, explicit daemon binary path, explicit Android update manifest/APK paths, fixed `127.0.0.1:4041`, logs under `~/.freehand/logs`, and service-control state under `~/.freehand/state/launchd`
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
  - `cargo run -p xtask -- gates check` always executes the OpenMinis UI migration manifest gate before implementation nodes may be promoted
  - loop governance starts as report-only project control, not unattended automation
- fixtures / replay inputs / runtime evidence paths:
  - repo filesystem layout
  - `docs/migrations/openminis-ui/ui-tree.manifest.json`
  - `docs/migrations/openminis-ui/ui-tree.md`
  - focused in-memory manifest mutations and RAII-owned temporary JSON evidence/retirement repositories in `xtask/src/openminis_ui_migration/tests/`; fixture roots are removed when their guard drops
  - pinned OpenMinis source checkout at `external/OpenMinis` and GitHub Actions checkout of `OpenMinis/OpenMinis@9cf3a855fecd27bb5735b84cacbd56852a3ab8dd`
- OpenMinis positive/negative test matrix:
  - positive lifecycle: current design baseline passes; `owner_mapped` without target symbols passes; exact source-bound symbol/row/operation/UI-contract binding passes
  - negative lifecycle: unknown status, contract-ready pending protocol field, fabricated operation projection/query, generated command or surface path, missing/unrelated promoted route edges, source-bound missing target symbol, and blocked state that no longer retains its named pending boundary fail
  - positive/negative pinned source: exact commit/path/symbol inventory, one same-step YAML repository/ref/path checkout, a missing checkout provisioned through canonical-upstream sparse exact-SHA fetch, normal and `.git`-file worktree/submodule checkout roots verified through Git, an existing checkout verified while Freehand hook-style `GIT_DIR`/`GIT_WORK_TREE` values are inherited, an owner-bearing lock reclaimed only after its same-host PID is absent, and two concurrent first-run provisioners converging on one exact clean destination without lock/staging residue pass; a provisioner that does not clear caller Git variables, validate the exact checkout root, carry liveness-checked stale-lock reclamation, or serialize first-run installation with owned cleanup, wrong origin, wrong HEAD, dirty checkout, missing path, missing symbol, directly declared or recursively resolved BrowserUse/Cookie/Profile/Takeover path or symbol, missing Make/pre-commit provisioner wiring, CI drift, or values scattered into a later unrelated step fail
  - positive/negative exact graph and retirement closure: direct and method recursion retain self-edges; callable parameters and local values/`const`/`static` items do not resolve to same-named module functions; imported function aliases and nested-block imports retain their exact targets; ordinary macro arguments contribute parsed direct calls while opaque call-like macro bodies fail closed; migration-owned callers force external callee edges without registry self-authorization; migration-owned callees retain legitimate inbound callers while external callees retain only migration-owned direct callers and outside-to-outside helper edges are excluded; chained potentially local method calls cannot disappear; callable-local imports, typed closure parameters and typed closure container item receivers, inline `Result<Local, E>` / `Option<Local>` `unwrap`/`expect` receiver chains, repository-local items iterated through supported built-in containers, outer receivers restored after inner shadowing, shadowing initializers evaluated under the outer scope, every destructured `let`/closure/loop/match-arm/direct-or-chained-if-let/while-let binding replacing same-named outer receiver and iterable-item bindings, named struct-pattern fields retaining their field types instead of the container type, loop binders clearing/restoring iterable metadata, inherited trait-default dispatch, standard smart-pointer autoderef receivers, and grouped self imports retain exact local edges; built-in signature receivers, external method-chain bindings, and external iterator items remain external without suppressing unknown local-shadow failures; unresolved identifier receivers that could call repository-local methods fail closed in production and test callables; external and nested external `#[cfg(test)]` module files inherit test identity independent of file traversal order, nested cfg predicates that require `test` retain test ownership, and disjunctive predicates active without `test` remain production; control-flow-local JS declarations are rejected; owner scan roots containing binary Android assets still scan text identities and can retire
  - positive/negative topology: exact human/machine entrypoint, forward edges, returns, promoted-node incident route sets, and entrypoint reachability pass; entrypoint drift, edge drift, missing/unrelated promoted route ids, unreachable node, return missing/extra/semantic drift, duplicate row, and unknown endpoint fail
  - positive/negative binding: exact canonical repository-relative target/mainline path, a broad Android target directory containing supported Kotlin declarations plus non-source assets, exact symbol segment whose row names the declaration's exact file, and bound resource operation pass; a row pointing to another valid target file, absolute/non-canonical/repository-escaping or symlinked target/mainline path, forged target symbol, substring-only symbol, pending mainline row, wrong operation, and pending resource operation fail
  - positive/negative call graph: the `syn`-parsed Rust module/import graph automatically derives the complete multi-reference production function/method/initializer set, receiver/trait-qualified definitions, attribute/ancestry-derived test ownership, exact module-qualified direct caller/direct-test sets, and every real module-qualified production caller-to-callee edge including recursion and parenthesized/grouped direct callees without a callee-name whitelist; module, associated, discriminant, and block-local const/static initializer calls retain deterministic owner-qualified caller identities without duplicate outer-callable attribution, associated initializer `Self` calls retain their impl/trait owner, block-local initializers retain callable scope, callable-local types shadow same-named module types, and inactive cfg initializers fail closed; the positive method fixture covers inherent methods, trait defaults including an implementation that inherits rather than overrides the default, trait impl methods, `self.method()`, `Self::associated()`, `Type::associated()`, callable-local `Local::associated()`, callable-local `use`, statically typed trait receivers, imported receiver types, parsed local `Deref::Target`, supported container and typed-closure item receivers, inline `Result`/`Option` unwrap/expect receiver inference, module re-exports through qualified paths, reassigned local receiver truth, and impl/trait methods inherited from inline or external ancestor test modules; active impl/trait declarations nested below the callable body fail closed while cfg-disabled nested declarations stay absent; an unrelated external-trait method remains external without a false deref-target edge; graph-boundary fixtures retain migration-to-external-helper and external-to-migration edges while rejecting unrelated outside-to-outside helper callers; receiver-scope fixtures prove callable-value shadowing, untyped closure, closure-iterable, destructured `let`, unresolved loop, match-arm, direct/chained if-let, and while-let binders cannot reuse outer types and a `let` initializer still sees its outer binding; top-level `#[test]`, ancestor or external `#[cfg(test)]`, and `#[cfg(all(test, unix))]` are tests, while `#[cfg(any(test, unix))]` remains production on Unix and a production module merely named `tests` is not a test; unresolved local methods in either test or production code without an external-trait owner, ambiguous method dispatch, a hand-selected omission, removed/extra/prose/grouped caller, test drift, bare-name collision, or call-edge drift fails
  - call-graph cfg admission discovers production and test projections independently, processes only file modules reachable through declarations active in that projection, then keeps all production truth plus definitions and callers that exist only in the test projection; cfg-exclusive external modules, same-name imports, same-name definitions, and block-level imports/const/static bindings therefore never share or pollute one false scope. `cfg(not(test))` remains valid production truth, each callable traverses statements, nested expressions, match arms, block items, and enum discriminants under its actual projection, disabled subtrees contribute no edges, and active match arms/discriminants remain visible under deterministic initializer caller identities. Every function-local item is cfg-filtered before entering callable imports, type, field, impl, trait, or method indexes; local impl/trait method identities remain callable-qualified while unqualified calls in their bodies resolve against the real enclosing module. A cfg-gated function/module/method/enum inactive in both projections or using unsupported cfg fails instead of satisfying an active binding; nested functions fail explicitly, aliases plus module/callable/nested lexical glob imports resolve single- and multi-segment paths before direct-edge recording while multiple real candidates fail as ambiguous, imported receiver types resolve through local use aliases before method-edge recording and imported `impl` receiver indexing, qualified UFCS calls fail closed until explicitly modeled, local macro definitions and `include!` fail closed because unexpanded code can hide direct edges, and lexical impl/trait bodies receive independent method identities instead of being attributed to the outer caller
  - positive/negative evidence: complete matching repository-gate and externally signed online-gate JSON artifacts, non-recursive node-verifier commands, exact raw-report node/migration-unit/verifier/proof identity, full gate coverage, required gate-specific assertions, and a canonical manifest transition limited to top-level status plus per-node status/evidence/legacy-retirement after the attested source revision pass; missing/forged online provenance signatures, operation/source/target/map contract mutation in that same manifest, source/registry/verifier drift, aggregate-gate self-certification, cross-node report reuse, forged online command/proof/assertion/result, missing node/run id, non-JSON artifact, command/result/node/gate/run mismatch, failed result, and incomplete coverage fail
  - positive retirement structure: one exact `legacy_scan_roots` row in the owner feature mainline, exact manifest repetition of owner-registered removed identities, canonical repository-relative directory roots covering bound target and removed paths, physically absent registered legacy identities, and structurally matching no-touch evidence pass; aggregate lifecycle admission remains red until external no-touch provenance is source-bound
  - negative retirement: a fabricated/unregistered absent identity, arbitrary in-repository empty scan directory, missing/duplicate/wrong-owner/noncanonical-mainline registry row, registry/manifest scan-root or removed-identity drift, root that does not cover the bound target or removed path, absolute/non-canonical/repository-escaping path, scan-root or nested symlink, existing or broken-symlink legacy path, remaining symbol/import/caller, missing dedicated gate/evidence, or `legacy_touched=true` fails
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
  - ADP generated artifact freshness gate must stay implemented in `xtask`
  - OpenMinis migration lifecycle, pinned-source/CI, exact topology registry, exact target/mainline/resource-operation binding, exact Rust caller/test graph, JSON evidence identity, independent legacy-retirement, and focused positive/negative gates are implemented in `xtask` and bound through the foundation function/mainline maps
  - initial loop governance docs are landed under `docs/loops/freehand-framework-loop`
  - migrated mainline-call source and generated wiki are kept in sync with this test design
