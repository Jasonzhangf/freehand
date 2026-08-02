# Relay Agent Dashboard Mobile Closeout Plan

## 目标与验收标准

目标：按顺序完成手机端 Relay 登录、Agent Dashboard、当前 Agent 的 session/worker 视图、Android 真机验证、UI tree 文档锁定，并且每一项都必须完成后立即检查、验证、提交，不允许把多项混在一个大提交里。

验收标准：

1. 手机端入口链路闭合：`RelayLogin -> AgentDashboard -> AgentSessionDashboard -> SessionDetail`。
2. Relay 登录和 token 持久化使用 Relay/account owner truth，不在 WebUI 本地伪造登录状态。
3. 首页变为 Agent Dashboard：显示已登录账号可访问的在线/离线 Agent、角色、状态；点 Agent 后才进入该 Agent 的 session dashboard。
4. Agent 内 session dashboard 保持 Jason 锁定的移动端规则：主页只显示一行一个 session，按 `今天`、`过去一周`、`所有更早的` 分组；正在运行与历史列表互斥且不覆盖 selected session。
5. 进入任意 session 后，Home dashboard 离开主内容区，只显示该 session 的详情和当前 session 作用域内 worker/header 状态。
6. 多 worker header 使用 Claude Code TUI 风格：每个 worker 显示名称、实时状态、持续时间，点击后展开 owner-backed 子任务详情。
7. 子任务/worker 状态来自 ADP owner projections：`TaskBoard`、`AgentBoard`、`TaskHistory`、`EventInbox`、`WorkerControl`；禁止浏览器合成 lifecycle truth。
8. Android 真机证明覆盖 Dashboard、RelayLogin、AgentSessionDashboard、SessionDetail、多 worker header，以及竖屏自动排版无横向溢出。
9. 文档、resource/function/mainline/test map、local skill 同步；最终没有未提交目标代码。

## 范围与边界

In scope：

- WebUI/mobile UI route 和 surface 模块拆分。
- Relay 登录 UI、账号 token 保存、退出登录/错误状态退出口。
- Agent Dashboard 页面和 Agent session dashboard 页面。
- 当前 session 作用域内的 worker header、展开详情、返回父 session、切换 sibling worker。
- Android WebView 真机验证和截图证据。
- UI tree human doc、machine manifest、function map/mainline/test/gate 同步。
- 每个 step 的独立 commit。

Out of scope：

- 不重写 Relay backend 协议，除非 UI 接线发现 owner API 缺必要字段。
- 不做云部署策略扩展、TURN/WebRTC fallback、跨网络传输备用路径。
- 不做 provider 策略重构。
- 不物理删除生产 session/task truth；旧错误残留只通过 owner lifecycle cleanup/repair 处理。

## 设计原则

1. UI route first：先锁 route tree，再写控件；禁止继续在一个巨型文件里症状 patch。
2. Owner truth only：登录、Agent 状态、session 状态、worker 状态只读 owner projection，不做浏览器本地真源。
3. Phone primary body exclusive：手机竖屏主内容一次只属于一个 route，Home dashboard 和 SessionDetail 不可同时占主内容。
4. Error has exits：登录失败、ADP 失败、session refresh 失败、worker transcript 暂缺都必须有可见退出/重试路径，禁止 UI 死锁。
5. No fallback：缺字段、401、代理失败、worker 会话未投影时显式报错并修唯一 owner，不用静默降级。
6. Step closure：每项完成后立即跑定向验证、在线验证、截图/证据、commit；后续项不能依赖未提交脏改。

## 技术方案与文件清单

优先读取并同步：

- `docs/design/mobile-webui-ui-tree.md`
- `docs/design/mobile-webui-ui-tree.manifest.json`
- `docs/goals/mobile-webui-modular-rewrite-plan.md`
- `docs/goals/multi-agent-final-convergence-plan.md`
- `docs/function-maps/app.webui-smoke.md`
- `docs/function-maps/runtime.ui-command-dispatch.md`
- `docs/function-maps/ui.protocol.md`
- `docs/resource-maps/core.json`
- `.agents/skills/freehand-dev/SKILL.md`

