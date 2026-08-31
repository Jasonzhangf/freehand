# Freehand v2

v2 设计与验证总入口。先从这里看，不需要进入深层目录。

## 先看这几份

1. [架构总设计](docs/v2/v2-cordis-reasoning-channel-architecture.md)
2. [插件生态总合同](docs/v2/v2-plugin-ecosystem-contract.md)
3. [MVP 与模块分块](docs/v2/v2-foundation-mvp-ui-reason-network-plan.md)
4. [UI 设计合同](docs/v2/v2-ui-design.md)
5. [UI 信息架构](docs/v2/v2-ui-plugin-information-architecture.md)
6. [UI 插件合同](docs/v2/v2-ui-plugin-contract.md)
7. [测试设计](docs/v2/v2-test-design.md)
8. [项目黑盒验证](docs/v2/v2-project-blackbox-verification.md)
9. [AppSDK 治理清单](docs/v2/v2-governance-manifest.json)

## 静态页面

直接打开：

```text
http://127.0.0.1:4174/docs/v2/prototypes/v2-ui-plugin-console/index.html
```

原型说明：[docs/v2/prototypes/README.md](docs/v2/prototypes/README.md)

当前原型包含：

- Run：当前推理与 Session Log
- Location / Topology：机器、节点、Agent、Channel
- Attention / Notifications：按重要性与时间查看通知
- Sessions / Canvas：Active、Recent、History
- Search Plugin：搜索会话、来源和分类结果
- Memory Plugin：总结、保存、加载、导出
- Settings：模型、运行时、连接、可观测性、外观、关于
- Timer dashboard：relative、absolute、interval、daily、weekly、cron
- Tools registry：schema、exposure、permission、Invoke、Detach

## 设计顺序

```text
架构
  -> 模块分块
  -> MVP 主线
  -> UI / 推理分离
  -> Channel / 网络扩展
  -> 测试与治理
  -> 原型验证
```

## 当前边界

- v1 只作为入口和行为参考，不改 v1。
- v2 使用 Cordis 作为插件生态和编排基础。
- 每个可执行、可替换或对外连接的部分都是 Cordis 插件；UI 是可独立替换的插件族。
- UI 只消费 `UiAdaptor` 的 typed projection，不拥有 runtime truth。
- Search、Memory、Timer、Tools 都是独立插件表面，不是 UI 私有逻辑。
- 当前静态页面是 review-only mock，不是生产 runtime。
- 不提交 `target/`、`generated/`、`artifacts/` 或其他构建物。

## 当前工作区

- 项目：`freehand-v2`
- AppSDK：`0.1.6`
- 设计分支：`v2`
- 当前 UI 候选分支：`codex/v2-ui-plugin-information-architecture-20260831`
