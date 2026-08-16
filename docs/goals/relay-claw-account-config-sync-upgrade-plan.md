# Freehand Claw Relay、账号配置同步与双路径升级闭环

Status: closed on 2026-08-16

Final evidence:

- Main and `origin/main`: `6100a559e090ed1f1998d19661da937f17539955`
- Android: `0.2.14`, `versionCode=20260819`, APK SHA-256
  `76fd8dd3d36e626fccaea074fe33100a53c1667e56f5d847029f3626e5e222fb`
- Tailscale and Relay manifests: same version, size `2758213`, signer, and APK
  bytes
- Claw Relay: systemd active, `/relay/health=ok`, deployed binary SHA-256
  `e299cb48d160dec3c14d1ad3ec5122d1293b2d6e2125a121c256d582732ca4eb`
- Real Android Relay-only in-place upgrade: passed
- Workspace pre-push stack, mainlines, gates, Relay deployment/online/config
  smokes: passed
- DSH reviews: isolated commit `13eb006` PASS; main commit `6100a55` PASS

## 目标与验收标准

目标：参考 `~/code/zterm`，收口 Freehand 现有 Relay 能力，把同一 Relay/配置服务可重复部署到 Claw，并实现：账号登录后同账号共享非 secret 配置；Android/客户端通过 Tailscale 直连与 Relay 公网两条显式路径下载同一个已签名升级产物；所有升级、部署、配置边界都能被重复验证。

验收标准：
1. Claw 上 `freehand-relay-server` 可重复安装、重启、升级，运行版本与源码构建版本一致；Tailscale 与公网路径都可达，账号 `jason` 的认证、账号隔离、Agent presence/tunnel 保持现有行为。
2. 同账号登录后，WebUI/Android/daemon 通过配置服务器获取并应用同一份版本化账号配置；跨账号完全隔离；secret 永远不上传。
3. 一次 release 只产生一个已签名 APK 和一份 update manifest；同一字节产物通过 daemon/Tailscale 的 `/android/update.json` 与 Relay 的 `/relay/updates/latest.json` 同时发布，version、signer、SHA-256、size 完全一致。
4. Android 原位升级能走 Tailscale 直连路径，也能走 Relay 公网路径；两条路径都是显式 endpoint，失败显式报错，不做静默切换。
5. `relay.transport`、`config.core`/新账号配置同步 feature、`foundation.workspace`、`app.android-client`、`node.master-slave` 的 owner/资源关系在 resource map、function map、mainline call map、test design、verification map 全部落定；`xtask gates check` 与 workspace baseline 全绿。
6. 最终按 AGENTS rule 36 取得明确 Codex Review PASS 后才交付。

## 当前基线与已完成能力

不得把本任务写成“从零建立 Relay”。以下能力已存在并在线验证过：

- `relay.transport` 独立 owner：`crates/freehand-relay` + 薄 host `apps/freehand-relay-server`；账号、token hash、Argon2、presence lease、account-scoped Agent directory、control/data/error typed tunnel、HTTP/ADP/generic WebSocket 代理、generation fence。
- Relay server v2 已部署在 Claw `159.75.134.56`，Tailscale `100.124.49.106:19091`；账号 `jason` 已注册；本地 4 个 Agent（4042/4043/4044/4046）已在线。
- `foundation.workspace` 已有 release/install/deploy 流程；当前工作区存在其他 worker 正在改的 Relay updates 路由、`claw-deploy.sh`、`verify-dual-path-update.sh`、Android hash/size 校验等未提交改动，先刷新 `.agent-collab` 并按 owner 收口，不得覆盖或重做。

## 范围与边界

### In Scope

- 收口并验证现有 Claw Relay 部署/升级脚本与 Relay updates 静态服务；把运行版本、systemd、env、store、updates 目录做成可重复发布链。
- 新建账号配置同步能力：版本化 account config document、revision/ETag、账号隔离、并发冲突、设备同步状态、安全 schema/校验/应用。
- `config.core`/新 feature 与 `relay.transport` 的配置服务器路由边界：配置内容真源归配置 feature，Relay 只提供账号认证与账号作用域 HTTP 路由边界。
- 一次 release 产出唯一已签名 APK/manifest，Tailscale 与 Relay 双路径发布同一字节；`scripts/verify-dual-path-update.sh` 作为门禁。
- Android 通过 daemon WebUI Settings 的既有 updater bridge 支持双 manifest URL：Tailscale daemon 路径与 Relay 公网路径；真机原位升级证据。
- resource map、module registry、feature map、function map、mainline call map、test design、verification map、wiki、skill、memory 同步；每阶段独立 commit。

