# Multi-Platform UI Architecture

## Status

- **Status**: design-locked
- **Owner**: `ui.platform-architecture`
- **Design doc source**: `docs/design/multi-platform-ui-architecture.md`
- **Feature map ref**: `docs/architecture/feature-map.md` -> `ui.platform-architecture`
- **Last updated**: 2026-06-23

## 1. Design Goal

One UI protocol truth, one adaptive design system, multi-platform shells.

Freehand 的 UI 不按平台分裂语义。Web、Android、iOS 共用同一份 projection 规则、同一个 transport 协议、同一个设计令牌系统。差异只出现在：

- **navigation 模式**：desktop 多 panel 平铺 ↔ mobile 单列堆叠/底部导航
- **系统集成**：推送/后台/文件选择 通过 native bridge 暴露
- **渲染壳**：Android/iOS v1 使用 WebView 内嵌自适应 WebUI，不做原生重写

## 2. Platform Strategy

| Platform | Shell | UI source | Transport | Version |
|----------|-------|-----------|-----------|---------|
| Desktop Web | HTML/CSS responsive grid | `apps/freehand-server` serves WebUI assets | HTTP query + SSE subscribe | v1 exists, needs responsive upgrade |
| Mobile Web | Same HTML/CSS, stacked nav | Same `freehand-server` | Same transport + touch adaptations | v1 pending |
| Android | WebView wrapper + native bridge | Same WebUI served by `freehand-server` | WebView loads same origin + JS Bridge | v1 pending |
| iOS | WebView wrapper + native bridge | Same WebUI | Same as Android | v2 |

### Android v1 不做原生壳的理由

1. **Finger repo 无原生 Android 实现可复用**
2. **Freehand 已有 WebUI** 且 protocol 层纯 HTTP+SSE，WebView 可直接消费
3. **降低维护成本**：一份 UI truth，不需要 Android/iOS 两套独立实现
4. **渐进路径**：v1 WebView 验证多平台可行性，v2 如有性能瓶颈再做原生壳

## 3. Information Architecture (Shared)

所有平台共用以下信息骨架：

```
┌─────────────────────────────────────────────────────┐
│  Rail (常驻)     │  Conversation (主区)  │  Inspector │
│                   │                       │            │
│  agent 列表      │  对话流                │  debug     │
│  会话列表        │  turn 卡片             │  工具面板   │
│  设置入口        │  输入框               │  usage     │
│  slave agent    │  中断/暂停             │  provider  │
│  状态指示器       │                       │  元数据     │
└───────────────────┴───────────────────────┴────────────┘
```

### 三层定义

| Layer | Purpose | Desktop | Mobile | Android |
|-------|---------|---------|--------|---------|
| **Rail** | 常驻 agent/session 切换 + 全局入口 | 左侧窄栏，常驻展开 | 底部 tab bar 或顶部 hamburger 菜单 | 同 mobile web |
| **Conversation** | 主对话区域：消息流 + 输入 | 中间主区，占据大部分宽度 | 全宽单列，消息列表 + 底部固定输入 | 同 mobile web |
| **Inspector** | 调试/工具/usage/元数据详情 | 右侧可折叠面板 | 底部 sheet 或滑动抽屉 | 同 mobile web |

## 4. Navigation Model

### Desktop Web (≥1180px)

```
┌─────┬─────────────────────┬──────────────┐
│     │                     │              │
│ 60  │  Conversation       │  Inspector   │
│ px  │  (消息流 + 输入)    │  (可折叠)    │
│     │                     │              │
│     ├─────────────────────┤              │
│     │  Bottom Panel       │              │
│     │  (slave status/     │              │
│     │   progress)         │              │
└─────┴─────────────────────┴──────────────┘
```

- Rail: 常驻左侧（60px 窄条，hover/click 展开完整 sidebar）
- Conversation: 自适应宽度，消息流 + 底部输入框
- Inspector: 右侧面板，可折叠，默认展开时 360-520px
- Bottom Panel: slave 状态/进度条，可折叠

### Tablet (880-1180px)

- Rail 自动收起为 icon bar
- Inspector 转为底部抽屉（点击展开）
- Conversation 占满主体
- Bottom panel 保持但更矮

### Mobile Web / Android WebView (<880px)

