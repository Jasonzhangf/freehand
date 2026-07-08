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

## Gap 4: `task.orchestration` / `agent.lifecycle` — master/worker 执行闭环未接上

| Field | Value |
|---|---|
| feature_id | `task.orchestration`, `agent.lifecycle`, planned `worker_control` |
| owner crate | `crates/freehand-task` for Task Center and Agent Lifecycle skeleton; future runtime-control owner pending |
| gap kind | Phase 1 已有 TaskBoard、AgentBoard、ExecutionFact、SchedulerTick headless truth，但真实 master/worker task execution queue、worker claim loop、EventInbox cursor、worker_control safe-point inbox、master poll loop 尚未实现 |
| why not violation | Phase 1 交付范围只声明 headless foundation；restart same-id proof 已验证当前 truth surface。真实 worker queue/control/poll loop 是 Phase 2 scope |
| risk | 如果先做 UI dashboard，会变成静态投影或假状态；如果 agent 私下通信或直接改状态，会绕过 Task Center 和 Agent Lifecycle owner truth |
| gate | 当前 gate 锁 Phase 1 owner/map/mainline 和 tool surface；尚未要求 master/worker execution sample 或 worker_control inbox |
| current producers | `TaskRuntime::query_task_board`, `TaskRuntime::query_agent_board`, `TaskRuntime::apply_execution_fact`, `TaskRuntime::run_scheduler_tick`, CLI `phase1-foundation-sample` |
| missing coverage | worker task queue notification, automatic/explicit worker claim loop proof, master EventInbox cursor, reject -> retry -> approve -> close master/worker sample, real lifecycle event coverage from worker execution, `worker_control(op=...)` owner map and safe-point inbox |
| design doc | `docs/design/framework-mediated-agent-operations.md`, `docs/goals/multi-task-foundation-phase2-gap-plan.md` |
| priority | high — should happen before WebUI/Android task dashboard |
| closure path | 1) Phase 2A worker execution loop sample 2) Phase 2B master poll loop + EventInbox cursor 3) Phase 2C worker_control safe-point channel 4) Phase 2D UI projection |

## 管理规则

1. 本文件只记录 **非违规欠账**。违规必须改或删。
2. 新增 gap 必须有：`feature_id`、owner、gap kind、risk、gate 状态、closure path。
3. gap 关闭后，从此文件删除，不可留"已关闭"占位符。
4. 本文件由 `docs/architecture/feature-map.md` 路由索引中 `architecture-gaps` 段引用。
