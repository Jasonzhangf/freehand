# Freehand UI Protocol 单文件拆分实现计划

## 1. 目标与验收标准

**目标**：将 `crates/freehand-ui-protocol/src/lib.rs`（原 8232 行单文件）按职责拆分为多个模块文件，消除 monolith，同时保持对外 API 与 wire 协议语义完全不变。

**验收标准**：
- `lib.rs` 只保留模块声明、re-export 和 `mod tests`，不再承载大段实现。
- 每个模块有清晰单一职责，`owned_paths` 在 module-registry 中声明。
- `cargo test -p freehand-ui-protocol` 79 个测试全部通过（与拆分前基线一致）。
- `cargo build --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 全绿。
- 生成的 `adp-protocol.schema.json` / `adp-protocol.js` 产物不变（wire 语义零变化）。
- 对外 `pub` API 符号集合不变（仅内部模块化，不删不改名）。

## 2. 当前进度（已完成的拆分）

| 文件 | 行数 | 内容 | 状态 |
|---|---|---|---|
| `src/dto.rs` | 1309 | 全部 `Ui*` DTO（command/projection/update/task/timer/tool/worker） | ✅ 已拆 |
| `src/adp_wire.rs` | 576 | ADP 协议面（UiProjection、UiAdpRequest/Response、wire、serde、manifest、version/capability） | ✅ 已拆 |
| `src/adp_descriptor.rs` | 786 | 命令描述符表（UI_COMMAND_DESCRIPTORS）、frame class、public/internal 分类、turn_projection_from_events | ✅ 已拆 |
| `src/lib.rs` | 5596 | 剩余：ports、state、validate、projection helpers、tests | 待拆 |

## 3. 范围与边界

**In scope**：
- 将 lib.rs 剩余 5596 行拆为 ports / state / validate / projection / tests 模块。
- 必要的 `pub(crate)` 可见性调整（跨模块访问私有 item）。
- 补 `docs/module-registry/ui.protocol.json` module-registry。

**Out of scope**：
- 不改任何 `pub` API 签名、不改 wire 序列化、不改 DTO 字段。
- 不引入新依赖、不改协议版本号。
- 不做语义重构（只移动代码 + 可见性调整）。

## 4. 设计原则

1. **字节等价优先**：每个模块内容从 lib.rs 原样搬移，不做逻辑改动。
2. **可见性最小化**：只把跨模块必需的私有 item 改为 `pub(crate)`，不扩大公开面。
3. **re-export 保真**：lib.rs 用 `mod X; pub use X::*;` 保持对外 API 不变。
4. **每步编译验证**：拆一个模块编译一次 + 跑测试，确保零回归。

## 5. 技术方案与文件清单

### 目标模块划分

| 新模块文件 | 源 lib.rs 行段 | 内容 |
|---|---|---|
| `src/ports.rs` | 36-115 | `UiCommandDispatchPort`、`UiRuntimeQueryPort`、`UiQueryAccessScope`、`UiProtocolOnlyQueryPort`、`StaticUiCommandDispatchPort`、`SubscriptionSelector` |
| `src/state.rs` | 117-850 | `UiProtocolState`、`UiProtocolError`、`UiCommandDispatchPortError`、`impl UiProtocolState`、`model_request_activity_from_waiting`、`impl Default` |
| `src/validate.rs` | 859-1529 | `validate_command`、各 validate fn、`accept_command_ingress`、`protocol_rejection`、`build_command_dispatch_envelope`、`dispatch_port_failure`、`subscription_selector`、`subscription_matches` |
| `src/projection.rs` | 1531-2074 | `terminal_text_projection`、`public_conversation_items`、tool activity helpers、`public_turn_projection`、session projection helpers、`turn_projection_for_client` |
| `src/tests.rs` | 2075-5596 | `mod tests`（3521 行） |

### 关键可见性调整（跨模块引用）
- `validate.rs` 调用 `state.rs` 的 `UiProtocolState`（pub）、`adp_descriptor` 的 `command_kind`/`is_command_ingress_kind`/`command_dispatch_target`（已 pub(crate)）。
- `state.rs` 调用 `projection.rs` 的 projection helpers、`validate.rs` 的 `validate_command`（pub）。
- `projection.rs` 调用 `adp_descriptor` 的 `turn_projection_from_events` 等。
- 逐模块编译后按报错补齐 `pub(crate)`，只动必需的最小面。

## 6. 风险与规避

| 风险 | 规避 |
|---|---|
| 拆分破坏 wire 序列化 | 每模块搬移字节等价；跑 79 测试 + 产物 diff 校验 |
| 循环模块依赖 | state/validate/projection 互相引用时，统一在 lib.rs 层 re-export，模块间用 `crate::` 引用 |
| pub(crate) 扩大过度 | 只按编译报错最小补齐，review 时核对 |
| tests 依赖 lib.rs 内部私有 item | tests 保留在 lib.rs `mod tests` 或独立 tests.rs 用 `use super::*` |

## 7. 测试计划

1. **基线**：拆分前 `cargo test -p freehand-ui-protocol` = 79 passed。
2. **每模块后**：`cargo build -p freehand-ui-protocol` + `cargo test -p freehand-ui-protocol`。
3. **全量**：`cargo build --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo run -p xtask -- gates check`、`git diff --check`。
4. **产物**：确认 `adp-protocol.schema.json` / `adp-protocol.js` 无 diff。

## 8. 实施步骤（顺序）

1. 拆 `src/ports.rs`（36-115）→ 编译 + 测试。
2. 拆 `src/state.rs`（117-850）→ 编译 + 测试。
3. 拆 `src/validate.rs`（859-1529）→ 编译 + 测试。
4. 拆 `src/projection.rs`（1531-2074）→ 编译 + 测试。
5. 拆 `src/tests.rs`（2075-5596）→ 编译 + 测试。
6. 补 `docs/module-registry/ui.protocol.json`。
7. 全量验证（build/clippy/test/gates/diff-check）。
8. 更新 `docs/function-maps/ui.protocol.md`、`docs/wiki/ui.protocol.md`、`docs/mainline-calls/ui.protocol.json` 的模块路径。
9. codex-review → PASS → 提交。

## 9. 完成定义（DoD）

- [ ] `lib.rs` 不再承载 ports/state/validate/projection 实现（只留 mod 声明 + re-export + tests）。
- [ ] 79 测试通过。
- [ ] workspace build/clippy/test/gates 全绿。
- [ ] 产物 schema/js 无 diff。
- [ ] module-registry 补录。
- [ ] codex-review PASS。
- [ ] 提交。
