# note.md

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

## 2026-06-26 数据/控制 分离审计（只读）

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

### F. 待办（建议顺序，不在本轮 commit）

1. **`MetadataKind::DebugLink` 与 `Control` 是死变体**（hard rule 10 适用）— `crates/freehand-metadata/src/lib.rs:35` 声明了但生产代码 0 次使用。两种处理：
   - 删除变体（强制 schema 收紧）
   - 给变体定义明确 producer 计划（推荐：作为 `freehand-debug` 接入 metadata 中心化的 gap 关闭路径之一）
2. **provider/debug 接入 metadata 中心化**（Gap 2 closure path 第 1-3 步）— 已有 closure plan，优先级低
3. **`MetadataCenter` 查询接口单一**（`by_trace` 之外）— 没有 `by_owner` / `by_kind` / `by_node` 维度。当前审计只能 grep `MetadataKind::`，多 producer 写入的可观测性受限于 trace_id 单一维度
4. **MetadataCenter 是 `Mutex<MetadataCenter>` 形式持有** — 写入串行化。`freehand-runtime` 多处持有同一个 `Arc<Mutex<MetadataCenter>>`，并发 producer 写入需要锁
5. **`verify_data_control_boundaries` 静态扫描只覆盖 `freehand-contracts` 的 `ReasonReq*`** — 不扫描 `ReasonReq*` 之外的请求节点（如 `ReasonReq04ToolCall`/`ReasonReq05ToolResultReentry`/`ReasonResp01..03`/`ErrorErr01`），不扫 `freehand-ui-protocol` 里的 `UiCommand` 是否携带 metadata/debug 字段。已知受限范围（gate 文件注释里没写）
