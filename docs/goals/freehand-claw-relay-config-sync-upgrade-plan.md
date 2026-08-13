 # Freehand Claw Relay + 配置共享 + 升级闭环实现计划

 ## 目标与验收标准

 目标：在 Freehand 建立可部署到 Claw 的 Relay + 配置共享 + 升级闭环。Tailscale 与 Relay 为两条显式 transport mode；同账号登录后配置服务器共享非 secret 配置；同一发布产物通过 Tailscale 与 Relay 两条路径都可发现、下载、校验 hash、完成 Android/daemon 升级。

 验收标准：
 1. `relay.transport` 已有 account/presence/tunnel owner，Claw 部署 `freehand-relay-server` 服务通过 Tailscale 与 Relay 两条路径可达，账号 `jason` 已注册。
 2. Claw 上 Relay server 与本地 Tailscale 直连使用同一套 `crates/freehand-relay`；Claw relay 只部署 server 进程，不部署 daemon。
 3. `config.core` 是唯一账号配置服务器，声明 relay URL + token env；同账号登录后 WebUI/Android/daemon 可见共享非 secret 配置（provider registry、model groups、relay endpoint candidates）。
 4. Android 同账号登录后能拉取配置服务器的共享配置并同步到 UI。
 5. 升级产物（daemon binary、APK、manifest）通过 Tailscale daemon `/android/update.json` 与 Relay `/relay/updates/latest.json` 两条路径都可发现、下载、校验 hash 一致、完成 Android 原位升级与 daemon 二进制替换。
 6. 三条主线物理隔离：relay.transport 只管账号/Agent presence/tunnel；config.core 只管账号配置服务器/配置共享真源；foundation.workspace 只管单一升级产物与 Tailscale/Relay 发布门禁。
 7. 无 fallback：Tailscale 与 Relay 是显式 transport mode；失败必须显式报错，不静默切换。
 8. 所有 resource map、function map、mainline call map、verification map、xtask gates 全绿；codex review PASS 后交付。

 ## 范围与边界

 ### In Scope
 - Claw 部署 Freehand Relay server（已有 server 二进制，待补 deploy 脚本/env/systemd）。
 - `config.core` 增加账号级配置服务器路由：Agent 可选配置 relay URL + token env，通过 Relay account 登录后向配置服务器拉取共享配置。
 - Relay server 增加 `/relay/updates/latest.json` 与 `/relay/updates/<file>` 静态文件服务，Android/daemon 可通过 Relay 路径下载更新产物。
 - `app.android-client` 接线：同账号登录 Relay 后从配置服务器拉取 provider registry、model groups 等非 secret 配置，同步到 WebUI/Android settings。
 - `app.runtime-daemon` / `foundation.workspace` 补升级发布门禁：daemon + APK 发布产物通过 Tailscale 与 Relay 两条路径可验证 hash 一致。
 - 更新 resource map、function map、mainline call map、test design、verification map。
 - 每 phase 独立 commit。

 ### Out of Scope
 - 不改 `relay.transport` 核心协议（account/register/login/presence/directory/tunnel 协议已锁定）。
 - 不做跨 Freehand daemon 进程的真实跨进程配置共享（Freehand daemon 是单进程 agent，不同于 zterm 多设备 relay 场景）。
 - 不做 relay signaling（Tailscale 直连已有 Agent 间 WebSocket 能力）。
 - 不做 TURN/Traversal relay（Tailscale Funnel/SSH reverse tunnel 已覆盖 NAT 穿透）。
 - 不把 provider key、token、secret 写入配置服务器 payload 或 manifest。

 ## zterm 对照审计结论

 ### 采用
 1. **独立 relay server 发行包**：`@jsonstudio/zterm-relay-server` + systemd/env/template 模式。Freehand 直接复用 `freehand-relay-server` 独立进程部署到 Claw。
 2. **relay server 托管 `/updates/` 静态文件**：`server.ts` 的 `/updates/latest.json` 和 APK 路径分发。Freehand 在 `freehand-relay-server` 增加同等的 `/relay/updates/latest.json` 路由。
 3. **relay 更新路径注入**：Android 从 relay login 响应里获取 manifest URL 并拼接 `relayUrl + /updates/latest.json`。Freehand 复用此模式。
 4. **配置共享 link**：`connection-config-share.ts` 用 URL-safe base64 JSON deep link 编码多 host + quick actions + shortcut actions。Freehand 复用此格式做同账号非 secret 配置同步。
 5. **Tailscale/relay 双路径升级**：daemon 本地 `/android/update.json` + relay 公网 `/updates/latest.json`。Freehand 已在 `freehand-server` 服务 `/android/update.json`，只需补 relay `/relay/updates/latest.json`。

 ### 不采用
 1. **daemon npm 包**：Freehand 是 Rust 原生 daemon，不走 npm。
 2. **WS signaling / peer lease**：Freehand relay 是 HTTP/ADP proxy + directory，不做 RTC signaling（zterm 用 relay 做 WebRTC NAT穿透；Freehand 用 Tailscale Funnel）。
 3. **RT relay-only candidate**：`iceTransportPolicy=relay` 是 zterm 特定需求。Freehand relay 是纯 HTTP/ADP 代理，不处理 ICE。
 4. **WS `/connect/` 二进制转发**：zterm relay 支持任意 WS 路径代理。Freehand relay 只代理 `/relay/agents/{id}/adp` 和 `/relay/agents/{id}/connect`。
 5. **Mac daemon + relay 合包**：Freehand daemon 和 relay server 天然分离。

 ## 设计原则

 1. **显式双路径**：Tailscale 直连（`http://<tailscale-ip>:<port>/android/update.json`）与 Relay 公网（`https://<relay-host>/relay/updates/latest.json`）是两条独立 upgrade channel；不在应用层 fallback。
 2. **控制面/业务 payload 隔离**：账号、路由、presence、upgrade state、error 不能进入 HTTP/ADP 业务 payload 或 metadata。
 3. **Secret 安全引用**：provider key、relay token 只通过 env/env-file/keychain 解析；配置服务器 payload 不含 secret 字段。
 4. **唯一 owner**：relay.transport 管 tunnel；config.core 管账号配置服务器；foundation.workspace 管升级发布门禁；app.android-client 管 WebView 接线。
 5. **无隐式 fallback**：transport mode 失败必须显式报错；upgrade channel 失败必须报错，不自动切换另一条路径。
 6. **持久化先成功再发布**：账号/token/presence/upgrading truth 先持久化再替换内存 truth。

 ## 技术方案与文件清单

 ### A. Claw Relay Server 部署（Phase 1）

 **Owner**：`relay.transport`（`apps/freehand-relay-server`）

 **文件**：
 - `apps/freehand-relay-server/`：已有 Rust server 实现。
 - 新增 `apps/freehand-relay-server/deploy/claw-deploy.sh`：Claw 部署脚本（scp 二进制 + systemd）。
 - 新增 `apps/freehand-relay-server/deploy/relay.env.example`：env 模板（bind/store/lease/token/updates dir）。
 - 新增 `apps/freehand-relay-server/deploy/freehand-relay.service`：systemd unit。
 - Claw 部署目标：`159.75.134.56`（已有 v2 relay 在此 IP）。

 **关键路由**（已有，待确认 Claw 部署链路）：
 ```
 /relay/health
 /relay/api/auth/register
 /relay/api/auth/login
 /relay/api/agents
 /relay/api/agents/subscribe
 /relay/agents/{agent_id}/adp
 /relay/agents/{agent_id}/connect
 /relay/updates/latest.json    ← 新增 relay-hosted 升级 manifest
 /relay/updates/<file>        ← 新增 relay-hosted 升级产物
 ```

 ### B. 配置服务器 + 账号配置共享（Phase 2）

 **Owner**：`config.core`（`crates/freehand-config`）+ `relay.transport`

 **文件**：
 - `crates/freehand-config/src/lib.rs`：扩展 Agent relay 连接配置，增加 relay account login 后拉取共享配置的 client 逻辑。
 - `crates/freehand-relay/src/agent_client.rs`：RelayAgentClient 增加配置服务器 HTTP API 调用。
 - `crates/freehand-server/src/lib.rs`：WebUI settings 配置状态从 `config.core` safe projection 读取。
 - `apps/freehand-android/`：Android WebView 读取共享配置并同步到 settings。

 **设计**：
 1. Agent 启动时若配置了 `relay_url` + `relay_token_env`，RelayAgentClient 登录 relay 后：
    - 调用 relay HTTP API `/relay/api/config/share`（新增）拉取同账号共享配置。
    - 配置服务器返回非 secret 配置：provider registry（id/type/protocol/base_url/model/auth_source）、model groups（id/label/routes/weights）。
    - `config.core` 合并共享配置与本地配置，冲突时本地优先。
 2. WebUI/Android 从 `config.core` safe projection 读取配置状态；不直接访问 relay payload。
 3. Secret（API key、pair token、relay token）不在共享 payload 中，通过 env/env-file 解析。

 ### C. 升级产物 + 双路径发布（Phase 3）

 **Owner**：`foundation.workspace` + `app.android-client`

 **文件**：
 - `scripts/release.sh`：已生成 `dist/android/update.json` 和 `dist/android/freehand-android-release.apk`。
 - 新增：relay server 的 `/relay/updates/latest.json` 和 `/relay/updates/freehand-android.apk` 服务路由（可在 `apps/freehand-relay-server/src/main.rs` 增加 static file serving）。
 - `apps/freehand-android/`：已实现 `AndroidApkUpdater` 拉取 daemon `/android/update.json`；需扩展为可配置 manifest URL（默认 daemon 本地，relay 账号登录后切换到 relay 公网路径）。
 - `apps/freehand-server/src/lib.rs`：已有 `/android/update.json` + `/android/freehand-android.apk` 服务。

 **设计**：
 1. **Tailscale 路径**：daemon WebUI 在 `http://<tailscale-ip>:4042/android/update.json` 服务 manifest + APK；本地 launchd profile 安装的 Android APK 直接用此路径检查升级。
 2. **Relay 公网路径**：relay server 的 `/relay/updates/latest.json` 服务 relay-hosted manifest；Android 登录 relay 账号后，`AndroidApkUpdater` 配置 manifest URL 为 `https://<relay-host>/relay/updates/latest.json`，APK URL 同理。
 3. **Hash 一致性**：`scripts/release.sh` 同时发布到 daemon dist 和 relay dist（或 relay server 直接从 daemon dist scp/rsync 拉取），两条路径的 `sha256` 必须一致。
 4. **Upgrade manifest 格式**（已有）：
    ```json
    {
      "versionCode": 20001,
      "versionName": "0.2.1",
      "apkUrl": "/android/freehand-android.apk",
      "sha256": "<hex>",
      "size": 12345678
    }
    ```

 ## 实施步骤

 ### Phase 0：Claw Relay Deploy 完成（收口）
 1. 确认 `apps/freehand-relay-server` 二进制在 Claw 上运行在 `0.0.0.0:19091`。
 2. 补 `claw-deploy.sh` 部署脚本 + `relay.env.example` + `freehand-relay.service`。
 3. 验证 `/relay/health` + account register/login + directory API。
 4. 更新 deploy 文档和 map。

 ### Phase 1：Relay Updates 路由
 1. `apps/freehand-relay-server/src/main.rs` 增加 `/relay/updates/` 静态文件服务。
 2. `scripts/release.sh` 发布后自动 rsync/scp 到 Claw relay server 的 updates 目录。
 3. 验证 Tailscale 路径（daemon `/android/update.json`）与 Relay 路径（`/relay/updates/latest.json`）hash 一致。
 4. 更新 resource map、function map、mainline、gates。

 ### Phase 2：配置服务器 + 账号配置共享
 1. 在 `crates/freehand-relay` 增加 `/relay/api/config/share` HTTP 端（返回 `config.core` safe projection JSON）。
 2. `crates/freehand-relay/src/agent_client.rs` 增加登录后拉取共享配置的逻辑。
 3. `crates/freehand-config` 增加配置合并策略（共享 + 本地，冲突本地优先）。
 4. WebUI settings 页面从 `config.core` 读取并显示共享配置。
 5. 写红测覆盖共享配置拉取、合并、secret 不泄露。
 6. 更新 maps、gates、review。

 ### Phase 3：Android Upgrade 接线
 1. `AndroidApkUpdater` 支持配置 manifest URL（默认 daemon 本地，relay 登录后可切换到 relay 公网路径）。
 2. 同账号登录 relay 后，manifest URL 自动更新为 `https://<relay-host>/relay/updates/latest.json`。
 3. `verify-device-ui.sh` 增加 relay 路径 upgrade 验证。
 4. 写红测覆盖双路径 upgrade channel、hash 校验、安装流程。
 5. 更新 maps、gates、review。

 ### Phase 4：收口与 Review
 1. 全部 gates：`cargo build --workspace`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo run -p xtask -- mainlines generate/check`、`cargo run -p xtask -- gates check`。
 2. 在线验证：Claw relay health、account login、config share API、双路径 upgrade manifest hash 一致、Android upgrade 流程。
 3. codex review PASS。
 4. 独立 commit + memory/skill 同步。

 ## 完成定义

 - Claw relay server 可通过 Tailscale 和 Relay 两条路径访问，health 正常。
 - 同账号登录后共享配置（provider registry、model groups、relay endpoint）可被 WebUI/Android 拉取。
 - daemon `/android/update.json` 与 relay `/relay/updates/latest.json` 的 sha256 完全一致。
 - Android 通过 Tailscale 路径和 Relay 公网路径都能完成 upgrade manifest 下载、hash 校验、APK 下载、系统安装流程。
 - 所有 resource/function/mainline/verification map 更新并一致。
 - workspace baseline + mainlines + gates 全绿。
 - codex review PASS。
 - 每 phase 独立 commit；无目标相关 dirty files。
