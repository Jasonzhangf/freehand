# Architecture Gaps

Non-violation pending items. Not regressions. Not false positives. Each gap has explicit owner, known scope, documented risk, and no active gate violation.

## Gap 1: `tool.preview` — `delete_range` 无 preview/execute 实现

| Field | Value |
|---|---|
| feature_id | `tool.preview` |
| owner crate | `crates/freehand-tools` |
| gap kind | 能力面未闭环 — 只有 spec（`implemented=false`），缺 `plan_delete_range` + `execute_delete_range` + preview/execute parity test |
| why not violation | spec 正确声明 `implemented=false`；runtime preview 入口显式拒绝非 `write_file/edit_file/multi_edit` 的工具；未伪装已完成 |
| risk | 如果后续把 `delete_range` 改为 `implemented=true` 但不补 preview + checkpoint gate，则 writable file-mutation 会绕过 checkpoint ledger，产生不可回滚状态 |
| gate | 当前 gate 不会拦（因为 xtask gates 不检查 `implemented` 状态与 preview 入口之间的差距） |
| entry symbol | `BuiltinToolRegistry::preview`（`crates/freehand-tools/src/lib.rs:141`）只 dispatch `write_file/edit_file/multi_edit` |
| spec location | `crates/freehand-tools/src/lib.rs:288` |
| test design | `docs/testing/tool.preview.md` — known gaps 已记录 |
| function map | `docs/function-maps/tool.preview.md` — sync status 已记录 pending |
| priority | 中 — 当前无外部路径可达 `delete_range`（`implemented=false`），不阻塞任何 E2E 流程 |
| closure path | 1) 锁定 `delete_range` anchor 语义（start_anchor/end_anchor/inclusive） 2) 实现 `plan_delete_range` → preview + execute 3) 加 preview/execute parity test 4) 暴露到 runtime checkpoint gate |

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

## 管理规则

1. 本文件只记录 **非违规欠账**。违规必须改或删。
2. 新增 gap 必须有：`feature_id`、owner、gap kind、risk、gate 状态、closure path。
3. gap 关闭后，从此文件删除，不可留"已关闭"占位符。
4. 本文件由 `docs/architecture/feature-map.md` 路由索引中 `architecture-gaps` 段引用。
