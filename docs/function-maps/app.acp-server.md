# Function Map: `app.acp-server`

- feature_id: `app.acp-server`
- owner crate: `crates/freehand-acp`
- owner module: `crates/freehand-acp/src/lib.rs`
- resource map: `docs/resource-maps/core.json`
- module registry: `docs/module-registry/app.acp-server.json`
- verification map: `docs/verification-maps/app.acp-server.json`
- mainline call source: `docs/mainline-calls/app.acp-server.json`
- generated wiki: `docs/wiki/app.acp-server.md`
- owned resources: `acp_transport`
- touched resources: `master_work`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources: `acp_transport`
- touched resources: `master_work`
- resource operations:
  - `acp_transport.prompt_turn` (`acp_transport` -> `master_work`)
  - `master_work.project_acp_broadcast` (`master_work` -> `acp_transport`)
- forbidden shortcuts:
  - ACP must not depend on reason persistence, provider adapters, task truth, metadata, debug truth, or node transport.
  - ACP must not serialize control state or ACP transport state into business text.
  - ACP must not implement a second ADP frame or session persistence path.
  - The wire, framing, and transport are owned by the official agent-client-protocol Rust SDK; `freehand-acp` only adapts the ACP method surface to runtime.

## Request Mainline

`FreehandAgent::connect_to` registers the ACP v1 method handlers
(initialize, session/new, session/prompt, session/cancel) onto the official
agent-client-protocol SDK and drives the stdio transport.
`session/prompt` drives one turn through the runtime live-reason turn mainline
(`master_work`) with a per-session cancel token, streaming runtime broadcast
events back as ACP `session/update` notifications. `session/load` and
`session/resume` remain out of scope for this slice; ACP owns only the
transport-local session registry, and the runtime owns no ACP session
persistence.

## Response Mainline

ACP JSON-RPC results carry initialize capability advertisement, the generated
sessionId, and the turn stop reason (`end_turn`/`cancelled`/`refusal`).
`session/prompt` also emits streaming `session/update` notifications for
runtime reasoning, message, tool, and tool-result events. Usage events are not
projected because the runtime lacks a provider context-window projection.
Tool kind classification is delegated to the `tool.display` owner through
`freehand-runtime`'s typed port. Runtime turn execution is owned by
`master_work`; ACP only adapts the wire.

## Error Mainline

Framing, JSON-RPC, and parameter validation are enforced by the
agent-client-protocol SDK before reaching handlers. Unknown sessions return
typed invalid-params errors. Runtime turn failures surface as
`StopReason::Refusal`.

## Function Call Table

| step | symbol path | file path | responsibility | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- |
| 01 | `run` | `apps/freehand-daemon/src/main.rs` | route the ACP stdio subcommand; on completion return empty Ok so stdout carries only JSON-RPC frames | daemon main | `FreehandAgent::connect_to + Stdio` | binding bound |
| 02 | `run_turn_blocking` | `crates/freehand-acp/src/lib.rs` | drive one turn through the runtime live-reason mainline with the session cancel token on the tokio blocking pool, streaming broadcast events through hooks | prompt handler | `run_live_reason_turn_with_hooks` | binding bound |
| 03 | `FreehandAgent::connect_to` | `crates/freehand-acp/src/lib.rs` | register initialize/session/new/session/prompt handlers and session/cancel notification, then drive the ACP stdio transport via agent-client-protocol SDK | daemon acp entry | `Agent::builder + Stdio` | binding bound |
| 04 | `run_prompt_with_reset` | `crates/freehand-acp/src/lib.rs` | run the prompt through the turn runner on the blocking pool and reset the session cancel token after each turn | prompt handler | `run_turn_blocking` | binding bound |
| 05 | `AcpBroadcaster::on_broadcast` | `crates/freehand-acp/src/lib.rs` | project runtime broadcast events into ACP `session/update` notifications and stream them | `run_turn_blocking` | `project_semantic` / `project_tool_call` / `project_tool_result` / `project_usage` | binding bound |
