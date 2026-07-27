# Architecture Gaps

Non-violation pending items. Not regressions. Not false positives. Each gap has explicit owner, known scope, documented risk, and no active gate violation.

## Gap 2: `metadata.core` — provider/debug producers 未全覆盖

| Field | Value |
|---|---|
| feature_id | `metadata.core` |
| owner crate | `crates/freehand-metadata` |
| gap kind | 控制/观测 provenance 覆盖面不全 — provider adapter 错误路径已通过 `provider.reason-live-bridge` 的 `RuntimeLive05ProviderError` 接入 metadata 中心化；请求构造/响应解析生命周期仍依赖 raw capture 而非 metadata |
| why not violation | metadata owner、隔离 gate、已接 producer 都成立 |
| risk | 请求构造/响应解析成功路径无 metadata 中心化记录（依赖 raw capture callback）；OpenAI adapter 无 executor，未来 executor 接入时需补充 |
| gate | 当前 gate 不会拦（xtask gates 只锁 metadata/request 类型隔离，不锁 producer 注册） |
| current producers | `freehand-reason`（turn lifecycle）、`freehand-runtime`（live bridge lifecycle + provider error）、`freehand-node`（node lifecycle） |
| missing coverage | 请求构造成功路径 metadata、响应解析成功路径 metadata、OpenAI executor provider error metadata |
| test design | `docs/testing/metadata.core.md` — known gaps 已记录 |
| function map | `docs/function-maps/metadata.core.md` — sync status 已记录 pending |
| priority | 低 — provider error 已覆盖，成功路径依赖 raw capture 已有可观测性，不阻塞任何 E2E 流程 |
| closure path | 1) 按需在 `RuntimeLive02ProviderRequestBuilt` 之前补充请求构造前验证 metadata（如 adapter 配置校验） 2) OpenAI executor 接入时复用 `RuntimeLive05ProviderError` 模式 3) 同步 function map + test design |

## Gap 3: `control.center` / `error.center` — 流程控制和错误处理未中心化

| Field | Value |
|---|---|
| feature_id | `control.center`, planned `error.center`, planned `task.orchestration` |
| owner crate | `crates/freehand-control`; current related owners are `metadata.core`, `reason.turn`, `runtime.ui-command-dispatch`, `node.master-slave` |
| gap kind | agent-framework control and error policy are still distributed; the basic `control.center` status parser and stopHook skeleton exist, but completion schema retry, provider/tool error decisions, task/subagent action tools, and runtime flow rhythm are not fully admitted through one metadata-watermarked control/error center |
| why not violation | existing landed behavior still has explicit owners and metadata/request isolation gates; basic status stopHook has owner docs and tests; task/error orchestration remains next refactor scope, not a claimed implemented feature |
| risk | runtime/provider/tool paths can make local retry/fail/block decisions without one auditable control/error policy; future task/subagent routing could accidentally execute side effects from status schema instead of compact built-in task tools |
| gate | no current gate requires control/error center admission before flow decisions |
| current producers | `control.center` writes basic hook/status metadata through runtime hook calls; `reason.turn` and runtime live bridge write existing lifecycle metadata; provider/tool/runtime errors can materialize shared `ErrorErr01RuntimeClassified` but not through a central error policy owner |
| missing coverage | full status repair loop, compact `task` action tool, task lifecycle persistence, task/subagent dispatch manager, error-center classifier, recovery decision owner, richer control/error watermark schema helpers, task state transitions linked to accepted action metadata |
| design doc | `docs/design/control-error-center-refactor.md` |
| priority | high — required before task/subagent orchestration refactor |
| closure path | 1) add feature-map/function-map/test-design entries for `error.center` and `task.orchestration` 2) extend `control.center` from basic stopHook to schema repair feedback and action admission 3) implement compact `task` built-in action tool 4) extend metadata center with status/action/error watermark helpers 5) route runtime/reason/provider/tool failures through error center 6) gate owner state transitions on accepted action metadata |

## Gap 4: `task.orchestration` / `provider.reason-live-bridge` / `app.runtime-daemon` — 生产 master/worker loop 未从 smoke 提升