```
┌──────────────────────────────────┐
│  Top Bar: agent 名 + slave 状态  │
├──────────────────────────────────┤
│                                  │
│  Conversation (消息流)            │
│                                  │
├──────────────────────────────────┤
│  Input Bar (固定底部)             │
├──────────────────────────────────┤
│  Bottom Nav:                     │
│  [Chat] [Agents] [Settings]      │
└──────────────────────────────────┘
```

- **无 Rail**：底部 tab bar 做导航
- **无常驻 Inspector**：通过消息卡片上的"详情"按钮展开底部 sheet
- **输入框固定底部**：消息流在输入框上方滚动
- **Slave agent 状态**：top bar 小圆点 + 下拉 sheet

## 5. Screen Model

### 5.1 主屏幕 (Chat)

所有平台必须渲染的组件：

| Component | Desktop | Mobile | Source |
|-----------|---------|--------|--------|
| MessageList | 中间主区 | 全宽滚动 | `ui.protocol` projection |
| TurnCard | 消息列表中的块 | 同 desktop | `ui.protocol` projection |
| ToolCallCard | 工具调用卡片 | 可折叠卡片 | `ui.protocol` projection |
| InputBar | 底部固定 | 底部固定（safe area） | `ui.protocol` command ingress |
| SlaveStatus | bottom panel | top bar badge | `ui.protocol` node status |
| InterruptButton | input bar 旁 | 同 desktop | `ui.protocol` command ingress |

### 5.2 设置屏幕

| Component | Desktop | Mobile |
|-----------|---------|--------|
| Provider config | inspector 面板 或 侧栏 tab | 全屏设置页（stack nav） |
| Agent config | inspector 面板 | 全屏设置页 |
| Theme toggle | top bar 或 inspector | 设置页 |
| Debug toggle | inspector | 设置页 |

### 5.3 Slave Agent 子屏幕

| Component | Desktop | Mobile |
|-----------|---------|--------|
| Slave turn list | bottom panel 内嵌 | 全屏子页面 |
| Slave turn detail | inspector 或 对话区内嵌卡片 | 全屏子页面 |

## 6. Design System Token Architecture

### 6.1 层次

```
design tokens (CSS custom properties)
  └── theme modules (light / dark)
       └── component tokens (per-component overrides)
            └── platform adaptations (mobile touch target, safe area)
```

### 6.2 当前 WebUI 已有的 CSS 变量（webui.css + theme.css）

现有 `theme.css` 已有 light/dark 切换能力。需要扩展为完整的 token 系统：

```css
/* Core tokens */
:root {
  --color-bg-primary: #fafafa;
  --color-bg-secondary: #f0f0f0;
  --color-text-primary: #1a1a1a;
  --color-text-secondary: #666;
  --color-accent: #2563eb;
  --color-line: #e5e5e5;
  --color-success: #22c55e;
  --color-error: #ef4444;
  --color-tool-call: #8b5cf6;    /* 工具调用卡片边框色 */
  --color-tool-success: #22c55e;
  --color-tool-error: #ef4444;
  --color-assistant-card: #f8fafc;
  --color-user-card: #eff6ff;

  /* Layout tokens */
  --rail-width: 60px;
  --sidebar-width: 320px;
  --inspector-width: 400px;
  --inspector-collapsed-width: 44px;
  --bottom-panel-height: 280px;
  --bottom-panel-collapsed-height: 44px;
  --topbar-height: 48px;

  /* Typography */
  --font-mono: 'SF Mono', 'Cascadia Code', 'Fira Code', monospace;
  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  --font-size-xs: 11px;
  --font-size-sm: 13px;
  --font-size-base: 15px;
  --font-size-lg: 18px;

  /* Spacing */
  --space-xs: 4px;
  --space-sm: 8px;
  --space-md: 16px;
  --space-lg: 24px;
  --space-xl: 32px;

  /* Border radius */
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;

  /* Z-index layers */
  --z-rail: 100;
  --z-inspector: 200;
  --z-sheet: 300;
  --z-modal: 400;
  --z-toast: 500;
}

/* Dark theme overrides */
body.theme-dark {
  --color-bg-primary: #0f172a;
  --color-bg-secondary: #1e293b;
  --color-text-primary: #e2e8f0;
  --color-text-secondary: #94a3b8;
  --color-accent: #3b82f6;
  --color-line: #334155;
  --color-assistant-card: #1e293b;
  --color-user-card: #1e3a5f;
  --color-tool-call: #a78bfa;
}
```