### Out of Scope

- 不改已锁定的 Relay control/data/error tunnel 协议、账号目录协议与 presence lease 语义。
- 不做 transport fallback：Tailscale 与 Relay 是显式 endpoint candidate，失败必须显式报错。
- 不做 TURN/WebRTC signaling；Freehand 的 Relay 是 HTTP/ADP 代理 + account directory，不用 zterm 的 WebRTC relay-only 候选。
- 不把 provider API key、pair token、Relay token、ADP token、密码、环境变量值、本地路径、host-specific `local_web_url` 上传到配置服务器。
- 不做 daemon 二进制自更新；daemon 升级继续走既有 release/install 流程，本计划只要求 Relay server 在 Claw 可重复安装/重启/版本一致。
- 不实现 Android 本地第二套配置/账户 UI；Android 继续作为 canonical WebUI 的 WebView host，通过 WebUI Settings 与 `AndroidApkUpdateBridge` 消费 owner truth。

## 设计原则

1. 唯一 owner：`relay.transport` 只管账号/presence/tunnel/Claw host；配置 schema/校验/版本化文档/应用归 `config.core` 与新增 `config.account-config-sync`；release/发布门禁归 `foundation.workspace`；Android 安装交接归 `app.android-client`；远端 daemon 路由投影归 `node.master-slave`。
2. 配置共享不等于整机配置上传：只能同步 schema 允许的非 secret 共享字段；设备本地 secret/env/keychain 引用保留在本机。
3. 配置同步使用版本化文档 + revision/ETag；并发写显式 409 Conflict，禁止 last-write-wins，禁止静默本地优先覆盖服务器真源。
4. 升级产物只有一个真源：同一签名 APK、同一 manifest 字段、同一 SHA-256；Relay 只托管 `foundation.workspace` 发布好的字节，不重新构建、不修改 manifest。
5. 控制面与业务 payload 物理隔离：账号配置同步走 typed config/control resource；不得塞进 ADP/HTTP 业务 payload、`metadata` 或 Relay tunnel 数据帧。
6. 持久化成功后再发布：配置 revision、账号目录、upgrade manifest 状态都必须先 durable write/fsync 成功，再对外可见。
7. 无 fallback：缺配置、未配置 updates 目录、manifest/APK hash 不一致、并发冲突、账号隔离失败都显式失败。

## Owner 与资源边界

实现前先以 `docs/resource-maps/core.json`、`docs/architecture/feature-map.md`、module registry、function map、mainline call map、verification map 为准冻结以下边界；status 一律先标 `design`，落地并接 gate 后才可标 `active`。

| 语义 | 唯一 owner | 允许路径 | 禁止路径 |
| --- | --- | --- | --- |
| Relay 账号、presence、tunnel、Claw host、updates 静态服务 | `relay.transport` | `crates/freehand-relay/**`、`apps/freehand-relay-server/**`、relay maps/tests/verifiers | 配置文档真源、provider 配置内容、Android 安装逻辑 |
| Freehand 配置 schema、校验、本地 canonical config、配置应用 | `config.core` | `crates/freehand-config/**`、config maps/tests | 从 Relay 业务 payload 推断账号/路由/升级状态 |
| 账号配置同步（服务器端 revision/ETag、账号隔离、冲突、设备同步状态） | 新 feature（工作名 `config.account-config-sync`） | 新 crate（工作名 `crates/freehand-account-config`）及其 test/map/docs | 把文档塞进 `freehand-relay` store、WebUI local state、ADP payload |
| release 构建、确定性产物、版本、manifest、signer/hash、双路径发布门禁 | `foundation.workspace` | `scripts/release.sh`、`scripts/verify-dual-path-update.sh`、`docs/function-maps/foundation.workspace.md` 等 | 在 Relay/daemon/Android 里二次构建或改写产物 |
| Android manifest 检查、APK 下载、hash/size 校验、系统安装器交接 | `app.android-client` | `apps/freehand-android/**`、Android test/verifier | native 更新面板、静默安装、第二套 UI/config truth |
| 账号目录与远端 daemon 路由投影 | `node.master-slave` | `crates/freehand-node/**` 相关 map/source | 拥有账号配置内容、provider/model 配置真源 |

新增资源/feature 的精确 `feature_id`、`resource_id`、truth store、operations、projections、allowed/forbidden relations、source edges 必须先由执行者按 map 规范落盘并通过 gate，再写实现代码。禁止先改代码后补 map。

## 技术方案

### A. Claw Relay 可重复部署/升级（收口）