主要代码面：

- `apps/freehand-server/assets/webui/`：模块化 route/surface/controller/view。
- `apps/freehand-server/assets/webui/legacy-monolith.js`：只允许作为迁移源；新增结构不继续膨胀巨型文件。
- `apps/freehand-server/src/assets.rs`：WebUI asset version bump。
- `crates/freehand-ui-protocol/`：仅在缺少 owner-backed DTO/query 时新增协议。
- `crates/freehand-runtime/`：仅做 thin dispatch bridge，不承载 UI/local truth。
- `apps/freehand-android/`：仅 Android WebView/bridge/verification 必要更新。
- `scripts/verify-*.mjs`、`apps/freehand-android/scripts/verify-device-ui.sh`：新增或更新真实验证脚本。

建议拆分模块：

- `app-shell/routes/relay-login/*`
- `app-shell/routes/agent-dashboard/*`
- `app-shell/routes/agent-session-dashboard/*`
- `app-shell/routes/session-detail/*`
- `app-shell/components/worker-header/*`
- `app-shell/components/session-row/*`
- `app-shell/state/relay-auth/*`
- `app-shell/adp/relay-client.js`

## 风险与规避

| 风险 | 规避 |
| --- | --- |
| 手机页面继续混 Home 和 SessionDetail | route manifest + DOM verifier 强制互斥 |
| worker 状态来自旧全局任务残留 | verifier 创建固定 parent session + scoped child task，只接受 parent_session_id/task id 命中 |
| Relay 登录 401 被 UI 吞掉 | negative test + DOM error card + retry/logout exit |
| Android 缓存旧 WebUI | bump asset version，验证 relay-served hash 与 workspace hash |
| true-device 不可达 | 明确标 blocked，不得声称 Android 闭环 |
| 多项改动混在一个提交难以回滚 | 每 step 结束强制 `git status`、targeted stage、独立 commit |

## 测试计划

每个 step 的最小通用验证：

- `node --check` 覆盖所有改动的 WebUI JS 文件。
- 相关 Rust package focused tests。
- `cargo fmt --check`。
- `cargo clippy --workspace --all-targets -- -D warnings`，除非本 step 明确只改纯前端且已有项目规则允许更小栈；否则跑全量。
- `cargo run -p xtask -- mainlines generate`。
- `cargo run -p xtask -- mainlines check`。
- `cargo run -p xtask -- gates check`。
- `git diff --check`。
- S-profile/relay 在线 smoke：health + ADP smoke + served asset hash/version。
- 浏览器/Playwright DOM proof。
- Android true-device proof：涉及手机可见行为时必须跑；设备不可用只可标 blocked。

专项验证：

1. RelayLogin/AgentDashboard：注册/登录成功、错误 key/password、token 持久化、logout、Agent online/offline、跨账号不可见。
2. AgentSessionDashboard：session 一行一条、三段时间分组、running/history disjoint、点击 session 后 Home dashboard 消失。
3. Worker Header：运行中/失败/阻塞/关闭状态、持续时间冻结/递增规则、展开详情、parent return、sibling switch、无 global fallback。
4. Android：启动、登录/已登录态、Dashboard、SessionDetail、Worker header 展开、竖屏无横向溢出、无 stale live/running 标签。
5. UI tree lock：human doc 和 manifest node/edge/return parity；verifier 失败时不得提交通过结论。

## 实施步骤

### Step 0：开工前真源定位

- 读 `AGENTS.md`、`CACHE.md`、`MEMORY.md`、`note.md`、`~/.codex/USER.md`。
- 搜 MemoryPalace。
- 查 resource map、function map、mainline call map、test design。
- 刷新 `.agent-collab` 视图；如需改同一语义面，claim 对应 `feature_id`/`resource_id`。
- 记录当前 dirty files，确认不覆盖他人改动。

完成检查：没有代码改动也要写入 `note.md` 当前 run 的目标、owner、验证计划。

