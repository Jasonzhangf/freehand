# Mainline Migration Closeout Plan

## Goal

完成 Freehand 剩余 feature 的 machine-readable mainline-call 迁移收口，使 feature-map、function-map、test-design、mainline-call JSON、generated wiki、xtask gate 五层真源完全对齐。

## Acceptance

目标仅在以下条件全部满足时视为完成：

1. 剩余未迁移 feature 全部补齐：
   - `foundation.workspace`
   - `config.core`
   - `contracts.core`
   - `node.master-slave`
   - `app.cli-runtime-smoke`
   - `app.cli-live-turn`
   - `app.webui-smoke`
   - `app.runtime-daemon`
2. 上述每个 feature 都具备：
   - `docs/function-maps/<feature>.md`
   - `docs/testing/<feature>.md`
   - `docs/mainline-calls/<feature>.json`
   - `docs/wiki/<feature>.md`
   - `docs/architecture/feature-map.md` 中的 `mainline_call_doc` 与 `generated_wiki_doc`
3. `docs/mainline-calls/README.md` 与 `docs/wiki/README.md` 完整列出全部已迁移 feature。
4. `xtask gates check` 强制要求上述 feature 的 function map、test design、JSON source、generated wiki。
5. `cargo run -p xtask -- mainlines check` 通过，证明 wiki 全部由 JSON 真源生成且无陈旧。
6. `cargo build --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo run -p xtask -- gates check` 全部通过。
7. 每个迁移 feature 完成后都有独立小提交；全部迁移完成后 worktree 干净。

## Scope

### In Scope

- 剩余未迁移 feature 的 mainline-call JSON 真源补齐
- generated wiki 产物补齐
- feature-map 迁移字段补齐
- `xtask` required-file gate 同步补齐
- `CACHE.md`、`MEMORY.md`、`note.md` 同步记录迁移真相

### Out of Scope

- 非迁移目的的运行时逻辑重构
- 改变已有 feature 的 owner、语义边界或测试策略
- 引入新的 feature_id
- 修改 provider/reason/runtime/UI 的业务逻辑，只允许为保持文档真源一致而做最小文字更新

## Design Principles

1. 只做真源迁移，不借机改业务语义。
2. generated wiki 永远不手写，只能由 `xtask mainlines generate` 生成。
3. function map、test design、mainline-call JSON、generated wiki 必须同一变更集同步更新。
4. `xtask gates check` 必须成为最终硬门禁，不能只靠人工约定。
5. 每次只迁一个 feature，小步提交，避免混入无关差异。
6. 若某 feature 的 function map 或 test design 绑定不完整，先修文档真源再做迁移。

## Remaining Feature Inventory

### 1. `foundation.workspace`

- owner: `xtask`, workspace root
- target: 补齐 workspace gate / mainline generation / wiki freshness 的 machine-readable mainline 真源
- expected checks:
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `cargo test --workspace`

### 2. `config.core`

- owner: `crates/freehand-config`
- target: 补齐 config load / validate / provider selection / peer topology bootstrap 的 mainline 真源
- expected checks:
  - `cargo test -p freehand-config`
  - `cargo run -p xtask -- gates check`

### 3. `contracts.core`

- owner: `crates/freehand-contracts`
- target: 补齐 shared IDs / request-response-error contract / serialization boundary 的 mainline 真源
- expected checks:
  - `cargo test -p freehand-contracts`
  - `cargo run -p xtask -- gates check`

### 4. `node.master-slave`

- owner: `crates/freehand-node`
- target: 补齐 pairing / delegation / direct-message / slave turn publication 的 mainline 真源
- expected checks:
  - `cargo test -p freehand-node`
  - `cargo run -p xtask -- gates check`

### 5. `app.cli-runtime-smoke`

- owner: `apps/freehand-cli`
- target: 补齐 config-selected runtime harness CLI smoke 的 mainline 真源
- expected checks:
  - `cargo test -p freehand-cli`
  - `cargo run -p xtask -- gates check`

### 6. `app.cli-live-turn`

- owner: `apps/freehand-cli`
- target: 补齐 live provider CLI boundary / completion-loop projection / tool summary 的 mainline 真源
- expected checks:
  - `cargo test -p freehand-cli`
  - `cargo run -p xtask -- gates check`

### 7. `app.webui-smoke`

- owner: `apps/freehand-server`
- target: 补齐 protocol-only WebUI transport/query/subscribe/command ingress 的 mainline 真源
- expected checks:
  - `cargo test -p freehand-server`
  - `cargo run -p xtask -- gates check`

### 8. `app.runtime-daemon`

- owner: `apps/freehand-daemon`
- target: 补齐 runtime-backed HTTP/SSE host / restore/bootstrap / dispatch injection 的 mainline 真源
- expected checks:
  - `cargo test -p freehand-daemon`
  - `cargo run -p xtask -- gates check`