现状：`apps/freehand-relay-server/deploy/claw-deploy.sh`、`freehand-relay.service`、`relay.env.example` 已存在；Claw 上已有 v2 服务。需要完成：

- deploy 脚本必须从一次 release 产物（`dist/relay/updates/`）安装，并校验二进制、manifest、APK 与本地 staging 的 SHA-256。
- systemd service 只允许 `freehand-relay-server serve` 和显式 env；`FREEHAND_RELAY_BIND`、`FREEHAND_RELAY_STORE`、`FREEHAND_RELAY_PRESENCE_LEASE_SECONDS`、`FREEHAND_RELAY_SECURE_COOKIE`、`FREEHAND_RELAY_UPDATES_DIR` 全部显式；缺失即 fail-fast。
- 每次部署记录远端 binary SHA-256、systemd unit hash、env 指纹、store 存在性、`/relay/health`、`/relay/updates/latest.json`；不保存 secret。
- Claw 上只运行 Relay/配置服务进程，不运行 daemon/WebUI/Android 逻辑。

### B. 配置服务器与账号配置共享

推荐部署形态（执行者在 map 冻结时确认唯一形态，不要并行实现两套）：

- 配置服务器 = Claw 上 `freehand-relay-server` 进程内的账号作用域命名空间 `/relay/api/config`；`relay.transport` 只负责 Relay 账号认证与账号隔离边界，配置文档的 schema/校验/持久化/冲突由 `config.account-config-sync` 新 crate 拥有。
- 若 module registry 判断 host 边界必须独立，则拆 `apps/freehand-config-server`，但必须一次只落一个 owner 形态，禁止同语义双实现。

账号配置 document（`config-v1`，服务器真源）：

```json
{
  "schemaVersion": 1,
  "revision": 42,
  "etag": "sha256-of-normalized-document",
  "updatedAt": "2026-08-12T00:00:00Z",
  "document": {
    "providerRegistry": [],
    "modelGroups": [],
    "relayEndpointCandidates": [],
    "remoteDaemonRegistry": []
  }
}
```

字段边界：

- 允许同步：provider 定义（不含 auth 值，只含 `auth.type`、`auth.source`、env 变量名）、model group 定义与账号级选择、relay endpoint candidate（URL + token env 名，不含 token 值）、remote daemon registry（不含 one-time credential）。
- 禁止同步：API key、pair token、Relay/ADP token、密码、环境变量值、本地绝对路径、host-specific `local_web_url`、设备状态。
- 禁止把整个 `~/.freehand/config.toml` 上传。

API 契约（先落 typed contract，再实现）：

- `GET /relay/api/config`：返回账号最新 document；缺账号/未认证/跨账号返回显式错误。
- `PUT /relay/api/config`：请求携带 `If-Match: <etag>`；成功返回新 revision/etag；并发冲突返回 409 与服务器当前 document，客户端显式展示冲突，禁止静默覆盖。
- 设备同步状态：客户端本地持久化 last-seen revision，Settings 显示已同步/可更新/冲突/失败；不使用 WebUI local state 作为真源。
- 首次迁移：通过显式“上传/导入”入口把现有本地非 secret 配置写入服务器；禁止登录后自动静默上传整机配置。

客户端应用：

- `config.core` 编译账号共享配置 + 设备本地 secret 引用，输出统一 effective config projection；共享字段以服务器 revision 为准，本地冲突显式暴露。
- 账号绑定 Agent 启动/登录后必须拉取并校验配置；拉取失败且无有效本地缓存时显式失败，不用旧配置伪装成功。
- WebUI 通过 ADP `ConfigStatus` 类 owner projection 显示账号配置状态；Android 通过 canonical WebUI Settings 消费，不新增 native 配置客户端。

### C. Tailscale + Relay 双路径升级

现状：工作区已有未提交改动：Relay `/relay/updates/latest.json`/`<file>` 静态服务、`FREEHAND_RELAY_UPDATES_DIR` env、`claw-deploy.sh` updates 上传、`scripts/release.sh` 双路径 staging、Android `sha256`/`size` 校验、`scripts/verify-dual-path-update.sh`。执行者按 owner 收口、补测试和 map，不重写他人 hunk。

必须锁定的契约：

