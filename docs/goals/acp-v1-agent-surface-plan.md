# Freehand ACP v1 Agent Surface Implementation Plan (SDK Adapter)

## 1. Objective and Acceptance Criteria

**Objective**: expose Freehand as an Agent Client Protocol (ACP) v1 endpoint
(JSON-RPC 2.0 over NDJSON) by adapting the runtime live-reason turn
mainline onto the official `agent-client-protocol` Rust SDK. The hand-rolled
protocol/server/handler modules from earlier designs are removed.

**Acceptance criteria**:
- `crates/freehand-acp` implements `ConnectTo<Client>` and registers
  `initialize`, `session/new`, `session/prompt`, and `session/cancel`
  handlers through the official SDK.
- daemon `freehand-daemon acp` binds a `FreehandAgent` over `Stdio` and
  serves NDJSON JSON-RPC frames until EOF. stdout carries only JSON-RPC
  frames; stderr stays clean.
- `session/prompt` runs one turn through `run_live_reason_turn_with_hooks`
  with the per-session cancel token attached, streaming runtime
  semantic/tool/tool-result events as ACP `session/update` notifications.
  `RuntimeLiveBridgeError::Cancelled` maps to `StopReason::Cancelled`; every
  other error maps to `StopReason::Refusal`.
- `session/new` records the session working directory; `session/prompt`
  forwards it to `LiveReasonTurnRequest.cwd`.
- `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo run -p xtask -- gates check`, and
  `cargo run -p xtask -- mainlines check` are all green.
- module-registry, resource-map, function-map, mainline-call-map,
  verification-map, wiki, and design doc are all bound and consistent
  with the SDK adapter.

## 2. Scope and Boundary

In scope:
- `crates/freehand-acp` (one source file) implementing `FreehandAgent`.
- daemon `acp` subcommand binding `FreehandAgent` over `Stdio`.
- architecture documentation (module-registry, resource-map, function-map,
  mainline, verification-map, wiki, design) bound to the SDK adapter.

Out of scope:
- `authenticate`, `session/load`, `session/resume`, `session/list`,
  `session/close`, `session/delete`.
- ADP changes (the internal WebUI transport).
- any hand-rolled ACP protocol/server/handler modules.

## 3. Resource and Dependency Ownership

`acp_transport` (resource type) is owned by `app.acp-server`. Its allowed
operations are exactly `initialize`, `new_session`, `cancel_turn`,
`prompt_turn`. The adapter is allowed to depend on the SDK, on `freehand-
contracts`, on `freehand-runtime`, and on `tokio` for the blocking pool.
It must not depend on `freehand-reason`, `freehand-task`,
`freehand-provider-*`, `freehand-node`, or `freehand-ui-protocol`.

## 4. Method Surface Mapping

| ACP method | adapter mapping |
| --- | --- |
| `initialize` | respond with `InitializeResponse` advertising only the prompt capabilities the adapter handles |
| `session/new` | allocate transport-local session id and cancel token; record session cwd |
| `session/prompt` | drive one `run_live_reason_turn_with_hooks` on the tokio blocking pool with the session cwd and cancel token attached, streaming broadcast events as ACP `session/update` notifications |
| `session/cancel` | flip the per-session cancel token |

## 5. Verification Matrix

1. `cargo test -p freehand-acp` (4 unit tests).
2. `cargo run -p freehand-daemon -- acp` piped initialize + session/new +
   session/prompt returning `stopReason end_turn`, using the hermetic local
   mock provider fixture in `scripts/verify-acp-stdio.sh` so the gate does
   not depend on real provider credentials.
3. `cargo run -p freehand-daemon -- acp` piped initialize + session/new +
   session/cancel + session/prompt returning `stopReason cancelled`
   without invoking the provider.
4. workspace `cargo build`, `cargo clippy`, `cargo test`,
   `xtask gates check`, `xtask mainlines check` all green.
