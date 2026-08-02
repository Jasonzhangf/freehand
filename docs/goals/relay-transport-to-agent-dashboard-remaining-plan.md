# Relay Transport To Agent Dashboard Remaining Plan

## 目标与验收标准

目标：完成独立 Relay 传输模块、真实 Agent 主动连接、云端部署、产品接线、Agent Dashboard、Android 升级和真机闭环。严格按“模块自测 -> 云端真实验证 -> 接线 -> UI/Android 验证”执行。

验收标准：

1. `relay.transport` 是账号、Agent presence、远程 HTTP/ADP tunnel、Relay 部署的唯一 owner。
2. Agent 只通过出站连接主动接入云端 Relay；Relay 不依赖云端进程访问 Agent 上报的 `127.0.0.1` 或 LAN URL。
3. Relay 控制面与 HTTP/ADP 业务数据面使用独立 typed channel；账号、路由、role、presence、health、retry、error 不进入业务 payload 或 `metadata`。
4. Tailscale 直连和 Relay tunnel 是两个显式 transport mode；不做失败后静默切换、双路径补偿或隐藏 fallback。
5. 手机入口闭合：`RelayLogin -> AgentDashboard -> AgentSessionDashboard(agent_id) -> SessionDetail(session_id)`。
6. Dashboard 显示账号范围内 Agent 在线状态、Master/Worker 角色、工作状态；点击 Agent 后才加载该 Agent 的 session truth。
7. Session Dashboard 一行一个 session，按“今天 / 过去一周 / 所有更早的”分组；进入 session 后 Dashboard 不再占据主内容区。
8. 当前 session 的 worker header 显示持续时间、实时状态和可展开详情；生命周期 truth 仍由对应 Agent 的 ADP owner 提供。
9. 云 Relay、Agent daemon、WebUI、Android APK 运行版本与源码一致；真实公网/异网样本完成登录、Agent 在线、session 打开、对话、断线恢复和账号隔离验证。
10. 每阶段独立提交。最终 Codex review PASS。无目标相关未提交改动。

## 范围与边界

### In Scope

- 收口当前 `crates/freehand-relay`、`apps/freehand-relay-server`、部署文件、maps、gates、测试和 review findings。
- 参考 `~/code/zterm` 的 Relay 与 Tailscale 连接机制，建立 Freehand 自有 outbound Agent tunnel。
- Relay 账号登录、token hash、Agent 身份、presence lease、role/status projection、HTTP/ADP multiplexing。
- Relay 云端部署代码、service、配置 schema、安装/升级/回滚说明和真实部署验证。
- Agent config、daemon Relay client、心跳/重连/重启恢复接线。
- WebUI RelayLogin、Agent Dashboard、Agent session route、worker header。
- Android WebView 接线、签名 APK 升级、真机竖屏和异网验证。
- resource map、module registry、function map、mainline call map、test design、wiki、skill、memory、CI/build gate。

### Out Of Scope

- Relay 不拥有 Agent 内部 Session/Task/Lifecycle/Provider 真源。
- Relay 不解析或重建 ADP 业务语义。
- WebUI/Android 不保存 Agent session/task 生命周期真源。
- 不做自动 transport fallback。选中 transport 失败必须显式报错。
- 不把账号密码、token、provider key 写入仓库、manifest、业务 payload、日志或 debug artifact。

## 设计原则

1. **模块先行**：Relay module 完整验证和独立提交前，禁止正式 config/daemon/WebUI/Android 接线。
2. **出站 tunnel**：Agent 建立长期出站 TLS WebSocket/QUIC 连接；云 Relay 通过该 tunnel 转发，不读取 Agent 提供的任意 upstream URL。
3. **控制/数据隔离**：presence、role、lease、route、auth、error 使用 control resource；HTTP/ADP frame 使用 data resource。两者类型不可接。
4. **唯一 owner**：Relay account/presence/tunnel 语义只在 `freehand-relay`；进程、daemon、WebUI、Android 只做薄接线。
5. **显式错误**：缺 config、store、字段、clock、upstream/tunnel、auth、lease、route 均 fail-fast；禁止默认值和 success-wrapped error。
6. **持久化后发布**：账号/token/Agent 控制 truth 先持久化成功，再替换内存 truth。
7. **远端证据优先**：同机 loopback smoke 只算模块证据；云 Relay + 异网 Agent/手机才算产品闭环。

