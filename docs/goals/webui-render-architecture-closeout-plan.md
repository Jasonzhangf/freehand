# WebUI Render Architecture Closeout Plan

## 目标

把 WebUI 的渲染路径收口成“协议投影 -> 渲染模型 -> 视图”三层，确保每个 turn、每个 tool、每个 model-wait 生命周期都按自己的状态独立渲染，不再依赖全局 timer 或扁平 state 直接驱动 DOM。

当前要解决的核心问题是：

1. 历史 turn 仍然会被全局 timer 误当作 live turn，导致静态内容继续闪动。
2. tool turn、model waiting、schema retry、tool result continuation 等状态被混在同一个全局状态里，容易出现错误合并、错误停转、错误失败投影。
3. 用户看到的是“卡死/静默失败/重复卡片”，不是“明确的等待、明确的失败、明确的继续执行”。

## 现状修正

这次收口必须先纠正一个关键事实：

1. WebUI 的主控制/状态路径已经是 ADP WebSocket `/adp`，不是 SSE。
2. HTTP query / SSE subscribe 只是兼容路径，不是主路。
3. 1 秒刷新不是 HTTP polling，它只是本地 elapsed-time 重绘；问题不是“有 timer”，而是“timer 驱动范围过大、状态没有按 turn/tool 隔离”。

因此本次不做“把主路切到 SSE”，也不把 SSE 当成下一步重点。优先级是 render projection 分层和生命周期隔离。

## 验收标准

完成必须同时满足以下条件，并且需要真实验证：

1. 历史 turn 在新 turn 开始后保持静态，不再出现假动画。
2. 只有当前 live turn 可以显示 dispatching / thinking / schema retry / tool waiting / tool result continuation 的计时状态。
3. 每个 turn card 都由独立的 `RenderTurn` 投影对象渲染，不直接读取可变全局 state。
4. 每个 tool row 都由独立的 `RenderRow` / `RenderToolActivity` 投影对象渲染，且按 `tool_call_id` 绑定，不按 DOM 顺序或原始文本绑定。
5. tool 语义渲染必须使用 `UiToolActivity.display`，UI 不得通过原始 tool name / args / result 自己猜类别。
6. 模型等待、schema retry、tool result continuation 这些状态必须只挂在当前 live turn 上。
7. 完成态、失败态、取消态的历史 turn 必须静态显示，且不能带 live timer。
8. ADP 仍然是 WebUI/Android/CLI 的统一控制与状态路径。
9. 真实浏览器截图必须证明：历史 turn 静态、当前 turn 动态、工具状态不串、终态不假闪。
10. 功能 map、test design、note、MEMORY 必须同步更新。

## 范围

### In Scope

- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/assets/webui.css` 如需少量结构支持
- `apps/freehand-server/src/lib.rs` 的 asset smoke / boundary tests
- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- `docs/design/webui-architecture-review.md` 如需修正结论
- `note.md`
- `MEMORY.md` 仅在真实验证后追加

### Out of Scope

- 改 ADP 协议 framing
- 把主控制路径切到 SSE
- React / Vue / 框架迁移
- 全面 UI 重设计、字体重设计、配色重设计
- Android 页面重做
- provider / reason / runtime 语义改造，除非验证证明后端 truth 本身错误

## 设计原则

1. 协议 truth 只来自 ADP / ui.protocol。
2. UI 只消费投影，不写 truth。
3. 渲染模型必须显式分层，不能在 view 层猜测语义。
4. 生命周期状态必须按 turn id / tool_call_id / phase key 隔离。
5. 历史态是静态事实，live 态才允许动画和计时。
6. 错误必须显式暴露，不允许静默失败或 fallback 成成功。
7. 任何未知/无法分类的投影都必须渲染成显式 neutral / unknown，而不是伪造 running/success。
8. 视图层只做 DOM，不做协议归一化、不做分类推断、不做状态机决策。

## 目标架构

### 渲染分层

WebUI 最终要拆成三层逻辑，即使物理上仍在单文件里也必须按 owner 边界分段：

1. Projection layer
2. Render model layer
3. View / DOM layer

### 推荐的渲染模型

```text
RenderConversation
  - selectedSessionId
  - turns: RenderTurn[]
  - pendingSubmit
  - adpFailure

RenderTurn
  - turnId
  - sessionId
  - orderKey
  - lifecycle
  - rows: RenderRow[]

RenderLifecycle
  - phase
  - isLive
  - label
  - elapsedMs

RenderRow
  - kind
  - title
  - body
  - status
  - identity
  - lifecycle