## Technical Plan

### Phase 1. Inventory lock

对剩余未迁移 feature 做一次只读盘点：

- 确认 `feature-map` 中是否已存在 `mainline_call_doc` / `generated_wiki_doc`
- 确认 `docs/function-maps/` 与 `docs/testing/` 文档是否足够支撑 JSON 抽取
- 确认 `xtask` 是否已强制要求对应文件

若发现文档绑定缺口，先修文档，再做 JSON 迁移。

### Phase 2. Per-feature migration

每个 feature 统一按以下步骤执行：

1. 在 `docs/architecture/feature-map.md` 为该 feature 补：
   - `mainline_call_doc`
   - `generated_wiki_doc`
   - 如有必要，补 `allowed_paths`
2. 在 `docs/function-maps/<feature>.md` 追加 mainline/wiki 同步状态说明。
3. 在 `docs/testing/<feature>.md` 追加 mainline/wiki 同步状态说明。
4. 新增 `docs/mainline-calls/<feature>.json`。
5. 运行 `cargo run -p xtask -- mainlines generate` 生成 `docs/wiki/<feature>.md` 并刷新 `docs/wiki/README.md`。
6. 更新 `docs/mainline-calls/README.md`。
7. 更新 `xtask/src/main.rs` required-file 列表。
8. 更新 `CACHE.md`、`MEMORY.md`、`note.md`。
9. 跑该 feature 映射测试 + `mainlines check` + `gates check` + workspace baseline。
10. 做一个独立 commit。

### Phase 3. Final closeout

所有剩余 feature 迁移完成后：

1. 再次核对 `docs/architecture/feature-map.md` 中所有需要迁移的 feature 都具备 mainline/wiki 字段。
2. 再次核对 `docs/mainline-calls/README.md` 和 `docs/wiki/README.md` 与真实文件集一致。
3. 跑完整 workspace 基线：
   - `cargo build --workspace`
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - `cargo run -p xtask -- mainlines check`
   - `cargo run -p xtask -- gates check`
4. 确认 worktree 干净。

## File List

本次 closeout 预计会持续触达：

- `docs/architecture/feature-map.md`
- `docs/mainline-calls/README.md`
- `docs/wiki/README.md`
- `xtask/src/main.rs`
- `CACHE.md`
- `MEMORY.md`
- `note.md`

以及每个目标 feature 对应的：

- `docs/function-maps/<feature>.md`
- `docs/testing/<feature>.md`
- `docs/mainline-calls/<feature>.json`
- `docs/wiki/<feature>.md`

## Risks And Avoidance

### Risk: 迁移时把 wrong feature 的 allowed_paths 或迁移字段打到相邻 section

Avoidance:

- 每次 patch 前先用精确上下文读取目标 section
- diff 审核时重点核对相邻 feature 是否被误改

### Risk: `xtask` required-file 列表漏项，导致 gate 不能真正锁住迁移

Avoidance:

- 每迁一个 feature 都同步补：
  - function map
  - test design
  - mainline JSON
  - generated wiki
- 每次都跑 `cargo run -p xtask -- gates check`

### Risk: generated wiki 被误当手写文档修改

Avoidance:

- 所有 wiki 只通过 `cargo run -p xtask -- mainlines generate` 更新
- 每次都跑 `cargo run -p xtask -- mainlines check`

### Risk: app-boundary feature 的 mainline 描述夹带实现语义漂移

Avoidance:

- app feature 只描述 boundary、dispatch、projection、host bootstrap
- 不在迁移任务中改 runtime/provider/reason 的 owner 语义

## Test Plan

### Per-feature minimum

- `cargo fmt --all --check`
- `cargo run -p xtask -- mainlines check`
- feature owner crate/app 定向测试
- `cargo run -p xtask -- gates check`

### Full regression baseline

- `cargo build --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

## Implementation Order

推荐顺序：

1. `foundation.workspace`
2. `config.core`
3. `contracts.core`
4. `node.master-slave`
5. `app.cli-runtime-smoke`
6. `app.cli-live-turn`
7. `app.webui-smoke`
8. `app.runtime-daemon`

理由：

- 先收口底层治理与 shared contracts
- 再收口 node / runtime / CLI / server app boundary
- 最后一次性跑 workspace closeout

## Definition Of Done

满足以下全部条件才算收口完成：

1. 剩余未迁移 feature 全部补齐 mainline JSON 与 generated wiki。
2. `feature-map`、`function-map`、`test-design`、`mainline-call`、`wiki` 五层真源全部对齐。
3. `xtask gates check` 对全部迁移 feature 实施 required-file 强校验。
4. `xtask mainlines check` 通过。
5. workspace build / fmt / clippy / test / gates 全通过。
6. 每个 feature 都有独立提交，最终 worktree 干净。