## 当前状态与已知缺口

当前 dirty slice 已包含独立 Relay crate/server、Argon2、token hash、presence、HTTP/ADP proxy、systemd/env、maps、xtask gate 和 smoke。第一轮 review 为 `VERDICT: FAIL`，指出默认配置、缺字段默认值、隐式空 store、HTTP rewrite 吞错、clock fallback、ADP 先升级后连接、call-map 假绑定、持久化前发布、Tokio worker 阻塞。

当前工作区已开始修复上述 findings：显式 config parser、`init-store`、versioned store、copy-on-write commit、blocking executor、严格 rewrite、clock error、ADP upstream-first、真实 caller/callee map、反向测试。该修复尚未完成全部 gates、部署/在线复验、第二轮 review 和提交，不能视为完成。

更关键缺口：现有 proxy 仍以 heartbeat 上报 `upstreamBaseUrl` 让 Relay 主动访问 Agent。该模型只在同机/LAN smoke 成立，不满足“Agent 主动连接云服务器”。必须在产品接线前改为 outbound tunnel，并同步资源关系和测试。

## 技术方案与文件面

### Canonical Docs

- `docs/resource-maps/core.json`
- `docs/architecture/feature-map.md`
- `docs/function-maps/relay.transport.md`
- `docs/mainline-calls/relay.transport.json`
- `docs/testing/relay.transport.md`
- `docs/wiki/relay.transport.md`
- `docs/goals/relay-agent-dashboard-mobile-closeout-plan.md`
- `.agents/skills/freehand-dev/SKILL.md`

### Relay Owner

- `crates/freehand-relay/src/config.rs`: 显式 Relay runtime/server 配置 parser。
- `crates/freehand-relay/src/model.rs`: account、Agent control、tunnel frame contract；控制和数据类型分离。
- `crates/freehand-relay/src/store.rs`: versioned durable account/token/control truth。
- `crates/freehand-relay/src/service.rs`: auth、directory、tunnel admission、HTTP/ADP multiplexing、显式 error chain。
- 建议拆分超大 service：`auth.rs`、`presence.rs`、`control_tunnel.rs`、`http_tunnel.rs`、`adp_tunnel.rs`、`error.rs`。拆分前先更新 module registry owned paths。

### Thin Hosts And Wiring

- `apps/freehand-relay-server/**`: serve/init/deploy 薄 host。
- `apps/freehand-daemon/**`: Agent tunnel client 生命周期和本地 daemon bridge；不拥有 Relay 账号/presence/tunnel 语义。
- `crates/freehand-config/**`: 仅 canonical Agent connection config；secret 与非 secret 配置隔离。
- `apps/freehand-server/assets/webui/**`: RelayLogin、AgentDashboard、AgentSessionDashboard、SessionDetail route/surface。
- `apps/freehand-android/**`: WebView/secure credential bridge/APK upgrade；不实现第二套 UI truth。
- `scripts/verify-relay-*.sh`、WebUI/Android verifiers：真实运行证据。

### Transport Contract

控制链与数据链必须独立编号并登记：

- `RelayControlIn01...`：Agent authenticated connect、presence heartbeat、role/status、lease、route admission、disconnect/error。
- `RelayDataIn01...` / `RelayDataOut01...`：Relay request envelope、Agent local HTTP/ADP bridge、opaque frame response。
- `RelayErrorIn01...` / `RelayErrorOut01...`：auth、route、lease、tunnel close、backpressure、timeout、process restart error。

具体节点在实现前写入 lifecycle manifest、resource map 和 mainline call map。只允许相邻转换。禁止从业务 frame 反推 presence/role/health。

## 风险与规避

