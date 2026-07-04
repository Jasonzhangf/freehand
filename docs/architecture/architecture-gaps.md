# Architecture Gaps

Non-violation pending items. Not regressions. Not false positives. Each gap has explicit owner, known scope, documented risk, and no active gate violation.

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

## Gap 3: `control.center` / `error.center` — 流程控制和错误处理未中心化

| Field | Value |
|---|---|
| feature_id | `control.center`, planned `error.center`, planned `task.orchestration` |
| owner crate | `crates/freehand-control`; current related owners are `metadata.core`, `reason.turn`, `runtime.ui-command-dispatch`, `node.master-slave` |
| gap kind | agent-framework control and error policy are still distributed; the basic `control.center` status parser and stopHook skeleton exist, but completion schema retry, provider/tool error decisions, task/subagent action tools, and runtime flow rhythm are not fully admitted through one metadata-watermarked control/error center |
| why not violation | existing landed behavior still has explicit owners and metadata/request isolation gates; basic status stopHook has owner docs and tests; task/error orchestration remains next refactor scope, not a claimed implemented feature |
| risk | runtime/provider/tool paths can make local retry/fail/block decisions without one auditable control/error policy; future task/subagent routing could accidentally execute side effects from status schema instead of compact built-in task tools |
| gate | no current gate requires control/error center admission before flow decisions |
| current producers | `control.center` writes basic hook/status metadata through runtime hook calls; `reason.turn` and runtime live bridge write existing lifecycle metadata; provider/tool/runtime errors can materialize shared `ErrorErr01RuntimeClassified` but not through a central error policy owner |
| missing coverage | full status repair loop, compact `task` action tool, task lifecycle persistence, task/subagent dispatch manager, error-center classifier, recovery decision owner, richer control/error watermark schema helpers, task state transitions linked to accepted action metadata |
| design doc | `docs/design/control-error-center-refactor.md` |
| priority | high — required before task/subagent orchestration refactor |
| closure path | 1) add feature-map/function-map/test-design entries for `error.center` and `task.orchestration` 2) extend `control.center` from basic stopHook to schema repair feedback and action admission 3) implement compact `task` built-in action tool 4) extend metadata center with status/action/error watermark helpers 5) route runtime/reason/provider/tool failures through error center 6) gate owner state transitions on accepted action metadata |

## 管理规则

1. 本文件只记录 **非违规欠账**。违规必须改或删。
2. 新增 gap 必须有：`feature_id`、owner、gap kind、risk、gate 状态、closure path。
3. gap 关闭后，从此文件删除，不可留"已关闭"占位符。
4. 本文件由 `docs/architecture/feature-map.md` 路由索引中 `architecture-gaps` 段引用。
