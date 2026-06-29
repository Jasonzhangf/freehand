# WebUI Layered Controls And Attachment Contract

## Status

- draft for review
- intended as the first implementation contract for layered WebUI expansion
- does not change ADP core framing or reasoning/provider truth ownership

## Purpose

Freehand 的 WebUI 不是把 ADP 包装成一个大页面，而是在 ADP 之上逐层扩展 UI 能力。

核心原则只有两条：

1. ADP / `ui.protocol` 继续作为稳定真源，不因为 UI 变复杂而改变核心能力。
2. UI 可以不断增加控件、布局、渲染语义和辅助操作，但这些都属于表现层或控制层，不是新的推理真源。

This document locks the layering contract for:

- session management
- attachment lifecycle
- control surface expansion
- low-noise transcript rendering
- session-scoped placeholder history

## Layer Model

### 1. Transport Layer

Stable truth:

- ADP command/query/subscribe frames
- HTTP/SSE compatibility routes
- fixed daemon port and background service behavior

This layer is not expanded for visual needs. It only carries protocol-owned frames.

### 2. Protocol Projection Layer

Stable truth:

- `ui.protocol` owns query/projection semantics
- projections stay read-only
- session/query/subscription boundaries remain explicit
- tool semantic display comes from `tool.display`, not from WebUI parsing

This layer may gain richer projection fields only when the protocol owner decides the shape.

### 3. Control Layer

This is the layer we expand first.

Controls are input-layer affordances over ADP/protocol truth, including:

- session list and session switching
- `/new` session creation
- refresh / re-query
- model selection
- attachment upload
- image/video/file attachment preview
- cancel / stop / retry
- slash commands
- keyboard shortcuts
- debug and checkpoint entry points that remain read-only or explicitly routed

Control layer rules:

- controls may submit commands or query state
- controls may not directly mutate reason truth
- controls may not invent a second transport path
- controls may not classify tools from raw args/results

### 4. Presentation Layer

This layer controls how protocol truth is rendered:

- conversation stream
- tool cards
- waiting animations
- timers and elapsed state
- failure status
- session rail density
- attachment chips / placeholder rows
- compact summary vs expanded detail

Presentation may change more freely than protocol truth, but it must remain a pure consumer of protocol state and control-layer state.

## Session And Attachment Contract

### Session Truth

Each session is the durable container for:

- selected session id
- transcript history
- attachment metadata
- control-draft state when the composer is open

The transcript history is not a raw payload dump. It is a rendered record of turns plus placeholders.

### Attachment Truth

Attachments include:

- files
- images
- videos
- other binary media later if needed

Attachment rules:

- attachments are scoped to a session
- attachments are visible in the current send only
- transcript history stores placeholders / metadata, not binary payload blobs
- the actual payload is only attached to the outbound send batch
- successful send clears the draft attachment batch from the composer
- failed send keeps the draft attachment batch so retry resends the same payload set
- retry must not silently drop or duplicate attachments
- reloading a session must restore placeholder history, not rehydrate old binary payloads into the composer

### Placeholder Format

The public transcript may show attachment placeholders such as:

- file name
- media kind
- size
- count
- upload/attach state

It must not expose:

- raw binary content
- full internal provider payloads
- duplicated attachment bodies in multiple turns

### Failure Semantics

Attachment lifecycle must be explicit:

- permission failure is visible before send
- upload failure is visible in the attachment control row
- send dispatch failure preserves the draft batch
- runtime failure is not converted into a successful attachment send

No fallback is allowed:

- do not silently strip attachments
- do not silently switch to a different transport
- do not silently fold attachment failure into a success state

## UI Control Surface

The input area below the composer should host compact controls, not a second page.

Recommended first-layout control slots:

- session rail toggle / selected session indicator
- new session
- refresh
- model selector
- file attach
- image attach
- video attach
- file preview
- slash command launcher
- shortcut hint / help
- cancel / retry

The layout may grow more controls later, but the control layer should remain horizontally and visually compact, with only rare actions moved into secondary menus.

## Rendering Rules

### Conversation

- one visible assistant card per logical execution cycle
- user prompt remains visible
- assistant text is semantic, not raw transport dump
- raw completion schema blocks do not pollute the primary timeline
- tool cards are one semantic item per `tool_call_id`
- same tool call updates the same card instead of duplicating it

### Tool Display

- read/list tools show action + target + target summary
- file mutation tools show diff / change summary
- plan tools show a structured plan card
- shell tools show command summary plus result summary
- generic or unknown tools show parameter summary plus result summary
- waiting state uses animation and elapsed time
- execution result updates the same card rather than creating a new card

### Session Rail

- show only key session information
- keep the list compact
- preserve selected-session state across reload
- default view should remain anchored to the selected session, not global latest turn

### Transcript History

- history is chronological
- same logical cycle can merge display-wise, but not by destroying truth
- placeholder entries remain visible where attachments were sent
- errors must update state, not silently disappear

## Owner Split

### `ui.protocol`

Owns:

- query/subscription/projection truth
- public conversation projection
- tool activity projection
- explicit session selection in submit/query semantics

### `app.webui-smoke`

Owns:

- DOM controls
- session rail rendering
- attachment chips / preview controls
- slash commands and keyboard shortcuts
- control-to-ADP wiring

### `tool.display`

Owns:

- tool semantic parsing and classification
- action/target/parameter/result/diff projection

### `reason.persistence`

Owns:

- authoritative persisted session history
- restart recovery truth

The WebUI may render placeholder history over persisted truth, but it does not own the recovery model.

## Out Of Scope

- changing ADP framing to fit UI needs
- adding fallback paths for missing attachment or session truth
- letting WebUI parse raw provider payloads as a source of truth
- duplicating tool semantics inside JavaScript
- rewriting reason/provider runtime logic unless later protocol changes require it

## Verification Targets

Once implemented, this contract should be proven with:

- real browser screenshots for session switch, attachment upload, retry, and success-clear behavior
- failure screenshots for permission failure, upload failure, and send failure retention
- live ADP evidence that the selected session is the active render target
- tests proving placeholder history survives reload while draft attachments are cleared only on successful send
- tests proving the same draft attachment batch is preserved across send failure and retry

## Update Triggers

Update this doc when:

- the control surface changes
- attachment lifecycle changes
- placeholder history rules change
- session selection semantics change
- tool display semantics change
- ADP framing changes
- the layered UI architecture gets a new owner boundary