| 风险 | 规避 |
| --- | --- |
| loopback smoke 掩盖 NAT/公网不可达 | 云 Relay 与异网 Agent 强制真实验证；禁止以本机 `upstreamBaseUrl` 宣称闭环 |
| control 字段混入 ADP/HTTP metadata | 独立 typed control/data contract + compile/gate 红测 |
| tunnel 断开后仍显示 online | connection ownership + lease + disconnect event；正反测试和重启测试 |
| Relay 成为 SSRF 入口 | 删除 client-selected arbitrary upstream；只桥接已认证 Agent tunnel 的本地固定 daemon endpoint |
| Argon2/磁盘阻塞 async runtime | store actor 或 `spawn_blocking` owner boundary；并发 health test |
| 云部署 secrets 泄漏 | secret manager/env file 权限；manifest/log/artifact scan gate |
| UI 再次耦合 Dashboard 与 SessionDetail | route manifest + DOM mutual-exclusion verifier |
| Android 使用旧 WebUI/APK | asset version、APK versionCode、签名和多端 SHA-256 一致性验证 |
| 并行 worker 覆盖 dirty worktree | `.agent-collab` semantic claim、精确 stage、逐阶段 commit |

## 测试与验证矩阵

### Relay Module

- 正向：explicit init/load、register/login、token restart、same-account directory、presence lease、HTTP round trip、ADP bidirectional round trip、Agent outbound tunnel、Relay restart/reconnect。
- 反向：missing config/store/field、corrupt/incomplete store、failed write、wrong password/token、cross-account、expired lease、unreachable/disconnected tunnel、invalid rewrite、clock failure、backpressure/timeout、already-closed tunnel。
- 并发：Argon2 和持久化期间 health/directory 不被 Tokio worker 阻塞。
- 架构：owner paths、control/data physical isolation、real caller/callee/import edges、old direct-upstream semantics 物理删除。

### Deployment And Live Network

- standalone binary `init-store`/`serve`、restart persistence、systemd/service-scoped restart。
- 云主机 TLS endpoint、账号注册/登录、Mac Agent 出站连接、lease/role/status、HTTP/ADP tunnel。
- 手机使用非同 LAN 网络访问 Relay，完成 Agent list、session query、消息 round trip。
- Relay、Agent、手机任一侧中断后：online truth 收敛、错误可退出、恢复后重新建立完整 lifecycle。
- Tailscale direct mode 与 Relay mode 分开验证；禁止一次失败后自动切换另一模式。

### Product UI And Android

- RelayLogin success/failure/logout/restore。
- AgentDashboard online/offline/role/status/account isolation。
- AgentSessionDashboard 三段时间、一行一 session、CRUD、route mutual exclusion。
- SessionDetail 对话、worker header duration/status/expand。
- Android signed upgrade、竖屏无横向溢出、异网 Relay 登录和对话、安装后版本/签名/hash 证据。

### Mandatory Gates