### 6.3 TurnCard 渲染契约

```html
<!-- Assistant turn -->
<div class="turn-card turn-card--assistant">
  <div class="turn-card__header" style="border-left: 3px solid var(--color-accent)">
    <span class="turn-card__role">assistant</span>
    <span class="turn-card__status turn-card__status--success">completed</span>
  </div>
  <div class="turn-card__body">
    <!-- Text content -->
    <div class="turn-text">...</div>
    <!-- Tool calls -->
    <div class="tool-call tool-call--success" style="border-left: 3px solid var(--color-tool-call)">
      <span class="tool-call__name">edit_file</span>
      <pre class="tool-call__preview">...</pre>
    </div>
    <!-- Tool results -->
    <div class="tool-result tool-result--error" style="border-left: 3px solid var(--color-tool-error)">
      <span class="tool-result__summary">File not found</span>
    </div>
  </div>
  <!-- Collapsible detail section -->
  <details class="turn-card__details">
    <summary>Details</summary>
    <!-- debug/usage/reasoning -->
  </details>
</div>

<!-- User turn -->
<div class="turn-card turn-card--user">
  <div class="turn-card__header" style="border-left: 3px solid var(--color-user-card)">
    <span class="turn-card__role">user</span>
  </div>
  <div class="turn-card__body">
    <div class="turn-text">用户输入文本</div>
  </div>
</div>
```

关键规则：

- 每个 turn card 左侧 3px 彩色 border 表示角色/类型
- 工具调用有独立彩色边框 + 状态色（success/failed/running）
- 详情的折叠块 `details/summary` 可展开 debug/usage/reasoning
- 终端状态（completed/failed/blocked/interrupted/cancelled）在 header 右侧显示
- Mobile 上 tool-call 默认折叠，点击展开

## 7. Transport Model (Unchanged)

全平台复用现有 transport：

| Operation | Method | Endpoint | Response |
|-----------|--------|----------|----------|
| Submit command | POST | `/api/command` | dispatch receipt |
| Query turn | GET | `/api/query/turn/:turn_id` | snapshot |
| Query conversation | GET | `/api/query/conversation/:session_id` | public conversation |
| Query checkpoint | GET | `/api/query/checkpoint/:session_id` | checkpoint summary |
| Subscribe turn | GET | `/api/subscribe/turn/:turn_id` | SSE stream |
| Subscribe latest | GET | `/api/subscribe/latest` | SSE stream |
| Subscribe node | GET | `/api/subscribe/node` | SSE stream |
| Query debug | GET | `/api/query/debug/:turn_id` | debug snapshot |
| Subscribe debug | GET | `/api/subscribe/debug/:turn_id` | SSE debug stream |

Android/iOS WebView 通过 HTTP client + EventSource polyfill 复用同样接口。

## 8. Android WebView Bridge (v1)

### 8.1 Bridge API

```typescript
// Native → Web (通过 WebView.evaluateJavascript)
interface NativeToWeb {
  // 推送通知
  onTurnComplete(turnId: string, status: string): void;
  onSlaveStatusChange(agentId: string, status: string): void;
  // 后台会话
  onResumeSession(sessionId: string): void;
  // 文件选择结果
  onFileSelected(path: string): void;
}

// Web → Native (通过 @JavascriptInterface)
interface WebToNative {
  // 文件选择
  pickFile(mimeTypes: string[]): Promise<string>;
  // 通知权限
  requestNotificationPermission(): Promise<boolean>;
  // 分享
  shareText(text: string): void;
  // 系统主题检测
  getSystemTheme(): 'light' | 'dark' | 'system';
}
```

### 8.2 Android WebView 配置

- WebView 加载 `http://localhost:<port>/`（freehand-server 监听 localhost）
- `JavaScriptEnabled = true`
- `DomStorageEnabled = true`
- 启用 `@JavascriptInterface` bridge
- 禁用缩放
- 匹配系统 safe area（状态栏、导航栏）
- 后台连接保持：WebView 切后台时不销毁，只暂停 SSE 连接

## 9. Current WebUI → Multi-Platform Migration Path