| Field | Value |
|---|---|
| feature_id | `task.orchestration`, `agent.lifecycle`, `worker.control`, `provider.reason-live-bridge`, `app.cli-runtime-smoke`, `app.runtime-daemon` |
| owner crate | `crates/freehand-task` owns Task Center / Agent Lifecycle / worker-control truth; `crates/freehand-runtime` owns live provider loop and daemon dispatch; `apps/freehand-cli` owns current proof harness |
| gap kind | Phase 1/2A/2B/2C/2D foundation, production Master evaluation, Worker runner, config-compiled multi-Worker routing, isolated three-process controlled-provider closure, cross-process lease serialization, queryable Worker process health/restart owner truth, and launchd-managed three-Worker KeepAlive restart proof are implemented. Remaining gap is real-provider production recovery/takeover closure plus cross-machine transport. |
| why not violation | Current claims remain scoped: `paired_agents` compiles an ordered Worker set, Master task mutation accepts only set members, each Slave consumes exactly one Master, launchd names Worker services per agent, isolated online verifier proves three distinct Worker processes/agents/executions with reject/rework/next-round evaluation, `agent.lifecycle` projects Worker PID/process-instance heartbeat/offline/restart-count truth, and launchd verifier proves same-agent KeepAlive restart increments owner `restart_count`. Production promotion remains open because real-provider recovery, typed takeover, and cross-machine Worker transport are not closed. |
| risk | The controlled online topology is real process/runtime truth, process health is owner-queryable, and launchd-managed Worker crash restart is proven for three services; it still does not prove real providers preserve the same task/execution/agent identities through blocked/retry/reassignment/takeover recovery. |
| gate | Current gates lock owner maps, unique multi-peer config validation, legacy singular schema rejection, Worker-set assignment admission, role-specific tool surfaces, Worker claim/heartbeat/review-ready/blocked behavior, cross-process lease RMW serialization, Slave daemon mode, async blocking boundary, parent evaluation/rework semantics, agent-specific launchd naming, `agent.lifecycle` process heartbeat TTL projection, isolated online three-PID/three-agent/no-cross-claim/offline/restart proof, and launchd-managed KeepAlive recycling through AgentBoard restart-count proof. No gate yet proves real-provider crash/takeover. |
| current producers | `LoadedConfig::select_agent`, `ProductionMasterRunner`, `ProductionWorkerRunner`, `ProductionWorkerRunner::record_process_started`, `ProductionWorkerRunner::record_process_heartbeat_in`, `WorkerHeartbeat::start`, `run_master_mode`, `run_worker_mode`, `configured_worker_task_boundary_failure`, `TaskRuntime::apply_agent_lifecycle_event`, `TaskRuntime::claim_next_task`, Worker heartbeat renewal, Worker `ReviewReady`/`Interrupted`/`Blocked` execution facts, durable parent evaluation, TaskBoard/AgentBoard/EventInbox truth, agent-specific Worker launchd service files, WebUI current-session projection |
| missing coverage | pool allocation/release under concurrent tasks, real-provider online proof for review close/reject retry/crash recovery/blocked handling, typed takeover, formal current-source research/document E2E, browser-visible same-session WebUI proof, multi-BigTask context switching, and cross-machine Worker transport |
| design doc | `docs/design/framework-mediated-agent-operations.md`, `docs/goals/multi-task-foundation-phase2-gap-plan.md`, `docs/function-maps/runtime.master-worker-loop.md`, `docs/testing/runtime.master-worker-loop.md` |
| priority | high — compiled topology and isolated three-process evaluation closure are proven; next step is managed-service and real-provider lifecycle truth |
| closure path | 1) prove real-provider blocked/reject/retry/crash/reassignment/takeover with same ids 2) close formal current-source research and browser-visible same-session evidence 3) upgrade cross-machine Worker transport from the remaining singular node transport model |


## Gap 6: ADP 作为独立协议尚不成立 — 无版本化、无 schema 单一来源、Query 通道存在 mutation 旁路