### Step 1：RelayLogin + AgentDashboard

目标：实现手机端账号登录、token 持久化、Agent Dashboard 首页。

完成后必须检查：

- 登录失败不静默，UI 有错误卡片和退出/重试路径。
- 已登录 token 能恢复 Agent Dashboard。
- logout 后清理本地 token projection，回到 RelayLogin。
- Agent 列表只显示 owner-backed account scope，online/offline/role/status 正确。

完成后必须验证并提交：

- 定向 JS/Rust/relay/ui-protocol tests。
- Relay local online smoke。
- Browser DOM proof。
- `git diff --check`。
- commit 1：`feat(webui): add relay login agent dashboard`。

### Step 2：AgentSessionDashboard + session CRUD/route 互斥

目标：点 Agent 后进入该 Agent 的 session dashboard；session 一行一条，按 `今天`、`过去一周`、`所有更早的` 展示；点击 session 后只显示该 session。

完成后必须检查：

- Home/Agent dashboard 与 SessionDetail 手机主内容互斥。
- `正在运行` 只显示 owner truth 仍可唤醒的 lifecycle；没有运行中时不显示假 running。
- 历史 session 按时间排序；每个 session 只占一行。
- session CRUD 入口合理：rename 在 SessionDetail 内，不在列表行放“改”字；多选管理如批量归档/删除在 dashboard 管理模式。

完成后必须验证并提交：

- DOM verifier 覆盖三段时间、一行一 session、route 互斥、CRUD 管理模式。
- S-profile 在线 proof。
- Android 若可达则做真机截图；不可达只记录 blocker，不声称手机闭环。
- commit 2：`feat(webui): implement agent session dashboard`。

### Step 3：多 worker header 与生命周期闭合 UI

目标：实现 current-session scoped worker header，显示 worker 名称、状态、持续时间，点击展开 owner-backed details。

完成后必须检查：

- Header 只显示当前 selected session 的 child worker/task，不读全局旧残留。
- running duration 实时递增，terminal duration 冻结。
- 成功/失败/阻塞/等待/关闭都有明确状态；请求错误不能继续显示等待中。
- child task 点击进入 canonical `worker_session_id`，返回父 session 和 sibling switch 正常。
- master/slave lifecycle 意外退出后，重启恢复通过 owner truth 收敛，不依赖浏览器状态。

完成后必须验证并提交：

- 正反测试：success、failure、blocked、still-running、already-terminal、request-error。
- 浏览器在线 proof 创建固定 parent + 多 child tasks。
- 如涉及 Android 可见 header，跑真机证明或明确 blocker。
- commit 3：`feat(webui): add scoped worker lifecycle header`。

### Step 4：Android true-device 与 UI tree 文档锁定

目标：完成 Android APK/WebView 真机截图验证，并锁定 UI tree human + manifest + map + skill 标准。

完成后必须检查：

- APK 启动进入 RelayLogin 或 AgentDashboard，无卡死。
- Dashboard、AgentSessionDashboard、SessionDetail、Worker header 都有截图/DOM evidence。
- 竖屏按行高比自适应，无横向溢出。
- `docs/design/mobile-webui-ui-tree.md` 与 manifest 节点、边、return path 一致。
- `freehand-dev` skill 写入可复用标准，不写流水账。

完成后必须验证并提交：

- Android true-device verifier + screenshot artifact。
- UI tree parity verifier。
- 全量 baseline：build/fmt/clippy/tests/mainlines/gates/diff check。
- Codex review 按 AGENTS rule 36；PASS 后才能最终交付。
- commit 4：`docs(webui): lock relay agent mobile ui tree`。

## 完成定义

- 四个 step 均有独立 commit hash。
- 每个 commit 都有对应验证命令和 artifact 路径。
- 没有未解释的目标相关 dirty files。
- Android 真机证据存在；如果设备不可达，最终状态只能是 blocked，不能说完成。
- `note.md`、`MEMORY.md`、必要 local skill 均同步。
- Codex Review 最终 PASS；若 review 后有修改，重新验证和 review。
