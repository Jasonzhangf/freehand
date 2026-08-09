# Wiki: `app.acp-server`

Generated from `docs/mainline-calls/app.acp-server.json`. Do not edit by hand.

- owner crate: `crates/freehand-acp`
- owner module: `crates/freehand-acp/src/lib.rs`
- function map: `docs/function-maps/app.acp-server.md`
- generated wiki: `docs/wiki/app.acp-server.md`
- test design: `docs/testing/app.acp-server.md`

## Resource Operation Backlinks

- acp_transport.prompt_turn

## Request Mainline

- daemon `acp` subcommand binds a FreehandAgent over Stdio and serves NDJSON JSON-RPC frames until EOF
- initialize negotiates protocol version 1 and advertises agent capabilities; no session is opened
- session/new registers a transport-local ACP session and a per-session cancel token
- session/prompt drives one turn through the runtime live-reason turn mainline using the per-session cancel token
- session/list, session/set_mode, session/load, session/resume, and session/close adapt the transport-local ACP session registry onto the runtime reason-persistence truth
- session/load and session/resume replay persisted turn history as full ACP session/update notifications

## Response Mainline

- initialize returns protocolVersion 1, agentCapabilities, agentInfo, and authMethods as a JSON-RPC result
- session/new returns a JSON-RPC result carrying the generated sessionId
- session/prompt returns a StopReason (end_turn/cancelled/refusal) after the turn completes
- session/prompt streams runtime reasoning, message, tool, tool-result, and usage events as ACP session/update notifications via the AcpBroadcaster projection
- session/load and session/resume return the standard response after replaying persisted history; session/close marks the session closed and cancels any in-flight turn
- ACP wire is owned by the official agent-client-protocol Rust SDK; FreehandAgent only adapts methods to runtime

## Error Mainline

- Framing, JSON-RPC, and parameter validation are enforced by the agent-client-protocol SDK before reaching handlers
- session/prompt for an unknown session returns a typed invalid-params error
- runtime turn failures surface as StopReason::Refusal; the SDK transports protocol errors as JSON-RPC error frames

## Shared Multi-Reference Functions


## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run` | `apps/freehand-daemon/src/main.rs` | route the ACP stdio subcommand and serve NDJSON JSON-RPC frames until stdin EOF | daemon acp command | served ACP connection | daemon main | FreehandAgent::connect_to + Stdio |  |  |  | bound |
| 02 | `run_turn_blocking` | `crates/freehand-acp/src/lib.rs` | load the selected agent config and run one live-reason turn with the session cancel token on the tokio blocking pool | ACP sessionId plus prompt text | LiveReasonTurnOutcome observed under cancel | FreehandAgent::connect_to prompt handler | run_live_reason_turn | acp_transport | master_work | acp_transport.prompt_turn | bound |
| 03 | `FreehandAgent::connect_to` | `crates/freehand-acp/src/lib.rs` | register initialize/session/new/session/prompt handlers and session/cancel notification, then drive the ACP stdio transport via agent-client-protocol SDK | ACP client transport (Stdio) | ACP v1 method surface | daemon acp entry | Agent::builder + Stdio | acp_transport | master_work | acp_transport.prompt_turn | bound |
| 04 | `run_prompt_with_reset` | `crates/freehand-acp/src/lib.rs` | run the prompt through the configured turn runner on the tokio blocking pool and reset the session cancel token after each turn so a stale cancel cannot brick the next prompt | ACP session plus prompt text plus turn runner | StopReason (end_turn/cancelled/refusal) | FreehandAgent::connect_to prompt handler | run_turn_blocking | acp_transport | master_work | acp_transport.prompt_turn | bound |
| 05 | `AcpBroadcaster::on_broadcast / project_broadcast` | `crates/freehand-acp/src/lib.rs` | project runtime broadcast events (semantic, tool, tool-result, usage) into ACP session/update notifications and stream them to the client | ReasonBroadcastEvent stream | ACP session/update notifications | run_turn_blocking | project_semantic / project_tool_call / project_tool_result / project_usage | acp_transport | master_work | acp_transport.prompt_turn | bound |
| 06 | `replay_session_history / emit_replay_turn` | `crates/freehand-acp/src/lib.rs` | replay persisted turn history for session/load and session/resume as full ACP session/update notifications from the runtime reason persistence | ACP sessionId | replayed session/update notifications | FreehandAgent::connect_to load/resume handler | ReasonPersistence::restore_turn_snapshots_for_ui | acp_transport | master_work | acp_transport.prompt_turn | bound |
| 07 | `list_sessions` | `crates/freehand-acp/src/lib.rs` | project the transport-local ACP session registry into ACP SessionInfo entries for session/list | ACP session registry | Vec<SessionInfo> | FreehandAgent::connect_to session/list handler | SessionInfo::new | acp_transport | master_work | acp_transport.prompt_turn | bound |

## Sync Status Against Mainline Call

- FreehandAgent is implemented on top of the official agent-client-protocol Rust SDK 2.0; the hand-rolled protocol/server/handler modules are removed
- daemon `acp` subcommand binds a FreehandAgent over Stdio and is proven end-to-end via initialize + session/new + session/prompt returning stopReason end_turn
- session/cancel flips the per-session cancel token; the runtime live-reason turn polls the token mid-flight and aborts with RuntimeLiveBridgeError::Cancelled, which the adapter maps to stopReason cancelled. The token is reset after each turn returns (either completing normally or aborting) so a stale cancel does not leak into the next prompt.
- capability advertisement (initialize) declares only the methods and prompt content kinds that the adapter actually handles
- session/new records the session working directory; session/prompt forwards it to LiveReasonTurnRequest.cwd
- session/prompt streams runtime reasoning/message/tool/usage events as ACP session/update notifications via AcpBroadcaster; replay (session/load and session/resume) emits full chunks from the runtime reason persistence truth and never emits usage_update during replay
- session/list projects the transport-local registry; session/set_mode and session/close are accepted with typed responses, and session/close marks the session closed and cancels any in-flight turn
- the ACP adapter depends on serde_json only for building tool-call raw_input and typed error data; the runtime owns reason persistence and live-reason turn truth
- stdout carries NDJSON JSON-RPC frames only; daemon acp returns Ok(empty) so nothing else is written
- ADP WebUI surface is untouched and remains the internal UI protocol
- generated wiki must be regenerated from docs/mainline-calls/app.acp-server.json when this truth changes