### Step 1: 扩展 CSS 变量（无功能变更）

- 从 webui.css 提取已有样式为 token-based CSS custom properties
- 添加 `theme.css` 作为 token 定义层
- 验证 light/dark 双主题渲染一致

### Step 2: TurnCard 渲染标准化（视觉变更）

- 当前 `renderMessages()` 在 webui.js 中生成 DOM
- 改为按 TurnCard 契约统一渲染（彩色边框、工具卡片独立块、折叠详情）
- 验证 desktop + mobile 响应式表现

### Step 3: 响应式 Shell 重构（布局变更）

- 当前 app-shell 是 `grid-template-columns: 60px 300px minmax(0, 1fr)`
- 改为三层语义布局：rail / conversation / inspector
- 1180px / 880px 两个断点保留
- 添加 <880px 的底部导航模式

### Step 4: Android WebView 包装

- 新建 `apps/freehand-android` 目录
- Android 项目骨架（build.gradle, AndroidManifest.xml）
- WebView 壳 + JS Bridge 接口
- 验证 SSE 在 WebView 中的行为（EventSource polyfill 或 native SSE client）

### Step 5: iOS WKWebView 包装 (v2)

## 10. Platform-Specific Adaptations (Non-Breaking)

| Feature | Desktop | Mobile Web | Android WebView |
|---------|---------|------------|-----------------|
| File attachment | File input | File input | WebView file chooser → native picker |
| Push notification | No | No | Web → Native bridge → Android Notification |
| Background keepalive | Tab keeps alive | Tab may suspend | WebView not destroyed, SSE reconnect |
| System theme | Manual toggle | `prefers-color-scheme` | Bridge `getSystemTheme()` |
| Safe area | `env(safe-area-inset-*)` | `env(safe-area-inset-*)` | WebView safe area CSS |
| Touch targets | 32px min | 44px min (WCAG) | 44px min |
| Keyboard handling | Default | `visualViewport` API | `window.ResizeObserver` + input scroll |

## 11. Test Design

### Module Black-Box (ui.platform-architecture)

| Test | Target | Verification |
|------|--------|-------------|
| CSS token override smoke | theme.css | light/dark toggle changes all token vars |
| Responsive breakpoint smoke | webui.css | 1180px / 880px / <880px layout matches expected |
| TurnCard render contract | webui.js | TurnCard DOM structure matches spec |
| Mobile nav model | webui.js | <880px shows bottom nav, no rail |

### Project Black-Box

| Test | Target | Verification |
|------|--------|-------------|
| WebUI loads on mobile viewport | freehand-server | 375px viewport: layout stacks, input visible |
| SSE subscribe on mobile | freehand-server | Mobile SSE subscribe delivers same events |
| Android WebView loads page | freehand-android | WebView renders WebUI, JS Bridge available |
| Command ingress on mobile | freehand-server | POST command returns dispatch receipt |

## 12. Non-Goals (v1)

- 不重写 WebUI 为 React/Vue/Svelte
- 不做 Android 原生 UI（v1 走 WebView）
- 不做 iOS 原生 UI（v2）
- 不改变 transport 协议
- 不改变 `freehand-ui-protocol` 的 projection 规则
- 不做离线模式
- 不做 PWA

## 13. File Map

```
apps/freehand-server/
  assets/
    theme.css          ← 设计令牌层（扩展为完整 token 系统）
    webui.css           ← 组件样式 + 响应式布局
    webui.js            ← 组件渲染 + transport + 交互
    theme.js            ← 主题切换逻辑

apps/freehand-android/  (new, v1)
  app/
    src/main/
      java/com/freehand/
        WebViewActivity.kt
        FreehandBridge.kt
      res/
        layout/
        values/
  build.gradle.kts
  settings.gradle.kts

docs/design/
  multi-platform-ui-architecture.md  ← 本文
```

## 14. Open Questions (Locked for v1)

| Question | Decision |
|----------|----------|
| Android 是否做原生壳？ | v1 不做，WebView wrapper |
| 是否重写 WebUI 为框架？ | 不重写，保持纯 HTML/CSS/JS |
| 是否支持离线模式？ | v1 不支持 |
| 是否支持 PWA？ | v1 不支持 |
| 是否改变 transport？ | 不改变 |
| 是否改变 projection 规则？ | 不改变 |