- `scripts/release.sh` 构建唯一签名 APK；`dist/android/update.json` 与 `dist/relay/updates/latest.json` 的 `versionCode`、`versionName`、`sha256`、`size` 完全一致，APK 字节与 signer 一致。
- daemon 路径：`http://<tailscale-host>:<port>/android/update.json` + `/android/freehand-android.apk`。
- Relay 路径：`https://<relay-host>/relay/updates/latest.json` + `/relay/updates/freehand-android.apk`。
- Android `ApkUpdateManifest` 对 positive 更高版本必须要求 `sha256` 与 `size`；下载后先验 size 再验 SHA-256，不匹配拒绝安装并显式报错。
- Android 每次只使用显式选中/配置的 manifest URL；两条路径都是显式 endpoint，禁止失败后自动切换另一条。
- `scripts/verify-dual-path-update.sh <tailscale-manifest-url> <relay-manifest-url>` 是发布门禁，比较两个 manifest 并下载两个 APK 做实际 hash/size 校验。

## 文件清单

以下为预期主要文件；执行时以 owner map 和现状 dirty 文件为准，逐文件核实后用可审查 hunk 修改：

- `apps/freehand-relay-server/deploy/claw-deploy.sh`、`freehand-relay.service`、`relay.env.example`
- `crates/freehand-relay/src/service.rs`、`config.rs`、`tests/relay_http_blackbox.rs`（updates 收口）
- 新 `crates/freehand-account-config/**`（工作名，feature 落 map 后定）
- `crates/freehand-config/src/lib.rs`（配置 schema/校验/effective projection）
- `crates/freehand-ui-protocol/**`、`crates/freehand-runtime/**`（账号配置状态 query/command bridge）
- `apps/freehand-server/assets/webui/**`（Settings 账号配置状态/冲突/上传入口）
- `apps/freehand-android/**`（`AndroidApkUpdater`、`ApkUpdateManifest`、verifier）
- `scripts/release.sh`、`scripts/verify-dual-path-update.sh`、`scripts/verify-relay-deployment-smoke.sh`
- `docs/resource-maps/core.json`、`docs/architecture/feature-map.md`、`docs/module-registry/**`、`docs/function-maps/**`、`docs/mainline-calls/**`、`docs/testing/**`、`docs/verification-maps/**`、`docs/wiki/**`
- `.agents/skills/freehand-dev/SKILL.md`、`MEMORY.md`、`note.md`

## 风险与规避

| 风险 | 规避 |
| --- | --- |
| 把配置共享做成整机 config.toml 上传 | 只同步 schema 允许的非 secret 字段；schema/scan/正反测试锁定 secret 拒绝 |
| last-write-wins 静默覆盖配置 | revision/ETag + 409 显式冲突；客户端必须展示冲突 |
| Tailscale 与 Relay 被当成 fallback | 显式 endpoint candidate 建模；verifier 证明失败不切换 |
| 两条升级路径产物不一致 | release 单真源 + 双路径 byte 比较门禁 |
| 配置同步逻辑混入 Relay/ADP payload | typed config resource/API + owner gate；禁塞 metadata/业务 payload |
| 新 feature 未落 map 就写代码 | 先 resource/function/mainline/test/gate 落盘并 `xtask gates check`，status 先 design |
| 覆盖并行 worker 改动 | 刷新 `.agent-collab`、按语义 claim、逐文件 hunk、精确 stage/commit |
| Claw 部署/重启是外部状态变更 | 实际部署前确认凭据与时机；部署脚本必须可重复、可回滚、留版本证据 |

## 测试与验证矩阵

### 配置同步

- 正向：同账号多设备拉取同一 revision；PUT 成功生成新 revision/etag；客户端应用后设备同步状态更新。
- 反向：跨账号读取/写入 403；`If-Match` 过期 409 且服务器文档完整；secret 字段上传被 schema/gate 拒绝；整机 config 上传被拒绝。
- 并发：两个客户端同一 base revision 并发 PUT，一个成功一个 409；服务器持久化失败不发布新 revision。

### 双路径升级

- 正向：同一 release 的 Tailscale manifest 与 Relay manifest version/sha256/size 一致；两条路径下载的 APK 字节一致；Android 真机从 Relay 公网路径完成原位升级。
- 反向：manifest 缺 sha256/size、hash 不匹配、size 不匹配、updates 目录未配置、路径穿越、manifest/APK 版本不一致全部显式失败；Tailscale 路径失败不会自动切 Relay。

### 部署与在线

- `scripts/verify-relay-deployment-smoke.sh`、`scripts/verify-remote-relay-local-online.sh`、`scripts/verify-dual-path-update.sh` 全部执行。
- Claw 服务安装/重启后：binary SHA-256 与源码构建一致、`/relay/health` ok、账号登录/隔离、updates manifest 可读、旧 store 保留。
- 真机（如可用）：Android 异网登录、配置共享、Relay 路径升级、原位升级后 `versionCode`/signer 证据；设备不可用必须显式标 blocked，不得宣称闭环。

### 架构与基线

