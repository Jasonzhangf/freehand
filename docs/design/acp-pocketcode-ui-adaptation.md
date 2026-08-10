# ACP ↔ pocketcode UI 适配设计

## Purpose

Freehand exposes an ACP v1 **agent/server** face. pocketcode is an ACP
**client** (手机 UI) that spawns `opencode acp` and drives it via the
`@agentclientprotocol/sdk@0.21.0` `ClientSideConnection`. To let pocketcode
render a Freehand agent, the Freehand ACP server must match the wire structure
pocketcode's UI consumes. This document defines that Freehand-side adaptation.
pocketcode code is NOT modified; the two sides are later integrated and
interoperability-tested.

## Roles (no change)

```text
pocketcode app ──wire──▶ pocketcode daemon (ACP client, ClientSideConnection)
                                    │  stdio NDJSON
                                    ▼
                            Freehand acp (ACP server, FreehandAgent)
```

Freehand stays the ACP **agent**. pocketcode stays the ACP **client**.

## pocketcode client behavior (source-of-truth from ~/code/pocketcode)

Handshake:
- `initialize({ protocolVersion:1, clientCapabilities:{ elicitation:{form:{},url:{}} } })`
  → expects `InitializeResponse.agentInfo.version` + `agentCapabilities`.
- `authenticate({ methodId:"opencode-login" })` expects `{}`. reasonix skips
  (authMethodId=null). Freehand has no `opencode-login`; it should either skip
  authentication (capability does not advertise auth) or accept and no-op.

Method whitelist driven by pocketcode daemon:
`newSession / loadSession / listSessions / resumeSession / closeSession /
setSessionConfigOption / setSessionMode / setSessionModel /
forkSession(extMethod "session/fork") / prompt / cancel`.

Event face (display model): `session/update` notification with `sessionUpdate`:
- `user_message_chunk` (replay echo + turn anchor; ContentChunk, messageId)
- `agent_message_chunk` (streaming delta, messageId-grouped append)
- `agent_thought_chunk` (reasoning delta, separate messageId group)
- `tool_call` (pending) → `tool_call_update` (in_progress/completed/failed)
- `available_commands_update`, `usage_update`, `session_info_update`

Live-stream semantics: during `prompt`, agent streams session/update deltas;
deltas of one message share one `messageId` (UUID; change = new message);
`usage_update` seals the current turn. Replay (load/resume) emits full
`user_message_chunk` echo + one full chunk per message; replay emits no
`usage_update`.

Agent-originated requests: `requestPermission` (pocketcode MVP auto-cancels),
`unstable_createElicitation`.

## Freehand ACP current state

- Implements: `initialize`, `session/new`, `session/prompt`, `session/cancel`.
- `session/prompt` blocks and returns the complete turn outcome; no streaming
  `session/update`.
- Session registry is an in-process `HashMap`; no persistence → no
  load/resume/list.
- No `requestPermission`/elicitation handling.
- No thought/message/tool/usage projection to ACP.

## Adaptation gaps (Freehand side to add)

### Gap 1 — streaming `session/update` projection (P0, unblocks live UI)

Freehand runtime already exposes `run_live_reason_turn_with_hooks` taking
`FB: FnMut(&ReasonBroadcastEvent)`. Map each event to ACP session/update:

| ReasonBroadcastEvent | ACP sessionUpdate |
| --- | --- |
| `Semantic(kind=Reasoning)` | `agent_thought_chunk` (messageId = stable per reasoning block) |
| `Semantic(kind=Text)` | `agent_message_chunk` (messageId = stable per message block) |
| `Tool(ReasonReq04ToolCall)` | `tool_call` (pending/in_progress, toolCallId, kind, title, rawInput) |
| `ToolResult(ReasonReq05ToolResultReentry)` | `tool_call_update` (completed/failed, output/rawOutput) |
| `Usage(ReasonResp02UsageEvent)` | `usage_update` (used/size, seal turn) |
| `Terminal(ReasonResp03TerminalEvent)` | terminal: seal current turn; map status to StopReason |

The adapter emits these as `session/update` notifications through the SDK
before returning the `PromptResponse`. Live deltas for one assistant message
share one `messageId` (UUID); tool call deltas key on `toolCallId`.

### Gap 2 — session persistence + load/list/resume/close (P1, unblocks history)

Upgrade the in-process `HashMap` registry to a persistent store keyed by
sessionId, recording: cwd, created/updated timestamps, and a turn transcript
(the projected user/assistant/tool events) so `session/load` and
`session/resume` can replay `user_message_chunk` + full per-message chunks.
Implement:
- `session/list` → return known sessions (id, cwd, title, updatedAt)
- `session/load` → replay full transcript as `user_message_chunk` + chunks
- `session/resume` → same as load (Freehand sessions are single-continuation)
- `session/close` → mark session closed / evict from active set

### Gap 3 — handshake parity (P0)

- `initialize`: keep advertising only genuinely supported capabilities; add
  `session/list`/`session/load`/`session/resume` capability markers when Gap 2 lands.
- `authenticate`: no-op accept (Freehand has no `opencode-login`); do not
  advertise auth-required in capabilities.

### Gap 4 — optional (P2)

- `setSessionConfigOption` / `setSessionMode` / `setSessionModel`: accept and
  store per-session (no-op or thin pass-through), so pocketcode's composer
  controls do not error.
- `available_commands_update` / `session_info_update`: emit on demand for
  slash-command hints and session title.
- `requestPermission` / `unstable_createElicitation`: decide policy (pocketcode
  auto-cancels; Freehand may reply cancelled or defer to app).

## Non-goals (this slice)

- No ADP (internal WebUI) changes.
- No pocketcode source modification.
- No socket transport yet (stdio first; socket is a later extension).
- No true per-request streaming push from runtime threads; streaming projection
  is driven by the `run_live_reason_turn_with_hooks` callback during the turn,
  which already yields events incrementally.

## Verification matrix (integration later)

1. `cargo build --workspace`, `cargo clippy -D warnings`, `cargo test -p freehand-acp`,
   `xtask gates check`, `xtask mainlines check` green.
2. Freehand `acp` over stdio; pocketcode daemon spawns Freehand instead of
   `opencode acp` (via `spawnArgs`/`opencodeBinary` override) → initialize +
   authenticate + newSession + prompt → app renders assistant text/thought/tool
   live, usage seals turn.
3. loadSession/listSessions/resumeSession after restart → history replays as
   user_message_chunk + full chunks.
4. cancel mid-turn → prompt returns `cancelled`, following prompt works.