| Field | Value |
|---|---|
| feature_id | `ui.protocol`, `runtime.ui-command-dispatch`, `app.webui-smoke` |
| owner crate | `crates/freehand-ui-protocol`, `apps/freehand-server` |
| gap kind | 2026-07-27 状态：① `UiAdpRequest`/`UiAdpResponse` 已强制顶层 `protocol_version=1`，`/adp` 已要求首帧 handshake 并返回 capability；剩余 gap 是 WebUI 仍以手写字符串镜像 serde 编码、无 JSON Schema/TS 生成物，`/adp` 无鉴权且暴露 `ApplyExecutionFact`/`ClaimNextTask` 等调度内部命令，ui-protocol 仍混装服务端状态机 `UiProtocolState` 与投影引擎，`target_owner_module` 仍把仓库内部路径序列化进线协议 |
| why not violation | Query 通道 mutation 旁路已收口：`UiCommandFrameClass` 穷举 + `accept_query_ingress` + 服务端 `handle_adp_query` 前置校验，`RunMasterPoll` 与只读 `QueryMasterPoll` 已拆分，且服务端 WebSocket 测试锁定 mutation 在 runtime query port 前被拒。ADP version/handshake 已由协议 serde roundtrip/negative test、服务端 pre-handshake rejection/duplicate-handshake test、CLI/WebUI/daemon handshakes 锁定。剩余 gap 是 schema 生成物、crate 瘦身、鉴权和命令面收缩。 |
| risk | 旧客户端会被显式 `protocol_version` 反序列化失败或 handshake failure 拒绝，必须同步升级；任意 WebSocket 客户端在完成 handshake 后仍可走 Command 帧驱动内部调度；协议 crate 仍无法被非 Rust 端做 schema 提取 |
| gate | Query 读写分类与服务端白名单已有协议/服务端测试锁定；版本化/首帧握手已有协议、服务端、CLI、daemon 和 JS 语法验证；schema 生成物、`/adp` 鉴权和命令面收缩尚无 gate |
| closure path | 1) ~~读写分类 + Query 通道白名单 + `direct_task_mutation_forbidden` 提升进 `UiProtocolError`~~ 已落地 2) ~~帧加 protocol_version 与首帧握手~~ 已落地 3) schemars 导出 schema，WebUI 从生成物导入 4) `UiProtocolState`/投影引擎迁出至独立 crate，`target_owner_module` 降级 debug-only 5) `/adp` 鉴权并收缩命令面 |
| priority | 高 — 多端独立发布节奏已经存在（Android APK / CLI 二进制 / 内嵌 WebUI） |


## Gap 8: WebUI monolith 迁移完成度约 10-15%，版本串手工同步 38 处

| Field | Value |
|---|---|
| feature_id | `app.webui-smoke` |
| owner crate | `apps/freehand-server` |
| gap kind | 2026-07-26 审计确认：`legacy-monolith.js` 仍占前端主体；session-detail surface 仅迁出薄壳，transcript/composer/worker-rail 仍在 monolith；surfaces 依赖 monolith 现场构造 context；bootstrap→monolith 仍靠 window 全局 + 动态 import 隐式时序；edge-registry 的 `allowedEffects`/`forbiddenEffects` 零运行时消费 |
| why not violation | 模块化重构方向与 `docs/goals/webui-render-architecture-closeout-plan.md` 一致；版本串已收口到 `apps/freehand-server/src/assets.rs::WEBUI_ASSET_VERSION` + `__WEBUI_ASSET_VERSION__` 服务端 stamp，SSE turn stream 已有 `closeSseTurnSubscription` + `pagehide` close。剩余是 session-detail 迁移与 edge-registry 运行时断言，不是版本串/SSE 泄漏事故。 |
| risk | session-detail 迁移期 thin shell 与 window 全局时序会继续把过渡态固化；edge-registry effects 无运行时消费时，surface edge 契约只能靠代码审查 |
| gate | 服务端 root/asset smoke 锁定 stamp 后的版本串与 no-store；SSE close 仍依赖 WebUI 代码路径，无独立自动化 gate |
| closure path | 1) ~~版本串服务端单点注入~~ 已落地；SSE close 已落地 2) 迁 session-detail 渲染主线，context 收敛为启动时构造一次的显式接口 3) edge-registry effects 装运行时断言或移回文档 |
| priority | 中 — 先拆雷（版本串/时序）再迁肉（session-detail） |

## 管理规则

1. 本文件只记录 **非违规欠账**。违规必须改或删。
2. 新增 gap 必须有：`feature_id`、owner、gap kind、risk、gate 状态、closure path。
3. gap 关闭后，从此文件删除，不可留"已关闭"占位符。
4. 本文件由 `docs/architecture/feature-map.md` 路由索引中 `architecture-gaps` 段引用。
