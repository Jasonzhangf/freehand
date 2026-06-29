# WebUI Layered Controls And Attachment Plan

## Goal

把 Freehand WebUI 收口成“分层扩展”的第一版：ADP 和 `ui.protocol` 保持稳定，WebUI 在其上增加可扩展的控制层、会话管理、附件生命周期和更清晰的语义渲染；附件要按 session 保存，历史只保留占位符，成功发送后清空草稿，失败则保留以便重试。

## Related Design Truth

- `docs/design/webui-layered-controls-and-attachments.md`
- `docs/goals/webui-session-transcript-alignment-plan.md`
- `docs/design/ui-protocol-design.md`
- `docs/design/multi-platform-ui-architecture.md`

## Acceptance

目标只有在以下条件全部满足时才算完成：

1. WebUI 的底层控制面仍然通过 ADP / `ui.protocol` 工作，没有新建平行真源。
2. 会话管理存在且默认渲染的是当前选中 session，不会刷新后丢历史或回退到全局最新 turn。
3. 附件按 session 保存，但 transcript 里只显示占位符，不回放二进制内容。
4. 发送成功后草稿附件被清理；发送失败后草稿附件保留，可直接重试。
5. 输入框下方的控制区可扩展，至少包含 session、附件、刷新、模型、预览、slash command、快捷键等入口。
6. 工具、等待、错误和完成状态仍然按语义渲染，不回退成 raw term 透传。
7. 真实页面操作 + 截图 + ADP 证据都要齐，不能只靠静态测试。

## Scope

### In Scope

- WebUI 分层布局
- session rail / selected session persistence
- attachment draft lifecycle
- placeholder-based transcript rendering
- low-noise tool card rendering
- slash commands and keyboard shortcuts
- ADP/default control path wiring
- real browser verification and screenshot evidence

### Out Of Scope

- ADP wire framing redesign
- reason/provider core semantics redesign
- Android / CLI 视觉重写
- fallback / 双路径 / 静默降级
- 把附件 payload 混进历史 transcript truth

## Design Principles

1. Protocol first, presentation second.
2. UI can expand, core capability must not drift.
3. Session truth is authoritative; attachments are session-scoped and draft-scoped.
4. History uses placeholders, not payload replay.
5. Success clears draft attachments; failure preserves them.
6. Control surface is explicit and compact.
7. UI must render waiting, error, and retry states visibly.

## Technical Plan

### 1. Lock the layering contract

- write the durable design truth in `docs/design/webui-layered-controls-and-attachments.md`
- keep `ui.protocol` as read-only projection truth
- keep `app.webui-smoke` as the UI control/render owner

### 2. Add session-scoped attachment state

- draft attachments belong to the selected session
- outgoing send payload uses the current draft set only
- transcript stores placeholder metadata only
- successful send clears the draft attachment set
- failed send keeps the draft attachment set unchanged

### 3. Render attachments as placeholders

- show compact chips or rows in the conversation timeline
- show filename / kind / size / count, not binary content
- use the same placeholder representation after reload

### 4. Expand the composer control strip

- session selection / `/new`
- refresh / re-query
- model selector
- file/image/video attach
- file preview
- slash commands
- shortcut hints
- cancel / retry

### 5. Keep tool display semantic

- tool display must continue to come from `tool.display`
- read/list/file mutation/plan/shell/generic categories must keep their current parser owner
- UI only renders projection data

### 6. Prove with live evidence

- drive the actual WebUI page
- attach files/images
- submit success and failure samples
- verify draft-clear / retry-retain behavior
- save screenshots as proof

## Verification Matrix

### White-box

- targeted protocol / WebUI rendering tests for session and attachment state
- tool display parser tests remain green
- session-selection behavior tests remain green

### Module black-box

- WebUI session persistence smoke
- WebUI attachment clear-on-success smoke
- WebUI attachment retain-on-failure smoke
- WebUI placeholder transcript render smoke
- WebUI shortcut / slash-command smoke

### Project black-box

- real browser session with multiple turns
- real browser attachment send and retry
- real browser reload and restoration
- screenshot evidence for each lifecycle stage

### Workspace gates

- `cargo fmt --check`
- `cargo test --workspace`
- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- `make ci`

## Implementation Steps

1. Confirm the layering doc is the contract.
2. Build the control strip and attachment state model.
3. Make transcript history placeholder-based.
4. Keep success/failure send semantics distinct for draft attachments.
5. Verify in the real WebUI with screenshots.
6. Sync test design / function maps / memory if implementation truth changes.

## Completion Signal

Use `/goal` until all of these are true:

- session-based UI rendering is stable after reload
- attachment drafts behave correctly on success and failure
- the control strip is layered and extensible
- the transcript stays semantic and low-noise
- verification includes live browser evidence
