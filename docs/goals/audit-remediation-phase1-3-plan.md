# Audit Remediation Phase 1-3 Plan

2026-07-26 全仓审计后的修复计划。Phase 0（三 gate + 止血）已完成，本文件是 Phase 1-3 的执行真相源。

## 背景（已完成的前置，不要重做）

Phase 0 已落地：

1. 三个 gate 已进 `cargo run -p xtask -- gates check`：
   - `verify_dependency_graph`（xtask/src/main.rs）：`FORBIDDEN_DEPENDENCY_EDGES` 基线，其中 `baseline_violation: true` 的边是待清零债务：`node → ui-protocol`、`cli → testkit`、`runtime → provider-{openai,anthropic}`
   - ADP 读写分类：`UiCommandFrameClass`（crates/freehand-ui-protocol）编译期穷举 + 服务端 `accept_query_ingress` 白名单，`direct_task_mutation_forbidden` 已是协议错误码；`RunMasterPoll`（Command）与 `QueryMasterPoll`（只读，走 `preview_master_poll`）已拆分
   - `verify_task_status_single_writer`：`TASK_STATUS_WRITER_BASELINE = [create_task, assign_task, mutate_task, apply_event]`，目标收口到 mutate_task/apply_event
2. 审计发现已登记 `docs/architecture/architecture-gaps.md` Gap 5-8，每条带 closure path
3. WebUI 版本串已单点化（`assets.rs::WEBUI_ASSET_VERSION` + `__WEBUI_ASSET_VERSION__` 占位符）；SSE 已补 close；worker 主循环已加指数退避

## 完成定义（每个任务）

- 对应 gate 基线条目清零（翻转 `baseline_violation` 为 false / 从 `TASK_STATUS_WRITER_BASELINE` 删除条目）
- 从 architecture-gaps.md 删除对应 gap 段落
- 按 AGENTS.md 规则 6 同步 resource map / function map / test design / mainline / wiki

## Phase 1 — 并发正确性（必须先于 Phase 2.1）

对应 Gap 7（crates/freehand-task、crates/freehand-runtime）：

1. **ledger 写入跨进程事务**：`append_event_and_snapshot`（task/src/lib.rs 约 2654 行）的 ledger append + snapshot 原子写 + index 重写三步收进 flock 临界区（复用 `with_lease_state_lock` 的 `fs2::FileExt::lock_exclusive` 模式）；event seq 在锁内基于磁盘 truth 重新分配，消除基于内存快照 `last_event_seq+1` 的重复 event_id。
2. **只读 boot**：`TaskRuntime::boot` 内的三个 reconcile（含 `reconcile_running_leases` 写 TaskInterrupted）从查询路径剥离——提供 `boot_read_only`（UI query 路径用）与 `boot`（master/worker runner 用，保留 reconcile）。runtime lib.rs 里约 20 处 boot 调用点按用途分流。
3. **封双重执行窗口**：worker 心跳线程（worker_runner/heartbeat.rs）失败时通过 AtomicBool/channel 即时通知执行线程中断，不再等 turn 结束；provider executor 的 `reqwest::blocking::Client` 配显式超时；lease 过期产生的 Interrupted 事件携带 fencing token（execution_id 代际），旧 worker 迟到的 ExecutionFact 被 task center 拒绝。
4. **状态机收口**：`create_task`/`assign_task` 的内联状态赋值改走 `validate_transition`；处理 `TaskStatus::Failed` 死状态（实现生产者或删除并同步 UI 过滤器）；完成后从 `TASK_STATUS_WRITER_BASELINE` 删除对应条目。
5. **query_event_inbox 增量化**：现在每次全量加载所有 task ledger（O(历史总量)，master 1s 轮询一次），改为按 watermark 增量读取或缓存。

验证：`cargo test -p freehand-task -p freehand-runtime`、新增多进程并发写测试（两个进程同时 mutate 同一 task 无 seq 冲突）、gates check。

## Phase 2 — 多任务执行模型补齐（对照 Claude Code 参考模型）

参考模型：master 并行派发子任务、子任务独立 session、可追问、结束刚性通知、timer 活性检查、等待中接受用户输入。B（上下文隔离）已达标，勿破坏。

