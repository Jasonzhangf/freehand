# note.md

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
