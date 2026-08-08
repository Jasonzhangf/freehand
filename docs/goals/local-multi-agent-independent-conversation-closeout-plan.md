# Local Multi-Agent Independent Conversation Closeout Plan

## 目标

完成并提交 Freehand 本地多 Agent 独立对话闭环。Master 和三个 Worker 分别运行独立 WebUI、持久化命名空间和会话列表；Dashboard 能发现并进入每个 Agent；每个 Agent 只显示和操作自己的 session。本阶段不扩展跨 Agent 对话、任务派发或 Relay 协议。

## 验收标准

1. `4042`、`4043`、`4044`、`4046` 分别稳定提供 Master、Worker 1、Worker 2、Worker 3 的独立 WebUI。Worker 3 不得使用 Chromium 明确禁止的 `4045` 端口。
2. 四个 Agent 的 persistence namespace 隔离；新建、刷新、重启和继续对话都不会串 session、turn、provider/model identity 或运行状态。
3. Dashboard 初始没有 selected session；Agent 目录每个 Agent 只占一行，并显示稳定名称、角色和在线状态。
4. 点击三个 Worker 分别进入对应 WebUI；进入后只显示目标 Agent 的 session，不保留 Master 的 session 或运行内容。
5. `SessionList` 使用 persistence truth；terminal session 正确收口，不因旧活动投影长期显示运行中或等待中。
6. 没有 `local_web_url` 的旧 Worker 仍可只运行 Worker runner；显式 `--bind`、配置 URL 和 Relay 临时 loopback listener 按 typed config truth 判定。
7. URL 选择由 owner-backed topology truth 决定。远程访问不暴露 loopback URL；只有 loopback 浏览器访问明确标记为当前本机的 Agent 时才允许使用 `local_web_url`。
8. 四个 launchd 服务安装本次构建产物，精确重启后通过 health、HTML、asset、ADP、identity、provider/model 和移动 viewport 在线验证。
9. Codex review 明确 PASS 后提交唯一目标 slice；提交中没有其他 Worker 改动、临时文件或历史协作垃圾。

## 范围

In scope：Dashboard Agent directory、本地 Agent route、独立 WebUI host、persistence namespace、SessionList/SessionTurns、terminal projection、local/relay URL projection、本机 Agent typed marker、旧配置兼容、服务安装重启、浏览器在线证明、map/test/wiki/skill/memory 同步、review 和 commit。

Out of scope：跨 Agent 对话协议、A 控制 B 的新命令面、worker pool 和调度扩展、Relay 云部署、provider fallback、Android 真机闭环。不得清理无关工作树改动。

## 真源与 Owner

按顺序读取 `AGENTS.md`、`CACHE.md`、`MEMORY.md`、`note.md`、`.agents/skills/freehand-dev/SKILL.md`、`~/.codex/USER.md`、resource map、feature/function map、mainline call map、verification map、test design 和 module registry。

至少审查 `config.core`、`agent.lifecycle`、`node.master-slave`、`app.runtime-daemon`、`runtime.ui-command-dispatch`、`reason.persistence`、`ui.protocol`、`app.webui-smoke`、`relay.transport`。`design/pending` 条目不得作为已生效真源。

## 设计原则

1. Dashboard 只发现和选择 Agent；进入 Agent 后，session、turn 和状态全部受该 Agent namespace 约束。
2. 浏览器 hostname 不能证明 Agent 是否本机；本机性和 URL 必须由 config/node/relay owner 输出 typed truth。
3. `local_web_url` 缺失不等于配置错误；旧 Worker-only runner 必须继续运行。
4. 配置 URL 与显式 `--bind` 冲突时显式失败，不得静默选址。
5. 远程浏览器永不收到可导航的 loopback Agent URL。
6. Session 和 lifecycle 状态来自 owner；WebUI 不合成第二真源。
7. 控制面与业务 payload 物理隔离；禁止 fallback、silent cleanup 和浏览器猜测。
8. 生命周期、地址选择、terminal 收口和兼容行为均需正反测试。

## 必修 P1

### 旧配置兼容

- 无 `local_web_url`、无 `--bind`：运行 Worker-only runner，不强制启动 WebUI。
- 无 `local_web_url`、有 `--bind`：按显式地址启动 WebUI。
- 有 `local_web_url`、无 `--bind`：按配置地址启动 WebUI。
- 两者存在且一致：启动；不一致：显式失败。
- Relay Worker 无 local URL：保留 Relay 所需临时 loopback listener，不因缺字段失败。

### Agent URL 选择

- 投影增加明确的本机 Agent typed truth或等价能力字段。
- loopback 浏览器访问明确本机 Agent才可选择 `local_web_url`。
- 远程浏览器访问远程 Master 时，远端 Worker 使用 `relay_web_url`。
- 远程浏览器访问任意 Agent 都不得返回 loopback URL。
- 缺少安全可达 URL 时显式不可达，不做 fallback。
- count-only Worker 扩容没有端点输入；若模板已有 `local_web_url`，配置 owner 必须原子拒绝扩容，禁止复制端口或按名称猜端口。
- ADP transport 必须把本地/远程访问范围作为 typed side-channel 传给 ConfigStatus projector；远程结果物理省略 loopback URL。

## 实施顺序

1. 建立当前 run 和 semantic claim，检查 active runs、claims、events、evidence、merge queue 和 `KILL_SWITCH`。
2. 完成改前模块边界审查，确认唯一 owner、allowed/forbidden paths、相邻资源边和验证栈。
3. 为两个 P1 先补正反测试，再在唯一 owner 修实现。
4. 收口四个 Agent 的 persistence、SessionList、terminal projection 和 URL projection。
5. 同步 resource/function/mainline/verification/module registry、test design 和 generated wiki；运行 mainline generate。
6. 跑定向测试和 workspace baseline；失败必须修根因并从受影响阶段重跑。
7. 构建并安装 daemon，记录 SHA-512；精确重启四个 service。
8. 在线验证 `4042`、`4043`、`4044`、`4046` 和 Chrome `390x844` 真实点击。
9. 在 isolated staged slice 上审计 diff，完成 Codex review；任何 review 后修改都使旧验证和 PASS 失效。
10. Review PASS 后提交，更新 note、MEMORY、必要 skill 和当前 run evidence。

