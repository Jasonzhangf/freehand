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

## Response Mainline

- initialize returns protocolVersion 1, agentCapabilities, agentInfo, and authMethods as a JSON-RPC result
- session/new returns a JSON-RPC result carrying the generated sessionId
- session/prompt returns a StopReason (end_turn/cancelled/refusal) after the turn completes
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

## Sync Status Against Mainline Call

- FreehandAgent is implemented on top of the official agent-client-protocol Rust SDK 2.0; the hand-rolled protocol/server/handler modules are removed
- daemon `acp` subcommand binds a FreehandAgent over Stdio and is proven end-to-end via initialize + session/new + session/prompt returning stopReason end_turn
- session/cancel flips the per-session cancel token; the runtime live-reason turn polls the token mid-flight and aborts with RuntimeLiveBridgeError::Cancelled, which the adapter maps to stopReason cancelled. The token is reset after each turn returns (either completing normally or aborting) so a stale cancel does not leak into the next prompt.
- capability advertisement (initialize) declares only the methods and prompt content kinds that the adapter actually handles
- session/new records the session working directory; session/prompt forwards it to LiveReasonTurnRequest.cwd
- stdout carries NDJSON JSON-RPC frames only; daemon acp returns Ok(empty) so nothing else is written
- ADP WebUI surface is untouched and remains the internal UI protocol
- generated wiki must be regenerated from docs/mainline-calls/app.acp-server.json when this truth changes
