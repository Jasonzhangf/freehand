# ADP Unified UI Closeout Plan

## Goal

完成 Freehand 收口剩余项：让 WebUI、Android、CLI/headless 统一通过 daemon 的 ADP WebSocket `/adp` 做状态查询、订阅和命令交互，HTTP/SSE 仅保留兼容路径，同时补齐自动化验证、固定端口启动、后台守护和文档真源同步。

## Acceptance

目标仅在以下条件全部满足时视为完成：

1. WebUI 默认通过 `ws://127.0.0.1:4041/adp` 进行 query / subscribe / command，不再把 HTTP/SSE 作为主控制面。
2. Android 客户端通过同一 ADP 真源完成状态查询、订阅和命令交互，不再维持一套独立的 HTTP/SSE 主路径。
3. CLI 或 headless 具备可复用的 ADP 自动化入口，可在无 UI 场景下完成最小闭环 smoke。
4. daemon 以固定端口后台启动，并能在本地重启后恢复服务与日志记录。
5. WebUI 能显式渲染等待、订阅已接受、命令结果和失败状态，不允许静默失败。
6. 所有变更同步更新 feature-map、function-map、test-design、mainline-call JSON、generated wiki、`CACHE.md`、`MEMORY.md`、`note.md`。
7. 相关定向测试、`mainlines check`、`gates check`、`make ci` 与真实 ADP 端到端验证全部通过。

## Scope

### In Scope

- `ui.protocol` 的 ADP frame 真源与 UI projection 语义
- `apps/freehand-server` 的 WebUI ADP 默认接入
- `apps/freehand-android` 的 ADP 接入与投影适配
- CLI/headless ADP smoke 入口
- daemon 固定端口后台启动、守护、日志与重启验证
- 文档、mainline-call、wiki、记忆文件同步

### Out of Scope

- reason / provider / node 核心语义重构
- UI 视觉重设计
- 新的 fallback / 双路径兼容逻辑
- 与 ADP 收口无关的功能扩展

## Design Principles

1. 单一真源优先：ADP frame、状态投影和错误投影只允许一个权威 owner。
2. UI 与推理分离：UI 只消费投影，不写入 reason/provider 语义。
3. 命令、查询、订阅必须分流，禁止混用或静默兜底。
4. 失败必须可见：等待、断连、错误和无效请求都要显式暴露。
5. 兼容路径只能保留为兼容，不得继续作为主路径。

## Existing Truth To Reuse

- `crates/freehand-ui-protocol/src/lib.rs`
- `docs/function-maps/ui.protocol.md`
- `docs/testing/ui.protocol.md`
- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/src/lib.rs`
- `apps/freehand-daemon/src/main.rs`
- `apps/freehand-android/app/src/main/java/com/freehand/android/data/`
- `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt`
- `apps/freehand-cli/src/main.rs`
- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- `docs/function-maps/app.runtime-daemon.md`
- `docs/testing/app.runtime-daemon.md`
- `docs/function-maps/app.android-client.md`
- `docs/testing/app.android-client.md`

## Technical Plan

### 1. Lock the ADP protocol truth

确认并收敛 `ui.protocol` 中的 ADP request / response / failure / subscription frame 语义，保证 WebUI、Android、CLI 复用同一 frame contract。

Likely files:

- `crates/freehand-ui-protocol/src/lib.rs`
- `docs/function-maps/ui.protocol.md`
- `docs/testing/ui.protocol.md`
- `docs/mainline-calls/ui.protocol.json`

### 2. Move WebUI default transport to ADP

让 WebUI 默认走 daemon `/adp`，并在同一个界面内渲染：

- subscription accepted / waiting
- subscription event
- command receipt
- query result
- explicit failure

兼容 HTTP/SSE 仍可保留，但不再是主控制路径。

Likely files:

- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/src/lib.rs`
- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- `docs/mainline-calls/app.webui-smoke.json`

### 3. Move Android to the same ADP truth

让 Android 侧从 HTTP/SSE 主路径切换到 ADP 统一控制面，并把状态、等待、失败投影保持在同一套 protocol truth 上。

Likely files:

- `apps/freehand-android/app/src/main/java/com/freehand/android/data/ProtocolClient.kt`
- `apps/freehand-android/app/src/main/java/com/freehand/android/data/SseEventStream.kt`
- `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt`
- `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt`
- `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt`
- `docs/function-maps/app.android-client.md`
- `docs/testing/app.android-client.md`
- `docs/mainline-calls/app.android-client.json`

### 4. Add a CLI/headless ADP smoke path

补一个无 UI 的自动化入口，能直接连 daemon `/adp` 做最小闭环验证，用于回归和故障定位。

Likely files:

- `apps/freehand-cli/src/main.rs`
- `scripts/` 下的 smoke 脚本或辅助入口
- `docs/function-maps/<new or updated feature>.md`
- `docs/testing/<new or updated feature>.md`

### 5. Close the release and daemon workflow

确保全局安装、launchd 后台启动、固定端口、日志、自动恢复和第一次权限申请流程在本地真实机器上可验证。

Likely files:

- `scripts/install-global.sh`
- `scripts/install-launchd.sh`
- `apps/freehand-daemon/src/main.rs`
- `docs/function-maps/app.runtime-daemon.md`
- `docs/testing/app.runtime-daemon.md`

### 6. Sync docs and gates

把所有受影响 feature 的 function-map、test-design、mainline-call JSON、generated wiki 和 feature-map 同步收口，并刷新 gate。

Likely files:

- `docs/architecture/feature-map.md`
- `docs/mainline-calls/*.json`
- `docs/wiki/*.md`
- `CACHE.md`
- `MEMORY.md`
- `note.md`

## Risks And Avoidance

### Risk: WebUI 或 Android 继续保留 HTTP/SSE 作为默认主路径

Avoidance:

- 把 ADP 设成默认入口
- HTTP/SSE 只做兼容面
- 文档与测试必须直接反映默认路径

### Risk: 状态层看似可用，但等待 / 失败 / 订阅已接受等状态不显式

Avoidance:

- 让 UI 明确渲染协议层状态
- 增加正反向测试，锁住静默失败

### Risk: CLI/headless 只有 UI，没有无界面自动化闭环

Avoidance:

- 提供独立的 ADP smoke 入口
- 让它可直接对 daemon 做 query / subscribe / command 验证

### Risk: 文档真源与代码实现脱节

Avoidance:

- 每次行为变化同步更新 function-map、test-design、mainline-call JSON、generated wiki
- 完成后跑 `mainlines check` 和 `gates check`

## Verification Matrix

### White-box

- `cargo test -p freehand-ui-protocol`
- `cargo test -p freehand-server`
- `cargo test -p freehand-daemon`
- `cd apps/freehand-android && ./gradlew testDebugUnitTest`

### Module black-box

- WebUI ADP query / subscribe / command smoke
- Android ADP projection smoke
- CLI/headless ADP smoke
- daemon fixed-port startup and restart smoke

### Project black-box

- 真实 daemon 上的 `/adp` 端到端对话与状态查询
- WebUI 与 Android 在同一控制面下对同一状态真源作出一致响应
- 无 UI 场景下仍可完成自动化回归

### Workspace gates

- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- `make ci`

## Implementation Steps

1. 确认 `ui.protocol` 的 ADP frame 真源与错误投影不需要再改。
2. 先把 WebUI 默认路径切到 ADP，并补齐等待 / 失败 / 命令结果渲染。
3. 再把 Android 切到同一 ADP 真源。
4. 增加 CLI/headless ADP smoke 入口。
5. 用真实 daemon、固定端口和后台启动流程做端到端验证。
6. 同步修正文档、mainline、wiki、记忆文件。
7. 跑完 workspace gates 后再收口提交。

## Definition of Done

完成时必须满足：

- WebUI、Android、CLI/headless 都能通过 daemon `/adp` 做统一控制与状态查询
- UI 对等待、订阅、结果和错误的投影是显式的，不会静默失败
- 固定端口的后台 daemon 可启动、可重启、可记录日志
- 相关文档、mainline-call 和 wiki 全部同步
- 所有定向测试、`mainlines check`、`gates check`、`make ci` 与真实 ADP 验证通过