- `cargo build --workspace`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`。
- `cargo run -p xtask -- mainlines generate`、`cargo run -p xtask -- mainlines check`、`cargo run -p xtask -- gates check`。
- `git diff --check`；每阶段定向 commit；不把 `.agent-collab`、`output/`、`err*.log`、`__pycache__`、`.DS_Store` 提交。

## 实施步骤

### Phase 0：基线冻结

1. 读 `AGENTS.md`、`CACHE.md`、`MEMORY.md`、`note.md`、`~/.codex/USER.md`；搜 MemoryPalace（当前无 `freehand` wing，记录索引缺口，以 MEMORY/maps 为真源）。
2. 刷新 `.agent-collab`：现有 `feature_relay.transport_*`、`feature_config.core_*`、`feature_foundation.workspace_dual_path_release`、`feature_node.master-slave_remote_daemon_relay` 均标 active；确认 heartbeat，语义冲突则按协议避让/接管。
3. 读取 resource map、feature map、module registry、function map、mainline call map、test design、verification map，锁定 owner/allowed/forbidden paths。
4. 盘点当前 dirty 文件，区分并行 worker 改动与本任务改动；不覆盖、不恢复。

### Phase 1：Claw Relay/updates 收口

1. 按 owner 收口现有 Relay updates 路由、env、deploy 脚本、release staging、dual-path verifier 的未提交改动；补缺失红测和 map 绑定。
2. 本地全量验证 + deployment smoke + local online smoke + dual-path update smoke。
3. 实际 Claw 部署/重启前向 Jason 确认凭据与时机；部署后验证版本/hash/health/账号/隔离/updates。
4. 独立 commit。

### Phase 2：账号配置同步 feature 落 map + 实现

1. 新增 `config.account-config-sync` feature/resource（工作名）到 resource map、feature map、module registry、function map、mainline call map、test design、verification map；status 先 design，gate 先红测。
2. 实现配置 document schema/校验/持久化/revision/ETag/冲突/账号隔离；配置服务器路由边界由 `relay.transport` 提供。
3. `config.core` 增加共享配置 schema 校验、effective projection、secret 白名单拒绝；客户端（WebUI Settings、Android WebView）消费 owner projection。
4. 写正反/并发测试，更新 maps/gates，独立 commit。

### Phase 3：Android 双路径升级闭环

1. 收口现有 Android manifest/size/hash 校验改动；明确 Relay manifest URL 配置入口（canonical WebUI Settings），不做 native 第二套 UI。
2. 本地 Android JVM test + release APK + signer/hash 验证 + 双路径 verifier。
3. 真机原位升级（Tailscale 与 Relay 两条路径分别证明，或显式 blocked）。
4. 独立 commit。

### Phase 4：收口与 Review

1. 全量 baseline、mainlines、gates、部署/在线/双路径/真机证据闭环。
2. `note.md` 提炼 -> `MEMORY.md` -> 更新 `freehand-dev` skill -> 补齐 MemoryPalace 索引缺口（若环境允许）。
3. 按 AGENTS rule 36 运行 Codex Review，明确 PASS 才交付；review 后任何代码/测试/配置改动都必须重新验证再 review。
4. 精确 stage/commit；汇报剩余风险与下一步。

## 完成定义（DoD）

- Claw Relay 可重复部署/升级，运行 binary 与源码构建 SHA-256 一致，`/relay/health`、账号认证、账号隔离、updates 服务全部在线验证。
- 同账号配置共享闭环：版本化 document、revision/ETag、409 冲突、账号隔离、设备同步状态、secret 拒绝都有正反证据；Android/WebUI 通过 owner projection 消费，不持有本地真源。
- Tailscale 与 Relay 双路径发布同一已签名 APK/manifest/hash；Android 原位升级证据存在，或设备不可达时显式 blocked。
- resource/function/mainline/test/verification map、wiki、skill、memory 与实现同步，`xtask gates check` 全绿。
- workspace baseline 全绿，每阶段独立 commit，Codex Review 明确 PASS。

## 非目标与既有计划关系

- 本计划取代 `docs/goals/freehand-claw-relay-config-sync-upgrade-plan.md` 中“本地优先合并/自动切换/上传整机 config”的错误设计；不删除该文件（其他 worker 未提交产物），只以本文档为执行真源。
- `docs/goals/relay-transport-to-agent-dashboard-remaining-plan.md`、`docs/goals/relay-agent-dashboard-mobile-closeout-plan.md` 已过时部分（如 outbound tunnel 未实现、Android 第二套 UI）不得作为新 goal 唯一依据；本文档与最新 maps/MEMORY 为准。
