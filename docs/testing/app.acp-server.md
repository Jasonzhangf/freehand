# Test Design: `app.acp-server`

- feature_id: `app.acp-server`
- owner: `crates/freehand-acp`
- resource map: `docs/resource-maps/core.json`
- lifecycle: initialize -> session/new -> prompt -> cancel -> close

## Resource Operation Test Coverage

| operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `acp_transport.prompt_turn` | bound | `cargo test -p freehand-acp -- --nocapture` | `cargo test -p freehand-acp extract_text monotonic_id cancel_token -- --nocapture` | `cargo run -p freehand-daemon -- acp` piped initialize + session/new + session/prompt returning stopReason end_turn; session/cancel followed by session/prompt returns stopReason cancelled |

## White-box

- `extract_text` joins ContentBlock text blocks with newlines and skips
  non-text blocks.
- `monotonic_id` returns strictly increasing ids within one process.
- cancel token flip changes subsequent observation atomically.
- The agent-client-protocol SDK enforces JSON-RPC framing, initialize-before-
  methods, and parameter validation before reaching the adapter handlers.

## Module Black-box

- handler registration through `FreehandAgent::connect_to` exposes
  initialize/session/new/session/prompt/session/cancel.
- session registry stores the session working directory and forwards it to
  `LiveReasonTurnRequest.cwd` on each prompt.
- the prompt handler maps `RuntimeLiveBridgeError::Cancelled` to
  `StopReason::Cancelled` and every other runtime error to
  `StopReason::Refusal`.
- the cancel token is reset after every turn returns so a stale cancel
  cannot brick the session.

## Project Black-box

- `scripts/verify-acp-stdio.sh` runs the installed daemon over real
  stdin/stdout with a deterministic payload and asserts stdout carries only
  valid NDJSON JSON-RPC frames including both `cancelled` and `end_turn`
  stop reasons, and that stderr stays empty.

## Project Black-box

- daemon `acp` subcommand over real stdin/stdout returns only JSON-RPC
  frames on stdout and keeps stderr clean.
- end-to-end initialize + session/new + session/prompt returns a stop
  reason through the real runtime live-reason turn mainline with the
  configured agent.
- session/cancel then session/prompt returns stopReason cancelled without
  invoking the provider (next-prompt semantics) and mid-flight cancellation
  aborts the live turn with `RuntimeLiveBridgeError::Cancelled`, which the
  adapter surfaces as stopReason cancelled.
- After the cancelled turn returns, the cancel token is reset; the next
  prompt runs normally (stopReason end_turn) instead of being cancelled again.
  `scripts/verify-acp-stdio.sh` covers both the cancelled and end_turn
  follow-up frames plus the empty-stderr invariant.