```

### 关键约束

- `conversationTurnsForRender()` 之上再加一层 `buildConversationRenderModel()`
- `turnExecutionCard()` 只吃 `RenderTurn`
- `executionRow()` 只吃 `RenderRow`
- `toolSummaryBody()` 只能消费已经投影好的 `display` 字段
- `renderMessages()` 只能负责“拿 render model -> 更新 DOM”
- timer 只能作用于 live turn 的生命周期字段，不能驱动全局重绘污染历史 turn

## 技术方案

### 1. 引入渲染投影构建器

在 `apps/freehand-server/assets/webui.js` 内先完成逻辑分层，再考虑是否拆文件：

- `buildConversationRenderModel()`
- `buildRenderTurn(turn, options)`
- `buildRenderRows(turn, lifecycle, options)`
- `buildToolActivityRenderRow(turn, item)`
- `buildModelRequestRenderRow(turn, lifecycle)`
- `buildTerminalRenderRow(turn, item)`
- `renderModelHasLiveLifecycle()`
- `turnIsCurrentLiveTurn(turn)`

这些函数负责：

- 合并 selected session transcript 和最新同 session turn
- 决定哪些 turn 是 live，哪些 turn 是静态
- 把 tool / model / terminal / error 分成不同 row
- 把 schema retry / tool waiting / thinking / continuation 分成明确 phase

### 2. 作用域隔离 timer

当前全局 timer 需要改成 turn/tool 作用域：

- live turn 的 model waiting 才更新 elapsed
- live turn 的 tool waiting 才更新 elapsed
- 历史 turn 不能继续计时
- tool timer 必须按 `turn_id + tool_call_id` 定位

如果某条历史状态无法可靠回放 elapsed，就宁可显示静态状态，也不要伪造计时。

### 3. 视图层只消费 render model

DOM 层要改成只吃 render model，不直接吃后端原始对象：

- `turnExecutionCard(renderTurn)`
- `executionRow(renderRow)`
- `pendingExecutionCard(renderPending)`
- `renderMessages()` 负责总装，不负责分类

### 4. 保持 ADP 主路不变

本次不把主路径改成 SSE。需要明确保留：

- WebUI 默认 ADP WebSocket `/adp`
- Android 继续复用 ADP 路径
- CLI/headless 继续通过 ADP 做无 UI 验证
- HTTP query / SSE 作为兼容和回放通道保留

### 5. 锁住工具语义投影

工具显示必须继续通过 `UiToolActivity.display` 统一投影：

- read/list tool: 只要显示读了什么
- file mutation tool: 以 diff / 改动语义显示
- plan tool: 以计划内容显示
- shell / generic tool: 显示语义目标、参数摘要、结果状态

UI 不得再从 raw text 自己猜类别。

## 文件清单

### 主要实现文件

- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/assets/webui.css`
- `apps/freehand-server/src/lib.rs`

### 约束同步文件

- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- `docs/design/webui-architecture-review.md`
- `note.md`
- `MEMORY.md`

### 可选后续拆分文件

如果单文件过大但测试仍可控，再做物理拆分：

- `apps/freehand-server/assets/webui/projection.js`
- `apps/freehand-server/assets/webui/view.js`
- `apps/freehand-server/assets/webui/controller.js`
- `apps/freehand-server/assets/webui/adp.js`

但这一步不是前置要求，先把逻辑边界锁住。

## 验证矩阵

### 静态验证

- `node --check apps/freehand-server/assets/webui.js`
- `cargo fmt --check`
- `cargo test -p freehand-server -- --nocapture`

### 架构门禁

- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`

### 安装与运行验证

- `scripts/install-global.sh`
- `scripts/install-launchd.sh restart`
- `curl -4fsS http://127.0.0.1:4041/health`
- `curl -4fsS http://127.0.0.1:4041/`
- `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp`

### 行为验证

- 成功样本：至少一轮 turn 正常完成
- 失败样本：至少一轮 tool failure 后继续下一轮，再终态成功或明确失败
- 多轮样本：至少 2 轮连续推理，验证历史 turn 静态、当前 turn 动态
- schema retry 样本：验证只在 terminal-candidate finish reason 下触发
- tool result continuation 样本：验证工具失败返回模型后继续下一轮，而不是把 tool failure 当 terminal failure

### 真实页面验证

- 使用真实浏览器或 CDP 在固定端口 `4041` 上验证
- 保存截图作为证据
- 重点截图：
  - 当前 turn 运行中，旧 turn 静态
  - tool waiting / model thinking 有计时但不串历史
  - tool error 显式返回并继续后续轮次
  - 终态无旧动画残留

## 风险与规避

1. 风险：只改视觉不改状态边界，问题会再次复发。
   - 规避：先改 projection，再改 view，最后才是样式。
2. 风险：timer 仍旧穿透到历史 turn。
   - 规避：timer key 必须包含 turn id 和 tool_call_id，且只对 live turn 生效。
3. 风险：UI 再次按 raw text 猜工具语义。
   - 规避：view 层只能消费 `display`。
4. 风险：验证只做静态测试，没做页面实证。
   - 规避：必须补真实浏览器截图和 ADP transcript。
5. 风险：混入 unrelated dirty changes。
   - 规避：只提交 WebUI 相关验证过的文件，其他工作树变更不碰。

## 实施步骤

### Step 1: 锁定当前渲染主线

确认当前 `webui.js` 的 render 入口、timer 入口、turn/tool 入口，定位哪些函数仍在直接读全局 state。

### Step 2: 建 render model

新增 render projection 层，先保持现有 UI 外观不变，只把数据流转到 render model。

### Step 3: 拆 live / static 逻辑

把 live 生命周期和历史静态生命周期分开，确保历史 turn 不再进入动画和计时路径。

### Step 4: 锁工具/模型状态

把 tool waiting、thinking、schema retry、tool result continuation 的状态全部变成 turn-scoped / tool-scoped。

### Step 5: 更新 smoke / docs

补齐函数 map、test design、review 说明、note。

### Step 6: 真实页面验证

固定端口安装启动，用 ADP + 真实浏览器验证多轮场景并保存截图。

### Step 7: 收口提交

验证通过后提交，只包含本任务相关文件。

## 完成定义

任务完成的定义只有一个：

1. WebUI 真实页面上的多轮 turn / tool / model waiting 生命周期是正确的。
2. 历史 turn 不再闪动，不再吃 live timer。
3. tool failure 会继续返回模型并进入下一轮，不会被错误终止。
4. schema retry 只在 terminal-candidate 触发，不会把 tool_use 或未完成 tool call 错当终止。
5. ADP、浏览器截图、测试、docs、memory 全部对齐。