## 验证矩阵

定向验证：`cargo fmt --check`、相关 JS `node --check`、`freehand-config`、`freehand-ui-protocol`、`freehand-runtime`、`freehand-server --lib`、`freehand-daemon`、`freehand-relay` 测试，以及 WebUI foundation contracts。

全局验证：`cargo clippy --workspace --all-targets -- -D warnings`、`cargo build --workspace`、`cargo test --workspace`、`cargo run -p xtask -- mainlines generate`、`mainlines check`、`gates check`、`git diff --check`。无关既存失败必须给出可复现证据，不得伪称全绿。

在线验证：四端 `/health`、root HTML、assets、ADP handshake/query/subscribe/command failure、Agent identity、provider/model identity；重启前后 namespace 和 session 隔离。

浏览器验证：Dashboard 初始未选 session；四个 Agent 每行一个；依次点击三个 Worker 跳到 `4043/4044/4046`；目标 Agent 页面只显示自己的 session；无横向溢出、业务资源 404 或 bootstrap exception；本地和远程 URL 选择满足 typed truth。

## 安装、Review 与提交

安装 `target/debug/freehand-daemon` 到 `~/.local/bin/freehand-daemonS-bin` 并记录 SHA-512。只使用 service-scoped restart：`com.freehand.daemonS`、`com.freehand.workerS.worker`、`com.freehand.workerS.worker-2`、`com.freehand.workerS.worker-3`。

完成安装、重启和在线验证后运行 `codex -p cc review --uncommitted`；review 必须明确给出 `VERDICT: PASS` 或无歧义语义 PASS。若 review 后修改代码、测试、构建或运行配置，重新执行受影响验证、安装、重启、在线验证和 review。

PASS 后只 stage 本任务文件并提交：`git commit -m "feat(runtime): add local multi-agent session hosts"`。提交前检查 staged slice 无其他 Worker 文件和临时文件，`git diff --cached --check` 通过；提交后记录 commit SHA、binary SHA-512、测试、在线和 review 证据。

## 完成定义

只有源码、运行中的四个服务、浏览器真实行为、文档 map、review PASS 和 commit 全部指向同一版本时才算完成。最终报告必须包含变更、两个 P1 根因与修复、正反测试、workspace/gates、`4042/4043/4044/4046` 身份、浏览器点击、review、commit SHA、binary SHA-512，并明确列出未完成的跨 Agent 协议、Relay 服务器同步和 Android 真机证据。

## 2026-08-06 收口状态与剩余执行

已完成的实现和验证不得重做：两个 P1 已修复；config、UI protocol、runtime、server、daemon、relay 定向测试通过；WebUI foundation contracts、workspace build/clippy、mainlines 和 gates 已通过；最终源码对应 daemon 已安装并精确重启；Master、Worker 1、Worker 2 的在线 ADP 和 Chrome 跳转已通过。

当前唯一新增根因已由原版 Chrome 实证：访问 `http://127.0.0.1:4045/` 返回 `net::ERR_UNSAFE_PORT`。这是 Chromium 的端口硬禁用，不是 daemon、HTTP 或 ADP 故障，也不得通过测试浏览器参数放行来伪造普通用户可用性。剩余执行必须：

1. 将 Worker 3 的配置和运行地址从 `4045` 迁移到安全端口 `4046`，同步配置、服务运行真源、文档与在线验收；不得改无关协议或引入端口 fallback。
2. 从受影响阶段重跑验证，重新安装同一构建、记录 SHA-512，并只用四个精确 launchd service label 重启。
3. 对 `4042/4043/4044/4046` 完成 health、HTML、assets、authenticated ADP v3 handshake/query/subscribe/command failure、Agent identity、provider/model identity。
4. 用原版 Chrome `390x844` 从 Master Dashboard 逐个点击三个 Worker，证明分别进入 `4043/4044/4046`，且每个 Agent 只显示自己的 session，无横向溢出、业务资源 404 或 bootstrap exception。
5. 实跑 `cargo test --workspace`；任何失败必须给出可复现证据并判定是否属于本改动，不得引用旧口头结论。
6. 更新 `note.md`、`MEMORY.md`、必要 local skill 和当前 run evidence；重新 mine 并检索唯一短语验证记忆可查。
7. 完成全部在线验证后，按最新 `codex-review` skill 首先运行 `codex -p cc review --uncommitted`；只有明确 PASS 才能提交。review 后修改任何代码、测试、构建或运行配置时，旧 PASS 失效并重跑受影响闭环。
8. 仅提交本任务 staged slice，提交信息为 `feat(runtime): add local multi-agent session hosts`；不得包含 `.agent-collab` 历史垃圾、备份、输出或其他 Worker 改动。

## 2026-08-07 v2 已验证收口

- v2 唯一补丁已完成：ConfigStatus 真源写入后重新投影 SessionList，修复 SessionList-before-ConfigStatus 的 direct-session group 身份不收敛。
- 四个服务已安装同一构建并精确重启；在线与 Chrome artifact 为 `artifacts/webui-online/mobile-ui-tree-phase1-20260807T143011-8206`。
- 待完成仅剩本文件中 review、精确 stage、commit，以及明确未包含的跨 Agent 协议、Relay 服务器版本同步和 Android 真机证据。
