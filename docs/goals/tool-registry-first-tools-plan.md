# Tool Registry First Tools Plan

## 1. 目标与验收标准

目标：

- 在已锁定的 `tool.registry` harness 下，完成第一批真实内建工具实现，并让 live provider tool loop 通过统一 registry 真正执行这些工具，而不是依赖占位或编排层逻辑。

验收标准：

- 第一批工具在 `crates/freehand-tools` 内有真实实现，且不在 runtime / app / provider / reason 编排层重复实现。
- 每个工具都有明确 spec、显式 `implemented` 状态、函数绑定、测试设计和代码实现。
- runtime 仅通过 `BuiltinToolRegistry::implemented_definitions` 暴露工具，通过 `BuiltinToolRegistry::execute` 执行工具。
- 针对新增工具的白盒、模块黑盒、项目黑盒测试全部通过。
- 全仓 gate 通过，且工具错误路径仍保持显式失败、无 fallback。

## 2. 范围与边界

In Scope：

- `crates/freehand-tools` 第一批真实工具实现
- `docs/function-maps/tool.registry.md` 更新
- `docs/testing/tool.registry.md` 更新
- 如需要，补充 `docs/design/tool-registry-design.md`
- runtime live bridge 对 registry 的消费验证

Out of Scope：

- 新 provider 协议实现
- UI 展示改版
- master/slave 网络拓扑扩展
- side-effect 很重或权限模型未设计完的工具全量落地

## 3. 设计原则

- `freehand-tools` 是 built-in tool spec 和执行唯一 owner。
- 先 owner、后实现；先 spec / map / test-design，后暴露到 live path。
- 禁止 fallback，未知工具、未实现工具、参数非法必须显式失败。
- 编排层纯编排；共享语义和工具执行逻辑不得落在 runtime/app/reason。
- 先落地生命周期清晰、权限边界清晰、便于验证的工具。

## 4. 技术方案

首批建议工具：

- `read_file`
- `grep`
- `glob`
- `ls`

选择理由：

- 以只读工具为主，生命周期和权限边界更容易先锁住。
- 能明显增强多轮推理和检索能力。
- 可以在不引入写盘/命令执行风险的前提下完成真实 E2E。

文件清单：

- `crates/freehand-tools/src/lib.rs`
- `crates/freehand-runtime/src/lib.rs`
- `docs/function-maps/tool.registry.md`
- `docs/testing/tool.registry.md`
- `docs/design/tool-registry-design.md`
- 如需新增 fixture，则放在对应 owner crate 下

实现要点：

- 在 `freehand-tools` 中为首批工具补参数解析、路径/模式校验、执行逻辑、显式错误。
- 保持 registry spec 与实现状态同步：
  - 已实现工具 `implemented = true`
  - 未实现工具继续 `implemented = false`
- runtime 不增加第二份工具逻辑，只消费 registry。
- 如需通用 helper，优先下沉共享 owner，不在 orchestrator 新增临时 helper。

## 5. 风险与规避

风险：

- 工具语义和 Reasonix 命名对齐但行为不完全一致。
- 文件系统读类工具容易引入路径边界不清的问题。
- runtime 侧可能再次出现“临时 demo tool”回流。

规避：

- 行为差异必须写入 function map 和测试设计，不能靠聊天约定。
- 只读工具先锁目录边界、错误类型、输出格式，再开放实现。
- 用模块黑盒和项目黑盒测试证明 runtime 只消费 owner registry。

## 6. 测试计划

白盒：

- spec 导出顺序与 `implemented` 状态
- 每个新工具的参数校验
- 成功执行路径
- 非法参数路径
- unknown / unimplemented 显式错误路径

模块黑盒：

- runtime 暴露的工具列表仅来自 `implemented_definitions()`
- runtime 对完成的 tool call 只通过 `BuiltinToolRegistry::execute` 执行

项目黑盒：

- live provider tool loop 能真实触发至少一个新实现的只读工具
- 完整链路中不再依赖 runtime 内部 demo tool

验证矩阵：

- 定向：`cargo test -p freehand-tools`
- 定向：`cargo test -p freehand-runtime`
- 全量：`cargo build --workspace`
- 全量：`cargo fmt --check`
- 全量：`cargo clippy --workspace --all-targets -- -D warnings`
- 全量：`cargo test --workspace`
- gate：`cargo run -p xtask -- gates check`

## 7. 实施步骤

1. 先锁定首批工具名单和边界，优先只读工具。
2. 更新 `tool.registry` function map，把新增工具的 spec / execute 绑定写实。
3. 更新 `tool.registry` test design，补每个工具的白盒、黑盒、E2E 覆盖说明。
4. 在 `freehand-tools` 实现首批工具，并保持未实现工具显式失败。
5. 校正 runtime 侧工具暴露和执行链路，只保留 owner registry 入口。
6. 增加定向和端到端测试，覆盖成功/失败两面。
7. 跑全量 gate，修到全部通过。

## 8. 完成定义（DoD）

- 第一批真实工具已落在 `freehand-tools`
- registry / function map / test design / design doc 同步
- runtime 无硬编码 demo tool 逻辑残留
- live provider tool loop 至少有一个真实只读工具可用
- 全仓 gate 通过