- 模块和受影响 package tests/build/clippy/fmt。
- workspace baseline：`cargo build --workspace`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`。
- `cargo run -p xtask -- mainlines generate`、`mainlines check`、`gates check`。
- deployment smoke、真实云端/异网 smoke、WebUI Playwright/DOM、Android true-device。
- 安装/重启/在线证据完成后，运行 `codex -p asxs review`；无最终结论再用 `codex -p tcm review`。修改后重跑受影响验证和 review。

## 实施步骤

### Phase 0：收口当前 Relay Review Findings

1. 刷新 `.agent-collab`、确认 claim/dirty files，不覆盖并行改动。
2. 完成第一轮 review 九项 findings 修复；补正反测试和 map/gate。
3. 运行 Relay/module/server/daemon 定向测试、clippy、fmt、mainlines/gates、deployment smoke、local online smoke。
4. 不以当前 local smoke 宣称云连接完成。
5. review PASS 后精确提交独立模块修复切片。

### Phase 1：建立 Outbound Agent Tunnel

1. 只读审查 `~/code/zterm` Relay/Tailscale owner、连接生命周期、账号、部署和测试。
2. 更新 resource map、module registry、function map、mainline/lifecycle manifest、test design，先锁 control/data/error chains。
3. 在 `freehand-relay` 实现 Agent outbound control/data tunnel；物理删除 `upstreamBaseUrl` 拉取代理死语义。
4. 完成独立 server + synthetic Agent module black-box，覆盖断线、重启、lease、backpressure、账号隔离。
5. 所有模块 gates 和 review PASS 后独立提交。未通过禁止产品接线。

### Phase 2：云端部署与真实远程自测

1. 用仓库部署代码部署 Relay server、TLS、非明文 secret 配置和 service。
2. 用独立测试 Agent 从非云主机网络主动连接。
3. 从另一网络客户端验证登录、directory、HTTP、ADP、断线/重启恢复。
4. 保存版本、endpoint、service PID、TLS、请求/响应、lease、restart evidence；不保存 secret。
5. 云端真实验证和 review PASS 后独立提交部署/SOP变更。

### Phase 3：Agent Config 与 Daemon 接线

1. Agent config 增加显式账号凭据引用、Relay server endpoint、transport mode、Agent identity；debug endpoint 仅作为显式 debug config。
2. daemon thin client 负责启动/停止 tunnel、周期 heartbeat、重连状态机、进程重启恢复；Relay semantics 仍由 `freehand-relay` owner 提供。
3. role/status 来自声明的 control truth；Session/Task/Lifecycle 仍由 Agent ADP owner 提供。
4. 安装、service-scoped restart、真实 Agent 在线/离线/恢复验证。PASS 后独立提交。

### Phase 4：WebUI Agent Dashboard 接线

执行 `docs/goals/relay-agent-dashboard-mobile-closeout-plan.md` 的 UI 树，顺序固定：

1. RelayLogin。
2. AgentDashboard。
3. AgentSessionDashboard：今天/过去一周/所有更早、一行一 session、CRUD。
4. SessionDetail route mutual exclusion。
5. Current-session worker header。

每项做 DOM/Playwright + 在线 Relay/ADP 验证并独立提交。禁止巨型文件和跨入口 UI 耦合。

### Phase 5：Android Upgrade 与真机闭环

1. Android 只承载 canonical WebUI 和必要 secure bridge。
2. 构建签名 release APK，递增 versionCode，验证 signer、version、built/dist/runtime/served hash 一致。
3. 已安装旧 app 原位升级，不清数据。
4. 真机异网验证 RelayLogin、AgentDashboard、SessionDetail 对话、worker header、竖屏排版和错误退出。
5. 完成全量 gates、最终 review、精确提交和 APK 交付记录。

### Phase 6：记忆、Skill、协作与最终交付

1. `note.md` 写探索和证据；确证后追加 `MEMORY.md`。
2. `.agents/skills/freehand-dev/SKILL.md` 固化 outbound tunnel、control/data 隔离、云端/异网验证、Android upgrade 标准。
3. `mempalace mine` 同一 wing，再搜新短语确认可检索。
4. `.agent-collab/evidence.jsonl` 写命令、结果、artifact、commit；claim 标 completed。
5. 最终汇报：根因、唯一 owner、正反证据、commit、APK/云 endpoint、剩余风险。

## 完成定义

- 当前 Relay review findings 全部关闭，独立模块 review PASS。
- direct `upstreamBaseUrl` 云端拉取模型已物理删除，Agent outbound tunnel 成为唯一 Relay 数据路径。
- control/data/error 资源和类型物理隔离，gate 接入 CI/build。
- 云 Relay 与异网 Agent/手机真实在线验证通过。
- config/daemon/WebUI/Android 接线完成且各自 owner 边界不变。
- Dashboard/UI tree 和 Android 真机验收通过。
- workspace baseline、mainlines、gates、deployment/live/browser/device 验证通过。
- 最终 Codex review PASS；review 后无未复验修改。
- 每 phase 独立 commit；无目标相关 dirty files；证据、memory、skill、MemoryPalace 完成。
