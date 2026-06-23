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
| gap kind | 控制/观测 provenance 覆盖面不全 — 当前已接 producer：`reason.turn`、`runtime live bridge`、`node runtime`；未接 producer：`freehand-provider-anthropic`、`freehand-provider-openai`、`freehand-debug` |
| why not violation | metadata owner、隔离 gate、已接 producer 都成立；未接 producer 不走 metadata 中心化渠道，但仍符合隔离规则 |
| risk | provider adapter 运行时生命周期事件（连接失败、HTTP 状态码、原始错误）无 metadata 中心化审计；debug 事件无 metadata 中心化入口 |
| gate | 当前 gate 不会拦（xtask gates 只锁 metadata/request 类型隔离，不锁 producer 注册） |
| current producers | `freehand-reason`（turn lifecycle）、`freehand-runtime`（live bridge lifecycle）、`freehand-node`（node lifecycle） |
| missing producers | `freehand-provider-anthropic`、`freehand-provider-openai`、`freehand-debug` |
| test design | `docs/testing/metadata.core.md` — known gaps 已记录 |
| function map | `docs/function-maps/metadata.core.md` — sync status 已记录 pending |
| priority | 低 — 当前 producer 已覆盖核心生命周期（turn/runtime/node），不阻塞任何 E2E 流程 |
| closure path | 1) 为每个 missing producer 定义 `MetadataWriteOwner` + `MetadataWriteNode` 2) 在 provider adapter 关键路径（请求构造、响应解析、错误分类）前插入 metadata write 3) 在 `freehand-debug` 关键 sink 路径前插入 metadata write 4) 加 producer 白盒测试 5) 同步 function map + test design |

## 管理规则

1. 本文件只记录 **非违规欠账**。违规必须改或删。
2. 新增 gap 必须有：`feature_id`、owner、gap kind、risk、gate 状态、closure path。
3. gap 关闭后，从此文件删除，不可留"已关闭"占位符。
4. 本文件由 `docs/architecture/feature-map.md` 路由索引中 `architecture-gaps` 段引用。