1. **worker 池化**（依赖 Phase 1 完成）：单 worker daemon 支持 N 并发执行槽（每槽独立线程 + 独立 `worker-task-{id}` session）；`resolve_dispatch`（task/src/lib.rs 约 2959 行）感知槽位余量；master 决策 prompt 注入并发容量。
2. **追问通道打通**：worker 在安全点（BetweenRounds/BeforeToolExecution）消费 `AskAtSafePoint`/`AddConstraint` 队列（现状：task/src/lib.rs 约 3328 行只入队，worker_runner.rs 的 `latest_task_state_worker_control` 只看 Pause/Resume/Cancel——队列有入无出）；对已 Closed 任务提供基于 worker session 的只读追问命令。
3. **事件处理与决策解耦**：master 循环（master_runner.rs `run_once`）拆两级——事件收取/admit 保持 1s 节奏，lifecycle 决策 turn 放执行池不阻塞收取；可选 notify crate 文件 watch 降低延迟下界。
4. **用户输入排队**：`register_master_active_work` 冲突时（master_runner.rs 约 407 行 "cannot register concurrent turn"）不再 DispatchFailed，改入 pending-input 队列，安全点或 turn 结束时消费。
5. **框架级活性兜底**：master runner 对每个 dispatched 任务自动维护 liveness timer（约 3 分钟无新事件注入检查 turn），不依赖模型自觉调 `timer(op="schedule")`；timer 检查移出 `pending_attention` 为空的前置条件（master_runner.rs 约 888 行），按份额调度避免饥饿。

验证：现有 `master_worker_autonomy` / `production_worker_runner_` 测试全绿 + 每项新增行为测试；在线验证走 `scripts/verify-master-worker-autonomy-online.sh` 模式。

## Phase 3 — 协议契约化与结构还债（可与 Phase 2 并行）

对应 Gap 5/6/8：

1. **ADP schema 单一来源**：ui-protocol 加 schemars 导出 JSON Schema（或 build 脚本生成 TS 定义），WebUI 从生成物导入命令构造器，替换 legacy-monolith.js 约 40 处手写 PascalCase 字符串；帧加 `protocol_version` + 首帧握手。
2. **协议 crate 瘦身**：`UiProtocolState`/投影引擎/对 blocks/control 的依赖移入新 `freehand-ui-projection` crate；ui-protocol 退化为纯类型+校验+错误码（仅依赖 serde+contracts）；`target_owner_module` 仓库路径降级 debug-only；`/adp` 加鉴权，从 UI 命令面移除 `ApplyExecutionFact`/`ClaimNextTask`/`RunSchedulerTick` 内部调度命令。
3. **runtime 拆解 + provider trait**：provider-core 增 executor trait，runtime 改依赖 trait 对象（翻转两条 provider 边基线）；`run_live_provider_reason_turn`（约 2000 行）按 provider 重试/schema 校验/attention 等待/工具循环四段拆分；错误可重试性从字符串 contains（worker_runner.rs 约 629 行 `"anthropic_http_status_5"`）改为错误类型携带结构化 `retryable`。
4. **node 解耦 ui-protocol**：node 内部状态类型迁入 contracts 或 node 自有投影（翻转 node 边基线）；cli 的 testkit 依赖迁独立 bin 或 feature-gate（翻转 cli 边基线）。
5. **WebUI session-detail 迁移**：transcript 渲染（`renderMessages`/`turnExecutionCard`）、composer 提交、worker rail 从 legacy-monolith.js 迁入 `surfaces/session-detail/`；context 收敛为启动时构造一次的显式 `SurfaceServices` 接口；edge-registry 二选一（effects 装运行时断言或移回文档）。
6. **工程债务**：verify 脚本抽 `scripts/lib/verify-harness.mjs`（`createCdpClient` 有 8 份逐字拷贝，11 个脚本各自持有 `assetVersion` 常量应改为从服务端读取或统一注入）；MEMORY.md（344KB）/note.md（733KB）按月归档 + gates 加大小上限断言。

## 通用约束

- 顺序：Phase 1 全部完成后才可做 2.1；其余可按任务并行
- 保护资产（勿破坏）：session 隔离与摘要回流（worker-task session + review truth）、durable ledger + watermark 不丢事件语义、reject-retry 续 session、attention 安全点机制
- 每任务验证基线：`cargo build --workspace`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo run -p xtask -- gates check`、`cargo run -p xtask -- mainlines check`
- 文档同步随任务走（AGENTS.md 规则 6），不预写未实施的计划
