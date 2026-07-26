# note.md

# 2026-07-25 Android daemon config legacy compatibility

- owner resources: `android_apk_update`, `DaemonConnectionConfigStore`, compatibility projection for older APKs.
- implementation:
  - `daemon-connection.json` now stays as a legacy-compatible connection projection.
  - new `daemon-connection-registry.json` sidecar stores remote_registry truth for current APKs.
  - bootstrap import and remote_registry loads rewrite the legacy file so older APKs can still read a direct host/port projection.
  - `DaemonConnectionConfigTest` now covers sidecar/legacy sync and load precedence.
- verification:
  - `cd apps/freehand-android && JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest --tests com.freehand.android.data.DaemonConnectionConfigTest --tests com.freehand.android.data.HostConfigTest --tests com.freehand.android.data.ApkUpdateManifestTest --no-daemon`

# 2026-07-24 image attachments and Android turn-finish notifications

- owner resources: `input_attachment`, `ui_projection`, `android_notification`, provider adapter image wire rendering.
- implementation:
  - `SubmitUserInput.metadata.attachments` carries current-send image bytes; session/turn history persists only attachment id/kind/name/media type/size metadata.
  - Runtime maps UI image metadata into provider-neutral `ProviderInputAttachment` only for the active submit and keeps continuation rounds/history free of raw base64.
  - OpenAI Responses/Chat Completions and Anthropic Messages adapters render provider-owned image wire without leaking attachment id/name metadata.
  - WebUI supports multi-image selected pool, thumbnail preview, delete, metadata-only restored display, fixed-session test hooks, and Android `turnFinished` bridge on live nonterminal-to-terminal transitions only.
  - Android requests `POST_NOTIFICATIONS` at startup where required, creates a turn-finished channel, posts a tappable notification through `FreehandAndroidNotifications`, and logs `FreehandNotification` permission/post/dedupe truth. `verify-device-ui.sh` now captures notification logcat and `dumpsys notification` artifacts.
  - Resource map, function maps, test designs, mainline manifests, and generated wiki were synced.
- online proof:
  - `scripts/install-launchd.sh restartS` rebuilt and service-scoped restarted S daemon/relay.
  - `curl -fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` -> `provider=minimax`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search_effective=hosted_declared`, `auth_source=inline`.
  - `node scripts/verify-webui-image-attachment-online.mjs` passed repeatedly; latest artifact `artifacts/webui-online/image-attachment-notification-2026-07-24T053116863Z/summary.json`, fixed session `webui-image-attachment-proof-fixed`, selected pool/preview/remove/submit metadata/history metadata-only/one notification/no historical terminal notify all true.
  - fixture env grep for provider retry/autonomy keys returned no matches.
- local proof:
  - `cargo test -p freehand-ui-protocol image -- --nocapture` passed 2/2.
  - `cargo test -p freehand-provider-openai image_input -- --nocapture` passed 2/2.
  - `cargo test -p freehand-provider-anthropic renders_messages_image_input_as_base64_source -- --nocapture` passed 1/1.
  - `cargo test -p freehand-runtime live_bridge_sends_image_payload_once_and_persists_metadata_only -- --nocapture` passed 1/1.
  - `cargo test -p freehand-server --lib android -- --nocapture` passed 6/6.
  - `cargo test -p freehand-cli --no-run`, `cargo test -p freehand-daemon --no-run`, and `cargo test -p freehand-server --lib --no-run` passed.
  - `cd apps/freehand-android && JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest assembleDebug --no-daemon` passed; debug APK path `apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk`.
  - `node --check apps/freehand-server/assets/webui.js`, `node --check scripts/verify-webui-image-attachment-online.mjs`, `bash -n apps/freehand-android/scripts/verify-device-ui.sh`, `jq empty ...`, `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- true-device gap:
  - `adb pair 100.107.194.67:33039 579786` failed with `protocol fault`; `adb connect 100.107.194.67:45099` returned `Connection refused`; `adb devices -l` was empty.
  - Android install, runtime permission dialog, real system notification popup, tap-return, and `dumpsys notification` posted proof remain unclosed until a reachable/unlocked ADB device is available.

# 2026-07-11 master/worker path and symlink contract closeout

- user report: Master and Worker both mishandled external paths and symlinked paths; Master searched outside its responsibility before delegating, and Worker did not show enough requested/canonical path evidence.
- owner: `runtime.master-worker-loop`.
- implementation:
  - strengthened Master task orchestration guidance for external paths, `~`, symlinks, target_cwd preservation, and no invented `/workspace`/`/tmp`/sibling output dirs.
  - strengthened Worker execution guidance for path preflight, symlink evidence, and blocked missing-path reporting.
  - Worker runner expands leading `~`, canonicalizes the locked workspace through symlinks, preserves requested `target_cwd`, and injects requested/canonical path preflight into the Worker prompt.
  - added Worker runner tests for `~/...` symlink success and missing `~/...` blocked-before-model behavior.
  - updated function map, test design, mainline manifest, and generated wiki.
- local verification:
  - `cargo test -p freehand-runtime production_worker_runner_ -- --nocapture` -> 11 passed.
  - `cargo fmt --check`.
  - `cargo clippy -p freehand-runtime --all-targets -- -D warnings`.
  - `cargo run -p xtask -- mainlines generate`.
  - `cargo run -p xtask -- mainlines check`.
  - `cargo run -p xtask -- gates check`.
  - `git diff --check`.
- online verification:
  - S-profile health `http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` -> master/worker, minimax, MiniMax-M3.
  - task `task-verify-path-1783740443` history via `freehand-cliS adp-task-query --history` -> 49 events ending `TaskReviewSubmitted,TaskReviewApproved,TaskClosed`.
  - Worker evidence preserved requested `/Users/fanzhang/github/xiaozhi-esp32-2.2.4`, canonical `/Users/fanzhang/Documents/github/xiaozhi-esp32-2.2.4`, and `symlink_detected=true`.
  - Worker metadata showed 9 rounds, 22 tool executions, one provider retry, one failed tool result re-entered to model, then terminal success.
- conclusion:
  - path/symlink contract is fixed and online closed for this live sample.
  - long `running + heartbeat` was a long Worker execution with periodic heartbeat until review submission, not lifecycle deadlock.

# 2026-07-09 master-worker autonomy online closeout

- user requirement: prove master model/tool-loop autonomy, not command-driven ADP task mutation. The proof must cover worker success, execution error, and rejected-review retry/close.
- owner: `app.cli-runtime-smoke` for CLI/headless sample and online verifier; consumed runtime owner proof remains `provider.reason-live-bridge`; task truth is `task.orchestration` plus `agent.lifecycle`.
- implementation:
  - added `freehand-cli master-worker-autonomy-sample --url ... --scenario <all|success|execution-error|reject-retry>`.
  - create mode submits only `UiCommand::SubmitUserInput`; it then queries transcript, TaskBoard, AgentBoard, AgentLifecycle, and TaskHistory to verify model/tool-created truth.
  - verify mode re-queries the same task/execution/agent ids after restart.
  - CLI mock ADP test rejects any direct task mutation command with `direct_task_mutation_forbidden`, so the sample cannot pass by scripting task commands.
  - added `scripts/verify-master-worker-autonomy-online.sh`, which temporarily points S-profile provider config at a local Anthropic-compatible fixture. The fixture dynamically parses actual `FHMA_*` ids from provider requests and returns scenario-specific `task` tool_use sequences.
  - updated `docs/architecture/feature-map.md`, `docs/function-maps/app.cli-runtime-smoke.md`, `docs/testing/app.cli-runtime-smoke.md`, `docs/mainline-calls/app.cli-runtime-smoke.json`, and generated `docs/wiki/app.cli-runtime-smoke.md`.
- verified so far:
  - `cargo check -p freehand-cli`
  - `cargo test -p freehand-cli master_worker_autonomy -- --nocapture` -> 2 passed
  - `cargo test -p freehand-cli -- --nocapture` -> 26 passed
  - `cargo test -p freehand-runtime live_bridge_master_autonomy -- --nocapture` -> 3 passed
  - `cargo fmt --check`
  - `bash -n scripts/verify-master-worker-autonomy-online.sh`
  - `jq empty docs/mainline-calls/app.cli-runtime-smoke.json`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - S-profile `scripts/install-launchd.sh restartS`; 4042 health `ok`; `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
  - `scripts/verify-master-worker-autonomy-online.sh` passed with `mock_attempts=27`.
  - online scenario evidence:
    - success: task `task-cli-master-autonomy-success-FHAUTO1783599325364293000`, status `closed`, lifecycle `closed`, `tool_executions=8`, `review_submissions=1`.
    - execution-error: task `task-cli-master-autonomy-execution-error-FHAUTO1783599326069343000`, status `blocked`, lifecycle `blocked`, `tool_executions=6`, `review_submissions=0`.
    - reject-retry: task `task-cli-master-autonomy-reject-retry-FHAUTO1783599326552699000`, status `closed`, lifecycle `closed`, `tool_executions=10`, `review_submissions=2`, ordered events include `TaskReviewRejected` then `TaskExecutionRecovering`.
  - after `restartS`, same-id verify passed for all three scenarios.
  - post-proof config restored: `base_url_host=api.minimaxi.com default_model=MiniMax-M3 auth_source=inline`; `daemonS.env` has no `FREEHAND_MASTER_AUTONOMY_FIXTURE_KEY`.
- remaining before commit:
  - run full local closeout: workspace build, workspace clippy, final mainlines/gates/diff check.
  - mine updated memory after `MEMORY.md` and skill updates.

# 2026-07-07 Android legacy banner removal and device verifier repair

- user report: Android/WebUI surface still showed old native notification banners such as `Freehand APK is up to date`; these overlays are noisy and must not appear on the conversation surface.
- owner: `app.android-client`.
- implementation:
  - `StatusBannerController` is now scoped to blocking native-shell connection/configuration problems only.
  - APK update status and file picker status are routed into the drawer status area, not the top overlay banner.
  - remote WebUI load hides the native banner so Android does not overlay legacy chrome on the shared WebUI conversation surface.
  - `verify-device-ui.sh` now waits for installed package availability before launching, requires current resumed/focused activity to be Freehand, and backs out of a system picker before relaunching Freehand for WebUI layout validation.
- root-cause details:
  - previous true-device verifier defaulted to debug APK unless `FREEHAND_ANDROID_APK` was explicitly set, so release validation could be overwritten by debug install.
  - previous foreground check matched any historical Freehand task/window mention in dumpsys; it could then report `missing_webui_layout_probe` while another app was actually focused.
  - Android package replacement can race Activity start; logcat showed `Invalid packageName: com.freehand.android` before package availability stabilized.
- verified:
  - `bash -n apps/freehand-android/scripts/verify-device-ui.sh`
  - `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew testDebugUnitTest`
  - `scripts/install-global.sh`
  - `scripts/install-launchd.sh restart`
  - `curl -fsS http://100.66.1.82:4041/health`
  - `~/.local/bin/freehand-cli adp-smoke --url ws://100.66.1.82:4041/adp`
  - `FREEHAND_ANDROID_APK=/Users/fanzhang/Documents/code/freehand/dist/android/freehand-android-release-unsigned.apk FREEHAND_ANDROID_SETTLE_SECONDS=18 apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555`
  - final true-device evidence: `artifacts/android-device/20260707T115207Z-100.104.163.65_5555-61741/summary.json`; screenshot shows Freehand WebUI foreground with no old native top notification; layout probe reports `shape=tall_phone`, `conversationPrimary=true`, both drawers fixed and offscreen.

# 2026-07-05 same-session continuation history repair

- user report: WebUI same-session follow-up appeared not to include prior turns; continuation must include all historical turns.
- owner: `provider.reason-live-bridge` in `crates/freehand-runtime`, with `reason.session-history` as the consumed history owner.
- root cause: restored live sessions used persisted `SessionHistory` as-is, but persisted closed/effective turns were not rebuilt into `SessionHistory.base_context_segments` before the next provider request, so `ReasonTurnEngine::start_turn` planned only current input plus any existing base context.
- implementation:
  - runtime live bridge now calls `ReasonPersistence::restore_turn_snapshots_for_ui(session_id)` on restored sessions and converts effective turns into deterministic `SessionMemory` base segments before starting the next round.
  - helper path: `rebuild_session_history_from_effective_turns` -> `effective_turn_context_segments` -> `turn_context_segment` -> `history_visible_assistant_text`.
  - provider/system failure projection in `finish_live_submit` now replaces only the failed session's turns, preserving already-restored other-session transcripts.
  - WebUI online verifier accepts either a live progress card or a fast terminal second turn, preventing fast provider responses from being reported as a missing live-card failure.
- regression locks:
  - `live_bridge_restores_same_session_history_into_follow_up_provider_request` proves second provider request contains `Historical turn 1`, first user prompt, first assistant answer, and second prompt.
  - `live_dispatch_failure_preserves_other_session_transcripts` proves provider retry exhaustion keeps unrelated session transcript visible while failed session gets its own failed projection.
- validation:
  - `cargo fmt --check`
  - `cargo test -p freehand-runtime -- --nocapture` -> 74 passed
  - `node --check scripts/webui_verify_online.mjs`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `scripts/install-launchd.sh restartS`, `curl -4fsS http://127.0.0.1:4042/health`, `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
  - `make verify-webui-online` -> `artifacts/webui-online/20260705-verify-4042-1783254821320/summary.json`
  - real WebUI same-session context proof -> `artifacts/webui-online/20260705-history-4042-1783254901868/summary.json`; session `webui-session-20260705123503-9d9824e6`, turns `runtime-turn-64,runtime-turn-65`, token `FHCTX-1783254901867` recovered in second answer and preserved after refresh.
- exclusions:
  - old wrong-profile `artifacts/webui-online/20260705-verify-4041-*` remain unrelated untracked evidence and were not touched.

# 2026-07-04 error.center first skeleton
  - user requirement: implement the first `error.center` skeleton with feature/function/test/mainline/wiki truth, classify schema/tool/provider errors, write watermarked metadata decisions, and prevent runtime-local bypass for those paths.
  - owner: `error.center`.
  - implementation:
    - added `ErrorCenterObservedFailure`, `ErrorCenterDecision`, error domain/class/recovery/visibility enums, and `classify_error_center_failure` in `crates/freehand-control`.
    - runtime now writes `error.center` metadata for completion schema rejection, failed tool result, and provider executor failure before repair/re-entry/failure materialization continues.
    - error-center metadata uses writer owner `error.center`, write-node provenance, retry fields, public visibility, owner target, repair fields, and raw hash; raw error text is not written into `error.center` rows.
    - added `docs/function-maps/error.center.md`, `docs/testing/error.center.md`, `docs/mainline-calls/error.center.json`, and generated `docs/wiki/error.center.md`.
    - updated feature map, control/error design truth, and adjacent control/task docs.
  - verified:
    - `cargo test -p freehand-control -- --nocapture`
    - `cargo test -p freehand-runtime live_bridge_records_error_center_metadata_for_schema_repair -- --nocapture`
    - `cargo test -p freehand-runtime live_bridge_returns_unknown_tool_as_failed_tool_result_without_terminalizing -- --nocapture`
    - `cargo test -p freehand-runtime live_bridge_writes_provider_error_metadata_on_executor_failure -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p xtask -- --nocapture`
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `cargo test --workspace`
    - `cargo clippy --workspace --all-targets -- -D warnings`
  - remaining gaps:
    - task/node/UI error policy is not routed through error center yet.
    - ADP query/subscribe projection for error-center metadata is not implemented.
    - WebUI error-center cards are not implemented.
    - full status schema repair loop and selectable user option projection remain pending.

# 2026-07-04 control.center basic status stopHook
  - user requirement: implement the basic status stopHook on the fixed four-hook skeleton, while keeping task dispatch built-in tool lifecycle as a separate review topic.
  - owner: `control.center`.
  - implementation:
    - added `crates/freehand-control` with `parse_control_status_block`, `control_status_rhythm_decision`, and `strip_control_status_block`.
    - runtime live bridge includes status contract guidance before model request, writes `control.center` metadata at the four fixed hook points, and accepts `simple_request=true` status stop without requiring legacy `<freehand_completion>`.
    - UI protocol public projection strips hidden status blocks from assistant and terminal text.
    - docs updated: feature map, function map, test design, control/error design doc, architecture gap registry.
  - current non-goals:
    - no compact `task` action tool yet.
    - no task lifecycle persistence/dispatch yet.
    - no centralized `error.center` yet.
    - no selectable user-option UI projection yet.

# 2026-07-04 task.orchestration persistence skeleton
  - user requirement: land task persistence/lifecycle/memory/startup/recovery design, then start implementation.
  - owner: `task.orchestration`.
  - implementation:
    - added `docs/design/task-orchestration-design.md`, `docs/function-maps/task.orchestration.md`, and `docs/testing/task.orchestration.md`.
    - added `crates/freehand-task` with task ids, task statuses, agent statuses, task snapshots, agent snapshots, append-only ledger events, self-agent bootstrap, create/query/list_agents/query_agent, and runtime memory rebuild on boot.
    - added one built-in `task` tool schema to `freehand-tools`; runtime handles `task` tool calls via `execute_task_tool` and routes to `freehand-task`.
    - first scope supports create with self/auto assignment or WaitingAgent; no real worker execution yet.
  - verified:
    - `cargo fmt --check`
    - `cargo test -p freehand-task`
    - `cargo test -p freehand-tools`
    - `cargo test -p freehand-runtime task_tool_create_persists_and_queries_task -- --nocapture`
    - `cargo test --workspace`
    - `cargo clippy --workspace --all-targets -- -D warnings`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-07-04 task.orchestration lifecycle and mainline migration
  - user requirement: each implementation round must include white-box/black-box tests plus mainline caller and function map updates.
  - implementation:
    - added lifecycle methods: append, pause, resume, submit_review, approve, reject, close.
    - added transition validation so close before approval fails.
    - runtime `task` tool supports the lifecycle ops.
    - migrated task.orchestration to machine-readable mainline call source and generated wiki.
    - updated xtask required-file gate list, function map, test design, and design doc.
  - verified:
    - `cargo fmt --check`
    - `cargo test -p freehand-task -- --nocapture`
    - `cargo test -p freehand-runtime task_tool_create_persists_and_queries_task -- --nocapture`
    - `cargo test -p freehand-runtime task_tool_review_lifecycle_rejects_early_close_and_closes_after_approval -- --nocapture`
    - `cargo test -p xtask -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `cargo test --workspace`
    - `cargo clippy --workspace --all-targets -- -D warnings`

# 2026-07-04 task.orchestration lease heartbeat recovery
  - user requirement: continue implementation in rounds with white-box and black-box tests, plus mainline caller and function map updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `TaskLease` persisted under `~/.freehand/state/task-runtime/<agent_id>/leases.json`.
    - `resume_task` now enters `Running` and creates an active lease-backed heartbeat record.
    - added `task(op="heartbeat")` runtime/tool schema bridge.
    - `TaskRuntime::boot` loads leases and conservatively changes `Running` tasks with missing, mismatched, inactive, or expired lease to `Interrupted`.
    - leaving `Running` removes the active lease; heartbeat for non-running tasks is rejected and writes no lease.
    - updated design doc, test design, function map, feature map, machine mainline caller, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 8 passed; covers resume lease creation, heartbeat refresh, expired lease recovery to `Interrupted`, and non-running heartbeat rejection.
    - module black-box: `cargo test -p freehand-runtime task_tool_resume_and_heartbeat_persist_running_lease -- --nocapture` -> 1 passed.
    - existing runtime task black-box: create/query and review lifecycle tests passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - mainline/gate: `cargo run -p xtask -- mainlines generate`, `mainlines check`, `gates check` passed.
    - full regression: `cargo test --workspace` -> 398 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues.
  - remaining gaps:
    - no real worker execution loop yet.
    - no UI task projection yet.
    - no multi-agent dispatch/agent create-close operations yet.

# 2026-07-04 task.orchestration agent registry lifecycle
  - user requirement: continue execution in implementation rounds with white-box/black-box testing and mainline/function map updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `assign`, `cancel`, `create_agent`, and `close_agent` to the single `task` tool op surface.
    - `TaskRuntime::boot` now loads all persisted agent snapshots, not only the self agent.
    - `create_agent` persists available worker snapshots with declared capabilities.
    - `assign_task` moves `WaitingAgent`/`Created`/`Interrupted` tasks to `Assigned` only when the target agent is available.
    - assigned tasks count as queued work; resume/heartbeat moves work to running count; cancel/review/terminal release assignee state.
    - `close_agent` closes only idle agents and rejects busy/queued/running agents.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 12 passed; covers agent create/recover/close, waiting assign, cancel release/reject resume, and busy-agent close rejection.
    - module black-box: `cargo test -p freehand-runtime task_tool_agent_assign_cancel_close_lifecycle -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for create/query, review lifecycle, and heartbeat lifecycle passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 403 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution loop yet.
    - no UI task projection yet.
    - multi-agent dispatch still has no real worker process/channel; agent registry lifecycle is only persisted skeleton truth.

# 2026-07-04 task.orchestration priority claim skeleton
  - user requirement: continue task lifecycle implementation in tested rounds with function map and mainline caller updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `claim_next` to the single `task` tool op surface.
    - added `TaskRuntime::claim_next_task`, which lets an agent claim its highest-priority assigned task into lease-backed `Running`.
    - adjusted assign semantics so an agent can hold multiple queued assigned tasks; assigned work increments queued count, claim/resume moves one queued task into running count.
    - empty queue claim returns an explicit no-task result without mutating task/agent/lease truth.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 14 passed; covers highest-priority claim, running lease creation, queue count, and empty queue no-mutation.
    - module black-box: `cargo test -p freehand-runtime task_tool_claim_next_runs_highest_priority_task -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for agent lifecycle and heartbeat lifecycle passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 406 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no UI task projection yet.
    - no worker debug stream/turn update projection yet.

# 2026-07-04 task.orchestration worker execution record skeleton
  - user requirement: continue task/multi-agent lifecycle implementation in tested rounds with function map and mainline caller updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `record_execution` to the single `task` tool op surface.
    - added `TaskRuntime::record_execution`, which writes semantic worker progress only for `Running` tasks.
    - execution records write `TaskExecutionRecorded` events into the task ledger and keep task status `Running`.
    - non-running tasks reject `record_execution` with invalid transition and do not advance event sequence.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 16 passed; covers running progress write/recovery and non-running rejection/no sequence advance.
    - module black-box: `cargo test -p freehand-runtime task_tool_record_execution_requires_running_task -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for claim_next and heartbeat lifecycle passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 409 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no UI task projection yet.
    - no worker debug stream/turn update projection yet.

# 2026-07-04 task.orchestration ledger history query
  - user requirement: continue task/multi-agent lifecycle implementation in tested rounds with function map and mainline caller updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `history` to the single `task` tool op surface.
    - added `TaskRuntime::task_history`, which reads the append-only task ledger and returns ordered lifecycle events.
    - history for unknown task returns explicit `TaskNotFound`.
    - runtime `task(op="history")` returns task timeline JSON.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 18 passed; covers ordered ledger event history and unknown-task failure.
    - module black-box: `cargo test -p freehand-runtime task_tool_history_returns_ordered_execution_timeline -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for record_execution and claim_next passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 412 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no UI task projection yet.
    - no worker debug stream/turn update projection yet.

# 2026-07-04 task.orchestration task list query
  - user requirement: continue task/multi-agent lifecycle implementation in tested rounds with function map and mainline caller updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `list_tasks` to the single `task` tool op surface.
    - added `TaskRuntime::list_tasks`, which returns task snapshots filtered by status and assignee and sorted by priority.
    - runtime `task(op="list_tasks", status, agent_id)` returns queue/UI projection JSON without mutating task truth.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 19 passed; covers status/assignee filtering and priority order.
    - module black-box: `cargo test -p freehand-runtime task_tool_list_tasks_filters_queue_projection -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for history and record_execution passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 414 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no UI task projection yet.
    - no worker debug stream/turn update projection yet.

# 2026-07-04 runtime-backed ADP task query
  - user requirement: continue closing task/multi-agent gaps with real implementation, white-box and black-box verification, function map and mainline caller updates.
  - owners: `ui.protocol`, `runtime.ui-command-dispatch`, app transport callers.
  - implementation:
    - added protocol-owned `QueryTaskList` and `QueryTaskHistory` commands plus UI-safe task list/history DTOs.
    - added `UiRuntimeQueryPort` so app transports stay protocol-only while daemon/runtime can answer owner-backed read-only queries.
    - added `RuntimeCommandDispatcher::query_runtime`, routing task list/history through `TaskRuntime::list_tasks` and `TaskRuntime::task_history` without duplicating task filtering or ledger ordering.
    - wired WebUI/daemon ADP query handling to ask the runtime query port first, then protocol state only when no runtime owner handles the query.
    - added `freehand-cli adp-task-query` for no-UI task list/history verification.
    - updated feature map, function maps, test designs, mainline JSON, and generated wiki for touched features.
  - verified:
    - white-box: `cargo test -p freehand-runtime runtime_query_reads_task_truth_from_task_runtime -- --nocapture`.
    - module black-box: `cargo test -p freehand-daemon daemon_adp_queries_runtime_task_truth -- --nocapture`.
    - target packages: `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-server`, `cargo test -p freehand-cli`, `cargo test -p freehand-task`, `cargo test -p freehand-runtime`, `cargo test -p freehand-daemon`.
    - mainline/gates: `cargo run -p xtask -- mainlines generate`, `mainlines check`, `gates check`, `cargo test -p xtask`.
    - online S daemon: `scripts/install-launchd.sh installS`, 4042 health ok, `freehand-cliS adp-smoke`, `freehand-cliS adp-task-query --status waiting_agent` returned `count=0`, and missing history returned `command_dispatch_target_not_found`.
    - full regression: `cargo test --workspace` -> 416 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no push subscription for task truth yet; current task visibility is ADP query only.
    - no WebUI visual task management panel yet.

# 2026-07-04 runtime-backed ADP task list subscription
  - user requirement: continue closing task/multi-agent gaps with real implementation and validation.
  - owners: `ui.protocol`, `runtime.ui-command-dispatch`, `app.webui-smoke`, `app.runtime-daemon`, `app.cli-runtime-smoke`.
  - implementation:
    - added protocol-owned `SubscribeTaskList { status, agent_id }`, `UiStreamKind::TaskList`, and `UiProjection::TaskList`.
    - added `UiProtocolState::publish_task_list_projection` so runtime can publish task projections without making UI protocol the task truth owner.
    - ADP subscribe initial snapshot now asks injected `UiRuntimeQueryPort` for task list truth before subscribing.
    - runtime live task tool bridge publishes task list projection after successful task truth mutation ops.
    - added `freehand-cli adp-task-subscribe` for no-UI live task subscription verification.
    - updated function maps, test designs, mainline JSON, and generated wiki for touched features.
  - verified:
    - white-box: `cargo test -p freehand-ui-protocol task_list_subscription_matches_runtime_projection_only -- --nocapture`.
    - runtime white-box: `cargo test -p freehand-runtime runtime_task_tool_mutation_publishes_task_list_projection -- --nocapture`.
    - daemon black-box: `cargo test -p freehand-daemon daemon_adp_subscribes_runtime_task_truth -- --nocapture`.
    - package/full gates: `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-runtime`, `cargo test -p freehand-server`, `cargo test -p freehand-daemon`, `cargo test -p freehand-cli`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo run -p xtask -- mainlines generate`, `mainlines check`, `gates check`.
    - online S daemon: `scripts/install-launchd.sh installS`, `curl -4fsS http://127.0.0.1:4042/health`, `freehand-cliS adp-task-query --status waiting_agent`, and `freehand-cliS adp-task-subscribe --status waiting_agent` passed.
  - remaining gaps:
    - no WebUI task management panel yet.
    - no real worker execution process/channel yet.
    - task history remains query-only; worker debug stream remains separate future scope.

# 2026-07-04 freehand-framework-loop initialization
  - user requirement: initialize project loop governance according to loop-governance, ask for missing decisions only if needed.
  - owner: `foundation.workspace`.
  - implementation:
    - added `docs/loops/freehand-framework-loop/` with `LOOP.md`, `STATE.md`, `loop-constraints.md`, `loop-budget.md`, `loop-run-log.md`, and `README.md`.
    - loop starts as `L1 report-only`, manual trigger only, with L2/L3 disabled until explicit approval.
    - bound loop governance to feature map, function map, test design, mainline JSON, and generated wiki.
  - verification:
    - `cargo test -p xtask -- --nocapture`
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
  - finding:
    - gate rejected a docs-only call-table row as a fake source binding; kept loop docs in mainline prose instead of call-table symbol binding.

# 2026-07-04 development symlink launchd profile
  - user requirement: development validation must not repeatedly reinstall/replace the global release binary or trigger the same macOS permission path; global release mode and development symlink mode must coexist with S-suffixed names.
  - owner: `foundation.workspace`.
  - implementation:
    - added `scripts/install-symlink.sh`, which builds debug host binaries and exposes `freehand-cliS`, `freehand-serverS`, and `freehand-daemonS` as symlinks to `target/debug/*`.
    - `freehand-daemon-launchdS` is installed as a prefix-local wrapper copy instead of a symlink because launchd refused to execute a symlink wrapper with `Operation not permitted`.
    - `scripts/install-launchd.sh installS/restartS` manages `com.freehand.daemonS`, `~/.freehand/daemonS.env`, `127.0.0.1:4042`, and `daemonS.*.log`.
    - existing `install/restart` still manage global `com.freehand.daemon`, `~/.freehand/daemon.env`, and `127.0.0.1:4041`.
    - Makefile adds `install-symlink`, `install-launchdS`, `restart-launchdS`, `uninstall-launchdS`, `launchd-statusS`, and `launchd-logsS`.
  - verification:
    - `scripts/install-launchd.sh installS` created S commands and started `com.freehand.daemonS`.
    - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
    - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
    - `scripts/install-launchd.sh restartS` restarted only `com.freehand.daemonS`.
    - `curl -4fsS http://127.0.0.1:4041/health` and `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` still passed, proving global service stayed available.
    - `make ci` -> exit 0.
  - durable workflow: normal development online validation should use S mode on `127.0.0.1:4042`; global `127.0.0.1:4041` is for release/promotion closeout.
  - follow-up repair during WebUI verification:
    - launchd became unreliable when executing the S wrapper or the `freehand-daemonS` symlink after repeated debug rebuilds; symptoms were `Operation not permitted` / `getcwd` stderr and a process that printed listening but did not accept 4042 connections.
    - current S profile keeps `freehand-cliS` and `freehand-serverS` as symlinks, but uses `~/.local/bin/freehand-daemonS-bin` as a local debug daemon copy for launchd.
    - plist now starts through `/bin/bash -lc 'cd <repo> && exec <daemonS-bin> serve ...'`, with S env values projected into launchd environment.
    - verified: `scripts/install-launchd.sh installS`, `curl -4fsS http://127.0.0.1:4042/health`, `freehand-cliS adp-smoke`, and served/workspace `webui.js` hash match.

# 2026-07-04 WebUI chat bubble / SSE display repair
  - user requirement: render as chat conversation; user right-aligned and visually distinct, assistant left-aligned, tool activity embedded inside assistant card, reasoning italic, normal assistant text regular, SSE refresh supported, semantic tool display, shell command shown/truncated, lifecycle colors blue/green/red.
  - implementation:
    - WebUI render path now emits chat bubbles from `RenderConversation` rows; assistant rows and tool rows share one assistant bubble role surface.
    - `ensureSseTurnSubscription()` consumes latest-turn SSE as a display-refresh mirror and routes events through `setTurnProjection()`.
    - `tool.display` projects ordinary shell command fields and keeps `pwd` semantic instead of exposing raw `command=pwd`.
    - WebUI source arrays and DOM fragments dedupe same `turn_id` + visible card text to prevent live latest-turn/session transcript races from duplicating r2 assistant/final cards.
    - S profile repair was required before browser verification because launchd symlink execution on the external workspace became unreliable.
  - verified:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo test -p freehand-blocks -- --nocapture` -> 42 passed
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `scripts/install-launchd.sh installS` with 4042 health, ADP smoke, and served/workspace JS hash match
    - browser evidence captured completed/success and failed selected sessions after click, with selected session pinned, user right alignment, assistant left alignment, semantic shell tool embedded inside assistant, green success, red failure, SSE, no raw completion schema leak, and no duplicate r2 assistant/final card: `artifacts/webui-online/20260704-chat-bubble-sse-4042-click-proof-1783141455456/summary.json`.
    - browser evidence captured current-code running blue tool block plus italic reasoning and SSE: `artifacts/webui-online/20260704-chat-bubble-sse-4042-running-proof-1783141498366/summary.json`.
  - remaining verification gap:
    - freeform provider/tool behavior can still time out independently of WebUI rendering; use ADP sample sessions or already-terminal sessions for deterministic terminal visual proofs.

# 2026-07-01 WebUI submitted input/history disappearance trace
  - user live feedback: after submitting a request, the composer text disappeared and the conversation area showed no user-visible history while the top status still showed live model/tool-result state.
  - correction: clearing the composer after send is acceptable only if the submitted input is immediately preserved in the conversation transcript or pending render projection. Already-observed history must never be removed or hidden by later live status transitions.
  - investigation target: compare raw ADP session transcript with WebUI `RenderConversation` output for `webui-session-20260701131739-e31eb6cf` / `runtime-turn-65`.
  - follow-up user feedback: two real consecutive requests still disappeared. Prior proof only covered a single immediate pending render and did not cover repeated submit / dispatch failure / later refresh lifecycle.
  - live follow-up: screenshot showed visible ADP failure `reason ledger sequence is invalid: expected 338, got 337`. Online 4041 verification found corrupted reason ledger sequence in `~/.freehand/ledgers/reason/master/webui-session-20260701131739-e31eb6cf.jsonl` line 338 and `runtime-session-master.jsonl` line 380. This is runtime persistence truth failure, not a WebUI display-only bug.
  - root cause in code: `ReasonPersistence::persist_row` computed `next_seq` from authoritative cursor before acquiring any session-wide lock, while `append_row_only` only locked the file append. Concurrent same-session writers could both allocate the same next seq, append duplicate/regressed rows, and later block projection/recovery.
  - implementation: added session-scoped reason persistence lock around cursor read -> seq allocation -> ledger append -> snapshot/sidecar refresh; added `concurrent_same_session_writes_allocate_monotonic_sequences` regression.
  - online verification gap: installing this fix caused 4041 bootstrap to fail because the already-corrupted production ledger still blocks restore. Data repair/quarantine of `~/.freehand/ledgers/reason/master/webui-session-20260701131739-e31eb6cf.jsonl` requires explicit authorization.

# 2026-07-01 WebUI selected-session render source trace
  - user issue: continuing a previous conversation and submitting new input left the WebUI visually stale.
  - diagnosis:
    - `renderMessages()` used `state.sessionTurns.length > 0` as a hard switch and ignored `state.turn` whenever a selected session transcript existed.
    - ADP subscription/query updates can deliver the latest same-session turn before the selected transcript is refreshed, so stale `sessionTurns` can hide the new in-flight/completed turn.
    - render state was split across transcript truth and latest-turn truth without one view selector.
  - implementation:
    - added `conversationTurnsForRender()` in `apps/freehand-server/assets/webui.js`.
    - render now merges selected-session transcript with the latest same-session turn before drawing chronological cards.
    - draft empty state remains clean only when there is no latest turn.
  - pending verification:
    - completed
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `scripts/install-global.sh` -> full release/install completed
    - `scripts/install-launchd.sh restart`
    - fixed `127.0.0.1:4041` health returned `ok`
    - served WebUI JS hash matched workspace hash `95b46401c605d0adaf78a4a3f85d765f99ce7ebceb92b6623c05c9acaf2fa07a`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - controlled same-session ADP continuation: `webui-render-e2e-1782893413637890000` first query count 1, second query count 2, new turn `runtime-turn-67`
    - real headless Chrome/CDP WebUI continuation on the same session:
      - screenshot `artifacts/webui-selected-session-render/20260701-continue/headless-old-session-third-turn.png` shows immediate third-turn pending card and cleared input
      - screenshot `artifacts/webui-selected-session-render/20260701-continue/headless-old-session-third-turn-final.png` shows third turn in `THINKING... 17S`
      - screenshot `artifacts/webui-selected-session-render/20260701-continue/headless-old-session-third-turn-terminal.png` shows final `runtime-turn-68-r9` terminal card in the selected session

# 2026-07-01 WebUI render architecture closeout plan
  - user request: produce a complete implementation plan, land it to docs, then provide a `/goal` prompt.
  - review correction:
    - WebUI main control/status path is ADP WebSocket `/adp`; SSE is compatibility only and should not be treated as the primary next step.
    - the main bug class is render/state/lifecycle coupling, not merely "missing SSE".
    - historical turns must not keep animating after a later live turn appears.
  - plan landed:
    - `docs/goals/webui-render-architecture-closeout-plan.md`
    - plan now explicitly calls for projection -> render model -> view separation, turn-scoped and tool-scoped lifecycle clocks, ADP as the unified path, and real browser + ADP transcript verification.
  - implementation:
    - `apps/freehand-server/assets/webui.js` now builds `RenderConversation` / `RenderTurn` / `RenderRow` before DOM rendering.
    - model wait clocks use `lifecycleClocks` keyed by session/turn/phase/detail; removed the old global `modelRequestStartedAt` state.
    - tool timings are keyed by turn/tool identity; completed/failed tool timings freeze at terminal observation instead of continuing to count in historical cards.
    - `conversationTurnsForRender()` preserves transcript order and appends latest same-session turn instead of sorting by `runtime-turn-*` ordinal, because runtime turn ordinals can reset after restart.
    - current prompt-first submit state remains live `dispatching` until model/tool projection arrives, so the conversation area no longer goes blank while the status strip says dispatching/thinking.
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `scripts/install-global.sh` -> completed release/install and installed host binaries to `~/.local/bin`
    - `scripts/install-launchd.sh restart`
    - fixed `127.0.0.1:4041` health returned `ok`
    - served JS hash matched workspace hash `4b999956af46174a99ecc83c6d40307187121b6a7c2a24b91057acec32b52e41`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - Playwright WebUI evidence under `artifacts/webui-render-architecture-closeout/20260701-live/`:
      - `03-current-live-old-static.json/png`: `blockCount=4`, `liveCount=1`, `nonLastLiveCount=0`; only bottom `runtime-turn-65` is live dispatching, historical turns are static.
      - `04-terminal-no-stale-animation.json/png`: `blockCount=5`, `liveCount=0`, `nonLastLiveCount=0`; terminal state has no stale animation, slow bash tool row is completed with frozen elapsed.

# 2026-06-30 ADP multi-round sample closeout
  - user correction: one-round success is not valid evidence; failure sample must complete a continuous multi-round tool loop before reporting success.
  - implementation:
    - `freehand-cli adp-turn-sample --sample failure` now creates an isolated sample session instead of using the shared runtime session.
    - after matching the final projection, the CLI queries `QuerySessionTurns` and requires transcript evidence; failure sample requires `rounds>=2`, at least one unique tool execution, and at least one unique failed tool result.
    - CLI de-duplicates transcript tool counts by `tool_call_id` because final round projections can aggregate earlier tool activity.
    - CLI now fails immediately when the target sample session reaches a system/provider failed terminal instead of waiting for timeout.
    - provider executor failures now materialize `provider_executor_failure` error truth and failed terminal truth before dispatch failure returns, preventing silent active-turn hangs.
    - WebUI inactive text-only/restored turns now render neutral waiting/active state rather than fake streaming animation.
  - live verification:
    - `target/debug/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure` -> `adp_turn_sample_ok ... session=cli-adp-sample-failure-1782833766680278000 turn=runtime-turn-69-r2 rounds=2 tool_executions=1 failed_tools=1 ... command_receipt ... reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 ... EXIT:0`
    - ADP/latest-turn for prior run `runtime-turn-68-r4` showed full multi-round completion after schema retry: failed `read_file` tool activity plus final `terminal_status=Success`.
    - metadata ledger for `runtime-trace-66` showed round 1 provider request, failed `read_file`, `ReasonReq05ToolResultReentry`, round 2 provider request with `tool.exchange_count=1`, and `RuntimeLive04TurnClosed` with `bridge.rounds=2`, `bridge.tool_executions=1`, `terminal.status=Success`.
    - provider/system failure negative path observed on `runtime-turn-65-r4`: after schema retries, provider HTTP send failure surfaced as `terminal_status=Failed` with explicit error instead of hanging.
  - verification commands:
    - `cargo test -p freehand-cli -- --nocapture` -> 12 passed
    - `cargo fmt --check` -> passed

# 2026-06-30 WebUI session CRUD and tool card follow-up
  - user correction: WebUI still could not multi-select/delete sessions, so CRUD was not usable despite ADP/session protocol support.
  - user correction: tool/result "merge" must be semantic, not just visual style; successful tool results should update the same execution card status instead of becoming a separate mechanical content item.
  - implementation:
    - sidebar adds visible `session-bulk-toolbar` with selected count, Clear, and Delete
    - session rows now use checkboxes for multi-select and a separate session button for navigation
    - Delete sends ADP `DeleteSession` for every selected session, clears local selection, refreshes `QuerySessionList`/selected transcript
    - server smoke asserts HTML/JS include multi-select/delete controls and ADP `DeleteSession`
  - tool display follow-up:
    - removed old live wait helper path that could render extra waiting cards
    - tool card status bar now owns waiting/completed timing; card body only shows semantic target/result
    - `tool.display` projects `bash pwd` as `Read current working directory` without `command=pwd`
    - `ui.protocol` public tool body now prefers semantic target/diff display for waiting/completed/failed tools; success/failure result text is status/outcome, not primary body content
    - WebUI execution cards now render one execution-cycle card and no longer append `display.result_summary` as another success-result line
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 39 passed
    - `target/debug/xtask mainlines generate/check` and `target/debug/xtask gates check` -> ok
    - `cargo build --release -p freehand-cli -p freehand-server -p freehand-daemon` -> ok, host binaries installed to `~/.local/bin`
    - `scripts/install-launchd.sh restart` restarted fixed `127.0.0.1:4041`
    - fixed-port `/assets/webui.js` contains `turnExecutionCard` / `pendingExecutionCard` and no longer contains `display.result_summary`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - CDP-operated Chrome screenshots:
      - `artifacts/webui-semantic-merge/20260630-page/13-cdp-after-send-immediate.png` shows submit immediately renders one execution card with `dispatching...0s`
      - `artifacts/webui-semantic-merge/20260630-page/14-cdp-after-send-update.png` shows explicit ADP timeout failure and retained composer input for retry
    - full `scripts/install-global.sh` did not complete reliably under this Codex tool session; stale install processes were terminated by exact PID only, and no install/release/cargo residual process remained after cleanup

# 2026-06-30 WebUI final summary and sample-label cleanup
  - user correction:
    - Final card should not render Evidence, Learned, or Completion reason by default; those are debug-only details
    - success card border should be green, failure card border red and smaller
    - bottom demo/sample wording should be removed
  - implementation:
    - WebUI adds `terminalBodyForDisplay`, `terminalSummaryLine`, and `stripDebugTerminalLines`
    - default Final rendering extracts only `Summary:` content; full terminal text is restored only when `Debug details` is enabled
    - topbar exposes `Debug details` toggle with `Debug off/on` state
    - execution card success/failed border colors now follow terminal status
    - visible buttons changed from `Success sample`/`Failure sample` to `Success`/`Failure`, and sample prompt visible prefixes were removed
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo run -p xtask -- mainlines generate/check`
    - `cargo run -p xtask -- gates check`
    - `cargo build --release -p freehand-cli -p freehand-server -p freehand-daemon`
    - installed host binaries to `~/.local/bin`, restarted launchd fixed `127.0.0.1:4041`
    - fixed-port HTML has `Debug details`, `Success`, `Failure`, and no `Success sample`/`Failure sample`
    - fixed-port JS has `terminalBodyForDisplay`, `stripDebugTerminalLines`, `scenario loaded`, `Debug off`, and no `ADP success sample`/`ADP failure sample`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - screenshot `artifacts/webui-semantic-merge/20260630-page/15-final-filtered-current-chrome.png` shows Final only has summary, no Evidence/Learned/Completion reason, and bottom buttons read `Success`/`Failure`
    - `scripts/install-global.sh` completed full Rust/Android release regression and installed matching `~/.local/bin/freehand-daemon`
    - `scripts/install-launchd.sh restart` restarted fixed `127.0.0.1:4041`
    - fixed-port HTML contains `session-bulk-count`, `session-clear-selection-button`, `session-delete-selected-button`
    - fixed-port JS contains `selectedSessionIds`, `deleteSelectedSessions`, `DeleteSession`, `session-selector`, `renderSessionBulkToolbar`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` passed

# 2026-06-30 fixed daemon port verification
  - occupied launchd service `com.freehand.daemon` stopped first with `launchctl stop gui/$(id -u)/com.freehand.daemon`
  - exact PID `49108` was terminated only after stop did not exit it
  - `scripts/install-launchd.sh install` rebuilt/reinstalled and relaunched the daemon
  - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
  - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
  - `~/.local/bin/freehand-cli adp-session-manage --url ws://127.0.0.1:4041/adp --action create --session webui-fixed-4041-check --title 'Fixed 4041 Check' --cwd /Volumes/extension/code/freehand` -> success
  - empty cwd create via `adp-session-manage` failed explicitly as `empty_session_cwd`

# 2026-06-30 WebUI session bulk select-all and typography follow-up
  - user request:
    - add a `Select all` action to the session bulk toolbar
    - small text should not use heavy bold weight
  - user correction after screenshot:
    - `Select all` showed `9 selected` while several visible sessions were still unchecked
    - root cause was `isDraftSessionId(sessionId)` using `sessionId.startsWith("webui-session-")`
    - persisted real sessions also use `webui-session-*`, so bulk select skipped them as if they were draft
  - implementation:
    - page shell now renders `session-select-all-button`
    - WebUI JS selects all non-draft session ids into the existing multi-select set
    - draft status is now only `state.draftSessionId === sessionId`; id prefix is never used
    - bulk toolbar is split into a summary row and a wrapping action row
    - small bulk count/button text is normal weight (`font-weight: 500`) instead of the previous heavy look
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - fixed-port HTML at `127.0.0.1:4041` contains `session-select-all-button`, `session-clear-selection-button`, `session-delete-selected-button`, and `session-bulk-count`
    - fixed-port JS at `127.0.0.1:4041` contains `selectAllSessions`, `sessionSelectAllButton`, `selectedSessionIds`, `draftSessionId: null`, and `state.draftSessionId === sessionId`; it no longer contains `startsWith("webui-session-")`
    - fixed-port CSS at `127.0.0.1:4041` contains `session-bulk-summary`, `session-bulk-actions`, `session-bulk-button.select-all`, and `font-weight: 500`
    - Chrome AppleScript DOM click verification was blocked by Chrome's "Allow JavaScript from Apple Events" setting, so no visual DOM count proof was captured in this slice

# 2026-06-30 WebUI new conversation state and chat visual follow-up
  - user correction:
    - after `New conversation`, sidebar showed both `no sessions` and a draft item, which is an incorrect state projection
    - conversation area still looked like old card UI rather than a chat surface
  - implementation:
    - when `state.sessions.length === 0` and `state.draftSessionId` exists, sidebar renders only the draft row
    - empty transcript now renders a dedicated `chat-empty-state` with title/copy instead of plain text or system-card feel
    - dialogue cards are narrowed to fit-content bubbles, borders/headers are reduced, and title weights are reduced to avoid heavy card UI
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo test -p freehand-server -- --nocapture`
    - full `scripts/install-global.sh` completed workspace tests, mainlines/gates, Android JVM, Rust release binaries, and Android release APK
    - `scripts/install-launchd.sh restart`
    - fixed-port `127.0.0.1:4041` CSS contains `chat-empty-title`, `width: fit-content`, and `border-bottom: 0`
    - fixed-port `127.0.0.1:4041` JS contains `if (state.draftSessionId)` and `Send a message to start this session.`
    - ADP smoke on `ws://127.0.0.1:4041/adp` passed
    - screenshot evidence saved to `artifacts/webui-session-ui-fix/20260630-new-session-chat/02-chrome-after-refresh.png`, but Chrome AppleScript JS permissions still block automated click/DOM-count verification

# 2026-06-29 tool display semantic owner
  - user requirement:
    - tool classification must have a standard independent file and locked owner
    - every parser must be an independent function in an independent module
    - UI must not guess categories; UI only consumes parsed projection
  - implementation:
    - added `tool.display` owner in `crates/freehand-blocks/src/tool_display.rs`
    - added structured `ToolDisplayProjection` with kind/outcome/action/target/parameter_summary/summary/result_summary/fields/diff
    - added independent parser functions for read/list, file mutation, search, plan, shell, and generic tools
    - `ui.protocol` now attaches `UiToolActivity.display` during tool call projection and updates it on tool result projection
    - public tool summaries now prefer structured display action/summary/parameter/result over raw detail
    - WebUI `toolSummaryBody` consumes `display` fields and renders tool parameters/results as secondary grey lines instead of classifying raw tool text
  - verification:
    - `cargo fmt --check`
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-blocks`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p freehand-cli -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
  - live WebUI verification:
    - global install + launchd fixed-port restart completed
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - real WebUI page operation loaded `Failure sample`, submitted it, and captured screenshots under `artifacts/webui-tool-display-e2e/20260629-parameters/`
    - `03-tool-parameter-visible.png` captured in-flight read_file tool card with `path=definitely-missing-freehand-file.txt`
    - `05-reloaded-final-parameter.png` captured terminal state with `turn completed`, no waiting text, `Read file` card showing parameter and failed result, plus shell command card showing `command=... · timeout=60`
    - ADP truth for `webui-session-20260629044814-5eb78029` showed `runtime-turn-43-r4` Success with `display.parameter_summary=path=definitely-missing-freehand-file.txt`

# 2026-06-29 session CRUD protocol support
  - user requirement:
    - before UI presentation, session CRUD must be supported through shared protocol/runtime truth
    - WebUI must not invent local-only session management state
  - implementation:
    - `ui.protocol` added session management commands: `CreateSession`, `RenameSession`, `ArchiveSession`, `RestoreSession`, `DeleteSession`
    - `UiSessionSummary` and `UiSessionTranscriptProjection` now expose `title` and `archived`
    - `UiProtocolState` can project metadata-only sessions and archived-session list separately
    - `reason.persistence` owns `PersistedSessionMetadataEntry` in `~/.freehand/state/ui/<agent>/session-metadata.json`
    - `delete` is currently non-destructive delete-as-archive because physical deletion of turn truth needs explicit destructive lifecycle approval
    - `runtime.ui-command-dispatch` routes CRUD commands into `ReasonPersistence` and refreshes shared UI projection
    - `freehand-cli adp-session-manage` provides no-UI ADP CRUD control
  - verification:
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 38 passed
    - `cargo test -p freehand-reason -- --nocapture` -> 56 passed
    - `cargo test -p freehand-runtime -- --nocapture` -> 51 passed
    - `cargo test -p freehand-cli -- --nocapture` -> 12 passed
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
  - live ADP verification:
    - existing fixed daemon `127.0.0.1:4041` health and ADP smoke passed
    - current workspace daemon on `127.0.0.1:4092` health passed
    - `target/debug/freehand-cli adp-session-manage --action create --session adp-crud-session-20260629 --title 'ADP CRUD Session' --cwd /tmp` returned `target=reason.persistence status=session_metadata_updated`
    - rename returned `session_metadata_updated`
    - active query showed `adp-crud-session-20260629:0:empty`
    - archive hid it from active session list
    - restore made it visible again
    - delete hid it again through non-destructive archive semantics
    - unknown archive returned explicit `command_dispatch_target_not_found`
  - remaining risk:
    - WebUI session context menu/UI presentation is not implemented in this slice
    - archived-session list is protocol-supported but CLI query helper currently prints only active list unless extended further
    - 4092 debug daemon health still responded after validation, but process PID was not discoverable through allowed exact-PID lookup; no broad kill was attempted

# 2026-06-29 session cwd owner wiring
  - user requirement:
    - session must have a working directory and WebUI must allow choosing it
    - `/new` should not render system feedback as a chat message
  - implementation:
    - `SubmitUserInput.cwd` is protocol-owned and empty cwd is rejected as `empty_session_cwd`
    - `UiTurnProjection`, `UiSessionSummary`, and `UiSessionTranscriptProjection` expose cwd
    - runtime canonicalizes requested cwd, binds it to the selected session, persists it on `TurnRecord.cwd`, restores it after bootstrap, and inherits it on later same-session submits
    - tool execution uses `freehand-tools::with_workspace_root` so session cwd is passed as an explicit per-call workspace root instead of mutating process-global cwd/env
    - WebUI adds a cwd input, forwards `SubmitUserInput.cwd`, shows cwd in topbar/session metadata, and `/new` now renders a clean empty state instead of the old selected-session/no-turns chat card
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo test -p freehand-tools -- --nocapture`
    - `cargo test -p freehand-reason -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
  - live WebUI verification:
    - local WebUI smoke server on `127.0.0.1:4088` returned `/health=ok`
    - Playwright real page operation clicked `New session`, set cwd `/Volumes/extension/code/freehand`, verified no `selected session:` system card, submitted a prompt, and observed command receipt/status
    - screenshots:
      - `artifacts/webui-session-cwd-e2e/20260629-session-cwd/01-initial-cwd-control.png`
      - `artifacts/webui-session-cwd-e2e/20260629-session-cwd/02-new-session-cwd-clean.png`
      - `artifacts/webui-session-cwd-e2e/20260629-session-cwd/03-submit-cwd-status.png`

# 2026-06-29 WebUI provider-request wait state repair
  - gap found:
    - submit/dispatch waiting was local WebUI state and did not prove provider request had been built/sent
    - runtime already emitted `RuntimeLive02ProviderRequestBuilt` debug truth, but ADP turn projection did not expose request-sent/model-response-waiting lifecycle state
  - fix:
    - `UiTurnProjection.model_request` carries protocol-owned request-sent/model-response-waiting state
    - `RuntimeLive02ProviderRequestBuilt` is mapped into `UiProtocolState::apply_model_request_waiting`
    - WebUI renders an animated "waiting for model response" card with elapsed time from protocol projection
    - model request wait clears on semantic response, tool call, tool result, usage, terminal, or error projection
  - verification:
    - `cargo fmt --check`
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI tool-result and model-wait progress repair
  - gap found:
    - `ui.protocol` intentionally kept public tool summary body status-only, so WebUI could not show tool execution output
    - WebUI only timed submit/dispatch and tool execution waiting; after tool completion while waiting for model continuation, there was no dedicated timed lifecycle card
  - fix:
    - `UiToolActivity.detail` now carries completed/failed tool result detail from `ToolResultContract.output`
    - public tool summaries expose the protocol-projected result detail
    - WebUI tool cards render result detail together with status and elapsed execution time
    - WebUI renders a timed animated "waiting for model" card after completed/failed tool activity until terminal/update arrives
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-server`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI lifecycle progress timing repair
  - gap found: tool waiting had animation/timer, but submit/dispatch waiting before first turn projection only had static pending/status text
  - fix:
    - WebUI records `submitStartedAt`
    - pending submit card renders animated running state and elapsed dispatch wait time
    - lifecycle status refreshes once per second for submit/dispatch and tool waiting
    - tool waiting status includes elapsed time in the main status strip as well as the tool card
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI assistant-card collapse repair
  - review found a remaining regression after multiround transcript merge: same logical turn still rendered multiple assistant cards because `derivePublicConversation` emitted one `AssistantText` per text chunk
  - fixed by collapsing assistant text inside each turn into one visible card while preserving tool summaries and terminal/error cards
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI multiround restore closeout
  - verified live fixed-port daemon at `http://127.0.0.1:4041/` and `ws://127.0.0.1:4041/adp`
  - real browser evidence now shows one logical transcript item per execution cycle, with `runtime-turn-N` and `runtime-turn-N-rM` merged for display only
  - assistant cards now strip raw `<freehand_completion>...</freehand_completion>` blocks, while the Final card keeps user-facing completion content
  - restart restore now rebuilds UI projections from reason-ledger per-turn snapshots so earlier-round tool activity survives daemon restart
  - verification evidence:
    - `artifacts/webui-visual-session/20260629-multiround-slowtool-success/`
    - `artifacts/webui-visual-session/20260629-final-restore-merged-clean/`
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-runtime live_bootstrap_restores_multiround_tool_activity_into_ui_state`

# 2026-06-28 Minimonth config and WebUI alignment goal
  - config check:
    - requested source config `/Volumes/extension/.rcc/provider/minimonth/config.v2.toml` contains provider id `minimonth`, type `anthropic`, base URL `https://api.53hk.cn`, default model `MiniMax-M2.7`, and a present API key
    - current runtime config `~/.freehand/config.toml` uses provider id `minimonth`, base URL `https://api.53hk.cn`, default model `MiniMax-M2.7`, and the active `master` and `worker` agents both point to `minimonth`
    - Freehand config schema requires explicit `protocol`; RCC `transportBackend` is not a Freehand runtime config field
  - goal doc:
    - added `docs/goals/webui-session-transcript-alignment-plan.md`
    - goal locks WebUI rendering to persisted session truth plus latest ADP overlay, Codex-style low-noise conversation/tool display, and Reasonix-style session restore/history rebuild
  - 2026-06-28 progress:
    - `~/.freehand/config.toml` updated to runtime provider `minimonth` with base URL `https://api.53hk.cn` and model `MiniMax-M2.7`; secret copied from RCC source without printing it
    - `freehand-cli --agent master` verified active provider `minimonth`, protocol `messages`, model `MiniMax-M2.7`, and Minimonth base URL
    - fixed session transcript ordering in `ui.protocol` and WebUI local overlay path so numeric turn ids such as `runtime-turn-10` do not sort before `runtime-turn-2`
    - added `freehand-cli adp-session-query --url ... [--session <id>]` for no-UI session list/transcript validation over ADP
    - WebUI input layer now has shortcut and slash-command affordances routed through existing ADP/query/cancel/sample helpers: `/help`, `/sessions`, `/reload`, `/success`, `/failure`, `/cancel`, `/clear`; shortcuts include Cmd/Ctrl+Enter, Esc, Cmd/Ctrl+R, Cmd/Ctrl+K, Cmd/Ctrl+1, Cmd/Ctrl+2
    - targeted verification passed: `node --check apps/freehand-server/assets/webui.js`, `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-cli`, `cargo test -p freehand-server`, `cargo test -p freehand-runtime`
  - 2026-06-29 final smoke:
    - `curl -4fsS http://127.0.0.1:4041/health` returned `ok`
    - `FREEHAND_PAIR_TOKEN_SHARED=test-pair-token ~/.local/bin/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample success` returned `adp_turn_sample_ok` with `turn=runtime-turn-41` and `rounds=1`

# 2026-06-28 WebUI conversation/session product repair
  - user correction:
    - current WebUI is not acceptable as a chat product because it behaves like a dashboard/slide surface
    - primary missing product contract: persistent session concept, session management, and refresh recovery
    - status/permission/tool failures must be rendered inside the conversation lifecycle, not as disconnected panels or silent failures
  - implementation direction:
    - add protocol-owned session list and session-turn query truth
    - WebUI restores selected session from localStorage and queries persisted/protocol state after refresh
    - page shell becomes normal chat layout: session list + conversation transcript + composer
    - permissions preflight/failure state will be attached to the same visible turn/status chain after session UI is stabilized
  - DeepSeek-Reasonix reference findings:
    - actual relevant implementation is `/Volumes/extension/code/DeepSeek-Reasonix/desktop`, not `~/code/reasonix`
    - Reasonix restores tabs/session paths on startup, persists session files only on turn completion, and front-end event subscription is live-only
    - front-end rebuilds visible transcript from session history first (`historyMessagesToItems`), then applies live events as updates (`turn_started`, `text`, `tool_dispatch`, `tool_result`, `turn_done`)
    - blocking prompts are explicitly replayed after subscription reconnect (`ReplayPendingPrompts`) so UI never waits silently without a visible pending action
    - Freehand equivalent must be: session/index/transcript is restart truth; ADP is latest live signal; WebUI/Android/CLI render session truth plus ADP deltas, never ADP-only history


# 2026-06-28 live tool failure UI projection repair
  - new real-session failure found after fixed-port/sample validation:
    - WebUI screenshot error was `dispatch port failure: failed to project live error turn from persistence: reason ledger sequence is invalid: expected 380, got 379`
    - real broken path is historical session `~/.freehand/ledgers/reason/master/runtime-session-master.jsonl`, not fresh ADP sample turns
    - current file inspection shows no internal blank lines and no extra trailing bytes beyond newline; `wc -l` > final `seq` came from counting the terminal newline, not an extra JSON row
    - likely real failure mode is restore racing a partially appended final ledger row during live error projection; current `load_reason_ledger` has no explicit "last line incomplete" recovery rule and parses whole file snapshot at once
    - WebUI also renders `adpFailure` before persisted conversation, so transport failure can visually preempt the user message / turn history
  - deeper runtime evidence after launchd restart:
    - historical `runtime-session-master` reason ledger contains old pre-`ToolResultContract.status` rows such as line 6 `tool_result={tool_call_id, output}` with no outer status field
    - historical metadata ledger `~/.freehand/ledgers/metadata/master/runtime-session-master.jsonl` line 495 contains two JSON objects concatenated on one physical line
    - metadata loader assumed one JSON object per line and metadata append had no file lock, so launchd bootstrap could fail on `trailing characters`

# 2026-06-28 live tool failure UI projection repair
  - root cause:
    - live bridge tool execution failure used to return `RuntimeLiveBridgeError::ToolExecutionFailed` before materializing failed turn truth, so protocol truth stayed active/non-terminal and WebUI could only show waiting
    - first repair wrote failed truth to persistence but runtime dispatch `Err` branch still skipped UI projection; fixed by refreshing `UiProtocolState` from authoritative persistence before returning dispatch failure
    - second live validation exposed a history-pollution bug: dispatch failure projection was aggregating all restored session turns; fixed by projecting only the current runtime-turn ordinal
    - failed terminal projection still left waiting tool activities as waiting; fixed `ui.protocol` to mark still-waiting tool activities `Failed`
  - locked by tests:
    - `live_bridge_fails_explicitly_on_unknown_tool_name`
    - `live_bridge_fails_explicitly_on_registered_unimplemented_tool_name`
    - `live_dispatch_projects_failed_tool_turn_into_ui_state` now covers consecutive failures without historical tool leakage
    - `failed_terminal_marks_waiting_tool_activity_failed`
  - validation passed:
    - `cargo test -p freehand-ui-protocol`
    - `cargo test -p freehand-runtime`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - fixed-port daemon `http://127.0.0.1:4041/health` -> 200 `ok`
    - real command ingress with `ls path=~/code/codex` -> HTTP 500 explicit `command_dispatch_port_failure`
    - latest-turn query for `runtime-turn-17` -> `terminal_status=Failed`, one current `tool_activities[0].status=Failed`, terminal/error public cards failed
    - latest-turn SSE emitted same failed turn projection

# 2026-06-28 low-noise tool card rendering
  - UI truth gap:
    - protocol projected tool cards still carried verbose generic wording like `Tool call requested` / `Tool result returned`
    - user only needs core tool semantics, blocking state, elapsed waiting time, and success/failure outcome
  - direction:
    - keep semantic tool identity in the shared protocol projection
    - render tool cards as a single updating card per `tool_call_id`
    - let WebUI add local elapsed-time animation for waiting cards instead of exposing raw term/detail in the main stream

# 2026-06-28 launchd fixed-port daemon bootstrap root cause
  - root cause:
    - `freehand-daemon serve --agent master` uses `RuntimeCommandDispatcher::from_default_config()`
    - that path requires `HOME` to resolve `~/.freehand/config.toml`
    - launchd environment did not provide `HOME`, so daemon bootstrap failed before bind even though the process itself remained alive briefly
  - fix direction:
    - launchd install must inject explicit `HOME` into both `daemon.env` and plist environment

# 2026-06-28 launchd restart readiness closeout
  - observed failure mode:
    - `scripts/install-launchd.sh restart` returned before the daemon was actually ready for `GET /health`
    - immediate curl after restart could fail even though launchd had already kicked the service
  - root cause:
    - startup window race, not a fixed-port or ADP protocol regression
  - fix:
    - `scripts/install-launchd.sh install` and `scripts/install-launchd.sh restart` now wait for `/health` readiness before reporting success
  - validation:
    - `scripts/install-launchd.sh restart`
    - bounded poll reached `health_ready_after=2`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - `freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok ... subscription_accepted ... query_result ... ingress_command_kind_mismatch`

# 2026-06-28 ADP success/failure sample closeout
  - added CLI/headless sample command:
    - `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample success`
    - `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure`
  - WebUI composer now has `Success sample` and `Failure sample` buttons that load the same prompts; actual submit still uses normal ADP command path
  - verification:
    - `cargo test -p freehand-cli` -> 11 passed
    - `cargo test -p freehand-server` -> 11 passed
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci` -> EXIT 0
    - `scripts/install-global.sh` -> EXIT 0
    - `scripts/install-launchd.sh restart`
    - installed fixed-port success sample -> `runtime-turn-21`, `terminal_status=Success`
    - installed fixed-port failure sample -> `runtime-turn-22`, `terminal_status=Failed`

# 2026-06-28 WebUI tool card/status repair
  - root cause: whole-turn projection could duplicate same `tool_call_id` as separate waiting activities, and WebUI rendered tool summaries as static cards without stable tool identity
  - fix:
    - `ui.protocol` now upserts duplicate tool calls by `tool_call_id`
    - public tool summaries carry `tool_call_id`
    - WebUI normalizes same-tool cards by `tool_call_id`, adds waiting animation, clears composer immediately on submit, and routes command status through one renderer
  - validation passed:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol`
    - `cargo test -p freehand-server`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
    - live local smoke: `freehand-server webui-serve-smoke --bind 127.0.0.1:4062`, `curl /` returned WebUI shell containing composer/status elements

# 2026-06-28 review/launchd/ui projection closeout
  - verified after fixes:
    - runtime final multi-round projection now aggregates only cross-round `tool_calls` and `tool_results`
    - final visible text / usage / errors / terminal status come from the final round
    - WebUI debug 404 now renders as `debug pending`; SSE transport errors now render as `debug stream reconnecting`
    - launchd wrapper requires explicit `FREEHAND_DAEMON_BIN` and fails on prefix mismatch instead of silently running an old binary
  - validation passed:
    - `cargo test -p freehand-runtime`
    - `cargo test -p freehand-server`
    - `cargo test -p xtask`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`

- 2026-06-27 launchd global daemon install closeout
  - added service scripts:
    - `scripts/freehand-daemon-launchd.sh`
    - `scripts/install-launchd.sh`
    - `scripts/uninstall-launchd.sh`
  - real install executed: `scripts/install-launchd.sh` exit 0
  - installed real commands:
    - `~/.local/bin/freehand-cli`
    - `~/.local/bin/freehand-server`
    - `~/.local/bin/freehand-daemon`
    - `~/.local/bin/freehand-daemon-launchd`
  - LaunchAgent installed:
    - label `com.freehand.daemon`
    - plist `~/Library/LaunchAgents/com.freehand.daemon.plist`
    - env `~/.freehand/daemon.env` mode 0600
    - logs `~/.freehand/logs/daemon.stdout.log` and `~/.freehand/logs/daemon.stderr.log`
    - fixed WebUI `http://127.0.0.1:4041/`
    - `RunAtLoad=true`, `KeepAlive=true`
  - verified active daemon:
    - launchctl showed `pid = 55614`, then exact PID killed to verify KeepAlive
    - launchd restarted it as `pid = 65923`, `runs = 3`
    - `curl /health` -> 200 `ok`
    - `curl /` -> 200, 5040-byte Freehand WebUI HTML
  - stdout log contains `freehand-daemon listening on http://127.0.0.1:4041`
  - permission note: localhost bind needs no macOS Accessibility/Full Disk permission; changing bind to LAN/Tailscale may trigger one-time firewall prompt
  - reinstall behavior note: `scripts/install-launchd.sh install` intentionally re-copies host binaries via `scripts/install-global.sh`, so repeated reinstall can make macOS re-evaluate the daemon binary; ordinary restarts must use `scripts/install-launchd.sh restart` and should not rewrite install state

- 2026-06-27 release/global-install/daemon startup closeout
  - added release truth: `scripts/release.sh` runs `make ci`, Android JVM tests, Rust release binaries, Android release APK, and artifact staging
  - added global install truth: `scripts/install-global.sh` installs `freehand-cli`, `freehand-server`, `freehand-daemon` to `${FREEHAND_PREFIX:-$HOME/.local}/bin`
  - Android `assembleRelease` repeatedly hung/failed in Lint Vital (`lintVitalAnalyzeRelease`, `lintVitalRelease` missing intermediate files); fixed at config owner by `lint { checkReleaseBuilds = false }` in `apps/freehand-android/app/build.gradle.kts`
  - release script uses Gradle `--no-daemon` to avoid persistent release-script child process leakage
  - verified release script exit 0 with staged artifacts: `freehand-cli`, `freehand-server`, `freehand-daemon`, `freehand-android-release-unsigned.apk`
  - verified install-global exit 0 with temp `FREEHAND_PREFIX`; installed binaries executable
  - verified installed daemon startup with temp `~/.freehand/config.toml`: `freehand-daemon serve --agent master --bind 127.0.0.1:4059`, `/health` 200 `ok`, `/` 200 with WebUI HTML
  - config smoke lesson: first local topology requires paired agents to resolve the same pair token value; separate env names with different values fail bootstrap explicitly

- 2026-06-27 android-client doc alignment pass
  - current truth: Android scaffold already exists under `apps/freehand-android`
  - live render host: `apps/freehand-android/app/src/main/assets/bridge.html`
  - design preview: `apps/freehand-server/assets/mocks/android/mobile-mock.html`
  - plan update: align design / execution / testing docs to the real native shell + protocol-only bridge split

- 2026-06-24T08:00+08:00 android-client execution plan locked
  - reviewed: `apps/freehand-android/` (existing scaffold) vs `apps/freehand-server/assets/mocks/android/mobile-mock.html` (locked design)
  - gap: WebView loads crude `mobile-shell.html`; no SSE; `TimelineProjector` only handles a tiny subset
  - plan doc: `docs/design/android-client-v1-android-shell.md`
  - execution order:
    1. bundle mobile-mock.html+css into Android assets; flip WebView loadUrl
    2. add SSE subscribe to ProtocolClient
    3. expand TimelineProjector to full ui.protocol mapping
    4. JS bridge: snapshot from projector -> window.__freehand.applySnapshot
    5. wire native controllers to projector state
    6. command ingress + cancel via existing CommandIngress
    7. theme: dark/light via Android night mode
    8. local.properties for SDK path
    9. compile + adb install
    10. run integration smoke against running freehand-daemon
  - hard constraints unchanged: no direct reason/provider/node imports; only ui.protocol consumer + command ingress

## 2026-06-24T11:35:57.426Z stopless learned

- requestId: openai-responses-XLC.key1-glm-5.2-20260624T193523692-397984-479
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: Android 客户端 milestone 已完成：APK 编译通过，SSE 协议客户端+WebView 渲染壳+native 控制器全部就位，freehand-daemon SSE 路由验证通过。剩余 applySnapshot JS 函数需在 HTML 中补齐（下一步）。
- evidence: 1. assembleDebug BUILD SUCCESSFUL, app-debug.apk 6.4M 2. curl http://127.0.0.1:4040/ui/subscribe/turn/latest → 200 text/event-stream 3. SseEventStream.kt, TimelineProjector.kt, MainActivity.kt, HostConfig.kt 全部代码已就位

OkHttp 4.12 SSE 需要 okhttp-sse 单独 artifact；Android buildFeatures 没有 webView flag；HostConfig URL 必须与 freehand-server 真实路由对齐（latest-active-turn 不是 turn/latest）；emulator 在 exec_command PTY 退出后会被杀，无法在当前环境维持长跑

## 2026-06-24T13:04:05.537Z stopless learned

- requestId: openai-responses-XLC.key1-glm-5.2-20260624T210335356-398243-738
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: ADB device 100.104.163.65:5555 connection refused; pairing code expired. Need new pairing code from device.
- evidence: adb connect 100.104.163.65:5555 -> Connection refused; adb pair -> protocol fault

Gradle 9.6 incompatible with AGP 8.2.2; must pin Gradle 8.7 via wrapper. ADB pairing codes expire quickly.

## 2026-06-24T13:40:03.410Z stopless learned

- requestId: openai-responses-XLC.key1-glm-5.2-20260624T213913294-398413-908
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: ADB 100.104.163.65:5555 connection refused, device wireless debug likely off. Need user to re-enable. Meanwhile fixing Gradle build issue (Gradle 9.6 incompatible with AGP 8.2.2, reverting to Gradle 8.7). Also need to sync all code changes, rebuild APK, then install once device is online.
- evidence: nc -zv 100.104.163.65 5555 -> Connection refused. adb connect 100.104.163.65:5555 -> failed. Device PLZ110 was previously connected via adb pair but connection lost after adb server restart.

ADB wireless debug ports expire; always need fresh connect. Gradle 9.6 incompatible with AGP 8.2.2.

## 2026-06-25T00:05:00+08:00 android-client tailscale-first closeout

- Android app no longer ships demo/mobile-shell assets as runtime truth.
- removed dead assets:
  - `apps/freehand-android/app/src/main/assets/mobile-mock.html`
  - `apps/freehand-android/app/src/main/assets/mobile-shell.html`
  - `apps/freehand-android/app/src/main/res/layout/activity_main.xml`
- runtime UI narrowed:
  - native side keeps the single input bar
  - drawer keeps only connection settings
  - fake session / agent quick-switch actions removed
- connection truth changed to tailscale-first:
  - bundled config host = `100.66.1.82`
  - bundled profile = `tailscale-main`
  - upgrade URLs switched to `100.66.1.82:4040`
  - `autoLanScan = false`
  - `HostStore.DEFAULT_HOST = 100.66.1.82`
  - old persisted localhost / `192.168.*` values are overridden by `MainActivity.selectPreferredHost()`
- build truth:
  - stale gradle wrapper lock at `~/.gradle/wrapper/dists/gradle-8.7-bin/.../gradle-8.7-bin.zip.lck`
  - removed stale lock, downloaded gradle 8.7 successfully
  - `cd apps/freehand-android && ./gradlew assembleDebug --no-daemon` -> BUILD SUCCESSFUL
- install truth:
  - `adb -s 100.104.163.65:5555 install -r .../app-debug.apk` -> Success
- packaged config truth:
  - `unzip -p apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk assets/config/client.json`
  - confirms host `100.66.1.82`, profile `tailscale-main`, `autoLanScan=false`
- remaining runtime evidence gap:
  - device screenshots are still covered by lockscreen / black overlay
  - `dumpsys window` shows `com.freehand.android/.ui.MainActivity` as focused app/task, but screenshot evidence is not user-visible UI yet
  - this blocks final visual acceptance, not code/build/install truth

## 2026-06-24T23:59:36.327Z stopless learned

- requestId: openai-responses-orangeai.key1-glm-5.2-20260625T075914215-399715-2210
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: 闪退根因已修，app 已不闪退。但截图仍然是黑屏（设备锁屏层覆盖），且 daemon 未运行在 Tailscale 地址上，Android 连不上 SSE。下一步：启动 daemon 绑 0.0.0.0:4040，再截图验证 bridge.html 渲染。
- evidence: 1. SecurityException crash stacktrace: DaemonDiscovery.getWifiIp -> WifiManager.getConnectionInfo -> ACCESS_WIFI_STATE permission missing. 2. 新 APK 已重装并验证不闪退: pid=26255 alive, focusedApp=com.freehand.android. 3. APK 内 config 确认 host=100.66.1.82 profile=tailscale-main. 4. daemon 未运行在 0.0.0.0:4040，设备 SSE 连接返回空. 5. 截图 18KB 黑屏是设备锁屏层覆盖不是 app 问题.

DaemonDiscovery 的 scanLan 路径在 autoLanScan=false 时仍然可以通过 health check 失败 fallback 触发。必须在 discover() 入口就拦截，不能依赖 config flag。另外 Android 新安装的 APK 需要重新 install，不能假设 build 产物已部署。

## 2026-06-25T13:08:00+08:00 android-client protocol and daemon truth closed

- Android bundled daemon truth is now unified to `100.66.1.82:4041`.
- Removed dead Android discovery owner:
  - deleted `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonDiscovery.kt`
- Fixed Android command ingress protocol shape:
  - old wrong payload: `{"type":"SubmitUserInput","text":"..."}`
  - canonical payload now matches `UiCommand` serde external-tag form:
    - `{"SubmitUserInput":{"text":"..."}}`
    - `{"CancelLatestActiveTurn":{}}`
- Fixed old persisted-host override gap:
  - same Tailscale host + legacy port `4040` now upgrades to bundled `4041`
- Runtime truth verified on real daemon process:
  - `env FREEHAND_PAIR_TOKEN_SHARED=devpair target/debug/freehand-daemon serve --agent master --bind 127.0.0.1:4041`
  - `curl http://127.0.0.1:4041/health` -> `200 ok`
  - `curl http://127.0.0.1:4041/ui/query/latest-active-turn` after submit returns submitted turn projection
  - `curl -sN http://127.0.0.1:4041/ui/subscribe/turn/latest` emits canonical `event: turn`
  - submitted prompt `reply with one short sentence and valid freehand completion schema` completed with `terminal_status=Success`
- Android build truth reverified:
  - `cd apps/freehand-android && ./gradlew assembleDebug --no-daemon` -> BUILD SUCCESSFUL
- Remaining device-side blocker:
  - `adb connect 100.104.163.65:5555` -> `failed to authenticate`
  - TCP `5555` is reachable, but host cannot currently reinstall APK or capture fresh runtime logs until device re-authorizes ADB.

Current real root cause split:
- earlier `connected + daemon unreachable` was app-side premature connected-state mutation plus wrong port collision (`4040` hitting `fin`)
- current Android command failure root cause was protocol payload mismatch, now fixed

## 2026-06-26 数据/控制 分离审计 + MetadataKind 死变体清理

### 审计结论（见上）

### 死变体清理 - 已完成 commit 5eae53e

### provider adapter error 接入 metadata 中心化 - 已完成 commit e4542f7

用户指令：Gap 2 closure path 第 1-2 步。

设计决策：
- provider adapter crates 保持 protocol-only，不加 freehand-metadata 依赖
- metadata 写入发生在 runtime bridge 的 executor 错误返回路径（单次 + 流式）
- 新 pipeline node: `RuntimeLive05ProviderError`（MetadataKind::Provider）
- 新 helper: `record_provider_error_metadata` + `emit_provider_error_debug`
- 白盒测试: HTTP 500 → metadata ledger 写入 RuntimeLive05ProviderError

当前 gap 状态：
- provider error ✓（RuntimeLive05ProviderError）
- 请求构造成功路径 — 依赖 RuntimeLive02ProviderRequestBuilt（已有）
- 响应解析成功路径 — 依赖 raw capture callback（已有可观测性）
- OpenAI executor — 当前无 executor，未来接入时复用 RuntimeLive05ProviderError 模式

用户指令：物理删除 `MetadataKind::Control` + `MetadataKind::DebugLink`（两个变体生产代码 0 次使用）。

用户要求：审计当前"推理与请求响应生命周期"中数据链 vs metadata 控制流的隔离状态。
范围：只读 audit。无代码改动。

### A. 核心结构已就位

- `crates/freehand-metadata/src/lib.rs` 是 metadata 唯一 owner
  - `MetadataCenter` (in-memory) + `MetadataLedger` (durable JSONL)
  - `MetadataWriteOwner` / `MetadataWriteNode` / `MetadataSubject` / `MetadataEntry` / `MetadataEnvelope` / `MetadataKind`
  - `validate_metadata_envelope` 强制 owner/node/subject + 拒绝 request-like key (`request.*`/`payload.*`/`prompt.*`/`input.*`/`content`/`text`/`messages` 等)
  - `is_reserved_request_key` 在 rust 字符串层做白名单，是元数据与请求数据硬隔离的第一道闸
- `crates/freehand-debug/src/lib.rs` 是 debug 唯一 owner
  - `DebugHub` + 3 类 sink (Memory / Stdout / File JSONL / Replay)
  - 独立 `DebugObservationFailure` 流 (`DebugHub::subscribe_failures`)
  - 观测-only，禁止承载请求内容
- `crates/freehand-contracts/src/lib.rs` 持有请求节点类型 (`ReasonReq01..05`, `ReasonResp01..03`, `ErrorErr01`)
- 静态 gate: `xtask/src/main.rs::verify_data_control_boundaries`
  - 拒绝 `ReasonReq*` 携带 metadata/debug/control 字段或类型
  - 拒绝 metadata owner struct 携带 request payload 字段
  - 拒绝 metadata owner struct 携带 control execution payload (cancel token / retry / checkpoint / route policy / gate decision)
  - 拒绝 `Metadata*` 类型出现在 `crates/freehand-metadata` 之外
  - 红测在 `cargo test -p xtask`

### B. 中心化元数据写入路径（已落实）

- 单例 ledger 路径: `~/.freehand/ledgers/metadata/<agent_id>/<session_id>.jsonl`
  - 由 `crates/freehand-runtime/src/lib.rs::metadata_ledger_path` 唯一生成
  - 没有第二处拼路径的代码（`metadata_ledger_path` 仅在 `freehand-runtime` 内出现）
- 唯一写入 helper: `write_live_bridge_metadata` (in `freehand-runtime`)
  - 构造 `MetadataWriteOwner`（`feature_id="provider.reason-live-bridge"`, `crate_name="freehand-runtime"`, `symbol_path` 由 spec 传入）
  - 构造 `MetadataWriteNode`（`pipeline_node` 显式标注：`RuntimeLive01RestoreResolved` / `RuntimeLive02ProviderRequestBuilt` / `RuntimeLive03ToolExecuted` / `RuntimeLive04TurnClosed`）
  - 入参是 `RuntimeMetadataWriteSpec`，不是裸 string/JSON
- 已经接入 metadata 中心化的 producer（按写入次数统计）：
  - `freehand-reason` (`ReasonTurnEngine::write_metadata`): 2 处 (`start_turn` + `apply_provider_output`)
  - `freehand-runtime` (`write_live_bridge_metadata`): 5 个 pipeline_node 节点
  - `freehand-node` (`LocalNodeRuntime::write_metadata`): 6 个节点
- debug producer 与 metadata 中心化 producer 数量级相同：
  - `freehand-reason` (`emit_debug`): 14 处生命周期点
  - `freehand-runtime` (`emit_live_bridge_debug`): 5 个 pipeline_node 节点
  - `freehand-node` (`emit_debug`): 6 个节点
- metadata write failure 在所有三个 producer 都是显式错误（`MetadataWriteFailed` / `NodeRuntimeError::MetadataWriteFailed` / `RuntimeLiveBridgeError::MetadataFailed`），无 fallback 吞错

### C. 隔离现状 — 已锁住

- `MetadataKind` 在生产代码只使用 4 个变体：
  - `RuntimeState` 10 次
  - `Routing` 5 次
  - `Cache` 4 次
  - `Provider` 2 次
  - `Control` 0 次
  - `DebugLink` 0 次（声明了但无生产 producer，参见 F）
- `metadata_ledger_path` 是 metadata 持久化的唯一真源路径生成点
- `MetadataCenter::by_trace` 是当前唯一的 metadata 查询接口（按 trace_id 反查）
- 测试覆盖：metadata ledger append/reload、corrupt ledger reject、validation-failed ledger reject、metadata write failure 不污染 turn truth（reason/node 两边都锁）

### D. 已记录的 gap（`docs/architecture/architecture-gaps.md`）

- Gap 2 明确：`metadata.core` 的 provider/debug producer 未全覆盖
  - 未接 producer：`freehand-provider-anthropic` / `freehand-provider-openai` / `freehand-debug`
  - 状态：非违规，gate 不会拦，closure path 已写在 gap 文件里

### E. 用户原则对照

1. "数据链与 metadata 控制流要分离" — 已落实：metadata 中心独立 crate + 静态 gate + 类型级禁止（`is_reserved_request_key` + `xtask::is_forbidden_request_field_*`）
2. "metadata 需要统一中心管理，不能零散写" — 已落实：唯一 ledger 路径 = `metadata_ledger_path`；唯一 helper = `write_live_bridge_metadata`；3 个 producer 都使用同一中心；没有第二份 metadata owner struct
3. "需要写入记录" — 已落实：ledger append-only JSONL 持久化 + `load_records` 回放 + `by_trace` 查询 + 静态 metadata/request gate 把"想散写"的尝试拦在编译期

### F. 待办（更新于死变体删除 + Gap 1/2 closure 后）

1. ~~**`MetadataKind::DebugLink` 与 `Control` 是死变体** — 已物理删除（commit 5eae53e）~~
2. **provider 成功路径 metadata 写入**（Gap 2 剩余项）— RuntimeLive02ProviderRequestBuilt 前补充请求构造验证 metadata / 响应解析成功路径。当前已覆盖 error 路径，成功路径有 raw capture 兜底。优先级低。
3. **`MetadataCenter` 查询接口单一**（`by_trace` 之外）— 没有 `by_owner` / `by_kind` / `by_node` 维度。当前审计只能 grep `MetadataKind::`，多 producer 写入的可观测性受限于 trace_id 单一维度
4. **MetadataCenter 是 `Mutex<MetadataCenter>` 形式持有** — 写入串行化。`freehand-runtime` 多处持有同一个 `Arc<Mutex<MetadataCenter>>`，并发 producer 写入需要锁
5. **`verify_data_control_boundaries` 静态扫描只覆盖 `freehand-contracts` 的 `ReasonReq*`** — 不扫描 `ReasonReq*` 之外的请求节点（如 `ReasonReq04ToolCall`/`ReasonReq05ToolResultReentry`/`ReasonResp01..03`/`ErrorErr01`），不扫 `freehand-ui-protocol` 里的 `UiCommand` 是否携带 metadata/debug 字段。已知受限范围（gate 文件注释里没写）

## 2026-06-26T04:49:25.671Z stopless learned

- requestId: openai-responses-minimax.key1-MiniMax-M3-20260626T124912992-402594-347
- sessionId: 019f0212-cf1b-7003-85bb-b0ada9de6601
- stopReason: MetadataKind 死变体 Control + DebugLink 物理删除并验证完成
- evidence: git commit 5eae53e；make ci EXIT 0；enum 验证 grep -A 5 只剩 4 个变体 (Routing/Provider/Cache/RuntimeState)

死变体必须按 hard rule 10 物理删除并同步 docs+gate+tests+memory，不能靠注释保留；本次 make ci 一次通过验证 enum + gate + docs sync 是闭环的

## 2026-06-26T05:18:48.331Z stopless learned

- requestId: openai-responses-mimo.key2-mimo-v2.5-20260626T131837059-403051-804
- sessionId: 019f0212-cf1b-7003-85bb-b0ada9de6601
- stopReason: 两轮 commit 均完成，make ci EXIT 0。第一轮物理删除 MetadataKind 死变体 Control+DebugLink；第二轮 provider adapter error 接入 metadata 中心化（RuntimeLive05ProviderError）。docs/function-map/mainline-test/gap 同步更新。
- evidence: git log: 5eae53e (delete dead variants) + e4542f7 (provider error metadata)。make ci EXIT 0。cargo test: 324 passed。白盒测试 live_bridge_writes_provider_error_metadata_on_executor_failure 验证 HTTP 500 → ledger 写入 RuntimeLive05ProviderError

1. metadata 与 debug 保持物理隔离比通过 DebugLink 变体交叉引用更干净 2. provider adapter 保持 protocol-only 不加 metadata 依赖，metadata 写入在 runtime bridge 错误返回路径 3. RuntimeLive05ProviderError 作为 provider error 的唯一 metadata 入口可复用于未来 OpenAI executor

## 2026-06-27 UI status/tool SSE repair

- Root cause: WebUI treated `/ui/query/debug/{turn}` 404 as command failure while turn SSE can arrive before debug snapshot; debug subscribe also returned 404, so late debug could not arrive over SSE. Existing tests only covered debug-present query/SSE, not turn-before-debug race.
- Fix path: `app.webui-smoke` keeps debug HTTP query snapshot-only but makes debug SSE wait for late snapshots; WebUI renders missing debug as pending instead of command failure.
- Tool lifecycle gap: `UiTurnProjection.tool_calls` only carried names, so WebUI could only guess running. Added `UiToolActivity` plus `apply_tool_result`; `reason.turn` broadcasts `ReasonBroadcastEvent::ToolResult`; runtime maps it into UI state so latest-turn SSE carries waiting -> completed updates.
- Locked by tests: `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-server`, `cargo test -p freehand-reason`, `cargo test -p freehand-runtime`; mainline docs regenerated.

## 2026-06-28 ADP daemon control/status

- User direction: WebUI, Android, CLI, and headless automation should share ADP for status/query/control, so failures can be inspected through ADP instead of UI-specific guessing.
- Implemented `UiAdpRequest`/`UiAdpResponse` in `ui.protocol` and `/adp` WebSocket in shared server transport; daemon exposes it on fixed launchd port through the existing injected runtime dispatcher and shared `UiProtocolState`.
- Debug finding: ADP subscription must return explicit `SubscriptionAccepted`, otherwise clients cannot distinguish waiting from a dead connection; command dispatch also must not block the connection loop, or subscription status events can starve behind long provider work.
- Verified: `daemon_adp_websocket_controls_command_query_and_subscription`, `daemon_adp_rejects_query_sent_as_command_frame`, full `make ci`, global install, launchd restart, and real Node WebSocket smoke against `ws://127.0.0.1:4041/adp`.
2026-06-28: 生成 ADP unified UI closeout 计划文档 `docs/goals/adp-unified-ui-closeout-plan.md`，目标是把 WebUI / Android / CLI-headless 收口到 daemon `/adp` 统一控制面，HTTP/SSE 仅保留兼容路径，并同步补齐固定端口、后台守护、自动化验证与 docs/mainline/wiki 真源。

2026-06-28: WebUI default ADP slice landed locally. `page.rs` adds `data-adp-endpoint="/adp"`; `webui.js` now opens one ADP WebSocket and routes query/subscribe/command through `UiAdpResponse` frames. Removed default `fetch`/`EventSource` usage from WebUI live path; HTTP/SSE/POST routes remain compatibility surfaces. Verified `node --check`, `cargo test -p freehand-server`, `xtask mainlines generate/check`, `xtask gates check`, local page/JS smoke on `127.0.0.1:4073`, and local WebSocket smoke on `ws://127.0.0.1:4074/adp` with accepted/event/query/failure frames.

2026-06-28: Android default ADP slice landed locally. `MainActivity` now wires `AdpEventStream` as the default live shell transport; `HostConfig.adpUrl` / `ClientConfig.adpPath` make `/adp` explicit; `TimelineProjector.applyAdp` consumes ADP query/subscription/failure frames and projects failure visibly to `bridge.html`. `ProtocolClient` and `SseEventStream` remain compatibility classes, not default shell path. Verified Android JVM tests, `xtask mainlines generate/check`, `xtask gates check`, and `cargo test -p freehand-server android_mock_route_returns_design_preview`.

2026-06-28: CLI/headless ADP smoke landed locally. `freehand-cli adp-smoke --url ws://.../adp` uses typed `UiAdpRequest/UiAdpResponse`, sends subscribe/query/query-as-command frames, and requires accepted/event/query plus explicit `ingress_command_kind_mismatch`. Verified `cargo test -p freehand-cli` with local mock WebSocket server and a real local `freehand-server webui-serve-smoke` `/adp` smoke.

## 2026-06-28 WebUI shortcuts slash closeout

- Live failure ADP sample re-run passed on fixed daemon: runtime-turn-32-r2, rounds=2, tool_executions=1, terminal_status=Success, read_file tool_activity status=Failed; proves tool execution failure is model-visible result, not system dispatch failure.
- WebUI JS contains shortcutHelp, keydown handlers for Cmd/Ctrl+Enter, Esc, Cmd/Ctrl+R, Cmd/Ctrl+K, Cmd/Ctrl+1, Cmd/Ctrl+2; slash commands /help /sessions /reload /success /failure /cancel /clear are present and server asset smoke locks them.

- After reinstall/restart, fixed-port served JS hash matched workspace hash 8b8df0fa84b37ec7c7802ca8ce5d7c88a2859ab2c7370e3655f68230f5195379.
- Full install-global passed, launchd pid 27507 healthy on 127.0.0.1:4041.
- Live ADP smoke passed. Sequential failure sample passed as runtime-turn-33-r2 with rounds=2/tool_executions=1. Sequential success sample passed as runtime-turn-35 with terminal_status=Success.
- Verification caution: running success/failure samples concurrently can produce command_dispatch_port_failure because runtime dispatch has a single active turn boundary; do not treat parallel sample verification as valid positive evidence.
- Found and fixed WebUI slash UX bug: liveTurnStatus always overrode local commandStatus, so /help and /sessions looked inert on completed turns; slash inputs also remained in composer. Added sticky command status and slash input consumption.

## 2026-06-28 goal completion audit continuation

- Objective file re-read: `/Users/fanzhang/.codex/attachments/098e982a-74ac-494e-8e66-6ebb387506f0/pasted-text-1.txt`.
- Minimax config current evidence:
  - RCC source `/Volumes/extension/.rcc/provider/minimax/config.v2.toml` declares providerId/id `minimax`, `type=anthropic`, `baseURL=https://api.minimaxi.com/anthropic`, `defaultModel=MiniMax-M3`.
  - Runtime truth `~/.freehand/config.toml` matches `provider=minimax`, `protocol=messages`, `defaultModel=MiniMax-M3`, and master/worker agents use `provider = "minimax"`.
  - `FREEHAND_PAIR_TOKEN_SHARED=test-pair-token ~/.local/bin/freehand-cli --agent master` and source `cargo run -p freehand-cli -- --agent master` both printed `provider=minimax provider_protocol=messages default_model=MiniMax-M3 base_url=https://api.minimaxi.com/anthropic`.
- Fixed daemon evidence:
  - `launchctl print gui/$(id -u)/com.freehand.daemon` showed `state = running`, pid `27507`, `keepalive | runatload`.
  - `curl -4fsS http://127.0.0.1:4041/health` returned `ok`.
- Missing screenshot evidence added under `artifacts/webui-session-alignment/20260628-continued/`:
  - `12-before-daemon-restart-history.png`: pre-restart selected session/history state.
  - `13-after-daemon-restart-session-restored.png`: service-scoped launchd restart + reload restored `runtime-session-master` and prior latest turn `runtime-turn-36-r2`.
  - `14-webui-bash-submit-cleared-pending.png`: WebUI submit cleared composer and showed pending/dispatching state.
  - `15-webui-tool-waiting-animation.png`: WebUI showed tool waiting/running state during `bash sleep 8`.
  - `16-webui-tool-completed-after-wait.png`: WebUI showed completed tool turn after wait sample.
- Explicit gate evidence:
  - `make ci` rerun with log `/tmp/freehand-make-ci.log`, tail showed `xtask mainlines check: ok`, `xtask gates check: ok`, and `MAKE_CI_EXIT=0`.
- Current post-restart headless ADP evidence:
  - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` passed with `subscription_accepted`, `subscription_event`, `query_result`, and `ingress_command_kind_mismatch`.
  - Sequential success sample passed: `runtime-turn-38`, `rounds=1`, `tool_executions=0`.
  - Sequential failure sample passed: `runtime-turn-39-r2`, `rounds=2`, `tool_executions=1`.
  - `adp-session-query --session runtime-session-master` returned 36 ordered turns through `runtime-turn-39-r2`.
- Latest projection for `runtime-turn-39-r2` has `terminal_status=Success` and one `read_file` tool activity with `status=Failed`.

2026-06-29: Current `/new` session issue is a render-state bug, not an ADP transport bug. The draft session can be created, but empty `QuerySessionTurns` results must not clear `draftSessionId`, and the main transcript should prefer the selected session over the global latest turn. Also avoid timestamp-only draft IDs because repeated `/new` can collide inside the same second.

2026-06-29: WebUI layering contract clarified. ADP / `ui.protocol` stays stable; UI expands only in control and presentation layers. Session attachments must be session-scoped, history must use placeholders only, and draft attachments must clear on success while surviving failure for retry. New durable docs: `docs/design/webui-layered-controls-and-attachments.md` and `docs/goals/webui-layered-controls-attachments-plan.md`.

2026-06-29: WebUI layered controls first implementation landed. Controls: attach file/image/video, preview, selected-session refresh, read-only model selector. Attachment draft metadata is session-scoped in localStorage, current page `File` handles are kept for retry, restored metadata is marked metadata-only, and submitted text gets placeholder lines only. Success clears draft after ADP command receipt; ADP timeout/dispatch failure restores composer text and retains attachment draft. Evidence: `node --check`, `cargo test -p freehand-server`, `cargo test -p freehand-ui-protocol`, `xtask mainlines generate/check`, `xtask gates check`, `make ci MAKE_CI_EXIT=0`, ADP smoke on `4079`, screenshots under `artifacts/webui-layered-controls-e2e/20260629-layered-attachments-success-v2/` and `...failure-v2/`.

2026-06-29: WebUI selected-session reload evidence added under `artifacts/webui-layered-controls-e2e/20260629-layered-session-reload-v2/`; before and after reload both showed `strip-session=session-webui-smoke` and `conversation=turn-webui-smoke`.

2026-06-29: WebUI tool rendering收口继续压缩。当前 live 结论：历史 completed turn 不再闪，tool 终态只用 compact color dot 表达成功/失败；正文只保留一条核心语义线，参数或 diff 已经足够时不再重复 result summary。真实 daemon `127.0.0.1:4102` 验证成功，failure sample 截图见 `artifacts/webui-state-render-fix/20260629-compact-tool-display-live-v2/01-live-failure-sample.png`，计数为 `wait_model_continue_count=0`, `compact_tool_state_count=1`, `tool_block_count=1`, `running_state_count=0`.

2026-06-30: WebUI 新 session 工作目录选择修复。根因：cwd 协议链已存在，但 WebUI 只有 composer 底部手输 cwd，`/new`/New session 没有明确的新 session workspace 选择动作，并且无 cwd 时会静默创建 draft。修复：session rail 新增 `Workspace directory` + `Use for new session`，`/new` 和 New session 都要求先选择 cwd，draft submit 无 cwd 会显式阻断。验证：`node --check`, `cargo fmt --check`, `cargo test -p freehand-server`, `xtask mainlines generate/check`, `xtask gates check`；真实页面 `127.0.0.1:4103` 截图在 `artifacts/webui-cwd-session-e2e/20260630-new-session-workspace/`，负向 `no_cwd_status=new session requires a workspace directory`，正向 `strip_cwd=/Volumes/extension/code/freehand`，提交后 `strip_turn=runtime-turn-43` 且 cwd 保持绑定。

2026-06-30: 用户纠正 session/workspace 设计分层。正确方向：拆成基础 Agent 层和应用执行框架层。基础 Agent 负责 cwd 持久化、session 目录、全局 workspace 命名、权限、启动/生命周期、global/worker 工作模式、中断恢复；session 目录形如 `~/.freehand/sessions/local/<absolute-path-slash-to-minus>/<uuid>/`。应用层负责 WebUI/master agent 接收用户命令、理解任务、派发 worker、汇总结果；正常流程用户只对 master/global agent 对话，worker 由 master 派遣，WebUI 直连 worker 仅作为 debug 模式。此前 `afb046e` 强制 WebUI new session 必填 cwd 是应用层误修，应后续 forward-fix 删除/替换。

2026-06-30: WebUI forward-fix direction now locked: sidebar must show `New conversation` and `New task` as separate actions. `New conversation`/`/new` is global conversation and must not require cwd. `New task`/`/task` requires a visible task target cwd and uses existing ADP `CreateSession` metadata path for cwd-bound task session creation. Do not claim master-worker task dispatch is complete from this UI slice alone.

2026-06-30: Completion schema rejection feedback was too weak for non-string fields. Root cause: completion parsing treated arrays/objects in optional text fields as missing, so `evidence: []` or `{}` surfaced as `is required` instead of telling the model to emit a plain string. Fix: `crates/freehand-blocks/src/lib.rs` now reports explicit type errors for non-string completion fields, and guidance now says required text fields must be plain strings. Verified by `cargo test -p freehand-blocks -- --nocapture` and `cargo test -p freehand-reason -- --nocapture`.

2026-06-30: Completion schema retry transparency repair. Root cause: schema rejection/retry was only runtime/ledger truth, so WebUI showed generic waiting rather than schema retry state. Fix: runtime publishes `CompletionSchemaRejected`, `ui.protocol` projects it through `apply_completion_schema_retry_waiting`, and WebUI renders compact `schema retry #N: <field issue>` detail in the same turn card with elapsed timing. Verification: `cargo test -p freehand-blocks`, `cargo test -p freehand-reason`, `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-runtime`, `cargo test -p freehand-server`, `node --check apps/freehand-server/assets/webui.js`, `xtask mainlines generate/check`, `xtask gates check`, fixed-port `/health`, ADP smoke, and live ADP subscription captured schema retry detail.

2026-06-30: Tool-result continuation wait and composer history repair. Root cause: WebUI inferred waiting-for-model from completed/failed tool cards, so the animation was fake and disappeared when projections changed; dispatch failure also refilled composer text. Fix: runtime publishes `ModelContinuationWaiting` after tool results are paired for the next provider request, WebUI renders only protocol `model_request` waits, the fake `turnIsWaitingForModel` path is removed, composer stays cleared after submit/failure, and Up/Down recalls local input history.

# 2026-07-01 live reasoning state/UI round rendering repair
  - user live feedback: schema retry state sticks and can override later tool/model phases; timers should start from submit/client dispatch and every real phase must animate/time.
  - user live feedback: runtime appears to schema-reject during tool-use/incomplete-tool phases; schema retry must only run when provider normalized finish reason is stop/end_turn, and consecutive stop/end_turn rejections count only across terminal candidates.
  - user UI correction: WebUI must not merge the whole user request, all rounds, all tools, and final summary into one card. Each provider round/tool execution lifecycle should render as its own chronological card that grows downward; final summary belongs at the end, not visually above execution history.
  - implementation:
    - runtime now selects latest unexecuted tool calls per id and returns incomplete tool_use as a failed tool result re-entry instead of schema retry
    - completion schema parse/retry is gated by terminal-candidate finish reason (`stop` / `end_turn` style)
    - consecutive schema retry counter resets on tool execution / non-schema continuation
    - `UiModelRequestActivity.kind` distinguishes `Thinking`, `SchemaRetry`, and `ToolResultContinuation`
    - WebUI model wait timing is keyed by turn + typed phase + detail, so schema retry cannot stick after phase changes
    - WebUI renders chronological per-round cards; later/superseded rounds hide duplicate user prompt and show `continued`; final summary stays in the final row at the bottom
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 41 passed
    - `cargo test -p freehand-runtime -- --nocapture` -> 52 passed
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate/check`
    - `cargo run -p xtask -- gates check`
    - fixed-port install/restart: release build, install to `~/.local/bin`, `scripts/install-launchd.sh restart`, `curl http://127.0.0.1:4041/health` -> ok
    - live ADP sample: `~/.local/bin/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure` -> `rounds=2 tool_executions=1 failed_tools=1 schema_rejections=0`
    - screenshot: `artifacts/webui-reasoning-state/20260701-round-cards/04-fixed-4041-round-sequence-tall.png`

2026-07-01 latest WebUI/live regression trace:
- Latest user session `webui-session-20260701041619-de431b82` did not actually terminal-fail through schema rejection; reason ledger showed `runtime-turn-66-r10` ended with provider `finish_reason=max_tokens`, `schema_rejections=0`, and runtime incorrectly closed it as failed with `Provider ended without a completion-schema candidate: max_tokens`.
- Last-card merge root cause is runtime projection, not CSS: `project_runtime_turn_history` aggregated all same-ordinal round tool calls/results into the final round projection, and restore grouped runtime rounds by ordinal before applying one UI projection. This violates one-round/one-card.
- Forward fix direction: schema retry exhaustion must not be `Failed`; use non-failed terminal truth (`Blocked`). Provider interruption/non-candidate such as `max_tokens` must be `Interrupted`. Runtime/UI projection must keep each `runtime-turn-N[-rM]` as its own chronological card and remove WebUI `logicalExecutionKey` / `__supersededRound` grouping.
- Follow-up lock: schema repair must close both sides, not just status labels. Runtime now tests that invalid completion schema feedback is sent back to the model in the next provider request with concrete missing fields (`completion_reason`, `evidence`, `learned`), and runtime dispatch UI-state tests prove clients can query `SchemaRetry` with retry index plus missing field summaries before the repair round completes.

# 2026-07-03 WebUI online validation after Android bridge review
  - user direction: skip Android tests; verify WebUI first.
  - fixed-port service was already healthy on `127.0.0.1:4041`; workspace WebUI asset hash initially matched served asset.
  - browser automation against real WebUI found runtime JS error after submit: `modelRequestPhase is not defined`. `node --check apps/freehand-server/assets/webui.js` did not catch this because the symbol was syntactically valid but undefined at runtime.
  - implementation: added `modelRequestPhase(turn)` beside `modelRequestKind` / `modelRequestLabel`, mapping typed `model_request.kind` to `thinking`, `schema_retry`, or `tool_result_continuation`; added server asset smoke assertions for the helper definition and call site.
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo build --release -p freehand-server -p freehand-daemon -p freehand-cli`
    - installed release binaries to `~/.local/bin`
    - `scripts/install-launchd.sh restart`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - served JS hash matched workspace hash `faab159c8376736ea66fd64c9041298aca9ff0a11e13c5cda4c948ea2135b00f`
    - Playwright real WebUI submit evidence under `artifacts/webui-online/20260703-webui-after-model-phase-fix/`
    - DOM after submit had two completed execution blocks, zero live/running blocks, composer cleared, submitted prompt visible, and no `modelRequestPhase` error; remaining console error was favicon 404 only
    - `~/.local/bin/freehand-cli adp-session-query --url ws://127.0.0.1:4041/adp --session cli-adp-sample-success-1782953474447457000` -> `turns=2`, `turn_ids=runtime-turn-1,runtime-turn-6`, session status success
  - reusable validation rule: WebUI lifecycle/helper edits need browser console capture in addition to `node --check`; syntax check alone cannot prove runtime helper binding.

# 2026-07-04 dirty-tree closeout verification
  - resumed from dirty tree containing Android bridge multi-turn projection, runtime/reason terminal status and persistence sequence-lock repairs, CLI ADP transcript evidence repair, WebUI `modelRequestPhase` helper, docs/mainline/test-design updates, skill and memory updates.
  - mapped owners touched: `app.android-client`, `app.webui-smoke`, `provider.reason-live-bridge`, `reason.persistence`, `runtime.ui-command-dispatch`, `ui.protocol`, `reason.turn`.
  - local verification passed:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo test -p freehand-reason -- --nocapture` -> 57 passed
    - `cargo test -p freehand-runtime -- --nocapture` -> 56 passed
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 41 passed
    - `cargo test -p freehand-cli -- --nocapture` -> 12 passed
    - `cd apps/freehand-android && ./gradlew testDebugUnitTest` -> build successful
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
  - fixed-port install/online verification passed:
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp`
    - `~/.local/bin/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample success` -> `session=cli-adp-sample-success-1783100519808659000`, `turn=runtime-turn-1`, `rounds=1`, `tool_executions=0`, `failed_tools=0`
    - `~/.local/bin/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure` -> `session=cli-adp-sample-failure-1783100523482624000`, `turn=runtime-turn-2-r2`, `rounds=2`, `tool_executions=1`, `failed_tools=1`
  - browser evidence:
    - live submit path saved `artifacts/webui-online/20260704-closeout-live-submit/01-before-submit.png`, `02-after-submit.png`, `03-after-wait.png`, `04-terminal-after-wait.png`, `browser-state.json`, `terminal-browser-state.json`
    - live browser submit proved prompt `webui closeout browser proof 1783101772921` became visible, composer cleared, and no page errors occurred; favicon 404 was the only console error.
    - same live browser prompt remained `waiting_model` after later ADP query, so it is not terminal proof and must not be used as completion evidence.
    - terminal sample proof saved `artifacts/webui-online/20260704-closeout-terminal-samples/cli-adp-sample-success-1783100519808659000.png`, `cli-adp-sample-failure-1783043566126808000.png`, and `terminal-samples-browser-state.json`
    - terminal sample browser states showed both selected sessions visible, composer cleared, `liveCount=0`, and completed final cards.
    - ADP query confirmed terminal sample sessions: success `cli-adp-sample-success-1783100519808659000` had `turns=1`, `turn_ids=runtime-turn-1`; failure `cli-adp-sample-failure-1783043566126808000` had `turns=1`, `turn_ids=runtime-turn-1-r2`.
  - remaining risk:
    - live freeform browser prompt `cli-adp-sample-failure-1783100523482624000` stayed `waiting_model`; not part of the passed terminal sample proof.
    - Android live APK/WebView was unit/release-built but not device-installed in this closeout.

# 2026-07-04 new-session lifecycle E2E test
  - user acceptance focus: input history recall + append, full transcript, client/provider/tool lifecycle timing and animation, semantic tool result projection, one turn card per lifecycle, no merge, and color semantics where success is green, failed is red, running is blue.
  - route/owner surface: `app.webui-smoke`, `runtime.ui-command-dispatch`, `provider.reason-live-bridge`, `reason.persistence`, `ui.protocol`, `tool.display`.
  - setup:
    - service-scoped restart via `scripts/install-launchd.sh restart`
    - fixed health `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - browser artifacts: `artifacts/webui-online/20260704-new-session-lifecycle-e2e-1783130843077/`
  - new WebUI session: `webui-session-20260704020727-d1b9081f`
  - marker: `fh-e2e-1783130848405`
  - flow evidence:
    - `01-new-conversation-clicked`: new draft session visible.
    - `03-first-after-submit`: first prompt immediately visible, composer cleared, `runtime-turn-1`, `liveCount=1`, dispatching timer visible.
    - `05-first-terminal`: first turn completed, marker visible, `liveCount=0`.
    - `06-second-recalled-and-appended`: ArrowUp recalled first input and composer appended `SECOND_LAYER_CONTEXT`.
    - `07-second-after-submit`: second prompt visible as a separate turn, first turn still visible, `liveCount=1`.
    - `09-second-terminal`: second turn completed, both marker and appended phrase visible.
    - `11-tool-after-submit` / `12-tool-running-00`: tool turn submitted and running state captured.
    - `13-tool-terminal`: bash/pwd tool result projected semantically as `Read current working directory` and `current workspace`, not raw JSON.
    - `15-failure-after-submit` / `16-failure-tool-state-00` / `17-failure-continuation-01`: missing-file read tool failure projected semantically with path, then `thinking after tool result... 0s` continuation captured.
    - `18-failure-terminal`: final continuation turn completed successfully; tool failure did not become command failure.
    - `19-after-refresh`: browser refresh preserved 6 cards and full history.
    - `20-after-daemon-restart-restore`: daemon restart + browser restore preserved `cardCount=6`, `liveCount=0`, marker, second-layer text, and missing path.
  - ADP evidence:
    - `~/.local/bin/freehand-cli adp-session-query --url ws://127.0.0.1:4041/adp --session webui-session-20260704020727-d1b9081f`
    - returned `selected_session=webui-session-20260704020727-d1b9081f`, `turns=6`, `turn_ids=runtime-turn-1,runtime-turn-2,runtime-turn-3,runtime-turn-3-r2,runtime-turn-4,runtime-turn-4-r2`, session status `success`.
  - passed:
    - new session was independent.
    - input history recall + append worked.
    - first and second user inputs stayed visible in order.
    - refresh and daemon restart restored same 6-turn transcript.
    - tool result projection was semantic for pwd and missing read_file.
    - tool failure returned to model and final state succeeded.
    - no browser `pageerror`; only favicon 404 console error observed.
  - failed acceptance:
    - running card color is not blue. Captured running cards have `className=dialog-block execution-block running-state` but `borderLeftColor=rgba(31, 108, 88, 0.44)` and pulse CSS is amber, not blue.
    - completed success card computed `borderLeftColor=rgb(31, 33, 30)`, not a green frame in computed CSS.
    - tool-execution precursor turns (`runtime-turn-3`, `runtime-turn-4`) restore as `pending-state` / `WAITING` after continuation terminal, leaving 2 pending cards in a fully terminal transcript (`20-after-daemon-restart-restore`: `successCards=4`, `pendingCards=2`, `failedCards=0`, `runningCards=0`).
    - no red turn card was produced for tool-failure precursor card; the failed tool row was visible, but the owning turn card stayed pending/waiting and final continuation card was success.
  - conclusion: lifecycle/content/history proof is mostly closed, but color semantics and non-terminal precursor-card lifecycle projection do not meet the user's acceptance.

# 2026-07-04 lifecycle color + restart continuation repair
  - implementation:
    - WebUI inactive tool precursor cards now derive lifecycle from protocol tool status: completed/success tools render as success cards, failed tools render as failed cards.
    - WebUI execution cards now have explicit state borders: running blue `rgb(47, 111, 237)`, success green `rgb(23, 107, 85)`, failed red `rgb(178, 72, 62)`.
    - Runtime live bootstrap now initializes `next_turn_ordinal` from the maximum persisted `runtime-turn-N` ordinal across all sessions, not only the default runtime session.
  - regression lock:
    - `live_restore_resumes_turn_ordinal_from_selected_non_default_session` creates a non-default WebUI-style session, restarts runtime dispatch, submits again to that session, and requires `runtime-turn-1`, `runtime-turn-1-r2`, `runtime-turn-2`, `runtime-turn-2-r2` without ID reuse.
  - validation:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-runtime live_restore_resumes_turn_ordinal_from_selected_non_default_session -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
  - online evidence:
    - initial color restore proof: `artifacts/webui-online/20260704-lifecycle-color-fix-1783132581/summary.json`
    - full clean-session proof: `artifacts/webui-online/20260704-full-fix-e2e-1783133697/summary.json`
    - full proof session: `webui-session-20260704025459-e136d862`, marker `fh-fix-1783133697681`
    - refresh after 4 logical requests: `cardCount=6`, `successCards=5`, `failedCards=1`, `pendingCards=0`, `liveCount=0`, marker/SECOND_LAYER_CONTEXT/missing path/pwd semantic all present.
    - daemon restart restore: same `cardCount=6`, `successCards=5`, `failedCards=1`, `pendingCards=0`, `liveCount=0`; failed `read_file` precursor card red `rgb(178, 72, 62)`, completed `pwd` precursor card green `rgb(23, 107, 85)`.
    - running captures for second, pwd, and failed-tool submits all blue `rgb(47, 111, 237)` with live animation.
    - post-restart continuation plus second restart: final restore `cardCount=7`, `successCards=6`, `failedCards=1`, `pendingCards=0`, `liveCount=0`, latest `runtime-turn-11`.
    - ADP truth: `webui-session-20260704025459-e136d862:7:success`, `turn_ids=runtime-turn-7,runtime-turn-8,runtime-turn-9,runtime-turn-9-r2,runtime-turn-10,runtime-turn-10-r2,runtime-turn-11`.
# 2026-07-04 WebUI duplicate/sample-button follow-up

- owner: `app.webui-smoke`
- user issue: selected-session chat view showed repeated assistant/final-looking cards and persistent `Success` / `Failure` composer buttons.
- implementation:
  - removed persistent sample buttons from `render_webui_smoke`, DOM bindings, click handlers, and CSS.
  - kept success/failure diagnostics only through `/success`, `/failure`, and keyboard shortcuts.
  - hardened `uniqueChatFragments()` with adjacent assistant visible-text de-duplication so duplicate assistant/final fragments from transcript/latest-turn refresh races do not render twice.
  - updated `app.webui-smoke` function map and test design to lock "no persistent sample buttons".
- verification:
  - `node --check apps/freehand-server/assets/webui.js`
  - `cargo test -p freehand-server -- --nocapture` -> 11 passed
  - `scripts/install-launchd.sh installS`, `curl -4fsS http://127.0.0.1:4042/health`, `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
  - served/workspace JS hash matched `fc95d9906aa760d16735c23711735498a6a710ba6e138e13a95b60a07b805ef5`
  - Playwright artifact `artifacts/webui-online/20260704-duplicate-buttons-followup-1783144225304/summary.json`: no `success-sample-button`/`failure-sample-button`, no visible `Success`/`Failure` buttons, no adjacent duplicate assistant cards in the reported cross-workspace session.
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- follow-up gap for user item 3:
  - current framework has session cwd binding and `SendDirectMessageToSlave`, plus `LocalNodeRuntime::delegate_task` status projection, but no protocol-owned "create task and dispatch to subagent for outside-workspace target" command/lifecycle.
  - current cross-workspace path failure therefore reaches provider/tool blocked/error semantics instead of a framework routing decision.

# 2026-07-04 agent-framework task-control correction

- user correction: Freehand is an agent framework, so the framework must remain passive. It defines status fields, prompt instructions, schemas, tags, retry policy, built-in action tools, and state transitions; the model returns explicit status fields and calls explicit tools; the framework starts/stops/switches flows only from accepted status/action truth, not from guessed request semantics.
- design correction:
  - cross-workspace/task/subagent routing must be model-status-plus-tool driven, not runtime NLP/path guessing.
  - status schema should live inside a hard invisible block such as `<<<freehand_status>>>...<<<\/freehand_status>>>`.
  - UI/public projection strips the status block from display.
  - incomplete/bad model schema triggers repair/retry through schema feedback; framework may structurally normalize compatible formats but must not infer missing task semantics.
  - task side effects use compact built-in tools, preferably one `task` tool with operation arguments; framework executes validated action-tool calls and reports status back through protocol truth.
- follow-up correction: task/subagent status and action signals are metadata/control truth, not data payload. Parsed status blocks and task tool calls must write to `metadata.core` / control center with explicit watermark provenance before any task lifecycle mutation. Control signals must not be passed through request/user-visible data chains or embedded into task input. Every write needs writer owner, pipeline node, schema version, status/action, source model/agent/turn, timestamp, validation status, and error/retry trace so bad control decisions can be audited and replayed.
- follow-up correction: flow rhythm and errors need explicit centers. New design doc `docs/design/control-error-center-refactor.md` and architecture gap entry define planned `control.center`, `error.center`, and `task.orchestration`; local runtime/provider/tool retry/fail/block decisions must move behind a centralized metadata-watermarked error policy before the task/subagent refactor.
- follow-up correction: model feedback has two channels. Status schema is no-side-effect interaction state used for reasoning rhythm and UI status; side effects must use compact built-in action tools, preferably one `task` tool with `op=create|dispatch|append|stop|close|query`, maximum three framework tools total. Status schema can allow simple stop, task-complete-with-evidence terminal, next-step continuation, blocked terminal, user-option stop, and schema repair feedback when required fields are missing.
- follow-up correction: first implementation must be a four-point hook skeleton, not a full-flow hook chain. Raw request-side checks needing the most precise local tool/result data mount after local tool result; outbound controls mount before model request send; raw response processing mounts immediately after model response receive; final client-return processing mounts after all processing immediately before returning to the client. No schema/action implementation may bypass these hook points.

# 2026-07-04 error-center ADP read surface closeout

- owner slice: `error.center`, `ui.protocol`, `runtime.ui-command-dispatch`, `app.runtime-daemon`, `app.cli-runtime-smoke`, `app.webui-smoke`, plus `foundation.workspace` for S profile restart repair.
- implementation already present on resume:
  - `UiCommand::QueryErrorCenterEvents` / `SubscribeErrorCenterEvents`.
  - `UiQueryResult::ErrorCenterEvents` / `UiProjection::ErrorCenterEvents`.
  - runtime `RuntimeCommandDispatcher::query_runtime` route to `query_error_center_events_for_ui`.
  - server ADP query and initial subscription projection support.
  - daemon black-box test and CLI `adp-error-query`.
- documentation closeout:
  - synced `error.center`, `runtime.ui-command-dispatch`, `ui.protocol`, `app.cli-runtime-smoke`, `app.runtime-daemon`, `app.webui-smoke`, and `foundation.workspace` function maps/test designs/mainline JSONs.
  - regenerated generated wiki from mainline JSON truth.
- S profile runtime gap found:
  - `restartS` previously only kickstarted launchd; because launchd runs copied `freehand-daemonS-bin`, daemon could keep stale code while `freehand-cliS` symlink used current code.
  - fixed `scripts/install-launchd.sh restartS` to run `scripts/install-symlink.sh` before `launchctl kickstart`, refreshing debug binaries and the launchd daemon copy without touching global service.
  - updated `.agents/skills/freehand-dev/SKILL.md` to preserve the stale-code diagnostic rule.
- verification:
  - `cargo fmt --check`
  - `cargo test -p freehand-control -- --nocapture` -> 5 passed
  - `cargo test -p freehand-runtime runtime_query_reads_error_center_metadata_without_raw_text -- --nocapture`
  - `cargo test -p freehand-daemon daemon_adp_queries_runtime_error_center_truth -- --nocapture`
  - `cargo test -p freehand-cli -- --nocapture` -> 12 passed
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 43 passed
  - `cargo test -p freehand-server -- --nocapture` -> 11 passed
  - `cargo test --workspace` -> 423 passed
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `bash -n scripts/install-launchd.sh scripts/install-symlink.sh scripts/freehand-daemon-launchd.sh`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `cargo test -p xtask` -> 18 passed
  - `make ci` -> exit 0
- online S-profile proof:
  - `scripts/install-launchd.sh installS`, health `ok`, `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`.
  - before `restartS` fix, `adp-error-query` timed out because daemon copy was stale; after `installS` refresh and script fix, query passed.
  - real failure sample: `freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample failure` -> session `cli-adp-sample-failure-1783177452885366000`, turn `runtime-turn-37-r2`, `rounds=2`, `tool_executions=1`, `failed_tools=1`.
  - query proof: `freehand-cliS adp-error-query --url ws://127.0.0.1:4042/adp --session cli-adp-sample-failure-1783177452885366000` -> `count=1`, event `tool:validation:repair_schema:tool_result_failed:451bc61e1a05e812`.
  - filter proof: `--domain tool` -> `count=1`; `--domain provider` -> `count=0`.
  - raw ADP subscribe proof with correct externally tagged `UiCommand`: accepted plus initial event `tool:validation:repair_schema:tool_result_failed:451bc61e1a05e812`.
  - post-fix `scripts/install-launchd.sh restartS` rebuilt S binaries, restarted `com.freehand.daemonS`, health `ok`, ADP smoke passed, and persisted error-center query still returned `count=1`.
- remaining gaps:
  - live push when new error-center metadata is written after subscription is still pending.
  - WebUI visible error-center cards are still pending.
  - task/node/UI error policy integration remains future scope.

# 2026-07-05 session CRUD + double-Esc rollback S-profile closeout

- owner slice: `ui.protocol`, `reason.persistence`, `runtime.ui-command-dispatch`, `app.webui-smoke`, `app.runtime-daemon`, `app.cli-runtime-smoke`.
- local gates passed before online proof:
  - `node --check apps/freehand-server/assets/webui.js`
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 44 passed
  - `cargo test -p freehand-reason -- --nocapture --test-threads=1` -> 60 passed
  - `cargo test -p freehand-runtime -- --nocapture --test-threads=1` -> 70 passed
  - `cargo test -p freehand-server -- --nocapture` -> 11 passed
  - `cargo test -p freehand-daemon -- --nocapture` -> 17 passed
  - `cargo test -p freehand-cli -- --nocapture` -> 13 passed
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `cargo test --workspace` -> 430 passed
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `make ci` -> exit 0
- S profile proof:
  - `scripts/install-launchd.sh restartS` refreshed S symlink/debug daemon copy and restarted `com.freehand.daemonS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> subscription/query/failure smoke passed.
  - real browser/CDP evidence saved under `artifacts/webui-online/20260705-session-crud-rollback-4042-1783188706/`.
  - session: `webui-session-20260704182300-fe1fceed`.
  - marker: `fh-crud-rollback-1783189377071`.
  - flow: new conversation -> first terminal `runtime-turn-38` -> WebUI rename -> refresh title persisted -> WebUI archive -> active list excluded and archived list included titled session -> WebUI restore -> transcript intact -> second terminal `runtime-turn-39` -> double Esc rollback -> composer restored second prompt and visible transcript hid `SECOND_TURN_FOR_ROLLBACK` -> edited replacement terminal `runtime-turn-40` -> restartS + browser reload restored title and effective transcript.
  - ADP before restart: `turns=2`, `turn_ids=runtime-turn-38,runtime-turn-40`, status `success`.
  - ADP after restart: same `turns=2`, `turn_ids=runtime-turn-38,runtime-turn-40`, status `success`.
  - browser states reported `pageErrors=0`, console errors none.
- validation note:
  - For CDP online proof in this environment, spawn headless Chrome inside the automation process and shut down only that explicit PID. Starting Chrome as a background child of a short-lived shell can leave the DevTools port unavailable because Chrome exits when the parent shell closes.

# 2026-07-05 instruction capability loader index slice

- user request: align Freehand AGENTS.md and skills design with `~/code/codex`, support local skills/local AGENTS.md, and index global AGENTS.md from `~/.freehand/AGENTS.md`.
- owner: new `instruction.capability-loader` feature in `crates/freehand-instructions`.
- implementation:
  - added `InstructionCapabilityCompileInput`, `InstructionCapabilityManifest`, `compile_instruction_capability_manifest`, and `write_instruction_capability_manifest`.
  - compiler indexes global `~/.freehand/AGENTS.md`, global `~/.freehand/skills/**/SKILL.md`, local `AGENTS.md` from project root to cwd, and local `.agents/skills/**/SKILL.md` from project root to cwd.
  - manifest entries include scope, precedence, normalized path/root, byte count, and content hash; skill entries also include parsed `name` and `description`.
  - malformed skill frontmatter becomes explicit manifest error records while valid entries remain indexed.
  - current slice is index-only; runtime/context-planner consumption remains pending and must use the compiled manifest rather than loose directory scanning.
- docs/gates:
  - added feature-map seed, function map, test design, design doc, mainline-call manifest, generated wiki, design index entry, and xtask required-file/workspace gates.
  - updated `freehand-dev` skill to lock owner boundary and forbid runtime/UI/provider direct directory scanning.
- validation:
  - `cargo test -p freehand-instructions -- --nocapture` -> 3 passed.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `cargo fmt --check` -> ok.
  - `cargo test -p xtask -- --nocapture` -> 18 passed.
  - `cargo test --workspace` -> 433 passed.
  - `cargo clippy --workspace --all-targets -- -D warnings` -> no issues.

# 2026-07-05 Anthropic max_tokens default 8192

- user correction: default provider output budget must be 8192, not current 512.
- owner slice: `provider.anthropic-adapter` plus `provider.reason-live-bridge` caller wiring in runtime.
- implementation:
  - added `DEFAULT_ANTHROPIC_MAX_TOKENS: u64 = 8192` in `crates/freehand-provider-anthropic`.
  - runtime live Anthropic executor now uses that constant instead of hardcoded `512`.
  - Anthropic adapter tests now assert rendered request `max_tokens=8192`.
  - provider Anthropic function map, test design, mainline JSON, and generated wiki synchronized.
- validation:
  - `cargo fmt --check` -> ok.
  - `cargo test -p freehand-provider-anthropic -- --nocapture` -> 15 passed.
  - `cargo test -p freehand-runtime live_bridge_interrupts_non_candidate_max_tokens_without_failed_status -- --nocapture` -> 1 passed.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `scripts/install-launchd.sh restartS` rebuilt/restarted S profile on `127.0.0.1:4042`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> ok.
  - `freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample success` -> session `cli-adp-sample-success-1783219259178878000`, turn `runtime-turn-42`, `rounds=1`, terminal command receipt.
  - `cargo clippy -p freehand-provider-anthropic -p freehand-runtime --all-targets -- -D warnings` -> no issues.

# 2026-07-05 WebUI online gate profile correction

- user correction: dev/S profile uses fixed `4042`; release/global profile uses fixed `4041`.
- implementation:
  - `make verify-webui-online` now defaults to S profile `http://127.0.0.1:4042/`, `ws://127.0.0.1:4042/adp`, `freehand-cliS`, and `FREEHAND_WEBUI_PROFILE=4042`.
  - `make verify-webui-release-online` is the explicit release wrapper for `4041` and `freehand-cli`.
  - browser verifier renamed from `scripts/webui_verify_4041.mjs` to `scripts/webui_verify_online.mjs` and parameterized by environment.
  - `xtask` gate now locks default/release URL, health URL, ADP URL, CLI, and profile snippets; xtask fixtures include both wrapper scripts.
  - docs/function map/test design/mainline JSON/generated wiki/release doc/MEMORY corrected so alpha closeout no longer points at 4041.
- validation:
  - `cargo run -p xtask -- mainlines generate`
  - `bash -n scripts/verify-webui-online.sh scripts/verify-webui-release-online.sh`
  - `node --check scripts/webui_verify_online.mjs`
  - `cargo test -p xtask -- --nocapture` -> 18 passed
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `scripts/install-launchd.sh installS` started `com.freehand.daemonS` on `127.0.0.1:4042`
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`
  - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> ok
  - `make verify-webui-online` -> run `20260705-verify-4042-1783248727371`, all checks true, ADP session query exit 0
  - `make ci` -> exit 0
- evidence:
  - `artifacts/webui-online/20260705-verify-4042-1783248727371/summary.json`
  - session `webui-session-20260705105207-a9295d35`
  - turns `runtime-turn-53,runtime-turn-54,runtime-turn-54-r2`
- exclusion:
  - old `20260705-verify-4041-*` artifacts are wrong-profile intermediate evidence and are intentionally not staged.

# 2026-07-05 provider retry + schema polishing boundary

- user correction: schema mismatch is not failure and should not be called schema repair. It is response-schema mismatch polishing: feedback tells the model what fields/types are missing/wrong so the next model response can align to the Freehand completion contract.
- implementation:
  - provider executor failures are classified into concrete codes such as `anthropic_http_status_500`, `anthropic_http_request_failed`, `anthropic_stream_read_failed`, `anthropic_adapter_failed`, `anthropic_invalid_config`, and `anthropic_callback_failed`.
  - recoverable non-stream provider failures retry five attempts with production exponential backoff `1s,2s,4s,8s,16s`; tests can override backoff with `FREEHAND_PROVIDER_RETRY_BACKOFF_MS`.
  - provider retry attempts write `error.center` metadata with retry index/cap; pre-cap decisions are `retry_same_step`, cap-exhausted decisions are `fail_turn`.
  - failed terminal truth uses the concrete provider error code instead of generic `provider_executor_failure`.
  - UI/user-visible model request text changed from `schema retry #N` to `schema polishing #N`; internal protocol kind `SchemaRetry` and recovery action `repair_schema` are retained for compatibility only.
- regression locks:
  - provider success after earlier 500s: `live_bridge_retries_recoverable_provider_errors_then_succeeds`.
  - provider five-attempt failure: `live_bridge_fails_after_five_provider_retries_with_error_code`.
  - provider metadata/error truth: `live_bridge_writes_provider_error_metadata_on_executor_failure`.
  - schema mismatch stays schema-domain polishing, not provider/fail_turn: `live_bridge_records_error_center_metadata_for_schema_repair`.
  - UI protocol projects `schema polishing #N`: `schema_mismatch_projects_as_model_polishing_activity`.
- validation:
  - `cargo test -p freehand-control -- --nocapture`
  - `cargo test -p freehand-ui-protocol -- --nocapture`
  - `cargo test -p freehand-runtime -- --nocapture --test-threads=1`
  - `cargo test -p freehand-server -- --nocapture`
  - `cargo fmt --check`
  - `node --check apps/freehand-server/assets/webui.js`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `scripts/install-launchd.sh restartS`, `curl -4fsS http://127.0.0.1:4042/health`, and `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
  - `make verify-webui-online` -> `artifacts/webui-online/20260705-verify-4042-1783252779663/summary.json`
  - temp-HOME CLI live provider fixture -> `artifacts/provider-retry/20260705-provider-retry-1783252973529124000/summary.json`, `requestCount=5`, stderr contains `anthropic_http_status_500`
- exclusions:
  - old wrong-profile `artifacts/webui-online/20260705-verify-4041-*` remain untracked and are not staged.

# 2026-07-05 same-session continuation clippy closeout

- context:
  - after commit `0e19a45 fix(runtime): restore follow-up history`, full `make ci` exposed `clippy::too_many_arguments` in `record_provider_error_metadata`.
  - this was an implementation-shape issue in `provider.reason-live-bridge`, not a behavior change.
- implementation:
  - grouped provider error metadata inputs into `ProviderErrorMetadataSpec<'_>`.
  - kept retry index/cap, concrete provider error code, error-center metadata, and runtime metadata write semantics unchanged.
- validation:
  - `cargo test -p freehand-runtime -- --nocapture --test-threads=1` -> 74 passed.
  - `cargo clippy -p freehand-runtime --all-targets -- -D warnings` -> no issues.
  - `make ci` -> exit 0.
  - `scripts/install-launchd.sh restartS` refreshed S profile on `127.0.0.1:4042`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> ok.
  - `make verify-webui-online` -> `artifacts/webui-online/20260705-verify-4042-1783258042728/summary.json`.
- online evidence:
  - session `webui-session-20260705132723-566c48e2`.
  - ADP reported `turn_ids=runtime-turn-66,runtime-turn-67,runtime-turn-67-r2`.
  - browser checks true: submitted prompts stay visible, composer clears, second turn observes progress, failed read_file result returns to model, terminal has `liveCount=0`, refresh preserves both turns.
- exclusions:
  - pre-existing untracked wrong-profile or intermediate evidence directories remain untouched.

# 2026-07-05 mobile WebUI responsive direction

- user direction:
  - mobile/WebUI layout switching should use aspect-ratio plus width, not width-only breakpoints.
  - mobile clients need an independent daemon connection config location.
  - daemon connection config must be file-backed and persistent.
  - default remote access mode is Tailscale; relay server is a later extension.
- design updates:
  - `docs/design/multi-platform-ui-architecture.md` now documents ADP-first transport, aspect-ratio shape matrix, mobile daemon config schema, Tailscale default, relay-disabled placeholder, and no silent fallback rule.
  - `docs/design/android-client-v1-execution.md` now marks ADP as default Android live transport and records file-backed config as required next slice.
  - `docs/testing/app.android-client.md` now adds file-backed config and aspect-ratio layout verification targets while preserving current SharedPreferences implementation as a gap.
  - `docs/function-maps/app.android-client.md` now states current SharedPreferences host/port persistence is scaffold-only and the target is app-owned JSON config.
- validation:
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `git diff --check` -> ok.

# 2026-07-05 mobile WebUI goal prompt plan

- user asked whether L1 audit was done and requested a complete implementation `/goal` prompt.
- answer basis:
  - scoped L1 report-only audit was completed in commit `3a5b461`.
  - it was not a recurring unattended loop, so no `LOOP.md` / `STATE.md` / `loop-budget.md` files were required.
- implementation plan:
  - added `docs/goals/mobile-webui-responsive-plan.md` with L1 audit status, goals, scope, design principles, implementation slices, verification matrix, risks, and DoD.
  - linked the goal plan from `docs/design/design-doc-index.md`.

# 2026-07-06 mobile WebUI responsive + Android config closeout

- implementation:
  - WebUI layout classifier now uses viewport width plus aspect ratio and writes only `body[data-layout-shape]` / shell layout shape attributes.
  - WebUI responsive CSS covers phone portrait, tall phone, phone landscape, tablet portrait, foldable/tablet landscape, and desktop large.
  - WebUI global auto-scroll no longer calls `window.scrollTo(documentElement.scrollHeight)`; it scrolls the conversation stream instead so sidebar/inspector height cannot push the composer out of view.
  - Online verifier now fails on false checks, captures viewport matrix screenshots/JSON, and gates `layoutShape`, composer visibility, and message-list visibility.
  - Android daemon connection config is app-owned JSON bootstrapped from bundled `assets/config/client.json`; SharedPreferences host persistence was removed.
  - Android active config is Tailscale-first with relay parsed but rejected while disabled.
  - Android `MainActivity` no longer uses `defaultTailscale()` as a runtime fallback after config-load failure; config errors disable input and show `daemon config error`. The drawer may still use default Tailscale only as an editable repair seed before writing a new explicit config.
  - Freehand local skill now forbids implementation searches over generated/runtime output and MemoryPalace corpora; generated artifacts are verification evidence only.
- WebUI online evidence:
  - S profile restarted via `scripts/install-launchd.sh restartS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> ok.
  - `make verify-webui-online` -> `artifacts/webui-online/20260705-verify-4042-1783269156414/summary.json`.
  - session `webui-session-20260705163237-e71a1a3e`; ADP `turn_ids=runtime-turn-80,runtime-turn-81,runtime-turn-81-r2`; selected session `success`.
  - checks true: composer clears after both submits, submitted first prompt survives refresh, failure prompt survives refresh, second turn progress observed, terminal has `liveCount=0`, `staleHistoricalLiveAfterSecondSubmit=0`, all viewport shapes covered, composer visible, and message list visible.
  - viewport evidence includes screenshots/JSON for `375x812`, `430x932`, `844x390`, `768x1024`, `1024x768`, `900x1000`, `1280x900`.
- Android validation:
  - `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew testDebugUnitTest` -> success.
  - `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew assembleDebug` -> success.
  - APK installed to explicit serial `100.104.163.65:5555` with `adb -s 100.104.163.65:5555 install -r apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk` -> `Success`.
  - Device remained on Oplus lockscreen/Dozing after ADB wake/dismiss attempts; `dumpsys window` still showed `mDreamingLockscreen=true` and focus on `NotificationShade` / non-Freehand activity, so Freehand UI screenshot/interaction validation is blocked until the device is manually unlocked.
  - Lock-screen screenshots are under `artifacts/android-device/20260705-debug-install-10010416365/`; they are evidence of the blocker, not UI acceptance evidence.
- gates:
  - `node --check apps/freehand-server/assets/webui.js`
  - `node --check scripts/webui_verify_online.mjs`
  - `node scripts/verify-webui-layout-shapes.mjs`
  - `cargo test -p freehand-server -- --nocapture`
  - Android JVM tests and debug APK build as above.
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace` -> `437 passed`.
  - `make ci` -> exit 0.

# 2026-07-06 source-only search boundary

- user correction: implementation/debug searches must not include generated output, runtime artifacts, build products, generated wiki, or MemoryPalace corpora; only source code, tests, maintained scripts, and canonical docs should be searched.
- implementation:
  - added `.ignore` so default `rg` skips `target/`, `dist/`, `artifacts/`, `docs/wiki/`, `.mempalace/`, `memory/*-mempalace-corpus/`, `test-palaces/`, and package build caches.
  - added `scripts/source-search.sh` as the source-only implementation search wrapper; it excludes generated/runtime paths and does not include `CACHE.md`, `MEMORY.md`, or `note.md` as search roots.
  - added `verify_source_search_policy` to `xtask gates check` with positive and negative tests.
  - updated `foundation.workspace` function map/test design/mainline JSON, debug workflow docs, dev gates docs, feature map, and local `freehand-dev` skill.
- verification:
  - `bash -n scripts/source-search.sh`
  - `scripts/source-search.sh "20260705-verify-4042"` -> no matches, proving generated evidence and memory notes are excluded from source-search roots.
  - `cargo fmt --check`
  - `cargo test -p xtask -- --nocapture` -> 20 passed.
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`

# 2026-07-06 Android device verifier closeout

- current goal audit:
  - WebUI responsive/mobile layout has fresh online S-profile evidence at `artifacts/webui-online/20260705-verify-4042-1783272863538/summary.json`.
  - Android JVM tests and debug APK build pass, but true Android UI/WebView acceptance still requires a connected/unlocked device.
- implementation:
  - added `apps/freehand-android/scripts/verify-device-ui.sh`.
  - script requires an explicit ADB serial, installs the debug APK unless skipped, starts `com.freehand.android/.ui.MainActivity`, captures `adb devices`, activity/window dumps, logcat, screenshot, and `summary.json`.
  - script exits non-zero with `status=blocked` for offline/unavailable ADB, locked/dozing device, or Freehand not foreground; it exits failed for fatal/exception logcat. It does not broad-kill or silently unlock/switch endpoint.
  - synchronized `app.android-client` function map, test design, feature map, mainline JSON, and generated wiki.
- verification:
  - `bash -n apps/freehand-android/scripts/verify-device-ui.sh`
  - `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` -> blocked with `adb_state_unavailable`, evidence at `artifacts/android-device/current-blocker/summary.json`.
  - `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew testDebugUnitTest assembleDebug` -> success.
  - `cargo test -p freehand-server -- --nocapture` -> 11 passed.
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
- remaining blocker:
  - `adb devices -l` currently returns no device, so true-device UI screenshot/logcat acceptance is not complete.

# 2026-07-06 source-search bypass hardening

- user correction: implementation/debug search must never include generated outputs; only code and canonical docs are valid search corpus.
- implementation:
  - `scripts/source-search.sh` now rejects unsafe `rg` ignore-bypass options including `--no-ignore`, `--unrestricted`, and `-u`.
  - hard generated-output exclude globs now run after caller-provided args, so a caller-provided include glob such as `--glob=artifacts/**` cannot re-include generated evidence.
  - `xtask verify_source_search_policy` now gates the unsafe-argument guard, hard exclude order, and forbidden implementation-search roots.
  - added a negative xtask test for missing unsafe-argument guard.
  - synchronized `foundation.workspace` function map, test design, mainline JSON/generated wiki, dev docs, and local `freehand-dev` skill.
- verification:
  - `bash -n scripts/source-search.sh`
  - `scripts/source-search.sh "20260705-verify-4042"` -> no matches
  - `scripts/source-search.sh --glob 'artifacts/**' "20260705-verify-4042"` -> no matches
  - `scripts/source-search.sh --no-ignore "anything"` -> refused with exit 2
  - `scripts/source-search.sh -u "anything"` -> refused with exit 2
  - `cargo fmt --check`
  - `cargo test -p xtask source_search -- --nocapture` -> 3 passed
  - `cargo test -p xtask -- --nocapture` -> 21 passed
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`

# 2026-07-06 Android device verifier retry

- active goal audit:
  - goal file remains `/Users/fanzhang/.codex/attachments/75141fb8-5139-49b6-ac44-ffc7f7608b3a/pasted-text-1.txt`.
  - WebUI responsive/mobile proof remains available from prior S-profile online verifier evidence; Android true-device UI/WebView acceptance is still the open closeout item.
- device attempt:
  - `adb devices -l` returned no devices.
  - `adb connect 100.104.163.65:5555` did not return within 30 seconds and was interrupted as a single explicit command session.
  - `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` exited blocked with `adb_state_unavailable`.
  - blocker evidence: `artifacts/android-device/20260705T180437Z-100.104.163.65_5555/summary.json`.
- current-head Android code evidence:
  - `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew testDebugUnitTest assembleDebug` -> `BUILD SUCCESSFUL`.
- conclusion:
  - do not mark the mobile WebUI / Android closeout goal complete until the Android device is connected/unlocked and `verify-device-ui.sh` passes with Freehand foreground screenshot and no fatal logcat, or Jason explicitly accepts the remaining Android device risk.

# 2026-07-06 Android device blocked audit

- active goal audit:
  - objective remains the mobile WebUI / Android WebView closeout from `/Users/fanzhang/.codex/attachments/75141fb8-5139-49b6-ac44-ffc7f7608b3a/pasted-text-1.txt`.
  - completion still requires Android true-device UI/WebView evidence; existing blocker summaries are explicitly not acceptance evidence.
- repeated blocker:
  - `adb devices -l` returned no devices.
  - `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` exited blocked again with `adb_state_unavailable`.
  - blocker evidence: `artifacts/android-device/20260705T180852Z-100.104.163.65_5555/summary.json`.
- blocked decision:
  - same external blocker has now repeated across consecutive goal continuations with no available local progress path: ADB device `100.104.163.65:5555` is unavailable.
  - next meaningful progress requires the Android device/emulator to be connected and unlocked, then rerun `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555`.

# 2026-07-06 Android verifier classification fix

- device state:
  - `adb connect 100.104.163.65:5555` succeeded and `adb devices -l` showed `100.104.163.65:5555 device product:PLZ110 model:PLZ110`.
  - first verifier run after reconnect reported `freehand_activity_not_foreground`.
- root cause trace:
  - manual launch with cleared logcat showed `AndroidRuntime` crash: `ClassNotFoundException: com.freehand.android.ui.MainActivity`.
  - source and compile output had `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` and `build/tmp/kotlin-classes/debug/com/freehand/android/ui/MainActivity.class`.
  - stale/incremental APK initially lacked `com.freehand.android.ui.MainActivity`; `cd apps/freehand-android && ./gradlew clean assembleDebug` rebuilt an APK whose dex contained the activity.
- implementation:
  - `apps/freehand-android/scripts/verify-device-ui.sh` now preflights APK launcher activity class via `apkanalyzer` when available.
  - verifier now checks Freehand fatal/exception logcat before lockscreen/not-foreground blocker classification, so app crashes become `failed` rather than `blocked`.
  - `docs/function-maps/app.android-client.md` and `docs/testing/app.android-client.md` document the failed-vs-blocked split.
- verification:
  - `bash -n apps/freehand-android/scripts/verify-device-ui.sh` -> ok.
  - `FREEHAND_ANDROID_ACTIVITY=.ui.DoesNotExist apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` -> failed with `apk_missing_launcher_activity_class`; evidence `artifacts/android-device/20260706T002036Z-100.104.163.65_5555/summary.json`.
  - `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew testDebugUnitTest assembleDebug` -> `BUILD SUCCESSFUL`.
  - `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, `git diff --check` -> ok.
  - current real-device acceptance still blocked by `device_locked_or_dreaming`; evidence `artifacts/android-device/20260706T002048Z-100.104.163.65_5555/summary.json`.

# 2026-07-06 mobile WebUI / Android WebView release closeout attempt

- active goal audit:
  - objective remains mobile WebUI / Android WebView closeout from `/Users/fanzhang/.codex/attachments/75141fb8-5139-49b6-ac44-ffc7f7608b3a/pasted-text-1.txt`.
  - WebUI and release daemon evidence is closed; Android true-device WebView acceptance is still blocked by secure keyguard.
- WebUI S-profile online evidence:
  - `FREEHAND_DAEMON_BIND=127.0.0.1:4042 scripts/install-launchd.sh restartS` -> restarted `com.freehand.daemonS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - `make verify-webui-online` -> exit 0.
  - latest evidence: `artifacts/webui-online/20260706-verify-4042-1783303750890/summary.json`.
  - checks true: composer clears after submit, submitted prompts survive refresh, second turn progress observed, no stale historical live animation, terminal has no live animation, viewport matrix covered, mobile conversation is primary, session/detail drawers open, drawer closes.
- release daemon evidence:
  - `tailscale ip -4` -> `100.66.1.82`.
  - `FREEHAND_DAEMON_BIND=100.66.1.82:4041 scripts/install-launchd.sh restart` -> restarted `com.freehand.daemon`.
  - `curl -4fsS http://100.66.1.82:4041/health` -> `ok`.
  - `~/.local/bin/freehand-cli adp-smoke --url ws://100.66.1.82:4041/adp` -> `adp_smoke_ok`.
- Android device evidence:
  - `adb devices -l` shows `100.104.163.65:5555 device product:PLZ110 model:PLZ110`.
  - `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` exited blocked with `device_locked_or_dreaming`.
  - blocker evidence: `artifacts/android-device/20260706T021041Z-webui-mobile-release-4041/summary.json`.
  - screenshot evidence: `artifacts/android-device/20260706T021041Z-webui-mobile-release-4041/screenshot.png`; visual state is not Freehand WebView, so it is blocker evidence only.
  - `dumpsys-window.txt` shows `KeyguardServiceDelegate showing=true secure=true inputRestricted=true`, `mCurrentFocus=NotificationShade`, and `mFocusedApp=com.freehand.android/.ui.MainActivity`.
  - `adb shell wm dismiss-keyguard`, statusbar collapse, wake, swipe, HOME, and explicit `am start` could not remove secure keyguard.
  - app-owned daemon config on device is correct: `tailscale-main`, host `100.66.1.82`, port `4041`, `adpPath=/adp`, relay disabled.
- local gates:
  - `node --check apps/freehand-server/assets/webui.js` -> ok.
  - `node --check scripts/webui_verify_online.mjs` -> ok.
  - `node scripts/verify-webui-layout-shapes.mjs` -> ok.
  - `cargo fmt --check` -> ok.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `cargo test -p freehand-server -- --nocapture` -> 11 passed.
  - `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew testDebugUnitTest assembleDebug` -> `BUILD SUCCESSFUL`.
  - `cargo clippy --workspace --all-targets -- -D warnings` -> no issues.
  - `cargo test --workspace` -> 440 passed.
  - `make ci` -> exit 0.
- conclusion:
  - code/docs/WebUI/release daemon are verified.
  - do not claim Android true-device WebView UI acceptance until the Oplus device is manually unlocked and `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` passes with Freehand foreground screenshot.

# 2026-07-06 restartS bind and WebUI online verifier closeout

- root cause:
  - `scripts/install-launchd.sh restartS` recomputed `bind_addr` from default Tailscale detection instead of reading `~/.freehand/daemonS.env`, so health could check `100.66.1.82:4042` while the S daemon was actually configured for `127.0.0.1:4042`.
  - `scripts/webui_verify_online.mjs` still assumed `new-conversation-button` directly created a session. After the `/new` dialog change, the script opened the dialog but did not confirm it, so prompts could land in a stale localStorage-selected archived session and produce false refresh-history failures.
- implementation:
  - S-profile default bind is now fixed to `127.0.0.1:<port>` inside `default_daemon_bind`; release profile can still use explicit `FREEHAND_DAEMON_BIND` or its default path.
  - launchd script resolves `bind_addr` from explicit `FREEHAND_DAEMON_BIND`, then existing env file `FREEHAND_DAEMON_BIND`, then profile default.
  - xtask gate now locks the S loopback branch and env-backed health bind snippets, with a negative CI/CD fixture test for missing env bind.
  - online verifier now waits for the New dialog, confirms conversation mode, waits for the draft session, and after reload waits until both success and failed-tool prompts are visible before screenshot/assertion.
  - synchronized foundation function map, test design, mainline JSON, generated wiki, and local freehand-dev skill.
- verification:
  - `bash -n scripts/install-launchd.sh`
  - `node --check scripts/webui_verify_online.mjs`
  - `node --check apps/freehand-server/assets/webui.js`
  - `cargo test -p xtask ci_cd -- --nocapture` -> 4 passed.
  - `cargo test -p freehand-server -- --nocapture` -> 11 passed.
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
  - `scripts/install-launchd.sh restartS` -> restarted `com.freehand.daemonS`, S install output showed `--bind 127.0.0.1:4042`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - `make verify-webui-online` -> exit 0; evidence `artifacts/webui-online/20260706-verify-4042-1783310624927/summary.json`.
  - online checks true: composer clears after both submits, first and failure prompts survive refresh, failed-tool continuation reaches terminal success, stale historical live count is `0`, terminal live count is `0`, viewport matrix and mobile drawers pass.
- remaining:
  - Android true-device WebView acceptance remains blocked until device is unlocked and `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` passes.

# 2026-07-06 Android true-device WebView acceptance closeout

- prerequisite:
  - device `100.104.163.65:5555` is online: `adb devices -l` showed `device product:PLZ110 model:PLZ110`.
  - release/Tailscale daemon endpoint verified before device run:
    - `curl -4fsS http://100.66.1.82:4041/health` -> `ok`.
    - `~/.local/bin/freehand-cli adp-smoke --url ws://100.66.1.82:4041/adp` -> `adp_smoke_ok`.
- Android verifier:
  - command: `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555`.
  - result: passed.
  - evidence: `artifacts/android-device/20260706T040938Z-100.104.163.65_5555-64346/summary.json`.
  - summary fields: `status=passed`, `reason=freehand_activity_foreground_no_fatal_logcat`, `package=com.freehand.android`, `activity=.ui.MainActivity`, APK `apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk`.
  - screenshot: `artifacts/android-device/20260706T040938Z-100.104.163.65_5555-64346/screenshot.png`.
  - visual inspection: screenshot shows daemon-hosted Freehand WebUI mobile layout with `Sessions` / `Status` drawer buttons, conversation-first body, completed turn card, and bottom composer; it is not the old native/fallback desktop-column UI.
- closure impact:
  - previous blocker `device_locked_or_dreaming` is cleared for this device run.
  - mobile WebUI/Android WebView goal now has both S-profile browser evidence and Android true-device foreground WebView evidence.
  - final full local gate after Android pass: `make ci` -> exit 0.

# 2026-07-06 phone UI stale asset correction

- user reported phone UI did not update after the closeout.
- root cause:
  - Android app is configured for release/Tailscale daemon `100.66.1.82:4041`, while the freshest WebUI assets were only proven on S profile `127.0.0.1:4042`.
  - before global reinstall, served release hashes did not match workspace:
    - workspace `webui.css`: `8ed33b2d1d8be68632668d5375d2455bd44e1b93fefb1c3eb64a07ce39bda1a1`
    - workspace `webui.js`: `2a7cc005245ed0f017d7a4f4f7761e15e9975733785f8a3f6844f333c19c21af`
    - old `100.66.1.82:4041` `webui.css`: `13100627a7bbbc91c025a9ef48adaff14d3f4621267c23256a065e34c986ca9f`
    - old `100.66.1.82:4041` `webui.js`: `0775ab6e51a6687b0b596e2ae8058d452fff91110c969add28447837e0ad19b3`
- correction:
  - ran `scripts/install-global.sh && scripts/install-launchd.sh restart`.
  - release daemon now runs `com.freehand.daemon` on `100.66.1.82:4041`.
  - release served CSS/JS hashes now match workspace exactly.
  - `curl -4fsS http://100.66.1.82:4041/health` -> `ok`.
  - Android verifier passed: `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` -> `artifacts/android-device/20260706T042507Z-100.104.163.65_5555-11074`.
  - screenshot visual check: phone now shows updated mobile WebUI header tags (`Master`, `Task cwd`, `runtime-turn-99-r2`) and updated tool/card rendering.
  - logcat layout probe: `shape=tall_phone`, `conversationPrimary=true`, session/detail drawers fixed but offscreen.
- lesson:
  - S-profile online proof is not enough for phone UI proof when Android points to release 4041.
  - phone verification must include release served asset hash check before Android screenshot acceptance.
# 2026-07-06 WebUI new-session clean-state release verification

- root cause found on release 4041: served root HTML and assets were updated, but `/new` still failed in browser because `crypto.randomUUID` is unavailable on non-secure Tailscale HTTP (`http://100.66.1.82:4041`), leaving the old selected session visible and reporting `new session failed: crypto.randomUUID is not a function`.
- implementation:
  - `apps/freehand-server/assets/webui.js` now uses `browserRandomId()` for draft session ids and local request ids; it prefers `crypto.randomUUID`, falls back to `crypto.getRandomValues`, then a local timestamp/random suffix.
  - `scripts/webui_verify_online.mjs` now removes existing sessions through ADP `DeleteSession` before creating a new session, then asserts clean `/new` state before submitting samples.
  - `apps/freehand-server/src/lib.rs` asset smoke now locks the shared id helper and forbids direct `crypto.randomUUID().slice` use for draft ids.
  - synchronized `app.webui-smoke` function map and test design.
- verification:
  - `node --check apps/freehand-server/assets/webui.js`
  - `node --check scripts/webui_verify_online.mjs`
  - `cargo test -p freehand-server -- --nocapture`
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `scripts/install-global.sh`
  - `FREEHAND_DAEMON_BIND=100.66.1.82:4041 scripts/install-launchd.sh restart`
  - `curl -4fsS http://100.66.1.82:4041/health` -> `ok`
  - `~/.local/bin/freehand-cli adp-smoke --url ws://100.66.1.82:4041/adp` -> `adp_smoke_ok`
  - `FREEHAND_WEBUI_BASE_URL=http://100.66.1.82:4041/ FREEHAND_WEBUI_ADP_URL=ws://100.66.1.82:4041/adp FREEHAND_WEBUI_CLI=$HOME/.local/bin/freehand-cli FREEHAND_WEBUI_PROFILE=4041 node scripts/webui_verify_online.mjs`
  - latest evidence: `artifacts/webui-online/20260706-verify-4041-1783329891362/summary.json`.
  - verifier removed old session `webui-session-20260706091227-20c2b581`; after cleanup ADP reported `sessions=0`.
  - new draft `webui-session-20260706092451-c6f51f63` showed `selectedTurn=-`, `messageCount=0`, `messageText="New conversation\nSend a message to start this session."`, `newSessionStartsClean=true`, and `newSessionDoesNotLeakPreviousTurn=true`.
  - same run completed success + failed-tool continuation samples; ADP reported `turn_ids=runtime-turn-112,runtime-turn-113,runtime-turn-113-r2`, `terminal2NoLive=true`, and no stale historical live animation.

# 2026-07-06 WebUI session truth gate trace

- user issue: after removing old sessions and creating/refreshing a new WebUI session, old assistant/tool cards could still appear.
- root cause:
  - `DeleteSession` is non-destructive metadata removal, so old turn truth can remain queryable as `latest-active-turn`.
  - WebUI had multiple render inputs: session list/transcript truth plus latest-active query/subscription/SSE fallback.
  - when ADP session list was empty or the selected session had been removed, latest-active turn projection could still be accepted and render old `runtime-turn-*` content.
- implementation:
  - `state.sessionListLoaded` marks when session list truth exists.
  - after session list truth exists, `sessionTruthAllowsSessionId` gates latest-turn query, ADP subscription event, SSE event, and selected-session transcript projection.
  - allowed exceptions are only current draft session and current pending-submit session.
  - rejected stale projections clear local render truth instead of selecting the old session.
  - asset smoke now locks `sessionTruthAllowsSessionId` and transcript gate snippets.
- verification:
  - `node --check apps/freehand-server/assets/webui.js`
  - `cargo test -p freehand-server -- --nocapture` -> 12 passed.
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
  - `scripts/install-global.sh` -> full release regression/build/install passed.
  - `FREEHAND_DAEMON_BIND=100.66.1.82:4041 scripts/install-launchd.sh restart`
  - `curl -4fsS http://100.66.1.82:4041/health` -> `ok`.
  - `~/.local/bin/freehand-cli adp-smoke --url ws://100.66.1.82:4041/adp` -> `adp_smoke_ok`.
  - release served asset hashes match workspace:
    - `webui.js`: `9954f9bdb1f85a55c69f84abbdf8cc58975892c731bef895b982f7eea7d53841`
    - `webui.css`: `b207c63c9bf20ae6697b68d899a9821d9608ac0ae953bc08915272a99647dbcc`
  - `FREEHAND_WEBUI_BASE_URL=http://100.66.1.82:4041/ FREEHAND_WEBUI_ADP_URL=ws://100.66.1.82:4041/adp FREEHAND_WEBUI_CLI=$HOME/.local/bin/freehand-cli FREEHAND_WEBUI_PROFILE=4041 node scripts/webui_verify_online.mjs`
  - latest evidence: `artifacts/webui-online/20260706-verify-4041-1783332261674/summary.json`.
  - verifier first created old latest turn `cli-adp-sample-success-1783332254816883000` (`runtime-turn-118`), then removed all sessions by ADP; cleanup after-state reported `sessions=0`.
  - clean new draft `webui-session-20260706100422-93a7a172` showed `selectedTurn=-`, `messageCount=0`, `messageText="New conversation\nSend a message to start this session."`, `newSessionStartsClean=true`, and `newSessionDoesNotLeakPreviousTurn=true`.
  - same run completed success plus failed-tool continuation; ADP reported `turn_ids=runtime-turn-119,runtime-turn-120,runtime-turn-120-r2`, `terminal2NoLive=true`, and stale historical live count `0`.

# 2026-07-06 user-visible protocol wording cleanup

- user issue: ADP is an internal protocol and should be transparent to users; WebUI/Android visible status/cards/prompts must not show `ADP`.
- implementation:
  - WebUI status text now says connection/service/request/conversation instead of ADP.
  - WebUI pending cards say `Request accepted. Waiting for service dispatch.`
  - WebUI failure bubble title is `Connection`, not `ADP`.
  - WebUI verifier prompts now use `Online success sample` / `Online failure sample`, so screenshots do not show ADP in user messages.
  - Android `TimelineProjector` protocol failure public projection now uses `Connection` / `connection failure`.
  - Android command send errors now use `service connection is not ready`, `request send failed`, and `request sent`.
  - local skill now records ADP as internal terminology only.
- verification:
  - `node --check apps/freehand-server/assets/webui.js`
  - `node --check scripts/webui_verify_online.mjs`
  - `cargo test -p freehand-server -- --nocapture` -> 12 passed.
  - `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew testDebugUnitTest` -> passed.
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
  - `scripts/install-global.sh` -> full release regression/build/install passed.
  - `FREEHAND_DAEMON_BIND=100.66.1.82:4041 scripts/install-launchd.sh restart`
  - `curl -4fsS http://100.66.1.82:4041/health` -> `ok`.
  - `~/.local/bin/freehand-cli adp-smoke --url ws://100.66.1.82:4041/adp` -> `adp_smoke_ok`.
  - release served asset hashes match workspace:
    - `webui.js`: `e4a01ee878de0fdbfb4bddd3f07245d4da148f4beb58d380df9729178b71a7bd`
    - `webui.css`: `b207c63c9bf20ae6697b68d899a9821d9608ac0ae953bc08915272a99647dbcc`
  - online WebUI evidence: `artifacts/webui-online/20260706-verify-4041-1783348272047/summary.json`.
  - WebUI checks all true; `messageTextContainsADP=false`; command statuses include `new conversation ready`, `dispatching...`, `thinking after tool result...`, and `turn completed`.
  - Android true-device evidence: `artifacts/android-device/20260706T143159Z-100.104.163.65_5555-81364/summary.json`, `status=passed`.

# 2026-07-07 OpenMinis config UI L1 report-only

- user request: reference OpenMinis UI, identify missing Freehand config functionality/UI, then make it a loop to complete.
- skills used: `freehand-dev` and `loop-governance`.
- OpenMinis evidence:
  - local source search found no local OpenMinis checkout.
  - GitHub repo `OpenMinis/OpenMinis` is public but source is not available yet; README says public source is still being organized.
  - `https://openminis.app/` is the usable reference: first-run provider setup, provider/model table, advanced config, model groups, agent loop models, skills, session filesystem namespaces, native/mobile capability settings, FAQ-style diagnostics, compact Apple-like cards/disclosures.
- Freehand gap summary:
  - WebUI has conversation/session/mobile rendering and a read-only model display, but no full settings/config surface.
  - Android has file-backed daemon connection config, but normal connected UI loads daemon-hosted WebUI, so WebUI must expose the shared settings entry.
  - Config truth is `config.core` at `~/.freehand/config.toml`; WebUI must not parse/write config directly.
  - Missing user-facing surfaces: first-run/config-needed flow, provider list/editor/status, editable model selection contract, agent/provider topology view, skill registry, session filesystem namespace view, daemon/profile status, task/background settings.
- loop/plan files added:
  - `docs/loops/openminis-config-ui-closeout/LOOP.md`
  - `docs/loops/openminis-config-ui-closeout/STATE.md`
  - `docs/loops/openminis-config-ui-closeout/loop-constraints.md`
  - `docs/loops/openminis-config-ui-closeout/loop-budget.md`
  - `docs/loops/openminis-config-ui-closeout/loop-run-log.md`
  - `docs/goals/openminis-config-ui-closeout-plan.md`
- L2 batch order:
  - read-only settings shell
  - owner-backed config projection
  - provider/model edit flow with restart-required semantics
  - Android connection settings convergence
  - advanced surfaces only after owners exist
- L1 non-action: no product code, runtime config, launchd, Android install, or provider secret changes.

# 2026-07-07 OpenMinis config UI L2 Batch 1 read-only Settings shell

- implementation:
  - WebUI now has a desktop Settings button and mobile Settings drawer entry.
  - right inspector can switch between debug and Settings panels using `inspectorPanel`; this is UI-only and does not mutate ADP/session truth.
  - Settings shell is read-only and compact: connection, provider/model, sessions/workspace, skills, files/attachments, tasks/background, diagnostics.
  - provider/model/skill/task edit actions are disabled until owner-backed `config.core` / `ui.protocol` contracts exist.
  - WebUI does not add API-key/password inputs and does not write config/local config state.
  - user-visible `ADP waiting` / `protocol state` copy in WebUI shell/refresh status was changed to service wording.
  - online verifier now checks desktop Settings, mobile Settings drawer, read-only disabled controls, no secret inputs, and conversation remains intact after closing Settings.
- bugs caught by online verification:
  - `renderSettingsShell()` initially called nonexistent `currentDraftAttachments()`, causing a real browser `ReferenceError` and `/new` failure; fixed to existing `currentAttachments()` and locked in asset smoke.
  - `.settings-shell { display: grid }` overrode the HTML `hidden` attribute, so closing Settings left it visible; fixed with explicit `.settings-shell[hidden]` / `.inspector-debug-panel[hidden]` CSS rule and asset smoke.
- verification:
  - `node --check apps/freehand-server/assets/webui.js`
  - `node --check scripts/webui_verify_online.mjs`
  - `cargo test -p freehand-server -- --nocapture` -> 12 passed.
  - `cargo fmt --check`
  - `git diff --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `scripts/install-launchd.sh restartS`
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - served asset hashes matched workspace:
    - `webui.js`: `b47acaa05b93160c9ec485e66b1281a71223623c19daefb09a41747e6b00e11a`
    - `webui.css`: `115709c6f33c1d3bcd4e8e67e93d1976b534a7a48eee89adb33f05538e8744ec`
  - `make verify-webui-online` -> passed.
  - online evidence: `artifacts/webui-online/20260706-verify-4042-1783371813295/summary.json`.
  - online checks true: desktop Settings read-only, Settings close keeps conversation, mobile Settings drawer opens, mobile drawers close, success + failed-tool multi-round conversation terminal, no stale live animation, clean `/new`, viewport matrix.
- remaining:
  - Batch 1 is read-only only. Provider/model edits, first-run setup, config projection, and Android native pre-connection settings convergence remain future batches.

# 2026-07-07 OpenMinis config UI L2 Batch 2 owner-backed config projection

- implementation:
  - `config.core` now preserves `ProviderAuthSourceKind` (`inline` / `env`) on selected provider config, separate from runtime-only resolved API key.
  - `ui.protocol` now owns `QueryConfigStatus` and `UiQueryResult::ConfigStatus(UiConfigStatusProjection)`; protocol state rejects this query as runtime-owned and command ingress rejects it as query-route misuse.
  - `runtime.ui-command-dispatch` now maps `QueryConfigStatus` from live selected agent config to UI-safe projection: active agent/mode/node, paired agent/mode/node, provider id/type/protocol, base URL host only, default model, auth type/source, restart-required flag.
  - `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` now verifies config status without UI.
  - WebUI Settings now renders real owner-backed agent/provider/model/auth-source values and explicit config query error state; it does not parse TOML or render credential values.
- online proof:
  - S profile restarted on `127.0.0.1:4042`.
  - `freehand-cliS adp-config-query` returned `agent=master`, `provider=minimax`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`.
  - served JS/CSS hashes matched workspace.
  - `make verify-webui-online` passed with evidence `artifacts/webui-online/20260706-verify-4042-1783376428677/summary.json`.
  - Settings evidence: `settingsAgent=master`, `settingsProvider=minimax`, `settingsProviderHost=api.minimaxi.com`, `settingsProviderAuth=credential · inline`, `settingsModel=MiniMax-M3`, `settingsConfigError=none`, `apiKeyTextVisible=false`, `secretTextVisible=false`.
- validation:
  - `cargo test -p freehand-config -- --nocapture` -> 14 passed.
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 45 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 75 passed.
  - `cargo test -p freehand-cli -- --nocapture` -> 13 passed.
  - `cargo test -p freehand-server -- --nocapture` -> 12 passed.
  - `node --check apps/freehand-server/assets/webui.js` -> ok.
  - `node --check scripts/webui_verify_online.mjs` -> ok.
  - `cargo fmt --check` -> ok.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `git diff --check` -> ok.
- remaining:
  - Provider/model edit flow is still locked for future Batch 3. No config writes were implemented.
# 2026-07-07 OpenMinis config UI L2 Batch 3 provider/model edit closeout

- task:
  - Continue OpenMinis config UI closeout loop.
  - Close Batch 3 provider/model edit flow through the owner chain only: `app.webui-smoke` -> `ui.protocol` -> `runtime.ui-command-dispatch` -> `config.core`.
- implementation:
  - `config.core` owns `ProviderConfigUpdate` and `update_provider_config_in_path`; valid env-var auth updates are persisted atomically, invalid URLs fail before overwrite, and saved config writes `api_key_env` instead of resolved credential values.
  - `ui.protocol` owns `UiProviderConfigUpdate` and `UiCommand::UpdateProviderConfig`; command validation rejects empty fields and unsupported protocol, and serialization has no raw credential field.
  - `runtime.ui-command-dispatch` routes config updates to `config.core`, stores pending restart-required config projection, and does not hot-reload the active runtime provider/model before restart.
  - WebUI Settings form now submits provider endpoint/default model/credential env var via protocol command, shows visible save failure or restart-required success, and re-queries config projection.
  - `scripts/webui_verify_online.mjs` now validates invalid and valid Settings saves online; Settings credential-leak checks scan the Settings surface instead of the whole conversation body, because provider/runtime error text in chat is not the Settings config projection.
  - `scripts/install-launchd.sh restartS` refreshes S symlink binaries, rewrites plist, service-scope reloads launchd, and sources env file from ProgramArguments; `xtask` gate now accepts the XML-escaped plist string for the env-source snippet.
- validation:
  - Restored real `~/.freehand/config.toml` and `~/.freehand/daemonS.env` from `/tmp/freehand-config-verify.1szrlj` before live reasoning.
  - Temporarily added verification env vars to `daemonS.env` only for the valid env-auth save branch, then restored both files again after browser proof.
  - `scripts/install-launchd.sh restartS`; `curl -4fsS http://127.0.0.1:4042/health`; `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`.
  - Online browser evidence: `artifacts/webui-online/20260707-verify-4042-1783388440427/summary.json`; all checks true, including invalid update visible, valid update restart-required, no Settings credential leak, multi-round failed-tool continuation terminal, mobile Settings drawer, and no stale live animation.
  - Post-restore query: `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` returned `auth_source=inline`, confirming real config/env were restored.
  - Final gates: `cargo test -p freehand-config -- --nocapture` -> 16 passed; `cargo test -p freehand-ui-protocol -- --nocapture` -> 47 passed; `cargo test -p freehand-runtime -- --nocapture` -> 77 passed; `cargo test -p freehand-server -- --nocapture` -> 12 passed; `cargo test -p freehand-cli -- --nocapture` -> 13 passed; `cargo test -p xtask ci_cd -- --nocapture` -> 4 passed; `node --check apps/freehand-server/assets/webui.js`; `node --check scripts/webui_verify_online.mjs`; `cargo fmt --check`; `git diff --check`; `cargo run -p xtask -- mainlines generate`; `cargo run -p xtask -- mainlines check`; `cargo run -p xtask -- gates check`.
- remaining:
  - Batch 3 does not implement model groups, provider health checks, secret store, Android native pre-connection redesign, or release 4041 proof.
  - Android/release proof is separate because this batch only claims S-profile WebUI 4042.

# 2026-07-07 OpenMinis config UI L2 Batch 4 release propagation trace

- scope:
  - Close release/Android propagation slice after provider/model Settings batch.
  - Keep Minis screenshots as functional IA reference only, not visual style source.
- Minis screenshot-derived future gaps:
  - provider/model groups/token usage/appearance/skills/persona or SOUL.md/memory/MCP/env vars/storage/shared folders/mount external folder/permissions/background notifications/logs/about/privacy/feedback.
  - These must become owner-backed future slices; do not add fake editable controls before owner truth exists.
- release 4041 trace:
  - Initial release served stale WebUI assets after Batch 3.
  - `scripts/install-global.sh` plus release restart updated release assets; `http://100.66.1.82:4041/assets/webui.js` and `.css` hashes now match workspace.
  - First release online verifier failed because release `restart` did not rewrite launchd env/plist before restart, so newly added verification env vars did not enter the daemon process; a wrong CLI path also contributed to that run.
  - Fix: `scripts/install-launchd.sh restart` now calls `write_launchd_env` and `write_launchd_plist` before `restart_launchd`, matching `restartS` propagation semantics.
  - Release online proof after fix: `artifacts/webui-online/20260707-verify-4041-1783390967374/summary.json`; Settings invalid update visible, valid update restart-required, no Settings secret leak, mobile Settings drawer, and multi-round samples terminal.
  - After validation, release config/env were restored; `freehand-cli adp-config-query --url ws://100.66.1.82:4041/adp` reports `auth_source=inline`.
- validation:
  - `bash -n scripts/install-launchd.sh`
  - `cargo test -p xtask ci_cd -- --nocapture` -> 4 passed.
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
  - `curl -4fsS http://100.66.1.82:4041/health` -> `ok`.
  - workspace/release hash match:
    - JS `61e92a96c82ce0d4456fcce7896b03b6ae93f69f1b17ae6d7e713b84d77c9f1f`
    - CSS `1ab81aa107b1ba52da0721d162d3f120fd519b81b2315b609c9cf1c719c83b03`
  - current `adb devices -l` has no connected devices, so Android true-device proof is blocked and not claimed.

# 2026-07-07 Minis screenshot functional fit L1 report

- task:
  - Jason clarified that before implementing screenshot-derived Minis functions, first determine what should be built and whether it fits Freehand.
- scope:
  - L1 report-only. No product code, runtime config, launchd, release, or Android device mutation.
- owner checks:
  - read feature map and function maps for `config.core`, `app.webui-smoke`, `app.android-client`, `instruction.capability-loader`, `reason.rewrite-policy`, `task.orchestration`, `debug.core`, and `tool.registry`.
- report:
  - wrote `docs/loops/openminis-config-ui-closeout/minis-screenshot-l1-report.md`.
  - accepted near-term: provider registry/status, model/default update, connection diagnostics, sessions/workspace presentation, skills read-only registry, task read-only status, diagnostics/log summary.
  - design-first: token usage, appearance persistence, persona/SOUL.md, memory CRUD, MCP, env-var manager, storage/shared folders/mounts, permissions, background/notifications, model groups.
  - rejected now: copying Minis style, UI-local fake controls, raw API-key editor, duplicate Android connected settings, marketplace-like toggles without owner, archive product surface revival.

# 2026-07-07 correction: Minis phone-local mounts do not map to Freehand

- Jason corrected the L1 fit report:
  - Minis runs the agent on the phone, so its storage/shared-folder/task-mount IA is phone-local execution IA.
  - Freehand phone is a UI; execution and filesystem authority live on the computer daemon.
- correction:
  - Do not treat Minis phone-local mount/share features as Freehand phone task-mount requirements.
  - If Freehand needs storage/workspace UX, it should be a computer-daemon workspace/files projection or permission/status design, not phone-local mount semantics.

# 2026-07-07 provider edit Batch 3 audit

- objective attachment required auditing OpenMinis config UI Batch 3 provider/model edit completion.
- evidence checked:
  - `docs/goals/openminis-config-ui-provider-edit-plan.md`
  - function/test maps for `config.core`, `ui.protocol`, `runtime.ui-command-dispatch`, `app.webui-smoke`
  - code symbols `ProviderConfigUpdate`, `update_provider_config_in_path`, `UiCommand::UpdateProviderConfig`, `RuntimeCommandDispatcher::dispatch_update_provider_config`, `submitProviderConfigUpdate`
  - online artifact `artifacts/webui-online/20260707-verify-4042-1783388440427/summary.json`
- issue found:
  - provider config save path was functionally owner-backed, but WebUI command status displayed internal strings `provider_config_saved_restart_required -> config.core`.
- fix:
  - WebUI maps provider config receipts to `Provider config saved. Restart required.`
  - follow-up audit found the first mapping used a generic success fallback for unknown receipt status; fixed to throw `Config save returned an unexpected service status.` instead of pretending success.
  - server asset smoke rejects the old provider-save status template.
  - server asset smoke rejects the generic provider config success fallback.
  - online verifier asserts provider valid-save command status does not contain `provider_config_saved_restart_required` or `config.core`.
- validation:
  - owner tests passed: config 16, ui-protocol 47, CLI 13, runtime provider update success/no-overwrite tests, server asset smoke.
  - `make verify-webui-online` passed with `artifacts/webui-online/20260707-verify-4042-1783399680000/summary.json`; valid save status is `Provider config saved. Restart required.`, Settings secret scan passed, page/console errors empty.
  - restart-after-save proof temporarily saved `default_model=MiniMax-M3-Restart-Proof` with env-var auth, restarted S-profile, and `adp-config-query` still returned the updated model/auth source; trap restored real config afterward.
  - real `~/.freehand/config.toml` and `daemonS.env` restored; `freehand-cliS adp-config-query` reports `auth_source=inline`.
# 2026-07-07 Android/WebUI phone top chrome cleanup

- user issue: phone WebUI displayed non-actionable top chips (`Master`, `Task cwd`, raw `runtime-turn-*`) above the conversation, creating visual noise and exposing internal runtime/session plumbing.
- root source:
  - `apps/freehand-server/src/page.rs` hardcoded `work-context-tags` and the three chip buttons.
  - `apps/freehand-server/assets/webui.js::renderTurnMeta` wrote worker/cwd/turn id values into those tags.
  - `apps/freehand-server/assets/webui.css` reserved phone header space for the tags.
- fix:
  - physically removed the `work-context-tags` DOM from the conversation header.
  - removed JS writes to `worker-context-tag`, `task-context-tag`, and `transport-context-tag`.
  - removed CSS for those tags and changed the conversation header to only keep the real turn status on the right.
  - server asset smoke now asserts those ids/classes are absent.
  - local skill now records that phone/WebUI visible chrome must not expose non-actionable internal runtime labels; put diagnostic detail behind Status/Debug/Settings instead.
- verification:
  - `node --check apps/freehand-server/assets/webui.js`
  - `cargo test -p freehand-server -- --nocapture`
  - `scripts/install-global.sh` full release build/regression/install passed.
  - `scripts/install-launchd.sh restart`; release health `http://100.66.1.82:4041/health` returned `ok`.
  - served asset hashes matched workspace:
    - `webui.js` `fbe9194e1da3e19dde7b54484738d287ead42e1db211c7aef499ed41840827a0`
    - `webui.css` `40350269f9da1cb067f17bf1f7910a5e049a3d061509aa3a1acb465b9b93ba0e`
  - APK installed to `100.104.163.65:5555`.
  - `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` passed with artifact `artifacts/android-device/20260707T073843Z-100.104.163.65_5555-33325`; layout log reports `shape=tall_phone`, `conversationPrimary=true`, and session/detail drawers offscreen.

# 2026-07-07 phone whitespace and Settings IA correction

- user issue:
  - Phone WebUI still had a large blank strip between conversation content and the fixed composer.
  - Settings displayed read-only runtime/status cards (`Connection`, `Active agent`, sessions/workspace, skills/files/tasks/diagnostics) that could not be edited, so they were status/debug information rather than settings.
- root source:
  - `apps/freehand-server/assets/webui.css` used `padding-bottom: min(52svh, 420px)` for phone portrait conversation scroll space, which reserved up to half the viewport even when no content needed it.
  - `apps/freehand-server/src/page.rs` hardcoded non-actionable Settings cards, and `apps/freehand-server/assets/webui.js::renderSettingsShell` populated them from local/runtime status.
  - `docs/function-maps/app.webui-smoke.md` and `docs/testing/app.webui-smoke.md` still described Settings as a config/status drawer, encouraging fake disabled status controls.
- fix:
  - Settings now shows only the owner-backed provider/model/auth-env edit form.
  - Read-only connection, active-agent, sessions/workspace, skills/files/tasks, and diagnostics cards are absent from Settings; they must move to Status/Debug/future owner-backed surfaces only when actionable.
  - Phone portrait conversation padding now reserves composer safe-area height instead of half the viewport.
  - Asset and online verifiers now assert provider/model Settings presence and read-only status card absence.
- verification plan:
  - local syntax/unit/gate checks, then release install/restart on fixed `100.66.1.82:4041`.
  - compare served JS/CSS hashes against workspace.
  - run Android true-device WebView verifier against `100.104.163.65:5555` and visually inspect screenshot for reduced blank space and Settings IA.

# 2026-07-07 WebUI mobile focused composer and Final summary closeout

- objective:
  - Close focused mobile composer blocking noise and dense Final/Summary rendering.
  - Keep work inside `app.webui-smoke`; no ADP/protocol/reasoning/session truth changes.
- implementation:
  - Added plan doc `docs/goals/webui-mobile-composer-final-summary-closeout-plan.md`.
  - Updated `docs/testing/app.webui-smoke.md` and `docs/function-maps/app.webui-smoke.md` to lock focused composer and Final summary behavior.
  - `apps/freehand-server/assets/webui.css` keeps focused phone/tall/tablet portrait composer compact and keeps `.composer-control-strip`, `#attachment-tray`, and `#command-status` hidden instead of reopening attachment/CWD/model/status into the main input area.
  - `apps/freehand-server/assets/webui.js` routes final rows through `renderFinalSummary()` / `finalSummaryBlocks()` and splits long summary lines into structured `.final-summary-item` blocks without changing protocol/session truth.
  - `apps/freehand-server/src/lib.rs` asset smoke locks the new CSS/JS classes and functions.
- validation:
  - local: `node --check apps/freehand-server/assets/webui.js`; `cargo test -p freehand-server -- --nocapture` -> 13 passed; `cargo fmt --check`; `git diff --check`; `cargo run -p xtask -- mainlines check`; `cargo run -p xtask -- gates check`.
  - S profile: `scripts/install-launchd.sh restartS`; `curl -4fsS http://127.0.0.1:4042/health` -> `ok`; `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - served asset hashes matched workspace:
    - JS `172c210d8410f093a88a3d0b54a69e80cf5bac5b0269e83fda6ea5822a89e167`
    - CSS `0ebf4c8f03521d0d38de87ad8c1e7b2545781e0b4c0cc36dc9192f42deec2d6b`
  - real browser mobile proof: `artifacts/webui-online/mobile-summary-1783427720726/summary.json`, screenshots `01-focused-composer.png` and `02-final-summary.png`.
  - browser checks all true: `layoutShape=tall_phone`, focused composer height `110`, control strip/tray/status display `none`, no `no draft attachments`/CWD/model visible, one `.final-summary`, three `.final-summary-item`, no page/console errors.
- remaining:
  - Release 4041 / Android true-device proof not run for this dev closeout; run only if promoting this slice to release/phone surface.

# 2026-07-07 Final summary source-format correction

- correction:
  - Final/Summary rendering must reflect actual terminal response format, not hardcoded business wording or punctuation-based inferred structure.
  - Plain one-line `Summary:` source must render as one readable block.
  - Explicit source newlines / line-start labels / numbering may render as multiple blocks.
- root source:
  - `terminalSummaryLine()` only extracted the first `Summary:` line, so multi-line summary source could be lost before rendering.
  - `splitFinalSummaryInlineStructure()` / `inlineStructureIndexes()` split inside a single line, which could invent layout from punctuation and content shape.
- fix:
  - `terminalSummaryBlock()` now extracts the complete `Summary` block until `Evidence`, `Learned`, or `Completion reason`.
  - Removed inline structure splitting helpers; `finalSummaryBlocks()` now splits only by actual source newlines and parses each line independently.
  - Asset smoke now requires `terminalSummaryBlock` / `normalizeFinalSummaryLine` and rejects the old inline split helpers.
  - Function map, test design, goal plan, CACHE, and local skill now lock source-format preservation instead of long-text forced splitting.
- verification:
  - local: `node --check apps/freehand-server/assets/webui.js`; `cargo test -p freehand-server -- --nocapture` -> 13 passed; `cargo fmt --check`; `git diff --check`; `cargo run -p xtask -- mainlines check`; `cargo run -p xtask -- gates check`; `node --check scripts/webui_verify_online.mjs`.
  - S profile: `scripts/install-launchd.sh restartS`; `curl -4fsS http://127.0.0.1:4042/health` -> `ok`; `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`; served JS/CSS hashes matched workspace.
  - online browser proof: `artifacts/webui-online/summary-format-1783430851153/summary.json`; screenshots `02-plain-terminal.png` and `03-structured-terminal.png`.
  - online checks: plain ADP summary had one source line and DOM rendered one item; structured ADP summary had three source lines and DOM rendered three matching items; no visible Evidence/Learned/Completion reason; no page/console errors.

# 2026-07-07 Android APK install after Summary fix

- action:
  - Ran `scripts/install-global.sh`; release build/regression/install completed and produced `dist/android/freehand-android-release-unsigned.apk`.
  - Restarted release daemon with `scripts/install-launchd.sh restart`.
  - Verified release 4041 through Tailscale: `http://100.66.1.82:4041/health` returned `ok`, ADP smoke passed, and served `webui.js` / `webui.css` hashes matched workspace.
  - Installed release APK to device `100.104.163.65:5555` through `FREEHAND_ANDROID_APK=dist/android/freehand-android-release-unsigned.apk apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555`.
- evidence:
  - APK hash: `fe036c5505f0345c5a9d1726af1476ccb273f1e716ef9044ae40f305c6eb8b20`.
  - Install log: `artifacts/android-device/20260707T134429Z-100.104.163.65_5555-97306/install.txt` shows `Success`.
  - Device package state: `pm path com.freehand.android` returns installed package path; `dumpsys package` reports `versionCode=1`, `versionName=0.1.0`, `lastUpdateTime=2026-07-07 21:44:32`.
- blocker:
  - True Android UI verification is blocked by `device_locked_or_dreaming`; artifact `artifacts/android-device/20260707T134429Z-100.104.163.65_5555-97306/summary.json`.
  - This run proves APK install, not foreground WebView acceptance.

# 2026-07-07 WebUI phone no-left-edge and compact focused composer closeout

- request:
  - First commit previous state, then fix phone WebUI cards: no space-consuming borders, keep colored backgrounds.
  - Remove the large input/composer obstruction above the mobile input.
- prior checkpoint:
  - Previous summary work was already committed as `6fb98d0 fix(webui): tighten mobile summary rendering`.
- root source:
  - `apps/freehand-server/assets/webui.css` mobile v3 reintroduced left state strips with `box-shadow: inset 2px 0 0 ...` for assistant and tool cards.
  - The later mobile focused override reset `.conversation-region` to `padding-bottom: min(46svh, 330px)` and focused composer to `max-height: min(44svh, 330px)`, overriding the earlier compact mobile rules.
  - `.final-summary-item` still inherited desktop `padding-left: 10px` and `border-left: 2px`.
- implementation:
  - Mobile assistant/tool states now use whole-card color backgrounds (`#eef3fb`, `#edf6ef`, `#f8ece9`) and `box-shadow: none`.
  - Mobile final summary items now use `padding-left: 0` and `border-left: 0`.
  - Focused mobile conversation padding is `calc(112px + env(safe-area-inset-bottom))`; focused composer max height is `132px`; focused input is `68px..76px`.
  - `apps/freehand-server/src/lib.rs` asset smoke rejects `padding-bottom: min(46svh, 330px)` and `inset 2px 0 0`.
  - `scripts/webui_verify_online.mjs` now accepts `FREEHAND_WEBUI_DEBUG_PORT`, probes mobile computed styles, and checks `mobileNoLeftEdgeIndicators`, `mobileFocusedComposerCompact`, and `mobileFocusedNoLeftEdgeIndicators`.
  - Function map, test design, and local skill now lock mobile no-left-edge and compact composer behavior.
- validation:
  - Local: `node --check apps/freehand-server/assets/webui.js`; `node --check scripts/webui_verify_online.mjs`; `cargo test -p freehand-server -- --nocapture`; `cargo fmt --check`; `git diff --check`; `cargo run -p xtask -- mainlines check`; `cargo run -p xtask -- gates check`.
  - S-profile: `scripts/install-launchd.sh restartS`; health `ok`; `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`; served JS/CSS hash matched workspace.
  - Online browser proof: `artifacts/webui-online/20260707-verify-4042-1783433140002/summary.json`; screenshot `26-mobile-focused-composer.png`; checks true for no left-edge indicators and compact focused composer. Probe measured assistant/tool/final `borderLeftWidth=0px`, `boxShadow=none`, final `paddingLeft=0px`, composer card height `92`, input height `76`, padding `112px`.
  - Release: `scripts/install-global.sh` completed full Rust/workspace/Android JVM/release APK path; `scripts/install-launchd.sh restart`; release `http://100.66.1.82:4041/health` `ok`; release ADP smoke passed; served JS/CSS hashes matched workspace.
  - Android: `FREEHAND_ANDROID_APK=dist/android/freehand-android-release-unsigned.apk apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` passed; artifact `artifacts/android-device/20260707T142056Z-100.104.163.65_5555-51679/summary.json`; layout log `shape=tall_phone`, `conversationPrimary=true`.
- notes:
  - Browser plugin was unavailable (`agent.browsers.list()` returned `[]`); the accepted online browser evidence used the repo Chrome/CDP verifier with `FREEHAND_WEBUI_DEBUG_PORT=9237`.
  - The verifier provider-config branch was run under config/env backup and restore, then S-profile was restarted; final `adp-config-query` returned `auth_source=inline`.

# 2026-07-07 workspace-owned session taxonomy

- context:
  - Jason clarified the multi-agent model before implementation.
  - Session must not belong to a worker.
  - Worker is a schedulable resource slot.
  - Workspace is cwd-bound and owns the durable session truth.
  - Execution is the worker runtime activity attached to a workspace.
- durable doc:
  - added `docs/design/workspace-session-execution-taxonomy.md`.
  - linked it from `docs/design/design-doc-index.md`.
- key rule:
  - worker attaches to a workspace session, inherits that workspace context, runs an execution, and writes admitted results back through owner APIs.
  - worker must not carry one workspace session into another workspace.

# 2026-07-07 worker pool and passive master task creation rule

- Jason clarified next multi-agent interaction rules:
  - master startup config controls worker resource quantity.
  - workers need a default English display-name pool of 20 names; after the pool is exhausted, generated workers use sequence names.
  - master is a global coordinator and must not directly do work outside its allowed workspace boundary.
  - if user asks for work in another cwd, model should choose schema/tool action to create/select workspace/task/agent.
  - framework is passive: it provides prompt contracts, schemas, validators, built-in tools, and owner APIs; it must not sniff raw paths or assistant prose and create tasks by itself.
- doc update:
  - extended `docs/design/workspace-session-execution-taxonomy.md` with `Worker Resource Pool Startup` and `Master Request Handling Contract`.

# 2026-07-07 task model refinement

- Jason asked to refine task semantics before continuing multi-agent design.
- design update:
  - task is now explicitly a workspace-scoped work item, not a worker and not a session.
  - task references `workspace_id`, `session_id`, source master session/turn, and target cwd.
  - execution is the worker runtime activity bound to one task/workspace/session/worker.
  - task can have multiple historical executions; default active execution policy is one active execution per assigned worker unless a later shard design changes it.
  - child tasks inherit workspace/session by default and can target another workspace only through explicit workspace-selection fields.
  - worker submits review; master/reviewer approves/rejects; worker does not close tasks unilaterally.
- docs updated:
  - `docs/design/workspace-session-execution-taxonomy.md`
  - `docs/design/task-orchestration-design.md`

# 2026-07-07 Reasonix context economy and search/decision split

- Jason added two Reasonix-inspired context design points:
  - failed tool/execution attempts are useful only until successful repair; after success, future model-visible history should prefer the successful path and prune raw failed attempts from cache-hit context.
  - broad search should run in clean small-context workers or independent agents, then return typed conclusions to the main model for analysis/decision.
- durable rule:
  - pruning failed attempts is prompt-history pruning only; debug/replay/error/task ledger truth remains durable.
  - do not prune unrepaired failures, active blockers, or failures lacking audit/debug truth.
  - parent/master context ingests typed final conclusions, not raw worker search transcripts.
- docs updated:
  - `docs/design/reason-context-planner-design.md`
  - `docs/design/reason-rewrite-policy-design.md`
  - `docs/design/workspace-session-execution-taxonomy.md`

# 2026-07-07 task dispatch flow refinement

- task dispatch flow was refined as a passive model-driven lifecycle:
  - master user turn enters conversation truth.
  - model emits status schema for intent only.
  - workspace selection/creation happens only through admitted action tools.
  - task creation validates workspace/session/task contract.
  - dispatch policy selects capabilities, model tier, parallelism, and context profile.
  - worker claim creates execution bound to task/workspace/session/worker.
  - broad search uses `clean_search`; worker returns typed conclusion; main model decides next step.
  - worker submits review; master/reviewer approves/rejects/closes.
- docs updated:
  - `docs/design/workspace-session-execution-taxonomy.md`
  - `docs/design/task-orchestration-design.md`

# 2026-07-07 active task dispatch prompt and condition rules

- Jason clarified the main design need is prompt and condition judgment, not only task/runtime mechanics.
- Design update:
  - `docs/design/multi-agent-dispatch-alignment.md` now defines master prompt contract, dispatch condition matrix, dispatch action schema shape, and wait/follow-up prompt contract.
  - Active dispatch means the master model proactively calls collaboration actions such as spawn/send/wait/resume/close when conditions match.
  - Framework remains passive: validate, persist, schedule, subscribe, and project. It must not sniff text/path and create tasks.
  - `docs/design/task-orchestration-design.md` links to the prompt/condition contract and remains the durable task-state owner after admitted actions.
# 2026-07-08 single-agent headless CLI sample closeout slice

- scope:
  - Continued `docs/goals/single-agent-closeout-before-multi-agent-plan.md`.
  - Stayed in single-agent scope; no worker pool, subagent, scheduling, or topology code.
- owner:
  - `app.cli-runtime-smoke`.
  - Read/updated function map, test design, and mainline-call manifest before/with code changes.
- implementation:
  - `adp-turn-sample` now accepts `success`, `failure`, `schema-mismatch`, and `provider-retry`.
  - Turn sample output now reports `rounds`, `tool_executions`, `failed_tools`, `schema_retries`, and `provider_retries`.
  - Added `session-continue-sample --url ...` to submit two prompts into one isolated session and verify the second answer contains a token from the first turn.
  - Added shared ADP transcript query helper and submit helper in CLI.
  - Added CLI mock WebSocket tests for schema mismatch, provider retry, and session continuation.
  - Fixed mock WebSocket close handling so transcript-query reconnects are not blocked by a stale first connection.
- local validation:
  - `cargo fmt --check`
  - `cargo test -p freehand-cli -- --nocapture` -> 16 passed
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
- online S-profile validation:
  - `scripts/install-launchd.sh restartS` rebuilt CLI/server/daemon symlinks and restarted `com.freehand.daemonS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - `freehand-cliS adp-turn-sample --sample success` -> success, session `cli-adp-sample-success-1783478883555866000`, `runtime-turn-162`, `rounds=1`, `tool_executions=0`.
  - `freehand-cliS adp-turn-sample --sample failure` -> success, session `cli-adp-sample-failure-1783478895802142000`, `runtime-turn-163-r2`, `rounds=2`, `tool_executions=1`, `failed_tools=1`.
  - `freehand-cliS session-continue-sample` -> success, session `cli-session-continue-1783478906701887000`, turns `runtime-turn-164,runtime-turn-165`, `restored_closed_turns=1`, token recovered in second terminal answer.
- not closed:
  - `freehand-cliS adp-turn-sample --sample schema-mismatch` returned `ADP schema-mismatch sample transcript missing expected evidence`; live model did not produce schema-polishing evidence.
  - `freehand-cliS adp-turn-sample --sample provider-retry` returned exit 2 with a `Blocked` turn, not provider-domain retry evidence.
  - Therefore schema/provider sample commands exist and fail transparently, but the live black-box branches need a controllable provider fixture/error-injection path before they can be marked closed.
  - At this point `task-lifecycle-sample` was still not implemented in CLI; see follow-up entry below.

# 2026-07-08 task lifecycle headless sample follow-up

- implementation:
  - Added `freehand-cli task-lifecycle-sample --url ...`.
  - The command submits one model-driven task-tool prompt, then verifies owner truth through ADP task list/history.
  - It requires a task whose title/goal contains a generated token, `status=closed` (case-insensitive), and history events `Created`, `ReviewSubmitted`, `ReviewApproved`, `Closed` (case-insensitive).
  - CLI does not mutate task truth directly; task mutation remains through model/tool/runtime owner path.
- local validation:
  - `cargo fmt --check`
  - `cargo test -p freehand-cli -- --nocapture` -> 17 passed
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- online S-profile validation:
  - `scripts/install-launchd.sh restartS`
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`
  - `freehand-cliS task-lifecycle-sample --url ws://127.0.0.1:4042/adp`
- online result:
  - First run created and closed `task-1783479423`, but CLI initially rejected lowercase `status=closed`; fixed CLI status/event comparison to be case-insensitive.
  - Later runs timed out waiting for command receipt even with 240s submit wait.
  - ADP truth after timeout: task list `task-1783479423:closed:50,task-1783479617:running:50,task-1783479801:running:50`.
  - Session truth after timeout: `cli-task-lifecycle-1783479397041761000:13:success`, `cli-task-lifecycle-1783479586638802000:6:submitted`, `cli-task-lifecycle-1783479794714560000:20:waiting_model`.
- not closed:
  - `task-lifecycle-sample` command exists and is locally tested, but live online task lifecycle is not stable/closed. The current blocker is model/tool-loop controllability and long-running task lifecycle receipt, not ADP task query capability.

# 2026-07-08 deterministic ADP task lifecycle command closeout

- scope:
  - Continued `docs/goals/single-agent-closeout-before-multi-agent-plan.md`.
  - Stayed in single-agent scope; no worker pool, subagent spawn, scheduling, or master/worker topology work.
- implementation:
  - `ui.protocol` now has protocol-owned task mutation commands: `CreateTask`, `SubmitTaskReview`, `ApproveTaskReview`, `CloseTask`.
  - `runtime.ui-command-dispatch` routes those commands into `TaskRuntime::create_task`, `submit_review`, `approve_review`, and `close_task`.
  - The new UI-task actor helper is `ui_task_actor`; existing model/tool task bridge helper `task_actor(turn)` remains separate for control/tool-originated task mutations.
  - `freehand-cli task-lifecycle-sample --url ...` now uses deterministic ADP task mutation commands instead of a model prompt, then verifies task list/history truth.
  - Updated function maps, test designs, mainline-call JSON, and regenerated generated wiki docs for `ui.protocol`, `runtime.ui-command-dispatch`, and `app.cli-runtime-smoke`.
- local validation:
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 47 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 77 passed.
  - `cargo test -p freehand-cli -- --nocapture` -> 17 passed.
  - `cargo fmt --check`.
  - `cargo run -p xtask -- mainlines generate`.
  - `cargo run -p xtask -- mainlines check`.
  - `cargo run -p xtask -- gates check`.
  - `git diff --check`.
- online S-profile validation:
  - `scripts/install-launchd.sh restartS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - `freehand-cliS task-lifecycle-sample --url ws://127.0.0.1:4042/adp` -> success, session `cli-task-lifecycle-1783481386959762000`, task `task-cli-FHTASK1783481386960124000`, status `closed`, events `TaskCreated,TaskAssigned,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`.
  - `freehand-cliS adp-turn-sample --sample success` -> success, session `cli-adp-sample-success-1783481399488508000`, turn `runtime-turn-171`, `rounds=1`.
  - `freehand-cliS adp-turn-sample --sample failure` -> success, session `cli-adp-sample-failure-1783481399610769000`, turn `runtime-turn-172-r3`, `rounds=3`, `failed_tools=1`, `schema_retries=1`.
  - `freehand-cliS session-continue-sample` -> success, session `cli-session-continue-1783481400025261000`, turns `runtime-turn-173,runtime-turn-174`, `restored_closed_turns=1`.
  - `freehand-cliS adp-turn-sample --sample schema-mismatch` -> success, session `cli-adp-sample-schema-mismatch-1783481424586054000`, turn `runtime-turn-175-r2`, `rounds=2`, `schema_retries=1`.
- not closed:
  - `freehand-cliS adp-turn-sample --sample provider-retry` still fails correctly. The live model produced prose claiming provider retries, but ADP/session truth had no provider-domain retry evidence. This needs a controllable provider fixture/error-injection path; prompt-only sampling is insufficient and must not be accepted as proof.

# 2026-07-08 provider retry online fixture proof

- scope:
  - Continued `docs/goals/single-agent-closeout-before-multi-agent-plan.md` under single-agent closeout.
  - Stayed in `app.cli-runtime-smoke`; no multi-agent worker/subagent/topology work.
- implementation:
  - Added `scripts/verify-provider-retry-online.sh`.
  - The script starts a local Anthropic-compatible HTTP 500 fixture on `127.0.0.1:18081`, temporarily adds `FREEHAND_PROVIDER_RETRY_FIXTURE_KEY` to `~/.freehand/daemonS.env`, updates S-profile provider config through ADP, restarts `com.freehand.daemonS`, runs `freehand-cliS adp-turn-sample --sample provider-retry`, queries provider-domain error-center rows, queries session truth, and restores original config/env through a trap.
  - The fixture is stopped only through its explicit PID; no broad process kill.
  - Updated `app.cli-runtime-smoke` feature map, function map, test design, mainline-call JSON, generated wiki, and goal plan to include this online proof path.
- online proof:
  - `scripts/verify-provider-retry-online.sh` passed.
  - Evidence: `provider_retry_online_ok session=cli-adp-sample-provider-retry-1783482215008491000 mock_attempts=5`.
  - Sample output included `runtime-turn-178`, `provider_retries=1`, and terminal provider failure `command_dispatch_port_failure`.
  - Error-center query returned five provider rows: four `retry_same_step` rows plus one `fail_turn`, all carrying concrete code `anthropic_http_status_500`.
  - Session truth reported `cli-adp-sample-provider-retry-1783482215008491000:1:failed`.
  - Post-run config restoration verified by `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp`: `base_url_host=api.minimaxi.com default_model=MiniMax-M3 auth_source=inline`.
- local validation:
  - `bash -n scripts/verify-provider-retry-online.sh`.
  - `scripts/verify-provider-retry-online.sh`.
  - `cargo test -p freehand-cli -- --nocapture` -> 17 passed.
  - `cargo test -p freehand-runtime live_bridge_retries_recoverable_provider_errors_then_succeeds -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-runtime live_bridge_fails_after_five_provider_retries_with_error_code -- --nocapture` -> 1 passed.
  - `cargo run -p xtask -- mainlines generate`.
  - `cargo run -p xtask -- mainlines check`.
  - `cargo run -p xtask -- gates check`.
  - `cargo fmt --check`.
  - `git diff --check`.
- durable rule:
  - Provider retry closure must reject model prose and require provider-domain truth: fixture/upstream attempt count plus error-center rows and session terminal state.

# 2026-07-08 WebUI online verifier fixture repair

- trigger:
  - `make verify-webui-online` failed at `settingsValidUpdateRestartRequired=false`.
  - The same run had success/failure multi-round UI, mobile layout, and ADP session truth passing, but Settings valid-save failed because the verifier submitted env var `FREEHAND_WEBUI_VERIFY_CREDENTIAL` while S daemon env did not contain it.
- root source:
  - `scripts/webui_verify_online.mjs` owned the Settings valid-save test input but did not own the required daemon credential env precondition.
  - Server-side rejection was correct: `dispatch port failure: provider minimax environment variable FREEHAND_WEBUI_VERIFY_CREDENTIAL is not set`.
- implementation:
  - `scripts/webui_verify_online.mjs` now backs up `~/.freehand/config.toml` and `~/.freehand/daemonS.env`, injects a verifier-only credential env, restarts only `com.freehand.daemonS`, runs the real browser proof, then restores config/env and restarts S in `finally`.
  - Updated `foundation.workspace` function map, mainline-call JSON, generated wiki, and test design; updated `app.webui-smoke` test design to lock the verifier-owned Settings fixture.
- validation:
  - `node --check scripts/webui_verify_online.mjs`.
  - `cargo run -p xtask -- mainlines generate`.
  - `make verify-webui-online` passed.
  - Evidence: `artifacts/webui-online/20260708-verify-4042-1783483076297/summary.json`.
  - Session: `webui-session-20260708035801-6cee1118`; ADP truth `turn_ids=runtime-turn-181,runtime-turn-182,runtime-turn-182-r2`, `turns=3`, `status=success`.
  - All checks true, including `settingsValidUpdateRestartRequired`, `settingsUpdateNoSecretLeak`, `first/second submit composer cleared`, refresh preservation, `terminal2NoLive`, mobile drawer/gesture/layout checks, and clean new session.
  - Post-run config restored: `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` returned `base_url_host=api.minimaxi.com default_model=MiniMax-M3 auth_source=inline`.
  - Post-run env restored: `grep FREEHAND_WEBUI_VERIFY_CREDENTIAL ~/.freehand/daemonS.env` returned no matches.
  - `cargo run -p xtask -- mainlines check`; `cargo run -p xtask -- gates check`; `git diff --check`.
- durable rule:
  - Online WebUI verifier must own its test fixture preconditions and restore daemon config/env afterward. A missing verifier env is a verifier setup failure, not a product Settings fallback.

# 2026-07-08 repaired-failure prompt context economy

- scope:
  - Continued `docs/goals/single-agent-closeout-before-multi-agent-plan.md`.
  - Stayed in single-agent scope and `provider.reason-live-bridge` / `reason.context-planner` ownership; no worker pool, subagent, scheduling, or topology code.
- root source:
  - Restored same-session context is rebuilt in `crates/freehand-runtime/src/lib.rs` through `rebuild_session_history_from_effective_turns` -> `effective_turn_context_segments`.
  - Before this slice, all effective persisted rounds could become `SessionMemory` segments, so a repaired logical turn like `runtime-turn-7` + `runtime-turn-7-r2` could carry the superseded failed attempt into future prompt context.
- implementation:
  - `effective_turn_context_segments` now groups restored turns by logical runtime ordinal and admits only the latest round for each logical turn into rebuilt future prompt context.
  - Raw failed attempts are not deleted; they remain in persisted turn files, reason ledger, UI transcript/projection, error/debug truth, and audit surfaces.
  - Added regression `effective_context_uses_last_repaired_round_without_raw_failed_attempt`.
  - Updated `provider.reason-live-bridge` and `reason.context-planner` function maps, test designs, mainline-call JSON, generated wiki docs, and local skill guidance.
- validation:
  - `cargo test -p freehand-runtime effective_context_uses_last_repaired_round_without_raw_failed_attempt -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 78 passed.
  - `cargo fmt --check`.
- durable rule:
  - Repaired failures are prompt-history pruning only: future default prompt context should prefer the repaired/latest round, while raw failure evidence remains durable in ledgers/UI/debug/error truth.

# 2026-07-08 single-agent restart recovery proof

- scope:
  - Continued `docs/goals/single-agent-closeout-before-multi-agent-plan.md`.
  - Stayed in S-profile single-agent proof on fixed `127.0.0.1:4042`.
- online validation:
  - `scripts/install-launchd.sh restartS` rebuilt symlink/debug daemon binaries and restarted only `com.freehand.daemonS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - `freehand-cliS session-continue-sample --url ws://127.0.0.1:4042/adp` -> session `cli-session-continue-1783484106591493000`, turns `runtime-turn-183,runtime-turn-183-r2,runtime-turn-184`, terminal `success`, second turn restored `restored_closed_turns=1`.
  - `freehand-cliS task-lifecycle-sample --url ws://127.0.0.1:4042/adp` -> task `task-cli-FHTASK1783484127920815000`, status `closed`, events `TaskCreated,TaskAssigned,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`.
  - After another `scripts/install-launchd.sh restartS`, `freehand-cliS adp-session-query --session cli-session-continue-1783484106591493000` still returned `turn_ids=runtime-turn-183,runtime-turn-183-r2,runtime-turn-184`.
  - After the same restart, `freehand-cliS adp-task-query --history task-cli-FHTASK1783484127920815000` still returned the full 5-event closed task history.
- durable rule:
  - Single-agent restart recovery proof must query the same session/task ids after daemon restart; a fresh sample after restart is not recovery evidence.

# 2026-07-08 single-agent closeout final baseline

- completion audit evidence:
  - Headless ADP success/failure/schema/continuation/task lifecycle samples were already current in this goal; provider retry fixture proof passed with exactly five attempts and provider-domain error-center rows.
  - WebUI online proof remained current at `artifacts/webui-online/20260708-verify-4042-1783483076297/summary.json`, with ADP/session truth and visible UI checks all true.
  - Restart recovery proof re-queried the same session and task ids after `restartS`.
  - Repaired-failure prompt context economy is committed in `82fae02`, with owner regression and docs/function-map/test-design updates.
- final local validation:
  - `cargo build --workspace` -> passed.
  - `cargo fmt --check` -> passed.
  - `cargo clippy --workspace --all-targets -- -D warnings` -> passed.
  - Direct `cargo test --workspace` repeatedly hit a tool PTY/session-return anomaly: the tool session stayed open while `ps` showed no real cargo/rustc/test child process. These attempts were explicitly interrupted and not counted as passing.
  - Equivalent full workspace package coverage was run with `cargo metadata --no-deps --format-version 1 | jq -r '.packages[].name'` and `cargo test -p <pkg>` for all 22 packages; result `PACKAGE_TESTS_EXIT=0`.
  - `cargo run -p xtask -- mainlines check` -> passed.
  - `cargo run -p xtask -- gates check` -> passed.
  - `git diff --check` -> passed.
- acceptance audit:
  - Session/restart truth: proven by same-session continuation and post-restart same-id session query.
  - Same-session provider history: proven by `session-continue-sample` restored history and earlier WebUI continuation evidence.
  - Failed-tool repair and schema polishing: proven by ADP samples and runtime tests.
  - Provider retry/backoff terminal distinction: proven by `scripts/verify-provider-retry-online.sh`.
  - UI observability/static historical turns/tool semantics: proven by current WebUI online verifier artifact and mapped UI tests.
  - Minimal single-agent task lifecycle: proven by deterministic ADP task lifecycle and post-restart task history query.
  - Prompt context economy: proven by `effective_context_uses_last_repaired_round_without_raw_failed_attempt`.
  - Multi-agent work remains out of scope; existing dirty multi-agent design docs were not staged.

# 2026-07-08 multi-task foundation design landing

- scope:
  - Jason clarified the next phase is not more single-agent reasoning foundation; that is already closed.
  - The next phase is multi-task task-management foundation before attaching capabilities to agents.
  - Design covers Master, Worker, Framework Task Runtime, Task Center, Agent Lifecycle, timers, state sensing, prompt/tool contract, and Phase 1 boundaries.
- landed design docs:
  - `docs/design/task-center-truth.md`: global Task Center truth for BigTask/SubTask/Execution/Review/EventInbox/SchedulerTick, task registration, sync, query, and recovery.
  - `docs/design/agent-lifecycle-semantics.md`: every agent, master or worker, emits live lifecycle semantics: state, current/last activity, model/tool/error stats, runtime control channel, safe points, AgentBoard.
  - `docs/design/master-worker-task-state-machine-phase1.md`: Phase 1 state machine for one active BigTask with multiple SubTasks, MasterPollLoop, WorkerExecutionLoop, FrameworkSchedulerTick, timeout/block/review/retry/recovery.
  - `docs/design/master-worker-prompt-contract-phase1.md`: master and worker prompt/tool behavior tables for state-driven task management.
  - `docs/design/multi-task-foundation-implementation-plan.md`: staged implementation plan from owner maps to Task Center skeleton, lifecycle reducer, scheduler tick, runtime control channel, samples, and UI projection.
  - Existing `workspace-session-execution-taxonomy.md` and `multi-agent-dispatch-alignment.md` are linked as vocabulary and broader dispatch direction; Phase 1 docs constrain immediate implementation scope.
- key decisions:
  - Task Center owns "what work exists and what state it is in".
  - Agent Lifecycle owns "what each agent is doing right now".
  - Framework owns time, timers, event cursor, state sensing, timeout/stale/blocker detection, and projections.
  - Master model owns task-management decisions from TaskBoard/AgentBoard truth.
  - Worker owns execution progress/block/submission/retry updates.
  - Phase 1 avoids multiple independent BigTasks and cross-session context switching.

# 2026-07-08 phase1 tool/action surface clarification

- Jason clarified that tool count must stay small: use a few logical tools plus typed parameters, not many single-action tool names.
- Durable design added `docs/design/master-worker-tool-action-contract-phase1.md`.
- Contract:
  - tool name is owner surface; `op` is operation; typed args are payload.
  - semantic actions remain visible in docs/prompts/tests, but exposed runtime tools should be few.
  - baseline continues the existing `task(op=...)` direction; `agent` and `worker_control` are only separate tool surfaces if owner-map analysis proves they are needed.
  - invalid op/args/state returns a paired action/tool error to the model; no fallback, no guessed nearby op, no provider-failure conflation.

# 2026-07-08 tool.registry red lock for task-management actions

- Added tool registry tests that lock task-management semantic actions out of the exposed provider tool-name surface.
- `task_management_semantic_actions_are_not_exposed_as_tools` rejects standalone names such as `query_task_board`, `dispatch_subtask`, `approve_submission`, and `close_big_task`.
- `task_tool_exposes_operation_parameter` verifies `task` remains present and requires string `op`.
- This is scoped to task-management actions only. General code tools such as read/write/search/shell remain separate because they are currently aligned with Codex/Reasonix categories and WebUI tool display semantics.
- Validation:
  - `cargo test -p freehand-tools -- --nocapture` -> 29 passed
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`

# 2026-07-08 agent lifecycle and worker_control clarification

- Jason clarified: Agent Lifecycle is an agent-owned property, not a standalone task-management tool.
- Design correction:
  - Agent Lifecycle is projected through AgentBoard/lifecycle snapshots, scheduler inputs, ADP/debug query surfaces, and master context summaries.
  - Do not introduce a model-facing `agent` tool just to query lifecycle unless a later owner map proves a separate action surface is required.
  - `worker_control` is only for safe-point control of a running worker execution.
- `worker_control` can include:
  - `query_status`
  - `ask_at_safe_point`
  - `add_constraint`
  - `request_checkpoint`
  - `request_submission_now`
  - `pause`
  - `resume`
  - `cancel`
- `worker_control` must not include task creation/assignment/review/close, hidden prompt-history mutation, raw transcript rewrite, or workspace/session truth mutation.

# 2026-07-08 multi-task foundation phase1 loop target

- Added `docs/goals/multi-task-foundation-phase1-loop.md` as the first executable target for the multi-task foundation implementation loop.
- Phase 1 objective:
  - Task Center board truth
  - Agent Lifecycle truth
  - Execution binding/facts
  - Scheduler tick/timer facts
  - headless ADP/CLI query samples
- First loop excludes WebUI dashboard, Android UI, worker autoscaling, cross-machine workers, multiple independent BigTasks, cross-session master context switching, standalone model-facing `agent` lifecycle tool, and general code-tool surface redesign.
- Completion standard requires D1-D6:
  - owner/maps closeout
  - TaskBoard skeleton
  - AgentLifecycle skeleton
  - ExecutionFact sync
  - scheduler tick skeleton
  - headless proof plus restart same-id proof
- Validation for this planning step:
  - `git diff --check`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`

# 2026-07-08 multi-task phase1 D1 owner/map closeout

- Goal run started from `/Users/fanzhang/.codex/attachments/63db509b-a35f-4511-88ab-ff396ba0eff7/pasted-text-1.txt`.
- D1 owner decision:
  - `task.orchestration` owns TaskBoard, ExecutionFact sync, and SchedulerTick Phase 1 pending surface.
  - new `agent.lifecycle` owns AgentLifecycleSnapshot, AgentBoardProjection, and lifecycle reducer truth.
  - `agent.lifecycle` is initially owned in `crates/freehand-task`; split later only if implementation proves a separate crate is needed.
- Added:
  - `docs/function-maps/agent.lifecycle.md`
  - `docs/testing/agent.lifecycle.md`
  - `docs/mainline-calls/agent.lifecycle.json`
  - generated `docs/wiki/agent.lifecycle.md`
- Updated:
  - `docs/architecture/feature-map.md`
  - `docs/function-maps/task.orchestration.md`
  - `docs/testing/task.orchestration.md`
  - `docs/mainline-calls/task.orchestration.json`
  - generated `docs/wiki/task.orchestration.md`
- Validation:
  - `cargo run -p xtask -- mainlines generate`
  - `git diff --check`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`

# 2026-07-09 multi-task phase2d WebUI projection closeout

- marker:
  - `phase2d-webui-projection-closeout-1783580948045`
- scope:
  - Continued Phase 2D from the multi-task foundation goal.
  - Implemented WebUI status drawer projection for owner-backed TaskBoard, AgentBoard, EventInbox, TaskHistory, and WorkerControl.
  - Added WorkerControl drawer actions that route through protocol commands and re-query owner truth.
  - Fixed WebUI and Android command receipt projection so user-facing text does not expose `target_feature_id` or task/execution/control payload ids.
  - Fixed receipt mapping no-fallback semantics: known statuses map explicitly; unknown statuses render unsupported instead of success text.
- implementation audit:
  - WebUI stores only transient render state: `state.taskBoard`, `state.agentBoard`, `state.eventInbox`, `state.taskHistory`, `state.workerControl`.
  - WebUI Phase 2D query path uses ADP/runtime query truth: `QueryTaskBoard`, `QueryAgentBoard`, `QueryEventInbox`, `QueryTaskHistory`, `QueryWorkerControl`.
  - WorkerControl buttons submit `WorkerControl` commands and re-query owner projection after command receipt.
  - Android `AdpEventStream::commandReceiptResponse` keeps raw dispatch status in `CommandResponse.code` and safe/unsupported user text in `message`.
- local validation:
  - `jq empty docs/mainline-calls/app.android-client.json docs/mainline-calls/app.webui-smoke.json` -> ok.
  - `node --check apps/freehand-server/assets/webui.js` -> ok.
  - `node --check scripts/webui_verify_online.mjs` -> ok.
  - `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew testDebugUnitTest` -> passed.
  - `cargo test -p freehand-server webui_smoke_renders_shell_and_asset_routes -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-server -- --nocapture` -> 13 passed.
  - `cargo test -p freehand-task -- --nocapture` -> 41 passed.
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 53 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 84 passed.
  - `cargo test -p freehand-cli -- --nocapture` -> 24 passed.
  - `cargo build --workspace` -> ok.
  - `cargo fmt --check` -> ok.
  - `cargo clippy --workspace --all-targets -- -D warnings` -> ok.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `git diff --check` -> ok.
- S-profile owner proof:
  - `scripts/install-launchd.sh restartS` restarted fixed `127.0.0.1:4042`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - Phase 2A sample passed, then same-id restart verify passed for task `task-cli-master-worker-FHPHASE2A1783579160756571000`, execution `exec-cli-master-worker-FHPHASE2A1783579160756571000`, worker `worker-cli-master-worker-FHPHASE2A1783579160756571000`.
  - Phase 2B sample passed, then same-cursor restart verify passed for task `task-cli-master-poll-FHPHASE2B1783579160756557000` with `inbox_after_cursor_events=0`.
  - Phase 2C sample passed, then same-control restart verify passed for task `task-cli-worker-control-FHPHASE2C1783579160756557000` with `control_events=8`.
- WebUI online proof:
  - Final `make verify-webui-online` passed after no-fallback fix.
  - Evidence: `artifacts/webui-online/20260709-verify-4042-1783580948045/summary.json`.
  - Session: `webui-session-20260709070942-4d8550f6`.
  - Phase 2D status snapshot: task board `18 task(s) · 1 blocked · 6 review · 8 stale`; agent board `12 agent(s) · 12 active`; event inbox `30 recent event(s) · updated`; history `12 execution event(s)`; worker control `review submitted · 0 control event(s)`.
  - Checks true: `phase2TaskBoardMatchesService`, `phase2AgentBoardMatchesService`, `phase2EventInboxMatchesService`, `phase2TaskHistoryMatchesService`, `phase2WorkerControlMatchesService`, `phase2ProjectionVisible`, `phase2NoRawInternalChrome`, plus submit clearing, refresh preservation, terminal no-live, mobile drawer/layout, Settings, and clean new-session checks.
  - Screenshots include `27-phase2-projection.png`; service truth captured in `27-phase2-truth.json`.
- Android device:
  - JVM tests passed.
  - True-device proof not closed: `adb connect 100.104.163.65:5555` returned `unauthorized`; `emulator-5554` was `offline`.
- final receipt mapping audit:
  - Found WebUI/Android command receipt user text was safe against id leakage but still used substring classification such as `task_` / `worker_control`.
  - Tightened both clients to derive a dispatch status code by stripping only `:` or whitespace suffixes, then map through exact known-code whitelist.
  - Added/updated tests to prove unknown task-like statuses such as `task_unknown:*` render unsupported instead of task success text.

# 2026-07-09 control simple_question and dynamic input-budget closeout

- Jason clarified:
  - model status must expose a standard `simple_question` field for previous user input.
  - simple question/answer requests may naturally stop without long-task interception.
  - model-visible inputs must not be blocked by arbitrary small local limits; only true context overflow should reject.
  - default output budget should stay 8192 tokens, not an ad hoc smaller runtime cap.
  - task-space state machine validation can check explicit standard fields only; natural-language target text must be presented to the model for self-review, not machine semantic correction.
- Implementation:
  - replaced control-status runtime path from `simple_request` to `simple_question`.
  - added negative control test proving legacy `simple_request=true` is not an alias.
  - changed runtime `previous-visible-output` and schema-feedback carryover to content-derived admission budgets.
  - kept Anthropic output default at provider-owned `DEFAULT_ANTHROPIC_MAX_TOKENS=8192`.
  - documented task-space validation boundary in `docs/design/master-worker-task-state-machine-phase1.md`.
- Validation:
  - `cargo test -p freehand-control -- --nocapture` -> 6 passed.
  - `cargo test -p freehand-runtime live_bridge_accepts_simple_status_stop_hook_without_completion_schema -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-runtime live_bridge_admits_long_operator_task_without_semantic_truncation -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-runtime live_bridge_admits_long_previous_visible_output_without_fixed_cap -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-ui-protocol public_conversation_strips_hidden_control_status_blocks -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-tools task_tool_exposes_operation_parameter -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 90 passed.
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 53 passed.
  - `cargo test -p freehand-tools -- --nocapture` -> 29 passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, `git diff --check`, and `cargo clippy --workspace --all-targets -- -D warnings` passed.
  - `scripts/install-launchd.sh restartS` rebuilt and restarted `com.freehand.daemonS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - `freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample success` -> `adp_turn_sample_ok`, session `cli-adp-sample-success-1783589237021233000`, turn `runtime-turn-193`, `rounds=1`, `schema_retries=0`.
  - `freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample failure` -> `adp_turn_sample_ok`, session `cli-adp-sample-failure-1783589267826118000`, turn `runtime-turn-194-r2`, `rounds=2`, `tool_executions=1`, `failed_tools=1`, `schema_retries=0`.

# 2026-07-09 simple_question current-agent revalidation

- Revalidated current dirty workspace before reporting:
  - `cargo test -p freehand-control -- --nocapture` -> 6 passed.
  - `cargo test -p freehand-runtime live_bridge_accepts_simple_status_stop_hook_without_completion_schema -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-ui-protocol public_conversation_strips_hidden_control_status_blocks -- --nocapture` -> 1 passed.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `git diff --check` -> ok.
  - `cargo fmt --check` -> ok.
  - `scripts/install-launchd.sh restartS` restarted S-profile on `127.0.0.1:4042`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - `freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample success` -> session `cli-adp-sample-success-1783589888991584000`, turn `runtime-turn-195`, `rounds=1`, `schema_retries=0`.
  - `freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample failure` -> session `cli-adp-sample-failure-1783589904038500000`, turn `runtime-turn-196-r2`, `rounds=2`, `tool_executions=1`, `failed_tools=1`, `schema_retries=0`.

# 2026-07-09 task-space context layout gap

- Jason asked whether task space is arranged in context with budget and cache-aware placement.
- Current code status:
  - no first-class `task-space` / `TaskSpaceSnapshot` context segment exists yet.
  - `original-task` is currently a `SessionMemory` segment with content-derived budget via `runtime_prompt_segment_token_budget`.
  - `previous-visible-output` and `completion-schema-feedback` are `SubagentConclusion` volatile segments with content-derived budgets.
  - planner order is kind-based: `SystemAnchor`, `DeveloperPolicy`, `CompletionContract`, `SessionMemory`, `SessionSummary`, `SubagentConclusion`, `ToolResultEvidence`, `UserTurnInput`.
  - this means a future task-space snapshot should be a stable/session-stable segment after static contracts/policies and before volatile execution results/user turn.
- Gap:
  - task space exists only as design text in `docs/design/master-worker-task-state-machine-phase1.md`, not as a persisted typed context segment.
  - first-round runtime carryover includes `completion-contract`, `control-status-contract`, `runtime-tool-guidance`, and `original-task`.
  - `next_round_segments()` currently rebuilds only `completion-contract`, `original-task`, plus volatile carryover; it omits `control-status-contract` and `runtime-tool-guidance` on later rounds.
- Required next implementation:
  - add a typed task-space snapshot owner and context segment contract.
  - place it after static contracts/policies and before volatile execution results.
  - assign content-derived admission budget and keep it cache-aware.
  - add tests locking order, budget, stable-prefix hash behavior, and multi-round static guidance retention.

# 2026-07-09 context ordering and Reasonix compression audit

- Audit scope:
  - Freehand context planner/runtime/rewrite source.
  - Reasonix reference at `~/code/DeepSeek-Reasonix/internal/agent/compact.go`, `cache_shape.go`, `prune.go`, and cache-hit tests.
- Freehand current truth:
  - `plan_context` sorts by segment kind, not insertion order: `SystemAnchor`, `DeveloperPolicy`, `CompletionContract`, `SessionMemory`, `SessionSummary`, `SubagentConclusion`, `ToolResultEvidence`, `UserTurnInput`.
  - cache diagnostics treat `Stable | SessionStable` leading segments as stable prefix.
  - current live first round injects `completion-contract`, `control-status-contract`, `runtime-tool-guidance`, `original-task`.
  - `original-task` is `SessionMemory + Cacheable` with content-derived budget.
  - `previous-visible-output` and `completion-schema-feedback` are volatile `SubagentConclusion + NoCache` with content-derived budgets.
  - `reason.rewrite-policy` pure policy exists with Reasonix-aligned thresholds: soft 50%, auto 80%, force 90%, max tail 16384, max consecutive compactions 2.
  - `ReasonRewriteRuntime` can call `SessionHistory::stage_compaction` only after policy approval.
- Freehand gaps:
  - no first-class `TaskSpaceSnapshot` / task-space context segment exists.
  - production `provider.reason-live-bridge` does not wire provider usage into `ReasonRewriteRuntime`; compaction is policy/harness-level, not live runtime behavior.
  - stale volatile pruning executor is pending.
  - no Reasonix-style byte-level provider cache-hit curve test proving request prefix stability across turns.
  - no live context status projection showing cache/prefix/compaction state to UI.
  - task-space dynamic state should not be inside `SessionMemory` if it changes every turn, or it will churn stable-prefix cache.
- Reasonix reference findings:
  - prompt grows append-only for high cache hit until soft/auto/force thresholds.
  - soft threshold reports context growth without rewriting prefix.
  - prune stale large tool results before paying summarizer cost.
  - compaction preserves system head plus summarized middle plus token-bounded recent tail.
  - tail selection is token-budgeted and aligned off orphan tool results.
  - compaction archives dropped originals and emits started/done UI events.
  - cache diagnostics hash system/tools/rewrite version and report cache hit/miss usage.
  - tests simulate provider cache-hit tokens and assert byte-stable request prefixes.
- Direction:
  - implement task-space as two context surfaces: stable `TaskContract` and volatile/cache-isolated `TaskSpaceSnapshot`.
  - put task surfaces after static contracts/policies and before volatile execution evidence/user input.
  - keep ordinary turns append-only except explicit rewrite gates.
  - wire real provider usage to rewrite runtime in live bridge, then add Reasonix-style cache-hit and compaction-loop tests.

# 2026-07-09 master autonomy gap audit

- Trigger:
  - Jason clarified that UI projection and command harnesses are not enough; the real requirement is a master agent that can autonomously trigger worker task dispatch and manage success, execution error, and rejected-submission retry loops.
- Current verified state:
  - Phase 2A/2B/2C/2D prove owner truth, ADP commands, restart recovery, and UI projection.
  - `master-worker-foundation-sample` is command-driven by CLI/ADP, not model-autonomous.
  - `reason-live --agent master --prompt ...` is the closest current headless live model path.
- Live probe:
  - First long prompt failed before provider call because `original-task` context segment exceeded the current 128-token budget.
  - Compressed prompt with ids `worker-auto-1783582650`, `task-auto-1783582650`, `exec-auto-1783582650` entered the real provider path and did call `task` tools.
  - ADP owner query proved task history reached `TaskCreated,TaskAssigned,TaskResumed,TaskHeartbeat`.
  - The turn then stalled for about four minutes without advancing to execution failure, review submission, rejection, retry, approval, or close. The exact headless process was interrupted with Ctrl-C to avoid a residual process.
- Conclusion:
  - Current implementation has partial model-trigger evidence for create/assign/claim, but does not yet have complete master-autonomous worker lifecycle proof.
  - Missing acceptance coverage: worker execution error, worker success, success-but-incomplete review rejection and retry, all initiated/managed by master model decisions rather than CLI scripted commands.

# 2026-07-08 multi-task phase1 implementation closeout

- Scope:
  - Continued goal `/Users/fanzhang/.codex/attachments/63db509b-a35f-4511-88ab-ff396ba0eff7/pasted-text-1.txt`.
  - Implemented the D2-D6 headless foundation over existing D1 owner decisions.
- Implementation summary:
  - `task.orchestration`: TaskBoard query, ExecutionFact sync, SchedulerTick durable fact path.
  - `agent.lifecycle`: AgentLifecycleSnapshot, AgentBoard projection, lifecycle reducer state.
  - `ui.protocol`: Phase 1 query/command DTOs for task board, agent board, agent lifecycle, execution facts, and scheduler ticks.
  - `runtime.ui-command-dispatch`: runtime-backed query/dispatch handlers and projection helpers.
  - `app.cli-runtime-smoke`: `phase1-foundation-sample` create and verify modes.
- Local validation:
  - `cargo test -p freehand-tools -- --nocapture` -> 29 passed.
  - `cargo test -p freehand-cli -- --nocapture` -> 18 passed.
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 47 passed.
  - `cargo test -p freehand-task -- --nocapture` -> 28 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 80 passed.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `git diff --check` -> ok.
- S-profile online proof:
  - `scripts/install-launchd.sh restartS` refreshed debug daemon copy and restarted `com.freehand.daemonS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok` with subscription/query/kind-mismatch evidence.
  - `freehand-cliS phase1-foundation-sample --url ws://127.0.0.1:4042/adp` -> `phase1_foundation_sample_ok`.
  - Created ids: `blocked_task=task-cli-phase1-blocked-FHPHASE11783500244602888000`, `review_task=task-cli-phase1-review-FHPHASE11783500244602888000`, `execution=exec-cli-phase1-FHPHASE11783500244602888000`, `agent=master`.
  - Counts/state: `blocked=1`, `review_ready=1`, `stale=1`, `recovering_event=true`, `lifecycle_state=blocked`.
  - After another `scripts/install-launchd.sh restartS`, verify mode against the same ids returned `phase1_foundation_verify_ok` with the same counts/state.
- Durable rule:
  - Phase 1 restart proof requires same-id verify after restart; a fresh post-restart sample is not recovery evidence.

# 2026-07-08 framework-mediated agent operations design

- Trigger:
  - Jason asked to clarify whether Agent and Task operations go through the framework, and to complete design docs before implementing the next gap.
- Added:
  - `docs/design/framework-mediated-agent-operations.md`
  - `docs/goals/multi-task-foundation-phase2-gap-plan.md`
- Updated:
  - `docs/design/design-doc-index.md`
  - `docs/design/multi-task-foundation-implementation-plan.md`
  - `docs/architecture/architecture-gaps.md`
  - `docs/goals/multi-task-foundation-phase1-loop.md`
  - `MEMORY.md`
- Durable design truth:
  - Task operations go through Task Center / `task(op=...)`.
  - Agent registry is resource registration, not lifecycle.
  - Agent Lifecycle is intrinsic typed-event projection, not a model-facing mutation tool.
  - Future `worker_control(op=...)` is safe-point runtime control only and cannot create/assign/approve/reject/close tasks.
  - Agent-to-Agent communication target is framework queue truth: Task Center/EventInbox and worker-control inbox, not private mutation.
  - Phase 2 order is worker execution loop, then master poll/EventInbox, then worker_control, then UI projection.
- Validation:
  - `git diff --check` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.

# 2026-07-08 multi-task phase2a master-worker closeout

- marker:
  - `phase2a-master-worker-closeout-1783515402294813000`
- scope:
  - Continued goal `/Users/fanzhang/.codex/attachments/ff7f6297-abd6-4a48-9557-2d0f35dd98ac/pasted-text-1.txt`.
  - Closed Phase 2A no-UI master/worker task execution loop only.
  - Did not implement WebUI/Android dashboard, worker_control, multi BigTask, or cross-machine worker.
- implementation audit:
  - `ui.protocol` owns Phase 2A command DTO/validation for worker creation, assignment, claim-next, review rejection, task dispatch, execution facts, and lifecycle/task queries.
  - `runtime.ui-command-dispatch` routes commands thinly into `TaskRuntime`; it does not decide business next actions.
  - `task.orchestration` owns create/assign/claim/progress/blocked/recovering/review/reject/retry/approve/close mutation truth.
  - `agent.lifecycle` persists typed lifecycle projection separately from worker resource state so released workers can still have restart-queryable closed lifecycle truth.
  - `app.cli-runtime-smoke` owns `master-worker-foundation-sample` create and verify modes.
- local validation:
  - `cargo fmt --check` -> passed.
  - `cargo test -p freehand-task -- --nocapture` -> 30 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 81 passed.
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 49 passed.
  - `cargo test -p freehand-cli -- --nocapture` -> 20 passed.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `git diff --check` -> ok.
- S-profile online proof:
  - `scripts/install-launchd.sh restartS` refreshed the debug daemon copy and restarted `com.freehand.daemonS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok` with subscription, query, and explicit command-kind mismatch failure evidence.
  - `freehand-cliS master-worker-foundation-sample --url ws://127.0.0.1:4042/adp` -> `master_worker_foundation_sample_ok`.
  - Created ids: `task=task-cli-master-worker-FHPHASE2A1783515402294813000`, `execution=exec-cli-master-worker-FHPHASE2A1783515402294813000`, `agent=worker-cli-master-worker-FHPHASE2A1783515402294813000`.
  - Online sample result: `status=closed`, `blocked_seen=true`, `review_ready_seen=true`, `lifecycle_state=closed`.
  - Ordered events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskExecutionRecorded,TaskBlocked,TaskResumed,TaskHeartbeat,TaskExecutionRecovering,TaskReviewSubmitted,TaskReviewRejected,TaskResumed,TaskHeartbeat,TaskExecutionRecorded,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`.
  - After another `scripts/install-launchd.sh restartS`, verify mode against the same task/execution/agent ids returned `master_worker_foundation_verify_ok` with `status=closed`, `blocked_seen=true`, `review_ready_seen=true`, `lifecycle_state=closed`, and the same ordered events.
- durable rule:
  - Multi-task Phase 2A restart proof requires same task/execution/worker ids after daemon restart; a fresh post-restart sample is not recovery evidence.

# 2026-07-08 phase2a final revalidation and memory indexing

- reran local closeout checks:
  - `cargo fmt --check`
  - `cargo test -p freehand-task -- --nocapture` -> 30 passed
  - `cargo test -p freehand-runtime -- --nocapture` -> 81 passed
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 49 passed
  - `cargo test -p freehand-cli -- --nocapture` -> 20 passed
  - `cargo test -p freehand-tools -- --nocapture` -> 29 passed
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
- reran S-profile online proof:
  - `scripts/install-launchd.sh restartS`
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`
  - `freehand-cliS master-worker-foundation-sample --url ws://127.0.0.1:4042/adp` -> `master_worker_foundation_sample_ok`
  - latest ids: task `task-cli-master-worker-FHPHASE2A1783516065760449000`, execution `exec-cli-master-worker-FHPHASE2A1783516065760449000`, worker `worker-cli-master-worker-FHPHASE2A1783516065760449000`
  - after `scripts/install-launchd.sh restartS`, verify mode against the same ids returned `master_worker_foundation_verify_ok`
- MemoryPalace:
  - built source-only safe corpus at `/Volumes/extension/code/memory/freehand-phase2a-mempalace-corpus-safe-1783516065760449000`
  - sensitive-marker scan returned zero matches
  - `mempalace mine ... --wing freehand --agent codex` processed 24 files
  - marker search `phase2a-master-worker-closeout-1783515402294813000` returned `phase2a-master-worker-closeout.md` rank 1

# 2026-07-08 multi-task phase2b master poll/EventInbox closeout

- marker:
  - `phase2b-master-poll-closeout-1783528034427562000`
- scope:
  - Continued Phase 2B no-UI foundation only.
  - Implemented EventInbox and MasterPoll owner path; did not implement WebUI/Android dashboard, worker_control, multi BigTask, or cross-machine worker.
- implementation audit:
  - `task.orchestration` owns EventInbox projection from task ledgers, cursor truth, legacy cursor compatibility, MasterPoll cursor persistence, and classification.
  - Event cursor changed to four-part `timestamp:task_id:seq:event_id` so same timestamp/task/seq rows are globally distinguishable.
  - Legacy three-part cursor mode skips all matching duplicate-prefix rows; unknown cursor still returns explicit `CursorNotFound`.
  - `ui.protocol` owns `QueryEventInbox` and `RunMasterPoll` DTO validation, including `replay_from_start=true` conflict rejection when `after_cursor` is also supplied.
  - `runtime.ui-command-dispatch` routes EventInbox/MasterPoll to task owner without local business decisions.
  - `app.cli-runtime-smoke` owns `master-poll-foundation-sample` create and verify modes. Create mode uses `replay_from_start=true` plus omitted limits to drain backlog, then rereads the final owner-backed persisted cursor before printing verify arguments.
  - A compile warning from a test-only legacy cursor helper was fixed with `#[cfg(test)]`; `cargo clippy -p freehand-task --all-targets -- -D warnings` is green.
- local validation:
  - `cargo test -p freehand-task phase2b_ -- --nocapture` -> 5 passed.
  - `cargo test -p freehand-task -- --nocapture` -> 35 passed.
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 50 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 82 passed.
  - `cargo test -p freehand-cli -- --nocapture` -> 22 passed.
  - `cargo clippy -p freehand-task --all-targets -- -D warnings` -> passed.
  - `cargo fmt --check` -> passed.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `git diff --check` -> ok.
  - `jq empty docs/mainline-calls/task.orchestration.json docs/mainline-calls/runtime.ui-command-dispatch.json docs/mainline-calls/app.cli-runtime-smoke.json docs/mainline-calls/ui.protocol.json` -> ok.
- S-profile online proof:
  - `scripts/install-launchd.sh restartS` rebuilt and restarted `com.freehand.daemonS` without warnings.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - `freehand-cliS master-poll-foundation-sample --url ws://127.0.0.1:4042/adp` -> `master_poll_foundation_sample_ok`.
  - Created ids: task `task-cli-master-poll-FHPHASE2B1783528034427562000`, execution `exec-cli-master-poll-FHPHASE2B1783528034427562000`, worker `worker-cli-master-poll-FHPHASE2B1783528034427562000`.
  - Online sample evidence: `status=review_submitted`, `inbox_events=187`, `poll_events=0`, persisted cursor `00000000001783528036:task-cli-phase1-review-FHPHASE11783500244602888000:00000000000000000006:task-cli-phase1-review-FHPHASE11783500244602888000:6`.
  - After `scripts/install-launchd.sh restartS`, verify mode with the same task/execution/worker/cursor returned `master_poll_foundation_verify_ok` with `inbox_after_cursor_events=0`, `poll_events=0`, same persisted cursor, and classifications containing blocked/review_ready/stale.
- durable rule:
  - Phase 2B closeout must prove all three: replay-from-start full drain, final owner-backed cursor reread, and same-cursor restart verification returning zero events after cursor. Finite page limits or fresh samples after restart are not valid recovery evidence.

# 2026-07-08 phase2b post-commit workspace clippy audit

- after commit `c01032d`, ran `cargo clippy --workspace --all-targets -- -D warnings`.
- clippy found one Phase 2B runtime test style error at `crates/freehand-runtime/src/lib.rs`: `assert_eq!(poll.task_board.include_terminal, true)`.
- fixed it to `assert!(poll.task_board.include_terminal)`.
- validation:
  - `cargo test -p freehand-runtime runtime_dispatches_phase2b_master_poll_and_event_inbox -- --nocapture` -> 1 passed.
  - `cargo clippy --workspace --all-targets -- -D warnings` -> passed.

# 2026-07-09 multi-task phase2c worker.control closeout

- marker:
  - `phase2c-worker-control-closeout-1783572587648779000`
- scope:
  - Continued Phase 2C no-UI foundation only.
  - Implemented safe-point runtime control channel for already-running worker executions.
  - Did not implement WebUI/Android task dashboard, multi BigTask, cross-machine worker, or actual worker runtime interruption beyond the owner-backed control ledger/snapshot foundation.
- implementation audit:
  - `task.orchestration` owns worker-control ledger rows, snapshot projection, status query, safe-point queueing, and task-state consequences.
  - `ui.protocol` owns worker-control command/query DTO validation and UI-safe projection shape.
  - `runtime.ui-command-dispatch` routes worker-control commands/queries to task owner without owning business semantics.
  - `app.cli-runtime-smoke` owns `worker-control-foundation-sample` create and verify modes.
  - Implemented operations: `query_status`, `ask_at_safe_point`, `add_constraint`, `request_checkpoint`, `request_submission_now`, `pause`, `resume`, `cancel`.
  - `query_status` persists `observed`; safe-point ops persist `queued`; `pause`/`resume`/`cancel` route through Task Center first and persist `applied` only after the task consequence succeeds.
  - Worker control does not create, assign, claim, review, approve, reject, or close tasks; it also does not mutate prompt history, raw transcripts, workspace truth, or session truth.
- local validation already completed:
  - `cargo test -p freehand-task worker_control -- --nocapture` -> 6 passed.
  - `cargo test -p freehand-ui-protocol worker_control -- --nocapture` -> 3 passed.
  - `cargo test -p freehand-runtime worker_control -- --nocapture` -> 2 passed.
  - `cargo test -p freehand-cli worker_control -- --nocapture` -> 2 passed.
  - `cargo test -p freehand-cli -- --nocapture` -> 24 passed.
  - `cargo test -p freehand-task -- --nocapture` -> 41 passed.
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 53 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 84 passed.
  - `cargo fmt --check` -> passed.
  - `cargo clippy --workspace --all-targets -- -D warnings` -> passed.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `git diff --check` -> ok.
- S-profile online proof already completed:
  - `scripts/install-launchd.sh restartS` restarted `com.freehand.daemonS` on `127.0.0.1:4042`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - `freehand-cliS worker-control-foundation-sample --url ws://127.0.0.1:4042/adp` -> `worker_control_foundation_sample_ok`.
  - Created ids: session `cli-worker-control-1783572587648756000`, task `task-cli-worker-control-FHPHASE2C1783572587648779000`, execution `exec-cli-worker-control-FHPHASE2C1783572587648779000`, agent `worker-cli-worker-control-FHPHASE2C1783572587648779000`, cancel control `wctl-cli-worker-control-cancel-FHPHASE2C1783572587648779000`.
  - Online sample evidence: `status=cancelled`, `control_events=8`, event statuses `query_status:observed,ask_at_safe_point:queued,add_constraint:queued,request_checkpoint:queued,request_submission_now:queued,pause:applied,resume:applied,cancel:applied`.
  - Task events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskExecutionRecorded,TaskPaused,TaskResumed,TaskHeartbeat,TaskCancelled`.
  - After `scripts/install-launchd.sh restartS`, verify mode against the same task/execution/agent/control ids returned `worker_control_foundation_verify_ok` with `status=cancelled`, `control_events=8`, and the same task/control event truth.
- durable rule:
  - Worker-control stateful consequences must persist `applied` control events only after the Task Center consequence succeeds; otherwise the ledger can falsely claim an action happened.

# 2026-07-09 context distribution closeout

- scope:
  - Closed P0 context/task-space distribution check for `contracts.core`, `reason.context-planner`, and `provider.reason-live-bridge`.
  - Added first-class `TaskContract` / `TaskSpaceSnapshot` segment kinds.
  - Runtime original operator task now enters as `TaskContract`, and every live round starts from completion contract + control status contract + runtime tool guidance + original task contract.
- local validation:
  - `cargo test -p freehand-contracts -- --nocapture` -> 10 passed.
  - `cargo test -p freehand-blocks -- --nocapture` -> 43 passed.
  - `cargo test -p freehand-runtime live_bridge_admits_long -- --nocapture` -> 2 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 90 passed.
  - `cargo test -p freehand-reason -- --nocapture` -> 60 passed.
  - `cargo test -p freehand-testkit -- --nocapture` -> 6 passed.
  - `cargo test -p freehand-cli -- --nocapture` -> 24 passed.
  - `cargo build --workspace` -> passed.
  - `cargo fmt --check` -> passed.
  - `cargo clippy --workspace --all-targets -- -D warnings` -> passed.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `git diff --check` -> ok.
- S-profile online validation:
  - `scripts/install-launchd.sh restartS` refreshed `freehand-cliS/freehand-daemonS` and restarted `com.freehand.daemonS` on `127.0.0.1:4042`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
  - success sample closed `cli-adp-sample-success-1783594455608254000`, `runtime-turn-197`, `rounds=1`, `tool_executions=0`.
  - failure sample after current restart closed `cli-adp-sample-failure-1783595301535093000`, terminal `runtime-turn-201-r3`, `rounds=3`, `tool_executions=2`, `failed_tools=1`.
- context distribution evidence from `~/.freehand/ledgers/reason/master/cli-adp-sample-failure-1783595301535093000.jsonl`:
  - `runtime-turn-201`: stable/session-stable prefix = `runtime-tool-guidance`, `completion-contract`, `control-status-contract`, `original-task`; volatile tail = `runtime-turn-201-user`.
  - `runtime-turn-201-r2`: same stable/session-stable prefix; volatile tail = `previous-visible-output`, `runtime-turn-201-r2-user`.
  - `runtime-turn-201-r3`: same stable/session-stable prefix; volatile tail = `previous-visible-output`, `runtime-turn-201-r3-user`.
  - stable prefix hash stayed `b25c8265c341fff3` across all three rounds; stable segment count stayed `4`; tool schema hash stayed `fe8c952141685333`.
  - token distribution stayed bounded: stable/tool guidance 394, completion 167, control status 157, original task 60; volatile previous-visible-output was 43 then 149 tokens.
- schema-mismatch sample audit:
  - Old prompt allowed tool contamination; online run `cli-adp-sample-schema-mismatch-1783594499298378000` called `bash` and timed out with ledger stuck at `ToolPending`.
  - Updated sample prompt to explicit no-tool and added CLI evidence gate `SchemaMismatch => rounds>=2 && schema_retries>=1 && tool_executions==0`.
  - Mock CLI test now asserts `tool_executions=0`.
  - Online no-tool run still returned valid schema in one round (`rounds=1`, `schema_retries=0`), so natural-prompt schema mismatch is not deterministic with the current model and must not be counted as schema-polishing online proof.
  - Future schema-polishing online proof needs a provider fixture or injected first invalid response, not prompt-only steering.
- MemoryPalace:
  - First corpus attempt copied source files and was rejected because test code contained `sk-inline` / token-like fixture strings.
  - Safe corpus `/Volumes/extension/code/memory/freehand-context-distribution-corpus-safe-1783595301535093000` copied only memory/docs/skill files.
  - Refined secret scan `trojan://|Secret(Id|Key)=|AKID|Bearer ...|(^|[^A-Za-z])sk-...|-----BEGIN` returned zero hits.
  - `mempalace mine ... --wing freehand --agent codex` processed 19 files.
  - Search for `cli-adp-sample-failure-1783595301535093000 stable prefix hash b25c8265c341fff3` returned `note.md` rank 1.
# 2026-07-09 production master/worker loop gap reconciliation

- marker:
  - `production-master-worker-loop-gap-reconcile-20260709`
- trigger:
  - Continued after `master-worker-autonomy-online-closeout-1783599325364293000`.
  - Read current `CACHE.md`, `MEMORY.md`, `note.md`, `freehand-dev` skill, feature map, function maps, and test designs.
  - MemoryPalace search for current production-loop gap returned no direct result, so repo docs were used as owner truth after source-only search.
- finding:
  - `docs/architecture/architecture-gaps.md` Gap 4 was stale. It still claimed Phase 2B EventInbox/master poll, Phase 2C worker_control, and Phase 2D UI projection were missing.
  - Current memory/source truth says Phase 2B/2C/2D are implemented and verified; `master-worker-autonomy-sample` also closed fixture-driven `SubmitUserInput`-only task-tool autonomy.
- updated:
  - `docs/architecture/architecture-gaps.md`
  - `docs/function-maps/task.orchestration.md`
  - `docs/testing/task.orchestration.md`
  - `docs/function-maps/app.cli-runtime-smoke.md`
  - `docs/testing/app.cli-runtime-smoke.md`
  - `docs/mainline-calls/app.cli-runtime-smoke.json`
  - `docs/mainline-calls/task.orchestration.json`
  - generated wiki for both mainline manifests
  - `docs/goals/multi-task-foundation-phase2-gap-plan.md` was rewritten from a stale active Phase2 plan into current Phase2 closeout plus production-loop gap plan.
- current gap now documented:
  - production non-smoke master/worker orchestration loop
  - daemon-owned scheduler/worker runner activation
  - configured worker resource acquisition/release beyond CLI samples
  - real worker queue claiming without scripted CLI mutation
  - non-fixture real-provider behavioral eval
  - Android true-device Phase 2 projection proof only if mobile dashboard changes are claimed
- validation:
  - targeted stale-pending phrase searches returned no hits in source docs/manifests.
  - `jq empty docs/mainline-calls/app.cli-runtime-smoke.json docs/mainline-calls/task.orchestration.json` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.

# 2026-07-09 formal real-provider E2E standard

- marker:
  - `formal-real-provider-e2e-standard-20260709`
- trigger:
  - Jason clarified that multi-agent production capability must be judged by
    online E2E real tasks, not only fixture or toy prompts.
- updated:
  - `docs/goals/multi-task-foundation-phase2-gap-plan.md`
  - `docs/architecture/architecture-gaps.md`
  - `docs/function-maps/app.cli-runtime-smoke.md`
  - `docs/testing/app.cli-runtime-smoke.md`
  - `docs/mainline-calls/app.cli-runtime-smoke.json`
  - `CACHE.md`
  - `MEMORY.md`
- standard:
  - formal E2E prompt asks master agent to research a current AI/semiconductor/
    international-technology-policy news item from the last 72 hours and write a
    markdown briefing document.
  - pass requires real S-profile daemon/provider, normal user-input path,
    current-source lookup or explicit missing-capability failure, owner truth
    from task/agent/event/history projections, output artifact, and same-id
    restart proof when workers are created.

# 2026-07-09 formal WebUI E2E worker-dispatch audit

- marker:
  - `formal-webui-e2e-worker-dispatch-audit-20260709`
- user-facing test:
  - Opened real WebUI in Chrome at `http://127.0.0.1:4042/`.
  - Selected live session `formal-e2e-news-1783606100493`.
  - Sent second-turn WebUI input asking to continue existing `task-1783606140` without creating duplicate task.
  - Screenshot evidence saved: `artifacts/webui-online/formal-e2e-webui-20260709/01-webui-second-turn-submitted.png`.
- task truth:
  - `freehand-cliS adp-task-query --url ws://127.0.0.1:4042/adp --history task-1783606140`
  - history events: `TaskCreated,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked,TaskProgressed`.
  - task list shows `task-1783606140:blocked:50`.
- ledger truth:
  - `TaskAssigned` payload assigns `worker-auto-1783582650`.
  - `TaskResumed` payload claims execution `exec-news-research-1783606140`.
  - `TaskBlocked` actor is `worker-auto-1783582650`, reason says current-source news research cannot proceed because runtime lacks `web_search`, `http_get`, or live-news fetch tool.
  - Later `TaskProgressed` actor is `master`, payload says master has `bash+curl` and will perform current-source lookup directly while worker stayed blocked.
- conclusion:
  - This run proves model/provider can create, assign, claim, and block a worker task through task truth.
  - It does not prove production worker autonomous execution.
  - The actual research after worker block is master-side self execution through `bash/curl`.
  - Production gap remains: daemon-owned worker runner plus worker tool capability/policy so master dispatch results in real worker execution and review/submit lifecycle, not master self-execution.

# 2026-07-09 Reasonix subagent constraint audit

- marker:
  - `reasonix-subagent-constraint-audit-20260709`
- reference source:
  - `/Users/fanzhang/code/Deepseek-reasonix/docs/GUIDE.md`
  - `/Users/fanzhang/code/Deepseek-reasonix/docs/SPEC.md`
  - `/Users/fanzhang/code/Deepseek-reasonix/internal/agent/task.go`
  - `/Users/fanzhang/code/Deepseek-reasonix/internal/agent/coordinator.go`
  - `/Users/fanzhang/code/Deepseek-reasonix/internal/skill/tools.go`
  - `/Users/fanzhang/code/Deepseek-reasonix/internal/skill/builtins.go`
- finding:
  - Reasonix does not constrain subagent behavior only by prompt text.
  - It constrains behavior through isolated child sessions, role/tool-registry filtering, top-level delegation tools, explicit meta-tool exclusion, profile selection, transcript ownership, and final-answer-only return into the parent context.
  - Planner mode is a separate model/session with read-only research tools only; executor receives a structured handoff and keeps writer/workflow tools.
  - `task` subagents receive a self-contained prompt, filtered tools, optional step/model/effort bounds, optional background execution, and persisted transcript refs for continue/fork.
  - Built-in subagent skills (`explore`, `research`, `review`, `security_review`) are exposed as natural top-level tools and are marked non-read-only to prevent parallel write races even when the skill itself is read-heavy.
  - Subagents exclude recursive/meta tools (`task`, `run_skill`, `read_skill`, `install_skill`, `explore`, `research`, `review`, `security_review`) by default, so delegation stays one layer deep.
- Freehand implication:
  - The production master/worker gap should not be solved by adding only prompt wording that says "use worker".
  - Required locks are: model-visible delegation affordance, child session/thread isolation, fork/continue ownership, role-specific config and instruction layering, tool/capability filtering for the spawned child, permission/hook/runtime gates at execution time, and typed parent-child communication.
  - It is wrong to solve this by a separate "admission round" that hides execution tools until a routing decision. Reasonix and Codex keep delegation as ordinary model-visible tools, then enforce boundaries when the delegation tool executes and when child tools run.
  - Current WebUI E2E showed the missing lock: worker blocked on missing search capability, then master continued through `bash/curl`; this must become a visible policy decision such as capability provisioning, worker retry/reassignment, explicit blocked state, or typed takeover, not silent master self-execution.
- proposed Freehand direction:
  - Keep `task` / `spawn_agent` style delegation visible to the master alongside other tools; do not depend on hiding tools by phase.
  - Make the delegation tool create durable Task Center truth plus a child execution context with explicit parent, task, workspace, role, model/effort, inherited runtime policy, and allowed capabilities.
  - Enforce boundaries in the tool/runtime gate: child agents cannot inherit recursive/meta tools unless explicitly allowed; parent self-execution after worker blockage must be surfaced as an explicit typed takeover or policy decision.
  - Worker runners should receive only the capability subset needed for the task class; missing capability becomes a typed worker blocked fact returned to the parent/master.
  - UI should project the task lifecycle and worker state, not infer delegation from raw tool names.
  - Gates must include a red case proving worker delegation cannot silently degrade into untracked master self-execution, and a green case proving approved typed recovery/takeover is recorded and queryable.

# 2026-07-09 Codex subagent constraint correction

- marker:
  - `codex-reasonix-subagent-correction-20260709`
- correction:
  - The earlier idea that Freehand should force a first "routing/admission" provider round with only routing tools is not aligned with Reasonix or Codex.
  - Reasonix exposes `task` as a normal tool. The tool execution creates an isolated subagent session, filters child tools, excludes recursive/meta tools, inherits permission gates, and returns only the final answer to the parent.
  - Codex exposes `spawn_agent` as a normal model-visible tool. The handler builds a child config from the parent turn, applies runtime-owned fields such as model, reasoning, approval policy, sandbox, and cwd, records `SessionSource::SubAgent(ThreadSpawn)`, sends typed inter-agent communication, and notifies the parent on terminal child turn.
  - Both systems rely on tool affordance + execution-time gates + child session/thread isolation + typed communication, not on removing direct tools from the model's first turn.
- Freehand corrected target:
  - Add delegation affordance and stronger runtime ownership gates without inventing a separate routing phase.
  - The model may choose direct execution or delegation; if it delegates, framework truth must preserve child ownership, status, capability boundary, and parent-visible result.
  - If direct execution is undesired for a class of tasks, constrain it through policy, role config, user instruction, and execution-time gates that return explicit blocked results to the model, not through unstable prompt-phase assumptions.

# 2026-07-10 master runtime-home workspace boundary closeout

- marker:
  - `master-runtime-home-boundary-closeout-20260710`
- implementation truth:
  - `freehand-tools` classifies built-ins as framework/workspace/shell/network and exposes a Master schema that excludes shell tools.
  - Master workspace tool execution canonicalizes and locks to `runtime_home`; external session CWD or absolute target returns a paired failed tool result with Worker delegation guidance.
  - `task` is framework-scoped and remains executable so the model can create/reuse a worker, create an external `target_cwd` task, and assign it.
  - Live Master session default CWD, checkpoint root, and launchd default workdir are all `~/.freehand`.
  - `RuntimeCommandDispatcher::from_selected_agent_inner` rejects non-Master selected agents, so the current Master-only live bridge does not accidentally impose this policy on a future Worker runner.
- local verification:
  - Existing running `make ci` completed successfully after build, fmt, clippy, workspace tests/doc-tests, mainline check, and gate check.
  - Targeted owner tests passed inside that run: `freehand-tools` 30, `freehand-runtime` 90, `freehand-daemon` 17, `xtask` 23.
- online S-profile verification:
  - `scripts/install-launchd.sh restartS` refreshed and restarted `com.freehand.daemonS`.
  - `~/.freehand/daemonS.env` reports `FREEHAND_DAEMON_WORKDIR="/Users/fanzhang/.freehand"`.
  - health returned `ok`; `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` returned `adp_smoke_ok`.
  - Created session `master-boundary-live-1783650252` with CWD `/Volumes/extension/code/freehand`, then submitted a real request asking Master to read `AGENTS.md`.
  - Master direct read was rejected with `allowed_root=/Users/fanzhang/.freehand` and requested target `/Volumes/extension/code/freehand`; the persisted model context confirms no file content was returned.
  - The same live turn used `task` to create/assign/claim `task-1783650293` under the external target CWD and ended successfully after 11 rounds.
  - Task history is `TaskCreated,TaskAssigned,TaskResumed,TaskHeartbeat`; this is boundary/delegation proof only. Production daemon-owned Worker execution and completion remain the existing separate gap.

# 2026-07-10 production Worker online closeout

- marker:
  - `production-worker-online-closeout-1783657707`
- first online failure:
  - S-profile Master was healthy on `127.0.0.1:4042`.
  - Configured Worker started in Slave mode and claimed the first real task, then panicked:
    - `Cannot drop a runtime in a context where blocking is not allowed`
    - panic originated while the synchronous Worker/provider path was running directly inside daemon's Tokio async runtime.
  - first task `task-1783657410` ended `interrupted`; this was treated as red evidence, not success.
- root fix:
  - `run_worker_mode` is async and enters `run_blocking_worker_service`.
  - `run_blocking_worker_service` owns one `tokio::task::spawn_blocking` boundary around `ProductionWorkerRunner::run`.
  - positive daemon test creates and drops a nested Tokio runtime inside that blocking service.
  - negative daemon test proves a blocking-task panic returns explicit `worker runner task failed`.
  - Worker runner source was split:
    - `crates/freehand-runtime/src/worker_runner.rs` = 460 lines
    - `crates/freehand-runtime/src/worker_runner/heartbeat.rs`
    - `crates/freehand-runtime/src/worker_runner/tests.rs`
- real-provider S-profile proof:
  - Master command ingress session: `production-worker-e2e-1783657707`.
  - external target cwd: `/tmp/freehand-worker-e2e-1783657707`.
  - task: `task-1783657707`.
  - Worker: `worker`.
  - execution: `exec-worker-worker-1783657761743691000-81`.
  - Worker turn: `worker-turn-exec-worker-worker-1783657761743691000-81-r4`.
  - deliverable: `/tmp/freehand-worker-e2e-1783657707/result.md`.
  - result file contains the source fields, calculation `13 + 29 = 42`, literal `sum=42`, and verification evidence after Worker re-read.
  - ordered TaskHistory:
    - `TaskCreated`
    - `TaskAssigned`
    - `TaskResumed`
    - `TaskHeartbeat`
    - periodic `TaskHeartbeat`
    - `TaskReviewSubmitted`
    - `TaskReviewApproved`
    - `TaskClosed`
  - after explicit Worker stop/restart, the same task/execution/agent history remained queryable.
- validation:
  - `cargo test -p freehand-runtime production_worker_runner -- --nocapture` -> 5 passed.
  - `cargo test -p freehand-runtime -- --nocapture` -> 97 passed.
  - `cargo test -p freehand-tools -- --nocapture` -> 31 passed.
  - `cargo test -p freehand-daemon worker_service -- --nocapture` -> 2 passed.
  - `cargo test -p freehand-daemon -- --nocapture` -> 19 passed.
  - `make ci` -> passed after `cargo run -p xtask -- mainlines generate`.
- remaining proof gap:
  - browser runtime discovery returned an empty browser list, so no real WebUI page operation or screenshot was captured.
  - the completed proof used the real daemon HTTP command ingress and owner truth; it must not be reported as browser-visible WebUI verification.

# 2026-07-10 current-topology lifecycle closeout start

- marker:
  - `current-topology-lifecycle-closeout-20260710`
- scope lock:
  - close every task lifecycle branch for the configured one-Master/one-Worker topology before adding worker concurrency, multiple BigTasks, or UI work
  - success must reach Master review and close or rejection
  - rejection must produce a new Worker execution with persisted requirements
  - Worker crash must persist `TaskInterrupted` and requeue with a new execution
  - terminal execution failure must stay `TaskBlocked` until an explicit Master decision
  - restart must resume loops from Task Center truth without requiring new user input
- first confirmed gap:
  - production Worker claims only `Assigned`
  - `Interrupted`, `Rejected`, and `Blocked` persist correctly but the current daemon loops do not close their next transitions autonomously

# 2026-07-10 stale runtime terminal overwrite online validation

- marker:
  - `stale-runtime-terminal-overwrite-online-1783682850768`
- scope:
  - S-profile only: daemon/worker restarted through `scripts/install-launchd.sh restartS` and `scripts/install-launchd.sh restartWorkerS`
  - release port 4041 was not touched; online validation used `127.0.0.1:4042`
- health:
  - `curl -4fsS http://127.0.0.1:4042/health` returned `ok`
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` returned `adp_smoke_ok`
  - `launchctl print gui/$(id -u)/com.freehand.workerS` showed `state = running`
  - `~/.freehand/state/agents/worker.json` showed worker `available` after the cancel test
- local verification after online proof:
  - `git diff --check` passed
  - `cargo test -p freehand-task -- --nocapture` passed: 46 tests
  - `cargo test -p freehand-runtime production_worker_runner -- --nocapture` passed: 9 tests
- online task:
  - task: `task-cancel-running-1783682850768`
  - execution: `exec-worker-worker-1783682851141101000-1177`
  - target cwd: `/tmp/cancel-running-1783682850768`
  - final event order:
    - `TaskCreated`
    - `TaskAssigned`
    - `TaskResumed`
    - `TaskHeartbeat`
    - `TaskCancelled`
  - after `TaskCancelled`, there were no later `TaskHeartbeat`, `TaskReviewSubmitted`, `TaskBlocked`, `TaskInterrupted`, `TaskAssigned`, or `TaskResumed`
- decisive log evidence:
  - `~/.freehand/logs/workerS.stderr.log` recorded `worker runner stopped: worker execution failed and blocked fact could not be persisted: invalid task transition from `Cancelled` using `TaskBlocked``
  - this proves stale Worker-side result/error reporting was rejected against persisted terminal task truth instead of overwriting cancellation
- remaining lifecycle gaps:
  - this validates terminal stale-write protection only
  - full one-Master/one-Worker lifecycle closure still needs online proof for success close, reject retry, crash/interrupted recovery, blocked decision, and restart recovery under the current topology

# 2026-07-10 one-master-one-worker manual lifecycle online proof

- marker:
  - `manual-lifecycle-online-proof-1783684556`
- scope:
  - S-profile only, endpoint `127.0.0.1:4042`
  - user-facing goal: determine what can be manually started and completed today
  - release 4041 was not touched
- service state:
  - health returned `ok`
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` returned `adp_smoke_ok`
  - final `~/.freehand/state/agents/worker.json` showed worker `available`
  - `launchctl print gui/$(id -u)/com.freehand.workerS` showed `state = running`, PID `87164`, and previous `last terminating signal = Terminated: 15` from the interruption test
- happy path proof:
  - task: `task-manual-happy-1783683798565`
  - target cwd: `/tmp/manual-happy-1783683798565`
  - result: `/tmp/manual-happy-1783683798565/result.md`
  - final event order: `TaskCreated,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
  - result content contained `freehand_manual_happy_ok` and evidence that `input.txt` token was read
- reject retry proof:
  - direct bad first review task: `task-manual-reject-direct-1783684045392`
  - target cwd: `/tmp/manual-reject-direct-1783684045392`
  - first execution: `exec-first-manual-reject-direct-1783684045392`
  - retry execution: `exec-worker-worker-1783684063797699000-1019`
  - final result content: `FH-MANUAL-REJECT-DIRECT-1783684045392`
  - final event order: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewRejected,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
  - rejected sample attempted first with natural prompt only (`task-manual-reject-1783683914712`) but that was invalid reject evidence because the model corrected within one execution and no `TaskReviewRejected` event appeared
- blocked decision proof:
  - task: `task-manual-blocked-1783684186916`
  - missing target cwd: `/tmp/manual-blocked-1783684186916-missing-cwd`
  - final event order: `TaskCreated,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked,TaskProgressed`
  - `TaskProgressed` payload included `blocked_decision:` explaining the missing cwd and required external action; no silent infinite retry was observed
- interrupted recovery proof:
  - task: `task-manual-interrupt-1783684433320`
  - target cwd: `/tmp/manual-interrupt-1783684433320`
  - worker service was interrupted with service-scoped `launchctl kill TERM gui/$(id -u)/com.freehand.workerS`; no broad kill was used
  - initial execution: `exec-worker-worker-1783684433462971000-1337`
  - recovered execution: `exec-worker-worker-1783684523540461000-22`
  - final event order: `TaskCreated,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskInterrupted,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
  - `TaskInterrupted` reason was `missing_or_expired_lease`
- restart review observation:
  - task: `task-restart-review-1783684249797`
  - after `scripts/install-launchd.sh restartS`, TaskHistory was still queryable and ended `TaskClosed`
  - this is not a strict pending-review recovery proof because the review may have been processed before restart completed
- updated manual-use conclusion:
  - current one-Master/one-Worker task lifecycle can complete happy path, reject retry after a persisted bad review, blocked decision note, and worker interrupted recovery online
  - remaining important gap is the user operation surface: WebUI must expose task creation/status/control/review truth cleanly enough for Jason to run these flows manually without raw ADP scripts
  - strict restart recovery still needs a deterministic test where pending review/blocked/rejected truth is created while Master is stopped or before the lifecycle runner can consume it

# 2026-07-10 strict master restart recovery proof

- marker:
  - `strict-master-restart-review-proof-1783686325`
- implementation:
  - added `freehand-cli task-restart-seed-review`
  - command loads the selected Master config and writes Task Center truth through `TaskRuntime` API only
  - command creates task, assigns configured Worker, claims an execution, and submits review-ready truth
  - command does not approve/close, does not use ADP, and does not write task JSON directly
- local verification:
  - `cargo test -p freehand-cli -- --nocapture` passed: 26 tests
- online S-profile proof:
  - installed updated S-profile with `scripts/install-launchd.sh restartS`
  - stopped daemonS with service-scoped `launchctl bootout gui/$(id -u)/com.freehand.daemonS`
  - verified `curl -4fsS http://127.0.0.1:4042/health` failed while daemon was offline
  - first offline seed attempt failed explicitly because `FREEHAND_PAIR_TOKEN_SHARED` was not loaded; no task was written
  - loaded `~/.freehand/daemonS.env` and seeded:
    - task: `task-strict-restart-1783686325`
    - execution: `exec-strict-restart-1783686325`
    - target cwd: `/tmp/strict-restart-1783686325`
    - seeded event order: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted`
  - restarted daemonS with `scripts/install-launchd.sh restartS`
  - health returned `ok`
  - ADP TaskHistory for `task-strict-restart-1783686325` reached `TaskReviewApproved,TaskClosed`
  - final event order: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
- conclusion:
  - strict pending-review restart recovery is now proven: review truth created while Master daemon was offline was consumed by the restarted Master lifecycle runner

# 2026-07-10 strict rejected and blocked restart recovery proof

- marker:
  - `strict-rejected-blocked-restart-proof-1783689029`
- implementation:
  - extended `freehand-cli task-restart-seed-review` into state-specific seed commands:
    - `task-restart-seed-review`
    - `task-restart-seed-rejected`
    - `task-restart-seed-blocked`
  - all variants still write through `TaskRuntime` owner API only
  - rejected seed writes `TaskReviewSubmitted` then `TaskReviewRejected`
  - blocked seed writes `TaskBlocked` through `apply_execution_fact`
  - no variant approves, closes, runs ADP, or writes task JSON directly
- local verification:
  - `cargo fmt --check` passed
  - `cargo test -p freehand-cli -- --nocapture` passed: 26 tests
- strict rejected online proof:
  - stopped daemonS and workerS with service-scoped `launchctl bootout`
  - seeded while both services were offline:
    - task: `task-strict-rejected-1783688577`
    - first execution: `exec-strict-rejected-1783688577`
    - target cwd: `/tmp/strict-rejected-1783688577`
    - seed events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewRejected`
  - restarted daemonS and workerS through `scripts/install-launchd.sh restartS` and `scripts/install-launchd.sh restartWorkerS`
  - final TaskHistory reached:
    - `TaskAssigned`
    - `TaskResumed`
    - `TaskReviewSubmitted`
    - `TaskReviewApproved`
    - `TaskClosed`
  - recovered execution: `exec-worker-worker-1783688747254581000-0`
  - final event order: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewRejected,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
- strict blocked online proof:
  - stopped daemonS with service-scoped `launchctl bootout`
  - seeded while daemonS was offline:
    - task: `task-strict-blocked-1783689029`
    - execution: `exec-strict-blocked-1783689029`
    - target cwd: `/tmp/strict-blocked-1783689029`
    - seed events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked`
  - restarted daemonS with `scripts/install-launchd.sh restartS`
  - final TaskHistory reached `TaskProgressed`
  - `TaskProgressed` payload contained `blocked_decision: external operator decision required...`
  - final event order: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked,TaskProgressed`
- final service state:
  - `curl -4fsS http://127.0.0.1:4042/health` returned `ok`
  - `~/.freehand/state/agents/worker.json` showed worker `available`
  - `launchctl print gui/$(id -u)/com.freehand.workerS` showed `state = running`
- caution:
  - during restore, daemonS and workerS install commands were started in parallel and waited on Cargo locks; future same-owner/service install validation should be sequential unless explicitly independent

# 2026-07-10 background Master lifecycle runner proof

- marker:
  - `background-master-runner-review-proof-1781783690592`
- current P0 target:
  - prove the Master daemon can consume Task Center lifecycle events in the background without a user sending another chat message
  - this validates the manual user path after a Worker submits review: the framework advances the task lifecycle by event polling
- implementation under review:
  - `ProductionMasterRunner` polls TaskEventInbox with a persisted cursor under `~/.freehand/state/master-loop/<agent>.json`
  - retryable lifecycle failures keep the same event cursor and increment persisted attempt state
  - Master daemon starts the WebUI/ADP server and the Master lifecycle runner together; the runner executes through a blocking boundary
  - live bridge supports a target task decision boundary so a lifecycle turn closes as soon as the expected task mutation is persisted
- local verification before online:
  - `cargo test -p freehand-runtime production_master -- --nocapture` passed: 11 tests
  - `cargo test -p freehand-runtime production_worker -- --nocapture` passed: 9 tests
  - `cargo test -p freehand-daemon worker_mode -- --nocapture` passed: 1 test
  - `bash -n scripts/install-launchd.sh && bash -n scripts/uninstall-launchd.sh` passed
- online proof:
  - restarted S-profile services after current workspace build:
    - `scripts/install-launchd.sh restartS`
    - `scripts/install-launchd.sh restartWorkerS`
  - stopped daemonS through service-scoped `launchctl bootout`
  - sourced `~/.freehand/daemonS.env` and seeded review truth while daemonS was offline:
    - task: `task-master-bg-review-1781783690592`
    - execution: `exec-master-bg-review-1781783690592`
    - seed events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted`
  - restarted daemonS through `scripts/install-launchd.sh restartS`
  - final TaskHistory from ADP:
    - `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
- observed mistakes:
  - first seed attempt failed because offline CLI was run without sourcing daemon env; no task was written
  - one restart attempt failed after sourcing daemon env because the env PATH did not include cargo; recovery used a clean shell and the same seeded task
  - a polling script used zsh readonly variable name `history`; rerun used `hist_out` and succeeded

# 2026-07-10 background Master/Worker non-happy-path online proof

- marker:
  - `background-master-worker-nonhappy-proof-1781783694885`
- implementation:
  - added `freehand-cli task-restart-seed-running`
  - added optional `--ttl-seconds` to the restart seed harness
  - running seed writes through `TaskRuntime` only: create, assign, claim, heartbeat; it does not write review/blocked/terminal truth
- local validation:
  - `cargo fmt --check` passed
  - `cargo check -p freehand-cli` passed
  - `cargo test -p freehand-cli --test config_startup -- --nocapture` passed: 26 tests
  - earlier full `cargo test -p freehand-cli -- --nocapture` PTY session returned later and passed: 26 tests
  - `git diff --check` passed before online proof
- rejected retry online proof:
  - stopped daemonS and workerS through service-scoped `launchctl bootout`
  - seeded rejected truth while both services were offline:
    - task: `task-bg-rejected-1781783693503`
    - first execution: `exec-bg-rejected-1781783693503`
    - seed events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewRejected`
  - restarted daemonS then workerS
  - final ADP TaskHistory:
    - `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewRejected,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
- blocked decision online proof:
  - stopped daemonS through service-scoped `launchctl bootout`
  - seeded blocked truth while daemonS was offline:
    - task: `task-bg-blocked-1781783693672`
    - execution: `exec-bg-blocked-1781783693672`
    - seed events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked`
  - restarted daemonS
  - final ADP TaskHistory:
    - `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked,TaskProgressed`
- worker crash/restart online proof:
  - restarted current S-profile binaries
  - stopped daemonS and workerS through service-scoped `launchctl bootout`
  - seeded lease-backed running truth with `--ttl-seconds 1`:
    - task: `task-bg-crash-1781783694885`
    - expired execution: `exec-bg-crash-1781783694885`
    - seed events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat`
  - waited for lease expiry, restarted daemonS then workerS
  - first recovery evidence:
    - `TaskInterrupted,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted`
  - final ADP TaskHistory:
    - `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskInterrupted,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
# 2026-07-11 normal test readiness verification

- marker:
  - `normal-test-readiness-webui-proof-1783734855627`
- headless normal gate:
  - reran `scripts/verify-normal-master-worker-e2e.sh`
  - final output: `normal_master_worker_e2e_ok url=ws://127.0.0.1:4042/adp`
  - autonomy fixture output: `master_worker_autonomy_online_ok mock_attempts=24`
  - production rejected retry task:
    - `task-normal-rejected-1781783734686`
    - history: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewRejected,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
  - production blocked decision task:
    - `task-normal-blocked-1781783734723`
    - history: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked,TaskProgressed`
  - production worker crash recovery task:
    - `task-normal-crash-1781783734746`
    - history: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskInterrupted,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
- WebUI online proof:
  - command: `make verify-webui-online`
  - result: exit code 0
  - artifact: `artifacts/webui-online/20260711-verify-4042-1783734855627/summary.json`
  - session: `webui-session-20260711015428-f13dd4b9`
  - screenshots include:
    - `01-after-new-conversation.png`
    - `06-second-running.png`
    - `07-second-terminal.png`
    - `08-after-refresh.png`
    - `26-mobile-focused-composer.png`
    - `27-phase2-projection.png`
  - truth snapshot:
    - `27-phase2-truth.json`
  - key checks true:
    - `removedExistingSessions`
    - `firstSubmitComposerCleared`
    - `secondProgressObserved`
    - `terminal2NoLive`
    - `phase2TaskBoardMatchesService`
    - `phase2AgentBoardMatchesService`
    - `phase2EventInboxMatchesService`
    - `phase2TaskHistoryMatchesService`
    - `phase2WorkerControlMatchesService`
    - `phase2ProjectionVisible`
    - `phase2NoRawInternalChrome`
    - `mobileFocusedComposerCompact`
    - `mobileFocusedNoLeftEdgeIndicators`
  - post-proof S-profile config:
    - `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp`
    - `provider=minimax`
    - `base_url_host=api.minimaxi.com`
    - `default_model=MiniMax-M3`
    - `auth_source=inline`
- conclusion:
  - current S-profile is ready for Jason's normal WebUI testing on `http://127.0.0.1:4042`
  - remaining non-dev promotion gaps are release `4041` and Android true-device verification if the target surface is release/phone

# 2026-07-11 cwd read/write boundary contract correction

- marker:
  - `cwd-read-write-boundary-contract-closeout-1783751556`
- corrected contract:
  - agent cwd/workspace root is `A`
  - target path `B` is not automatically a new cwd
  - read/query tools may inspect readable external absolute paths or parent paths outside `A`
  - write/edit/delete outside `A` must return a paired failed tool result to the model
  - write-boundary feedback must say the write target is outside current cwd and must be handled by confirming/selecting the correct target workspace cwd, then delegating through task/worker
  - boundary feedback must not say or imply that the path is missing
- implementation:
  - `freehand-tools` added read-path resolution for `read_file`, `grep`, and `ls`; file-mutation tools still use locked write resolution
  - Worker provider tool surface excludes unrestricted `bash`; injected Worker shell calls return paired failed tool results instead of executing
  - runtime Master path policy allows external read/query and denies external writes with `Write boundary denied`
  - Master/Worker prompts now describe read/write split, target cwd semantics, symlink checks, and worker shell unavailability
  - function maps, test designs, design docs, mainline manifests, generated wiki, gates, and local skill were updated
- validation:
  - focused local runtime test: `cargo test -p freehand-runtime live_bridge_ -- --nocapture` -> 40 passed
  - prior local suite from handoff: `cargo test -p freehand-tools -- --nocapture`, `cargo test -p freehand-runtime -- --nocapture`, `cargo test -p xtask -- --nocapture`, `cargo fmt --check`, targeted clippy, mainlines generate/check, gates check, and `git diff --check` passed before the final test-path hardening
  - online S-profile fixture used real external path `/tmp/freehand-path-policy-online-1783751533-96433`, not `~/.freehand/tmp`
  - online session `path-policy-online-1783751556` returned `reason_live_turn_completed rounds=3 schema_rejections=0 tool_executions=2`
  - reason ledger `~/.freehand/ledgers/reason/master/path-policy-online-1783751556.jsonl` contains `toolu_path_read` success with `external-readable`, then `toolu_path_write` failed with `Write boundary denied`
  - external file `/tmp/freehand-path-policy-online-1783751533-96433/external-write/target.txt` remained `original`
  - no checkpoint ledger existed for the session, so no external write `Applied` checkpoint was produced
  - mock provider log showed third provider request had `hasWriteBoundary=true`, proving the failed write result was paired back to the model
  - post-proof config restored: `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` returned `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`
- invalidated evidence:
  - earlier online write attempt `path-policy-write-1783751078` is not valid external-write proof because its fixture path lived under `~/.freehand/tmp`, inside the master runtime home

# 2026-07-11 target_cwd preflight error classification

- marker:
  - `target-cwd-preflight-classified-error-closeout-1783745289`
- issue:
  - Master/Worker path handling collapsed different meanings into path-not-found style feedback:
    - Master own cwd/runtime-home boundary
    - Worker task `target_cwd` workspace root
    - not-yet-created deliverable/output directory
  - This misled the model into thinking an existing user repo was missing.
- implementation:
  - `ProductionWorkerRunner` now classifies worker workspace preflight errors before model execution:
    - empty target
    - missing parent path
    - missing workspace under an existing parent, likely output-directory misuse
    - permission denied
    - generic canonicalization failure
    - canonical path is not a directory
  - Master workspace-boundary tool results now say direct access is denied by Master scope/permission and is not evidence that the external path is missing.
  - Local skill now records that `target_cwd` is an existing workspace/repository root, not an output directory.
- online proof:
  - first online task `task-preflight-message-1783745178` still showed old wording because `com.freehand.workerS` was stale after `restartS`.
  - service-scoped restart used `launchctl kickstart -k gui/$(id -u)/com.freehand.workerS`.
  - second online task `task-preflight-message-1783745289` reached history:
    - `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked,TaskProgressed`
  - persisted blocked reason says `target_cwd` under existing parent does not exist, is not a repository-permission denial, and likely means `target_cwd` was used for a not-yet-created output directory or wrong workspace root.
- remaining design gap:
  - `TaskCreateRequest` still has only `target_cwd`; first-class `output_dir` / artifact destination contract is still needed before we can support "repo exists, create output elsewhere" without overloading `target_cwd`.

# 2026-07-10 normal master/worker E2E gate closeout

- marker:
  - `normal-master-worker-e2e-gate-1781783700473`
- implementation:
  - added `scripts/verify-normal-master-worker-e2e.sh`
  - gate restarts S-profile master/worker on fixed `127.0.0.1:4042`
  - gate runs SubmitUserInput-only `master-worker-autonomy` fixture, then production branch proofs for rejected retry, blocked decision, and worker crash recovery
  - autonomy fixture now uses configured Worker id `worker` instead of dynamic `worker-cli-master-autonomy-*`
  - removed fixture `create_agent`; provider attempt gate is now exactly 24 attempts
  - autonomy verification now treats TaskBoard/TaskHistory/transcript as task-scoped truth; shared Worker AgentLifecycle is queried for visibility but not used as per-scenario terminal truth because the configured Worker global state can be overwritten by concurrent/background tasks
  - rejected/crash production branches now create deterministic `instructions.txt` files in target cwd so Worker tasks have exact file/output criteria
- root causes fixed:
  - old autonomy fixture generated dynamic worker ids, but production runtime accepts only configured paired Worker `worker`
  - old CLI verification used shared Worker's global lifecycle state as if it were task-local; online verify showed lifecycle could be `closed` or `running` while the target task truth was correctly `blocked`
  - initial normal rejected/crash branches had vague/empty target workspaces, causing Worker heartbeat without review submission
- local verification:
  - `cargo fmt --check` passed
  - `cargo check -p freehand-cli` passed
  - `cargo test -p freehand-cli --test config_startup -- --nocapture` passed: 26 tests
  - `cargo test -p freehand-cli -- --nocapture` passed: 26 tests
  - `cargo run -p xtask -- mainlines generate` passed
  - `cargo run -p xtask -- mainlines check` passed
  - `cargo run -p xtask -- gates check` passed
  - `cargo clippy --workspace --all-targets -- -D warnings` passed
  - `git diff --check` passed
- online S-profile proof:
  - `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` restored to `provider=minimax`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`
  - `scripts/verify-normal-master-worker-e2e.sh` passed with final `normal_master_worker_e2e_ok url=ws://127.0.0.1:4042/adp`
  - autonomy fixture: `master_worker_autonomy_online_ok mock_attempts=24`
  - autonomy tasks:
    - success `task-cli-master-autonomy-success-FHAUTO1783700365778249000`: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskExecutionRecorded,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
    - execution-error `task-cli-master-autonomy-execution-error-FHAUTO1783700366668480000`: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskExecutionRecorded,TaskBlocked`
    - reject-retry `task-cli-master-autonomy-reject-retry-FHAUTO1783700367280302000`: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewRejected,TaskResumed,TaskHeartbeat,TaskExecutionRecovering,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
  - production rejected retry `task-normal-rejected-1781783700382`: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewRejected,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
  - production blocked decision `task-normal-blocked-1781783700459`: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked,TaskProgressed`
  - production worker crash recovery `task-normal-crash-1781783700473`: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskInterrupted,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`
# 2026-07-11 independent timer internal tool implementation

- user correction:
  - timer/wakeup is an independent standard internal tool, not task truth.
  - do not model waiting as `task(op="wait")`, task notes, or task lifecycle state.
- implementation:
  - `freehand-tools` now exposes `timer` as a framework tool for Master only; Worker schema excludes it.
  - timer supports `schedule`, `cancel`, and `list`, with relative delay, absolute unix timestamp, local-time interval/daily/weekly rules, strict 5-field local-time cron expressions, weekdays, skip weekends, max runs, reason, examples, and persisted wakeup prompt.
  - `freehand-runtime` now persists timer schedules under `~/.freehand/state/timers/<agent>.json` and ledger events under `~/.freehand/ledgers/timers/<agent>.jsonl`.
  - Master runner claims due timers before Task Center EventInbox processing, starts an internal Master turn with the persisted prompt, completes one-shot timers, reschedules recurring timers, and releases failed wakeups back to active retryable timer truth.
  - docs, function maps, test designs, mainline manifests, generated wiki, and local skill were updated to lock timer as independent framework tool truth.
- verification:
  - `cargo test -p freehand-tools -- --nocapture` passed 34 tests.
  - `cargo test -p freehand-runtime timer -- --nocapture` passed 7 focused timer tests.
  - `cargo test -p freehand-runtime production_master_runner_ -- --nocapture` passed 9 Master runner tests.
  - `cargo test -p freehand-runtime -- --nocapture` passed 133 tests.
  - `cargo clippy -p freehand-runtime -p freehand-tools --all-targets -- -D warnings` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
  - S-profile config remains minimax/MiniMax-M3 inline auth; fixture env grep has 0 matches.
  - `scripts/verify-timer-tool-online.sh` passed on S-profile `127.0.0.1:4042`: `timer_tool_online_ok`, timer `timer-online-proof-1783774257-95369`, `mock_attempts=3`, ADP receipt `reason_live_turn_completed rounds=2 tool_executions=1`, fixture second request `sawToolResult=true`, third request `sawTimerWakeup=true`, due verified `status=completed fired_count=1`, ledger had `TimerScheduled`, `TimerFired`, and `TimerCompleted`.
  - `FREEHAND_TIMER_VERIFY_MODE=restart-due scripts/verify-timer-tool-online.sh` passed on S-profile `127.0.0.1:4042`: timer `timer-online-proof-1783774667-24465` was first verified persisted as `status=active next_due_at=1783774692`, then service-scoped `restartS` ran before due; after restart, Master runner fired the overdue timer, fixture third request had `sawTimerWakeup=true`, state became `completed fired_count=1`, and ledger had `TimerFired`/`TimerCompleted`.
- remaining:
  - `output/` remains unrelated untracked content and was not touched.

# 2026-07-11 metadata-only global sessions and worker child rows

- marker:
  - `metadata-only-session-list-worker-child-closeout-1783780857107`
- user requirement:
  - global session list shows only persisted sessions.
  - user New task/New conversation sessions are persisted and top-level.
  - subagent/worker task sessions are temporary/internal and must not appear top-level.
  - worker/subagent sessions appear only indented under the owning master session.
  - tests should use fixed sessions instead of continuously creating random sessions.
- implementation:
  - `ui.protocol` `session_list_projection` now builds top-level active/archived lists from `session_metadata` only, not raw turn grouping.
  - `user_visible_session_id` filters `master-lifecycle-*`, `master-timer-*`, and `worker-task-*` from global session lists while keeping direct `QuerySessionTurns` queryable.
  - `UiTaskSnapshotProjection` now carries `parent_session_id`, projected from `TaskSnapshot.parent.session_id` in runtime.
  - WebUI session rail removed agent-group top-level rendering; it renders persisted sessions as top-level rows and TaskBoard-parented `worker-task-*` sessions as indented temporary child rows.
  - WebUI session truth gate allows selected worker child transcript only when TaskBoard parent truth links it to a persisted parent; child rows are excluded from select-all/rename/remove.
  - docs/function maps/testing/mainline JSON/wiki and local skill were synced.
- validation:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `cargo check -p freehand-runtime -p freehand-cli -p freehand-server` passed.
  - `cargo test -p freehand-ui-protocol session -- --nocapture` passed 11 focused tests before the new redline was added.
  - `cargo test -p freehand-ui-protocol session_list -- --nocapture` passed 3 focused tests, including metadata-only redline.
  - `cargo test -p freehand-server webui -- --nocapture` passed 3 tests.
  - `cargo test -p freehand-runtime runtime_dispatches_phase2a_master_worker_loop_into_task_truth -- --nocapture` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
  - S-profile `127.0.0.1:4042` health and `freehand-cliS adp-smoke` passed after `scripts/install-launchd.sh restartS`.
  - fixed online proof used `webui-session-fixed-subagent-proof` and `task-fixed-subagent-proof`; no random verification session was created.
  - ADP owner proof: fixed session present top-level; top-level `worker-task-*`, `master-timer-*`, and `master-lifecycle-*` arrays empty; task parent `webui-session-fixed-subagent-proof`.
  - Browser proof: `artifacts/webui-online/fixed-session-subagent-1783780857107/summary.json`, screenshot `fixed-session-subagent.png`; `pass=true`, `fixedChildExists=true`, `fixedChildKind=worker`, `workerTopLevelRows=[]`, `masterLifecycleVisible=false`, `masterTimerVisible=false`.
  - final S-profile config remained minimax/MiniMax-M3 inline auth; fixture env grep returned 0 matches.
- remaining:
  - fixed proof task/session remain as durable verification truth by design; do not physically delete without explicit destructive approval.
  - unrelated dirty changes from timer/submission work and untracked `output/` remain untouched.

# 2026-07-11 WebUI scroll/composer and internal lifecycle session closeout

- marker:
  - `webui-scroll-lifecycle-session-closeout-1783757223354`
- issues fixed:
  - WebUI live render updates forced the operator back to the bottom while reading older transcript content.
  - Fixed/sticky composer could overlap the latest transcript row because bottom padding used fixed estimates instead of the real composer height.
  - Master lifecycle decisions created event/attempt-scoped internal sessions, producing many `master-lifecycle-*` sessions and polluting user-facing session lists.
- implementation:
  - `apps/freehand-server/assets/webui.js` now tracks `state.userScrollLocked`; `renderMessages()` follows bottom only when already near bottom or an explicit local submit forces it.
  - ordinary render updates no longer call `scrollIntoView`; scroll movement is constrained to the conversation scroll host.
  - WebUI measures `.composer-card` and writes `--composer-clearance`; CSS uses that variable for `.message-list` and fixed mobile composer layouts.
  - `crates/freehand-ui-protocol` filters `master-lifecycle-*` from user-facing `QuerySessionList` / archived list projections while keeping direct `QuerySessionTurns` queryable for debug truth.
  - Master lifecycle reason session naming is now task-scoped `master-lifecycle-<task>`; event/attempt isolation remains in turn id and trace id.
  - `scripts/webui_verify_online.mjs` now captures `scrollProof` screenshots/JSON for upward-scroll preservation, bottom-pinned visibility, composer clearance, and user-facing session-list internal-id hiding.
- validation:
  - `node --check apps/freehand-server/assets/webui.js`
  - `node --check scripts/webui_verify_online.mjs`
  - `cargo test -p freehand-server -- --nocapture` -> 13 passed
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 54 passed
  - `cargo test -p freehand-runtime production_master -- --nocapture` -> 11 passed
  - `cargo fmt --check`
  - `git diff --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `scripts/install-launchd.sh restartS`
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`
  - `make verify-webui-online` -> passed; artifact `artifacts/webui-online/20260711-verify-4042-1783757223354/summary.json`
  - new online checks true: `scrollLockHasScrollableTranscript`, `scrollLockPreservesManualReadPosition`, `bottomPinnedRefreshKeepsLatestVisible`, `composerClearanceApplied`, `sessionListHidesInternalLifecycle`; `all_false=[]`
  - post-proof config query restored to `provider=minimax`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`
  - remaining:
  - old internal lifecycle session truth is not physically deleted; it is hidden from user-facing lists and remains directly queryable for debug/replay.
  - `output/` remains an unrelated untracked directory and was not touched.

# 2026-07-11 provider retry and Worker same-task interruption closeout

- marker:
  - `provider-retry-worker-interrupted-closeout-1783761041`
- user requirement:
  - provider failures retry automatically 10 times.
  - retry waits vary between 1 and 20 seconds in production.
  - retries before final exhaustion should not become task/user-visible state.
  - task problems should not create new tasks; retry/recovery must stay in the same task lifecycle.
- implementation:
  - provider retry cap changed from 5 to 10.
  - production provider backoff now has bounded varied timing from 1s to 20s.
  - `TaskRuntime::apply_execution_fact` now supports `ExecutionFactKind::Interrupted`, writing `TaskInterrupted`.
  - Worker provider/network executor exhaustion is mapped to `TaskInterrupted`, not `TaskBlocked`.
  - non-provider Worker errors still map to `TaskBlocked` and are not silently retried.
  - `UiExecutionFactKind::Interrupted` was added so protocol/task harnesses can express the same owner truth.
  - provider retry online fixture now sets `FREEHAND_PROVIDER_RETRY_BACKOFF_MS=0` only for proof speed; config/env restoration removes the fixture env.
  - master-worker autonomy online fixture now creates a real temporary `target_cwd` so production workerS cannot correctly block a no-cwd deterministic fixture task.
- validation:
  - `cargo test -p freehand-task execution_fact_interrupted_marks_task_retryable_without_blocked_truth -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_worker_runner_provider_error_records_interrupted_and_requeues_same_task -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_worker_runner_non_provider_execution_error_records_blocked_not_retryable -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_fails_after_ten_provider_retries_with_error_code -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_retries_recoverable_provider_errors_then_succeeds -- --nocapture` passed.
  - `cargo test -p freehand-runtime -- --nocapture` passed 125 tests.
  - `cargo test -p freehand-task -- --nocapture` passed 47 tests.
  - `cargo test -p freehand-ui-protocol -- --nocapture` passed 54 tests.
  - `cargo test -p freehand-cli -- --nocapture` passed 26 tests.
  - `cargo test -p freehand-control -- --nocapture` passed 8 tests.
  - `cargo test -p freehand-daemon daemon_adp_queries_runtime_error_center_truth -- --nocapture` passed.
  - `cargo clippy -p freehand-runtime -p freehand-task -p freehand-ui-protocol --all-targets -- -D warnings` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
  - `scripts/verify-provider-retry-online.sh` passed on S-profile `127.0.0.1:4042`: session `cli-adp-sample-provider-retry-1783760766062266000`, `mock_attempts=10`, error-center rows `retry_same_step` x9 then `fail_turn` x1.
  - post provider proof config query returned minimax/MiniMax-M3 inline auth; `~/.freehand/daemonS.env` had no provider retry fixture env.
  - first `scripts/verify-normal-master-worker-e2e.sh` run failed because the autonomy fixture created a no-cwd task while production workerS was online; ledger showed `TaskBlocked` reason `assigned worker task is missing target_cwd`.
  - after fixture target_cwd repair, `scripts/verify-normal-master-worker-e2e.sh` passed with autonomy `mock_attempts=24`; success had no `TaskBlocked`; production branches covered rejected retry, blocked decision, and same-task crash recovery.
  - crash recovery online proof: `task-normal-crash-1781783761126` history included `TaskInterrupted,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`.
  - final config query returned minimax/MiniMax-M3 inline auth; no provider/master fixture env remained.
- remaining:
  - WorkerTurnExecutor still returns `String` errors, so Worker provider-system classification uses explicit provider code substrings. A later cleanup should carry structured runtime error info across that boundary.

# 2026-07-11 timer 3-minute wait prompt policy

- user requirement:
  - default tool prompt must say waits over 3 minutes should use timer.
  - Master must not dead-wait; after scheduling a timer, it should continue later ready work.
  - timer prompt must explain what to do when the waited item is revisited.
- implementation:
  - `timer` schema description and example now include the >3-minute wait rule, no-dead-wait instruction, continue-other-ready-work instruction, and prompt duty.
  - Master orchestration guidance now says if the next useful wait exceeds 3 minutes, call `timer(op="schedule")`, continue other ready Master-side work, and write a persisted prompt that names current truth to inspect, waited condition to revisit, and decision to make.
  - docs/function maps/test designs updated for `tool.registry` and `runtime.master-worker-loop`; mainline wiki regenerated.
  - local skill updated to keep this as Freehand timer policy.
- local validation:
  - `cargo fmt --check` passed.
  - `cargo test -p freehand-tools timer_tool_schema_exposes_internal_schedule_contract -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_admits_long_operator_task_without_semantic_truncation -- --nocapture` passed.
  - `bash -n scripts/verify-timer-tool-online.sh` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- online validation:
  - pre-check config on S-profile `127.0.0.1:4042` was `provider=minimax`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`; fixture env grep was 0 matches.
  - `scripts/verify-timer-tool-online.sh` passed: timer `timer-online-proof-1783775995-84734`, `mock_attempts=3`, first state `status=active`, due state `status=completed fired_count=1`, provider request 2 `sawToolResult=true`, provider request 3 `sawTimerWakeup=true`.
  - `FREEHAND_TIMER_VERIFY_MODE=restart-due scripts/verify-timer-tool-online.sh` passed: timer `timer-online-proof-1783776198-96001`, persisted `status=active next_due_at=1783776221` before service-scoped restart, after restart due state `status=completed fired_count=1`, provider request 3 `sawTimerWakeup=true`.
  - final config restored to minimax/MiniMax-M3 inline auth; fixture env grep 0 matches.
- remaining:
  - `output/` remains unrelated untracked content and was not touched.

# 2026-07-11 WebUI submit timeout must preserve draft session

- user report:
  - after sending a task, WebUI auto-refreshed and became an empty session.
  - screenshot showed command status `dispatch failed ... request timed out after 8s`, session rail `no sessions`, and conversation empty.
- root cause:
  - submit failure catch cleared `pendingUserInput`, `pendingSubmitSessionId`, and pending attachments.
  - then `setSessionList()` could receive an empty/no-new-session list while the backend request was still ambiguous, and because no draft/pending guard remained it called `clearLocalConversationTruth()`.
  - result: a command timeout was treated as proof that no backend work existed, so the visible pending request disappeared.
- implementation:
  - when submitting with no selected session, WebUI now creates a draft `webui-session-*` and sends that session id.
  - submit failure no longer clears pending user input, pending session, or attachment draft.
  - new `pendingSubmitError` marks the pending card as dispatch status unknown and tells the operator to refresh before sending a duplicate.
  - docs/function map/test design updated for `app.webui-smoke`; asset smoke locks `pendingSubmitError`, draft creation before submit, and the new unknown-dispatch status copy.
- validation:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `cargo test -p freehand-server webui -- --nocapture` passed: 3 tests.
  - `node --check scripts/webui_verify_online.mjs` passed.
  - `cargo fmt --check` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed after rerunning sequentially; first parallel run exited 137 from cargo/file-lock contention.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
  - `scripts/install-launchd.sh restartS` passed.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
  - served `webui.js` hash matched workspace: `7c48fc3448ea7f80763901c10cf0784a35e152b08339142193a61f86a1ae333e`.
  - Playwright online proof on real 4042 page:
    - created draft session `webui-session-20260711140007-7afeb94b`.
    - set browser offline, submitted marker `FH-WEBUI-DRAFT-OFFLINE-PENDING-1783778408982`.
    - DOM had `hasMarker=true`, `hasUnknownDispatch=true`, `hasRefreshInstruction=true`, `hasDraftSession=true`, `showsNoSessionsOnly=false`, `emptyStateOnly=false`, `composerValue=""`, `pass=true`.
- remaining:
  - online proof used browser offline to deterministically trigger the same timeout/transport failure class; it did not wait for a real provider turn to time out.
# 2026-07-11 tool guidance / latest failure sample analysis

- user report:
  - tool-call execution success is low.
  - model should not spend multiple tool rounds learning what the framework does.
  - latest call failures and task startup failure need evidence-backed analysis.
- evidence sampled:
  - latest session: `webui-session-20260711144341-d20c3ef1`.
  - provider raw ledgers: `~/.freehand/ledgers/providers/anthropic/master/webui-session-20260711144341-d20c3ef1/runtime-turn-271*.jsonl`.
  - reason ledger: `~/.freehand/ledgers/reason/master/webui-session-20260711144341-d20c3ef1.jsonl`.
  - duplicate/cancelled task ledger: `~/.freehand/ledgers/tasks/master/task-1783781111.jsonl`.
  - long blocked worker sample: `~/.freehand/state/turns/worker/worker-task-task-xiaozhi-struct-002/turns/worker-turn-exec-worker-worker-1783754038173229000-0-r25.json`.
- findings:
  - `glob("/Users/*/Documents/github/xiaozhi-esp32-2.2.4")` failed with `absolute patterns are not supported`; then `glob("~/**/xiaozhi-esp32-2.2.4")` also failed/no-match. Root cause: schema did not state relative-only/tilde-invalid strongly enough or tell the model to use known external path tools/Worker target_cwd instead.
  - model called `task(op=list_tasks, status="all")`, which failed with `unsupported task status all`. Root cause: status field did not explicitly say omit `status` for all visible tasks and list the valid status filters.
  - task `task-1783781111` was created/assigned/claimed, then Master cancelled it as a malformed duplicate. Root cause is not Worker startup failure alone; Master context lacked compact current Task Center/Agent truth and schema examples did not lock create/assign/current-truth workflow tightly enough.
  - model read runtime/config/ledgers over many rounds to infer framework behavior. Root cause: framework state and behavior were not fully present in model-visible context; tool schemas were too implicit, causing probing calls.
  - worker `task-xiaozhi-struct-002` blocked after provider/network failure (`anthropic_http_request_failed` to minimax anthropic endpoint), then timer cycles appended passive blocked decisions without re-entering a useful task state. This is provider health/recovery evidence, separate from the schema prompt-guard issue.
- implementation:
  - `glob` schema now says relative-only, no absolute paths or `~`, use `ls`/`read_file` for known external paths, or Worker `target_cwd` for external repo work.
  - `task` schema now points to `TaskSpaceSnapshot`, says not to use `status="all"`, lists valid status filters, includes `interrupted`, and documents `dispatch.mode="none"` plus configured Worker assign.
  - Master live context now injects `TaskSpaceSnapshot` from TaskBoard/AgentBoard/EventInbox before the original task, including configured Worker, known tasks, valid status filters, blocked/review-ready ids, agents, and recent events.
  - docs/function maps/testing and local skill updated to lock no-trial-error tool guidance.
- validation so far:
  - `cargo test -p freehand-tools glob_tool_schema_prevents_absolute_or_tilde_trial_calls -- --nocapture` passed.
  - `cargo test -p freehand-tools task_tool_exposes_operation_parameter -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_admits_long_operator_task_without_semantic_truncation -- --nocapture` passed after adding TaskSpaceSnapshot assertions.
- remaining:
  - run fmt, mainline generation/check, gates, diff check, and live S-profile evidence before closing.

# 2026-07-12 Master framework-only tool guidance closeout

- marker:
  - `master-framework-tool-guidance-closeout-1783789815`
- issue fixed:
  - v1/v2/v3 retests showed Master still had too much tool surface and guidance ambiguity: malformed optional `freehand_status` loops, `claim="continue"` used as async wait, and direct Master external repo read/search/write behavior.
  - v4 after framework-only surface exposed a second issue: task calls repeatedly omitted top-level `op`, and a verbal "timer scheduled" claim was not backed by a real timer tool result.
- implementation:
  - Master live provider schema is now framework-only: `task` and `timer`; it excludes file/search/write tools, `todo_write`, `complete_step`, and shell.
  - Runtime rejects injected Master non-framework calls with a paired failed capability-boundary tool result, Worker dispatch guidance, and no file-content leak.
  - Worker still keeps governed file/search/write tools and excludes task/timer/shell.
  - Completion guidance locks `continue` as immediate same-turn next model round and forbids using it for Worker/timer/user/external waits.
  - Optional `freehand_status` guidance says ordinary responses omit it and only output required `<freehand_completion>` unless schema feedback explicitly asks.
  - Task schema/guidance now says every task call must include top-level `op`, shows create/assign examples, and requires expanded absolute existing repository/workspace `target_cwd` instead of `~`, glob, or output dirs.
  - Timer schema/guidance now says Master cannot claim a timer exists unless the current turn got successful `Timer scheduled` tool result.
  - docs/function maps/test designs/local skill updated for `tool.registry`, `provider.reason-live-bridge`, and `runtime.master-worker-loop`.
- local validation:
  - `cargo test -p freehand-tools master_tool_surface_excludes_unsandboxed_shell -- --nocapture` passed.
  - `cargo test -p freehand-tools glob_tool_schema_prevents_absolute_or_tilde_trial_calls -- --nocapture` passed.
  - `cargo test -p freehand-tools task_tool_exposes_operation_parameter -- --nocapture` passed.
  - `cargo test -p freehand-tools timer_tool_schema_exposes_internal_schedule_contract -- --nocapture` passed.
  - `cargo test -p freehand-blocks completion -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_admits_long_operator_task_without_semantic_truncation -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_rejects_injected_master_read_then_accepts_worker_dispatch -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_returns_tool_execution_failure_to_model_for_next_round -- --nocapture` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- online validation:
  - S-profile only: `scripts/install-launchd.sh restartS`, health `ok`, `freehand-cliS adp-smoke` passed.
  - config stayed `provider=minimax`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`; fixture env grep returned 0 matches.
  - fixed retest session `webui-session-fixed-xiaozhi-tool-retest-v6` submitted the exact reused xiaozhi prompt and completed with `rounds=3 schema_rejections=0 tool_executions=2`.
  - v6 Master tool calls were exactly `task(op="query", task_id="task-1783783921")` and `timer(op="schedule", delay_seconds=300, ...)`; forbidden Master calls count was 0 for `read_file/ls/grep/glob/write_file/edit_file/multi_edit/complete_step/todo_write/bash`.
  - v6 tool results were both success; timer result was `Timer scheduled: timer_id=timer-master-1783789815 next_due_at=1783790115 max_runs=1 fired_count=0 status=active`.
  - v6 terminal success cited Worker task `task-1783783921`, target cwd `/Users/fanzhang/Documents/github/xiaozhi-esp32-2.2.4`, active execution `exec-worker-worker-1783789054065083000-1651`, and timer `timer-master-1783789815`.
- remaining:
  - `task-1783783921` is still a long-running Worker analysis task; current scope was Master guidance/tool-surface correctness and timer-backed wait, not completion of that external analysis deliverable.
  - existing unrelated dirty worktree and untracked `output/` / `scripts/verify-timer-tool-online.sh` remain untouched.

# 2026-07-12 Worker tool failure-rate retest for reused xiaozhi prompt

- marker:
  - `worker-tool-guidance-failure-rate-retest-1783818755`
- reused prompt/task:
  - prompt: `分析 ~/Documents/github/xiaozhi-esp32-2.2.4 的项目情况，把其中的内存分配，项目架构，模块关系通过 html 渲染出来，做一个详细的项目分析给我`
  - fixed Master proof session remained `webui-session-fixed-xiaozhi-tool-retest-v6`.
  - active Worker task remained `task-1783783921`; no replacement task was created for this retest.
- pre-fix evidence:
  - Master fixed session remained good: 2 tool calls (`task`, `timer`), 2 successful tool results, 0 schema rejections, 0 forbidden Master tool calls.
  - Worker ledger for `worker-task-task-1783783921` had 691 tool calls, 683 tool results, 24 failed tool results, 0 schema rejections, 5 provider errors.
  - failed tool-result rate by result count: 24 / 683 = 3.51%.
  - failure classes: 13 `glob` failures from absolute patterns under `/Users/fanzhang/Documents/github/xiaozhi-esp32-2.2.4`, 8 `read_file` failures from directories/missing/binary files, 3 `ls` failures from missing paths.
  - `project_analysis.html` did not exist, and ADP task history had no `TaskReviewSubmitted`, `TaskClosed`, or `TaskBlocked`; it was still running with repeated `TaskInterrupted` from provider/network failures.
- implementation:
  - `glob` now accepts relative patterns and absolute patterns only when they remain under the canonical locked workspace root; it rejects `~`, `..`, and external absolute patterns.
  - absolute `glob` boundary checking canonicalizes the non-glob prefix first, so macOS `/var` vs `/private/var` does not falsely violate the workspace boundary.
  - `ls` now reports one file entry as `path<TAB>size` instead of failing when the path is an existing file.
  - `read_file`, `glob`, and `ls` schema guidance now addresses observed bad calls: directories need `ls`, generated/missing outputs should not be read, exact existence checks should use `ls`, and binary sidecars are not valid UTF-8 file reads.
  - function map, test design, mainline JSON, generated wiki, and local skill were updated with the same owner truth.
- validation:
  - `cargo test -p freehand-tools glob_tool_schema_prevents_absolute_or_tilde_trial_calls -- --nocapture` passed.
  - `cargo test -p freehand-tools glob_accepts_absolute_patterns_inside_locked_workspace_only -- --nocapture` passed after canonical-prefix fix.
  - `cargo test -p freehand-tools file_tool_schemas_guide_worker_away_from_observed_bad_calls -- --nocapture` passed.
  - `cargo test -p freehand-tools ls_lists_entries_and_recursive_tree -- --nocapture` passed.
  - `cargo test -p freehand-tools -- --nocapture` passed 37 tests.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
  - `scripts/install-launchd.sh restartS` and `scripts/install-launchd.sh restartWorkerS` completed service-scoped restarts.
  - S-profile health returned `ok`; config remained `provider=minimax`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`; daemon/worker env fixture grep returned 0 matches.
- online observation after fix:
  - Worker ledger advanced to 710 tool calls, 703 tool results, 25 failed tool results, 0 schema rejections, 5 provider errors.
  - Incremental new results after the repair window: +19 tool calls, +20 tool results, +1 failed result. The new observed failure was `read_file` on directory `main/boards`; no new in-workspace absolute `glob` rejection was observed after the new worker binary was installed.
  - ADP task history advanced to `TaskInterrupted -> TaskProgressed -> TaskAssigned` but not `TaskResumed`/review/close within the observation window.
  - `project_analysis.html` still missing. The old prompt deliverable is not complete; the current blocker is ongoing Worker/provider progress, with historical `anthropic_http_request_failed` interruptions to `https://api.minimaxi.com/anthropic/v1/messages`.
- remaining:
  - Need a later online completion pass once Worker claims and finishes `task-1783783921` or provider connectivity stabilizes.
  - Current tool fix materially removes the dominant old absolute-`glob` failure class but does not make the whole old prompt complete.

# 2026-07-12 Path tools absolute/symlink support correction

- user correction:
  - The likely root is tool support for absolute paths and symlink paths, not only prompt wording.
- implementation:
  - Added `path_tools_accept_absolute_symlink_aliases_inside_locked_workspace`.
  - The test creates a real symlink alias to the locked workspace and verifies `glob`, `grep`, `read_file`, `ls`, and `write_file` all accept absolute alias paths that canonicalize back into the locked workspace.
  - This locks the expected Freehand behavior for user-facing paths like `~/github/repo` resolving to canonical paths like `/Users/fanzhang/Documents/github/repo`.
  - Docs/mainline/skill now say path tools must canonicalize symlink aliases before workspace-boundary decisions.
- validation:
  - `cargo test -p freehand-tools path_tools_accept_absolute_symlink_aliases_inside_locked_workspace -- --nocapture` passed.
  - `cargo test -p freehand-tools -- --nocapture` passed 38 tests.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- remaining:
  - No new online completion pass was run after this test-only contract hardening; previous online blocker remains Worker/provider progress for `task-1783783921`.

# 2026-07-12 Resource center before function map rule

- user correction:
  - Function map refactor must start from a resource center, not from the old feature-local function map model.
  - Global `AGENTS.md` and coding-principles rules must be updated before local project architecture refactor.
- implementation:
  - Updated global `/Users/fanzhang/.codex/AGENTS.md` with `资源中心先于 function map`.
  - Updated global `/Users/fanzhang/.codex/skills/coding-principals/SKILL.md` and compressed dev-flow-code skill with resource-center-first ordering.
  - Added `docs/resource-maps/core.json` and `docs/resource-maps/README.md`.
  - Updated Freehand function-map spec, function-map README, dev gates, feature map, and local dev skill so project work starts from resource map before function map.
  - Added `xtask` resource-map gate: parses resource manifest, enforces unique `resource_type`, owner feature presence, valid operation source/target resources, existing mainline call docs, valid direct/indirect relation resources, and valid forbidden direct relation `required_via`.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `git diff --check` passed.
- remaining:
  - This is the first resource-center skeleton and gate. It does not yet prove every code caller/callee edge is resource-bound; next step is to migrate existing feature maps/mainline call maps to reference resource operation bindings.

# 2026-07-12 Resource operation backlinks and gate hardening

- goal source:
  - `/Users/fanzhang/.codex/attachments/e2cfd589-69d8-4a0e-8e26-1b035fba8e86/pasted-text-1.txt`.
- implementation:
  - Project `AGENTS.md` now says Freehand is resource-map-first ownership, then function-map-bound development/debugging.
  - `docs/resource-maps/core.json` now covers required resources: `config`, `session`, `turn`, `request_context`, `provider_request`, `provider_response`, `tool_call`, `workspace_path`, `task`, `agent`, `timer`, `error`, `metadata`, `debug_trace`, `ui_projection`, `runtime_command`, `checkpoint`, `node_pairing`, `instruction_capability`.
  - Added resource operation bindings for checkpoint, node pairing, and instruction capability:
    - `workspace_path.checkpoint_before_write`
    - `runtime_command.rewind_checkpoint`
    - `config.bootstrap_node_pairing`
    - `node_pairing.project_to_ui`
    - `config.compile_instruction_capability`
    - `instruction_capability.admit_request_context` as `pending`
  - Added `resource_operations` backlinks to the relevant mainline JSON files and regenerated generated wiki.
  - Added resource-map backlinks and operation ids to paired function maps and test designs.
  - Hardened `xtask gates check` so resource operation ids are unique, owner feature matches the referenced mainline doc, operation bindings are backlinked from mainline JSON/function map/test design, relation resources are valid, and forbidden/indirect relation pairs cannot also appear as direct operation bindings.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - Gate now locks resource map consistency and operation backlinks. It still does not statically inspect every Rust call edge for resource shortcuts; that is the next deeper enforcement layer.

# 2026-07-12 Mainline call-table resource operation row backlinks

- implementation:
  - Added optional `resource_operation` to mainline call-table rows.
  - `xtask gates check` now requires every `bound` resource operation in `docs/resource-maps/core.json` to be backlinked from at least one call-table row in the referenced mainline JSON.
  - `xtask gates check` also rejects call-table row `resource_operation` values that are not listed in that same mainline source's `resource_operations`.
  - Generated wiki now includes a `resource operation` column in function call tables.
  - Added row-level backlinks for 13 bound operations:
    - `reason.persistence` step `06` -> `session.append_turn_to_turn`
    - `reason.context-planner` step `01` -> `turn.plan_request_context`
    - `provider.reason-live-bridge` step `08` -> `request_context.build_provider_request`
    - `reason.turn` step `02` -> `provider_response.apply_to_turn`
    - `tool.registry` step `05` -> `tool_call.execute_workspace_path`
    - `ui.protocol` step `26` -> `task.project_to_ui`
    - `runtime.master-worker-loop` step `12` -> `timer.fire_master_wakeup`
    - `error.center` step `04` -> `error.record_metadata`
    - `runtime.checkpoint-rewind` steps `03` and `05` -> checkpoint operations
    - `node.master-slave` steps `01` and `08` -> node pairing operations
    - `instruction.capability-loader` step `02` -> `config.compile_instruction_capability`
  - `instruction_capability.admit_request_context` remains doc-level `pending` and has no fake call-table row.
- validation:
  - `jq empty` on touched mainline JSON files passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This proves resource operation backlinks at manifest/mainline row level. Full source-code shortcut scanning remains a later gate.

# 2026-07-12 Required core resources gate

- goal source:
  - `/Users/fanzhang/.codex/attachments/e2cfd589-69d8-4a0e-8e26-1b035fba8e86/pasted-text-1.txt`.
- audit finding:
  - `docs/resource-maps/core.json` currently had all 19 resources required by the objective, but `xtask gates check` did not lock that required set.
  - Removing a core resource could fail indirectly later, but there was no targeted resource-coverage error.
- implementation:
  - Added `verify_required_core_resources` for `resource_map_id=freehand.core-resource-map`.
  - Required resources: `config`, `session`, `turn`, `request_context`, `provider_request`, `provider_response`, `tool_call`, `workspace_path`, `task`, `agent`, `timer`, `error`, `metadata`, `debug_trace`, `ui_projection`, `runtime_command`, `checkpoint`, `node_pairing`, `instruction_capability`.
  - Added red test `resource_map_rejects_missing_required_core_resource`.
  - Kept fixture maps exempt unless they opt into `freehand.core-resource-map`, so tests do not need to duplicate production resources.
  - Updated resource-map README and dev-gates docs.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 10 tests.
- remaining:
  - Run full manifest/gate stack after formatting.
  - This locks the objective's explicit resource coverage list; it does not prove code refactor completion.

# 2026-07-12 Resource owner crate backlink gate

- goal source:
  - `/Users/fanzhang/.codex/attachments/e2cfd589-69d8-4a0e-8e26-1b035fba8e86/pasted-text-1.txt`.
- audit finding:
  - Resource map entries had `owner_crate`, and feature-map seed entries had `owner`, but `xtask gates check` did not verify they agreed.
  - This allowed resource ownership to drift between resource map and feature map while both still mentioned the same `feature_id`.
- implementation:
  - Added feature-map seed owner parsing to `xtask`.
  - `xtask gates check` now requires every resource's `owner_crate` to appear in the owning feature's seed `owner` line.
  - Added red test `resource_map_rejects_feature_owner_crate_mismatch`.
  - Updated resource-map README and dev-gates docs.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 9 tests.
  - First full `xtask gates check` exposed a parser bug: `node.master-slave` was followed by a `## ui.platform-architecture` non-seed section whose `- owner:` line overwrote the seed owner.
  - Fixed parser to end a seed entry on any `## ` heading and to keep only the first `- owner:` line for each seed entry.
  - After parser fix, `cargo test -p xtask resource_map_ -- --nocapture` passed 9 tests and `cargo run -p xtask -- gates check` passed.
- remaining:
  - Run full manifest/gate stack after formatting.
  - This checks textual owner crate alignment, not all code-level ownership paths.

# 2026-07-12 Feature-map resource owner uniqueness gate

- goal source:
  - `/Users/fanzhang/.codex/attachments/e2cfd589-69d8-4a0e-8e26-1b035fba8e86/pasted-text-1.txt`.
- audit finding:
  - The Resource Ownership Index backlink existed, but the gate only checked resource-map -> feature-map presence.
  - It did not reject feature-map ownership rows listing unknown resources or the same resource under more than one feature.
- implementation:
  - `xtask gates check` now rejects unknown resources in `docs/architecture/feature-map.md` `Resource Ownership Index`.
  - Gate rejects duplicate feature-map resource owners for the same `resource_type`.
  - Gate rejects feature-map owner/resource pairs that do not match the resource map's `owner_feature_id`.
  - Added xtask red tests `resource_map_rejects_unknown_feature_map_resource` and `resource_map_rejects_duplicate_feature_map_resource_owner`.
  - Updated resource-map README and dev-gates docs.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 8 tests.
- remaining:
  - Run full manifest/gate stack after formatting.
  - This locks feature-map resource ownership uniqueness; it still does not auto-discover every Rust call edge.

# 2026-07-12 Feature-map resource ownership backlinks

- goal source:
  - `/Users/fanzhang/.codex/attachments/e2cfd589-69d8-4a0e-8e26-1b035fba8e86/pasted-text-1.txt`.
- audit finding:
  - `docs/resource-maps/core.json` mapped each resource to `owner_feature_id`, but `docs/architecture/feature-map.md` only had one global `resource_map_doc` reference and did not list owned resources per feature.
  - This made the feature map a weak backlink instead of an aligned resource ownership registry.
- implementation:
  - Added `## Resource Ownership Index` to `docs/architecture/feature-map.md`.
  - Indexed every current resource owner feature and owned `resource_type`, with a backlink to `docs/resource-maps/core.json`.
  - `xtask gates check` now parses the Resource Ownership Index and requires every resource owner feature to list its resource.
  - Gate rejects missing ownership rows, ownership rows pointing to a non-core resource map path, duplicate feature rows, and empty resource cells.
  - Added xtask red test `resource_map_rejects_missing_feature_map_resource_backlink`.
  - Updated resource-map README, dev-gates docs, function-map docs, function-map spec, and freehand-dev skill.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 6 tests.
- remaining:
  - Run full manifest/gate stack after formatting.
  - Feature-map backlink now covers resource ownership, not every relation edge; relation edge truth remains in the resource map.

# 2026-07-12 Resource operation allowlist binding

- goal source:
  - `/Users/fanzhang/.codex/attachments/e2cfd589-69d8-4a0e-8e26-1b035fba8e86/pasted-text-1.txt`.
- audit finding:
  - `operation_bindings` were registered, but many `operation_id` suffixes were not present in the source resource's `operations` allowlist.
  - This meant resource-level allowed operations were descriptive rather than authoritative.
- implementation:
  - Added bound/pending operation suffixes to the relevant source resource `operations` arrays in `docs/resource-maps/core.json`.
  - `xtask gates check` now requires operation ids to use `<source_resource>.<operation>` format.
  - Gate rejects operation bindings whose source prefix differs from `source_resource`.
  - Gate rejects operation bindings whose operation suffix is not listed in the source resource's `operations`.
  - Gate rejects duplicate operation names inside a resource.
  - Added xtask red test `resource_map_rejects_operation_not_declared_on_source_resource`.
  - Updated resource-map README, dev-gates docs, function-map spec, and freehand-dev skill.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 5 tests.
- remaining:
  - Run full manifest/gate stack after formatting.
  - This locks declared operation ids to resource allowlists; it does not inspect all arbitrary Rust function calls.

# 2026-07-12 Direct relation rules for bound operations

- goal source:
  - `/Users/fanzhang/.codex/attachments/e2cfd589-69d8-4a0e-8e26-1b035fba8e86/pasted-text-1.txt`.
- audit finding:
  - 13 bound operation pairs existed, but only `node_pairing -> ui_projection` had an explicit `allowed_direct=true` relation rule.
  - This left most direct relations implicit through operation bindings, which did not satisfy the goal that direct / indirect / forbidden direct relations be clear in the resource map.
- implementation:
  - Added `allowed_direct=true` relation rules for all 13 currently bound operation pairs.
  - Updated `xtask gates check` so every bound operation source/target pair must have an `allowed_direct=true` relation rule.
  - Gate now rejects duplicate relation rule ids, duplicate relation source/target pairs, and direct rules that declare `via_resources`.
  - Added xtask red test `resource_map_rejects_missing_direct_relation_rule`.
  - Updated resource-map README, dev-gates docs, function-map docs, and freehand-dev skill to state operation bindings do not imply direct relation permission.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 4 tests.
- remaining:
  - Run full manifest/gate stack after formatting.
  - Pending operation `instruction_capability.admit_request_context` still has no direct relation rule by design because it is not source-bound.

# 2026-07-12 Source edge registry

- goal source:
  - `/Users/fanzhang/.codex/attachments/e2cfd589-69d8-4a0e-8e26-1b035fba8e86/pasted-text-1.txt`.
- implementation:
  - Added `source_edge_registry` to `docs/resource-maps/core.json` as the resource-center-level index for code-bound direct resource edges.
  - Registered 13 currently bound mainline resource-operation rows:
    - `reason.persistence#06` -> `session.append_turn_to_turn`
    - `reason.context-planner#01` -> `turn.plan_request_context`
    - `provider.reason-live-bridge#08` -> `request_context.build_provider_request`
    - `reason.turn#02` -> `provider_response.apply_to_turn`
    - `tool.registry#05` -> `tool_call.execute_workspace_path`
    - `ui.protocol#26` -> `task.project_to_ui`
    - `runtime.master-worker-loop#12` -> `timer.fire_master_wakeup`
    - `error.center#04` -> `error.record_metadata`
    - `runtime.checkpoint-rewind#03` -> `workspace_path.checkpoint_before_write`
    - `runtime.checkpoint-rewind#05` -> `runtime_command.rewind_checkpoint`
    - `node.master-slave#01` -> `config.bootstrap_node_pairing`
    - `node.master-slave#08` -> `node_pairing.project_to_ui`
    - `instruction.capability-loader#02` -> `config.compile_instruction_capability`
  - `instruction_capability.admit_request_context` stays `pending` and has no fake source-edge row.
  - `xtask gates check` now validates source-edge registry entries against operation bindings and mainline call-table rows in both directions.
  - Added xtask red test `resource_map_rejects_missing_source_edge_registry`.
  - Updated resource-map, mainline-call, function-map, dev-gate, and local skill docs to make the registry a required resource-center truth.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 3 tests.
- remaining:
  - Run full manifest/gate stack after formatting.
  - Whole-repo automatic Rust call-graph discovery of undeclared resource edges remains future work.

# 2026-07-12 Mainline call-table resource endpoint gate

- implementation:
  - Added optional `source_resource` and `target_resource` to mainline call-table rows.
  - Generated wiki function call tables now render `source resource`, `target resource`, and `resource operation`.
  - `xtask gates check` now requires any row with `resource_operation` to declare both endpoints and match the exact `source_resource` / `target_resource` from `docs/resource-maps/core.json`.
  - `xtask gates check` rejects rows that declare only source/target resources without a `resource_operation`.
  - Current bound row endpoint triples:
    - `reason.persistence` step `06`: `session -> turn` via `session.append_turn_to_turn`
    - `reason.context-planner` step `01`: `turn -> request_context` via `turn.plan_request_context`
    - `provider.reason-live-bridge` step `08`: `request_context -> provider_request` via `request_context.build_provider_request`
    - `reason.turn` step `02`: `provider_response -> turn` via `provider_response.apply_to_turn`
    - `tool.registry` step `05`: `tool_call -> workspace_path` via `tool_call.execute_workspace_path`
    - `ui.protocol` step `26`: `task -> ui_projection` via `task.project_to_ui`
    - `runtime.master-worker-loop` step `12`: `timer -> turn` via `timer.fire_master_wakeup`
    - `error.center` step `04`: `error -> metadata` via `error.record_metadata`
    - `runtime.checkpoint-rewind` step `03`: `workspace_path -> checkpoint` via `workspace_path.checkpoint_before_write`
    - `runtime.checkpoint-rewind` step `05`: `runtime_command -> checkpoint` via `runtime_command.rewind_checkpoint`
    - `node.master-slave` step `01`: `config -> node_pairing` via `config.bootstrap_node_pairing`
    - `node.master-slave` step `08`: `node_pairing -> ui_projection` via `node_pairing.project_to_ui`
    - `instruction.capability-loader` step `02`: `config -> instruction_capability` via `config.compile_instruction_capability`
- validation:
  - `jq empty` on resource map and touched mainline JSON files passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This locks resource operation endpoints at manifest row level. It still does not parse Rust call graphs for undeclared resource edges.

# 2026-07-12 Resource source shortcut gate

- implementation:
  - Added `source_shortcut_gates` to `docs/resource-maps/core.json`.
  - Added `xtask gates check` validation for selected forbidden direct resource relations that are safe to scan statically.
  - Gate now checks the source resource owner crate `Cargo.toml` does not depend on forbidden target packages and Rust files under that owner crate do not contain forbidden target import/reference tokens.
  - First statically checked forbidden pairs:
    - `ui_projection -> task`: forbids `freehand-task` / `freehand_task`
    - `ui_projection -> session`: forbids `freehand-reason` / `freehand_reason`
    - `metadata -> request_context`: forbids `freehand-blocks` / `freehand_blocks`
    - `instruction_capability -> provider_request`: forbids `freehand-runtime`, `freehand-provider-core`, `freehand-provider-anthropic`, `freehand-provider-openai` and matching Rust import tokens
    - `ui_projection -> node_pairing`: forbids `freehand-node` / `freehand_node`
  - Runtime orchestrator forbidden pairs such as `timer -> task` and `runtime_command -> workspace_path` are intentionally not broad dependency-scanned because `freehand-runtime` legitimately depends on multiple owner crates; those need the later call-edge gate.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This is source-level shortcut scanning for selected owner crates, not full Rust call-graph enforcement.

# 2026-07-12 Function-map resource binding sections

- implementation:
  - `xtask gates check` now requires every function map tied to a mainline source with `resource_operations` to contain:
    - `## Resource Map Binding`
    - `owned resources:`
    - `touched resources:`
    - `forbidden shortcuts:`
  - Backfilled Resource Map Binding sections in 11 function maps:
    - `reason.persistence`
    - `reason.context-planner`
    - `provider.reason-live-bridge`
    - `reason.turn`
    - `tool.registry`
    - `ui.protocol`
    - `runtime.master-worker-loop`
    - `error.center`
    - `runtime.checkpoint-rewind`
    - `node.master-slave`
    - `instruction.capability-loader`
  - Each section names the resource map, owned resources, touched resources, resource operations, and forbidden shortcuts for that feature-local function map.
- validation:
  - `rg` confirmed all 11 function maps contain the required section labels.
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This locks function-map section presence and resource alignment language. It does not prove every listed owned/touched resource is semantically exhaustive.

# 2026-07-12 Resource operation test coverage gate

- goal source:
  - `/Users/fanzhang/.codex/attachments/e2cfd589-69d8-4a0e-8e26-1b035fba8e86/pasted-text-1.txt`.
- implementation:
  - Added `## Resource Operation Test Coverage` tables to the 11 current resource-backed test designs.
  - Each table row maps the resource operation id to `status`, `white-box`, `module black-box`, and `project black-box` coverage.
  - `instruction_capability.admit_request_context` remains explicit `pending`; no fake code binding or fake test claim was added.
  - `xtask gates check` now requires every resource operation binding to have a matching test coverage table row in the paired test design, checks the row status against `binding_status`, and rejects empty coverage cells.
  - Updated `docs/architecture/dev-gates.md`, `docs/resource-maps/README.md`, and `.agents/skills/freehand-dev/SKILL.md` to make resource-operation test coverage part of the resource-center workflow.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `cargo test -p xtask -- --nocapture` passed 23 tests.
  - `git diff --check` passed.
- remaining:
  - Test design coverage is now gate-structured at resource-operation level.
  - Full Rust source call-graph scanning for every undeclared resource edge remains a deeper future gate.
  - No business-code resource-owner refactor has started in this slice by design.

# 2026-07-12 Forbidden relation source-gate status

- implementation:
  - Added `source_gate_status` and `source_gate_reason` to every `forbidden_direct_relations` entry in `docs/resource-maps/core.json`.
  - `checked` relations must have a matching `source_shortcut_gates` entry.
  - `deferred` relations must carry an explicit reason; current deferred pairs are `timer -> task` and `runtime_command -> workspace_path` because `freehand-runtime` legitimately orchestrates multiple owner crates and needs a future precise call-edge gate instead of broad dependency scanning.
  - `xtask gates check` now rejects duplicate forbidden relation pairs, checked relations without a source shortcut gate, unsupported source gate statuses, missing status/reason fields, and `source_shortcut_gates` entries that are not backed by a declared forbidden relation.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, and `.agents/skills/freehand-dev/SKILL.md` to make checked/deferred source-gate status part of the resource-center workflow.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask -- --nocapture` passed 23 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed after rerunning sequentially; the earlier parallel run raced against generation and saw an out-of-date wiki.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - Source shortcut coverage is no longer a silent selected allowlist; unchecked forbidden relations are explicit deferred gaps.
  - The deferred runtime orchestrator pairs still need a precise source call-edge gate before full code-edge closure can be claimed.

# 2026-07-12 Precise source edge gates for runtime orchestrator relations

- implementation:
  - Added `precise_source_edge_gates` to `docs/resource-maps/core.json`.
  - Upgraded `timer -> task` and `runtime_command -> workspace_path` from `source_gate_status=deferred` to `source_gate_status=precise_checked`.
  - `timer -> task` precise gate checks `ProductionMasterRunner::handle_due_timer` in `crates/freehand-runtime/src/master_runner.rs`.
    - required owner-hop tokens: `claim_due_timer_schedule`, `timer_live_request`, `execute_timer`, `complete_due_timer_schedule`.
    - forbidden direct Task Center tokens: `open_task_center`, `TaskRuntime`, `TaskMutationRequest`, `record_execution`, `submit_review`, `approve_review`, `close_task`.
  - `runtime_command -> workspace_path` precise gate checks `rewind_checkpoint` in `crates/freehand-runtime/src/lib.rs`.
    - required owner-hop tokens: `RuntimeCheckpointStore::new`, `store.rewind`.
    - forbidden direct workspace mutation/tool tokens: `fs::`, `write_text_atomic`, `with_workspace_root`, `execute_registry_tool_call`.
  - `xtask gates check` now validates `precise_checked` forbidden relations have matching precise gates, each precise gate points to a declared forbidden relation, its file exists, its symbol resolves, its function body contains every required token, and its function body contains no forbidden token.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, and `.agents/skills/freehand-dev/SKILL.md` for `precise_checked` semantics.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask -- --nocapture` passed 23 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - The two previous deferred runtime orchestrator relations now have precise source body checks.
  - This is still selected precise source-edge coverage, not a complete whole-repo Rust call graph for every possible undeclared edge.

# 2026-07-12 Deferred source-gate status rejected

- implementation:
  - `xtask gates check` no longer accepts `source_gate_status="deferred"` for forbidden direct resource relations.
  - Added `parse_source_gate_status` with accepted statuses `checked` and `precise_checked`.
  - Added red test `resource_source_gate_status_rejects_deferred`; `cargo test -p xtask` now proves `deferred` is rejected.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, and `.agents/skills/freehand-dev/SKILL.md` so future forbidden relations must add `source_shortcut_gates` or `precise_source_edge_gates` instead of becoming documented gaps.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask -- --nocapture` passed 24 tests.
  - `cargo run -p xtask -- gates check` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `git diff --check` passed.
- remaining:
  - Current forbidden direct relations all have active source gate status: 5 `checked`, 2 `precise_checked`, 0 `deferred`.
  - Whole-repo automatic discovery of all undeclared Rust resource edges remains future work.

# 2026-07-12 Mainline unregistered direct resource edge red test

- implementation:
  - Added xtask fixture coverage for `verify_resource_map`.
  - `resource_map_accepts_registered_direct_edge` proves an aligned call-table row with `source_resource`, `target_resource`, and `resource_operation` passes when backed by `operation_bindings`.
  - `resource_map_rejects_unregistered_direct_edge_row` proves a row with `source_resource`/`target_resource` but no `resource_operation` fails.
  - Updated `docs/resource-maps/README.md` and `docs/architecture/dev-gates.md` to state that mainline rows cannot declare resource endpoints without a valid resource operation.
- validation:
  - `cargo test -p xtask -- --nocapture` passed 26 tests.
- remaining:
  - This locks machine-readable mainline rows against undeclared direct resource edges.
  - It does not yet auto-discover resource edges from arbitrary Rust code outside the manifest/call-table surface.

# 2026-07-12 Resource projection gate

- implementation:
  - `xtask gates check` now rejects resources with no declared projections.
  - Projection strings must be non-empty and unique within each resource.
  - Added red test `resource_map_rejects_missing_resource_projection`.
  - Updated resource-map README, dev-gates docs, and local Freehand skill so resource creation starts with owner, truth store, operations, and observable projections before code refactor.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 11 focused resource-map tests.
  - `cargo fmt --check` passed after running `cargo fmt`.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask -- --nocapture` passed 35 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
  - `mempalace mine . --wing freehand --agent codex --limit 3` passed after a prior palace lock cleared.
  - `mempalace search --wing freehand --results 3 "resource projection gate projectionless resources"` found the new MEMORY/note entries.
- remaining:
  - This proves projections are declared and gate-parseable; it does not yet prove every projection has complete runtime coverage.

# 2026-07-12 Operation binding contract field gate

- audit finding:
  - Objective says each operation binding must have `operation_id`, `owner_feature_id`, `source_resource`, `target_resource`, `effect`, `mainline_call_doc`, and `binding_status`.
  - `xtask gates check` already validated many semantic links, but did not explicitly reject empty `effect` or other empty binding contract fields before backlink checks.
- implementation:
  - `verify_resource_map` now rejects empty operation binding fields: `operation_id`, `owner_feature_id`, `source_resource`, `target_resource`, `effect`, `mainline_call_doc`, and `binding_status`.
  - Added red test `resource_map_rejects_empty_operation_binding_effect`.
  - Updated resource-map README, dev-gates docs, and local Freehand skill so operation binding field completeness is part of the resource-center gate.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 12 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 36 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
  - `mempalace mine . --wing freehand --agent codex --limit 3` passed.
  - `mempalace search --wing freehand --results 3 "EmptyOperationBindingEffect operation_bindings.effect"` found the new MEMORY/note entries.
- remaining:
  - This locks operation binding contract field presence. It does not yet add whole-repo automatic Rust call-graph discovery for every undeclared resource edge.

# 2026-07-12 Relation rule reason gate

- audit finding:
  - Resource relation rules had `reason`, but `xtask gates check` did not reject an empty reason.
  - This left direct/indirect relation permission structurally valid but under-explained in the resource-center truth.
- implementation:
  - `verify_resource_map` now rejects empty `relation_rules.reason`.
  - Added red test `resource_map_rejects_empty_relation_rule_reason`.
  - Updated resource-map README, dev-gates docs, and local Freehand skill so every relation rule must explain why the direct edge is allowed or why an indirect edge must route through `via_resources`.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 13 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 37 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This locks relation rule reason presence. It does not yet add whole-repo automatic Rust call-graph discovery for every undeclared resource edge.

# 2026-07-12 Forbidden/direct relation conflict gate

- audit finding:
  - `forbidden_direct_relations` rejected operation bindings for the same direct pair, but did not reject an `allowed_direct=true` relation rule for the same pair.
  - This allowed one resource edge to be simultaneously modeled as directly allowed and forbidden, weakening `docs/resource-maps/core.json` as the unique relation truth.
- implementation:
  - `verify_resource_map` now rejects a forbidden direct relation whose source/target pair is also present in an `allowed_direct=true` relation rule.
  - Added red test `resource_map_rejects_forbidden_allowed_direct_conflict`.
  - Updated resource-map README, dev-gates docs, and local Freehand skill so a resource pair cannot be both directly allowed and forbidden.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 14 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 38 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This locks allowed-vs-forbidden direct relation consistency. It does not yet add whole-repo automatic Rust call-graph discovery for every undeclared resource edge.

# 2026-07-12 Source shortcut gate no-op rejection

- audit finding:
  - `source_shortcut_gates` entries were required to point at forbidden direct relation pairs, but could declare no `forbidden_packages` and no `forbidden_import_tokens`.
  - Such an entry satisfies the structural backlink but enforces no shortcut boundary, which violates the no silent allowlist direction.
- implementation:
  - `verify_resource_map` now rejects source shortcut gates with empty `reason`.
  - Gate now rejects source shortcut gates with both `forbidden_packages` and `forbidden_import_tokens` empty.
  - Gate now rejects empty entries inside `forbidden_packages` and `forbidden_import_tokens`.
  - Added red test `resource_map_rejects_noop_source_shortcut_gate`.
  - Updated resource-map README, dev-gates docs, and local Freehand skill.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 15 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 39 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This locks source shortcut gates against no-op checked entries. It does not yet add whole-repo automatic Rust call-graph discovery for every undeclared resource edge.

# 2026-07-12 Source gate pair uniqueness

- audit finding:
  - `source_shortcut_gates` and `precise_source_edge_gates` were collected into sets for forbidden-relation matching, but duplicate source/target gate entries were not rejected.
  - This allowed multiple gate rows to claim the same forbidden resource pair, weakening the resource map as unique relation truth.
- implementation:
  - `verify_resource_map` now rejects duplicate `source_shortcut_gates` source/target pairs before source file scanning.
  - `verify_resource_map` now rejects duplicate `precise_source_edge_gates` source/target pairs before precise body scanning.
  - Precise source-edge `required_tokens` and `forbidden_tokens` entries now must be non-empty.
  - Added red test `resource_map_rejects_duplicate_source_shortcut_gate_pair`.
  - Updated resource-map README, dev-gates docs, and local Freehand skill.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 16 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 40 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This locks source/precise gate pair uniqueness. It does not yet add whole-repo automatic Rust call-graph discovery for every undeclared resource edge.

# 2026-07-12 Forbidden direct relation reason gate

- audit finding:
  - Objective requires every forbidden direct relation to write `required_via` and a reason.
  - `ResourceMapForbiddenRelation.reason` existed but was dead-code allowed and not validated, so a forbidden shortcut could have an empty reason.
- implementation:
  - `verify_resource_map` now rejects empty `forbidden_direct_relations.reason`.
  - Removed dead-code allowance from `ResourceMapForbiddenRelation.reason`.
  - Added red test `resource_map_rejects_empty_forbidden_direct_relation_reason`.
  - Updated resource-map README, dev-gates docs, and local Freehand skill.
- validation so far:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 17 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 41 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This locks forbidden direct relation reason presence. It does not yet add whole-repo automatic Rust call-graph discovery for every undeclared resource edge.

# 2026-07-12 Precise source edge duplicate red test

- audit finding:
  - `verify_resource_map` rejected duplicate `precise_source_edge_gates` pairs after the source gate pair uniqueness work, but only duplicate `source_shortcut_gates` had explicit fixture coverage.
  - This left the precise gate duplicate invariant covered by code and full gate only, not by a focused red test.
- implementation:
  - Added red test `resource_map_rejects_duplicate_precise_source_edge_gate_pair`.
  - Added fixture mode `DuplicatePreciseSourceEdgeGate`, which declares a `precise_checked` forbidden relation and two precise source-edge gates for the same `beta -> alpha` pair.
- validation:
  - `cargo fmt --check` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 18 focused resource-map tests at the time of this change.
  - Later full stack in the same resource-center closeout passed with `jq empty docs/resource-maps/core.json`, `cargo check -p xtask`, `cargo test -p xtask -- --nocapture` at 43 tests after the indirect-rule backlink addition, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check`.
- remaining:
  - MemoryPalace mine/search still needs to run after the latest note/MEMORY updates.

# 2026-07-12 Forbidden direct relation indirect-rule backlink

- audit finding:
  - `forbidden_direct_relations.required_via` named the required owner path, but `xtask gates check` only validated that each required resource existed.
  - Two production forbidden pairs, `metadata -> request_context` and `ui_projection -> node_pairing`, did not have matching `allowed_direct=false` relation rules, so the resource map could describe a forbidden shortcut without also declaring the indirect relation truth for that same pair.
- implementation:
  - Added indirect relation rules `metadata-to-request-context-forbidden-through-owner` and `ui-to-node-pairing-through-runtime-command` to `docs/resource-maps/core.json`.
  - Updated `verify_resource_map` so every forbidden direct relation must have a matching indirect relation rule with the same source, target, and `via_resources`/`required_via`.
  - Added red test `resource_map_rejects_forbidden_without_indirect_relation_rule`.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, `.agents/skills/freehand-dev/SKILL.md`, `docs/function-maps/foundation.workspace.md`, `docs/testing/foundation.workspace.md`, and `docs/mainline-calls/foundation.workspace.json`.
  - Regenerated wiki through `cargo run -p xtask -- mainlines generate`.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 19 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 43 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This locks forbidden direct relations to declared indirect relation rules.
  - Whole-repo automatic discovery of all undeclared Rust resource edges remains future work.

# 2026-07-12 Function-map resource binding section gate

- audit finding:
  - `verify_resource_map` required paired function maps to contain `## Resource Map Binding`, `owned resources:`, `touched resources:`, `forbidden shortcuts:`, and the operation id somewhere in the file.
  - This allowed a function map to satisfy the backlink with empty resource binding labels or scattered prose, weakening the requirement that function maps declare owned resources, touched resources, resource operations, and forbidden shortcuts.
- implementation:
  - Added `resource_map_binding_section` and `require_function_map_binding_label_has_value`.
  - `verify_resource_map` now checks the function map's `## Resource Map Binding` section specifically.
  - For every feature with resource operations, that section must declare non-empty `owned resources`, `touched resources`, `resource operations`, and `forbidden shortcuts`.
  - The same section must name the operation id plus the operation's source and target resource types.
  - Added red test `resource_map_rejects_empty_function_map_resource_binding`.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, `docs/function-maps/README.md`, `docs/architecture/function-map-spec.md`, and `.agents/skills/freehand-dev/SKILL.md`.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 20 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 44 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This locks function-map resource backlinks against empty binding sections.
  - It still does not perform whole-repo automatic Rust call-graph discovery for every undeclared resource edge.

# 2026-07-12 Bound resource operation test coverage placeholder gate

- audit finding:
  - `## Resource Operation Test Coverage` rows accepted any non-empty white-box/module black-box/project black-box text for `bound` operations.
  - Existing bound rows could still say `future`, `pending`, or `not claimed`, which made the test design look mapped while admitting the verification entrance was not current.
  - The row lookup also matched any table row containing an operation id anywhere in the row, so a coverage note mentioning another operation id could be mistaken for that operation's own row.
- implementation:
  - `require_resource_operation_test_coverage` now finds a coverage row only when the first table cell exactly matches the operation id.
  - For `binding_status=bound`, coverage cells now reject placeholder language: `pending`, `future`, `not claimed`, `not yet`, `todo`, and `tbd`.
  - Added red tests `resource_map_rejects_pending_coverage_for_bound_operation` and `resource_map_rejects_operation_only_mentioned_in_wrong_coverage_cell`.
  - Updated bound test-design rows in `instruction.capability-loader` and `node.master-slave` so their coverage cells describe current verification entrances and move live-transport gaps outside the bound coverage claim.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, and `.agents/skills/freehand-dev/SKILL.md`.
- validation:
  - `grep -RIn "| .*bound .*\\(pending\\|future\\|not claimed\\|not yet\\|TODO\\|TBD\\)" docs/testing/*.md || true` returned no matches.
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 22 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 46 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This prevents bound operations from carrying future/pending coverage placeholders and fixes exact coverage-row lookup.
  - It still does not prove that every named coverage phrase is an executable command; that is a separate stricter gate candidate.

# 2026-07-12 Bound coverage command-entry gate

- audit finding:
  - `bound` Resource Operation Test Coverage cells could be current-looking prose without an executable verification entry.
  - This made docs/gates prove that coverage was named, but not that another worker could run the mapped white-box/module black-box/project black-box checks.
- implementation:
  - `require_resource_operation_test_coverage` now rejects `bound` coverage cells that do not include a command-style verification entry.
  - Added red test `resource_map_rejects_bound_coverage_without_command_entry`.
  - Updated bound rows in `docs/testing/*.md` so white-box, module black-box, and project black-box cells include runnable command-style entries.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, and `.agents/skills/freehand-dev/SKILL.md`.
- validation:
  - `grep -RIn "| .*bound .*\\(pending\\|future\\|not claimed\\|not yet\\|TODO\\|TBD\\)" docs/testing/*.md || true` returned no matches.
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 23 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 47 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This proves every `bound` coverage cell contains a command-style entry, but it does not yet parse and execute each named command from every test-design row.
  - Whole-repo automatic discovery of all undeclared Rust resource edges remains future work.

# 2026-07-12 Resource-center objective completion audit

- current objective evidence:
  - `docs/resource-maps/core.json` currently declares 19 required core resources.
  - Current operation binding counts are 14 total, 13 `bound`, and 1 `pending`.
  - Current relation counts are 21 relation rules, 7 forbidden direct relations, 5 broad source shortcut gates, and 2 precise source-edge gates.
  - The only pending operation is `instruction_capability.admit_request_context`; it is intentionally not faked into `source_edge_registry` or call-table source edges.
  - All 7 forbidden direct relations have active `source_gate_status` values: 5 `checked` and 2 `precise_checked`; 0 `deferred`.
- validation evidence:
  - placeholder grep over `docs/testing/*.md` returned no matches.
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 23 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 47 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
  - `mempalace mine . --wing freehand --agent codex --limit 3` processed note/MEMORY/SKILL.
  - `mempalace search --wing freehand --results 5 "resource_map_rejects_bound_coverage_without_command_entry"` returned MEMORY rank 1 and note rank 2.
  - `mempalace search --wing freehand --results 5 "bound coverage command-style verification entry"` returned note rank 1 and SKILL rank 2.
- not complete yet:
  - The full objective is not complete because `instruction_capability.admit_request_context` remains a real pending resource operation.
  - The resource map gate still does not automatically discover every undeclared Rust source edge; it only validates declared source-edge registry rows plus selected broad/precise shortcut gates.
  - The test-design gate now proves command-style coverage entries are present, but does not parse and execute each command listed in every bound coverage cell.
  - No business-code resource-owner refactor should be claimed until a specific resource operation is selected after this skeleton audit and verified against its mapped white-box/module/project tests.

# 2026-07-12 Source-edge registry code-binding gate

- audit finding:
  - `source_edge_registry` validated that registry rows matched mainline call-table row strings, but did not directly prove the registry's file and symbol bindings existed in source.
  - A stale mainline row and stale registry row could therefore mutually backfill each other while the resource map still looked source-bound.
- implementation:
  - `verify_resource_map` now splits each `source_edge_registry.file_path` and `source_edge_registry.symbol_path` with the same binding segment rules used by mainline call-table checks.
  - Each registry file must exist.
  - Each registry symbol must resolve in the listed file set.
  - Added red test `resource_map_rejects_source_edge_registry_missing_symbol`.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, and `.agents/skills/freehand-dev/SKILL.md`.
- validation:
  - `cargo test -p xtask resource_map_rejects_source_edge_registry_missing_symbol -- --nocapture` passed.
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed after `cargo fmt`.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 24 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 48 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This strengthens declared source-edge registry rows. It still does not discover every undeclared Rust resource edge across the whole repo automatically.

# 2026-07-12 Bound coverage command target gate

- audit finding:
  - The previous coverage gate required command-style text in each `bound` Resource Operation Test Coverage cell, but did not verify that repo-owned command targets existed.
  - A coverage cell could point to a non-existent cargo package, script, or Makefile target while still looking executable.
- implementation:
  - `verify_resource_map` now extracts backticked command entries from `bound` coverage cells.
  - `cargo ... -p <package>` and `cargo ... --package <package>` entries must reference cargo package names found in repo Cargo manifests.
  - `scripts/...` entries must reference existing script files.
  - `make <target>` entries must reference a target in the repo `Makefile`.
  - Added red test `resource_map_rejects_bound_coverage_unknown_cargo_package`.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, and `.agents/skills/freehand-dev/SKILL.md`.
- validation:
  - `cargo test -p xtask resource_map_rejects_bound_coverage_unknown_cargo_package -- --nocapture` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 25 focused resource-map tests.
  - `cargo fmt --check` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask -- --nocapture` passed 49 tests.
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - The gate validates command entrances and repo-owned targets; it still does not execute every command listed in every coverage row.
  - Whole-repo automatic discovery of all undeclared Rust resource edges remains future work.

# 2026-07-12 Pending resource operation closure contract gate

- audit finding:
  - `binding_status=pending` previously recorded a real gap but did not require a separate pending reason, closeout owner doc, or verification entrance.
  - This made a pending resource operation explicit, but still too easy to leave vague or permanent.
- implementation:
  - `verify_resource_map` now requires pending operation bindings to declare non-empty `pending_reason`, `pending_closure_doc`, and `pending_verification`.
  - `pending_closure_doc` must point to an existing repo file.
  - Production pending operation `instruction_capability.admit_request_context` now names its reason, design doc, and required closure verification.
  - Added red test `resource_map_rejects_pending_operation_missing_contract`.
  - Updated `docs/resource-maps/README.md`, `docs/architecture/dev-gates.md`, and `.agents/skills/freehand-dev/SKILL.md`.
- validation:
  - `cargo test -p xtask resource_map_rejects_pending_operation_missing_contract -- --nocapture` passed.
  - `jq empty docs/resource-maps/core.json` passed.
  - `cargo fmt --check` passed after `cargo fmt`.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 26 focused resource-map tests.
  - `cargo test -p xtask -- --nocapture` passed 50 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - `instruction_capability.admit_request_context` is still pending; this gate makes the gap owned and auditable, not closed.
  - Whole-repo automatic discovery of all undeclared Rust resource edges remains future work.

# 2026-07-12 Resource-center goal plan audit correction

- audit finding:
  - `docs/goals/resource-center-top-down-refactor-plan.md` still named stale path families `docs/mainlines/*.json` and `docs/test-designs/*.md`.
  - Current repo truth is `docs/mainline-calls/*.json` and `docs/testing/*.md`.
  - The plan also needed an explicit current-state audit so the next worker does not mistake the skeleton gate pass for completed business-code resource-owner refactor.
- implementation:
  - Corrected the stale source-truth paths in `docs/goals/resource-center-top-down-refactor-plan.md`.
  - Added `## Current Resource-Center Audit` with the current counts: 19 required core resources, 14 operation bindings, 13 bound operations, 1 pending operation, 21 relation rules, 7 forbidden direct relations, 5 source shortcut gates, and 2 precise source-edge gates.
  - Recorded that `instruction_capability.admit_request_context` remains the only pending operation and must not be treated as a bound source edge before typed context-planner admission is implemented and verified.
  - Recorded residual gaps: no whole-repo automatic discovery of undeclared Rust resource edges, and coverage command entries are validated for presence/targets but not executed by the gate.
- validation:
  - `jq empty docs/resource-maps/core.json` passed.
  - `git diff --check` passed.

# 2026-07-12 Instruction capability typed request-context admission

- audit finding:
  - `docs/resource-maps/core.json` had one remaining pending operation: `instruction_capability.admit_request_context`.
  - `crates/freehand-instructions` compiled deterministic AGENTS.md/skill manifests, but runtime live bridge did not consume the manifest into provider-visible request context.
  - `ContextSegmentKind` had no instruction capability variant, so using `DeveloperPolicy` would have hidden the resource edge instead of making it typed.
- implementation:
  - Added `ContextSegmentKind::InstructionCapability`.
  - `freehand-blocks::plan_context` now labels, orders, and validates instruction capability segments as session-stable/cacheable developer context.
  - `freehand-instructions` now owns `render_instruction_capability_context`, which renders compiled AGENTS.md and skill entries into typed instruction capability context content. Runtime/provider code does not scan authoring directories directly.
  - `freehand-runtime` now depends on `freehand-instructions`, compiles the instruction capability manifest from runtime home + cwd, renders it through the instruction owner, and admits it as `instruction-capability` / `ContextSegmentKind::InstructionCapability` before context planning.
  - Updated resource map: `instruction_capability.admit_request_context` changed from `pending` to `bound`; added `source_edge_registry` row `instruction.capability-loader#07` for `instruction_capability_segment`; added direct relation rule `instruction-capability-admits-request-context-direct`.
  - Updated instruction capability and context planner function maps, mainline JSON, test designs, generated wiki, design doc, goal plan, and local Freehand skill.
  - Updated stale live_bridge tests that assumed Master could execute file tools; current truth is Master has framework-only task/timer tools, while Worker owns file read/write tooling inside its `target_cwd`.
- validation:
  - `cargo test -p freehand-instructions -- --nocapture` passed 4 tests.
  - `cargo test -p freehand-blocks -- --nocapture` passed 45 tests.
  - `cargo test -p freehand-runtime live_bridge -- --nocapture` passed 41 tests.
  - `cargo check -p freehand-runtime` passed.
  - `cargo check -p xtask` passed.
  - `cargo test -p xtask resource_map_ -- --nocapture` passed 26 focused tests.
  - `cargo test -p xtask -- --nocapture` passed 50 tests.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `cargo fmt --check` passed.
  - `git diff --check` passed before this note append.
  - S-profile online proof on `127.0.0.1:4042`: `scripts/install-launchd.sh restartS` completed, health returned `ok`, `freehand-cliS adp-smoke` passed, and config stayed `provider=minimax`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`.
  - Online sample `freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample success` passed for session `cli-adp-sample-success-1783840670133249000`, turn `runtime-turn-279`, `rounds=1`, `tool_executions=0`, `schema_retries=0`, `provider_retries=0`.
  - Persisted online turn truth proved both `planned_context.ordered_segments` and `provider_payload.input_segments` contain segment `instruction-capability` with `kind=InstructionCapability`, `stability=SessionStable`, `cache_policy=Cacheable`, `role=Developer`, `source=instruction_capability`, and `<freehand_instruction_capability>` content.
  - `freehand-cliS adp-error-query` for the online session returned `count=0`.
  - S-profile fixture env grep returned 0 matches.
- remaining:
  - Current production resource map has 14 operation bindings and all are `bound`.
  - The full top-down resource-center objective still has residual risk: gates validate declared source edges plus configured shortcut gates, but do not automatically discover every undeclared Rust resource edge across the whole repo.
  - Coverage command entries are target-validated by gate, but the gate still does not execute every command listed in every coverage row.

# 2026-07-12 Resource-center top-down goal completion audit

- audit finding:
  - After closing `instruction_capability.admit_request_context`, `docs/resource-maps/core.json` has 19 required resources, 14 operation bindings, 14 bound operations, 0 pending operations, 22 relation rules, 7 forbidden direct relations, 5 source shortcut gates, 2 precise source-edge gates, and 14 source-edge registry rows.
  - `docs/goals/resource-center-top-down-refactor-plan.md` still had the pre-closeout relation-rule count as 21; actual resource map count is 22 after adding `instruction-capability-admits-request-context-direct`.
- implementation:
  - Corrected the goal plan audit count to 22 relation rules.
  - Added `## Requirement Completion Audit` to the goal plan with requirement-by-requirement current evidence and status.
  - The audit marks the original top-down resource-center objective complete with residual risks explicitly scoped to stronger future automation: whole-repo undeclared Rust resource-edge discovery and gate execution of every coverage-row command.
- validation pending:
  - rerun resource-map JSON parse, fmt, xtask check/tests, mainlines generate/check, gates check, diff check, and MemoryPalace mine/search after this note update.
  - `rg -n "docs/mainlines|docs/test-designs" docs/goals/resource-center-top-down-refactor-plan.md docs/resource-maps/README.md .agents/skills/freehand-dev/SKILL.md docs/architecture/dev-gates.md` returned no matches.

# 2026-07-12 Master lifecycle waiting and WebUI observer repair

- finding:
  - Completion guidance incorrectly told Master to use `claim="complete"` after dispatching Worker work or scheduling timers.
  - That allowed a user-facing Master turn to look completed even when the user objective still depended on future Worker review/close truth.
  - WebUI right inspector was still titled/debug-framed, and AgentBoard did not prioritize/click active Workers as lifecycle observation targets.
- implementation:
  - Added completion claim `waiting`, mapped to `TerminalStatus::ToolPending`, for durable async lifecycle waits after Worker dispatch/timer scheduling.
  - Runtime/reason accept `CompletionDecision::Waiting` as terminal turn truth with `ToolPending`; UI protocol projects it as `Lifecycle` / `running`, not `Final` / `completed`.
  - Runtime/tool guidance now says dispatch/heartbeat/timer/review pending are lifecycle progress, not final user-task completion.
  - WebUI lifecycle observer title/copy replace debug framing; AgentBoard sorts active Workers first, styles them distinctly, and clicking an active Worker opens the TaskBoard-parented temporary worker session while refreshing TaskHistory/WorkerControl.
  - Fixed the WebUI worker-control JS parse risk by ensuring only one `target` declaration remains.
  - Updated function maps, test designs, and local skill with the waiting lifecycle rule.
- validation:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `cargo test -p freehand-blocks completion -- --nocapture` passed 8 focused tests.
  - `cargo test -p freehand-ui-protocol tool_pending_terminal_projects_as_lifecycle_running_not_final_completed -- --nocapture` passed.
  - `cargo test -p freehand-server webui -- --nocapture` passed 3 focused tests.
  - `cargo check -p freehand-reason -p freehand-runtime -p freehand-ui-protocol` passed.
  - `cargo fmt --check`, `git diff --check`, `cargo run -p xtask -- mainlines check`, and `cargo run -p xtask -- gates check` passed.
  - S-profile `scripts/install-launchd.sh restartS` completed; `curl -4fsS http://127.0.0.1:4042/health` returned `ok`; `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
  - Served HTML/CSS/JS on `4042` include lifecycle observer, active Worker styling, resizers, `phase2SortedAgents`, `openWorkerTaskSession`, and `waiting lifecycle`.
  - Playwright DOM proof on `4042`: title `Task and Agent Lifecycle`, eyebrow `lifecycle observer`, copy names active Worker click behavior, both layout resizers present, grid `56px 254px 10px 866px 10px 244px`, AgentBoard container present.
  - Final S-profile config remained minimax/MiniMax-M3/api.minimaxi.com inline auth; fixture env grep returned 0 matches.
- remaining:
  - This did not run a new real-provider long Worker task to completion; it verified the fixed lifecycle semantics, WebUI projection, S-profile service, and ADP transport.
  - `output/` remains unrelated untracked work and was not touched.

# 2026-07-12 Xiaozhi lifecycle E2E attempt blocked by provider quota

- objective:
  - Use fixed session `webui-session-fixed-xiaozhi-lifecycle-e2e` on S-profile `4042` to submit the user's xiaozhi analysis prompt and prove full lifecycle: Master dispatch -> Worker execution/report -> Master review/close -> user summary with WebUI worker-session evidence.
- evidence:
  - S-profile config stayed `provider=minimax`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`.
  - Target repo exists at `/Users/fanzhang/Documents/github/xiaozhi-esp32-2.2.4`.
  - Fixed session query after submit: `webui-session-fixed-xiaozhi-lifecycle-e2e:1:failed`, turn `runtime-turn-283`.
  - Error-center for the fixed session: 10 provider rows, `anthropic_http_status_429`; first 9 `retry_same_step`, 10th `fail_turn`.
  - HTTP command failure payload: `已达到 Token Plan 用量上限：请升级 Token Plan 套餐或购买积分补充用量。 (2056)`.
  - WebUI screenshot saved to `output/xiaozhi-lifecycle-provider-429.png`, showing fixed session, submitted prompt, failed assistant terminal, lifecycle observer panel, TaskBoard/AgentBoard summary.
  - `/Users/fanzhang/Documents/github/xiaozhi-esp32-2.2.4/analysis/project_analysis.html` does not exist after the run.
- conclusion:
  - Full lifecycle objective is blocked before Worker execution by external provider quota.
  - Existing task `task-1783858124` remains only `TaskCreated,TaskWaitingAgent,TaskAssigned`; it has no `TaskResumed`, `TaskHeartbeat`, `TaskReviewSubmitted`, `TaskReviewApproved`, or `TaskClosed`.
  - Do not claim Worker delivered or Master summarized until provider quota is restored or Jason explicitly authorizes a different provider/fixture path.

# 2026-07-12 Provider-neutral OpenAI Responses wire proof

- objective:
  - Remove the live bridge's Anthropic-only assumption and keep provider-specific wire rendering/execution inside the selected provider adapter/executor.
  - Validate RCC `cc` OpenAI Responses on S-profile `4042` without touching release `4041`.
- implementation:
  - `freehand-provider-openai` now owns `OpenAiExecutor`, endpoint selection, HTTP/SSE execution, raw response/error/stream capture, Responses tool declarations, Responses `function_call` / `function_call_output` re-entry, Chat Completions tool declarations, and Chat Completions `assistant.tool_calls` / `tool` re-entry.
  - `freehand-runtime` now selects a provider-neutral `LiveProviderDriver` from config. Runtime maps Anthropic/messages, OpenAI/responses, and OpenAI/chat_completions descriptors, but does not build OpenAI wire bodies itself.
  - Function map and mainline call map were synced for `provider.openai-adapter`, `provider.reason-live-bridge`, and stale error-center naming.
- local validation:
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo test -p freehand-provider-openai -- --nocapture` passed 8 tests.
  - `cargo check -p freehand-runtime` passed.
  - `cargo test -p freehand-runtime live_bridge_maps_openai_protocols_to_provider_descriptor -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_rejects_unsupported_provider_selection -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_writes_provider_error_metadata_on_executor_failure -- --nocapture` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- online validation:
  - S-profile config after restart: `provider=cc`, `provider_type=openai`, `provider_protocol=responses`, `base_url_host=api.anyint.ai`, `default_model=gpt-5.5`, `auth_source=env`.
  - `scripts/install-launchd.sh restartS` and `scripts/install-launchd.sh restartWorkerS` completed service-scoped restarts; 4042 health returned `ok`; `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
  - Fixed proof task `provider-openai-tool-proof-fixed` reached `TaskCreated,TaskAssigned,TaskResumed,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskHeartbeat,TaskReviewSubmitted`.
  - Worker session `worker-task-provider-openai-tool-proof-fixed`, turn `worker-turn-exec-worker-worker-1783864675743871000-122-r4`, ended `TerminalStatus::Success`.
  - Provider raw ledgers under `~/.freehand/ledgers/providers/openai-compatible/worker/worker-task-provider-openai-tool-proof-fixed/` show OpenAI Responses returned tool schema names `complete_step,delete_range,edit_file,glob,grep,ls,multi_edit,read_file,todo_write,write_file`; model emitted `ls` calls in the first provider response, `read_file` in the second, `complete_step` in the third, and final completion in the fourth.
  - Worker final summary: first markdown heading read from `AGENTS.md` is `# Freehand Project AGENTS`.
  - `freehand-cliS adp-error-query --url ws://127.0.0.1:4042/adp --session worker-task-provider-openai-tool-proof-fixed --domain provider` returned `count=0`.
  - Fixture env grep for provider retry and master autonomy fixture keys returned 0 matches.
- remaining:
  - Master background runner did not approve/close the fixed proof task within the short observation window; do not claim full Master close for this proof.
  - Xiaozhi full E2E and `analysis/project_analysis.html` remain incomplete in this slice.
  - S-profile is intentionally left on RCC `cc/openai/responses` for continued Responses verification, not restored to Minimax.

# 2026-07-12 Submit failure observability and first-launch permission preflight

- user correction:
  - Worker execution errors are normal lifecycle facts. Correct flow is Worker returns error/block/interrupted truth to Master; Master continues processing from TaskBoard/EventInbox instead of treating worker failure as whole-system completion/failure.
  - Online verification must observe subagent/worker state through session/task/agent truth, not just wait on command receipts.
  - Install/first launch should request/check file permissions up front to avoid runtime task failures.
- implementation:
  - `RuntimeCommandDispatcher::prepare_live_submit_user_input` now returns explicit errors instead of swallowing selected-session/cwd resolution failures and falling back to non-live submit.
  - `finish_live_submit` now calls `restore_or_materialize_failed_live_submit`: if live bridge already persisted failed turn truth, restore it; if provider/protocol failed before recovery truth exists, persist a failed turn under the selected session and project it to `UiProtocolState`; if persistence is corrupt/unreadable, fail explicitly.
  - Added regression `live_dispatch_materializes_failed_turn_when_provider_fails_before_persistence`.
  - Added `scripts/freehand-file-permission-preflight.sh` and wired `scripts/install-launchd.sh` install/restart paths to run it before launchd bootstrap/restart. On macOS it checks runtime home, workdir, Documents, Desktop, Downloads, optional extra paths, writes `~/.freehand/state/file-permission-preflight.json`, opens Full Disk Access settings on denial, and fails by default unless `FREEHAND_FILE_PERMISSION_PREFLIGHT=warn` is explicitly set.
  - Updated runtime dispatch and foundation workspace function maps, mainline JSON, test designs, generated wiki, and local skill.
- validation:
  - `bash -n scripts/freehand-file-permission-preflight.sh` passed.
  - `bash -n scripts/install-launchd.sh` passed.
  - `FREEHAND_FILE_PERMISSION_PREFLIGHT=warn scripts/freehand-file-permission-preflight.sh` passed and wrote status ok.
  - `jq empty docs/mainline-calls/runtime.ui-command-dispatch.json docs/mainline-calls/foundation.workspace.json` passed.
  - `cargo test -p freehand-runtime live_dispatch_materializes_failed_turn_when_provider_fails_before_persistence -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_dispatch_failure_preserves_other_session_transcripts -- --nocapture` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p freehand-runtime` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
  - `scripts/install-launchd.sh restartS` passed with permission preflight ok and restarted `com.freehand.daemonS` on 4042.
  - `curl -4fsS http://127.0.0.1:4042/health` returned ok.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
  - S config stayed `provider=cc`, `provider_type=openai`, `provider_protocol=responses`, `base_url_host=api.anyint.ai`, `default_model=gpt-5.5`, `auth_source=env`.
  - fixture env grep for provider retry/master autonomy keys returned 0 matches.
  - Correct internally tagged ADP online submit to fixed session `online-fixed-observable-submit-proof` showed after 2s `QuerySessionTurns` already had `runtime-turn-287`, original user_text, no terminal yet; final query showed `turn_count=1`, `terminal_status=Success`, original user_text and terminal_text visible. This proves submit does not collapse into an empty session while provider runs.
  - Concurrent `QueryAgentBoard` showed worker truth: `worker state=blocked`, `current_task_id=task-1783787230`, `current_execution_id=exec-worker-worker-1783866078571451000-1266`, current_activity reason `worker live execution failed: provider live executor failed: openai_http_request_failed: error sending request for url (https://api.anyint.ai/openai/v1/responses)`.
- correction:
  - An earlier manual WebSocket proof used the wrong ADP envelope shape (`{"Command":...}`) and returned `invalid_adp_message: missing field kind`; that was not system evidence. Correct ADP JSON is internally tagged with `kind`.
  - Do not use receipt waiting as sole progress proof; always query session/task/agent truth in parallel.
- remaining:
  - `cargo test -p freehand-runtime live_dispatch -- --nocapture` has one pre-existing/adjacent red test `live_dispatch_projects_failed_tool_result_without_command_failure` due strict unknown-tool request text assertion; the two focused submit-failure tests passed. This was not fixed in this slice.
  - Worker task `task-1783787230` remains blocked on current provider request failure; this is observable task/agent truth, not a hidden empty-session state.

# 2026-07-12 Standard fixed-session ADP observability script

- correction:
  - User correctly pointed out that online testing should use a standard fixed script and observe subagent/worker state instead of waiting.
- implementation:
  - Added `scripts/verify-adp-fixed-session-observability-online.py`.
  - The script sends the correct internally tagged ADP command envelope (`kind=command`) to a fixed session, queries pending selected-session turns after a short delay, then waits for receipt or timeout and queries final selected-session turns plus TaskBoard/AgentBoard truth.
  - It outputs one JSON proof with `pending`, `receipt`, `final`, and worker/blocked task summaries.
  - Updated local Freehand skill and foundation workspace docs/mainline/test design to prefer this script for fixed-session ADP online submit validation.
- validation:
  - `python3 -m py_compile scripts/verify-adp-fixed-session-observability-online.py` passed.
  - `scripts/verify-adp-fixed-session-observability-online.py --url ws://127.0.0.1:4042/adp --session online-fixed-observability-standard` passed with `ok=true`.
  - Proof session `online-fixed-observability-standard`: pending had `runtime-turn-288`, original user_text, terminal_status=null; final had the same turn with `terminal_status=Success`; receipt was `reason_live_turn_completed rounds=1 schema_rejections=0 tool_executions=0`.
  - Same proof exposed Worker owner truth: `worker state=blocked`, `current_task_id=task-1783787230`, `current_execution_id=exec-worker-worker-1783866078571451000-1266`, reason `openai_http_request_failed` to `https://api.anyint.ai/openai/v1/responses`.

# 2026-07-12 Worker/Master observability closeout rerun and OpenAI provider failure classification

- objective:
  - Continue the active closeout goal from `/Users/fanzhang/.codex/attachments/ef38b4c5-a8ef-463a-9612-aaf6c24d9bba/pasted-text-1.txt`.
  - Preserve S-profile `127.0.0.1:4042`, fixed session validation, no release `4041`, no broad kill, no commit, no `output/` cleanup.
- gap found:
  - `worker_execution_error_is_retryable_system_failure` classified Anthropic provider/network failures as retryable but did not include OpenAI-compatible provider error codes.
  - Current S-profile uses `cc/openai/responses`; an `openai_http_request_failed` Worker provider failure could therefore be written as `TaskBlocked` instead of retryable `TaskInterrupted`.
- implementation:
  - Added OpenAI-compatible retryable provider/network codes to Worker classification: `openai_http_request_failed`, `openai_stream_read_failed`, and retryable `openai_http_status_*` values.
  - Left `openai_adapter_failed`, `openai_callback_failed`, and content/deliverable errors non-retryable so they still map to `TaskBlocked`.
  - Updated `production_worker_runner_provider_error_records_interrupted_and_requeues_same_task` to use the current `openai_http_request_failed` sample.
  - Added classifier coverage for Anthropic/OpenAI retryable codes and non-retryable OpenAI adapter/callback/content examples.
  - Updated `docs/testing/runtime.master-worker-loop.md`, regenerated wiki, and updated `.agents/skills/freehand-dev/SKILL.md`.
- local validation:
  - `bash -n scripts/freehand-file-permission-preflight.sh` passed.
  - `bash -n scripts/install-launchd.sh` passed.
  - `python3 -m py_compile scripts/verify-adp-fixed-session-observability-online.py` passed.
  - `FREEHAND_FILE_PERMISSION_PREFLIGHT=warn scripts/freehand-file-permission-preflight.sh` passed and wrote `~/.freehand/state/file-permission-preflight.json` with `status=ok`.
  - `jq empty docs/mainline-calls/runtime.ui-command-dispatch.json docs/mainline-calls/runtime.master-worker-loop.json docs/mainline-calls/foundation.workspace.json` passed.
  - `cargo test -p freehand-runtime live_dispatch_materializes_failed_turn_when_provider_fails_before_persistence -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_dispatch_failure_preserves_other_session_transcripts -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_worker_runner_provider_error_records_interrupted_and_requeues_same_task -- --nocapture` passed with OpenAI request-failure sample.
  - `cargo test -p freehand-runtime production_worker_runner_non_provider_execution_error_records_blocked_not_retryable -- --nocapture` passed.
  - `cargo test -p freehand-runtime worker_retryable_provider_error_classifier_covers_supported_provider_families -- --nocapture` passed.
  - `cargo fmt --check` passed.
  - `cargo check -p freehand-runtime` passed.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- online validation:
  - `scripts/install-launchd.sh restartS` completed service-scoped restart of `com.freehand.daemonS`; no broad kill used.
  - `scripts/install-launchd.sh restartWorkerS` completed service-scoped restart of `com.freehand.workerS` so Worker loaded the new classifier.
  - `curl -4fsS http://127.0.0.1:4042/health` returned `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` returned `adp_smoke_ok`.
  - S config remained current RCC truth: `provider=cc`, `provider_type=openai`, `provider_protocol=responses`, `base_url_host=api.anyint.ai`, `default_model=gpt-5.5`, `auth_source=env`.
  - `~/.freehand/state/file-permission-preflight.json` reported `status=ok`, `runtime_home=/Users/fanzhang/.freehand`, `workdir=/Users/fanzhang/.freehand`.
  - Fixed session script passed: `scripts/verify-adp-fixed-session-observability-online.py --url ws://127.0.0.1:4042/adp --session online-fixed-observability-standard` returned `ok=true`.
  - Fixed session proof: pending `turn_count=2`, current `runtime-turn-289` had original `user_text` and `terminal_status=null`; final still in the same session, `runtime-turn-289` reached `terminal_status=Success` with original `user_text`.
  - Receipt: `reason_live_turn_completed rounds=1 schema_rejections=0 tool_executions=0 restored_closed_turns=1`.
  - AgentBoard proof still exposes historical Worker failure truth: `worker state=blocked`, `current_task_id=task-1783787230`, `current_execution_id=exec-worker-worker-1783866078571451000-1266`, current activity reason `openai_http_request_failed` to `https://api.anyint.ai/openai/v1/responses`.
  - Fixture env grep for provider retry/master autonomy keys returned 0 matches.
- remaining:
  - Historical task `task-1783787230` remains blocked from pre-fix/current provider failure truth; it is observable, but this rerun did not mutate historical blocked truth into interrupted truth.
  - `output/` remains unrelated untracked work and was not touched.
  - No commit was made.

# 2026-07-12 Fixed-session online full validation rerun

- objective:
  - Clean top-level visible sessions, keep one fixed persisted session, and rerun S-profile `4042` provider retry plus normal Master/Worker E2E with active session/task/agent inspection instead of passive waiting.
- implementation:
  - `scripts/verify-provider-retry-online.sh` now validates the provider-retry sample session by direct session-turn query instead of requiring hidden sample sessions to appear in the global session list.
  - `scripts/verify-master-worker-autonomy-online.sh` now has a 90s health wait with launchd/log diagnostics and `FREEHAND_MASTER_AUTONOMY_LEAVE_SERVICES_STOPPED=1` for callers that need fixture config restored without immediately restarting production services.
  - `scripts/verify-normal-master-worker-e2e.sh` now calls autonomy with services-left-stopped, waits for health with diagnostics, sources S daemon env for seeded restart tasks, and prints task history plus AgentBoard observations while waiting for branch transitions.
- online validation:
  - Top-level ADP session list after cleanup stayed `sessions=1 ids=online-clean-full-validation:1:success`, turn `runtime-turn-290`.
  - Provider retry online passed: `provider_retry_online_ok session=cli-adp-sample-provider-retry-1783870115617254000 mock_attempts=10 session_status=Failed`; provider error-center rows were 9 `retry_same_step` and 1 `fail_turn` for `anthropic_http_status_500`.
  - Normal Master/Worker E2E passed: `normal_master_worker_e2e_ok url=ws://127.0.0.1:4042/adp`.
  - Autonomy success task `task-cli-master-autonomy-success-FHAUTO1783871236726229000` events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskExecutionRecorded,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`; no `TaskBlocked`.
  - Autonomy execution-error task `task-cli-master-autonomy-execution-error-FHAUTO1783871242940445000` immediate verify events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskExecutionRecorded,TaskBlocked`; no review/approve/close in the captured fixture proof.
  - Autonomy reject-retry task `task-cli-master-autonomy-reject-retry-FHAUTO1783871247523130000` events: `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskReviewSubmitted,TaskReviewRejected,TaskResumed,TaskHeartbeat,TaskExecutionRecovering,TaskReviewSubmitted,TaskReviewApproved,TaskClosed`.
  - Production rejected retry branch task `task-normal-rejected-1781783871279` reached `TaskReviewRejected -> TaskAssigned -> TaskResumed -> ... -> TaskReviewSubmitted -> TaskReviewApproved -> TaskClosed`, with AgentBoard showing `worker` on that same task.
  - Blocked decision branch task `task-normal-blocked-1781783871376` reached `TaskBlocked,TaskProgressed`, with AgentBoard showing `worker:blocked`.
  - Crash recovery branch task `task-normal-crash-1781783871411` stayed same-id and reached `TaskInterrupted -> TaskAssigned -> TaskResumed -> ... -> TaskReviewSubmitted -> TaskReviewApproved -> TaskClosed`; AgentBoard showed `worker` running the same task and later `waiting_review`.
  - Final S config restored to current RCC truth: `provider=cc`, `provider_type=openai`, `provider_protocol=responses`, `base_url_host=api.anyint.ai`, `default_model=gpt-5.5`, `auth_source=env`.
  - Final fixture env grep returned 0 matches for provider retry and master autonomy fixture keys.
- local validation:
  - `bash -n scripts/verify-master-worker-autonomy-online.sh` passed.
  - `bash -n scripts/verify-normal-master-worker-e2e.sh` passed.
  - `cargo test -p freehand-runtime live_bridge_fails_after_ten_provider_retries_with_error_code -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_worker_runner_provider_error_records_interrupted_and_requeues_same_task -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_worker_runner_non_provider_execution_error_records_blocked_not_retryable -- --nocapture` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- remaining:
  - `cargo test -p freehand-task execution_fact_interrupted_marks_task_retryable_without_blocked_truth -- --nocapture` could not produce fresh evidence in this run: repeated exec sessions hung with no cargo/rustc/test process visible, and were interrupted. Do not report it as newly passed.
  - Historical archived metadata rows remain because `DeleteSession` is non-destructive archive/delete-as-hidden; active top-level session list is clean.
  - Historical TaskBoard/AgentBoard rows remain observable owner truth and include old polluted autonomy fixture tasks; final evidence uses the latest immediate fixture verify lines and latest production branch task histories.
  - `output/` remains unrelated untracked work and was not touched.

# 2026-07-13 Cargo focused-test hang investigation

- finding:
  - The previously reported `freehand-task` focused-test "hang" was not a Rust test deadlock.
  - Re-running the exact test through `/opt/homebrew/bin/timeout` showed `RC=0`; the test itself ran in `0.01s` and passed.
  - Direct focused `cargo test -p freehand-task execution_fact_interrupted_marks_task_retryable_without_blocked_truth -- --nocapture` now returns normally.
  - The confusing cases were compile/no-output windows and pipeline/list probes. Process inspection must account for the local command wrapper (`rtk cargo`) plus `rustc`; a narrow process grep can miss the active compile child and falsely suggest "no cargo process".
- implementation:
  - Added `scripts/run-cargo-test-with-evidence.sh`.
  - The script wraps `cargo test` with a bounded timeout, writes stdout/stderr logs under `/tmp` by default, prints status/log paths/captured output, and returns the real cargo/timeout exit code.
  - Updated `.agents/skills/freehand-dev/SKILL.md` to use the evidence script for focused cargo tests that appear to hang or emit no output, and to avoid narrow process-grep conclusions.
- validation:
  - `bash -n scripts/run-cargo-test-with-evidence.sh` passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-task execution_fact_interrupted_marks_task_retryable_without_blocked_truth -- --nocapture` passed with `status=0`; stdout showed `1 passed; 0 failed; 46 filtered out`; stderr showed `Finished test profile` and the `freehand_task` unit-test binary path.

# 2026-07-13 Master three-worker parent-session lifecycle proof

- user expectation checked:
  - Expected flow is one user-visible Master session dispatching three Worker tasks, Workers returning results, Master reviewing all three, and then Master presenting one final user-facing summary.
  - Previous `scripts/verify-normal-master-worker-e2e.sh` did not prove this full parent-session flow; it proved separate lifecycle branches.
- implementation:
  - Added a runtime Master completion gate in `crates/freehand-runtime/src/lib.rs`: user-session Master `claim="complete"` is rejected while any Task Center child task with the same `parent_session_id` is not `Closed`.
  - Added regression `live_master_rejects_complete_while_parent_child_task_open`.
  - Added `scripts/verify-master-three-worker-e2e-online.sh` and switched its stable fixed session to `online-master-three-worker-e2e-current` because the earlier fixed session `online-master-three-worker-e2e` contains an old persisted active/tool-running turn and current rollback rejects active-turn sessions.
  - Updated runtime master-worker function map, test design, and local skill to record that parent/child lifecycle closure is runtime-gated, not prompt-only.
- local validation:
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime live_master_rejects_complete_while_parent_child_task_open -- --nocapture` passed.
  - `cargo test -p freehand-runtime master_lifecycle_closes_in_same_round_as_target_task_mutation -- --nocapture` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- online evidence:
  - S-profile restored after each script run: `provider=cc`, `provider_type=openai`, `provider_protocol=responses`, `base_url_host=api.anyint.ai`, `default_model=gpt-5.5`, `auth_source=env`; fixture env grep returned 0 matches.
  - First old-session run against `online-master-three-worker-e2e` exposed the fixed-session cleanup gap: no fixture requests and no new turn because the fixed session still had old active/tool-running truth.
  - Stable current-session run `online-master-three-worker-e2e-current` created three child tasks under the same parent session with stamp `1781783908345`.
  - Runtime rejected premature Master final completion while children were open; parent terminal became `Blocked`, not `Success`, with rejection message naming open child tasks `task-three-worker-1781783908345-beta:Running` and `task-three-worker-1781783908345-gamma:Assigned`.
  - Child tasks eventually reached:
    - alpha: `TaskCreated, TaskWaitingAgent, TaskAssigned, TaskResumed, TaskHeartbeat..., TaskReviewSubmitted, TaskReviewApproved, TaskClosed`
    - beta: `TaskCreated, TaskWaitingAgent, TaskAssigned, TaskResumed, TaskHeartbeat..., TaskReviewSubmitted, TaskReviewApproved, TaskClosed`
    - gamma: `TaskCreated, TaskWaitingAgent, TaskAssigned, TaskResumed, TaskHeartbeat...`, then later `review_submitted`, then `closed`.
- remaining product gap:
  - The premature-success bug is fixed, but the full expected flow is still not closed.
  - After all three child tasks closed, the parent user session did not automatically resume and synthesize the final user-facing answer.
  - Missing owner behavior is a parent-session aggregator/resume path: when all child tasks for a parent session close, runtime should trigger a Master follow-up turn in the same persisted parent session, inspect child review summaries, and write the final answer.

# 2026-07-13 Parent-session aggregator/resume gap source trace

- status:
  - No code changed in this trace; evidence is source/docs inspection only.
  - `TaskClosed` already projects into Master-visible EventInbox as `task_closed`.
  - `ProductionMasterRunner::handle_event` only treats `review_ready`, `execution_blocked`, and `execution_interrupted` as actionable, so `task_closed` is consumed as non-actionable and cannot trigger parent aggregation.
  - `master_parent_session_completion_rejection` correctly blocks user-session Master `claim="complete"` while same-`parent_session_id` child tasks remain open, but it is only a rejection gate, not a resume trigger.
  - `ReasonPersistence::restore_turn_snapshots_for_ui` already exists, and `UiProtocolState::replace_session_turn_projections` can refresh one session transcript; a background parent aggregation turn still needs query/projection refresh so ADP/WebUI sees the persisted follow-up turn.
- fix direction:
  - Add `task_closed` handling in `runtime.master-worker-loop`.
  - For the closed task's `parent.session_id`, query all terminal-included TaskBoard children with the same parent session.
  - If any child is not `Closed`, no-op and advance cursor.
  - If all required children are `Closed`, collect latest `TaskReviewSubmitted` summary/deliverables/evidence from each child history and run a follow-up Master turn in the same parent session.
  - Add durable idempotency keyed by `parent_session_id` plus closed child task set/version to avoid duplicate summaries on cursor replay/restart.
  - Add runtime/UI query projection coverage proving `QuerySessionTurns` sees the background-persisted parent aggregation turn.

# 2026-07-13 Parent-session aggregator/resume implementation and online closure

- implementation:
  - `ProductionMasterRunner::handle_event` now routes `task_closed` to `handle_parent_task_closed`.
  - The handler requires `closed_task.parent.session_id`, queries terminal-included Task Center truth, filters all children with the same parent session, and no-ops while any sibling is not `Closed`.
  - Closed children are deterministically sorted; each child contributes only its latest `TaskReviewSubmitted` `summary`, `deliverables`, and `evidence`.
  - The follow-up Master turn runs through the ordinary live reason path in the original persisted parent session. Its internal prompt forbids raw Worker transcripts and additional task lifecycle mutations.
  - Aggregation identity uses the parent session plus sorted child ids and each child `last_event_seq`, rendered as a stable `<freehand_parent_aggregation id="...">` marker.
  - Restart/crash idempotency checks successful reason persistence for that marker before provider execution. This covers the window where the reason turn closed successfully but the Master cursor or `completed_parent_aggregations` state was not persisted.
  - Runtime-backed `QuerySessionTurns` now restores current snapshots from `ReasonPersistence` and replaces the selected session projection, so a background-created aggregation turn becomes visible without daemon restart.
  - UI user-text projection suppresses internal parent-aggregation prompts; the final assistant summary remains visible.
- tests and maps:
  - Added positive `production_master_runner_aggregates_closed_children_in_parent_session`.
  - Added negative `production_master_runner_does_not_aggregate_while_sibling_open`.
  - Added replay/restart `production_master_runner_parent_aggregation_is_idempotent_on_event_replay`.
  - Added runtime/query projection `runtime_query_session_turns_restores_background_parent_aggregation`.
  - Retained the earlier negative gate `live_master_rejects_complete_while_parent_child_task_open`.
  - Updated runtime Master/Worker and UI command-dispatch function maps, test designs, mainline-call manifests, and generated wiki.
- online verifier:
  - `scripts/verify-master-three-worker-e2e-online.sh` now creates an isolated temporary `HOME/.freehand`, uses isolated port `4142`, switches both Master and Worker configs to the deterministic fixture before submission, records explicit PIDs, and stops only those PIDs.
  - Isolation is required because global `~/.freehand` contains an unrelated old EventInbox event for missing task `task-three-worker-1781783906334-beta`; no global Task Center/EventInbox truth was cleaned, skipped, or rewritten.
  - Acceptance checks the original parent prompt, three same-parent tasks, full `TaskCreated -> TaskAssigned -> TaskResumed -> TaskReviewSubmitted -> TaskReviewApproved -> TaskClosed` lifecycle, a later successful aggregation turn, all three result tokens, hidden synthetic prompt text, and restart non-duplication.
- verified online evidence:
  - Session: `online-master-three-worker-aggregator-1783916209`.
  - Tasks: `task-three-worker-1781783916209-alpha`, `task-three-worker-1781783916209-beta`, `task-three-worker-1781783916209-gamma`.
  - Every task was `closed`, shared the parent session, and contained `TaskCreated`, `TaskWaitingAgent`, `TaskAssigned`, `TaskResumed`, `TaskHeartbeat`, `TaskReviewSubmitted`, `TaskReviewApproved`, and `TaskClosed`.
  - Parent aggregation: `turn_id=runtime-turn-2`, `terminal_status=Success`, `turn_count=11`.
  - Final visible text contained `worker_result_alpha=1781783916209`, `worker_result_beta=1781783916209`, and `worker_result_gamma=1781783916209`.
  - Original parent submit receipt had `rounds=10 schema_rejections=3 tool_executions=7`, proving premature completion was rejected before the automatic follow-up succeeded.
  - Restart proof: `aggregation_count=1`, `aggregation_turn_id=runtime-turn-2`, `restart_idempotent=true`, `turn_count=11`.
- local evidence already obtained:
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_master_runner_ -- --nocapture`: 14 passed.
  - Focused tests `live_master_rejects_complete_while_parent_child_task_open`, `runtime_query_session_turns_restores_background_parent_aggregation`, and `live_bootstrap_restores_all_persisted_sessions_into_ui_state` passed.
  - `cargo clippy -p freehand-runtime --all-targets -- -D warnings`, `cargo fmt --check`, `cargo run -p xtask -- mainlines generate/check`, `cargo run -p xtask -- gates check`, `git diff --check`, JSON parse checks, and verifier shell syntax passed.
- known regression status:
  - Full `cargo test -p freehand-runtime -- --nocapture --test-threads=1` is not green: 132 passed and 12 failed in pre-existing/adjacent live-tool, checkpoint rewind, autonomy boundary, and task-list publication tests.
  - Aggregator-focused coverage passed. Do not claim the entire `freehand-runtime` package suite is green.
  - No commit was made. Unrelated dirty/untracked work, including `output/`, remains untouched.

# 2026-07-13 Parent-session aggregator/resume continuation verification

- restored context:
  - MemoryPalace search for `parent session aggregator resume child closed master worker` returned the existing gap and implementation records.
  - Current worktree already contains an uncommitted parent-session aggregator/resume implementation plus unrelated dirty/untracked files.
- local validation rerun:
  - `FREEHAND_CARGO_TEST_TIMEOUT_SECONDS=600 scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_master_runner_ -- --nocapture` passed: 14 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime live_master_rejects_complete_while_parent_child_task_open -- --nocapture` passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime runtime_query_session_turns_restores_background_parent_aggregation -- --nocapture` passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime live_bootstrap_restores_all_persisted_sessions_into_ui_state -- --nocapture` passed.
  - `cargo fmt --check` passed.
  - `cargo clippy -p freehand-runtime --all-targets -- -D warnings` passed.
  - `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- full package status:
  - `FREEHAND_CARGO_TEST_TIMEOUT_SECONDS=1200 scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime -- --nocapture --test-threads=1` remains non-green with 132 passed and 12 failed.
  - The 12 failures are in existing/adjacent live-tool, checkpoint rewind, autonomy boundary, and task-list publication tests; parent aggregation tests passed in the same full run.
- online validation rerun:
  - `scripts/verify-master-three-worker-e2e-online.sh` passed in isolated HOME/port 4142 with deterministic fixture.
  - Session: `online-master-three-worker-aggregator-1783918267`.
  - Tasks: `task-three-worker-1781783918267-alpha`, `task-three-worker-1781783918267-beta`, `task-three-worker-1781783918267-gamma`.
  - Each task reached `TaskCreated, TaskWaitingAgent, TaskAssigned, TaskResumed, TaskHeartbeat, TaskReviewSubmitted, TaskReviewApproved, TaskClosed`.
  - Parent aggregation turn: `runtime-turn-2`, `terminal_status=Success`, `turn_count=11`.
  - Final visible text contained `worker_result_alpha=1781783918267`, `worker_result_beta=1781783918267`, and `worker_result_gamma=1781783918267`.
  - Original submit receipt: `reason_live_turn_completed rounds=10 schema_rejections=3 tool_executions=7 restored_closed_turns=0`.
  - Restart idempotency proof: `aggregation_count=1`, `aggregation_turn_id=runtime-turn-2`, `restart_idempotent=true`.
- remaining:
  - No commit made.
  - Unrelated dirty/untracked work, including `output/`, remains untouched.

# 2026-07-13 User correction: parent evaluation is not result aggregation

- corrected objective:
  - Closing all current Worker child tasks is not evidence that the overall user
    goal is complete.
  - The all-children-closed event must resume Master evaluation in the original
    parent session.
  - Master must compare original user objective history, decomposed child
    goals/deliverables/acceptance, and accepted Worker review results.
  - The decision is one of: reject/rework before child close, create correction
    or improvement work, create newly discovered next-round work, record an
    explicit external blocker, or claim final completion only when the overall
    objective is verified complete.
- identified design error:
  - The current `<freehand_parent_aggregation>` prompt instructs the Master to
    synthesize a final answer and forbids task lifecycle mutations.
  - That design can summarize accepted work without proving the total goal and
    therefore is not a complete Master/Worker autonomy loop.
- required repair:
  - Replace aggregation semantics with an idempotent parent evaluation loop.
  - Include parent objective truth and full child task acceptance semantics in
    the evaluation input.
  - Permit next-round task creation/assignment.
  - Add an online proof where the first completed child set causes a new
    improvement task, and only the later evaluation reaches final success.

# 2026-07-13 Parent-session evaluation loop implementation and closure

- implementation:
  - Replaced `ParentAggregated` and `<freehand_parent_aggregation>` runtime
    semantics with `ParentEvaluated` and `<freehand_parent_evaluation>`.
  - Parent evaluation input now contains deduplicated root user objective turns
    only; repair rounds such as `runtime-turn-N-r2` are excluded.
  - Each completed child contributes its task `content`, `goal`, required
    `deliverables`, `acceptance`, and latest accepted `TaskReviewSubmitted`
    summary/deliverables/evidence.
  - The evaluation prompt explicitly requires comparing total objective and
    decomposed task truth, permits correction/improvement/new task creation,
    permits an explicit blocker, and forbids final `claim="complete"` unless the
    overall objective is verified complete.
  - Parent evaluation idempotency is keyed by parent session plus sorted child
    ids/event versions. Persisted `Success`, `ToolPending`, or `Blocked`
    evaluation turns count as durable decisions because they may already have
    created next-round tasks; failed/interrupted/cancelled evaluations retry.
  - `QuerySessionTurns` hides all `<freehand_parent_...>` synthetic prompts and
    restores the evaluation/final assistant result from reason persistence.
- tests:
  - `production_master_runner_` passed 16/16.
  - Coverage includes overall-goal input, internal repair-text exclusion,
    missing parent goal explicit failure, open sibling no-op, next-round task
    creation, and ToolPending replay/restart idempotency.
  - `runtime_query_session_turns_restores_background_parent_evaluation` passed.
  - `cargo fmt --check`, `cargo clippy -p freehand-runtime --all-targets -- -D warnings`,
    `xtask mainlines generate/check`, `xtask gates check`, and
    `git diff --check` passed.
  - Full `freehand-runtime` run: 134 passed / 12 failed. The same adjacent
    live-tool/checkpoint/autonomy/task-list tests remain failing; no new parent
    evaluation failure appeared.
- online iteration:
  - First corrected verifier run used a stale `target/debug/freehand-daemon`;
    persisted prompt evidence still showed `<freehand_parent_aggregation>`.
    Rebuilding the exact daemon binary fixed that validation error.
  - The next run exposed fixture truth pollution: the fixture treated the
    expected integration token in task input as completed Worker output. The
    runtime completion gate correctly rejected final completion while the
    integration task remained open. The fixture now reads only
    `review_summary` truth.
- final online proof:
  - Session: `online-master-three-worker-evaluation-1783921598`.
  - Initial tasks: alpha, beta, gamma.
  - Beta history proved quality review and rework:
    `TaskReviewSubmitted -> TaskReviewRejected -> TaskAssigned -> TaskResumed -> TaskReviewSubmitted -> TaskReviewApproved -> TaskClosed`.
  - After the first accepted child set closed, parent evaluation created and
    assigned `task-three-worker-1781783921598-integration`.
  - Integration reached full review/approve/close lifecycle.
  - A second parent evaluation produced final `runtime-turn-3` only after all
    four accepted results satisfied the overall goal.
  - Final text contained alpha, corrected beta, gamma, and integration result
    tokens.
  - Restart proof: `final_evaluation_count=1`,
    `final_evaluation_turn_id=runtime-turn-3`, `restart_idempotent=true`,
    `turn_count=16`.
- remaining:
  - No commit made because the worktree still contains broad pre-existing
    dirty/untracked changes interleaved with this runtime batch.
  - `output/` and unrelated dirty files remain untouched.

# 2026-07-13 Multi-agent dashboard static WebUI prototype

- scope:
  - Research and produce a review-only static WebUI design.
  - Do not modify existing Freehand WebUI assets, UI protocol, or runtime.
- research direction:
  - Codex: project/session switching, parallel task supervision, and reviewable
    work progress.
  - Claude Code Remote Control: mobile as a remote view/control surface for a
    computer-hosted session.
  - OpenMinis: phone-first agent/settings information architecture only; do not
    copy phone-local storage/mount semantics.
  - Manus: task/subtask and autonomous execution status cues.
- design decision:
  - Desktop keeps a Codex-like left session rail and conversation canvas.
  - The middle-top dashboard shows total goal, Master phase, Worker task/review
    state, and the decision queue.
  - Master semantics are quality evaluation, reject/rework, next-round task,
    blocker, or overall completion; Worker results are not treated as a final
    aggregation.
  - Tablet portrait and phone use a session drawer, compact agent status strip,
    and detailed agent bottom sheet.
- artifact:
  - `docs/prototypes/freehand-agent-dashboard/index.html`
  - Single-file offline HTML/CSS/JS with explicit mock-data labeling and seven
    lifecycle state scenarios.
- verification:
  - Playwright rendered `1440x900`, `834x1112`, and `390x844`.
  - No horizontal overflow.
  - Desktop rail/dashboard visibility and portrait drawer/status-strip layout
    passed.
  - Drawer open/close, sheet open, close button, scrim close, seven state
    transitions, three Worker cards, decision rows, and mobile composer
    clearance passed.
  - Screenshot visual inspection passed for desktop, phone, phone drawer, and
    phone agent sheet.
- boundary:
  - Existing `apps/freehand-server/assets/webui.js`, `webui.css`, protocol, and
    runtime were not modified by this design task.

# 2026-07-13 Multi-agent dashboard visual confirmation

- user feedback:
  - Original prototype was functionally acceptable but slightly too plain and
    warm-toned.
  - Preferred visual direction is black/white/gray with only a small amount of
    blue or green accent.
  - Mobile implementation should follow this confirmed prototype direction.
- update:
  - Adjusted `docs/prototypes/freehand-agent-dashboard/index.html` palette to
    black/white/gray base.
  - Kept blue for active/running/evaluation accents and green for accepted/OK.
  - Converted rework/review/neutral statuses to gray instead of loud warm/red
    colors.
  - Structure and mobile layout were unchanged.
- verification:
  - Playwright smoke passed at `1440x900`, `834x1112`, and `390x844`.
  - Verified no horizontal overflow, three Worker cards, `Master reviewing`
    default phase, and hidden closed sheet.
  - Visual screenshots reviewed for desktop and phone after palette change.
- product direction:
  - Future mobile WebUI implementation should use this prototype as the target
    layout and visual baseline, but must still bind to owner-backed projections
    and run normal WebUI/mobile online gates when implementation begins.

# 2026-07-13 Production mobile Agent Dashboard closeout

- owner/scope:
  - `app.webui-smoke`
  - production WebUI shell/JS/CSS/tests/verifier only
  - no runtime/provider/protocol truth changes
- implementation:
  - phone/tall-phone/tablet portrait keep conversation-first layout
  - compact top Agent strip opens a bottom sheet containing Master decision,
    Worker tasks, Agents, review history, and Worker control
  - session navigation remains a left drawer with one expandable Master agent
    group; persisted parent sessions contain TaskBoard-parented temporary Worker
    child rows
  - drawer and Agent sheet are mutually exclusive
  - black/white/gray base with minimal blue active/evaluation and green accepted
    accents
  - mobile dashboard derives presentation only from TaskBoard, AgentBoard,
    EventInbox, TaskHistory, WorkerControl, and selected-session turn truth
  - all Worker children closed without selected parent Success renders
    `Awaiting Master evaluation`, not `Goal complete`
- online gaps found and fixed:
  - New conversation previously existed only as a browser draft, so turns were
    queryable while `QuerySessionList` returned zero sessions after reload;
    `startNewConversation` now sends protocol-owned `CreateSession`, refreshes
    session list, and refreshes the selected transcript
  - mobile drawer CSS/verifier expected an agent -> sessions hierarchy while JS
    emitted only flat parent wrappers; `renderSessionAgentGroup` now owns the
    Master group presentation without changing session truth
  - verifier now recognizes blocked/failed/cancelled/interrupted terminal turns,
    uses a genuinely scrollable portrait viewport for scroll-lock proof, checks
    left-edge styling only on cards actually emitted by the live provider, and
    waits for mobile sheet geometry to settle before screenshots
- verification:
  - `node --check apps/freehand-server/assets/webui.js`
  - `node --check scripts/webui_verify_online.mjs`
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-server -- --nocapture`
    passed 13 tests
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
  - S-profile health and `freehand-cliS adp-smoke` passed
  - final browser/ADP proof:
    `artifacts/webui-online/20260713-verify-4042-1783933067766/summary.json`
  - all summary checks are true, including mobile strip/sheet, close/scrim,
    drawer mutual exclusion, state preservation, service-truth matching,
    no raw internal chrome, closed-child evaluation semantics, tablet portrait,
    desktop regression, drawer swipe/hierarchy, and scroll-lock proof
  - visually reviewed final screenshots:
    `19-mobile-session-drawer-open-swipe.png`,
    `32-mobile-agent-dashboard-main.png`,
    `33-mobile-agent-dashboard-sheet.png`,
    `37-tablet-agent-dashboard-sheet.png`,
    `38-desktop-agent-dashboard-regression.png`
- restored runtime profile:
  - provider `cc`
  - provider type `openai`
  - protocol `responses`
  - host `api.anyint.ai`
  - model `gpt-5.5`
  - auth source `env`
  - verifier credential env marker absent
# 2026-07-13 WebUI lifecycle current-session scope fix

- trigger:
  - Android/WebUI phone surface showed `Blocked · 144 task(s) · 31 blocked`
    after user cleared sessions.
  - Root cause was presentation-layer fallback: selected session with no child
    tasks fell back from parent-scoped TaskBoard to global TaskBoard history.
- implementation:
  - Added current-session lifecycle selectors in `apps/freehand-server/assets/webui.js`.
  - TaskBoard counts/cards, AgentBoard rows, EventInbox rows, TaskHistory target,
    WorkerControl target, and mobile Agent Dashboard now scope to selected
    parent session via TaskBoard `parent_session_id` / task ids.
  - Switching selected sessions clears cached task history and worker control.
  - Updated app.webui-smoke function map, test design, and asset smoke locks.
- evidence:
  - Local: `node --check apps/freehand-server/assets/webui.js`.
  - Local: `cargo test -p freehand-server webui_smoke_renders_shell_and_asset_routes -- --nocapture`.
  - Local: `cargo run -p xtask -- mainlines check`.
  - Local: `cargo run -p xtask -- gates check`.
  - Local: `git diff --check`.
  - S daemon rebuilt/restarted via `scripts/install-symlink.sh` and `scripts/install-launchd.sh restartS`; health returned `ok`.
  - Online browser artifact `artifacts/webui-online/20260713-verify-4042-1783939658253` shows selected session `webui-session-20260713104810-b82fe0ad` with `0 current task(s)`, `0 current agent(s)`, `0 current event(s)`, zero cards, and empty selected-session lifecycle panel even though global Task Center still has 144 historical tasks.
- remaining:
  - Full `scripts/verify-webui-online.sh` still failed later on `timeout waiting for phase2 projection dashboard`; the artifact before failure proves this specific current-session lifecycle scope fix, but the full WebUI online gate is not green.
  - Android true-device final screenshot is blocked by device keyguard/dozing; ADB cannot dismiss the security lock without manual unlock.

# 2026-07-13 WebUI selected-session scope verifier follow-up

- trigger:
  - `scripts/verify-webui-online.sh` still failed after current-session scope fix.
- root causes found:
  - verifier second-turn diagnostic prompt still asked Master to call `read_file`, but current Master live tool surface is framework-only `task`/`timer`.
  - verifier progress wait treated a visible selected pending turn with `dispatching` status as no progress when no `[data-live=true]` card existed yet.
  - `scripts/install-launchd.sh restartS` used a fixed 30s health wait; launchd restart could become healthy shortly after the script timed out.
  - current S-profile provider `cc` / `api.anyint.ai` / `gpt-5.5` returned `openai_http_status_402` insufficient credits, blocking full live provider terminal proof.
- implementation:
  - replaced WebUI hidden/online failure sample with Master-safe `task` query against `definitely-missing-freehand-task`.
  - expanded verifier progress detection to accept selected visible prompt + dispatching state.
  - made launchd health wait configurable and defaulted to 60s.
  - removed leaked dummy `FREEHAND_WEBUI_VERIFY_CREDENTIAL` from `~/.freehand/daemonS.env` and restarted S.
- evidence:
  - `node --check apps/freehand-server/assets/webui.js`
  - `node --check scripts/webui_verify_online.mjs`
  - `cargo test -p freehand-server webui_smoke_renders_shell_and_asset_routes -- --nocapture`
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
  - `scripts/install-launchd.sh restartS` returned 0 and `curl http://127.0.0.1:4042/health` returned `ok`.
  - latest online artifact before provider terminal wait failure: `artifacts/webui-online/20260713-verify-4042-1783943684651/03-first-materialized.json` shows selected session `webui-session-20260713115517-dbb8eb42` with `0 current task(s)`, `0 current agent(s)`, `0 current event(s)`.
- remaining:
  - full `scripts/verify-webui-online.sh` is still not green because the live provider returned 402 insufficient credits before first terminal; this is not a UI lifecycle-scope failure.
  - no historical Task Center files were deleted; clearing sessions is separate from deleting global task ledgers.

# 2026-07-13 WebUI online gate rerun with minimax

- action:
  - Temporarily switched S-profile agents.master/agents.worker provider from `cc` to `minimax` only for the online verifier run.
  - Ran `scripts/verify-webui-online.sh` against `http://127.0.0.1:4042/`.
  - Restored S-profile back to `cc` after the run and restarted S.
- evidence:
  - verifier exited 0.
  - summary artifact: `artifacts/webui-online/20260713-verify-4042-1783944723612/summary.json`.
  - all summary checks were true; failed check set was `{}`.
  - selected session: `webui-session-20260713121233-ebc85775`.
  - desktop lifecycle proof: `0 current task(s) · 0 blocked · 0 review · 0 stale`; `0 current agent(s) · 0 active`; `0 current event(s) · updated`.
  - mobile Agent Dashboard proof: `0 current task(s) · 0 blocked · 0 review · 0 stale`; `0 agent(s) · 0 active`.
  - after restore, `~/.freehand/config.toml` has agents.master/agents.worker provider `cc`, `api.anyint.ai`, `gpt-5.5`, OpenAI Responses; health `http://127.0.0.1:4042/health` returned `ok`.

# 2026-07-13 WebUI path-leading input observability fix

- trigger:
  - User submitted a normal task beginning with `/Volumes/extension/code/freehand ...` from WebUI.
  - UI showed `unknown slash command: /Volumes/...`; the request never rendered as submitted work or progress.
- root cause:
  - `runSlashCommand` treated any input starting with `/` as a slash command.
  - Absolute macOS paths are valid user task text and must not be intercepted unless the first token exactly matches a known command.
  - After ADP submit timeout, command-status showed unknown dispatch, but top turn-status could remain `dispatching...` because the catch path did not force `renderTurnMeta`.
- implementation:
  - `apps/freehand-server/assets/webui.js` now only intercepts exact known slash commands.
  - Unknown slash-leading text falls through to normal submit so the submitted user text is rendered immediately.
  - Pending submit timeout/error sets both command-status and turn-status to `dispatch status unknown · refresh needed`, keeps the user input visible, and suppresses continued live lifecycle styling for that pending card.
- evidence:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - S-profile rebuilt/restarted with `scripts/install-launchd.sh restartS`; health returned `ok`.
  - Online headless Chrome proof: `artifacts/webui-online/slash-path-observable-1783947943516/state.json` shows pending `promptVisible=true`, `unknownSlashVisible=false`, and after timeout `commandStatus=dispatch status unknown · refresh needed`, `turnStatus=dispatch status unknown · refresh needed`.
  - Screenshots: `artifacts/webui-online/slash-path-observable-1783947943516/01-pending-visible.png` and `02-timeout-visible.png`.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-server webui_smoke_renders_shell_and_asset_routes -- --nocapture` passed.
  - `cargo fmt --check` and `git diff --check` passed.
- remaining:
  - Provider main/backup route work is still incomplete; current live requests may still time out/keep runtime work pending until provider failover is fully closed.

# 2026-07-13 Provider primary/backup and WebUI observability closeout

- configuration truth:
  - `AgentConfig` and selected config now support `fallback_provider`.
  - S profile uses `provider = "cc"` and `fallback_provider = "minimax"` for both Master and Worker.
  - Primary and fallback must exist, be enabled, differ, and resolve protocol/model/base URL/auth independently.
- runtime behavior:
  - Non-stream HTTP status/network/stream-read failures are failover eligible.
  - Retryable primary failures exhaust the existing primary retry policy before one fallback switch; eligible non-retryable HTTP failures such as 402 switch immediately.
  - Adapter, invalid-config, and callback failures do not switch.
  - Fallback reconstructs the provider descriptor/driver/request while preserving tools, tool choice, and tool exchanges.
  - Successful fallback persists fallback model and metadata:
    `provider.route=fallback`, `provider.failover_from`, `provider.failover_to`,
    and `provider.failover_error_code`.
  - Error Center uses recoverable `failover_provider`; it does not write
    `fail_turn` for the primary error when fallback succeeds.
  - Fallback exhaustion materializes one failed turn.
- WebUI:
  - Full online verifier passed:
    `artifacts/webui-online/20260713-verify-4042-1783952632179/summary.json`.
  - Session: `webui-session-20260713142425-a0eb7daf`.
  - All submit/progress/refresh/terminal, Settings, secret-leakage,
    selected-session lifecycle, mobile drawer/sheet, tablet, and desktop checks
    were true.
  - Settings valid-save now edits the current primary provider. It does not
    change primary to the configured fallback, because config truth correctly
    rejects identical primary/fallback routes.
- controlled online failover proof:
  - Production S daemon and ADP were used.
  - Provider id `cc` was temporarily routed to a local fixture returning exactly
    one OpenAI Responses 402; fallback remained the real `minimax` endpoint.
  - Session: `cli-adp-sample-success-1783953723678094000`.
  - Turn: `runtime-turn-347`.
  - Persisted result: `model=MiniMax-M3`, `terminal=Success`.
  - Metadata: `error.code=openai_http_status_402`,
    `error.recovery_action=failover_provider`, `provider.route=fallback`,
    `provider.failover_from=cc`, `provider.failover_to=minimax`.
  - ADP error query returned one provider/recoverable/failover_provider event.
  - This is controlled primary-fixture 402 to real minimax fallback, not a real
    `api.anyint.ai` 402.
  - Fixture was stopped through its explicit process session. Config was
    restored.
- verification:
  - `cargo test -p freehand-config`: 17 passed.
  - `cargo test -p freehand-control`: 8 passed.
  - Runtime focused positive/negative tests passed for primary success, 402
    failover success, retry-exhaustion failover success, ineligible adapter
    failure, and fallback exhaustion.
  - Targeted clippy, `cargo fmt --check`, JS syntax, freehand-server asset smoke,
    mainlines generate/check, gates check, and `git diff --check` passed.
  - Full `freehand-runtime`: 139 passed / 12 failed. Failures are the adjacent
    live-tool/checkpoint/autonomy/task-list batch; the package is not green.
- restored state:
  - S profile: `cc`, OpenAI Responses, `api.anyint.ai`, `gpt-5.5`, env auth.
  - Master and Worker retain `fallback_provider = "minimax"`.
  - MasterS and WorkerS restarted; `127.0.0.1:4042/health` returned `ok`.
  - Fixture env marker scan returned zero matches.
- remaining:
  - Stream failover is not implemented. Partial output/tool-call side effects,
    rollback, and resume need a typed contract before enabling it.
  - No commit. Existing unrelated dirty/untracked files, including `output/`,
    remain untouched.

# 2026-07-13 Multi-worker topology and three-process convergence proof

- configuration truth:
  - `AgentConfig` now uses ordered `paired_agents = [...]` as the only peer
    topology schema.
  - Master agents may declare multiple reciprocal Slave Worker peers.
  - Slave Worker agents must declare exactly one reciprocal Master peer.
  - Legacy singular `paired_agent` is rejected by config parsing; no
    compatibility parser, primary-worker field, or reverse lookup fallback was
    added.
- runtime/UI behavior:
  - Master guidance, TaskSpaceSnapshot, task assignment boundary checks, runtime
    bootstrap token checks, and UI config projection consume the complete
    configured Worker set.
  - Worker runner startup verifies the selected agent is Slave mode, has exactly
    one paired Master, and uses the selected Worker agent id for task execution
    identity.
  - Parent evaluation remains a quality/goal decision loop, not aggregation:
    all current child tasks closed only triggers evaluation; the Master may
    reject, create next-round work, block, or finish only after total-goal
    verification.
- launchd and verifier:
  - `scripts/install-launchd.sh` can name agent-specific worker services such
    as `com.freehand.workerS.worker-alpha`.
  - `FREEHAND_LAUNCHD_PLAN_ONLY=1` plus
    `scripts/verify-launchd-worker-naming.sh` verifies labels/env/log paths
    without installing or mutating launchd state.
  - `scripts/verify-master-three-worker-e2e-online.sh` now runs in an isolated
    temporary `HOME/.freehand`, starts one Master plus three explicit Worker
    daemon processes, and stops only the PIDs it created.
- bug found during real three-process online proof:
  - First true concurrent Worker run failed one Worker at Task Center boot with
    `No such file or directory (os error 2)`.
  - Root cause was `crates/freehand-task/src/lib.rs::write_json_atomic` using a
    seconds-granularity temp file name, so concurrent writers to the same JSON
    path raced on one temp path and one rename lost its source.
  - Fix uses process/nanosecond/atomic-counter unique temp paths.
- latest online proof:
  - Session: `online-master-three-worker-evaluation-1783960928`.
  - Evidence dir:
    `/tmp/freehand-three-worker-home.Zyn1U6/.freehand/tmp/three-worker-e2e-20260714T004208-50363`.
  - Distinct Worker PIDs: `worker-alpha=51939`, `worker-beta=51940`,
    `worker-gamma=51941`; after verifier cleanup all three were stopped.
  - Initial task mapping:
    `alpha -> worker-alpha`, `beta -> worker-beta`, `gamma -> worker-gamma`.
  - Integration next-round task mapping: `integration -> worker-alpha`.
  - Beta was rejected and rerun with a new execution id before approval/close.
  - First all-children-closed evaluation created the integration task instead
    of final completion.
  - Second evaluation completed at `runtime-turn-3`; restart proof kept
    `final_evaluation_count=1` and `restart_idempotent=true`.
- verification already run for this batch:
  - `cargo test -p freehand-config -- --nocapture` -> 20 passed.
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 56 passed.
  - `cargo test -p freehand-runtime master_assignment_gate -- --nocapture` ->
    2 passed.
  - `cargo test -p freehand-runtime production_worker_runner -- --nocapture` ->
    13 passed.
  - `cargo test -p freehand-runtime parent_evaluation -- --nocapture` ->
    4 passed.
  - `cargo test -p freehand-daemon worker_mode -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-task atomic_json_write_survives_parallel_same_path_writers -- --nocapture`
    -> 1 passed.
  - `cargo test -p freehand-task -- --nocapture` -> 50 passed.
  - targeted clippy for config/task/runtime/ui-protocol/daemon/cli passed.
  - `cargo fmt --check`, `xtask mainlines generate/check`, `xtask gates check`,
    launchd naming verifier, script syntax checks, online three-worker verifier,
    and `git diff --check` passed before final memory updates.
- known non-green adjacent baselines:
  - `cargo test -p freehand-cli -- --nocapture`: 24 passed / 2 failed from stale
    tests expecting old Master tool/provider unsupported behavior.
  - `cargo test -p freehand-daemon -- --nocapture`: 14 passed / 5 failed from
    stale Master tool/workspace/provider expectation and an existing
    subscription timeout path.
  - These are not closed by the topology commit and should not be hidden.
- remaining multi-agent gaps:
  - launchd-managed three-service lifecycle is not yet online-proven.
  - Worker health/restart owner truth is not complete.
  - real-provider crash/recovery/takeover proof is not complete.
  - cross-machine multi-peer node transport remains singular/first-peer in old
    node transport paths.
  - shared `leases.json` remains a potential multi-process read-modify-write
    lost-update risk even though the latest controlled proof passed.

# 2026-07-14 Shared lease RMW concurrency closeout

- trigger:
  - The controlled three-Worker topology uses one Master Task Center namespace,
    so independent Worker processes mutate the same
    `state/task-runtime/<master>/leases.json`.
  - Process-unique atomic temp names fixed rename collisions but did not make
    the preceding load/mutate/write transaction atomic.
- red evidence:
  - New test `lease_state_rmw_preserves_parallel_distinct_writers` started 24
    concurrent distinct lease writers against the old implementation.
  - Old implementation retained only 1 of 24 leases.
  - Failure log:
    `~/Library/Application Support/rtk/tee/1783987933_cargo_test.log`.
- unique owner fix:
  - `TaskStore::with_lease_state_lock` owns one `leases.lock` advisory lock.
  - Lease create/refresh/remove holds the lock across load, mutate, and atomic
    rename.
  - Boot reconciliation removes only invalid task ids from current locked lease
    truth instead of writing its stale pre-lock full map.
  - No new lease schema, per-task fallback store, or compatibility path was
    introduced.
- local evidence:
  - `lease_state_rmw_preserves_parallel_distinct_writers` passed.
  - `lease_state_rmw_removes_only_target_during_parallel_refresh` passed.
  - `cargo test -p freehand-task -- --nocapture` passed 52 tests.
  - `cargo clippy -p freehand-task --all-targets -- -D warnings` passed.
  - `cargo fmt --check`, mainlines generate/check, gates check, and
    `git diff --check` passed.
- online evidence:
  - `scripts/verify-master-three-worker-e2e-online.sh` passed with session
    `online-master-three-worker-evaluation-1783988351`.
  - Worker PIDs were `60857/60858/60859` for
    `worker-alpha/worker-beta/worker-gamma`; no PID remained after cleanup.
  - Alpha/beta/gamma stayed bound to their configured Worker identities.
  - Beta used two distinct execution ids through reject/rework.
  - First parent evaluation created integration work; second evaluation closed
    the overall goal at `runtime-turn-3`.
  - Restart proof kept `final_evaluation_count=1` and
    `restart_idempotent=true`.
  - Evidence dir:
    `/tmp/freehand-three-worker-home.CrH6iE/.freehand/tmp/three-worker-e2e-20260714T081911-60827`.
- remaining:
  - launchd-managed three-service lifecycle and queryable Worker health/restart
    truth remain open.
  - real-provider crash/recovery/reassignment/takeover remains open.
  - cross-machine multi-peer node transport remains open.
# 2026-07-14 Worker process health/restart owner truth closeout

- target:
  - close P3 queryable Worker health/restart truth under `agent.lifecycle`.
  - keep launchd as supervisor only; no UI/daemon PID inference.
- implementation:
  - `AgentLifecycleEvent` now has typed `ProcessStarted` and
    `ProcessHeartbeat`.
  - `AgentLifecycleSnapshot` persists `process_id`, `process_instance_id`,
    `process_started_at`, `process_heartbeat_at`, and `restart_count`.
  - `TaskRuntime::query_agent_board` and `query_agent_lifecycle` project
    `alive` from `process_heartbeat_at + 5s TTL`; missing/stale process
    heartbeat does not fallback to task activity or AgentSnapshot status.
  - `ProductionWorkerRunner` writes process start at construction and heartbeat
    on every `run_once`; `WorkerHeartbeat::start` also refreshes process
    heartbeat while provider execution is running.
  - ADP/UI projection exposes process truth under `agent.process` to avoid
    bloating `UiAdpResponse` enum variants.
- online evidence:
  - `scripts/verify-master-three-worker-e2e-online.sh` now also proves Worker
    process health/restart in isolated HOME.
  - latest pass session:
    `online-master-three-worker-evaluation-1783993233`.
  - evidence dir:
    `/tmp/freehand-three-worker-home.AwGYEs/.freehand/tmp/three-worker-e2e-20260714T094033-93242`.
  - gamma proof:
    old PID `93601` fresh `alive=true`, stopped -> `alive=false` after TTL while
    retaining `task-three-worker-1781783993233-gamma` and
    `exec-worker-worker-gamma-1783993239925554000-3`, restarted as PID `96753`
    with new process instance and `restart_count=1`.
  - all verifier PIDs stopped after cleanup.
- local evidence:
  - `cargo test -p freehand-task -- --nocapture` -> 54 passed.
  - `cargo test -p freehand-runtime production_worker_runner -- --nocapture`
    -> 14 passed.
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 56 passed.
  - `cargo test -p freehand-daemon worker_mode -- --nocapture` -> 1 passed.
  - `cargo test -p freehand-cli --no-run` passed.
  - targeted clippy for task/runtime/ui-protocol/daemon passed.
  - `cargo fmt --check`, mainlines generate/check, gates check,
    `git diff --check`, and script syntax passed.
- remaining:
  - launchd-managed three-service KeepAlive/crash restart proof remains open.
  - real-provider crash/recovery/reassignment/takeover remains open.
  - cross-machine multi-peer transport remains open.

# 2026-07-14 launchd-managed three Worker lifecycle closeout

- target:
  - prove three configured Worker services are launchd-managed, not just
    manually started daemon processes.
  - keep process truth owned by `agent.lifecycle`; launchd remains only the
    supervisor.
- implementation:
  - `scripts/verify-master-three-worker-e2e-online.sh` now supports
    `FREEHAND_THREE_WORKER_WORKER_START_MODE=launchd`.
  - launchd Worker mode installs three unique agent-specific labels under an
    isolated HOME, starts Workers through `scripts/install-launchd.sh
    restartWorkerS`, then kills only the explicit gamma PID and waits for
    KeepAlive to provide a new PID.
  - `scripts/install-launchd.sh` now has `enable_launchd_service` and
    `FREEHAND_LAUNCHD_SKIP_ENABLE=1`; production defaults still call
    `launchctl enable`, while temporary online verifiers skip persistent enable
    overrides.
  - `scripts/verify-launchd-three-worker-services-online.sh` wraps the
    three-Worker verifier with launchd mode, unique label prefix, isolated
    runtime, and fixed 4143 ADP/health port.
- online evidence:
  - first pass session:
    `online-launchd-three-worker-evaluation-1783994180-13520`, evidence dir
    `/tmp/freehand-three-worker-home.tKntzt/.freehand/tmp/three-worker-e2e-20260714T095620-13531`.
  - second pass after skip-enable cleanup fix:
    `online-launchd-three-worker-evaluation-1783994336-33327`, evidence dir
    `/tmp/freehand-three-worker-home.rDDvDe/.freehand/tmp/three-worker-e2e-20260714T095856-33335`.
  - gamma proof from the second pass:
    launchd label
    `com.freehand.verify.three-worker.skipenable.1783994336-33322.worker-gamma`,
    old PID `35572`, new PID `36923`, same gamma task
    `task-three-worker-1781783994336-gamma`, same execution
    `exec-worker-worker-gamma-1783994347588322000-3`, new process instance, and
    `restart_count=1`.
  - the second pass also verified beta reject/rework, required integration
    next-round work, final parent success only after integration, and
    restart-idempotent final evaluation.
  - cleanup check for the second pass prefix returned zero launchctl matches.
- local evidence:
  - `bash -n scripts/verify-master-three-worker-e2e-online.sh` passed.
  - `bash -n scripts/verify-launchd-three-worker-services-online.sh` passed.
  - `cargo build -p freehand-daemon` passed before online verification.
- remaining:
  - real-provider crash/recovery/reassignment/takeover remains open.
  - cross-machine multi-peer transport remains open.

# 2026-07-14 Provider transient retry UI recovery closeout

- user correction:
  - Provider errors during retry/failover are transient status updates, not durable user-visible turn errors.
  - Recovered turns must update in place: show provider retry/failover while pending, then clear it when normal provider output arrives.
  - Only exhausted provider routes may materialize a failed turn/Error card.
- root cause 1:
  - OpenAI-compatible successful Responses payloads can contain `"error": null`.
  - The adapter treated field presence as error evidence, so `error:null` emitted `ProviderSemanticOutput::Error` and created persistent `openai_error` / UI Error truth on a successful turn.
- root cause 2:
  - Runtime emitted provider retry debug events but drained the debug receiver only after the backoff/next provider output path, so true transient retry status was not reliably observable in WebUI during the retry window.
- implementation:
  - OpenAI Responses and Chat Completions parsers now ignore JSON-null `error` and still emit typed errors for non-null error objects.
  - UI protocol added `UiModelRequestKind::ProviderRetry` and `ProviderFailover`; WebUI renders them as a Provider status row.
  - Runtime maps `RuntimeLive05ProviderError` / `RuntimeLive05ProviderFailover` to transient same-turn model-request activity and drains debug immediately after provider request built, retry scheduled, and failover switch events.
  - Recovered retry/failover turns keep `turn.error_events` empty; normal semantic response clears transient provider activity.
  - Added `scripts/verify-provider-recovery-webui-online.mjs` fixture verifier: first two OpenAI Responses requests return HTTP 500, third returns `status=completed,error=null`, with S-profile config/env restored in `finally`.
- evidence so far:
  - `cargo test -p freehand-provider-openai` -> 11 passed.
  - `cargo test -p freehand-ui-protocol provider_recovery_activity_updates_in_place_and_clears_on_response` passed.
  - `cargo test -p freehand-runtime provider_recovery_debug_updates_same_turn_activity` passed.
  - `cargo test -p freehand-runtime live_bridge_retries_recoverable_provider_errors_then_succeeds` passed.
  - `cargo test -p freehand-runtime live_bridge_publishes_provider_retry_before_next_attempt` passed.
  - `cargo test -p freehand-runtime live_bridge_failover_` -> 2 passed.
  - `cargo test -p freehand-server webui_smoke_renders_shell_and_asset_routes` passed.
  - Online S-profile verifier passed: `artifacts/webui-online/provider-recovery-20260714T092758-10239/summary.json` has `requestCount=3`, `providerRetryVisible=true`, `finalNoOpenAiRequestFailed=true`, `adpTerminalStatus=Success`, `adpErrors=[]`.
  - During-retry DOM showed Provider/provider retry row with no Error text; final DOM cleared provider status and showed normal final response.
  - Post-verifier S-profile restored to `provider=cc`, `provider_protocol=responses`, `base_url_host=api.anyint.ai`, health `ok`, fixture env grep empty.
- remaining before commit:
  - run fmt/clippy/mainline/gates/diff checks after final doc/memory updates.

# 2026-07-14 timer prompt injection and closed-loop contract unit lock

- user correction:
  - Timer due does not restore/resume an old session turn. It injects the persisted wakeup prompt as a new next-ordinal turn into the original source session, pushing reasoning forward from current truth.
  - Master closed-loop standard is not aggregation. Master evaluates total goal + decomposed child goals + accepted Worker review truth, then either creates next-round work, records a blocker, or completes only after objective truth is closed.
  - EventInbox delivery must not guess from time ordering. Use deterministic contract/protocol truth.
- implementation:
  - `timer_live_request` now resolves timer source ancestry and injects due source-backed timer prompt into the original session as a new `runtime-turn-N`; source-less timers still use internal `master-timer-*` turns.
  - timer prompt says this is a new follow-up turn injected by due timer, not a resume/reopen of source turn.
  - Task attachment truth added through `TaskRuntime::attach_task_to_session`; query/history from visible user sessions persists `TaskSessionAttached` without changing immutable parent.
  - WebUI current-session task filtering uses parent or `attached_session_ids`, not global task fallback.
  - EventInbox cursor changed to v2 per-task sequence watermark. Eligibility is `event.seq > watermark[task_id]`; timestamp/task id only order a returned batch.
  - `run_scheduler_tick` now applies each event to the task snapshot before building the next scheduler event so same-task multiple scheduler facts get monotonic seq.
- unit evidence:
  - `cargo test -p freehand-runtime production_master_runner_injects_due_timer_prompt_as_new_source_session_turn -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_master_runner_fires_source_less_timer_in_internal_new_turn -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_master_runner_resolves_chained_timer_to_original_session -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_master_runner_releases_due_timer_after_wakeup_failure -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_master_runner_closed_loop_requires_next_round_before_final_evaluation -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_master_runner_does_not_evaluate_while_sibling_open -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_master_rejects_complete_while_parent_child_task_open -- --nocapture` passed.
  - `cargo test -p freehand-task phase2b_v2_cursor_delivers_new_lower_task_id_event_with_same_timestamp -- --nocapture` passed.
  - `cargo test -p freehand-task phase2b_event_cursor_uses_task_sequence_watermark_and_legacy_skips_duplicates -- --nocapture` passed.
  - `cargo test -p freehand-task attach_task_to_session_is_idempotent_and_preserves_parent -- --nocapture` passed.
  - `cargo test -p freehand-runtime task_tool_query_attaches_existing_task_to_current_visible_session -- --nocapture` passed.
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture` passed.
  - `cargo test -p freehand-task -- --nocapture` passed: 56 tests.
  - `cargo test -p freehand-runtime master_runner::tests:: -- --nocapture` passed: 25 tests.
  - `bash -n scripts/verify-timer-tool-online.sh`, `node --check apps/freehand-server/assets/webui.js`, `cargo fmt --check`, `cargo run -p xtask -- mainlines generate/check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- remaining:
  - Online proof still not rerun after this exact unit-contract change. Do not claim online closed loop until S-profile verifies timer injected turn appears in original session transcript and current-session task dashboard observes attachment.
# 2026-07-14 Android launcher icon switched to repository logo

- request: use `assets/logo.png` as the project app icon.
- owner: `app.android-client`.
- implementation:
  - replaced square and round launcher PNGs for mdpi/hdpi/xhdpi/xxhdpi/xxxhdpi with deterministic 48/72/96/144/192 px derivatives of `assets/logo.png`.
  - added `apps/freehand-android/scripts/generate-launcher-icons.sh` as the single derivation path.
  - added `apps/freehand-android/scripts/verify-launcher-icons.sh` to reject missing assets, wrong dimensions, and pixel drift.
  - synced function map, test design, mainline call manifest, and generated wiki.
- evidence:
  - launcher verifier passed for all ten Android resources.
  - `./gradlew testDebugUnitTest assembleDebug` passed; APK is `apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk`.
  - extracted APK resources match resized `assets/logo.png`; manifest still binds `android:icon` and `android:roundIcon`.
  - bash syntax, mainlines generate/check, gates check, and `git diff --check` passed.
- live gap:
  - `adb devices -l` returned no devices.
  - `adb connect 100.104.163.65:5555` did not complete and was interrupted explicitly.
  - `verify-device-ui.sh 100.104.163.65:5555` returned blocked `adb_state_unavailable`; evidence: `artifacts/android-device/20260714T140530Z-100.104.163.65_5555-97976`.
  - APK installation and launcher screenshot are not claimed.

# 2026-07-14 Android WebUI-only shell and native fallback removal

- user correction:
  - Android must align directly with the existing daemon WebUI.
  - No local HTML, native conversation/settings/update/status UI, Android protocol projector, mock page, automatic endpoint switch, schema migration, or replacement UI is acceptable; errors remain explicit failures.
- root cause:
  - `MainActivity` previously loaded `file:///android_asset/bridge.html` and rendered native topbar/drawer/composer/status/timeline/update surfaces while waiting for Android-owned ADP transport.
  - the device retained `127.0.0.1:4042`, which is unreachable from the phone and left the obsolete native shell visible.
- implementation:
  - `MainActivity` is now a thin WebView/platform bridge and immediately loads `http://<host>:<port>/?client=android-webview`.
  - physically removed local `bridge.html`, native UI controllers/resources, Android ADP/SSE/HTTP clients/projector/update UI, their tests, and the server `/mock/android` page/assets.
  - Android config now admits only active Tailscale profile identity plus host/port. Removed transport/relay fields fail strict schema validation and are neither ignored nor migrated.
  - device verifier now requires `webuiShell=true`, `layoutClient=android-webview`, and a mobile `layoutShape`; old native-drawer probes were removed.
  - deleted obsolete Android native-shell design/goal documents and updated feature map, function maps, test design, mainline JSON/wiki, architecture doc, local skill, and README.
- verified evidence:
  - `./gradlew testDebugUnitTest assembleDebug` passed.
  - `cargo test -p freehand-server --lib -- --nocapture` passed 13 tests, including canonical Android WebUI shell and `/mock/android` 404.
  - APK inspection shows `assets/config/client.json` and no `bridge.html`, native UI/projector/transport artifacts.
  - launcher icon verifier, verifier bash syntax, `cargo fmt --check`, mainlines generate/check, gates check, and `git diff --check` passed.
  - daemon live URL `http://100.66.1.82:4041/?client=android-webview` returned canonical `data-webui-shell=true` HTML.
  - debug APK installed successfully on ADB device `100.104.163.65:5555` (PLZ110); app-owned config was explicitly replaced with strict `100.66.1.82:4041` host/port schema.
- true-device closure:
  - after Jason unlocked the device, ADB was reconnected explicitly to `100.104.163.65:5555` and `verify-device-ui.sh` passed.
  - evidence: `artifacts/android-device/20260715T000253Z-100.104.163.65_5555-46095`.
  - `FreehandWebUiLayout` reported `webuiShell=true`, `layoutClient=android-webview`, and `layoutShape=tall_phone`.
  - summary recorded foreground `com.freehand.android/.ui.MainActivity` with no fatal logcat.
  - screenshot manual review confirmed the canonical WebUI mobile header, agent strip, conversation cards, and composer; no native settings/update/status/timeline shell remained.
  - app-owned config remained strict `100.66.1.82:4041` host/port-only truth.

# 2026-07-15 current-session delegated task dashboard correction

- Jason corrected the mobile multi-Agent information architecture:
  - Header must show current-session running Agents and delegated tasks.
  - first tap must show the current session's child-task list only.
  - task tap must open the task-bound Worker session conversation.
- live evidence before the fix:
  - `QueryTaskBoard(include_terminal=false)` returned running `task-1784013898` with parent `webui-session-20260714072351-8e59cbcd` but omitted `attached_session_ids`.
  - its task ledger contained 23 `TaskSessionAttached` events for `webui-session-20260714100320-084ee172`, while the current task snapshot had `attached_session_ids=null`; later heartbeat/execution snapshot writers erased observation projection.
  - persisted Worker turns existed under `~/.freehand/state/turns/worker/worker-task-task-1784013898`, while ADP `QuerySessionTurns(worker-task-task-1784013898)` returned zero turns because runtime queried only the master reason namespace.
  - WebUI synthesized `worker-task-<task_id>` and changed selection without immediately refreshing `QuerySessionTurns`.
- implementation in progress:
  - task snapshot load hydrates attachment membership from only matching `TaskSessionAttached` ledger rows without replaying/rewinding lifecycle status or sequence.
  - TaskBoard DTO now carries canonical `worker_session_id` from runtime owner projection.
  - runtime `QuerySessionTurns` searches only the master and configured Worker reason namespaces and preserves owning agent/node source.
  - mobile Agent sheet was reduced to delegated child tasks; Header renders running Agent/delegated task counts and active task title; task tap consumes `worker_session_id` and immediately refreshes the Worker transcript.
- local evidence so far:
  - `attachment_survives_later_snapshot_without_attachment` passed after fixing the stale-snapshot fixture to carry its later event seq.
  - `runtime_query_session_turns_restores_worker_task_namespace` passed, including the missing-session negative case.
  - `freehand-server --lib` passed 13 tests; JS syntax, mainlines generate/check, gates check, and diff check passed before the final doc/test adjustments.
- remaining before closure:
  - rerun final package/gate stack after all edits.
  - rebuild/restart the live master daemon, prove TaskBoard attachment plus Worker transcript over ADP, then verify Header -> task list -> Worker conversation on the unlocked Android device with screenshots/DOM evidence.
# 2026-07-15 Android startup overlay and Agent resource count closeout

- user target:
  - Android startup must not show a long blank white screen; it needs a simple visible loading animation.
  - the top/mobile Agents surface must expose configurable Agent resource count, currently capped at 5, with today shared provider semantics and a future path for per-Agent provider/properties.
- implementation:
  - Android `MainActivity` now renders a native dark startup overlay with the repo launcher logo, `Freehand`, `Connecting to workspace`, and an indeterminate blue spinner before WebView readiness.
  - `WebUiStartupGate` removes the overlay only after the WebView DOM probe reports `webuiShell=true` and `layoutClient=android-webview`; page-finished alone is not accepted as ready.
  - config owner added `AgentResourceConfigUpdate` with `1..=5` validation, Master-only mutation, atomic config persistence, deterministic Worker peer grow/shrink, and shared provider/fallback provider copied from the first Worker template.
  - UI protocol and runtime added `UpdateAgentResourceConfig`; runtime routes the mutation into `config.core`, returns restart-required config projection, and does not fabricate live AgentBoard Workers.
  - WebUI mobile Agents sheet now shows `Agent resources / Worker pool`, +/- controls, limit, shared provider label, save action, and current delegated task list; header summary includes running/delegated/configured counts.
- validation:
  - `cargo test -p freehand-config update_agent_resource_config -- --nocapture` passed.
  - `cargo test -p freehand-ui-protocol agent_resource_config -- --nocapture` passed.
  - `cargo test -p freehand-runtime runtime_dispatch_updates_agent_resource_count_without_fabricating_live_agents -- --nocapture` passed.
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `cargo test -p freehand-server --lib -- --nocapture` passed 13/13; the printed `dispatch worker panicked` is the intentional join-failure projection test.
  - `cargo fmt --check`, `git diff --check`, `cargo run -p xtask -- mainlines check`, and `cargo run -p xtask -- gates check` passed.
  - Android: `JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest --tests 'com.freehand.android.ui.WebUiStartupGateTest'` passed, and `./gradlew assembleDebug` passed.
  - online WebUI: `http://100.66.1.82:4041/?client=android-webview&v=20260715-agent-resource-final` served canonical shell containing `mobile-agent-resource-count/save/increment/decrement`; JS asset returned `cache-control: no-store`.
  - Playwright mobile evidence `/tmp/freehand-mobile-agent-resource-sheet.png` showed header `1 running agent · 1 delegated task · 1 configured`, `Worker pool`, count `1 of 5`, `shared · cc`, save disabled when unchanged, and one current delegated task card.
  - Android device `100.104.163.65:5555` installed the rebuilt debug APK successfully; package `lastUpdateTime=2026-07-15 12:34:14`.
  - pre-lock true-device screenshot `/tmp/freehand-android-startup-or-webui.png` showed the startup overlay instead of white screen; logcat earlier reported canonical `FreehandWebUiLayout` JSON.
- remaining/risk:
  - after reinstall, the device is on secure lockscreen (`mDreamingLockscreen=true`, NotificationShade focus), so the current installed APK's post-load visible screen could not be re-screenshotted without Jason unlocking the phone. Do not claim that screenshot as current-install visual proof until unlocked.
  - prior full `cargo test -p freehand-runtime --lib -- --nocapture` still has unrelated live bridge/checkpoint failures and is not a green umbrella.
  - MemoryPalace re-mine was attempted after dry-run proved the first 5 files were `Cargo.toml`, `note.md`, `MEMORY.md`, `CACHE.md`, and `rust-toolchain.toml`; actual mine was blocked by existing external palace lock PID 30147 running a 12h+ `mempalace mine ... --wing routecodex ...`. Do not kill that unrelated process without explicit authorization.

# 2026-07-15 Worker transcript restore speed and fake User prompt closeout

- user target:
  - session switch must not flash an empty `New conversation` before the real transcript renders.
  - Worker/subagent transcript must not render framework task/continuation prompts as user-authored messages.
  - provider retry/failover errors that recover stay same-step transient provider activity; they do not require restarting reasoning or persisting Error cards.
- root causes:
  - runtime projected Worker `original_task` context segments into `UiTurnProjection.user_text`, so Worker internal task prompts looked like repeated User messages.
  - WebUI cleared the selected transcript and rendered immediately on session switch, so slow `QuerySessionTurns` looked like a new empty conversation.
  - `ReasonPersistence::restore_turn_snapshots_for_ui` replayed the full reason ledger and keyed rows by full `turn_id`; a real Worker sample had only 6 authoritative turn files but a 7.4GB reason ledger and expanded to 218 UI rows, causing a 75.2s online ADP query.
- implementation:
  - runtime `QuerySessionTurns` searches configured Master/Worker reason namespaces, preserves source-agent attribution, and hides `worker-task-*` framework prompts from `user_text`.
  - WebUI selected-session switch now uses `sessionRefreshInFlight` / `sessionRefreshError`, pins the requested session id, discards late transcript responses, renders an explicit loading/failure card, and never shows empty `New conversation` while a selected transcript query is in flight.
  - Historical 2026-07-15 behavior: `ReasonPersistence::restore_turn_snapshots_for_ui` used authoritative closed/active turn snapshots plus rollback-marker sidecar truth when available, coalesced repaired `-rN` rounds to the latest logical turn, and reserved ledger rebuild for missing authoritative snapshots.
  - Superseded on 2026-07-18: Worker/tool transcript observability requires exact per-round UI restore for selected `QuerySessionTurns`; daemon bootstrap alone uses authoritative-only snapshots to avoid historical ledger scans.
  - rollback now persists a compact `rollback-markers.json` sidecar so effective UI transcript filtering no longer requires reading the full reason ledger on normal query paths.
- local evidence:
  - `cargo test -p freehand-reason --lib -- --nocapture` passed 61/61.
  - `cargo test -p freehand-runtime runtime_query_session_turns -- --nocapture` passed both parent-evaluation and Worker task namespace cases.
  - `cargo test -p freehand-server --lib -- --nocapture` passed 13/13; the printed `dispatch worker panicked` is the intentional join-failure projection test.
  - provider recovery contract tests passed: runtime same-step retry success, retry-before-next-attempt, provider recovery debug update, and UI protocol recovery-clears-activity.
  - `node --check apps/freehand-server/assets/webui.js`, `jq empty` for touched mainline JSON, `cargo fmt --check`, `xtask mainlines generate/check`, `xtask gates check`, and `git diff --check` passed.
- online S-profile evidence:
  - `scripts/install-launchd.sh restartS` installed the current debug daemon and restarted `com.freehand.daemonS`.
  - `curl http://127.0.0.1:4042/health` returned `ok`; `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
  - served WebUI JS/CSS hashes matched workspace: `webui.js` `d408e0c06875cc2c9eaa680e13da68052eeec4e23fcf06b0b3c21e38ce2dae59`, `webui.css` `64501adb5952bfa62edd4a73aa2f9c26af3a7daf28cea9775b93a9a5b7f57096`.
  - before reason.persistence fix, ADP `QuerySessionTurns(worker-task-task-1784013898)` took `75204ms`, returned `218` rows, `user_text` non-null `0`, forbidden prompt count `0`.
  - after fix, the same real Worker session returned in `133ms`, raw response `9291` bytes, `6` rows, `user_text` non-null `0`, forbidden prompt count `0`, `terminalOrFinalRows=6`, `sources=["worker"]`.
  - mobile WebUI CDP artifact `artifacts/webui-online/session-switch-worker-1784128322135/summary.json`: parent selected session `webui-session-20260714100320-084ee172`; task click selected `worker-task-task-1784013898`; immediate state had loading visible, no chat empty `New conversation`, userMessageCount `0`; final state had worker nav, chatMessageCount `2`, userMessageCount `0`, fake prompt regex `false`.
- remaining:
  - no Android APK rebuild was needed in this slice because Android native code did not change; Android WebView consumes the daemon-hosted WebUI assets.

# 2026-07-15 framework tool semantics and per-child inspection closeout

- root causes:
  - `task` and `timer` were classified as generic tool displays, so WebUI could only show `Run tool · task/timer` instead of owner-projected operation semantics.
  - live pending/cancelled Worker projections published raw internal `worker-task-*` request text even though persisted `QuerySessionTurns` already hid it.
  - mobile delegated-task rendering capped the current-session list at eight rows, preventing deterministic one-by-one inspection for larger parent sessions.
- implementation:
  - `tool.display` now owns first-class `Task` and `Timer` kinds. `parse_task_tool_display` projects operation/task/title/agent/status/cwd/dispatch; `parse_timer_tool_display` projects operation/timer/timing/reason/wakeup prompt. `ui.protocol` carries those fields to public tool activity.
  - runtime live pending/cancelled and persisted query projections share `ui_should_hide_user_text`; framework parent/Worker prompts project no user-authored message while ordinary session prompts remain visible.
  - mobile Agent sheet renders every current parent child task, adds `1/N`, `data-task-id`, and owner-projected `data-worker-session-id`.
  - `scripts/verify-worker-subtasks-online.py` read-only enumerates TaskBoard children for one parent and queries every canonical Worker transcript.
  - `scripts/verify-worker-subtasks-webui-online.mjs` drives the real mobile WebUI through CDP, waits for sheet open/closed geometry, verifies task identity, Worker entry, prompt absence, return path, and saves screenshots.
  - timer verifier now uses an independent temporary `timer-fixture` provider id so a configured `fallback_provider=minimax` does not violate the primary/fallback-distinct config contract.
- local evidence:
  - `freehand-blocks` projection tests: 10 passed.
  - `framework_tool_public_projection_uses_task_and_timer_display_semantics`: passed.
  - `live_worker_task_projection_hides_internal_user_text`: passed.
  - `live_regular_session_projection_keeps_user_text`: passed.
  - `runtime_query_session_turns`: 2 passed.
  - `root_and_asset_routes_return_webui_shell_files`: passed.
  - JS/Python/bash/JSON syntax, `cargo fmt --check`, mainlines generate/check, gates check, and `git diff --check`: passed.
- online S-profile evidence:
  - current debug daemon restarted on `127.0.0.1:4042`; health and ADP smoke passed.
  - served JS/CSS SHA-256 matched workspace: JS `cadd993553415833d04a052ae370c62a4c217a76e4a11f17ce1fb9082c978700`, CSS `64501adb5952bfa62edd4a73aa2f9c26af3a7daf28cea9775b93a9a5b7f57096`.
  - real three-child parent `online-master-three-worker-e2e-current` passed the read-only checker with alpha/beta/gamma transcripts non-empty, terminal Success, and `user_text_leak_count=0` for all three.
  - real mobile WebUI artifact `artifacts/webui-online/worker-subtasks-1784133884215/summary.json` passed: current parent sheet card carried task/Worker session ids; task click selected `worker-task-task-1784128683`; Worker nav was visible; `userMessageCount=0`; forbidden internal prompt text was absent; Back returned to the exact parent. Manual screenshot review confirmed the opened sheet and clean Worker transcript.
  - real archived ADP turn `online-master-three-worker-e2e` projected `task` as `Create Worker task` with task/title/cwd/dispatch semantics after daemon restart.
  - S-profile was restored to `cc` / OpenAI Responses / `api.anyint.ai` / `gpt-5.5` / env auth.
- unclosed evidence:
  - there was no current persisted `timer` tool activity available for a WebUI screenshot. Unit/protocol semantics are green, but timer-card browser-visible proof is not claimed.
  - `verify-timer-tool-online.sh` created and completed timer `timer-online-proof-1784133281-65729`, but its mock did not observe the due-turn provider request before the script timed out; the due turn later appeared in the source session and timer state was completed/fired once. Treat this as a verifier timing/route gap, not a green timer online closeout.
  - Android device `100.104.163.65:5555` was online but locked/dozing; `FREEHAND_ANDROID_SKIP_INSTALL=1 verify-device-ui.sh` was blocked at `artifacts/android-device/20260715T164610Z-100.104.163.65_5555-52926`. No current true-device claim.
# 2026-07-16 Master/Worker standalone lifecycle correction

- Jason corrected the acceptance order: Worker lifecycle and Master lifecycle
  must each close independently before integration; one three-Worker E2E cannot
  stand in for two state-machine proofs.
- Source trace confirmed Worker reject/reason-again, blocked-without-auto-retry,
  interrupted/reassign, review submit, approve, and close are bound.
- Confirmed Worker pause/resume gap: `WorkerControl::Resume` writes
  `TaskStatus::Running`, while `ProductionWorkerRunner::run_once` only claims
  `Assigned`; no production safe-point acknowledgement or deterministic
  reasoning re-entry exists.
- Confirmed major-change gap: `ExecutionFactKind` has no distinct typed
  attention/scope-change fact, so Master cannot distinguish task-contract
  invalidation from generic interruption/blockage.
- Confirmed Master busy gap: EventInbox is source-ordered and synchronous;
  there is no durable priority queue, active-work checkpoint, safe-point
  preemption, typed resolution, or exact context return path.
- Added the code-independent contract and review surface at
  `docs/lifecycles/master-worker-lifecycle.json` and
  `docs/wiki/master-worker-lifecycle.md`; pending edges remain explicitly
  pending and are not implementation claims.

# 2026-07-16 Master idle attention admission/dequeue contract

- Jason corrected the queue contract:
  - EventInbox admission and attention dequeue are different operations.
  - Admission remains strict source order and advances the durable cursor only
    after admission/classification.
  - Dequeue gives large weight to blocked showstoppers, critical/high semantic
    changes, and bounded task priority.
  - Admission-sequence aging, not wall-clock time, prevents low-priority
    starvation.
- Implementation:
  - `MasterLoopState` persists `pending_attention` and
    `next_attention_sequence`.
  - dequeue score is `severity_rank * 10000 +
    clamp(task_priority,-100,100) * 100 + admission_age * 5000`.
  - retryable failure keeps the same pending event/cursor/admission identity.
  - stale no-op attention is removed and selection continues in the same tick.
  - parent evaluation now reads only the first round of the first persisted
    user turn from authoritative reason truth; UI-coalesced repair/control
    rounds cannot replace the overall goal.
- Evidence so far:
  - `cargo test -p freehand-runtime master_attention -- --nocapture`: 4 passed.
  - `cargo test -p freehand-runtime master_runner::tests:: -- --nocapture`: 31 passed.
  - `cargo test -p freehand-runtime production_worker_runner -- --nocapture`: 18 passed.
  - `cargo test -p freehand-task attention_required -- --nocapture`: 1 passed.
  - `cargo test -p freehand-task master_poll -- --nocapture`: 2 passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate/check`,
    `cargo run -p xtask -- gates check`, JSON parse checks, and
    `git diff --check`: passed.
- Remaining:
  - busy-Master safe-point suspension/checkpoint/resolution/return path remains
    pending; idle weighted attention does not close that lifecycle.
  - MemoryPalace re-mine/search still required.

# 2026-07-16 Master busy active-work state-machine slice

- implementation:
  - added `master_work` active-work truth under `~/.freehand/state/master-loop/<master>.active-work.json` with a sibling lock file.
  - live Master submit registers active work before committing the next turn ordinal and clears it on terminal/error/cancel completion.
  - concurrent foreground Master submit now fails explicitly and does not consume a turn ordinal.
  - live bridge publishes provider/tool safe-point phases into `master_work` using the original logical turn id, not repaired round ids.
  - Master lifecycle runner defers lower-priority attention, requests suspend during provider/tool effect in-flight, suspends only at declared safe points, resolves isolated attention into typed changed-task identity, and restores exact work/session/turn/trace identity.
  - active-work resolution rejects raw Worker/control transcripts and provider request/response payload markers.
- evidence:
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime runtime_live_submit -- --nocapture`: 2 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_master_busy -- --nocapture`: 4 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_master_attention -- --nocapture`: 2 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_master_resume -- --nocapture`: 1 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime master_runner::tests:: -- --nocapture`: 38 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime live_bridge_cancel_token -- --nocapture`: 3 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_worker_runner -- --nocapture`: 18 passed.
  - `cargo clippy -p freehand-runtime --all-targets -- -D warnings`, `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check`: passed.
- remaining:
  - typed attention resolution is persisted but not yet injected into the original foreground reasoning continuation.
  - no online daemon/WebUI/Android proof for busy-Master live preemption yet.
  - evidence wrapper must not run in parallel because seconds-stamped logs collide.

# 2026-07-16 Busy Master attention continuation closeout

- corrected remaining gap from the prior busy-active-work slice:
  - live bridge now consumes `master_work.attention_resolution` exactly once in
    the original foreground Master turn.
  - the continuation refreshes TaskSpaceSnapshot and admits typed
    `AttentionResolution` as turn-volatile/no-cache developer context.
  - stale provider tool calls produced before attention resolution receive
    paired failed tool results and are not executed.
  - stale terminal candidates are discarded before terminal persistence and do
    not become durable closed-turn truth.
- root-cause bug fixed during verification:
  - the no-pending-tool branch previously used a Rust let-chain with
    `attention_resolution_after_provider.take()` before checking
    `pending_tool_calls`, which consumed and lost the resolution when no tool
    calls existed.
  - condition order is now `!pending_tool_calls.is_empty() && let Some(...)`,
    so terminal continuation can still consume the resolution.
- evidence:
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_master_resume -- --nocapture`: 3 passed after the final safe-point signature cleanup.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime live_master_attention -- --nocapture`: 2 passed after the final cleanup.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_master_foreground -- --nocapture`: 2 passed after the final cleanup.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime master_runner::tests:: -- --nocapture`: 42 passed before final signature cleanup.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime runtime_live_submit -- --nocapture`: 2 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-blocks -- --nocapture`: 52 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-contracts -- --nocapture`: 10 passed.
  - `cargo clippy -p freehand-runtime --all-targets -- -D warnings`, `cargo clippy -p freehand-blocks --all-targets -- -D warnings`, `cargo clippy -p freehand-contracts --all-targets -- -D warnings`, `cargo fmt --check`, `cargo run -p xtask -- mainlines generate/check`, `cargo run -p xtask -- gates check`, and `git diff --check`: passed.
- docs synced:
  - `master_work.admit_resolution_context` added to resource map, function map,
    mainline JSON, and test design.
  - `reason.context-planner` and `contracts.core` now document
    `AttentionResolution` segment coverage.
  - lifecycle manifest marks `master.edge.continue_original` bound by focused
    tests, while the wiki keeps isolated-control-turn and online product proof
    gaps explicit.
- remaining:
  - no daemon/WebUI/Android online proof for full busy-Master preemption yet.
  - `master.edge.handle_attention` remains pending until an isolated control
    turn is bound to a suspended active user turn with positive/negative tests.

# 2026-07-16 Master isolated attention control-turn binding

- test design first:
  - `master_work.resolve_attention` coverage now requires suspended-foreground
    linkage, isolated lifecycle request identity, and raw-control-transcript
    exclusion.
- focused proof:
  - `production_master_attention_uses_isolated_control_turn` observes the exact
    foreground checkpoint in `SuspendedByAttention` while the decision runs and
    proves the request uses a distinct task-scoped `master-lifecycle-*`
    session plus event/attempt-isolated turn and trace ids.
  - `production_master_attention_raw_transcript_never_enters_user_session`
    returns raw control/provider sentinel text from the executor and proves it
    is absent from foreground ReasonPersistence, `master_work`, and typed
    resolution constraints.
- binding result:
  - `master.edge.handle_attention` moved from `pending` to focused-test
    `bound`.
  - no production owner-path change was required; the prior active-work and
    lifecycle request implementation already satisfied the contract once the
    missing explicit binding tests were added.
- evidence:
  - `production_master_attention`: 4 passed.
  - `production_master_resume`: 3 passed.
  - `live_master_attention`: 2 passed.
  - `production_master_foreground`: 2 passed.
  - `production_master_busy`: 4 passed.
  - full `master_runner::tests::`: 44 passed.
  - targeted runtime clippy, fmt, mainlines check, gates check, JSON parse, and
    diff check passed.
- remaining:
  - full S-profile daemon/WebUI proof for suspend -> isolated decision -> typed
    continuation is still required before product closure.
  - production Worker safe-point pause/resume remains the next standalone
    lifecycle implementation gap.

# 2026-07-16 Worker pause/resume safe-point focused closeout

- objective slice: close the standalone Worker pause/resume lifecycle gap before broader multi-agent integration proof.
- root source truth checked first: goal prompt file, Freehand skill, CACHE/MEMORY/note, MemoryPalace, resource map, function map, mainline JSON, lifecycle manifest, and worker-control test design.
- implementation:
  - `ProductionWorkerRunner::run_once` now starts a `WorkerPauseMonitor` for the selected task/execution before calling the Worker live executor.
  - the monitor polls persisted `TaskRuntime::query_worker_control_events` and sets the `LiveReasonCancelToken` when latest task-state control is applied `pause`.
  - after executor return, runner re-queries pause truth; if pause is active, it returns `Idle` and does not write `TaskReviewSubmitted`, `TaskBlocked`, or heartbeat failure over `TaskPaused` truth.
  - persisted `resume` path still selects the same `Running` task/execution through `resumed_controlled_running_task`.
- regression repair:
  - `production_worker_runner_pause_stops_before_submission` now applies pause while executor is in flight and proves the runner-wired cancel token is observed.
  - stale success after pause now returns `Idle` and is ignored; previous expectation of explicit TaskCenter error was too coarse because pause is a valid cooperative stop, not a runner failure.
- evidence:
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_worker_runner_pause_stops_before_submission -- --nocapture`: 1 passed.
  - `scripts/run-cargo-test-with-evidence.sh -- -p freehand-runtime production_worker_runner -- --nocapture`: 19 passed.
  - `cargo run -p xtask -- mainlines generate`: passed.
  - `cargo run -p xtask -- mainlines check`: passed.
  - `cargo run -p xtask -- gates check`: passed.
- docs synced so far:
  - `worker.edge.pause` and `worker.edge.resume` moved to bound in `docs/lifecycles/master-worker-lifecycle.json`.
  - `docs/testing/worker.control.md`, `docs/function-maps/worker.control.md`, `docs/function-maps/runtime.master-worker-loop.md`, `docs/mainline-calls/runtime.master-worker-loop.json`, `docs/testing/runtime.master-worker-loop.md`, `docs/goals/multi-agent-final-convergence-plan.md`, and generated `docs/wiki/runtime.master-worker-loop.md` updated.
  - `docs/wiki/master-worker-lifecycle.md` manually updated because it is the human lifecycle review surface.
- remaining:
  - full S-profile multi-Worker/WebUI convergence proof is still not run for this slice.
  - busy-Master live preemption online proof remains separate.

# 2026-07-16 Parent objective recovery from turn-start ledger

- objective slice:
  - Fix the remaining parent-session evaluation gap where all children could
    close but Master could not build the final quality-evaluation follow-up
    because the original user objective had no effective closed snapshot.
- root cause:
  - Original operator text remained in authoritative reason ledger
    `TurnStarted` truth for `runtime-turn-1`.
  - Later repair/evaluation work left the effective closed snapshot at a
    repaired round such as `runtime-turn-1-r2`.
  - `parent_user_objectives()` read `ReasonPersistence::restore()` output,
    which returns effective closed/active snapshots and therefore could miss
    the original first-round objective.
  - UI/effective snapshots are conversation projection truth, not parent-goal
    truth.
- implementation:
  - Added `ReasonPersistence::restore_turn_start_snapshots`.
  - It reads authoritative reason-ledger `TurnStarted` rows, returns
    non-UI-coalesced request truth, and still filters rolled-back logical
    turns through rollback markers.
  - `parent_user_objectives()` now calls this owner API and admits only first
    user turn / first round (`runtime-turn-1`) while excluding repair/control
    prompts.
  - Synced `reason.persistence` and `runtime.master-worker-loop` function maps,
    mainline JSON, test designs, generated wiki, goal plan, CACHE, MEMORY, and
    local Freehand skill.
- local evidence:
  - `restore_turn_start_snapshots_preserves_original_round_and_respects_rollback`
    proves original `runtime-turn-1` plus repaired `runtime-turn-1-r2` starts
    are both recoverable before rollback and absent after rollback.
  - `production_master_runner_recovers_parent_goal_from_first_round_turn_start_ledger`
    proves parent evaluation receives original objective when the only
    restored closed snapshot is `runtime-turn-1-r2`, and repair prompt text is
    excluded.
- online evidence inherited from current source state:
  - process-mode verifier session `online-master-three-worker-evaluation-1784187343`
    reached alpha close, beta reject/rework close, gamma interrupted same-task
    takeover, integration next-round task, final Success on `runtime-turn-3`,
    and restart-idempotent `final_evaluation_count=1`.
  - launchd-mode verifier session
    `online-launchd-three-worker-evaluation-1784187532-2390` reached the same
    lifecycle with gamma KeepAlive restart from PID `5442` to `26638` and no
    cleanup residue for label prefix
    `com.freehand.verify.three-worker.1784187532-2390`.
- remaining:
  - The foreground receipt gap was resolved in the next verifier slice: the
    initial parent turn now returns a completed waiting receipt after dispatch,
    and background lifecycle completion is observed through ADP truth.
  - Full goal still requires S-profile daemon/WebUI current-session proof,
    busy-Master live preemption proof, real-provider non-fixture
    crash/recovery/takeover proof, and Android true-device proof if native or
    packaged assets change.

# 2026-07-16 Three-Worker foreground waiting verifier closeout

- objective slice:
  - Align the controlled three-Worker online verifier with production
    lifecycle semantics: the foreground parent turn acknowledges dispatch and
    waits; it does not busy-poll child Worker history until a fixture budget
    fails.
- implementation:
  - `scripts/verify-master-three-worker-e2e-online.sh` fixture now returns a
    `claim="waiting"` response once alpha/beta/gamma are created and assigned
    but not all review-submitted.
  - The Python verifier recursively checks the `SubmitUserInput` receipt and
    fails if it contains `error` or lacks `reason_live_turn_completed`.
  - Worker review, Master review, next-round integration, final completion,
    AgentBoard restart, and idempotency remain verified through ADP
    TaskBoard/TaskHistory/SessionTurns/AgentBoard truth after foreground
    dispatch.
- online evidence:
  - `scripts/verify-launchd-three-worker-services-online.sh` passed.
  - Session: `online-launchd-three-worker-evaluation-1784190586-60111`.
  - Evidence dir:
    `/tmp/freehand-three-worker-home.9OpUCD/.freehand/tmp/three-worker-e2e-20260716T162946-60118`.
  - Foreground receipt:
    `reason_live_turn_completed rounds=7 schema_rejections=0 tool_executions=6 restored_closed_turns=0`.
  - Beta task `task-three-worker-1781784190586-beta` had
    `TaskReviewRejected` and then a second execution
    `exec-worker-worker-beta-1784190613898933000-9` before approval/close.
  - Gamma task `task-three-worker-1781784190586-gamma` had provider 500,
    `TaskInterrupted`, and same-task takeover by worker-alpha with execution
    `exec-worker-worker-alpha-1784190719830844000-104`.
  - Next-round integration task
    `task-three-worker-1781784190586-integration` closed under worker-alpha.
  - Parent final turn `runtime-turn-3` reached `Success`.
  - Restart idempotency: `final_evaluation_count=1`.
  - Gamma KeepAlive restart: old PID `60815`, new PID `72469`,
    `restart_count=1`.
  - Cleanup check for `com.freehand.verify.three-worker.1784190586-60111`
    returned no launchctl matches.
- local evidence after the verifier change:
  - `bash -n scripts/verify-launchd-three-worker-services-online.sh` passed.
  - `bash -n scripts/verify-master-three-worker-e2e-online.sh` passed.
  - `cargo fmt --check` passed.
  - `cargo test -p freehand-runtime master_runner::tests:: -- --nocapture`
    passed 45/45.
  - `cargo run -p xtask -- mainlines generate` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining:
  - This slice closes the verifier foreground receipt gap only. Full goal still
    requires S-profile daemon/WebUI current-session proof, busy-Master live
    preemption proof, real-provider non-fixture crash/recovery/takeover proof,
    and Android true-device proof if native/package assets change.

# 2026-07-16 S-profile manual-test readiness closeout

- objective:
  - Continue the multi-agent closeout until Jason can begin manual testing on
    the fixed S-profile without creating more random sessions.
- implementation already present and verified this round:
  - Master daemon mode keeps WebUI/ADP host lifetime independent from the
    background Master lifecycle runner.
  - Master runner repairs stale EventInbox cursor, drops stale missing-task
    attention, and records missing-goal parent evaluation as skipped instead of
    killing the lifecycle loop.
  - `QuerySessionTurns` hides internal timer follow-up prompts from public UI
    projection.
  - AgentBoard releases Worker current binding on `TaskClosed` and repairs
    stale persisted lifecycle snapshots at boot.
  - WebUI Worker labels now use Worker-only ordinals.
  - Worker-subtasks WebUI verifier pins a fixed parent session, checks exact
    delegated-task count, clicks every child Worker card, verifies canonical
    `worker_session_id`, and rejects internal prompt leakage.
  - launchd Worker env generation copies credential-style provider env keys
    from the matching Master env; xtask CI/CD gate fixtures were fixed so the
    new gate requirement has aligned positive/negative tests.
- online S-profile evidence:
  - fixed parent session: `s-profile-three-worker-real-1781784192325`.
  - `scripts/verify-worker-subtasks-online.py --url ws://127.0.0.1:4042/adp --parent-session s-profile-three-worker-real-1781784192325 --include-terminal --require-transcript --require-count 4` passed with four closed Success child tasks and `user_text_leak_count=0`.
  - browser verifier passed with artifact
    `artifacts/webui-online/worker-subtasks-1784203306738`: Header
    `0 running agents · 4 delegated tasks · 3 configured`, sheet status
    `4 current task(s) · 0 blocked · 0 review · 0 stale`, Worker labels max 3,
    and all four Worker transcripts selected by canonical `worker-task-*`
    session with `userMessageCount=0` and `fakePromptVisible=false`.
  - session drawer DOM proof on the same parent had `topWorkerRows=[]` and
    `parentChildCount=4`, proving Worker task sessions are not top-level global
    sessions and are indented under the owning persisted Master session.
  - AgentBoard query showed `worker`, `worker-2`, and `worker-3` all
    `alive=true`, `state=idle`, `current_task_id=null`,
    `current_execution_id=null`, `current_activity=idle`,
    `last_activity=closed`.
  - S config stayed `provider=cc`, `provider_type=openai`,
    `provider_protocol=responses`, `base_url_host=api.anyint.ai`,
    `default_model=gpt-5.5`, `auth_source=env`.
  - fixture env grep over `~/.freehand/daemonS.env` and `workerS*.env` returned
    zero matches.
  - launchd list has only `com.freehand.daemonS`,
    `com.freehand.workerS.worker`, `com.freehand.workerS.worker-2`, and
    `com.freehand.workerS.worker-3`.
- local/gate evidence:
  - `master_mode_keeps_host_alive_when_lifecycle_runner_stops` passed.
  - `production_master_runner` passed 24/24.
  - `runtime_query_session_turns_restores_background_parent_evaluation` passed.
  - `production_worker_runner` passed 19/19.
  - `task_close_releases_agent_lifecycle_and_boot_repairs_stale_current_binding`
    passed.
  - `agent_process` passed 2/2.
  - full `freehand-task` passed 59/59.
  - `webui_smoke_renders_shell_and_asset_routes` passed.
  - xtask CI/CD gate subset passed 5/5 after fixture repair; full xtask passed
    50/50.
  - targeted clippy for `freehand-daemon`, `freehand-runtime`,
    `freehand-task`, `freehand-server`, and `xtask` passed after narrowing the
    daemon test's `HOME_LOCK` scope so the sync lock is not held across await.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`,
    `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- remaining:
  - Android true-device and release `4041` were not part of this S-profile
    manual-test readiness closeout.
  - Historical untracked `.agent-collab/`, `.DS_Store`, `output/`,
    `scripts/__pycache__/`, and WebUI artifact files were left untouched.

# 2026-07-16 Android S-profile true-device closeout

- device:
  - serial `100.104.163.65:5555`, model `PLZ110`, authorized and foreground.
  - current debug APK built with Android Studio JBR; `testDebugUnitTest assembleDebug` passed and `adb install -r` succeeded.
- endpoint truth:
  - release `4041` was not used or restarted.
  - S-profile remained `127.0.0.1:4042`, `cc/openai/responses`, `api.anyint.ai`, `gpt-5.5`, env auth.
  - device used `adb reverse tcp:4042 tcp:4042` plus app-owned `files/daemon-connection.json` set to `127.0.0.1:4042`; both were read back.
- canonical WebUI proof:
  - `apps/freehand-android/scripts/verify-device-ui.sh` passed with artifact `artifacts/android-device/s-profile-4042-20260716T125516Z-PLZ110`.
  - logcat probe: `webuiShell=true`, `layoutClient=android-webview`, `layoutShape=tall_phone`, `webuiCssApplied=true`, `webuiJsReady=true`, stylesheet URLs from `127.0.0.1:4042`.
  - dumpsys showed `com.freehand.android/.ui.MainActivity` top-resumed/focused; fatal logcat grep returned no matches; screenshot was manually reviewed.
- Agent/Worker navigation proof:
  - artifact `artifacts/android-device/s-profile-4042-20260716T130309Z-PLZ110-agent-nav`.
  - parent `webui-session-20260715151744-4957c72a` showed `0 running agents · 1 delegated task · 3 configured`.
  - Agent sheet listed only current task `task-1784128683`, closed under Worker 1.
  - task click selected canonical `worker-task-task-1784128683`, displayed Worker final output, had `userMessageCount=0` and `fakePromptVisible=false`, then returned to the exact parent session.
- remaining:
  - device currently depends on the active `adb reverse` mapping for S-profile manual testing; reconnecting adb may require recreating that mapping.
  - release `4041` remains untouched and unverified in this closeout.

# 2026-07-16 path diagnostics and ambiguous-submit recovery closeout

- objective:
  - Fix hardcoded-path/symlink confusion and the WebUI state where a submit
    timeout/error could render `dispatch status unknown · refresh needed`
    without first checking service truth.
- implementation:
  - Master task guidance and `task.target_cwd` schema now keep
    leading-~/symlink aliases valid only when they resolve to an existing
    workspace, reject broad/glob/output-directory targets, and require
    requested plus canonical path evidence.
  - Removed stale project-name path examples from Master guidance and replaced
    them with generic `~/work/repo-a` / `~/work/repo-b` and
    `/absolute/existing/workspace` examples.
  - `task(op="create")` and Worker target-cwd preflight now include
    `target_cwd_path_diagnostic` with requested, expanded, nearest existing,
    nearest existing canonical, symlink ancestors, and missing suffix.
  - WebUI submit catch path now calls an owner-truth refresh helper before
    showing unknown-dispatch. If submitted text materialized, pending state
    clears and the current session continues; if not, the selected fixed/draft
    session and pending card remain visible.
  - WebUI `Turn:null` query results are safe during refresh and no longer
    throw while checking `turn.session_id`.
  - Added `scripts/verify-webui-ambiguous-submit-recovery.mjs`, using a fixed
    session id and explicit test hook to verify both ambiguous-submit branches
    against the served WebUI asset without creating random persisted sessions.
- evidence:
  - Local focused tests passed: `freehand-tools task_tool_exposes_operation_parameter`,
    `freehand-runtime task_tool_create`, `production_worker_runner_missing`,
    `live_bridge_master_autonomy`, and `freehand-server webui_smoke_renders_shell_and_asset_routes`.
  - JS checks passed for `apps/freehand-server/assets/webui.js` and
    `scripts/verify-webui-ambiguous-submit-recovery.mjs`.
  - S-profile `127.0.0.1:4042` was service-scoped restarted; health and
    `adp-smoke` passed; served WebUI JS hash matched workspace hash
    `97301220e08baf34846a2b1e092a5c47926f3d33d0573f6c6fc5d35a29b9b993`.
  - `node scripts/verify-webui-ambiguous-submit-recovery.mjs` passed with
    fixed session `webui-ambiguous-submit-recovery-fixed`; summary shows
    `materializedClearsPending=true` and `unknownKeepsPendingSession=true`.
  - Hardcoded sample grep for `~/code/codex`, `Deepseek-reasonix`,
    `/Users/fanzhang`, and `github/codex` over touched product/docs/script
    paths returned zero matches.
  - Final checks passed: `cargo fmt --check`,
    `cargo clippy -p freehand-server --all-targets -- -D warnings`,
    `cargo run -p xtask -- mainlines generate/check`,
    `cargo run -p xtask -- gates check`, and `git diff --check`.
- restoration:
  - S-profile config remained `cc/openai/responses`, `api.anyint.ai`,
    `gpt-5.5`, env auth.
  - fixture env grep over daemonS and workerS env files returned zero matches.

# 2026-07-17 zterm remote-daemon connectivity contract slice

- objective:
  - Inspect `~/code/zterm` relay/Tailscale traversal mechanism and integrate the condensed Freehand local contract for account-scoped multi-daemon configuration, direct-first route selection, relay endpoint declaration, and QR/deep-link bootstrap import.
- zterm mechanism inspected:
  - `relay-directory.ts`: account directory publishes devices, daemon presence, endpoint candidates, and sessions.
  - `connection-config-share.ts`: URL-safe base64 JSON deep links use app/web prefixes.
  - traversal config/route selector: direct Tailscale/IPv6/IPv4 candidates are preferred before relay, with health diagnostics.
  - relay client/server/RTC bridge: real relay traversal is a WebSocket signaling plus WebRTC/DataChannel tunnel, not an ordinary WebUI URL fallback.
- implementation:
  - Added config-owned `remote_daemon_registry` under `config.core` with `[remote_daemon_accounts]`, `[remote_daemons]`, endpoint candidates, direct-first route diagnostics, route health records, selected route, and QR/deep-link bootstrap bundle helpers.
  - Added CLI `remote-daemon-bootstrap-link --daemon <id> --credential-env <ENV> [--ttl-seconds <seconds>] [--web]`.
  - Added Android `remote_registry` app-owned config mode, `freehand://daemon/import?payload=...` deep-link import, explicit relay endpoint validation, and version-addressed WebUI load for selected endpoint.
  - Fixed the bootstrap payload cross-platform field contract: Rust now emits daemon `activeEndpoint` for Android import, not `activeEndpointId`; the config test decodes the generated link and locks this.
  - Updated resource map, function map, mainline call source, test design, generated mainline docs, and local skill rule so Android cannot own account directory truth, route scoring, Tailscale probing, or relay tunnel semantics.
- evidence:
  - `cargo test -p freehand-config remote_daemon -- --nocapture` passed 6/6.
  - `cargo test -p freehand-config -- --nocapture` passed 29/29.
  - `cargo test -p freehand-cli cli_generates_remote_daemon_bootstrap_link_from_config_registry -- --nocapture` passed.
  - `JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest --tests com.freehand.android.data.DaemonConnectionConfigTest --tests com.freehand.android.data.HostConfigTest` passed.
  - `JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest assembleDebug` passed.
  - `cargo fmt --check`, `jq empty docs/resource-maps/core.json docs/mainline-calls/config.core.json docs/mainline-calls/app.android-client.json`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- validation gaps:
  - Full `cargo test -p freehand-cli -- --nocapture` is not green: `cli_runs_reason_live_tool_call_mock_and_persists` and `cli_runs_reason_live_unsupported_provider_smoke` fail in adjacent reason-live/tool/provider assertions.
  - This slice does not implement the Freehand relay server, relay WebRTC/DataChannel/pass-through IO, live account directory/presence service, Tailscale OS auto-connect/probing loop, or true-device QR scan proof.
  - Android can import selected endpoint config and load the daemon-hosted WebUI URL, but relay tunnel semantics are still unimplemented and must not be claimed.

# 2026-07-17 node-owned remote-daemon directory slice

- objective:
  - Continue the zterm remote-daemon integration by moving runtime directory/presence projection into `node.master-slave` instead of leaving only config/Android bootstrap contracts.
- implementation:
  - Added `RemoteDaemonDirectory` in `crates/freehand-node`: publishes account-scoped directory snapshots from `RemoteDaemonRegistryConfig`, stores per-daemon route resolutions, and maps route diagnostics without credentials.
  - Added resource-map resource `remote_daemon_directory` and direct operation `remote_daemon_registry.project_directory`, with source-edge registry binding to `RemoteDaemonDirectory::publish_registry`.
  - Updated node function map, mainline call source, test design, generated wiki, feature-map resource ownership index, and local skill.
- evidence:
  - `cargo test -p freehand-node remote_daemon_directory -- --nocapture` passed 2/2.
  - `cargo test -p freehand-node -- --nocapture` passed 23/23.
  - `cargo check -p freehand-runtime` passed after mapping `NodeRuntimeError::RemoteDaemonDirectory` to dispatch failure.
  - `cargo test -p freehand-config remote_daemon -- --nocapture` passed 6/6.
  - `cargo test -p freehand-cli cli_generates_remote_daemon_bootstrap_link_from_config_registry -- --nocapture` passed 1/1.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- gaps:
  - This still is not relay tunnel/pass-through implementation. Real relay signaling/DataChannel or equivalent tunnel IO, live account directory/presence service, Tailscale OS auto-connect/probing loop, and true-device QR scan proof remain open.

# 2026-07-17 local remote relay transport closeout

- objective:
  - Continue the remote-daemon work from config/bootstrap and node directory into a real local relay transport proof.
  - Keep the resource independent from task/session truth; relay is a standard daemon transport resource under `app.runtime-daemon`.
- implementation:
  - Added `apps/freehand-server/src/remote_relay.rs`.
  - Added `RemoteRelayDirectory`, `RemoteRelayHostRegistration`, account directory projection, `/relay/hosts`, `/relay/directory/{account_id}`, `/relay/daemon/{relay_host_id}/health`, and `/relay/daemon/{relay_host_id}/adp`.
  - Added `freehand-daemon remote-relay [--bind HOST:PORT]`.
  - Added `scripts/verify-remote-relay-local-online.sh`, which starts real upstream smoke server, relay daemon, and CLI ADP smoke processes, then validates register/directory/health/ADP/missing-host behavior.
  - Updated `remote_relay_transport` resource map, function map, test design, mainline call source, generated wiki, feature map, and local skill.
- evidence:
  - `cargo test -p freehand-server remote_relay -- --nocapture` passed 2/2.
  - `scripts/verify-remote-relay-local-online.sh` passed with `remote_relay_local_online_ok upstream_url=http://127.0.0.1:61093 relay_url=http://127.0.0.1:61094 relay_host=studio-host adp=ws://127.0.0.1:61094/relay/daemon/studio-host/adp`.
  - `cargo check -p freehand-daemon --message-format=short` passed.
  - `cargo test -p freehand-daemon master_mode_keeps_host_alive_when_lifecycle_runner_stops -- --nocapture` passed 1/1.
  - `cargo test -p freehand-daemon -- --list` listed 20 tests successfully.
  - `jq empty docs/resource-maps/core.json docs/mainline-calls/app.runtime-daemon.json` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- gap / risk:
  - Full unfiltered `cargo test -p freehand-daemon -- --nocapture` was started once and hung before test output with the test process at 0% CPU; it was interrupted and not counted as pass.
  - This slice proves local HTTP/ADP pass-through relay, not WebRTC/TURN/DataChannel signaling, Tailscale OS auto-connect/probing, authenticated persistent relay service, or true-device QR scan.

# 2026-07-17 relay WebUI HTTP and Android true-device attempt

- objective:
  - Extend `remote_relay_transport.proxy_http` from `/health` only to the daemon-hosted WebUI HTTP surface required by Android WebView, then install the APK on PLZ110 and prove the app loads through the relay endpoint.
- implementation:
  - `freehand-daemon remote-relay` now proxies registered-host HTTP paths under `/relay/daemon/{relay_host_id}/...` to the upstream daemon path.
  - The relay preserves query strings and streams non-rewritten upstream bodies.
  - Static WebUI HTML/JS responses are rewritten from daemon-root absolute paths (`/assets`, `/adp`, `/ui/...`) to the relay host namespace so `/relay/daemon/studio-host/?client=android-webview` can load assets and ADP through the same relay URL.
  - `scripts/verify-remote-relay-local-online.sh` now uses unique temporary ports, waits longer for health, and verifies namespaced WebUI HTML/CSS/JS/query plus ADP and missing-host behavior.
  - Resource map, function map, mainline call source, generated wiki, test design, and local skill now require namespaced WebUI HTTP relay proof, not only health/ADP.
- evidence:
  - `cargo test -p freehand-server --lib remote_relay -- --nocapture` passed 2/2.
  - `scripts/verify-remote-relay-local-online.sh` passed with `remote_relay_local_online_ok upstream_url=http://127.0.0.1:62798 relay_url=http://127.0.0.1:62799 relay_host=studio-host adp=ws://127.0.0.1:62799/relay/daemon/studio-host/adp`.
  - `JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest assembleDebug` passed.
  - Current APK installed on PLZ110 and app-owned `files/daemon-connection.json` was read back with `connectionMode=remote_registry` and `webUrl=http://127.0.0.1:44042/relay/daemon/studio-host/`.
  - Fixed relay device attempt artifact: `artifacts/android-device/relay-s-profile-4042-20260717T061410Z-PLZ110`.
  - Device-side relay setup succeeded before UI probe: `adb reverse tcp:44042 tcp:44042`, relay host registration, and `freehand-cli adp-smoke` over `ws://127.0.0.1:44042/relay/daemon/studio-host/adp`.
  - Local checks passed: `cargo fmt --check`, `jq empty docs/resource-maps/core.json docs/mainline-calls/app.runtime-daemon.json`, `cargo check -p freehand-daemon --message-format=short`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check`.
- blocked:
  - The true-device canonical WebUI probe is not closed because PLZ110 is security locked/dozing: `mDreamingLockscreen=true`, `secure=true`, `mCurrentFocus=NotificationShade`, while `mFocusedApp` is `com.freehand.android/.ui.MainActivity`.
  - `wm dismiss-keyguard` and wake/swipe did not unlock the device. Manual unlock is required before rerunning `apps/freehand-android/scripts/verify-device-ui.sh` against the fixed relay config.

# 2026-07-17 Android relay true-device closeout after unlock

- objective:
  - Complete the blocked PLZ110 true-device closeout after manual unlock.
- evidence:
  - Artifact: `artifacts/android-device/relay-s-profile-4042-20260717T063227Z-PLZ110-unlocked`.
  - Current debug APK installed successfully: `install.txt` has `Performing Streamed Install` and `Success`.
  - App-owned `files/daemon-connection.json` was read back with `connectionMode=remote_registry`, active daemon `studio-s-profile-relay`, and `webUrl=http://127.0.0.1:44042/relay/daemon/studio-host/`.
  - Relay host registration and account directory succeeded for account `jason`, daemon `studio-s-profile`, host `studio-host`, upstream `http://127.0.0.1:4042`.
  - Relay ADP proof succeeded: `adp_smoke_ok url=ws://127.0.0.1:44042/relay/daemon/studio-host/adp`.
  - `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` passed with summary `freehand_activity_foreground_no_fatal_logcat`.
  - Canonical WebUI layout probe: `layoutClient=android-webview`, `layoutShape=tall_phone`, `webuiShell=true`, `webuiCssApplied=true`, `webuiJsReady=true`, stylesheet URLs loaded from `http://127.0.0.1:44042/relay/daemon/studio-host/assets/...`.
  - Foreground evidence: dumpsys showed `topResumedActivity`, `ResumedActivity`, `mCurrentFocus`, and `mFocusedApp` all on `com.freehand.android/.ui.MainActivity`.
  - Screenshot `device-ui/screenshot.png` was manually reviewed and shows the canonical Freehand mobile WebUI with Agent summary and conversation content, not native fallback UI.
  - No fatal/exception logcat pattern matched for `com.freehand.android`.
- restoration:
  - Scoped relay process recorded in `relay.pid` exited; local `127.0.0.1:44042` was available after cleanup.
  - Device `adb reverse tcp:44042 tcp:44042` remains present for repeat testing.
  - S-profile `4042` config remained `cc/openai/responses`, `api.anyint.ai`, `gpt-5.5`, env auth.

# 2026-07-17 Android relay live recheck and manual-test readiness

- Reinstalled the current debug APK on unlocked PLZ110 and completed a fresh
  relay-path device verification in
  `artifacts/android-device/relay-s-profile-4042-20260717T064054Z-PLZ110-live-recheck`.
- The fixed artifact contains `adb install -r` success, relay ADP smoke
  success, app-owned `remote_registry` config readback, canonical WebUI layout
  truth (`android-webview`, `tall_phone`, shell/CSS/JS all ready), foreground
  Activity evidence, a reviewed screenshot, and no matching fatal app logcat.
- S-profile stayed on `4042` with `cc/openai/responses`,
  `api.anyint.ai`, `gpt-5.5`, env auth; release `4041` was not touched and
  fixture env grep remained empty.
- The one-shot shell background relay was not a durable hand-test service:
  the execution environment reaped the `nohup` child after command exit, even
  though relay stdout/stderr showed no application error. Do not report a
  one-shot background PID as running proof here.
- For immediate manual testing in this Codex run, relay `127.0.0.1:44042` is
  held by persistent exec session `22966`; `studio-host` is registered to
  `http://127.0.0.1:4042`, ADP smoke passes through the relay, the device keeps
  `adb reverse tcp:44042 tcp:44042`, and Freehand MainActivity was launched.

# 2026-07-17 framework-owned reasoning and AgentBoard online closeout

- Correction:
  - Master does not drive Worker reasoning.
  - The runtime reason loop owns schema validation, tool-call/result pairing,
    history continuation, and provider re-entry for both Master and Worker.
  - Worker runner owns claim, heartbeat, execution, and completion mapping into
    Task Center truth. Master only makes task-level decisions from current
    TaskBoard, AgentBoard, EventInbox, and review truth.
- Lifecycle implementation:
  - `TaskBlocked`, `TaskAttentionRequired`, `TaskReviewSubmitted`,
    `TaskReviewRejected`, `TaskReviewApproved`, `TaskInterrupted`,
    `TaskCancelled`, and `TaskClosed` release AgentBoard current task,
    execution, and turn bindings and retain typed audit truth in
    `last_activity`.
  - Boot reconciliation also clears legacy execution-only stale bindings where
    `current_task_id` was empty but `current_execution_id` remained set.
- Online evidence:
  - Rebuilt and service-scoped restarted `com.freehand.daemonS`,
    `com.freehand.workerS.worker`, `worker-2`, and `worker-3`.
  - `curl http://127.0.0.1:4042/health` returned `ok`; `freehand-cliS adp-smoke`
    passed.
  - Config remained `cc/openai/responses`, `api.anyint.ai`, `gpt-5.5`, env
    auth; fixture env grep was empty.
  - Final AgentBoard query returned 21 rows,
    `idle_current_binding_violations=[]`, and `non_idle_rows=[]`.
  - `worker`, `worker-2`, and `worker-3` were all `alive=true`, `state=idle`,
    `current_task_id=null`, `current_execution_id=null`, with historical audit
    state only in `last_activity`.
- Local verification:
  - `task_close_releases_agent_lifecycle_and_boot_repairs_stale_current_binding`
    passed.
  - `agent_process` passed 2/2.
  - `runtime_query_reads_phase1_task_and_agent_boards` passed after test-home
    isolation repair.
  - `runtime_dispatches_phase2a_master_worker_loop_into_task_truth` passed.
  - `production_worker_runner` passed 20/20.
  - `cargo fmt --check`, mainlines check, gates check, and `git diff --check`
    passed.
- Test isolation root cause:
  - Concurrent independent cargo test processes could generate the same
    nanosecond-plus-counter temp runtime home because the counter was
    process-local.
  - `temp_runtime_home()` now includes the OS pid, preventing false shared task
    truth across concurrent test processes.

# 2026-07-17 tool path diagnostic owner proof

- Correction:
  - The `/Users/fanzhang/github/codex` failure was a built-in path-tool owner
    bug, not a model reasoning bug.
  - On macOS/Linux, path tools must own relative-to-absolute normalization,
    leading-`~` expansion, symlink-parent inspection, nearest existing parent,
    canonical parent, and missing-leaf diagnostics.
- Implementation:
  - `freehand-tools` now renders `path_diagnostic` on path resolution failure
    with requested path, locked workspace, absolute path, existence/type,
    nearest existing parent, nearest existing canonical parent, missing suffix,
    and symlink ancestors.
  - `read_file`, `ls`, writable path resolution, and locked path resolution use
    that diagnostic; `glob` expands leading `~` and canonicalizes the nearest
    existing absolute glob prefix before workspace-boundary checks.
  - Tool schemas and `tool.registry` docs now tell Workers that relative paths
    resolve from the locked workspace, leading `~` expands, absolute in-workspace
    symlink aliases are valid, and `ls` is the existence/type probe.
- Online S-profile proof:
  - Artifact: `/tmp/freehand-tool-path-online-1781784281165`.
  - Task: `task-tool-path-diagnostic-1781784281165`.
  - Execution: `exec-tool-path-diagnostic-1781784281165`.
  - Worker session: `worker-task-task-tool-path-diagnostic-1781784281165`.
  - Target cwd: `/Users/fanzhang/github`, canonical `/Users/fanzhang/Documents/github`.
  - Missing requested leaf: `/Users/fanzhang/github/codex`.
  - Fixture forced real Worker provider call sequence: first response called
    `ls(path="/Users/fanzhang/github/codex")`; second provider request included
    the failed `tool_result`.
  - Second provider request evidence:
    `secondHadToolResult=true`, `secondHadDiagnostic=true`,
    `bodyLength=13646`, and matches for `path_diagnostic`,
    `requested=/Users/fanzhang/github/codex`,
    `absolute=/Users/fanzhang/github/codex`,
    `nearest_existing=/Users/fanzhang/github`,
    `nearest_existing_canonical=/Users/fanzhang/Documents/github`,
    `missing_suffix=codex`, and symlink ancestor
    `/Users/fanzhang/github -> /Users/fanzhang/Documents/github`.
  - Task history reached `TaskCreated, TaskWaitingAgent, TaskAssigned,
    TaskResumed, TaskHeartbeat, TaskInterrupted, TaskAssigned, TaskResumed,
    TaskHeartbeat, TaskBlocked` in the same task.
  - Restoration: S-profile health and `adp-smoke` passed; config restored to
    `cc/openai/responses`, `api.anyint.ai`, `gpt-5.5`, env auth; fixture env
    grep over `daemonS.env` and `workerS.worker.env` returned 0 matches.
- Harness lessons:
  - `task-restart-seed-running` with a long TTL keeps the running lease valid,
    so Worker correctly does not take over. To force same-task Worker recovery,
    seed with `--ttl-seconds 1`, wait for expiry, then restart the worker.
  - `worker` provider fixture must use a provider id distinct from its fallback
    provider id; `tool-path-fixture` was accepted while `minimax` was rejected.

# 2026-07-17 WebUI path diagnostic closed-loop proof

- Scope:
  - S-profile only: `ws://127.0.0.1:4042/adp` and
    `http://127.0.0.1:4042/health`.
  - Fixed parent session: `webui-path-diagnostic-fixed-v2`.
  - Prompt: WebUI-submitted Master/Worker path diagnostic for
    `/Users/fanzhang/github/codex` under symlink workspace
    `/Users/fanzhang/github -> /Users/fanzhang/Documents/github`.
- Fixes verified:
  - Stale/dead-owner `master.active-work.json` no longer blocks a new live
    submit. Runtime live submit recovers a dead-owner active work checkpoint,
    interrupts the stale persisted turn, clears the checkpoint, and then accepts
    the new foreground turn.
  - WebUI no longer renders `ToolPending` lifecycle truth as completed. Parent
    ToolPending turns render `waiting lifecycle` and terminal row `Lifecycle /
    running`; blocked Worker turns render `blocked` at assistant badge, final
    row, and bottom turn status.
  - `scripts/verify-webui-path-diagnostic-online.mjs` now rejects stale
    ToolPending evidence, requires current task id matching, and waits for
    Master lifecycle `task(op="append")` blocked decision to produce
    `TaskProgressed`.
- Final online artifact:
  - `/Volumes/extension/code/freehand/artifacts/webui-online/path-diagnostic-1784290032509`.
  - Task: `task-webui-path-diagnostic-1784290032509`.
  - Worker session: `worker-task-task-webui-path-diagnostic-1784290032509`.
  - Fixture evidence: `masterRequests=4`, `workerRequests=2`,
    `secondHadToolResult=true`, `secondHadDiagnostic=true`,
    `masterLifecycleAppendRequested=true`.
  - Master lifecycle append checks all true: lifecycle coordinator request,
    current task id, TaskBlocked/execution_blocked context, path_diagnostic,
    requested path, nearest existing canonical path, and missing suffix.
  - TaskHistory events: `TaskCreated, TaskWaitingAgent, TaskAssigned,
    TaskResumed, TaskHeartbeat, TaskBlocked, TaskProgressed`.
  - Parent WebUI DOM: selected current `runtime-turn-398-r3`,
    `selectedTerminalStatus=toolpending`, `turnStatus=waiting lifecycle`,
    `assistantStatus=waiting lifecycle`, `finalStatus=running`.
  - Worker WebUI DOM: `selectedTerminalStatus=blocked`,
    `turnStatus=blocked`, `assistantStatus=blocked`, `finalStatus=blocked`,
    `userMessageCount=0`, requested/canonical/missing_suffix visible, internal
    Worker prompt text hidden.
  - Agent sheet opened the current Worker card and
    `globalSessionListHasWorker=false`; direct ADP session list showed only
    persistent parent sessions, no top-level `worker-task-*`.
- Verification:
  - `node --check scripts/verify-webui-path-diagnostic-online.mjs`
  - `node scripts/verify-webui-path-diagnostic-online.mjs`
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture`
  - `cargo test -p freehand-ui-protocol tool_pending_terminal_projects_as_lifecycle_running_not_final_completed -- --nocapture`
  - `cargo test -p freehand-runtime live_dispatch_recovers_dead_owner_master_active_work_before_new_turn -- --nocapture`
  - `cargo test -p freehand-runtime production_master_recovery_candidates_require_resolved_legacy_attention -- --nocapture`
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
  - Final health/config/env restore: health `ok`; provider restored to
    `cc/openai/responses`, `api.anyint.ai`, `gpt-5.5`, env auth; fixture env
    grep returned 0 matches.
- Residual:
  - The fixed parent session intentionally avoids creating new sessions, but
    previous repeated validation runs left three blocked child tasks visible in
    that fixed session. The final run matches the newest task id exactly and
    does not expose Worker sessions globally. Cleaning historical test tasks is
    a separate destructive/runtime-state decision and was not performed.

# 2026-07-17 Android Tailscale relay correction

- correction:
  - A real Android device must not receive `127.0.0.1` for the Mac relay.
  - `adb reverse` is not product connectivity proof and must not remain part of
    the real-device acceptance path.
- live topology:
  - Mac Tailscale IPv4: `100.66.1.82`
  - relay listener: `100.66.1.82:44042`
  - registered upstream: `http://127.0.0.1:4042` (S-profile only)
  - Android relay WebUI: `http://100.66.1.82:44042/relay/daemon/studio-host/`
  - Android relay ADP: `ws://100.66.1.82:44042/relay/daemon/studio-host/adp`
- evidence:
  - relay health over Tailscale returned `ok`
  - `freehand-cli adp-smoke` over the Tailscale relay passed
  - app-owned Android config readback contains only the Tailscale relay URLs
  - `adb reverse --list` was empty after removing `tcp:44042` and `tcp:4042`
  - `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555`
    passed at `artifacts/android-device/tailscale-relay-s-profile-4042-20260717T-final`
  - WebView logcat proved stylesheet URLs came from
    `http://100.66.1.82:44042/...`, with `webuiShell=true`,
    `webuiCssApplied=true`, and `webuiJsReady=true`
- task distinction:
  - The displayed Worker `blocked` state is not a transport failure.
  - `/Users/fanzhang/github` resolves to `/Users/fanzhang/Documents/github`,
    but the requested leaf `codex` does not exist in either location.
  - Therefore the path-diagnostic task is correctly blocked pending a real
    repository path; it is not evidence that the phone failed to connect.

# 2026-07-18 WebUI accepted TaskBoard receipt closeout

- objective:
  - Remove user-visible `unknown` / "任务未知" style submit states when service
    truth has already accepted a request but the selected transcript has not
    materialized yet.
  - Keep fixed-session testing reusable and avoid creating new test sessions.
- implementation:
  - `UiTaskSnapshotProjection` carries task-owner `created_at`.
  - Runtime TaskBoard projection preserves owner `created_at`.
  - WebUI ambiguous-submit recovery clears pending state from either
    materialized transcript truth or same-parent TaskBoard task truth whose
    `created_at` is after the submit window.
  - If TaskBoard proves acceptance while transcript is still empty, WebUI
    renders an accepted service receipt card instead of the clean "New
    conversation" empty state.
  - Agent summary wording is locked away from old "delegated task" language and
    toward Worker lifecycle buckets.
- evidence:
  - `node --check apps/freehand-server/assets/webui.js`
  - `node --check scripts/verify-webui-ambiguous-submit-recovery.mjs`
  - `jq empty docs/mainline-calls/app.webui-smoke.json docs/mainline-calls/ui.protocol.json docs/mainline-calls/runtime.ui-command-dispatch.json`
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture`
  - `cargo test -p freehand-ui-protocol task_list_subscription_matches_runtime_projection_only -- --nocapture`
  - `cargo test -p freehand-runtime runtime_query_reads_phase1_task_and_agent_boards -- --nocapture`
  - `cargo test -p freehand-cli cli_runs_task_lifecycle_sample_against_mock_websocket -- --nocapture`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `cargo fmt --check`
  - `git diff --check`
  - `scripts/install-launchd.sh restartS`
  - S-profile `http://127.0.0.1:4042/health` returned `ok`.
  - Served JS hash matched workspace:
    `018c06b7f1a862dd4931b2a29ed9ff07ddecac7742a355039097004e40bf2fc3`.
  - `FREEHAND_WEBUI_BASE_URL=http://127.0.0.1:4042/ node scripts/verify-webui-ambiguous-submit-recovery.mjs`
    passed with artifact
    `artifacts/webui-online/ambiguous-submit-recovery-fixed/summary.json`.
  - Artifact checks all true: `materializedClearsPending`,
    `taskTruthClearsPending`, and `unverifiedKeepsPendingSession`.
  - Accepted receipt branch rendered
    `Service accepted this request through TaskBoard truth` and
    `task-ambiguous-submit-accepted`, with zero `New conversation` or `unknown`
    text.
  - Unverified branch kept the selected fixed session and pending card with
    `checking service truth · submit receipt not verified`.
  - Tailscale relay health and ADP smoke passed through
    `100.66.1.82:44042`.
- restoration / gap:
  - S-profile remained `cc/openai/responses`, `api.anyint.ai`, `gpt-5.5`, env
    auth; fixture env grep returned zero matches.
  - Android true-device proof is not closed in this slice because
    `adb connect 100.104.163.65:5555` timed out and `adb devices` was empty.
    Do not claim current Android closure until the device ADB endpoint is
    reachable and `verify-device-ui.sh` passes over the Tailscale relay with
    `adb reverse --list` empty.

# 2026-07-18 Android Tailscale relay final reproof

- objective:
  - Close the Android true-device gap from the WebUI accepted-receipt slice.
  - Keep device connectivity on Tailscale relay and prove no `adb reverse`.
- verifier fix:
  - `verify-device-ui.sh` previously misreported a local `apkanalyzer` Java
    runtime failure as `apk_missing_launcher_activity_class`.
  - It now records `blocked` / `apkanalyzer_failed` with
    `apkanalyzer-failed.txt`, instead of treating local toolchain failure as APK
    content truth.
- evidence:
  - `nc -vz -G 5 100.104.163.65 5555` succeeded and ping had 0% packet loss.
  - `adb connect 100.104.163.65:5555` succeeded; device reported
    `product:PLZ110 model:PLZ110`.
  - `adb reverse --list` was empty before and after verification.
  - App-owned `files/daemon-connection.json` readback used
    `connectionMode=remote_registry`,
    `relayUrl=http://100.66.1.82:44042`,
    `webUrl=http://100.66.1.82:44042/relay/daemon/studio-host/`, and
    `adpUrl=ws://100.66.1.82:44042/relay/daemon/studio-host/adp`.
  - Android local build passed:
    `JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest assembleDebug`.
  - Negative verifier proof without `JAVA_HOME` returned
    `VERIFY_EXIT=2`, summary `status=blocked`, `reason=apkanalyzer_failed`.
  - Final true-device proof passed:
    `JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" FREEHAND_ANDROID_ARTIFACT_DIR=artifacts/android-device/tailscale-relay-s-profile-4042-20260718T-final apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555`.
  - Final artifact:
    `artifacts/android-device/tailscale-relay-s-profile-4042-20260718T-final`.
  - Summary: `status=passed`,
    `reason=freehand_activity_foreground_no_fatal_logcat`.
  - WebUI layout log:
    `layoutClient=android-webview`, `layoutShape=tall_phone`,
    `webuiCssApplied=true`, `webuiJsReady=true`, `webuiShell=true`.
  - Stylesheet URLs came from
    `http://100.66.1.82:44042/relay/daemon/studio-host/assets/...`.
  - Foreground evidence showed `topResumedActivity`, `ResumedActivity`,
    `mCurrentFocus`, and `mFocusedApp` all on
    `com.freehand.android/.ui.MainActivity`, with
    `mDreamingLockscreen=false`.
  - App-scoped fatal grep returned zero matches.
  - Screenshot was manually inspected: it shows the canonical mobile Freehand
    WebUI with current session content, Agent summary, and `waiting lifecycle`,
    not a native fallback or empty session.

# 2026-07-18 Master stale waiting and message timestamps closeout

- trigger:
  - Fixed parent session `webui-session-20260716144723-93040bd0` had `0 running tasks` but stayed in stale blocked/waiting lifecycle after the correct Worker task closed.
  - Root evidence from `runtime-turn-401-r11`: completion schema rejected `claim=complete` because historical `task-1784213603:Cancelled` and `task-1784213319:Cancelled` were treated as still-open child Worker tasks.
- root fix:
  - `crates/freehand-runtime/src/lib.rs::master_parent_session_completion_rejection` no longer uses `status != Closed`.
  - New helper `task_status_blocks_parent_completion` blocks only actionable/unresolved statuses: `Created`, `WaitingAgent`, `Assigned`, `Running`, `Interrupted`, `Paused`, `Blocked`, `ReviewSubmitted`, `Approved`, and `Rejected`.
  - Terminal historical children `Cancelled`, `Failed`, and `Closed` no longer keep a parent session in stale waiting.
  - Added live regression `live_master_allows_complete_with_terminal_cancelled_child_tasks`; existing negative `live_master_rejects_complete_while_parent_child_task_open` still protects no-premature-complete.
- docs / skill:
  - Updated `docs/function-maps/runtime.master-worker-loop.md`.
  - Updated `docs/testing/runtime.master-worker-loop.md`.
  - Updated `.agents/skills/freehand-dev/SKILL.md` with the reusable gate rule.
- local proof:
  - `cargo fmt -p freehand-runtime`
  - `cargo fmt --check`
  - `cargo test -p freehand-runtime live_master_allows_complete_with_terminal_cancelled_child_tasks -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_master_rejects_complete_while_parent_child_task_open -- --nocapture` passed.
  - `cargo test -p freehand-runtime production_master_runner_recovers_closed_parent_workset_after_cursor_advanced -- --nocapture` passed.
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `cargo test -p freehand-runtime production_master_runner_ -- --nocapture` passed 25/25.
  - `cargo run -p xtask -- mainlines generate`, `mainlines check`, `gates check`, and `git diff --check` passed.
- S-profile online proof:
  - Pre/post config stayed `provider=cc provider_type=openai provider_protocol=responses base_url_host=api.anyint.ai default_model=gpt-5.5 auth_source=env`.
  - Fixture env grep over `daemonS.env` and three worker env files returned zero matches.
  - `scripts/install-launchd.sh restartS` passed and `http://127.0.0.1:4042/health` returned `ok`.
  - TaskHistory proved `task-1784213319` and `task-1784213603` ended with `TaskCancelled`, while `task-1784303605` ended with `TaskReviewSubmitted,TaskReviewApproved,TaskClosed`.
  - Same fixed parent session submit produced receipt `reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 restored_closed_turns=6`.
  - `adp-session-query --session webui-session-20260716144723-93040bd0` then showed session status `:7:success` with latest `runtime-turn-402-r2`.
  - `runtime-turn-402-r2.json` has terminal `Success`, `schema_rejections=0`, and summary saying the correct `~/code/codex` Worker audit was reviewed/closed and obsolete wrong-path tasks were cancelled.
  - `adp-error-query --turn runtime-turn-402-r2` showed only provider recoverable `retry_same_step` rows, no completion schema rejection.
  - `adp-task-query --status running` returned `count=0`; fixed-session observability output showed Worker idle with `current_task_id=null`.
- WebUI timestamp proof:
  - Headless Chrome opened `http://127.0.0.1:4042/?verify=message-times` with selected session `webui-session-20260716144723-93040bd0`.
  - DOM result: `messageCount=7`, `missingTimeCount=0`, selected turn `runtime-turn-402-r2`.
  - Artifact: `artifacts/webui-online/message-times-1784350844712/message-times.json` and `message-times.png`.
- toolchain note:
  - `mempalace` remains unavailable due bad interpreter `/Users/fanzhang/.local/pipx/venvs/mempalace/bin/python`; this was recorded as a tooling gap and did not block owner-map/doc/test verification.

# 2026-07-18 WebUI Header relationship schema contract strict closeout

- trigger:
  - Jason corrected that UI and all Master/Worker/session relationships must be
    locked by document contracts and schema, not guessed from UI copy, id
    prefixes, DOM order, or debug text.
- owner truth:
  - resource map route: `task.project_to_ui` is the allowed `task ->
    ui_projection` operation; UI-to-task remains indirect through projection /
    runtime command.
  - protocol schema: `UiTaskSnapshotProjection.parent_session_id`,
    `attached_session_ids`, `worker_session_id`, and `task_id`; Master root is
    persisted session metadata.
  - runtime projection owner always populates `worker_session_id` through
    `project_task_snapshot_for_ui`.
- implementation closeout:
  - WebUI already consumed `task.worker_session_id` through
    `workerSessionIdForTask()` and had no browser-side `worker-task-*`
    synthesis.
  - `scripts/verify-webui-path-diagnostic-online.mjs` still had a verifier-only
    `task.worker_session_id || worker-task-${taskId}` fallback; removed it.
  - The verifier now fails if the current task or any same-parent schema child
    task lacks `UiTaskSnapshotProjection.worker_session_id`, instead of
    filtering missing-schema children out of the comparison set.
  - `docs/function-maps/app.webui-smoke.md`,
    `docs/testing/app.webui-smoke.md`, and `.agents/skills/freehand-dev/SKILL.md`
    now state the relationship lock as schema-contract truth, including
    `task_id`.
- local proof:
  - `node --check apps/freehand-server/assets/webui.js`
  - `node --check scripts/verify-webui-path-diagnostic-online.mjs`
  - `node --check scripts/verify-worker-subtasks-webui-online.mjs`
  - `jq empty docs/mainline-calls/app.webui-smoke.json docs/mainline-calls/ui.protocol.json docs/resource-maps/core.json`
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture`
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `git diff --check`
- online S-profile proof:
  - command:
    `FREEHAND_WEBUI_PATH_RUN_STAMP=20260718-schema-contract-strict FREEHAND_WEBUI_PATH_TASK_ID=task-webui-path-diagnostic-schema-contract-strict FREEHAND_WEBUI_PATH_ARTIFACT_DIR=artifacts/webui-online/path-diagnostic-schema-contract-strict node scripts/verify-webui-path-diagnostic-online.mjs`
  - artifact:
    `artifacts/webui-online/path-diagnostic-schema-contract-strict/summary.json`
  - parent session: `webui-path-diagnostic-fixed-v2`
  - task id: `task-webui-path-diagnostic-schema-contract-strict`
  - worker session:
    `worker-task-task-webui-path-diagnostic-schema-contract-strict`
  - Header node proof: `relationSchema=UiTaskSnapshotProjection`,
    `relationSource=TaskBoard.worker_session_id`,
    `data-session-id=worker-task-task-webui-path-diagnostic-schema-contract-strict`,
    `data-task-id=task-webui-path-diagnostic-schema-contract-strict`.
  - Header dropdown proof: `dropdownHeight=420`, `viewportHeight=844`,
    `halfScreenOk=true`.
  - schema set proof:
    `headerTreeCoversEverySchemaChild=true` and
    `headerTreeHasNoExtraWorkerProjection=true`.
  - navigation proof:
    `headerWorkerClickSelectedProjectedSession=true` and
    `headerBackRestoredExactParent=true`.
  - Worker detail proof: selected Worker session was `blocked`, contained
    requested path `/Users/fanzhang/github/codex`, canonical parent
    `/Users/fanzhang/Documents/github`, and `missing_suffix=codex`, with
    `userMessageCount=0` and `fakePromptVisible=false`.
- restoration:
  - `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp`
    returned `provider=cc provider_type=openai provider_protocol=responses
    base_url_host=api.anyint.ai default_model=gpt-5.5 auth_source=env`.
  - fixture env grep over daemon and worker S env files returned 0 matches.
  - health `http://127.0.0.1:4042/health` returned `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
  - served asset hashes matched workspace:
    JS `18effaf9873d5bc75ede0dbec82aaf805b8ab7946365524b4f3f0b45e4edae33`,
    CSS `1b2c1a2f536f45f7b72eaef7b49797df7bd703b684574ffb71f0cc80d2f1ed9c`.
- residual:
  - The fixed parent session intentionally retains historical blocked child
    tasks from earlier fixed-session runs. The strict schema verifier now proves
    the Header tree renders exactly every same-parent TaskBoard schema child and
    no extra Worker projection. Cleaning old task truth remains a separate
    destructive/runtime-state decision and was not performed.

# 2026-07-18 Android WebView settings drawer back correction

- trigger:
  - User-visible phone state was reported as "closed loop" while the screen was
    still visibly wrong/stuck from the operator perspective.
- read-only evidence:
  - Device `100.104.163.65:5555` was online, app package version remained
    `versionCode=2`, `versionName=0.1.1`.
  - App-owned `daemon-connection.json` pointed to
    `http://100.66.1.82:44042/relay/daemon/studio-host/`, with no
    `adb reverse`.
  - Relay health and root HTML over `100.66.1.82:44042` returned canonical
    Freehand WebUI.
  - After scoped app restart, foreground evidence was
    `com.freehand.android/.ui.MainActivity`; `FreehandWebUiLayout` reported
    `layoutClient=android-webview`, `webuiShell=true`,
    `webuiCssApplied=true`, and `webuiJsReady=true`.
  - `verify-device-ui.sh` passed, but manual tap evidence found the real UX
    defect: Settings drawer content could scroll the Close header out of view,
    and Android Back did not close the WebUI drawer, so it could look stuck.
- fix:
  - WebUI mobile drawer headers are sticky for Sessions and Settings drawers.
  - WebUI exposes `window.__freehandHandleAndroidBack`, which blurs focused form
    controls, then closes WebUI dialog/Header tree/Agent sheet/mobile drawer.
  - Android `MainActivity` routes physical Back through that WebUI hook before
    WebView history or Activity finish.
- verification target:
  - Rebuild/reinstall APK, restart app on the Tailscale relay, open Config,
    scroll/focus provider field, prove Back first keeps the app in settings or
    clears focus, Back again returns to the conversation instead of exiting, and
    capture screenshot/logcat evidence.

# 2026-07-18 Android WebView settings drawer verification blocked by device lock

- completed local/install evidence:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `jq empty docs/mainline-calls/app.android-client.json docs/mainline-calls/app.webui-smoke.json` passed.
  - `cargo fmt -p freehand-server`, then `cargo fmt --check` passed.
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture` passed.
  - `JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest assembleDebug` passed.
  - `adb -s 100.104.163.65:5555 install -r apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk` succeeded.
  - App-owned `daemon-connection.json` still points to
    `http://100.66.1.82:44042/relay/daemon/studio-host/` and
    `ws://100.66.1.82:44042/relay/daemon/studio-host/adp`; `adb reverse --list`
    returned empty output.
  - Relay health returned `ok`; `freehand-cliS adp-smoke --url ws://100.66.1.82:44042/relay/daemon/studio-host/adp` passed.
  - `cargo run -p xtask -- mainlines generate`, `mainlines check`, `gates check`,
    and `git diff --check` passed after wiki regeneration.
- blocked true-device evidence:
  - After app-scoped restart, screenshots under
    `artifacts/android-device/live-corrective-20260718T1928-afterfix/` were
    black or lock screen, not WebUI.
  - `dumpsys power` reported `mWakefulness=Dozing` after timeout; `dumpsys
    window` reported `KeyguardServiceDelegate showing=true secure=true` and
    `screenState=SCREEN_STATE_OFF`.
  - Freehand activity/process was not a valid foreground UI proof; `dumpsys
    activity top` showed `com.freehand.android has been frozen`.
  - Standard verifier:
    `FREEHAND_ANDROID_SKIP_INSTALL=1 FREEHAND_ANDROID_ARTIFACT_DIR=artifacts/android-device/live-corrective-20260718T1928-afterfix-verify-locked apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555`
    exited 2 with `blocked: device is locked/dozing`.
- current conclusion:
  - APK/build/config/relay/local gates are verified.
  - The required real-device UI interaction closeout is still open until the
    phone is unlocked and kept awake long enough to tap Config, scroll/focus the
    provider form, and prove Android Back closes WebUI state instead of exiting.

# 2026-07-18 Worker tool-turn transcript render closeout

- trigger:
  - Worker/Master conversation only rendered the final summary for a completed
    Worker session; every intermediate tool-call reasoning turn was missing
    from the visible transcript.
- root evidence:
  - ADP `QuerySessionTurns` for `worker-task-task-1784351742` previously showed
    only final `worker-turn-exec-worker-worker-1784351747370675000-45119-r35`
    with no `tool_activities`.
  - Reason ledger
    `~/.freehand/ledgers/reason/worker/worker-task-task-1784351742.jsonl`
    contained 35 actual turn ids and provider/tool activity, while authoritative
    closed snapshots had only the final continuation file.
  - A first attempted full ledger backfill inside daemon bootstrap made S daemon
    startup parse large historical ledgers; `~/.freehand/ledgers/reason/worker`
    was about 18G and startup sampled inside
    `restore_all_persisted_sessions_into_ui -> restore_turn_snapshots_for_ui ->
    load_reason_ledger`.
- fix:
  - `ReasonPersistence::restore_turn_snapshots_for_ui` preserves exact turn ids
    instead of coalescing `runtime-turn-N` / `runtime-turn-N-rM` by logical key,
    and backfills incomplete authoritative snapshots from reason-ledger truth.
  - New `ReasonPersistence::restore_authoritative_turn_snapshots_for_ui` is the
    daemon bootstrap path; it reads only authoritative closed/active snapshots
    and never scans every historical reason ledger.
  - `restore_all_persisted_sessions_into_ui` now calls the authoritative-only
    API, while selected `QuerySessionTurns` keeps the heavier exact-round ledger
    backfill path.
  - Function maps, test designs, generated wiki, and `freehand-dev` skill were
    updated to lock: bootstrap is lightweight; selected transcript query is
    exact per-round and must not collapse tool rounds.
- local proof:
  - `cargo test -p freehand-reason ui_restore_ -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bootstrap_restores_multiround_turns_as_separate_ui_cards -- --nocapture` passed after switching the assertion to runtime `QuerySessionTurns`.
  - `cargo test -p freehand-runtime live_bootstrap_does_not_replay_incomplete_historical_reason_ledgers -- --nocapture` passed and proves incomplete historical snapshots plus a poisoned ledger no longer block bootstrap.
  - `node --check apps/freehand-server/assets/webui.js`, `cargo fmt --check`,
    `cargo test -p freehand-server --lib root_and_asset_routes_return_webui_shell_files -- --nocapture`,
    `cargo run -p xtask -- mainlines generate`, `mainlines check`,
    `gates check`, and `git diff --check` passed.
- online S-profile proof:
  - `scripts/install-launchd.sh restartS` rebuilt and service-scoped restarted
    S; launchd PID changed from the stuck old `57397` to `50117`.
  - Health `http://127.0.0.1:4042/health` returned `ok`.
  - Config remained `provider=cc provider_type=openai
    provider_protocol=responses base_url_host=api.anyint.ai
    default_model=gpt-5.5 auth_source=env`; fixture env grep over daemon and
    Worker S env files returned 0 matches.
  - ADP `QueryTaskBoard` for `task-1784351742` projected
    `worker_session_id=worker-task-task-1784351742` under parent
    `webui-session-20260718051445-b7657881`.
  - ADP `QuerySessionTurns` for that Worker returned `turn_count=35`,
    `tool_turn_count=34`, `total_tool_activities=36`, no user-text leak, and
    tool summaries including `todo_write`, `ls`, `grep`, and `read_file`.
  - WebUI mobile Header/Agent sheet verifier opened the Worker card, selected
    the canonical Worker session, returned to the parent, and found
    `userMessageCount=0` / `fakePromptVisible=false`.
  - DOM proof artifact
    `artifacts/webui-online/tool-turn-render-1784351742-dom/summary.json`
    showed selected session `worker-task-task-1784351742`, selected turn
    `...-r35`, `.chat-section-tool` `toolCardCount=36`,
    `successToolCardCount=36`, and semantic summaries for `List directory`,
    `Read file`, `Search text`, and `Update plan`.
- residual:
  - Worktree remains broadly dirty from unrelated in-progress features; this
    closeout does not clean or revert those files.
# 2026-07-18 Live active turn observability regression

- User-visible regression: a submitted Master request flashes dispatching, then UI returns to a prior-looking transcript and silently stops instead of showing provider retry/waiting/tool progress.
- Owner evidence: `webui-session-20260718051445-b7657881` has active `runtime-turn-411`; `active-turn.json` exists; ADP session status is `waiting_model`; error-center for the turn has recoverable provider retry rows (`openai_http_request_failed`, retry_same_step), but `QuerySessionTurns` projected `model_request=null`, `tool_count=0`, `terminal_status=null`.
- Root direction: selected-session transcript refresh replaces `UiProtocolState` session turns from reason persistence. Active turn persistence does not contain live-only `model_request`/debug retry activity, so refresh clears the UI-visible waiting/retry state. Fix should preserve same-turn nonterminal live activity during transcript replacement and keep terminal snapshots authoritative.

# 2026-07-18 Live tool-call round render closeout

- trigger:
  - User-visible WebUI execution flashed `dispatching`, then silently stopped or showed only the final summary while the runtime was still doing tool/model continuation work.
- fix:
  - `UiProtocolState::replace_session_turn_projections` preserves nonterminal same-turn live `model_request` and `tool_activities` across selected transcript refreshes, while terminal replacements stay authoritative and clear stale live activity.
  - Runtime-backed `QuerySessionTurns` now preserves active live provider retry and tool activity projections.
  - WebUI render lifecycle now treats terminal status/text and `ToolPending` as authoritative before submit-in-flight/tool/model waiting state; final rows cannot remain labeled `dispatching`, and `ToolPending` remains lifecycle/running instead of current-live model wait.
  - The online verifier `scripts/verify-webui-live-tool-render-online.mjs` uses fixed persisted session `webui-live-tool-render-fixed`, captures the pre-run transcript, scopes all DOM/ADP assertions to the current run marker and new turn ids, and avoids putting the exact final marker in the user prompt.
- online S-profile proof:
  - Artifact: `artifacts/webui-online/live-tool-render-1784384733439/summary.json`.
  - Current run marker `live-tool-render-1784384733439`; task path marker `definitely-missing-live-tool-render-live-tool-render-1784384733439`; final assistant marker `live tool render completed live-tool-render-1784384733439`.
  - Current-run ADP turns were `runtime-turn-415` and `runtime-turn-415-r2`.
  - Fixture saw `requestCount=2`, `toolOutputRequestCount=1`, `toolSchemaIncluded=true`, and provider requests included only framework tools `task,timer`; second request contained the paired tool output.
  - During the first round the WebUI selected `runtime-turn-415` and rendered one current-run tool card with running state for `task`.
  - During continuation the WebUI selected `runtime-turn-415-r2`, kept the failed `task` tool card visible, and rendered model waiting/continuation rows before final.
  - Final DOM selected `runtime-turn-415-r2`, projected terminal `Success`, showed command/turn status `completed`, had `liveCount=0` and `currentRunLiveCount=0`, and had no residual `dispatching`, thinking, or tool-running status for the current run.
  - Final ADP truth: latest current-run turn `runtime-turn-415-r2` was `Success`; tool activity stayed attached to `runtime-turn-415` with count `1`.
- local proof:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `node --check scripts/verify-webui-live-tool-render-online.mjs` passed.
  - `cargo test -p freehand-ui-protocol session_refresh -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-runtime runtime_query_session_turns_preserves_live -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- restoration:
  - S-profile config restored to `provider=cc provider_type=openai provider_protocol=responses base_url_host=api.anyint.ai default_model=gpt-5.5 auth_source=env`.
  - `~/.freehand/daemonS.env` contained no `FREEHAND_LIVE_TOOL_RENDER_FIXTURE_KEY`, `FREEHAND_PROVIDER_RETRY_BACKOFF_MS`, or `FREEHAND_PROVIDER_RECOVERY_FIXTURE_KEY`.
  - Health `http://127.0.0.1:4042/health` returned `ok`.

# 2026-07-18 Live tool-call round render continuation reproof

- trigger:
  - After context compaction, the latest stored artifact was a failed
    `verify-webui-live-tool-render-online.mjs` run where the browser timed out
    before the current-run `task` tool card.
- failure evidence rechecked:
  - Failed artifact:
    `artifacts/webui-online/live-tool-render-1784389975204/failure/failure.json`.
  - The failure had `requestCount=0`, `toolOutputRequestCount=0`, and
    `toolSchemaIncluded=false`; metadata stopped at
    `RuntimeLive01ContextPlanningStarted` for `runtime-turn-416` with no
    `ContextPlanningCompleted`, no `ReasonReq02ContextComposedInput`, and no
    `RuntimeLive02ProviderRequestBuilt`.
  - During that failed run, `master.active-work.json` belonged to the live
    daemon PID at the time, so normal dead-owner stale cleanup was not expected
    to clear it while the process was still alive. After the verifier's
    service-scoped restart, `~/.freehand/state/master-loop/master.active-work.json`
    was absent.
- fresh online S-profile proof:
  - Command: `node scripts/verify-webui-live-tool-render-online.mjs`.
  - Passing artifact:
    `artifacts/webui-online/live-tool-render-1784390175397/summary.json`.
  - Fixed session: `webui-live-tool-render-fixed`.
  - Current run marker: `live-tool-render-1784390175397`.
  - Current-run ADP turns: `runtime-turn-416`, `runtime-turn-416-r2`.
  - Fixture proof: `requestCount=2`, `toolOutputRequestCount=1`,
    `toolSchemaIncluded=true`; second provider request contained the paired
    tool output.
  - Browser proof:
    - first round rendered a running `task` tool card for current-run
      `runtime-turn-416` before final response;
    - continuation rendered the failed `task` tool card plus
      `tool result returned: 1 failed / 1 total · waiting model`;
    - final selected `runtime-turn-416-r2`, terminal `Success`, current-run
      failed tool card still visible, `liveCount=0`,
      `currentRunLiveCount=0`, and `finalStatusStillDispatching=false`.
  - Metadata proof now includes `RuntimeLive01ContextPlanningCompleted`,
    `ReasonReq02ContextComposedInput`, `RuntimeLive02ProviderRequestBuilt`,
    `RuntimeLive03ToolExecuted`, and `RuntimeLive04TurnClosed` for
    `runtime-trace-416`.
- focused local proof:
  - `node --check scripts/verify-webui-live-tool-render-online.mjs` passed.
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `jq empty docs/mainline-calls/provider.reason-live-bridge.json docs/mainline-calls/runtime.ui-command-dispatch.json docs/mainline-calls/ui.protocol.json docs/mainline-calls/app.webui-smoke.json` passed.
  - `git diff --check` passed.
  - `cargo test -p freehand-runtime debug_projects_model_waiting_ui_state -- --nocapture --test-threads=1` passed 2/2.
  - `cargo test -p freehand-runtime live_bridge_runs_single_shot_anthropic_provider_into_turn_truth -- --nocapture --test-threads=1` passed 1/1.
  - `cargo test -p freehand-runtime runtime_query_session_turns_preserves_live -- --nocapture --test-threads=1` passed 2/2.
  - `cargo test -p freehand-ui-protocol session_refresh -- --nocapture --test-threads=1` passed 3/3.
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, and
    `cargo run -p xtask -- gates check` passed.
- restoration:
  - Final S-profile config query returned
    `provider=cc provider_type=openai provider_protocol=responses
    base_url_host=api.anyint.ai default_model=gpt-5.5 auth_source=env`.
  - `grep` for `FREEHAND_LIVE_TOOL_RENDER_FIXTURE_KEY`,
    `FREEHAND_PROVIDER_RETRY_BACKOFF_MS`, and
    `FREEHAND_PROVIDER_RECOVERY_FIXTURE_KEY` in `~/.freehand/daemonS.env`
    returned zero matches.
  - Health `http://127.0.0.1:4042/health` returned `ok`.
  - `~/.freehand/state/master-loop/master.active-work.json` was absent.
- residual:
  - The fixed session intentionally retains historical successful verifier turns
    `runtime-turn-412` through `runtime-turn-416-r2`; the verifier scopes proof
    to the current marker and newly added turn ids instead of treating historical
    turns as current evidence.

# 2026-07-19 Live tool/render S-profile final closeout after context split

- marker:
  - `live-tool-render-final-closeout-20260719`
- target:
  - Continue the prior closeout after `freehand-runtime/src/lib.rs` split and
    fixed-session WebUI live tool verifier failures.
  - No release `4041`, no historical session deletion, no broad kill.
- current source state:
  - `crates/freehand-runtime/src/turn_projection.rs` is split out and bound in
    `runtime.ui-command-dispatch`.
  - `crates/freehand-runtime/src/lib.rs` is now 8283 lines; the split module is
    522 lines.
  - Existing fixes already present: symlink-safe instruction capability scanning,
    early failed/cancelled live submit materialization, active-work owner PID
    recovery, and Master parent reconciliation using authoritative snapshots
    instead of selected transcript ledger backfill.
- online proof:
  - `node scripts/verify-webui-live-tool-render-online.mjs` passed on S-profile
    `127.0.0.1:4042`.
  - Artifact:
    `artifacts/webui-online/live-tool-render-1784426079812/summary.json`.
  - Fixed session `webui-live-tool-render-fixed`, current-run turns
    `runtime-turn-420` and `runtime-turn-420-r2`.
  - Fixture proof: `requestCount=2`, `toolOutputRequestCount=1`,
    `toolSchemaIncluded=true`; request 1 exposed only framework tools
    `task,timer`, request 2 included function-call output.
  - DOM proof: current-run tool card visible before final, continuation waiting
    visible, final selected `runtime-turn-420-r2`, terminal `Success`,
    `liveCount=0`, `currentRunLiveCount=0`,
    `finalStatusStillDispatching=false`.
- local proof:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `node --check scripts/verify-webui-live-tool-render-online.mjs` passed.
  - `cargo test -p freehand-instructions --lib -- --nocapture --test-threads=1`
    passed 7/7.
  - `cargo test -p freehand-runtime instruction_capability -- --nocapture
    --test-threads=1` passed 1/1.
  - `cargo test -p freehand-runtime runtime_live_submit_ -- --nocapture
    --test-threads=1` passed 3/3.
  - Parent reconciliation/idempotency/turn-start focused runtime tests passed
    3/3.
  - `cargo test -p freehand-runtime debug_projects_model_waiting_ui_state --
    --nocapture --test-threads=1` passed 2/2.
  - `cargo test -p freehand-runtime runtime_query_session_turns_preserves_live
    -- --nocapture --test-threads=1` passed 2/2.
  - `cargo test -p freehand-ui-protocol session_refresh -- --nocapture
    --test-threads=1` passed 3/3.
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files
    -- --nocapture` passed.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`,
    `cargo run -p xtask -- mainlines check`,
    `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- restoration:
  - S health returned `ok`.
  - Final config query returned `provider=cc provider_type=openai
    provider_protocol=responses base_url_host=api.anyint.ai
    default_model=gpt-5.5 auth_source=env`.
  - Fixture-env grep over actual S env files returned zero matches.
  - `~/.freehand/state/master-loop/master.active-work.json` is absent.
  - `~/.local/bin/freehand-daemonS-bin` and `target/debug/freehand-daemon`
    hash matched:
    `28e57c20852b11098628146c1709cc643a55cac57c02a0532f431e8131a3accd`.
- residual:
  - Worktree remains broadly dirty with unrelated Android/relay/docs/runtime
    files and untracked `output/`; this closeout does not clean or revert them.

# 2026-07-19 Android WebView stale live projection closeout

- trigger:
  - PLZ110 Android WebView showed `provider retry... 3h 55m` on `runtime-turn-423` with `workspaceStatus=closed` and `liveCount=1`, while ADP owner truth for `webui-session-20260719051023-357f19ac` was terminal success through `runtime-turn-429` and `task-1784437931` was closed.
- root cause:
  - Phone was loading the Tailscale relay URL `http://100.66.1.82:44042/relay/daemon/studio-host/`.
  - The old relay process was manually started outside launchd, so S-profile `restartS` did not manage it.
  - The page asset URL still used `webui.js?v=20260718-header-tree-actions`, so Android WebView could keep the old JS cached even after the server-side reconnect/watchdog code existed.
- fix:
  - Added `scripts/install-relay-launchd.sh` and wired `scripts/install-launchd.sh restartS` / `installS` to restart `com.freehand.relayS` after `com.freehand.daemonS`.
  - `relayS` binds `100.66.1.82:44042`, registers `studio-host` to `http://127.0.0.1:4042`, and writes env/log truth under `~/.freehand`.
  - Bumped WebUI asset version to `20260719-mobile-live-reconnect` in `apps/freehand-server/src/page.rs` and `apps/freehand-server/assets/webui.js`.
  - Updated `app.runtime-daemon` docs/test design/feature map and `freehand-dev` skill so Android proof checks relay-served asset version plus true-device DOM.
- proof:
  - Before reload DOM: selected session `webui-session-20260719051023-357f19ac`, selected turn `runtime-turn-423`, script `?v=20260718-header-tree-actions`, `workspaceStatus=closed`, `turnStatus=provider retry... 3h 55m`, `liveCount=1`.
  - After CDP bypass-cache reload: selected turn `runtime-turn-429`, script/resource `?v=20260719-mobile-live-reconnect`, `workspaceStatus=connected`, `turnStatus=completed`, `commandStatus=completed`, `liveCount=0`, `hasProviderRetry=false`.
  - Screenshot: `artifacts/android-device/live-hang-20260719T-after-fix/screen.png` shows completed final answer and `0 running · 1 closed task`.
  - Android verifier with installed APK passed: `FREEHAND_ANDROID_SKIP_INSTALL=1 apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555`, artifact `artifacts/android-device/20260719T091258Z-100.104.163.65_5555-61229/summary.json`.
  - Focused gates passed: JS/shell syntax, `freehand-server` asset route test, `freehand-server remote_relay`, `scripts/verify-remote-relay-local-online.sh`, `cargo fmt --check`, `xtask mainlines generate/check`, `xtask gates check`, and `git diff --check`.
- residual:
  - Full Android install verifier without `FREEHAND_ANDROID_SKIP_INSTALL=1` is blocked by local `apkanalyzer`/`JAVA_HOME`, matching the known verifier blocker; the already installed APK/UI path was verified.

# 2026-07-19 Android WebView current-hang challenge reproof

- trigger:
  - Jason reported the mobile version was still hung for three hours, so the prior closeout could not stand on backend truth or a one-off manual CDP reload alone.
- current true-device checks:
  - Foreground activity was `com.freehand.android/.ui.MainActivity`, unlocked, Freehand PID `15873` before verifier; app-owned config used relay `http://100.66.1.82:44042/relay/daemon/studio-host/`.
  - Relay HTML served `webui.js?v=20260719-mobile-live-reconnect` and `theme/webui.css?v=20260719-mobile-live-reconnect`; relay ADP smoke passed.
  - Current foreground Freehand CDP DOM before app relaunch already showed `selectedSession=webui-session-20260719051023-357f19ac`, `selectedTurn=runtime-turn-429`, `workspaceStatus=connected`, `turnStatus=completed`, `commandStatus=completed`, `liveCount=0`, `hasProviderRetry=false`; screenshot saved at `artifacts/android-device/live-hang-20260719T-current/screen.png`.
  - The other WebView devtools socket belonged to `com.zterm.android`, not Freehand, so no second stale Freehand page was hidden.
- app-level relaunch proof:
  - `FREEHAND_ANDROID_SKIP_INSTALL=1 apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` passed and force-stopped/restarted the app; artifact `artifacts/android-device/20260719T092146Z-100.104.163.65_5555-3373/summary.json`.
  - New Freehand PID was `17450`; logcat showed relay stylesheet URLs with `?v=20260719-mobile-live-reconnect`, `webuiShell=true`, `webuiCssApplied=true`, and `webuiJsReady=true`.
  - Post-relaunch CDP DOM on PID `17450` showed `selectedTurn=runtime-turn-429`, `selectedTerminalStatus=success`, `workspaceStatus=connected`, `turnStatus=completed`, `commandStatus=completed`, `liveCount=0`, `hasProviderRetry=false`, and final text `贵州兴义八月初天气与适宜景点的联网资料整理任务已完成，最终攻略已交付用户，无需后续等待或重试。`.
  - Post-relaunch screenshot saved at `artifacts/android-device/live-hang-20260719T-after-restart/screen.png`.
- owner truth:
  - Relay ADP session query returned `webui-session-20260719051023-357f19ac:17:success` through `runtime-turn-429`.
  - Relay TaskHistory for `task-1784437931` returned 281 events ending `TaskReviewSubmitted,TaskReviewApproved,TaskClosed`.
- durable rule:
  - If the phone was already foreground when a WebUI/relay stale-live fix lands, Android acceptance must include app-level relaunch plus CDP reattachment to the new Freehand PID. Manual CDP reload proof is insufficient for user-visible recovery.

# 2026-07-19 WebUI live request-cycle card audit reproof

- trigger:
  - Jason restated the UI contract: one request/model round cycle is one chronological card; the card may update while live, must show tool calls/results/status in real time, and must freeze once that cycle completes. Later cycles must append as new cards and must not mutate old cards.
- audit result:
  - Current WebUI code uses `.turn-cycle-card` parents, stable `cycleKey` (`submit_id` or `session_id + turn_id`), `renderConversationFragments`, and `reconcileCycleCardFragments`.
  - Terminal cycle cards set `data-frozen="true"` and are reused during reconciliation; the transcript no longer uses a bare `messageList.replaceChildren();` full rebuild for normal cycle-only renders.
  - The latest failed artifact `artifacts/webui-online/live-tool-render-1784462576772/failure/failure.json` had `requestCount=0`, `toolOutputRequestCount=0`, and metadata stopped at `RuntimeLive01ContextSegmentStarted` for `instruction-capability`; that failure was pre-provider/runtime context admission, not evidence that tool-card DOM reconciliation failed.
- online S-profile proof:
  - `node scripts/verify-webui-live-tool-render-online.mjs` passed with artifact `artifacts/webui-online/live-tool-render-1784464051115/summary.json`.
  - Fixed session `webui-live-tool-render-fixed`; current run turns `runtime-turn-444` and `runtime-turn-444-r2`.
  - Provider fixture saw `requestCount=2`, `toolOutputRequestCount=1`, and `toolSchemaIncluded=true`.
  - Browser stages: `duringTool` selected `runtime-turn-444` with one current-run running `task` tool card ordered after the user cycle; `duringContinuation` selected `runtime-turn-444-r2` with the failed tool card plus model waiting; `finalState` selected `runtime-turn-444-r2`, terminal `success`, two current-run cycle cards, frozen count 2, live count 0, final dispatching false.
- local proof:
  - `node --check apps/freehand-server/assets/webui.js`.
  - `node --check scripts/verify-webui-live-tool-render-online.mjs`.
  - `jq empty docs/mainline-calls/app.webui-smoke.json docs/mainline-calls/instruction.capability-loader.json docs/mainline-calls/provider.reason-live-bridge.json`.
  - `cargo test -p freehand-instructions --lib -- --nocapture --test-threads=1` passed 7/7.
  - `cargo test -p freehand-runtime instruction_capability -- --nocapture --test-threads=1` passed 1/1 after waiting for cargo lock.
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture` passed.
  - `cargo test -p freehand-ui-protocol session_refresh -- --nocapture --test-threads=1` passed 3/3.
  - `cargo test -p freehand-runtime runtime_query_session_turns_preserves_live -- --nocapture --test-threads=1` passed 2/2.
  - `cargo test -p freehand-runtime debug_projects_model_waiting_ui_state -- --nocapture --test-threads=1` passed 2/2.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- restoration:
  - Final config query returned `provider=cc provider_type=openai provider_protocol=responses base_url_host=api.anyint.ai default_model=gpt-5.5 auth_source=env`.
  - Fixture env grep over S daemon/worker env files returned zero matches.
  - `http://127.0.0.1:4042/health` returned `ok`.
  - `~/.freehand/state/master-loop/master.active-work.json` was absent.
- durable verifier note:
  - In `verify-webui-live-tool-render-online.mjs` summary, `serviceTurn` is an early pre-tool snapshot and may legitimately still show live/dispatching; final acceptance must be read from `duringTool`, `duringContinuation`, and `finalState`.

# 2026-07-19 Provider registry Settings UI closeout

- trigger:
  - Jason required that users can switch provider and add new provider configs from UI; `minimax` must remain in registry while active provider may be `cc`.
- implementation notes:
  - Existing owner chain was already landed in config/protocol/runtime/WebUI/CLI from the handoff.
  - Fixed WebUI selector draft handling: fallback/current selectors now follow `QueryConfigStatus` until the operator changes a selector; provider definition upsert does not clear fallback selection.
  - Fixed `scripts/verify-provider-registry-ui-online.mjs`: CDP `Runtime.evaluate` now injects browser helper source and writes DOM/page-event debug artifacts on failure.
- online proof:
  - `FREEHAND_PROVIDER_REGISTRY_UI_DEBUG_PORT=9262 node scripts/verify-provider-registry-ui-online.mjs` passed.
  - Artifact: `artifacts/webui-online/provider-registry-ui-1784472971174/summary.json`.
  - Initial DOM: `currentProvider=cc`, `fallbackSelectValue=minimax`, provider ids `cc,minimax`.
  - After upsert: provider ids `cc,minimax,ui-verify-provider-registry`; `currentProvider=cc`; `fallbackSelectValue=minimax`.
  - After explicit switch: `currentProvider=minimax`, fallback cleared for the proof, registry still included `cc,minimax,ui-verify-provider-registry`.
  - Final restore: `provider=cc`, `fallback=minimax`, registry `cc,minimax`, fixture env grep 0, health `ok`.
- local proof:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `node --check scripts/verify-provider-registry-ui-online.mjs` passed.
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture` passed.
  - `cargo test -p freehand-config provider -- --nocapture --test-threads=1` passed 15/15.
  - `cargo test -p freehand-ui-protocol provider_config -- --nocapture --test-threads=1` passed 3/3.
  - `cargo test -p freehand-runtime runtime_query_projects_config_status_without_secrets -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-runtime runtime_dispatch_upserts_provider_registry_without_switching_active_selection -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-runtime runtime_dispatch_switches_agent_provider_selection_without_hot_reload -- --nocapture --test-threads=1` passed.
  - `cargo check -p freehand-cli`, `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- residual:
  - Worktree remains broadly dirty with unrelated Android/relay/runtime/doc changes and untracked `output/`; this closeout did not revert or clean unrelated state.

# 2026-07-20 Master/Worker closed-loop lifecycle proof

- marker:
  - `closed-loop-lifecycle-20260720`
- trigger:
  - Jason要求任务、Worker、retry/failure/recovery、Master监督各自都闭环，不能因为状态不可感知导致Master死等。
- root cause fixed in this slice:
  - Parent workset previously grouped children by exact `runtime-turn-N-rM`; a child closed from the first exact round could start parent evaluation while later same-logical-turn sibling tasks were still open.
  - Background Master lifecycle/provider retry visibility needed shared UI-state hooks and query-time ErrorCenter recovery so retry has owning session/turn evidence.
- implementation:
  - `runtime.master-worker-loop` groups parent children by logical Master turn ordinal: `runtime-turn-N`, `runtime-turn-N-r2`, and later rounds from the same user request are one workset.
  - Master mode wires `ProductionMasterRunner` to the same `UiProtocolState` as the runtime dispatcher, so background lifecycle, parent evaluation, and timer wakeup turns can publish reason/debug/task-list state.
  - `QuerySessionTurns` restores background provider retry/failover/schema waiting from ErrorCenter metadata only for the latest nonterminal turn and refuses to reactivate terminal or historical retry rows.
  - Added closed-loop lifecycle contract to `docs/testing/runtime.master-worker-loop.md`: Task/Execution, Worker process/Agent resource, Master supervisor attention, and Parent user session/workset each require owner truth, next action, observable projection, and verification entrance.
- online proof:
  - Provider recovery WebUI verifier passed at `artifacts/webui-online/provider-recovery-20260720T143541-39583/summary.json`: fixed session `webui-session-20260720143617-e5783025`, turn `runtime-turn-463`, `requestCount=3`, `retryObserved=true`, final `Success`, no persistent error card, no live rows.
  - Three-Worker online verifier passed on isolated `ws://127.0.0.1:4142/adp`: session `online-master-three-worker-evaluation-1784558664`, final `master_three_worker_e2e_ok`, final `runtime-turn-3 Success`.
  - Online task proof: alpha closed normally; beta history included `TaskReviewRejected -> TaskAssigned -> TaskResumed -> TaskReviewSubmitted -> TaskReviewApproved -> TaskClosed`; gamma stayed same task through `TaskInterrupted -> TaskAssigned(worker-alpha) -> TaskResumed -> TaskReviewSubmitted -> TaskReviewApproved -> TaskClosed`; integration task closed after first parent evaluation created next-round work.
  - Online AgentBoard proof: all Workers finished `idle` with `current_task_id=null`; worker-gamma showed `alive=false` after TTL in offline phase, then restarted with new pid `8730` and `restart_count=1`.
- local proof:
  - `cargo test -p freehand-runtime production_master_runner -- --nocapture --test-threads=1` passed 27/27.
  - `cargo test -p freehand-runtime runtime_query_session_turns_ -- --nocapture --test-threads=1` passed 7/7.
  - `cargo test -p freehand-runtime provider_retry -- --nocapture --test-threads=1` passed 4/4.
  - `cargo test -p freehand-runtime live_bridge_retries_recoverable_provider_errors_then_succeeds -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-task agent_process -- --nocapture` passed 2/2.
  - `cargo build -p freehand-daemon`, `node --check` for WebUI/provider verifier, `cargo fmt --check`, `cargo run -p xtask -- mainlines generate/check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- restoration:
  - S-profile final config: `provider=cc`, `fallback_provider=minimax`, `provider_type=openai`, `provider_protocol=responses`, `base_url_host=api.anyint.ai`, `default_model=gpt-5.5`, `auth_source=env`.
  - Fixture env grep for provider recovery/retry/live-tool/autonomy markers returned zero matches.
  - Health `http://127.0.0.1:4042/health` returned `ok`.
- residual:
  - Worktree remains broadly dirty with unrelated prior changes and untracked `.agent-collab/`, `.DS_Store`, `output/`, and `scripts/__pycache__/`; this slice did not clean or commit them.

# 2026-07-20 Closed-loop lifecycle verification refresh

- marker:
  - `closed-loop-lifecycle-20260720-refresh`
- verifier hardening:
  - A fresh provider recovery run first exposed a verifier gap: ADP already returned `Success`, but the captured DOM still had `selectedTerminalStatus=""`, stale `provider retry` text, and `liveCount=2`.
  - `scripts/verify-provider-recovery-webui-online.mjs` now treats DOM terminal success and zero `[data-live="true"]` rows as hard acceptance gates, not informational fields.
- online proof:
  - Hardened provider recovery verifier passed at `artifacts/webui-online/provider-recovery-20260720T164050-67308/summary.json`.
  - Provider recovery session `webui-session-20260720164106-bb045bf4`, turn `runtime-turn-466`, `requestCount=3`, `retryObserved=true`, ADP `Success`, DOM `selectedTerminalStatus=success`, `liveCount=0`, no persistent provider retry or error text.
  - Three-Worker verifier passed with `master_three_worker_e2e_ok url=ws://127.0.0.1:4142/adp session=online-master-three-worker-evaluation-1784565725`.
  - Initial child tasks: `task-three-worker-1781784565725-alpha:worker-alpha`, `task-three-worker-1781784565725-beta:worker-beta`, `task-three-worker-1781784565725-gamma:worker-gamma`.
  - Next-round integration task: `task-three-worker-1781784565725-integration:worker-alpha`.
  - Beta history included `TaskReviewRejected -> TaskAssigned -> TaskResumed -> TaskReviewSubmitted -> TaskReviewApproved -> TaskClosed`.
  - Gamma stayed same task id and recovered through `TaskInterrupted -> TaskAssigned -> TaskResumed -> TaskReviewSubmitted -> TaskReviewApproved -> TaskClosed`, with takeover assignee `worker-alpha`.
  - Final parent turn `runtime-turn-3` was `Success` and final text contained all four worker result markers.
  - AgentBoard final state had `worker-alpha`, `worker-beta`, and `worker-gamma` all `idle` with `current_task_id=null` and `current_execution_id=null`; gamma offline phase showed `alive=false` and restart phase showed new pid `93535` with `restart_count=1`.
  - Final idempotency evidence: `final_evaluation_count=1`, `final_evaluation_turn_id=runtime-turn-3`, `restart_idempotent=true`.
- local proof:
  - `cargo test -p freehand-runtime production_master_runner_groups_parent_workset_by_logical_turn_rounds -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-ui-protocol session_list_active_turn_id_tracks_only_nonterminal_turns -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-task agent_process -- --nocapture` passed 2/2.
  - `cargo test -p freehand-runtime production_master_runner -- --nocapture --test-threads=1` passed 27/27.
  - `cargo test -p freehand-runtime runtime_query_session_turns_ -- --nocapture --test-threads=1` passed 7/7.
  - `cargo test -p freehand-runtime provider_retry -- --nocapture --test-threads=1` passed 4/4.
  - `cargo test -p freehand-runtime live_bridge_retries_recoverable_provider_errors_then_succeeds -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-runtime production_worker_runner -- --nocapture --test-threads=1` passed 20/20.
  - `node --check apps/freehand-server/assets/webui.js` and `node --check scripts/verify-provider-recovery-webui-online.mjs` passed.
  - `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, `jq empty` for touched mainline JSON, and `git diff --check` passed.
- restoration:
  - `http://127.0.0.1:4042/health` returned `ok`.
  - S-profile config restored to `provider=cc`, `fallback_provider=minimax`, `provider_type=openai`, `provider_protocol=responses`, `base_url_host=api.anyint.ai`, `default_model=gpt-5.5`, `auth_source=env`, registry `cc,minimax`.
  - Fixture env grep over existing S env files returned zero matches for provider recovery/retry/backoff/live-tool/autonomy/three-worker markers.
- residual:
  - Full `cargo test --workspace` was not rerun in this refresh; previous known blockers remain separate owner issues: `freehand-blocks` harness hang and stale daemon checkpoint tests against the Master framework-only tool contract.
  - Android true-device was not rerun for this lifecycle refresh.
  - Untracked `.agent-collab/`, `.DS_Store`, `output/`, and `scripts/__pycache__/` remain untouched.

# 2026-07-21 Provider retry same-turn UI flow audit

- trigger:
  - Jason clarified provider retry is only an internal provider resend, not a new user request.
- diagnosis:
  - Runtime retry loop stays inside one `TurnRecord`; `retry_index` increments inside the provider executor loop and does not call `ReasonTurnEngine::start_turn`.
  - `runtime-turn-N-rM` is only created for actual model continuation rounds such as tool result/schema repair/continue, not for provider wire resend.
  - WebUI had an invalid local merge contract: `sameRenderableTurn()` required matching `turn_id` and matching visible `user_text`.
  - A live debug retry projection may arrive before transcript restore and therefore has `model_request` but no `user_text`; the later transcript projection for the same `turn_id` has the user text. The extra `user_text` equality can make one provider retry turn render as two local render turns/cards.
- owner:
  - First polluted node: WebUI local render merge for selected session projections.
  - Unique owner: `app.webui-smoke` thin protocol consumer, `apps/freehand-server/assets/webui.js`.
  - Runtime/protocol remain truth owners for retry state; WebUI must not classify retry from raw provider text.
- planned fix:
  - Merge local render turns by stable `session_id + turn_id` identity only.
  - Keep continuation-round hidden user text owned by runtime projection, not WebUI text guessing.
  - Add a server/webui static regression asserting same-turn merge no longer depends on `user_text`.

# 2026-07-21 Provider retry transport-substate closeout

- trigger:
  - Jason clarified that provider resend is only provider transport retry; it is unreasonable for it to appear as a reasoning-flow step.
- fix:
  - `UiModelRequestActivity` now carries provider retry/failover as `transport` substate while the main `kind` remains `Thinking`.
  - WebUI renders the current model request row as `Model`; retry details appear only as `transport retry: provider retry ...` inside the same turn card, and `modelRequestTimingKey` no longer includes retry attempt/detail.
  - WebUI verifier now fails if a selected retry turn renders a standalone `Provider` flow label.
- local proof:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `node --check scripts/verify-provider-recovery-webui-online.mjs` passed.
  - `cargo test -p freehand-ui-protocol provider_recovery_activity_updates_in_place_and_clears_on_response -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-ui-protocol session_refresh_preserves_active_model_request_activity -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-ui-protocol terminal_session_refresh_drops_stale_live_activity -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-runtime runtime_query_session_turns_preserves_live_provider_retry_activity -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-runtime runtime_query_session_turns_projects_background_provider_retry_from_error_center -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-runtime runtime_query_session_turns_does_not_reactivate_terminal_error_center_retry -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-runtime runtime_query_session_turns_does_not_reactivate_historical_retry_before_later_terminal_round -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-runtime provider_recovery_debug_updates_same_turn_activity -- --nocapture --test-threads=1` passed.
  - `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture` passed.
  - `cargo check -p freehand-cli`, `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- online proof:
  - `FREEHAND_PROVIDER_RECOVERY_BACKOFF_MS=750 node scripts/verify-provider-recovery-webui-online.mjs` passed on S-profile `ws://127.0.0.1:4042/adp`.
  - Artifact: `artifacts/webui-online/provider-recovery-20260721T091025-29669/summary.json`.
  - Fixed session: `webui-provider-recovery-fixed`; selected turn: `runtime-turn-477`; fixture requests: `requestCount=3`.
  - Retry DOM proof: `selectedTurnCycleCount=1`, `selectedTurnUserCardCount=1`, `duplicateCycleKeys=[]`, `providerRetryFlowLabelPresent=false`, detail row text contains `Model` plus `transport retry: provider retry 1/10 ... wait 750ms before internal resend ... raw_hash=88a4753071b254a4`.
  - Final DOM/ADP proof: selected terminal status `success`, ADP terminal `Success`, `liveCount=0`, no final provider retry text, no ADP errors.
  - ErrorCenter turn query for `runtime-turn-477` returned `count=2` with two provider `retry_same_step` rows for `openai_http_status_500`.
  - Final S config restored to `provider=minimax fallback_provider=cc provider_protocol=messages base_url_host=api.minimaxi.com default_model=MiniMax-M3 auth_source=inline`; fixture/test env grep returned 0 matches; health returned `ok`.

# 2026-07-22 Android APK update Settings bridge

- trigger:
  - Settings had no user-facing APK update entry, while the Android updater already checked `/android/update.json` only on app startup.
- owner path:
  - `android_apk_update` remains `app.android-client` truth for manifest comparison, APK cache download, and FileProvider system-installer handoff.
  - `app.webui-smoke` owns only the daemon WebUI Settings card, native bridge call, status rendering, and asset version bump.
- implementation:
  - WebUI Settings now renders `Android APK update` with `Check APK update`, update source, phase/status text, desktop/browser unavailable state, and Android-WebView bridge callback handling.
  - Android `MainActivity` exposes `FreehandAndroidApkUpdate.check`, reuses the single `AndroidApkUpdater`, records the latest `ApkUpdateStatus`, and replays status after page load so startup and manual checks stay observable.
  - `AndroidApkUpdater` emits stable phases: `checking`, `current`, `available`, `downloading`, `downloaded`, `installer_started`, `failed`, and `already_checking`; duplicate checks are blocked by one `AtomicBoolean`.
  - WebUI asset version was bumped to `20260722-android-apk-update` so Android WebView cannot keep the previous Settings JS.
- proof:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `cd apps/freehand-android && JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest assembleDebug` passed.
  - `cargo test -p freehand-server --lib -- --nocapture` passed 15/15; the dispatch-worker panic is the intentional join-failure negative test.
  - `cargo run -p xtask -- mainlines generate`, `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
  - Local Chrome/CDP Android-WebView bridge simulation passed with artifact `artifacts/android-apk-update-settings-local/2026-07-21T23-18-28-865Z/summary.json`: button enabled, script `webui.js?v=20260722-android-apk-update`, one bridge call, final card `installer_started`, status contained `versionName=0.4.2` and `bytes=123456`.
- blocker:
  - True-device closure is not complete because `adb devices` is empty; `adb connect 100.104.163.65:5555` timed out and `adb connect 100.107.194.67:33039` was refused.

# 2026-07-22 Android install-time file-access prompt

- user contract:
  - File/media and broad external-storage permissions must be requested centrally on the first app start after every install or update, not lazily when a later file operation runs.
- owner and implementation:
  - `app.android-client` owns `android_file_access`.
  - `MainActivity::requestInstallFileAccessIfNeeded` runs before APK update checking and WebView navigation.
  - Prompt admission keys off package `lastUpdateTime`, not `versionCode`; a same-version reinstall therefore creates a new install marker while ordinary app starts do not repeat the prompt.
  - Android 11+ all-files access is handed to package-scoped system settings because a normal application cannot silently grant `MANAGE_EXTERNAL_STORAGE`.
  - `FreehandFileAccess` logcat rows carry phase, versionCode, installMarker, runtime permission state, and all-files state.
- local proof:
  - Android `testDebugUnitTest assembleDebug` passed.
  - WebUI and verifier JavaScript syntax checks passed.
  - `freehand-ui-protocol` passed 64/64 and `freehand-server --lib` passed 15/15.
  - `cargo fmt --check`, mainlines check, gates check, and `git diff --check` passed.
- online/restoration:
  - S-profile config remained `minimax/MiniMax-M3` with fallback `cc`; fixture env grep returned zero matches.
- blocker:
  - True-device install/permission closure is not complete: `adb devices` is empty, `100.107.194.67:45099` refused connection, and `100.104.163.65:5555` timed out.

# 2026-07-22 Worker tool failure guidance diagnosis

- trigger:
  - Jason reviewed currently running Worker tasks and saw many tool failures. The expected behavior is that the model should receive enough framework/tool/path guidance to make correct first calls instead of probing formats or inventing tools.
- live evidence:
  - S-profile `ws://127.0.0.1:4042/adp` currently lists selected session `webui-session-20260721052724-8fab5f9e` as `toolpending`.
  - Worker reason ledger `~/.freehand/ledgers/reason/worker/worker-task-task-1784612160.jsonl` has `70 Success / 2 Failed`; failed calls were external-parent `ls` attempts on `/Users/fanzhang` and `/Users/fanzhang/Documents/github`.
  - Worker reason ledger `~/.freehand/ledgers/reason/worker/worker-task-task-1784694444.jsonl` has `142 Success / 20 Failed`; failed calls include external-parent `ls`, repeated missing `reports` reads/lists, directory-as-file `read_file`, missing guessed files, unknown `shell`, and unknown `readlink`.
- SOP/model flow:
  - Known flow: `provider.reason-live-bridge` builds Worker live context and exports `tool.registry` Worker schemas; provider output tool calls re-enter `BuiltinToolRegistry::execute`; failed tool results are paired back to the model for continuation.
  - Resource edges: `request_context -> provider_request -> provider_response -> tool_call -> workspace_path -> tool_result`.
  - Forbidden shortcut: Worker may not escape locked task cwd for read/search/write path tools; Master/Worker lifecycle should not be repaired by prompt-only fallback or UI hiding.
- hypotheses:
  - H1 confirmed: Worker runtime guidance says read-only tools may inspect external paths, but `tool.registry` `resolve_read_path` rejects external absolute paths after canonical/symlink resolution. Boundary error text repeats the same false external-read allowance and labels read/list failures as `Write boundary denied`.
  - H2 supporting: Worker guidance says shell is unavailable, but does not list the exact Worker tool surface or explicitly forbid invented `shell/readlink/pwd/cat/find`; unknown-tool error gives no recovery pattern.
  - H3 supporting: repeated directory-as-file and guessed-report reads show schema/guidance needs stronger `ls` before `read_file` and missing generated-output handling.
- first divergence:
  - `crates/freehand-runtime/src/live_context.rs::worker_execution_guidance` is the first polluted model-visible instruction.
  - `crates/freehand-runtime/src/lib.rs::registry_error_text` is the downstream polluted re-entry text after the tool owner returns boundary/unknown-tool truth.
- unique owner and allowed paths:
  - `provider.reason-live-bridge`: `crates/freehand-runtime/src/live_context.rs`, `crates/freehand-runtime/src/lib.rs`, focused runtime tests, function map/test design/mainline docs.
  - `tool.registry`: `crates/freehand-tools/src/lib.rs`, focused schema/export tests, function map/test design/mainline docs.
- required verification:
  - Red/green tests must prove Worker guidance no longer permits external reads, names exact Worker tools, forbids `shell/readlink`, and teaches `ls` before `read_file`.
  - Runtime failed-tool tests must prove Worker unknown-tool and boundary re-entry text gives exact recovery guidance without saying external read/query is allowed.
  - Online S-profile proof must inspect real provider/Worker ledgers, not just static tests.

# 2026-07-24 mobile UI tree static prototype

- Jason requested a static UI tree first, before gap analysis or flow implementation.
- Scope is review-only prototype under `docs/prototypes/mobile-ui-tree/`; no ADP/runtime wiring and no production WebUI changes.
- UI tree roots captured: icon-only top-left Config, icon-only top-right Timer and Built-in Tools, icon-only bottom-left New Conversation, icon-only bottom-right Session Search.
- Home page content is persisted history session list, timer dashboard, and current session dashboard. It must not render Worker session contents as a first-level session list entry; Worker details belong after entering the owning session.
- Config entrance audit now has `docs/prototypes/mobile-ui-tree/config.html`: provider registry/model groups, daemon connection, Worker capability, memory, skills, MCP, env, runtime directories, Android shell permissions/update, logs, and about. It explicitly excludes phone-local rootfs/shared-folder/mount-directory management.
- Visual direction after review: structure is accepted; palette is black/white plus logo green `#75daa7`. Avoid large black blocks and high-saturation category colors. Status is represented by small hollow square markers: green for normal/available, red for needs attention/error.
- Verification: custom Python HTML structure/link check passed; Playwright rendered mobile and desktop screenshots under `artifacts/prototypes/mobile-ui-tree-20260724/`; `git diff --check` passed for touched docs/prototype files.

# 2026-07-24 mobile UI tree Phase 1 production closeout

- Scope stayed Phase 1 UI-only: production WebUI shell, mobile home dashboard, icon-only quick entries, persisted-session-only global session list, and Settings review tree. No timer/search/provider/tool/runtime command semantics were added.
- Online proof passed on S-profile `http://127.0.0.1:4042/`: `node scripts/verify-webui-mobile-ui-tree-online.mjs` -> `mobile_ui_tree_phase1_ok`, artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260724T115954-78332`.
- Browser proof checked asset version `20260724-mobile-ui-tree-phase1`, 390/430/844/1280 viewports, no horizontal overflow, icon-only separated quick entries, mobile dashboard, settings tree provider/model entries, no phone-local storage-management terms, no top-level worker temporary sessions, and hollow green/red status markers.
- S-profile stayed restored: `freehand-cliS adp-config-query` returned `provider=minimax`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`, `web_search=auto`, `web_search_effective=hosted_declared`; fixture env grep returned zero matches.
- Local proof passed: `node --check apps/freehand-server/assets/webui.js`, `node --check scripts/verify-webui-mobile-ui-tree-online.mjs`, `jq empty docs/mainline-calls/app.webui-smoke.json`, `node scripts/verify-webui-layout-shapes.mjs`, `cargo test -p freehand-reason restore_ignores_leftover_atomic_tmp_turn_files -- --nocapture`, `cargo test -p freehand-server --lib -- --nocapture`, `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check`.
- Online S bootstrap blocker was not a new Phase 1 bug: current `reason.persistence` source already ignores non-`.json` atomic temp files and the focused regression passed. After current S binary was in service, 4042 health returned `ok`.
- Android true-device proof remains unclosed for this slice because `adb devices -l` returned an empty device list.

# 2026-07-22 turn timing observability request

- Jason requested that every turn records and displays wait duration and first-token/first-byte time, including historical turns after completion. Route as a separate UI/protocol observability feature: owner likely ui.protocol + reason.persistence projection for durable timing fields, with WebUI rendering after owner map/test design review. Do not mix into the current Worker tool-guidance fix.

# 2026-07-22 turn timing and Master/Worker state unification resume diagnosis

- trigger:
  - Jason requested per-turn wait/first-response timing to remain recorded and visible after completion, and called out that Master and subagent/Worker state are still not unified.
- current source/truth evidence:
  - `webui-path-diagnostic-state-sync-fixed` now has parent turns `runtime-turn-511,runtime-turn-511-r2,runtime-turn-511-r3,runtime-turn-512` and selected status `blocked`.
  - Task `task-webui-path-diagnostic-1784719186046` history is `TaskCreated,TaskWaitingAgent,TaskAssigned,TaskResumed,TaskHeartbeat,TaskBlocked,TaskProgressed`.
  - Parent follow-up turn `runtime-turn-512` closed `Blocked`; reason ledger rows record `time_to_first_response_ms=325900` and `total_elapsed_ms=326176`.
  - Prior active-turn snapshot for `runtime-turn-512` no longer exists; current divergence is not a permanently dangling parent turn.
- remaining risk:
  - The parent blocked follow-up took over 5 minutes before first response, so the online verifier must prove that the UI remains observable during that wait and final DOM state is blocked with timing, not stale lifecycle waiting.
  - Daemon stderr still contains historical `parent session ... has no persisted user objective truth` runner stops and one `persisted cursor is inconsistent` stop; source changes appear to convert missing objective into a skipped evaluation, but focused tests and a fresh S-profile run must prove runner state no longer wedges.
- diagnosis lock:
  - SOP/model flow: `runtime.master-worker-loop` parent workset reconciliation plus `reason.persistence` parent turn truth plus `app.webui-smoke` render projection.
  - First divergence under current evidence: pending until focused tests and fresh verifier; existing current-run truth no longer supports the earlier "parent follow-up never terminalized" hypothesis.
  - Required proof: focused Master blocked-parent tests, UI timing projection tests, and `scripts/verify-webui-path-diagnostic-online.mjs` on fixed S-profile session with DOM timing/blocked checks.

# 2026-07-22 effective session-history rollback diagnosis

- symptom:
  - Fixed-session WebUI path-diagnostic reset removed effective UI turns, but
    the next Master provider request was classified as a stale parent-blocked
    follow-up before the new task was created.
- evidence:
  - `artifacts/webui-online/path-diagnostic-1784728929941/failure.json`
    records the first Master request as
    `parent_blocked_follow_up_before_blocked_decision_append` and zero Worker
    requests.
  - `~/.freehand/state/turns/master/webui-path-diagnostic-state-sync-fixed/session-history.json`
    still contains `session-memory-runtime-turn-515-r3` and
    `session-memory-runtime-turn-516`, including the stale
    `<freehand_parent_blocked_follow_up>` prompt.
- confirmed first divergence:
  - `ReasonPersistence::load_authoritative_state` applies rollback markers to
    closed turns but not `SessionHistory.base_context_segments`.
  - `ReasonPersistence::persist_row_locked` then persists the unfiltered
    history after rollback, so UI transcript and model-visible context disagree.
- owner and scope:
  - unique owner `reason.persistence`.
  - allowed source paths:
    `crates/freehand-reason/src/persistence.rs`,
    `crates/freehand-reason/src/session_history.rs`, and owner-bound
    docs/tests.
  - forbidden workaround paths: WebUI, TaskBoard, provider fixture, or verifier
    classification changes.
- required proof:
  - red/green owner test retaining ordinary/effective memory while rejecting
    rolled-back and orphan `historical_turn:*` segments.
  - focused rollback/restore/runtime regressions.
  - exact fixed-session S-profile WebUI path-diagnostic replay.

# 2026-07-22 Android APK update distribution verification

- trigger:
  - Jason saw the Android Settings update check report no upgrade and asked to verify whether the APK update package actually changed.
- root cause fixed in current worktree:
  - `/android/update.json` had previously been able to fall back to hardcoded `versionCode=1`, `versionName=0.1.0`.
  - Android Gradle defaults also stayed at `versionCode=1`, `versionName=0.1.0` unless properties were set.
  - launchd daemon workdir is `~/.freehand`, so a relative default `dist/android/...` path points at runtime home, not the repo checkout.
- implementation evidence:
  - `apps/freehand-server/src/lib.rs` now serves `/android/update.json` from valid explicit env override or runtime-home sidecar `~/.freehand/dist/android/update.json`; missing sidecar returns 404 and invalid sidecar returns 500 instead of a false current-version manifest, and successful manifest/APK responses carry `Cache-Control: no-store, max-age=0`.
  - `AndroidApkUpdater.httpGetText` disables `HttpURLConnection` caches and sends `Cache-Control: no-cache` / `Pragma: no-cache` for manifest reads.
  - `scripts/release.sh` writes `dist/android/update.json` from `aapt dump badging` on the built release APK.
  - `scripts/install-launchd.sh` and `scripts/install-global.sh` stage `update.json` plus signed `freehand-android-release.apk` into `~/.freehand/dist/android`.
  - `apps/freehand-android/gradle.properties` now sets `freehandVersionCode=3` and `freehandVersionName=0.2.1`, after an intermediate v2 build was insufficient to prove an update if the phone was already on v2.
  - `apps/freehand-android/app/build.gradle.kts` signs release builds with the debug signing config for the current internal update channel; this keeps the signer compatible with the installed debug-channel app and avoids vivo rejecting `*-unsigned.apk` as missing a developer certificate.
- online proof:
  - `curl -i http://127.0.0.1:4042/android/update.json` returned `{"versionCode":3,"versionName":"0.2.1",...}` with `cache-control: no-store, max-age=0`.
  - `curl -i http://100.66.1.82:44042/relay/daemon/studio-host/android/update.json` returned the same `versionCode=3`, `versionName=0.2.1` and `cache-control: no-store, max-age=0`.
  - `aapt dump badging dist/android/freehand-android-release.apk` reported `versionCode='3' versionName='0.2.1'`.
  - `apksigner verify --verbose --print-certs` passed for the built and relay-downloaded APK; signer certificate is Android Debug with SHA-256 `ecd63a2c2070970735cc079b0bb090427ca0b59200da0ebc07c80b50a1dfffda`.
  - repo APK, runtime staged APK, and relay-downloaded APK all had SHA256 `d11fcba7e92e39229779ba6f6efd0e30404617d85c69cdeac0262e6c291a0c41`.
  - direct and relay ADP smoke both passed; final S config stayed `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `default_model=MiniMax-M3`, `auth_source=inline`.
- local proof:
  - `cargo fmt --check`, `cargo check -p freehand-server --lib`, focused `cargo test -p freehand-server --lib android_update -- --nocapture --test-threads=1` 4/4, Android `testDebugUnitTest assembleRelease`, `bash -n` for release/install scripts, `cargo run -p xtask -- mainlines generate/check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- remaining gap:
  - `adb devices -l` returned no connected devices during this verification, so the installed phone package `versionCode` and Settings click/logcat update flow were not true-device closed here. If the phone already has `versionCode>=3`, "no update" is expected; if it has lower version with the same debug signer, the signed relay APK should open Android's installer instead of vivo's missing-certificate rejection.

# 2026-07-23 shared logo replacement

- request:
  - Jason asked to replace the Freehand logo with the uploaded PNG.
- implementation:
  - `assets/logo.png` is the shared logo truth and was regenerated as a 1024x1024 RGBA PNG from the uploaded image.
  - Android launcher square/round mipmaps were regenerated from `assets/logo.png`.
  - WebUI now serves `/assets/logo.png` from the shared asset and renders it in the rail brand instead of text `FH`; the WebUI asset version was bumped to `20260723-logo-refresh`.
  - `app.webui-smoke` function map, test design, mainline call source, and generated wiki were synced for the shared logo asset route.
- evidence:
  - `assets/logo.png` SHA256 `7c89e6ef0cd2054afd593cfc11463bb8f43dde2e962d39b559f066aaf373fa2a`.
  - `apps/freehand-android/scripts/verify-launcher-icons.sh` passed.
  - `cargo test -p freehand-server --lib asset_response_serves_shared_logo_png -- --nocapture --test-threads=1` passed.
  - Android debug APK built at `apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk` with SHA256 `78b6e816b65f75d03d8f2025bcebb2577067cefedee2d1ba317be54a5be2387b`.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed after the logo change.
- gap:
  - `adb devices -l` showed no connected device, so no true-device install/screenshot closure is claimed for this logo replacement slice.

# 2026-07-23 web_fetch online verifier closeout

- trigger:
  - Continue the Master network/search capability slice and prove online that the model can call `web_fetch` and receives the fetched result in the next provider request.
- pre-provider failure root cause:
  - Previous verifier `artifacts/webui-online/web-fetch-tool-20260723T044345-74876/failure/failure.json` had `requestCount=0` and metadata stopped at `instruction-capability:started`; the script treated synchronous ADP command timeout as failure and then restored/restarted S, interrupting the live turn.
  - Fresh run proved `instruction-capability` itself is not hanging: runtime metadata for `runtime-turn-528` recorded `instruction-capability` completed in `16ms`, `task-space-snapshot` in `352ms`, provider request built, and two-round closure.
- verifier fix:
  - `scripts/verify-web-fetch-tool-online.mjs` now allows `SubmitUserInput` receipt timeout as non-terminal harness state, keeps provider/page fixtures alive, polls the fixed session's new turn ids and request evidence, writes `live-observation.json`, and restores only after terminal owner truth or final diagnosis timeout.
- online proof:
  - `node scripts/verify-web-fetch-tool-online.mjs` passed on S-profile `ws://127.0.0.1:4042/adp`.
  - Artifact: `artifacts/webui-online/web-fetch-tool-20260723T061519-41129/summary.json`.
  - Fixed session `web-fetch-tool-online-fixed`; current-run turns `runtime-turn-528,runtime-turn-528-r2`; terminal `Success`.
  - Provider requests: `requestCount=2`; page fixture requests: `pageRequestCount=1`.
  - First request tools included `delete_range,edit_file,glob,grep,ls,multi_edit,read_file,task,timer,web_fetch,write_file`.
  - First request context included Master network guidance and configured Worker capability surface; second request contained `function_call_output` and `WEB_FETCH_TOOL_ONLINE_BODY web-fetch-tool-20260723T061519-41129 fetched-content`.
  - ADP retained `web_fetch` tool activity and fetched body; final marker visible.
  - Restore proof: final script output returned `provider=minimax`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`.
- local proof:
  - `node --check scripts/verify-web-fetch-tool-online.mjs` passed.
  - `cargo test -p freehand-instructions renders_current_repo_instruction_capability_without_scanning_outside_roots -- --nocapture` passed.
  - `cargo test -p freehand-runtime live_bridge_admits_instruction_capability_manifest_as_typed_context -- --nocapture` passed.

# 2026-07-23 web_search provider-native follow-up

- trigger:
  - Jason pointed out that the next direction should be provider-adapted `web_search` for MiniMax and GPT-family providers, avoiding overuse of `web_fetch`.
- current research read:
  - OpenAI Responses exposes hosted web search as a provider-side tool surface.
  - MiniMax public docs expose `web_search` through Token Plan MCP/CLI surfaces; the current Freehand minimax path is Anthropic-compatible Messages, so native search support must be verified against the exact MiniMax wire before implementation.
- architecture constraint:
  - Do not fake broad search through `web_fetch`; `web_fetch` remains one concrete URL fetch.
  - Next implementation should start from resource map: add a `web_search_resource` / provider-hosted-search operation, then route through provider-specific wire drivers.
  - OpenAI Responses should use its hosted search tool declaration/result semantics; MiniMax needs a separate wire adapter if the selected provider/protocol supports native search, otherwise the model-visible tool must be absent or explicitly blocked.
- required future proof:
  - Provider-request black-box fixtures for OpenAI Responses and MiniMax-selected protocol.
  - Online S-profile proof that the model can request broad search, provider receives native search declaration, results return through the provider response chain, and no `web_fetch` concrete-URL substitute is used.

# 2026-07-23 provider-hosted web_search OpenAI Responses closeout

- trigger:
  - Jason pointed out OpenAI behavior can be verified directly from local Codex source, and Freehand should use provider-native web_search where supported instead of faking broad search with web_fetch.
- Codex source evidence:
  - `/Users/fanzhang/code/codex/codex-rs/tools/src/tool_spec.rs` serializes `ToolSpec::WebSearch` as Responses hosted tool type `web_search`.
  - `/Users/fanzhang/code/codex/codex-rs/core/src/tools/hosted_spec.rs` maps live web search to `external_web_access=true`.
  - `/Users/fanzhang/code/codex/codex-rs/core/tests/common/responses.rs`, `core/src/event_mapping_tests.rs`, and `protocol/src/models.rs` treat `web_search_call` as hosted search observation/item, not a local function call.
- implementation truth:
  - `provider.semantic` now carries provider-neutral `ProviderHostedToolDefinition::WebSearch` on `ProviderSemanticRequest.hosted_tools`.
  - `provider.openai-adapter` renders OpenAI Responses hosted `{"type":"web_search","external_web_access":true}` and maps `web_search_call` into provider-hosted reasoning observations, never `ProviderSemanticOutput::ToolCall`.
  - `provider.reason-live-bridge` derives hosted search only from config/provider descriptor/execution profile. OpenAI Responses GPT-family models with `web_search=auto` can mix hosted search with Master function tools; `clean_search` Worker uses hosted search only with zero local function tools.
  - `web_fetch` remains a concrete-URL tool. It may be declared on the Master mixed-tool surface, but online proof must show broad-search evidence came from provider `web_search_call`, not `web_fetch` tool execution.
  - MiniMax remains unsupported for hosted search because the current Freehand MiniMax baseline is Anthropic-compatible Messages at `api.minimaxi.com/anthropic` with model `MiniMax-M3`; RCC/CC config hints are not enough without exact native wire proof.
- resource/gate fix:
  - The first `xtask gates check` failed because the new `tool_call -> provider_hosted_search` shortcut gate forbade `freehand-provider-core`, while `freehand-tools` already legitimately depends on provider-core types.
  - The gate was corrected to `precise_checked` on `crates/freehand-tools/src/lib.rs::reasonix_aligned_builtin_specs`, forbidding a local `"web_search"` spec or provider hosted-search symbols while requiring concrete-URL `web_fetch`, `task`, and `timer` guidance.
- online proof:
  - `node scripts/verify-provider-hosted-web-search-online.mjs` passed on S-profile `ws://127.0.0.1:4042/adp`.
  - Artifact: `artifacts/webui-online/provider-hosted-web-search-20260723T111359-38940/summary.json`.
  - Fixed session `provider-hosted-web-search-online-fixed`; current-run turn `runtime-turn-532`; terminal `Success`.
  - Fixture saw `requestCount=1` to `/openai/v1/responses` with hosted tool `web_search`, `external_web_access=true`, no function tool `web_search`, Master function tools including `task`, `timer`, and concrete-url `web_fetch`.
  - ADP observed `provider-hosted web_search` plus the exact search query, final marker was visible, and checks proved `adpDidNotUseWebFetchAsSearch=true`.
  - Restore proof returned S config to `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `auth_source=inline`, with fixture env grep 0.
- local proof:
  - `node --check scripts/verify-provider-hosted-web-search-online.mjs`, `jq empty` for touched manifests, `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
  - `cargo test -p freehand-provider-core hosted_tool_metadata -- --nocapture` passed.
  - `cargo test -p freehand-provider-openai web_search -- --nocapture` passed 4/4.
  - Runtime hosted-search focused tests passed: `live_bridge_derives_hosted_web_search_only_for_supported_openai_responses`, `clean_search_worker_request_uses_hosted_search_without_local_instruction_scan`, `live_bridge_does_not_mix_search_only_hosted_tool_with_master_functions`, and `clean_search` 5/5.
  - `cargo check -p freehand-cli`, `cargo test -p freehand-config provider`, `cargo test -p freehand-ui-protocol config`, and `cargo test -p freehand-server --lib` passed; server panic output is the intentional dispatch-worker join-failure negative test.

# 2026-07-23 web_search auto effective capability diagnosis

- symptom:
  - WebUI Settings shows selected provider `web_search=auto`, but live Master session `webui-session-20260723001509-bd98e156` turn `runtime-turn-534` answered that the environment has no web_search capability/tool.
- evidence:
  - `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` shows selected master provider `minimax:anthropic:messages:MiniMax-M3:web_search=auto` and registry also has `cc:openai:responses:gpt-5.5:web_search=auto`.
  - `~/.freehand/state/turns/master/webui-session-20260723001509-bd98e156/turns/runtime-turn-534.json` runtime-tool-guidance says provider-hosted search may be used only if declared in the current provider request and that broad/current search can use `execution_profile="clean_search"`, but it does not list effective configured-vs-current-vs-worker route status.
  - Provider response ledger for `runtime-turn-534` shows MiniMax-M3 answered from that ambiguous guidance: no local function `web_search`, selected provider did not declare hosted search, therefore no capability.
- first divergence:
  - Runtime config projection and model-visible guidance collapse configured mode (`auto`) with effective capability. `project_config_status_for_ui` projects only `provider_web_search`, not current effective support/reason or configured worker clean_search routes.
- owner/scope:
  - unique owner: `provider.reason-live-bridge` for provider descriptor/effective capability and Master model-visible route guidance.
  - UI projection owner touched through protocol projection: `runtime.ui-command-dispatch` / `freehand-ui-protocol` config status shape.
  - allowed paths: `crates/freehand-runtime/src/lib.rs`, `crates/freehand-runtime/src/live_context.rs`, `crates/freehand-ui-protocol/src/lib.rs`, `apps/freehand-server/assets/webui.js`, owner docs/tests.
- fix direction:
  - Add configured-vs-effective hosted search projection for selected and registry providers.
  - Add model-visible active Web Search Route Status listing selected provider effective state and configured Worker clean_search-capable providers.
  - Keep `web_search` out of local tool registry; do not fake a function tool.
# 2026-07-23 provider-hosted web_search MiniMax/OpenAI closeout refresh

- trigger:
  - Jason pointed out Settings had `web_search=auto` but model-visible tools did not expose usable search, and asked why the provider capability was effectively blocked instead of testable/visible.
- root cause verified:
  - The provider capability projection/guidance initially collapsed configured `auto` with effective route state, so MiniMax could answer "no web_search capability" from ambiguous guidance.
  - Direct MiniMax ADP test initially failed after declaring the hosted server tool because `execute_provider_web_search_test` used `tool_choice=auto`; MiniMax-M3 chose not to call the server tool and returned text claiming no browsing/search capability. This proved prompt induction is not a capability test.
- implementation truth:
  - `provider.reason-live-bridge` now projects configured-vs-effective web search state for the selected provider and registry: `web_search_effective=hosted_declared` for OpenAI Responses and Anthropic Messages when `web_search=auto`, including MiniMax's current `api.minimaxi.com/anthropic` Messages baseline.
  - Model-visible route guidance now states the selected provider, protocol, effective hosted-search state, configured Worker clean_search routes, and explicitly says Freehand never exposes a local function tool named `web_search`.
  - `TestProviderWebSearch` is an ADP/runtime-owned command available from CLI and WebUI Settings. It sends a direct provider request with hosted search only and requires a provider-hosted observation.
  - For Anthropic/MiniMax Messages, the provider web_search test uses required `tool_choice={"type":"tool","name":"web_search"}`; for OpenAI Responses it keeps hosted `web_search` as a provider tool, not a fake function choice.
  - `web_fetch` remains a concrete-URL fetch function and is not a broad-search substitute.
- online proof:
  - Direct MiniMax S-profile proof passed after forcing the hosted server tool: `freehand-cliS adp-provider-web-search-test --url ws://127.0.0.1:4042/adp --provider minimax` -> `adp_provider_web_search_test_ok ... provider=minimax:protocol=messages:model=MiniMax-M3:hosted_tool=web_search:hosted_observed=true:semantic_outputs=6`.
  - Ordinary Master verifier passed: `node scripts/verify-provider-hosted-web-search-online.mjs`, artifact `artifacts/webui-online/provider-hosted-web-search-20260723T182747-13017/summary.json`, fixed session `provider-hosted-web-search-online-fixed`, turn `runtime-turn-540`, one `/openai/v1/responses` provider request with hosted tool `web_search`, `external_web_access=true`, no function `web_search`, Master functions still included `task`, `timer`, and concrete-url `web_fetch`, ADP observed hosted search/query, `adpDidNotUseWebFetchAsSearch=true`, terminal Success.
  - Restore proof after verifier: S config returned to `provider=minimax`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search=auto`, `web_search_effective=hosted_declared`; fixture env grep and active-work find returned no matches.
- local proof:
  - `cargo fmt --check`; `node --check scripts/verify-provider-hosted-web-search-online.mjs`; `jq empty` for touched mainline manifests; `git diff --check`.
  - `cargo test -p freehand-runtime provider_web_search_test -- --nocapture` passed 3/3, including Anthropic/MiniMax required server-tool choice.
  - `cargo test -p freehand-provider-anthropic web_search -- --nocapture` passed 2/2.
  - `cargo test -p freehand-instructions --lib -- --nocapture` passed 9/9; `cargo test -p freehand-runtime live_bridge_admits_instruction_capability_manifest_as_typed_context -- --nocapture` passed.
  - `cargo check -p freehand-cli`; `cargo test -p freehand-ui-protocol provider_web_search -- --nocapture`; `cargo test -p freehand-config provider -- --nocapture`; `cargo test -p freehand-server --lib -- --nocapture`; `cargo run -p xtask -- mainlines check`; `cargo run -p xtask -- gates check` passed.
- supersedes:
  - Earlier 2026-07-23 note/MEMORY statements that MiniMax hosted search remained unsupported are now superseded for the current Freehand MiniMax Anthropic-compatible Messages baseline. MiniMax native non-Anthropic hosted-search wire is still unverified and must not be hardcoded.
# 2026-07-24 mobile UI tree Phase 2 timer dashboard slice

- scope:
  - Phase 1 production WebUI shell was already committed as `be03a49`.
  - Current work closes only the Phase 2 Timer dashboard function slice: WebUI list/schedule/cancel/fired history projection through ADP/runtime TimerStore owner truth.
  - Full Phase 2 remains open for Provider management, model groups, Tools registry page, New/session/search, Android update/permission/notification settings, and persisted-session search.
- implementation:
  - `ui.protocol` now has `QueryTimerList`, `ScheduleTimer`, `CancelTimer`, timer schedule/repeat/list/event DTOs, route validation, and `TimerList` query result.
  - `runtime.master-worker-loop` now schedules/cancels/lists timers through `TimerStore` and projects UI-safe timer rows/events without Task Center truth.
  - WebUI Timer dashboard opens from the mobile quick entry, queries `QueryTimerList`, schedules relative/absolute/recurring/cron forms through `ScheduleTimer`, cancels active timers through `CancelTimer`, refreshes owner truth after receipts, and updates the mobile home timer summary from owner projection.
  - `scripts/verify-timer-tool-online.sh` was repaired after root cause: it wrongly reused generic `adp-turn-sample --sample success`, whose fixed transcript evidence fails when the timer fixture intentionally calls the timer. The verifier now submits a timer-specific ADP prompt, requires the post-schedule turn to terminalize as `ToolPending`/`claim="waiting"`, keeps the fixture provider alive, and only restores config after mock request 3 observes the due wakeup.
- online proof:
  - S-profile stayed on `ws://127.0.0.1:4042/adp` / `http://127.0.0.1:4042`.
  - `node scripts/verify-webui-mobile-ui-tree-online.mjs` passed: `mobile_ui_tree_phase1_ok`, artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260724T140115-46451`.
  - `node scripts/verify-webui-timer-dashboard-online.mjs` passed: `webui_timer_dashboard_ok`, timer `timer-master-source-less-ui-1784901696810754000-1`, artifact `artifacts/webui-online/webui-timer-dashboard-20260724T140133-47790`; proof includes DOM schedule row, ADP TimerList row, DOM cancel action, TimerCancelled ledger, and unchanged top-level SessionList ids.
  - `bash scripts/verify-timer-tool-online.sh` passed: timer `timer-online-proof-1784901474-25671`, session `timer-online-proof-session-1784901474-25671`, `session_turns=3`, `mock_attempts=3`, `timer_submit_verified ... waiting_turn=runtime-turn-544-r2 timer_tool_turn=runtime-turn-544 tool_executions=1`, `timer_due_verified ... status=completed fired_count=1`, and mock request 3 had `sawToolResult=true` plus `sawTimerWakeup=true`.
  - `FREEHAND_TIMER_VERIFY_MODE=restart-due bash scripts/verify-timer-tool-online.sh` passed: timer `timer-online-proof-1784901568-34363`, session `timer-online-proof-session-1784901568-34363`, `session_turns=3`, `mock_attempts=3`, `timer_submit_verified ... waiting_turn=runtime-turn-546-r2 timer_tool_turn=runtime-turn-546 tool_executions=1`, `timer_due_verified ... status=completed fired_count=1`, and mock request 3 had `sawToolResult=true` plus `sawTimerWakeup=true` after service-scoped `restartS`.
  - Final config restore check returned `provider=minimax`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search=auto`, `web_search_effective=hosted_declared`, `auth_source=inline`; fixture env grep returned 0 matches.
- local proof:
  - `bash -n scripts/verify-timer-tool-online.sh` passed.
  - `node --check scripts/verify-webui-timer-dashboard-online.mjs` and `node --check scripts/verify-webui-mobile-ui-tree-online.mjs` passed.
  - `jq empty docs/resource-maps/core.json docs/mainline-calls/runtime.master-worker-loop.json docs/mainline-calls/runtime.ui-command-dispatch.json docs/mainline-calls/ui.protocol.json docs/mainline-calls/app.webui-smoke.json` passed.
  - `cargo test -p freehand-ui-protocol timer_ -- --nocapture` passed 4/4.
  - `cargo test -p freehand-runtime runtime_timer_ui_commands -- --nocapture` passed 2/2.
  - `cargo test -p freehand-runtime timer -- --nocapture` passed 13/13.
  - `cargo test -p freehand-server --lib -- --nocapture` passed 19/19; the printed dispatch-worker panic is the intentional join-failure negative test.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines generate`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- remaining gaps:
  - Phase 2 as a whole is not complete; only Timer dashboard owner wiring is closed.
  - Android true-device proof was not run in this slice.
  - In the Codex shell, direct shebang execution of `scripts/verify-timer-tool-online.sh` returned `137` before creating a new timer temp dir; explicit `bash scripts/verify-timer-tool-online.sh` is the verified entrance in this environment.

# 2026-07-24 mobile UI tree Phase 2 model group Settings slice

- scope:
  - Current work closes only the Phase 2 model group Settings function slice.
  - Full Phase 2 remains open for Provider registry editing/testing, Tools registry/instruction capability page, Search, New/session/task/attachment wiring, Android update/permission/notification true-device closure, and lifecycle dashboard proof.
- implementation:
  - `config.core` owns persisted model group registry, safe projections, upsert, and active agent model-group selection in `~/.freehand/config.toml`.
  - `ui.protocol` owns `UpsertModelGroupConfig`, `UpdateAgentModelGroupSelection`, route/group update DTO validation, and `UiConfigStatusProjection.model_group_registry`.
  - `runtime.ui-command-dispatch` only bridges config commands to `config.core` and projects config status; WebUI does not read or write config truth directly.
  - WebUI Settings renders provider-backed model group registry and active group selector with primary/sub/search/title/fallback/load-balance route fields. Saved changes require restart and refresh through `QueryConfigStatus`.
  - Asset version is `20260724-model-groups-ui`.
- online proof:
  - `node scripts/verify-model-group-ui-online.mjs` passed on S-profile `ws://127.0.0.1:4042/adp`.
  - Artifact: `artifacts/webui-online/model-group-ui-1784907693362`.
  - Verifier created group `ui.verify.1784907693362` through WebUI DOM, observed ADP `model_group_registry`, switched active group through WebUI, and proved projected active route `provider_id=cc`, `default_model=gpt-5.5-model-group-ui`, `fallback_provider_id=minimax`.
  - Restore proof: final summary has `restored=true`, `restoreErrors=[]`, final provider `minimax`, protocol `messages`, base host `api.minimaxi.com`, model `MiniMax-M3`, `model_group_id=null`, groups `[]`, and fixture env match count `0`.
- local proof:
  - `node --check scripts/verify-model-group-ui-online.mjs` passed.
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `jq empty docs/resource-maps/core.json docs/mainline-calls/config.core.json docs/mainline-calls/ui.protocol.json docs/mainline-calls/runtime.ui-command-dispatch.json docs/mainline-calls/app.webui-smoke.json` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-model-group cargo test -p freehand-config model_group -- --nocapture --test-threads=1` passed 2/2.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-model-group cargo test -p freehand-ui-protocol model_group -- --nocapture --test-threads=1` passed 1/1.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-model-group cargo test -p freehand-runtime model_group -- --nocapture --test-threads=1` passed 1/1.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-model-group cargo test -p freehand-server --lib -- --nocapture --test-threads=1` passed 19/19; the printed dispatch-worker panic is the intentional join-failure negative test.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-model-group cargo check -p freehand-config -p freehand-ui-protocol -p freehand-runtime -p freehand-server` passed.
  - `cargo fmt --check`, `CARGO_TARGET_DIR=/tmp/freehand-target-model-group cargo run -p xtask -- mainlines check`, `CARGO_TARGET_DIR=/tmp/freehand-target-model-group cargo run -p xtask -- gates check`, and `git diff --check` passed.
  - Final config query returned `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search=auto`, `web_search_effective=hosted_declared`, `auth_source=inline`; fixture env grep returned 0 matches.

# 2026-07-24 mobile UI tree Phase 2 Tools registry UI slice

- scope:
  - Current work closes only the Phase 2 owner-backed Tools dashboard slice.
  - Tools dashboard is read-only registry/settings/instruction-capability projection; it does not execute tools and does not render conversation tool turns.
  - Full Phase 2 remains open for Provider registry editing/testing, Search, New/session/task/attachment wiring, Android update/permission/notification true-device closure, and lifecycle dashboard proof.
- implementation:
  - `tool.registry` now projects `BuiltinToolRegistry::registry_projection()` with version, global guidance, per-tool schema, examples, guidance, read-only/implemented flags, execution scope, and Master/Worker exposure.
  - `ui.protocol` now defines `QueryToolRegistry`, `UiToolRegistryProjection`, `UiToolRegistryToolProjection`, and `UiQueryResult::ToolRegistry`; local protocol state rejects the query so runtime/tool owner must answer.
  - `runtime.ui-command-dispatch` bridges `QueryToolRegistry` to the tool owner projection and maps it into UI DTOs.
  - WebUI Tools dashboard opens from the mobile top-right tools entry, queries `QueryToolRegistry`, renders owner-projected tool cards/schema/examples/guidance/exposure, and does not hardcode a browser-local tool list.
  - `scripts/verify-webui-tools-registry-online.mjs` uses ADP `QueryToolRegistry` plus browser DOM proof; it now prefers Playwright `chromium_headless_shell` because macOS can reuse an already-running normal Chrome and ignore new CDP port flags.
- online proof:
  - `node scripts/verify-webui-tools-registry-online.mjs` passed on S-profile `ws://127.0.0.1:4042/adp`.
  - Artifact: `artifacts/webui-online/webui-tools-registry-20260724T165955-27717`.
  - Summary: `webui_tools_registry_ok url=http://127.0.0.1:4042/ adp=ws://127.0.0.1:4042/adp tools=19`.
  - Checks true: production asset version, dialog opened, DOM tool names match ADP, core tools visible, no local `web_search` tool, `task`/`timer` Master-only, `web_fetch` Master+Worker, `bash` implemented but hidden from Master/Worker, Worker-only tools hidden from Master, path guidance visible, no top-level session created, no horizontal overflow.
  - Related online regressions passed with headless shell:
    - `mobile_ui_tree_phase1_ok`, artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260724T165539-5547`
    - `webui_timer_dashboard_ok`, timer `timer-master-source-less-ui-1784912153675880000-1`, artifact `artifacts/webui-online/webui-timer-dashboard-20260724T165550-6177`
    - `model_group_ui_online_ok`, group `ui.verify.1784912168782`, final provider `minimax`, final model `MiniMax-M3`, final group `none`, artifact `artifacts/webui-online/model-group-ui-1784912168782`
- local proof:
  - `jq empty docs/resource-maps/core.json docs/mainline-calls/tool.registry.json docs/mainline-calls/ui.protocol.json docs/mainline-calls/runtime.ui-command-dispatch.json docs/mainline-calls/app.webui-smoke.json` passed.
  - `node --check apps/freehand-server/assets/webui.js` and `node --check` for the four related online verifiers passed.
  - `cargo fmt --check` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-tools-registry cargo test -p freehand-tools registry_projection -- --nocapture --test-threads=1` passed 1/1.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-tools-registry cargo test -p freehand-ui-protocol tool_registry -- --nocapture --test-threads=1` passed 1/1.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-tools-registry cargo test -p freehand-runtime tool_registry -- --nocapture --test-threads=1` passed 1/1.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-tools-registry cargo test -p freehand-server --lib -- --nocapture --test-threads=1` passed 19/19; the printed dispatch-worker panic is the intentional negative test.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-tools-registry cargo check -p freehand-tools -p freehand-ui-protocol -p freehand-runtime -p freehand-server` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-tools-registry cargo run -p xtask -- mainlines generate/check` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-tools-registry cargo run -p xtask -- gates check` passed after fixing `docs/function-maps/tool.registry.md` to list touched `ui_projection`.
  - Final `git diff --check` passed after memory/skill append.
- restore proof:
  - Final S config query returned `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search=auto`, `web_search_effective=hosted_declared`, `auth_source=inline`.
  - Fixture env grep for model group/provider retry/master autonomy keys returned 0 matches.

# 2026-07-25 mobile UI tree Phase 2 Provider registry UI proof

- scope:
  - Current work closes only the Phase 2 Provider registry UI evidence slice.
  - No runtime code change was required in this slice; existing owner-backed code already exposes provider registry add/update, active provider switch, and provider-hosted web_search test controls through ADP/protocol.
  - Full Phase 2 remains open for Search, New/session/task/attachments, current-session lifecycle dashboard, Android update/permissions/notifications true-device closure, and diagnostics.
- evidence:
  - `FREEHAND_PROVIDER_REGISTRY_UI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell FREEHAND_PROVIDER_REGISTRY_UI_DEBUG_PORT=9273 node scripts/verify-provider-registry-ui-online.mjs` passed on S-profile.
  - Output: `provider_registry_ui_online_ok url=ws://127.0.0.1:4042/adp run_id=provider-registry-ui-1784913165666 added_provider=ui-verify-provider-registry switched_provider=cc final_provider=minimax final_fallback=cc final_registry=cc,minimax`.
  - Artifact: `artifacts/webui-online/provider-registry-ui-1784913165666`.
  - Proof contents include initial DOM config owner projection, WebUI `UpsertProviderConfig` add, proof that upsert did not change primary/fallback, WebUI `UpdateAgentProviderSelection` switch to `cc`, and post-finally restore.
- restore proof:
  - Final config query returned `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search=auto`, `web_search_effective=hosted_declared`, `auth_source=inline`.
  - Fixture env grep for provider registry/provider retry/master autonomy keys returned 0 matches.
# 2026-07-25 mobile UI tree Phase 2 persisted-session Search slice

- scope:
  - Current work closes only the Phase 2 persisted-session Search dashboard slice.
  - Full Phase 2 remains open for New/session/task/attachments, current-session lifecycle dashboard, Android update/permissions/notifications true-device closure, and diagnostics.
- implementation:
  - `ui.protocol` owns `QuerySessionSearch`, `UiSessionSearchProjection`, parent result DTOs, child result DTOs, and empty-query validation; local protocol state rejects the query so runtime owner must answer.
  - `runtime.ui-command-dispatch` routes Search to `reason.persistence` persisted session index plus session metadata truth and `task.orchestration` TaskBoard parent truth; metadata-only sessions created by WebUI are valid persisted parent sessions and are included as top-level search candidates.
  - Worker `worker-task-*` transcript matches are nested under the owning persisted parent session through TaskBoard `parent_session_id` / canonical worker session id; worker/debug/internal sessions are never top-level Search results.
  - WebUI Search opens from the session/history entry, queries ADP `QuerySessionSearch`, renders parent cards plus indented child matches, and clicking a result opens the parent session.
- online proof:
  - `FREEHAND_SESSION_SEARCH_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-session-search-online.mjs` passed on S-profile.
  - Artifact: `artifacts/webui-online/webui-session-search-1784932659523`.
  - Output: `webui_session_search_ok url=http://127.0.0.1:4042/ adp=ws://127.0.0.1:4042/adp session=webui-session-search-fixed`.
  - Summary checks true: owner projection contains fixed session, browser dialog opened, DOM rows match owner projection, no top-level worker result cards/sessions, fixed session opens, dialog closes after open, no unexpected top-level sessions, no horizontal overflow, asset version served.
- local proof:
  - `node --check scripts/verify-webui-session-search-online.mjs` and `node --check apps/freehand-server/assets/webui.js` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-session-search cargo test -p freehand-ui-protocol session_search -- --nocapture --test-threads=1` passed 1/1.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-session-search cargo test -p freehand-runtime runtime_query_session_search_returns_worker_hits_under_parent_session -- --nocapture --test-threads=1` passed 1/1 and now covers metadata-only parent search plus nested Worker hit.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-session-search cargo test -p freehand-server --lib -- --nocapture --test-threads=1` passed 19/19; dispatch-worker panic row is the intentional negative test.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
- restore proof:
  - Final S config query returned `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search=auto`, `web_search_effective=hosted_declared`, `auth_source=inline`.
  - Fixture env grep for provider retry/master autonomy keys returned 0 matches.
- lesson:
  - Search/list verifiers must include metadata-only sessions created by WebUI; treating persisted index rows as the only persisted-session truth misses valid newly-created empty sessions.

# 2026-07-25 mobile UI tree Phase 2 New conversation/task slice

- scope:
  - Current work closes only the Phase 2 New conversation / New task online proof slice.
  - Full Phase 2 remains open for attachment failure-retention proof, current-session lifecycle dashboard closure, Android true-device update/permission/notification closure, and diagnostics.
- implementation:
  - Production WebUI asset version is bumped to `20260725-new-session-ui`.
  - Browser-fixed draft session ids are available only behind `globalThis.__freehandEnableTestHooks` plus `__freehandDraftSessionIdsForTest`; normal production browsing still uses generated `webui-session-*` ids.
  - `scripts/verify-webui-new-session-online.mjs` clicks the mobile New entry, creates a no-cwd conversation and cwd-bound task through the actual dialog, waits for `QuerySessionList` owner truth after each `CreateSession`, and verifies no `worker-task-*` rows appear as top-level sessions.
- online proof:
  - `FREEHAND_NEW_SESSION_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-new-session-online.mjs` passed.
  - Output: `webui_new_session_ok url=http://127.0.0.1:4042/ adp=ws://127.0.0.1:4042/adp conversation=webui-new-conversation-fixed task=webui-new-task-fixed artifactDir=/Volumes/extension/code/freehand/artifacts/webui-online/webui-new-session-1784934872925`.
  - Summary checks true: asset version served, mobile New opens conversation dialog, conversation persisted through owner truth without cwd, conversation selected with clean empty state, task dialog accepts cwd, task persisted through owner truth with cwd, task selected/projected, no top-level Worker sessions, and no horizontal overflow.
- local proof:
  - `node --check scripts/verify-webui-new-session-online.mjs` and related WebUI verifier syntax checks passed.
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-new-session cargo test -p freehand-server --lib -- --nocapture --test-threads=1` passed 19/19; the printed dispatch-worker panic is the intentional negative test.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed in the first verification pass.
  - Final restore proof from that pass: S config was `minimax/MiniMax-M3`, `web_search_effective=hosted_declared`, inline auth, and fixture env grep was 0.
- lesson:
  - New/session online verifiers must not create random session spam. Use explicit test-gated fixed ids, then wait for owner `QuerySessionList` truth before asserting UI selection or persisted-session state.

# 2026-07-25 mobile UI tree Phase 2 attachment failure-retention slice

- scope:
  - Current work closes only the New/task attachment failure-retention proof slice.
  - Full Phase 2 remains open for current-session lifecycle dashboard closure, Android true-device update/permission/notification closure, diagnostics/logs owner-safe projection, and final requirement audit.
- implementation:
  - Production WebUI asset version is bumped to `20260725-attachment-failure-ui`.
  - Pending submit failure cards now show whether draft attachments were retained for retry.
  - WebUI test hooks expose selected session/cwd, pending submit state, attachment tray counts, and a test-only ADP socket close helper.
  - `scripts/verify-webui-ambiguous-submit-recovery.mjs` now also creates a fixed cwd-bound task session through the real mobile New dialog, selects an actual image via browser file input, forces deterministic offline/closed-socket submit failure, and proves owner/session/cwd/pending-card/attachment retention before running the existing ambiguous-submit branches.
- online proof:
  - `FREEHAND_WEBUI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-ambiguous-submit-recovery.mjs` passed on S-profile.
  - Output: `webui_ambiguous_submit_recovery_ok session=webui-ambiguous-submit-recovery-fixed attachment_session=webui-attachment-failure-retain-fixed artifact=/Volumes/extension/code/freehand/artifacts/webui-online/ambiguous-submit-recovery-fixed/summary.json`.
  - Summary checks true: `attachmentSessionCreatedThroughOwnerTruth`, `attachmentTaskSelectedWithCwd`, `imageSelectedThroughInput`, `failureKeepsSessionCwdAndPendingCard`, `failureKeepsAttachmentDraft`, `ownerSessionStillCwdBoundAfterFailure`, `materializedClearsPending`, `taskTruthClearsPending`, `unverifiedKeepsPendingSession`.
  - Evidence includes selected image `attachment-failure-proof.png`, `attachmentCount=1`, `thumbCount=1`, `removeCount=1`, command status `1 attachment draft(s) in selected session`, and failure text `Draft attachments retained for retry: 1.`.
- local proof:
  - `node --check apps/freehand-server/assets/webui.js` passed.
  - `node --check scripts/verify-webui-ambiguous-submit-recovery.mjs` passed.
  - Related WebUI verifier syntax checks passed: model group, mobile UI tree, New session, session search, Timer dashboard, and Tools registry.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-attachment-failure cargo test -p freehand-server --lib -- --nocapture --test-threads=1` passed 19/19; the printed dispatch-worker panic is the intentional negative test.
  - `cargo fmt --check` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-attachment-failure cargo run -p xtask -- mainlines check` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-attachment-failure cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- restore proof:
  - Final config query returned `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search=auto`, `web_search_effective=hosted_declared`, `auth_source=inline`.
  - Fixture env grep for provider retry/master autonomy/WebUI verifier/provider registry keys returned 0 matches.

# 2026-07-25 mobile UI tree Phase 2 current-session dashboard proof

- scope:
  - Current work closes only the current-session dashboard / Worker child navigation online proof slice.
  - Full Phase 2 remains open for Android true-device update/permission/notification closure, diagnostics/logs owner-safe projection, and final requirement audit.
- setup:
  - Created fixed persisted parent session `webui-current-dashboard-fixed` through ADP `CreateSession`.
  - Created fixed owner TaskBoard child `task-webui-current-dashboard-fixed` through ADP `CreateTask`, assigned to `worker`, claimed execution `exec-webui-current-dashboard-fixed`, and applied a `running` execution fact.
  - Owner truth row: status `running`, parent `webui-current-dashboard-fixed`, worker session `worker-task-task-webui-current-dashboard-fixed`, active execution `exec-webui-current-dashboard-fixed`, target cwd `/Volumes/extension/code/freehand`.
- online proof:
  - `FREEHAND_WEBUI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell FREEHAND_WORKER_SUBTASKS_PARENT_SESSION=webui-current-dashboard-fixed FREEHAND_WORKER_SUBTASKS_EXPECTED_COUNT=1 FREEHAND_WORKER_SUBTASKS_MAX_WORKER_LABEL=3 FREEHAND_WORKER_SUBTASKS_WEBUI_ARTIFACT_DIR=artifacts/webui-online/worker-subtasks-current-dashboard-fixed node scripts/verify-worker-subtasks-webui-online.mjs` passed.
  - Artifact: `artifacts/webui-online/worker-subtasks-current-dashboard-fixed/summary.json`; screenshots: `01-mobile-agent-sheet.png`, `02-worker-01-task-webui-current-dashboard-fixed.png`, `03-returned-parent.png`.
  - Header proof: selected parent `webui-current-dashboard-fixed`, summary `0 running · 1 running task · limit 3`, copy `running: Current dashboard fixed Worker proof`.
  - Sheet proof: `1 active · 0 review · 0 blocked · 0 closed · 0 stale`, one card with `data-task-id=task-webui-current-dashboard-fixed`, `data-worker-session-id=worker-task-task-webui-current-dashboard-fixed`, label `Worker 1` within configured limit 3.
  - Worker navigation proof: clicking the task selected `worker-task-task-webui-current-dashboard-fixed`, showed Worker nav, had zero user-message/internal-prompt leaks, and returned to exact parent session.
- restore proof:
  - Final config query returned `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search=auto`, `web_search_effective=hosted_declared`, `auth_source=inline`.
  - Fixture env grep for path diagnostic/provider retry/master autonomy keys returned 0 matches.
- known note:
  - `scripts/verify-webui-path-diagnostic-online.mjs` still starts with `scripts/install-launchd.sh restartS`, which calls default-target `scripts/install-symlink.sh`; in this environment that build can stall before proof. The current dashboard proof avoided that path by using protocol owner commands plus the dedicated WebUI worker-subtasks verifier.

# 2026-07-25 mobile UI tree Phase 2 diagnostics/logs slice

- scope:
  - Current work closes only the Diagnostics logs Settings slice.
  - Full Phase 2 remains open for Android true-device update/permission/notification closure and final requirement audit.
- implementation:
  - `ui.protocol` now owns `QueryDiagnostics`, `UiDiagnosticsProjection`, and `UiDiagnosticLogFileProjection`; local protocol state rejects the query so runtime owner must answer.
  - `runtime.ui-command-dispatch` projects diagnostics from the configured live runtime home only, reads `~/.freehand/logs/*.log`, returns relative `logs/<name>` metadata, and caps log tail reads to the last 64 KiB / 5 non-empty lines.
  - Diagnostic tail lines redact provider payload/request markers, auth/API-key/token/secret markers, pair tokens, and absolute `/Users` or `/Volumes` path lines before UI projection.
  - WebUI Settings renders a Diagnostics logs card from owner projection and refreshes through ADP `QueryDiagnostics`; it does not inspect browser-local files or raw runtime paths.
  - `scripts/verify-webui-diagnostics-online.mjs` verifies ADP projection safety, DOM row matching, no top-level session creation, asset version, no horizontal overflow, and redacted diagnostics DOM content.
- online proof:
  - S-profile `scripts/install-launchd.sh restartS` completed; health was `ok`.
  - Final config query stayed `provider=minimax`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search=auto`, `web_search_effective=hosted_declared`, and `auth_source=inline`.
  - `FREEHAND_WEBUI_DIAGNOSTICS_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-diagnostics-online.mjs` passed.
  - Output: `webui_diagnostics_ok url=http://127.0.0.1:4042/ adp=ws://127.0.0.1:4042/adp files=19 artifactDir=/Volumes/extension/code/freehand/artifacts/webui-online/webui-diagnostics-1784942454320`.
  - Fixture/env grep returned 0 matches for provider retry, master autonomy, path diagnostic, and diagnostics fixture markers.
- local proof:
  - `node --check apps/freehand-server/assets/webui.js` and `node --check scripts/verify-webui-diagnostics-online.mjs` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-diagnostics cargo test -p freehand-ui-protocol diagnostics_query -- --nocapture` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-diagnostics cargo test -p freehand-runtime runtime_query_projects_diagnostics_without_raw_secrets_or_absolute_home -- --nocapture --test-threads=1` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-diagnostics cargo test -p freehand-server webui_smoke_renders_shell_and_asset_routes -- --nocapture --test-threads=1` passed.
  - `cargo fmt --check`, `CARGO_TARGET_DIR=/tmp/freehand-target-diagnostics cargo run -p xtask -- mainlines check`, `CARGO_TARGET_DIR=/tmp/freehand-target-diagnostics cargo run -p xtask -- gates check`, and `git diff --check` passed.
- lesson:
  - Diagnostics/log UI must compare DOM rows to ADP owner projection, prove redaction/no absolute-path leakage, and prove the global session list is unchanged.

# 2026-07-25 mobile UI tree home/settings IA closeout

- implementation:
  - Mobile home body is now only two cards: `active sessions monitor` and `master session history`.
  - Timer dashboard and New Session remain reachable only from the mobile corner entries, not duplicated as body dashboard cards.
  - Settings top-level entries are split into LLM Provider, Diagnostics, Agent Runtime, and Android Shell.
  - LLM Provider is hierarchical: Provider configuration is separate from Provider switching and strategy.
  - Diagnostics is a top-level Settings page, not nested under Provider settings.
  - Provider registry/model-group/web_search online verifiers now restore config/env with service-scoped `launchctl kickstart -k gui/<uid>/com.freehand.daemonS` instead of rebuilding through `install-launchd.sh restartS` in `finally`.
- online proof:
  - `FREEHAND_WEBUI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-mobile-ui-tree-online.mjs` passed with artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260725T041144-9793`.
  - `FREEHAND_WEBUI_DIAGNOSTICS_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-diagnostics-online.mjs` passed with artifact `artifacts/webui-online/webui-diagnostics-1784952756601`.
  - `FREEHAND_WEBUI_TOOLS_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-tools-registry-online.mjs` passed with artifact `artifacts/webui-online/webui-tools-registry-20260725T041236-15439`.
  - `FREEHAND_WEBUI_TIMER_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-timer-dashboard-online.mjs` passed with timer `timer-master-source-less-ui-1784952762021826000-1`.
  - `FREEHAND_SESSION_SEARCH_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-session-search-online.mjs` passed with fixed session `webui-session-search-fixed`.
  - `FREEHAND_NEW_SESSION_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-new-session-online.mjs` passed with fixed sessions `webui-new-conversation-fixed` and `webui-new-task-fixed`.
  - `FREEHAND_PROVIDER_REGISTRY_UI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-provider-registry-ui-online.mjs` passed with artifact `artifacts/webui-online/provider-registry-ui-1784953122380`.
  - `FREEHAND_MODEL_GROUP_UI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-model-group-ui-online.mjs` passed with artifact `artifacts/webui-online/model-group-ui-1784953154684`.
  - `FREEHAND_PROVIDER_WEB_SEARCH_UI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-provider-web-search-settings-ui-online.mjs` passed with artifact `artifacts/webui-online/provider-web-search-settings-ui-1784953187858`.
- local proof:
  - `node --check` passed for `apps/freehand-server/assets/webui.js` and touched WebUI verifier scripts.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-settings-ia cargo test -p freehand-server --lib -- --nocapture --test-threads=1` passed 19/19.
  - `cargo fmt --check`, `CARGO_TARGET_DIR=/tmp/freehand-target-settings-ia cargo run -p xtask -- mainlines check`, `CARGO_TARGET_DIR=/tmp/freehand-target-settings-ia cargo run -p xtask -- gates check`, and `git diff --check` passed.
- restore proof:
  - Final `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` returned `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search_effective=hosted_declared`, and `auth_source=inline`.
  - Fixture env grep returned 0 matches.

# 2026-07-25 mobile UI tree Phase 2 Provider web_search Settings UI proof

- scope:
  - Current work closes the Provider Settings live-test UI proof gap: the Settings UI `Test web_search` buttons must actually submit `TestProviderWebSearch` and render owner-backed pass/fail status.
  - Full Phase 2 remains open only where true-device Android proof is blocked by device lockscreen and for final requirement audit.
- implementation:
  - Added `scripts/verify-provider-web-search-settings-ui-online.mjs`.
  - The verifier opens production S-profile WebUI Settings, proves `minimax` is visible as `anthropic/messages` with `web_search=auto -> hosted_declared`, clicks the `minimax` provider card's `Test web_search` button, and waits for DOM pass status.
  - It then adds an OpenAI/Responses fixture provider through the real Settings form, proves owner-projected `hosted_declared`, clicks that fixture provider card's `Test web_search` button, captures the fixture request, and verifies `tools=[{"type":"web_search"}]` without a function tool named `web_search`.
  - It restores `~/.freehand/config.toml` and `~/.freehand/daemonS.env`, service-scoped restarts S-profile, and verifies the fixture provider/env marker is gone.
- online proof:
  - First failed attempt `artifacts/webui-online/provider-web-search-settings-ui-1784944848042` was real product evidence: Settings button returned a provider failure because MiniMax did not observe hosted search in that attempt. CLI immediately after passed on the same S-profile, proving the provider/path can work and the initial failure was not a missing UI command route.
  - Script bug in browser-context predicate was fixed after second attempt hit `ReferenceError: fixtureProviderId is not defined`; this was verifier-only, not product code.
  - Final proof passed:
    - `FREEHAND_PROVIDER_WEB_SEARCH_UI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-provider-web-search-settings-ui-online.mjs`
    - output: `provider_web_search_settings_ui_ok url=http://127.0.0.1:4042/ adp=ws://127.0.0.1:4042/adp minimax=passed openai_responses=passed fixture_requests=1 artifactDir=/Volumes/extension/code/freehand/artifacts/webui-online/provider-web-search-settings-ui-1784945356860`
- restore proof:
  - The verifier final summary recorded `restored=true`, final provider `minimax`, final model `MiniMax-M3`, and fixture env grep `matchCount=0`.
- local proof:
  - `node --check scripts/verify-provider-web-search-settings-ui-online.mjs` passed.
- lesson:
  - Provider capability UI closure requires a browser-click proof from Settings to `TestProviderWebSearch`, not just CLI/ADP proof. For provider-hosted search, also prove the provider request declares hosted `web_search` and does not expose a local function tool named `web_search`.

# 2026-07-25 mobile UI tree final audit scaffold and Android blocker

- scope:
  - Current work did not add runtime/UI semantics. It adds a reusable final-audit verifier for the active mobile UI tree goal and records current true-device blocker evidence.
  - The verifier is foundation/workspace evidence tooling: it reads already accepted WebUI/Android artifact summaries, live S-profile config, `~/.freehand/daemonS.env`, and ADB lockscreen signals. It does not create sessions or mutate owner truth.
- implementation:
  - Added `scripts/verify-mobile-ui-tree-goal-audit.mjs`.
  - Updated `docs/function-maps/foundation.workspace.md` and `docs/testing/foundation.workspace.md` to document the audit verifier and its blocked-vs-failed behavior.
  - The script outputs `summary.json` and `report.md` under `artifacts/webui-online/<run-id>/`, classifying entries as `passed`, `blocked`, `missing`, `failed`, or `weak`.
- evidence:
  - `node scripts/verify-mobile-ui-tree-goal-audit.mjs` returned `mobile_ui_tree_goal_audit_blocked artifactDir=/Volumes/extension/code/freehand/artifacts/webui-online/mobile-ui-tree-goal-audit-1784946703448 passed=18 blocked=1 missing=0 failed=0 weak=0`.
  - The only blocker in that summary is `android_true_device`, with latest evidence `artifacts/android-device/20260725T022834Z-100.104.163.65_5555-92759/summary.json`.
  - Official Android verifier re-run with no reinstall: `FREEHAND_ANDROID_SKIP_INSTALL=1 FREEHAND_ANDROID_SERIAL=100.104.163.65:5555 apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` exited blocked with `device_locked_or_dreaming`; window truth remained `mCurrentFocus=NotificationShade`, `mFocusedApp=com.zterm.android/.MainActivity`, `mDreamingLockscreen=true`.
  - S-profile restore proof after audit: `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` returned `provider=minimax`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search_effective=hosted_declared`, `auth_source=inline`; fixture env grep returned 0 matches.
- validation:
  - `node --check scripts/verify-mobile-ui-tree-goal-audit.mjs` passed.
  - `cargo fmt --check` passed.
  - `cargo run -p xtask -- mainlines check` passed.
  - `cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- remaining gap:
  - The mobile UI tree goal is not fully complete while the Android true-device WebView/update/permission/notification proof remains blocked by the locked/dozing device.
# 2026-07-25 Settings IA layering / partial marker closeout

- scope:
  - Current work fixes Settings first-level IA only. It does not add provider/config/runtime semantics.
  - First-level Settings is now grouped as `Models`, `Agent Runtime`, `Connectivity`, `Observability`, `Appearance`, and `About`.
  - Provider configuration and Provider switching/strategy remain second-level pages under Models.
  - Diagnostics is an Observability detail, not a Provider child or flat first-level LLM-adjacent entry.
  - Status marker semantics are locked: green hollow square = owner-backed, orange hollow square = partial, red hollow square = placeholder/not implemented.
- implementation:
  - Production WebUI asset version bumped to `20260725-settings-layer-ui`.
  - `apps/freehand-server/src/page.rs` renders grouped Settings nav cards and a green/orange/red legend.
  - `apps/freehand-server/assets/webui.js` renders grouped review tree rows and preserves `partial` instead of mapping every non-ok row to red.
  - `apps/freehand-server/assets/webui.css` adds `.settings-status-marker.partial { border-color: var(--running); }`.
  - Static prototype config tree mirrors the grouped Settings IA and orange partial marker.
  - Function map, test design, mainline JSON, and generated wiki were synced.
- online proof:
  - S-profile deployed with `/tmp/freehand-target-settings-layer`; health and `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
  - `FREEHAND_WEBUI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-mobile-ui-tree-online.mjs` passed with artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260725T045905-74490`.
  - Artifact checks prove first-level titles `Models, Agent Runtime, Connectivity, Observability, Appearance, About`, no flat `LLM Provider`, `partial` marker counts in both nav/review tree, hollow markers, and `Diagnostics` under Observability.
  - `FREEHAND_WEBUI_DIAGNOSTICS_CHROME=... node scripts/verify-webui-diagnostics-online.mjs` passed with artifact `artifacts/webui-online/webui-diagnostics-1784955564587`.
  - `FREEHAND_PROVIDER_REGISTRY_UI_CHROME=... node scripts/verify-provider-registry-ui-online.mjs` passed with artifact `artifacts/webui-online/provider-registry-ui-1784955579027`.
  - `FREEHAND_MODEL_GROUP_UI_CHROME=... node scripts/verify-model-group-ui-online.mjs` passed with artifact `artifacts/webui-online/model-group-ui-1784955618922`.
  - First `verify-provider-web-search-settings-ui-online.mjs` run failed because MiniMax hosted web_search returned provider text error without observing a hosted search call; config/env restored. Immediate rerun passed with artifact `artifacts/webui-online/provider-web-search-settings-ui-1784955890342`, proving the UI route still works and the first failure was provider-side transient.
- local proof:
  - `node --check apps/freehand-server/assets/webui.js` and touched WebUI verifier scripts passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-settings-layer cargo test -p freehand-server --lib -- --nocapture --test-threads=1` passed 19/19.
  - `cargo fmt --check`, `CARGO_TARGET_DIR=/tmp/freehand-target-settings-layer cargo run -p xtask -- mainlines generate/check`, `CARGO_TARGET_DIR=/tmp/freehand-target-settings-layer cargo run -p xtask -- gates check`, and `git diff --check` passed.
- restore proof:
  - Final config query returned `provider=minimax`, `fallback_provider=cc`, `provider_protocol=messages`, `base_url_host=api.minimaxi.com`, `default_model=MiniMax-M3`, `web_search_effective=hosted_declared`, and `auth_source=inline`.
  - Fixture env grep returned 0 matches for provider retry, master autonomy, provider registry, provider web_search, and model group verifier keys.

# 2026-07-25 Chinese WebUI menu + Android APK rebuild

- scope:
  - User-facing WebUI/menu/settings/status labels were localized to Chinese while preserving internal symbols, DOM ids/data attributes, ADP variant strings, protocol field names, and verifier selectors in English.
  - Production WebUI asset version is `20260725-zh-menu-ui`.
  - Android remains a thin WebView/platform bridge; no native Android product menu was added.
- implementation/truth sync:
  - Updated daemon WebUI source (`apps/freehand-server/assets/webui.js`, `apps/freehand-server/src/page.rs`, `apps/freehand-server/assets/webui.css`) plus server assertions and online verifiers for Chinese visible labels.
  - Synced `app.webui-smoke` function map, mainline call manifest, test design, generated wiki, and local `freehand-dev` skill.
  - Rebuilt Android release APK `dist/android/freehand-android-release.apk`, versionCode `20260726`, versionName `0.2.4`, SHA-256 `3acf6e2ed9510fd3850b25c0abb042170bf7b95a0656e8d56aa15b68f77e0a8f`; `dist/android/update.json` and runtime-home staged APK match that hash.
- online/browser proof:
  - Served root after S-profile restart advertises only asset version `20260725-zh-menu-ui`.
  - `FREEHAND_WEBUI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell node scripts/verify-webui-mobile-ui-tree-online.mjs` passed after restart with artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260725T104042-44224`; checks include Chinese Settings root (`模型`, `智能体运行时`, `连接`, `可观测性`, `外观`, `关于`), split model subpages, Diagnostics under Observability, no horizontal overflow, and stable internal selectors.
  - Local and relay update endpoints served `update.json` versionCode `20260726`; `/android/freehand-android.apk` SHA matched the rebuilt dist/runtime-home artifact and used no-store headers.
- Android true-device proof:
  - `FREEHAND_ANDROID_APK=dist/android/freehand-android-release.apk ANDROID_HOME=$HOME/Library/Android/sdk JAVA_HOME=/Applications/Android Studio.app/Contents/jbr/Contents/Home apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` passed with artifact `artifacts/android-device/20260725T104143Z-100.104.163.65_5555-44388`.
  - `FreehandWebUiLayout` logged relay asset URLs with `?v=20260725-zh-menu-ui`, `layoutClient=android-webview`, `layoutShape=tall_phone`, `webuiShell=true`, `webuiCssApplied=true`, and `webuiJsReady=true`; `FreehandFileAccess` showed versionCode `20260726` and granted state.
  - Manual device interaction proof under `.../settings-menu-proof/` captured Settings tap screenshot showing Chinese `系统设置` menu and physical Back returning to Chinese home; post-interaction logcat had no Freehand fatal/exception pattern.
- validation:
  - Passed: WebUI/verifier `node --check`, `git diff --check`, `cargo fmt --check`, `CARGO_TARGET_DIR=/tmp/freehand-target-zh-menu cargo test -p freehand-server -- --nocapture --test-threads=1` (19/19), Android `./gradlew testDebugUnitTest assembleRelease`, Android `./gradlew --rerun-tasks assembleRelease`, `apksigner verify --verbose --print-certs`, `xtask mainlines check`, and `xtask gates check`.
- lesson:
  - Chinese localization must be visible-text only; run a CJK-in-internal-identifier audit before build/gates, and prove the changed menu in both browser and Android WebView when phone-visible.

# 2026-07-25T11:41:47Z session restore unlock continuation

- run_id: 20260725T114147Z-Macstudio.local-90958-78230c
- scope: continue handoff for WebUI deadlocked session restore, ToolPending lifecycle mislabel, session refresh exit path, and APK rebuild after Chinese menu closeout.
- initial guard refresh: USER/profile read, freehand-dev skill read, worktree/collab claims inspected; existing claims for app.webui-smoke_session_restore_exit, reason.persistence_partial_ui_restore, and runtime.ui-command-dispatch_session_query_restore are same-task handoff claims.
- continuation edit: fixed accidental visible-text localization leaking into internal mobile drawer state comparison (`drawer === "settings"` restored for Settings aria-expanded). Visible Chinese labels remain unchanged.
- docs/verifier update: added `scripts/verify-webui-session-restore-error-exit-online.mjs` and synced reason.persistence/runtime.ui-command-dispatch/app.webui-smoke docs for inactive partial restore warning, active incomplete hard error, ToolPending owner-evidence classification, and session-local refresh-error exits.

# 2026-07-25T12:08:34Z session unlock frozen-card continuation

- run_id: `20260725T114147Z-Macstudio.local-90958-78230c`
- refreshed USER/profile, freehand-dev, CACHE/MEMORY/note tail, resource/function/test maps, MemoryPalace, collab kill switch/claims.
- observed focused verifier blocker: frozen terminal `.turn-cycle-card` reuse keeps old lifecycle label `等待生命周期` after owner projections classify the same ToolPending turn as `等待用户选择`; header/session summary already updates.
- planned code owner path: `app.webui-smoke` WebUI cycle-card reconciliation only; reason/runtime restore code already locally verified by prior commands.

# 2026-07-25T16:16:40Z mobile session home / APK 0.2.6 closeout

- run_id: `20260725T141849Z-Macstudio.local-57457-8ca53856`
- scope:
  - Corrected mobile session home to be an in-flow page surface, not a floating session-list overlay.
  - Mobile home now has `正在运行` above `历史会话`; running sessions are uncapped and removed from the history list by session id.
  - Header session tree is an inline selected-session relationship panel only, not the main session list.
  - Session refresh error exits remain session-local; `新建会话`, `返回会话列表`, Android Back, and `忽略错误` are covered.
  - Rebuilt Android APK versionCode `20260728`, versionName `0.2.6`.
- implementation/truth sync:
  - Updated `apps/freehand-server/src/page.rs`, `apps/freehand-server/assets/webui.js`, `apps/freehand-server/assets/webui.css`, and server asset smoke assertions for `20260725-session-panel-ui`.
  - Updated `scripts/verify-webui-mobile-ui-tree-online.mjs` to assert running/history session id disjointness and no floating session tree.
  - Updated `scripts/verify-webui-session-restore-error-exit-online.mjs` to assert clean selected-session state instead of transient command-status text after refresh-error new-session exit.
  - Synced `app.webui-smoke` function map, mainline call manifest, generated wiki, test design, and local `freehand-dev` skill.
  - Updated Android `gradle.properties` and `dist/android/update.json`; staged runtime-home update manifest/APK.
- online proof:
  - `scripts/install-launchd.sh restartS` restarted S-profile and relay.
  - Local and relay endpoints served `update.json` versionCode `20260728`; `/android/freehand-android.apk` hash matched `dist/android/freehand-android-release.apk`.
  - `FREEHAND_WEBUI_CHROME=... node scripts/verify-webui-mobile-ui-tree-online.mjs` passed with artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260725T160711-98388`; portrait snapshots show `mobileHomeRunningHistoryOverlap=[]`, `mobileHomeFloatingTree=false`, and home sections `正在运行` / `历史会话`.
  - `FREEHAND_SESSION_RESTORE_CHROME=... node scripts/verify-webui-session-restore-error-exit-online.mjs` passed with artifact `artifacts/webui-online/webui-session-unlock-1784995916116`; all checks true, including `browserToolPendingWaitsForUserChoice`, `browserProblemTurnNotLifecycle`, Android Back exit, and clean new-session exit.
- APK proof:
  - Android `./gradlew testDebugUnitTest assembleRelease` passed.
  - `apksigner verify --verbose --print-certs` passed with v2 signature.
  - APK version inspected as `20260728 / 0.2.6`.
  - APK SHA-256: `602875167d259d8d9eff21a04ecc2deef4653dd718dc090c3026641dc459bca8`.
  - `dist/android/freehand-android-release.apk` and `~/.freehand/dist/android/freehand-android-release.apk` hashes match; manifest hashes match.
- local validation:
  - Passed: `node --check` for WebUI asset and touched verifier scripts, CJK-in-internal-selector audit, `cargo fmt --check`, `cargo test -p freehand-server --lib -- --nocapture --test-threads=1` 19/19, `cargo run -p xtask -- mainlines generate/check`, `cargo run -p xtask -- gates check`, and `git diff --check`.
  - Full `cargo test -p freehand-server -- --nocapture --test-threads=1` and a later repeat `--lib` command both hit a harness/session hang after prior pass evidence; they were interrupted by Ctrl-C on the specific terminal session only.
- blocked proof:
  - True-device Android verifier is blocked: `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` returned `adb_state_unavailable` at `artifacts/android-device/20260725T160731Z-100.104.163.65_5555-98443`, and `adb connect 100.104.163.65:5555` timed out.
  - MemoryPalace `mine /Volumes/extension/code/freehand --wing freehand --agent codex` is blocked by existing lock holder PID `16461` (`mempalace mine /Users/fanzhang/Documents/github/routecodex ...`). Search still works, but the new note/MEMORY entries were not re-mined in this turn.

# 2026-07-25T16:22:40Z post-handoff verification closeout

- run_id: `20260725T162240Z-Macstudio.local-21776-25bc03ea`
- MemoryPalace blocker cleared:
  - `mempalace mine /Volumes/extension/code/freehand --wing freehand --agent codex` completed.
  - Processed 75 files including `note.md`, `MEMORY.md`, `.agents/skills/freehand-dev/SKILL.md`, `webui.js`, `webui.css`, `page.rs`, `lib.rs`, and related mainline/function-map docs.
  - `mempalace search --wing freehand --results 5 "Mobile session home redesign closeout running history disjoint"` returned current `note.md`, local `SKILL.md`, and `app.webui-smoke.md` records in top results.
- ADB remains blocked:
  - `adb devices -l` returned no attached devices.
  - `adb connect 100.104.163.65:5555` timed out again.
- Rechecked gates after handoff:
  - `node --check` passed for WebUI asset and all touched WebUI verifier scripts.
  - `cargo test -p freehand-reason ui_restore_ -- --nocapture --test-threads=1` passed 4/4.
  - `cargo fmt --check`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check` passed.
  - Local/relay Android update manifests still served versionCode `20260728`, versionName `0.2.6`; dist/runtime-home APK SHA-256 still match `602875167d259d8d9eff21a04ecc2deef4653dd718dc090c3026641dc459bca8`.

# 2026-07-25T23:58:00Z Master parent/child stale waiting closeout

- run_id: `20260725T231358Z-Macstudio.local-42087-d961b27d`
- scope:
  - Investigated Jason's report that Master parent state waited forever while a child Agent/task had already closed.
  - Target owner: `runtime.master-worker-loop`; touched runtime completion-schema gating, provider-visible session-history projection, maps/tests/skill.
- live diagnosis:
  - S-profile ADP was reachable: `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
  - Concrete stuck sample `webui-session-20260723001509-bd98e156` / child `task-1784765749` proved the child notification path itself had fired: task snapshot status was `closed`, history ended in `TaskReviewSubmitted -> TaskReviewApproved -> TaskClosed`, and `~/.freehand/state/master-loop/master.json` contained `completed_parent_evaluations` entry `webui-session-20260723001509-bd98e156|task-1784765749:385` with no pending attention.
  - The same session history contained parent evaluation turn `runtime-turn-523` as `Success`, so the specific child close was not lost.
  - Remaining visible stuck state was later turn `runtime-turn-541-r3`, persisted as `TerminalStatus::ToolPending` from a user-choice wait (`Waiting for the user to pick option ...`) despite no same-session child/timer owner truth that could wake it.
  - Provider-visible `SessionHistory.base_context_segments` also leaked the internal `<freehand_parent_evaluation ...>` user prompt from `runtime-turn-523` into later Master requests through `historical_turn:runtime-turn-523`.
- root cause:
  - Runtime rejected premature Master `claim="complete"` while child tasks were open, but accepted Master `claim="waiting"` even when all same-session child tasks were terminal and no source timer was active/running. That created lifecycle `ToolPending` with no owner that could resume it.
  - UI projection hid internal parent/timer prompts, but rebuilt provider-visible session memory used the UI-derived/original-task candidate without reapplying internal-prompt hiding against raw `request.user_text` and effective user text.
- implementation:
  - `master_session_completion_rejection` now gates Master user-session `claim="waiting"` by owner truth: open same-session child task or active/running source timer. Without that owner truth, schema repair forces `claim="blocked"` for user choice or `claim="complete"` only with evidence.
  - `master_session_lifecycle_owner_truth` reads `TaskRuntime` TaskBoard plus `TimerStore` schedules and treats terminal children as non-waking.
  - `turn_projection::turn_context_segment` now uses `model_history_user_text_for_turn`, hiding internal parent/timer/framework prompts from provider-visible `SessionHistory.base_context_segments` while retaining terminal assistant summaries.
  - Synced `docs/function-maps/runtime.master-worker-loop.md`, `docs/testing/runtime.master-worker-loop.md`, `docs/mainline-calls/runtime.master-worker-loop.json`, generated wiki, and local `freehand-dev` skill.
- red/green proof:
  - `effective_context_hides_internal_parent_evaluation_prompt` failed before the projection fix because `<freehand_parent_evaluation>` leaked into provider-visible context; passed after the fix.
  - `live_master_rejects_waiting_when_child_tasks_are_terminal_and_no_owner_will_wake` failed before the runtime gate because stale waiting persisted as `ToolPending` and no schema repair request was sent; passed after the fix.
- local validation:
  - `CARGO_TARGET_DIR=/tmp/freehand-target-parent-closure cargo test -p freehand-runtime effective_context_ -- --nocapture --test-threads=1` passed 2/2.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-parent-closure cargo test -p freehand-runtime live_master_ -- --nocapture --test-threads=1` passed 6/6.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-parent-closure cargo test -p freehand-runtime runtime_query_session_turns_restores_background_parent_evaluation -- --nocapture --test-threads=1` passed.
  - `cargo fmt --check` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-parent-closure cargo run -p xtask -- mainlines check` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-parent-closure cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- online proof:
  - Rebuilt current workspace binaries with `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/tmp/freehand-target-parent-closure cargo build -p freehand-cli -p freehand-server -p freehand-daemon` and restarted the S daemon with that binary copy.
  - First online verifier attempt used repo root cwd and failed before provider IO with `instruction capability build timed out after 30s while reading AGENTS.md/skills`; no fixture provider requests were made and config/env restore succeeded. This confirmed the verifier setup was wrong, not runtime waiting logic.
  - Retried with minimal cwd `/tmp/freehand-parent-closure-cwd` and temporary fixture provider. The first fixture response returned `claim="waiting"` with no child/timer owner truth; the runtime sent a second provider request containing schema-repair feedback `claim=\`waiting\` requires open Task Center or timer owner truth`, and the second response closed as blocked.
  - Online artifact: `artifacts/runtime-online/parent-child-closure-20260725T235515-32394/result.json`.
  - Checks: `inScopeRequestCount=2`, `firstRequestHadToken=true`, `secondRequestHadWaitingRejection=true`, `secondRequestHadRepairLanguage=true`, `terminalStatus=Blocked`, `noTerminalToolPending=true`, `outOfScopeRequestCount=0`. Transcript turns: `runtime-turn-550` non-terminal repair round, `runtime-turn-550-r2` terminal `Blocked`.
  - Post-restore: fixture env grep for `FREEHAND_PARENT_CLOSURE_FIXTURE_KEY` / `FREEHAND_TEST_DISABLE_MASTER_LIFECYCLE_RUNNER` returned no matches, config had no `parent-closure-fixture`, health was `ok`, and `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` passed.
- lesson:
  - Parent/child closure must be checked at owner truth, not from UI state. A terminal child plus no timer cannot justify lifecycle `ToolPending`; user-choice waits are blocked/user-needed terminal states.
  - Runtime online verifiers that are not testing instruction-capability should use a minimal cwd to avoid loading the full repo local AGENTS/skills before the behavior under test.

# 2026-07-26T00:04:00Z staged-index verification

- Applied `git diff --cached` to clean detached worktree `/tmp/freehand-parent-closure-staged.vmp91B` from HEAD to prove the commit scope independently of unrelated dirty files.
- Passed there: `effective_context_`, `live_master_`, `runtime_query_session_turns_restores_background_parent_evaluation`, `cargo fmt --check`, `xtask mainlines check`, `xtask gates check`, and `git diff --check`.

# 2026-07-26T00:46:00Z crash/restart lifecycle closure

- run_id: `20260726T004623Z-Macstudio.local-44975-af0ed29f`
- scope: Jason corrected the target from prior stale-wait symptom chasing to a full Master/Slave lifecycle closure contract: if Master or Worker/Slave exits mid-lifecycle because of provider/system/process failure, the next daemon start must recover from owner truth and close/retry/block without requiring a user message to escape the state.
- first owner target: `runtime.master-worker-loop` with TaskRuntime leases, AgentLifecycle, EventInbox, MasterLoopState retry cursor, and master active-work checkpoints as durable truth; need prove foreground Master crash, background Master lifecycle crash/provider error, Worker Running crash/lease expiry, Worker provider/system interruption, and pending attention restart.

# 2026-07-26T01:30:00Z parent next-round context lifecycle fix

- Online three-worker verifier did not fail because Workers failed to close: offline artifact `/tmp/freehand-three-worker-home.x4cBSa/.freehand` shows alpha/beta/gamma/integration all `closed`, gamma provider failure wrote `TaskInterrupted`, Master reassigned same gamma task to `worker-alpha`, and integration closed.
- Real lifecycle gap found in owner truth: after integration closed, `state/master-loop/master.json` kept pending `task-three-worker-1781785027647-integration:13` with `retry_attempt=13`; parent evaluation prompt contained only integration accepted review truth, missing prior alpha/beta/gamma accepted truth, so the fixture correctly returned `parent evaluation missing accepted worker results: alpha,beta,gamma` and the parent could not close.
- Fix target: `runtime.master-worker-loop` parent evaluation keeps current closed workset as idempotency key, but prompt context now widens to same-objective prior closed child review truth since the latest external user objective ordinal, while excluding older user-turn child truth. Positive/negative tests added for final integration context and prior-user-turn exclusion.

# 2026-07-26T01:39:00Z lifecycle online proof after parent context fix

- Online isolated three-worker verifier passed after the parent next-round context fix: session `online-master-three-worker-evaluation-1785029016`, evidence dir `/tmp/freehand-three-worker-home.IuTFG5/.freehand/tmp/three-worker-e2e-20260726T092336-71162`.
- Owner truth: alpha closed, beta rejected then closed, gamma provider failure wrote `TaskInterrupted` then same-task takeover to `worker-alpha` and closed, integration task closed, final parent turn `runtime-turn-3` was `Success` with all four worker_result tokens, restart idempotency kept `final_evaluation_count=1`.
- Final Master loop state had `pending_attention=[]`, `retry_event_id=null`, `retry_attempt=0`, and completed parent evaluations for both first child set and integration set.

# 2026-07-26T01:37:00Z clean staged online proof

- To remove dirty-worktree ambiguity, applied `git diff --cached` to detached clean worktree `/tmp/freehand-lifecycle-staged-20260726T013236Z`, built daemon/CLI with `/tmp/freehand-target-lifecycle-staged-online`, and reran `scripts/verify-master-three-worker-e2e-online.sh` from that clean staged tree.
- Clean staged online proof passed: session `online-master-three-worker-evaluation-1785029697`, evidence dir `/tmp/freehand-three-worker-home.VtEpeP/.freehand/tmp/three-worker-e2e-20260726T093457-96986`, final `runtime-turn-3` Success with all four worker_result tokens, gamma explicit PID restart `99273 -> 2375`, restart idempotency `final_evaluation_count=1`.

# 2026-07-26T02:05:00Z mobile UI tree correction

- Jason corrected the approach: stop patching individual WebUI symptoms and first lock the full mobile UI tree.
- Design baseline added:
  - `docs/design/mobile-webui-ui-tree.md`
  - `docs/design/mobile-webui-ui-tree.manifest.json`
- Locked route split:
  - `Home` owns global `正在运行` and `历史会话`.
  - `SessionDetail(session_id)` owns one selected session transcript and composer.
  - The two body surfaces are mutually exclusive on phone.
- Lifecycle UI closure rule:
  - `等待用户选择` without open task/timer/Master retry owner truth is not `正在运行`.
  - stale `active_turn_id` / historical `ToolPending` cannot keep a session in Home running after owner truth closes.
- Tools registry design:
  - phone-first read-only owner projection page/sheet with sticky close/refresh, compact summary, collapsed details, and no document-level horizontal overflow.

# 2026-07-26T02:12:00Z mobile Home dashboard correction

- Jason clarified normal Home behavior:
  - four corner entries remain quick entrances.
  - center Home is a concise dashboard for running sessions plus time-ordered historical sessions.
  - Home must support session CRUD management through owner paths.
  - Home rows are for scan/status/open/manage, not expanded transcript/task/tool/debug dumps.
  - selecting a session enters `SessionDetail` to continue the conversation/work or inspect details.
- Updated `docs/design/mobile-webui-ui-tree.md` and manifest:
  - added `DashboardHeader` and compact row fields.
  - added owner-backed CRUD actions.
  - forbade default Worker child expansion, full transcript/task/event rows, raw ids, and browser-local CRUD truth on Home.

# 2026-07-26T02:18:00Z mobile Home history buckets

- Jason clarified Home history grouping:
  - history is grouped into exactly one line each for `今天`, `过去一周`, and `所有更早的`.
  - keep chronological rows inside those buckets.
  - do not add deeper date/month/year trees or extra headings by default.
- Updated `docs/design/mobile-webui-ui-tree.md`, manifest, and memory.

# 2026-07-26T02:20:00Z mobile Home one-row session rule

- Jason clarified: one session occupies one row.
- Updated mobile UI tree:
  - Home uses one-line compact rows.
  - row text is clipped/ellipsis.
  - no default multi-line cards or inline expansion.
  - full detail is only in `SessionDetail` or explicit detail/action sheet.

# 2026-07-26T02:23:00Z adaptive portrait layout rule

- Jason clarified: every WebUI surface must auto-layout for portrait by height/width ratio.
- Updated mobile UI tree and manifest:
  - layout input is width + height + height/width ratio + orientation + safe-area.
  - every route adapts, not only Home.
  - portrait shows one primary surface at a time.
  - Tools/Settings/Timer/Search/New use portrait-safe sheets/pages with sticky close/back.
  - long schema/log/prose must scroll or wrap internally, never widen the page.
  - layout changes preserve selected session/draft/route/scroll/pending submit/owner query state.

# 2026-07-26T03:43:32Z mobile modular WebUI implementation continuation

- run_id: `20260726T034240Z-Macstudio.local-8755-rbu80l`
- scope: implement Jason's mobile WebUI tree execution, not another plan: modular split from giant `webui.js`, explicit route/edge registry, Home vs SessionDetail mutual exclusion, compact one-row Home history, phone portrait-safe Tools layout, owner-projection-only CRUD surfaces.
- pre-edit checks refreshed: USER/profile, freehand-dev, CACHE/MEMORY/note, MemoryPalace search, app.webui-smoke function/test/mainline docs, collab kill switch/claims.

# 2026-07-26T05:40:00Z mobile modular WebUI continuation

- run_id: `20260726T052624Z-Macstudio.local-34723-jczbaq`
- objective source: `/Users/fanzhang/.codex/attachments/db4fa4e5-a5cd-4bf6-9bf7-c08e347fe49f/pasted-text-1.txt` says implement, not write a plan.
- implementation progress:
  - Added independent surface modules for `SessionDetail`, `SessionSearch`, `NewSession`, `Settings`, plus control modules for `ToolsRegistry` and `TimerDashboard`.
  - Legacy WebUI now delegates selected-session switching, New dialog mode/submit, SessionSearch render, Settings shell/diagnostics/nav, Tools open/refresh, and Timer open/refresh/schedule/cancel to surface modules.
  - Asset server now serves the additional surface module files.
  - Mobile/Android back intent now closes visible sheets/dialogs before leaving `SessionDetail`, matching the UI tree edge order.
- verification so far:
  - `node --check` passed for WebUI entry, bootstrap, legacy, and all WebUI submodules.
  - `node scripts/verify-webui-layout-shapes.mjs` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-mobile-modular cargo test -p freehand-server webui -- --nocapture --test-threads=1` passed 3/3 after updating asset smoke to check new modules.
- remaining:
  - Sync function-map/mainline/test docs/wiki/memory.
  - Run full local gates and S-profile online verifier; Android true-device only if ADB is reachable.

# 2026-07-26T06:35:00Z mobile modular WebUI validation/commit prep

- run_id: `20260726T052624Z-Macstudio.local-34723-jczbaq`
- implementation closeout:
  - `apps/freehand-server/assets/webui.js` is a thin bootstrap shell.
  - Split modules exist for `app-shell` (`adp-client`, `edge-registry`, `layout-shape`, `route-controller`) and surfaces `Home`, `SessionDetail`, `ToolsRegistry`, `TimerDashboard`, `Settings`, `SessionSearch`, and `NewSession`.
  - `legacy-monolith.js` now delegates Home rendering, selected-session switching, Tools/Timer controls, Settings shell/diagnostics/nav, Search rendering, and New session controls to split modules.
  - `apps/freehand-server/src/assets.rs` serves the split module assets.
  - Mobile Home/Sessions route behavior follows the locked UI tree: `Home` owns running/history dashboard; `SessionDetail(session_id)` hides the Home body on phone; Android/browser Back closes visible sheets/dialogs before route exit.
- online proof:
  - S-profile restarted with `FREEHAND_LAUNCHD_HEALTH_WAIT_SECONDS=90 scripts/install-launchd.sh restartS` after the old daemon served stale assets.
  - Passed: `node scripts/verify-webui-mobile-ui-tree-online.mjs`, artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260726T055239-75558`; it fetched split module assets and proved Home/SessionDetail mutual exclusion, one-row/fixed-bucket Home, Settings route, and no portrait overflow.
  - Passed: `node scripts/verify-webui-tools-registry-online.mjs`, artifact `artifacts/webui-online/webui-tools-registry-20260726T055254-75609`.
  - Passed: `node scripts/verify-webui-timer-dashboard-online.mjs`, artifact `artifacts/webui-online/webui-timer-dashboard-20260726T055254-75611`.
  - Passed: `node scripts/verify-webui-session-search-online.mjs`, artifact `artifacts/webui-online/webui-session-search-1785045174090`.
  - Passed: `node scripts/verify-webui-new-session-online.mjs`, artifact `artifacts/webui-online/webui-new-session-1785045244650`.
- local validation:
  - Passed JS checks for `apps/freehand-server/assets/webui.js`, `apps/freehand-server/assets/webui/bootstrap.js`, all split `apps/freehand-server/assets/webui/**/*.js`, and touched WebUI verifiers.
  - Passed `node scripts/verify-webui-layout-shapes.mjs`.
  - Passed `CARGO_TARGET_DIR=/tmp/freehand-target-mobile-modular cargo test -p freehand-server webui -- --nocapture --test-threads=1`.
  - Passed `CARGO_TARGET_DIR=/tmp/freehand-target-mobile-modular cargo test -p freehand-daemon -- --test-threads=1` 21/21 after updating daemon tests for current Master tool exposure and continuation-round user_text projection.
  - Passed `CARGO_TARGET_DIR=/tmp/freehand-target-mobile-modular cargo test -p freehand-cli --test config_startup -- --test-threads=1` 27/27 after updating the same Master tool exposure expectation.
  - Passed focused runtime regression `active_live_cancel_returns_before_provider_finishes_and_blocks_success_projection` after fixing its provider-release synchronization timeout.
  - Passed `cargo fmt --check`, `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check`.
- explicit gaps:
  - `cargo test --workspace` was interrupted once after `freehand-runtime` live tests stalled; targeted sample showed `active_live_cancel...` was waiting on provider release. The synchronization bug was fixed and its focused test passed.
  - Full serial `cargo test -p freehand-runtime -- --test-threads=1` is still not green: 241/250 passed, 9 failed in pre-existing adjacent live/autonomy/worker expectations (`live_bridge_admits_long_operator_task_without_semantic_truncation`, three Master autonomy tests, failed-tool projection, non-default session ordinal restore, `master_create_gate_rejects_implicit_dispatch_without_task_mutation`, `runtime_task_tool_mutation_publishes_task_list_projection`, and `production_worker_runner_clean_search_blocks_when_provider_has_no_hosted_search`). These are outside the WebUI modular surface owner slice and are not claimed closed.
  - Android true-device proof is blocked by lockscreen, not missing ADB: `adb connect 100.104.163.65:5555` succeeded, `verify-device-ui.sh` artifact `artifacts/android-device/20260726T062015Z-100.104.163.65_5555-9796` reports `device_locked_or_dreaming` with `mCurrentFocus=NotificationShade`, `mFocusedApp=com.freehand.android/.ui.MainActivity`, and `mDreamingLockscreen=true`. `node scripts/verify-mobile-ui-tree-goal-audit.mjs` now reports `mobile_ui_tree_goal_audit_blocked` with 18 passed and 1 blocked.
- lesson:
  - When WebUI is split into module assets, server asset smoke is not enough. The online verifier must fetch all module assets and capture browser runtime exceptions; the first stale-daemon run and `toolRegistryTools` `ReferenceError` proved that syntax/asset checks alone do not prove live bootstrap execution.

# 2026-07-26T08:27:09Z Home multi-select and SessionDetail rename closeout

- run_id: `20260726T075538Z-Macstudio.local-50695-webui-session-select-rename-closeout`
- scope: implement Jason's correction that Home is a session dashboard with multi-select management, while rename is not a list-row action and belongs only to the selected `SessionDetail(session_id)` header.
- implementation:
  - Home rows now render `.mobile-home-session-checkbox` selection controls and a compact batch bar with `全选`, `清空`, and `批量移除`.
  - Home inline rename/remove row actions were removed; Home keeps owner-backed multi-select remove only.
  - `home.rename_session` edge was removed; `session.rename_session` is scoped to `SessionDetail` with `rename_unselected_session` forbidden.
  - The current-session header now owns `#selected-session-rename-button`; it is hidden/disabled until the route is `session_detail` and the selected session is persisted.
  - `renameCurrentSession()` dispatches `session.rename_session`, calls ADP `RenameSession`, refreshes session list truth, and refreshes the selected session.
  - The UI tree, manifest, function map, mainline call map, test design, wiki, goal doc, and local skill were updated to lock this ownership split.
- local validation already passed in this slice:
  - WebUI/verifier JS syntax checks.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-session-select-rename cargo fmt --check`.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-session-select-rename cargo test -p freehand-server webui -- --nocapture --test-threads=1` passed 3/3.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-session-select-rename cargo run -p xtask -- mainlines check` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-session-select-rename cargo run -p xtask -- gates check` passed.
  - `git diff --check` passed.
- S-profile online proof:
  - Mobile UI tree verifier passed with artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260726T081741-71739`; checks included `homeMultiSelectWorks=true`, `homeRenameOnlyInSessionDetail=true`, `sessionDetailMutualExclusion=true`, `mobileRowsSingleLine=true`, `noHorizontalOverflow=true`, `modularWebuiAssets=true`, and `modularSurfaceAssets=true`.
  - Provider web_search Settings verifier passed with artifact `artifacts/webui-online/provider-web-search-settings-ui-1785053748916`; pre-restore summary proved `minimaxTestOutcome=passed`, fixture `fixtureRequestCount=1`, hosted tools `["web_search"]`, and function tools `[]`; final restore returned S config to `minimax/MiniMax-M3` with fixture env grep 0.
  - Model Group Settings verifier passed with artifact `artifacts/webui-online/model-group-ui-1785052967602`; it proved model-group upsert and selection UI paths, then restored provider/model/group truth to `minimax/MiniMax-M3` and no active model group.
- Android true-device proof is not closed for this slice: `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` artifact `artifacts/android-device/20260726T082041Z-100.104.163.65_5555-78309` is `blocked` with reason `device_locked_or_dreaming`.
# 2026-07-26T10:01:19Z header worker rail closeout

- run_id: `20260726T095327Z-Macstudio.local-86270-59463-header-worker-rail-closeout`
- built isolated S target `/tmp/freehand-target-header-workers`, installed S binaries explicitly, and kickstarted `com.freehand.daemonS` without broad process kills.
- online verifier passed with artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260726T100119-26232`.
- verified Header Worker rail: duration/status/expand details, `worker_session_id` open action, and composer still usable while the rail is open.
- recorded the dispatch-wait model in design/function-map/test-design/skill docs: isolated Worker transcript context, rigid parent-visible child outcomes, composer stays usable while waiting, timer checks stay owner-owned.

# 2026-07-26T10:40:39Z Android true-device recheck

- reran `apps/freehand-android/scripts/verify-device-ui.sh 100.104.163.65:5555` with the existing debug APK path.
- result stayed blocked by `device_locked_or_dreaming`.
- artifact: `artifacts/android-device/20260726T104039Z-100.104.163.65_5555-79306`.

## 2026-07-26 Android remote-registry APK recheck
- Jason corrected prior blocker: Tailscale `15t-1` / `100.104.163.65:5555` was reachable and foreground, not locked.
- Root cause evidence: installed debug APK `versionCode=3` read only legacy `daemon-connection.json`, so it loaded `http://100.66.1.82:44042/?client=android-webview`; relay root returned HTTP 404 while canonical shell is at `/relay/daemon/studio-host/?client=android-webview`.
- Built current debug APK with remote-registry sidecar preference and installed through `verify-device-ui.sh`.
- True-device pass: `artifacts/android-device/20260726T110620Z-100.104.163.65_5555-46257`; `FreehandWebUiLayout` shows `layoutClient=android-webview`, `layoutShape=tall_phone`, relay CSS URLs, `webuiCssApplied=true`, `webuiJsReady=true`, `webuiShell=true`; installed package `versionCode=20260728`.

# 2026-07-26T14:20:00Z audit remediation Phase 1 implementation slice

- Active goal: execute `docs/goals/audit-remediation-phase1-3-plan.md` Phase 1-3; this slice only advanced Phase 1 and did not claim goal completion.
- Implemented Phase 1 task-owner concurrency pieces:
  - `TaskRuntime::boot_read_only` for query/projection paths without self-agent creation or lease/lifecycle reconcile writes.
  - `TaskStore::append_event_and_snapshot` serializes ledger append + snapshot atomic write + task index rewrite under task-ledger flock and reallocates event seq from disk ledger truth.
  - ExecutionFact `Failed` terminalizes `TaskStatus::Failed` and releases the Worker.
  - `TaskInterrupted` lease recovery records stale `execution_id` as fencing token, clears active execution truth, and late stale ExecutionFacts are rejected.
  - EventInbox v2 path reads per-task ledger rows above sequence watermark instead of full materialization for v2 cursors.
  - Worker heartbeat thread now flips the live cancel token on heartbeat/lifecycle failure; provider blocking reqwest clients have explicit 120s timeout.
- Local targeted evidence:
  - `CARGO_TARGET_DIR=/tmp/freehand-target-phase1 cargo test -p freehand-task -- --test-threads=1` passed: 65 passed, 1 ignored child helper.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-phase1 cargo test -p freehand-runtime worker_runner::tests:: -- --test-threads=1` passed: 24/24.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-phase1 cargo test -p freehand-runtime runtime_dispatches_worker_control_to_task_owner -- --test-threads=1` passed.
  - `CARGO_TARGET_DIR=/tmp/freehand-target-phase1 cargo test -p xtask --quiet`, `xtask gates check`, and `xtask mainlines check` passed.
- Full `CARGO_TARGET_DIR=/tmp/freehand-target-phase1 cargo test -p freehand-runtime -- --test-threads=1` is still red with the adjacent pre-existing live-bridge/autonomy failures, so Gap 7 was reduced to a verification-closeout gap rather than removed. Do not start Phase 2.1 worker pooling until this is green or owner-separated.
- Process note: an external stale Claude process was repeatedly spawning `cargo test -p freehand-task` and once stashed `crates/freehand-task/src/lib.rs`; stopped explicit PIDs only and restored/kept the task patch before continuing.

# 2026-07-26T15:20:00Z stale ToolPending lifecycle trace

- Jason reported two Home sessions still show waiting from old failures; this is startup cleanup/check scope, not manual data deletion.
- Live ADP `QuerySessionList` shows two persisted `latest_status=toolpending` with `active_turn_id=null`: `webui-path-diagnostic-state-sync-fixed` and `webui-session-20260723001509-bd98e156`.
- TaskBoard truth:
  - `webui-session-20260723001509-bd98e156` has only related `task-1784765749:closed`; no timer owner. Its latest `runtime-turn-541-r3` has no model request/tool activity and is a user-choice wait, so Home must not classify it running.
  - `webui-path-diagnostic-state-sync-fixed` has multiple related path diagnostic children in `blocked`; latest target `task-webui-path-diagnostic-1784732067073` has Worker `TaskBlocked` and Master `TaskProgressed` blocked_decision at seq 7.
- `QuerySessionTurns(webui-path-diagnostic-state-sync-fixed)` fails with `reason ledger sequence is invalid: expected 1, got 209`. Runtime snapshot evidence shows raw `runtime-turn-521` Blocked follow-up exists, but rollback marker moved effective head back to `runtime-turn-520-r3 ToolPending`; master loop state already contains `blocked|webui-path-diagnostic-state-sync-fixed|runtime-turn-520|task-webui-path-diagnostic-1784732067073:7`, so startup reconcile currently treats the invalidated follow-up as completed and leaves the old waiting turn effective.
- Initial root layers: reason selected UI restore must not hard-fail inactive authoritative transcript just because a retained reason ledger starts at an offset; WebUI active owner classification must not treat blocked child tasks as open lifecycle; runtime blocked-parent reconciliation needs to re-run if idempotency marker exists but no effective terminal blocked follow-up is visible after rollback.

# 2026-07-26T17:12:00Z stale lifecycle retained-ledger live closeout

- run_id: `20260726T164323Z-Macstudio.local-13608-4398eb`
- Jason reported two old sessions still showed waiting/running after restart; this was treated as startup lifecycle cleanup, not manual session deletion.
- Root live evidence before closeout:
  - `webui-path-diagnostic-state-sync-fixed` had effective `runtime-turn-520-r3 ToolPending`, raw rolled-back `runtime-turn-521 Blocked`, retained reason ledger starting at seq 209, and stale `completed_parent_evaluations` marker.
  - `webui-session-20260723001509-bd98e156` had `runtime-turn-541-r3 ToolPending`, no active turn, only closed child task truth, and was a user-choice wait.
- Additional fixes:
  - `ReasonPersistence::restore` and `restore_turn_start_snapshots` now use authoritative snapshot truth when a retained-offset reason ledger starts after seq 1, preserving explicit sequence-gap failure only when no authoritative truth exists.
  - Master parent objective recovery accepts repair-round turns that carry `freehand_runtime/original_task` context, so retained `runtime-turn-N-rM` ToolPending parents can still close lifecycle after restart.
  - WebUI `ToolPending` classification no longer keeps stale `等待生命周期` just because timer projection is still loading when there is no waiting tool/model/open-task evidence; Android Back from session-local refresh error clears selected session and opens the session drawer.
- Live proof after S restart:
  - `QuerySessionTurns(webui-path-diagnostic-state-sync-fixed)` returns successfully and includes fresh `runtime-turn-522 Blocked` instead of dispatch failure or stale wait.
  - `QuerySessionList` shows `webui-path-diagnostic-state-sync-fixed latest_status=blocked latest_turn_id=runtime-turn-522` and `webui-session-20260723001509-bd98e156 latest_status=toolpending active_turn_id=null`.
  - `node scripts/verify-webui-session-restore-error-exit-online.mjs` passed, artifact `artifacts/webui-online/webui-session-unlock-1785085716976`.
  - `node scripts/verify-webui-mobile-ui-tree-online.mjs` passed, artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260726T170901-2353`.

# 2026-07-26T17:18:00Z stale lifecycle final verifier artifacts

- Final WebUI proofs after tightening Home `ToolPending` classification:
  - `node scripts/verify-webui-session-restore-error-exit-online.mjs` passed, artifact `artifacts/webui-online/webui-session-unlock-1785086148685`.
  - `node scripts/verify-webui-mobile-ui-tree-online.mjs` passed, artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260726T171436-19295`; Home `正在运行` ids are `[]`, while `webui-session-20260723001509-bd98e156` is in history with `等待中` and `webui-path-diagnostic-state-sync-fixed` is history `已阻塞`.

# 2026-07-26T17:50:47Z stale ToolPending bootstrap cleanup closeout

- run_id: `20260726T175357Z-Macstudio.local-26089-019a92-stale-lifecycle-bootstrap-closeout`
- scope: old user-session `ToolPending` residuals must be cleaned by startup lifecycle reconciliation, not manual session deletion or UI hiding.
- implementation: `RuntimeCommandDispatcher::new` live bootstrap now runs `recover_stale_lifecycle_waits_on_bootstrap` before `restore_all_persisted_sessions_into_ui`. The reconcile path uses `TaskRuntime::boot_read_only`, TimerStore schedules, and live non-recoverable `master_work` truth to decide whether a latest effective `ToolPending` turn has an owner that can wake it. No-owner stale waits are re-recorded through `ReasonPersistence::record_turn_closed` as terminal `Blocked`; open child-task/timer/live-master waits are preserved. `TaskRuntime::boot_read_only` was added so projection/reconcile reads do not create self-agent or lease side effects.
- positive/negative tests: `live_bootstrap_closes_stale_toolpending_without_lifecycle_owner` proves stale wait becomes `Blocked` and SessionList `latest_status=blocked`; `live_bootstrap_keeps_toolpending_when_child_task_can_wake_parent` proves an open child task prevents premature cleanup.
- S-profile proof already observed before this docs/commit closeout: `webui-path-diagnostic-state-sync-fixed:2:blocked`, `webui-session-20260723001509-bd98e156:8:blocked`; persisted turn `~/.freehand/state/turns/master/webui-session-20260723001509-bd98e156/turns/runtime-turn-541-r3.json` has `status=Blocked`; mobile UI artifact `artifacts/webui-online/mobile-ui-tree-phase1-20260726T180200-56388` has `phoneRunningIds=[]` and places the old session in history as `已阻塞`.

## 2026-07-27 02:49 Phase 1 runtime package closeout
- Fixed bootstrap poison-ledger hard-fail in `recover_stale_lifecycle_waits_on_bootstrap` (skip JsonParseFailed/coherence/gap).
- UI bootstrap now uses `restore_turn_snapshots_for_ui` so multi-round intermediate turns backfill from ledger.
- Owner: incomplete authoritative + poisoned ledger falls back to partial authoritative with integrity warning.
- Proof: `cargo test -p freehand-runtime -- --test-threads=1` => 255 ok; freehand-task 65 ok; xtask 50 ok; mainlines/gates ok; clippy reason+runtime -D warnings ok.
- Removed architecture Gap 7 after package-level green. Gap 5/6/8 remain.
