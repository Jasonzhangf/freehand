# Test Design: `app.acp-server`

- feature_id: `app.acp-server`
- owner: `crates/freehand-acp`
- resource map: `docs/resource-maps/core.json`
- lifecycle: initialize -> session/new -> prompt -> cancel

## Resource Operation Test Coverage

| operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `acp_transport.prompt_turn` | bound | `cargo test -p freehand-acp -- --nocapture` | `cargo test -p freehand-acp extract_text monotonic_id cancel_token -- --nocapture` | `cargo run -p freehand-daemon -- acp` piped initialize + session/new + session/prompt returning stopReason end_turn; session/cancel followed by session/prompt returns stopReason cancelled |
| `master_work.project_acp_broadcast` | bound | `cargo test -p freehand-acp project_tool_result tool_kind_for -- --nocapture` (deterministic tool-result output content and tool-kind projection) | `cargo test -p freehand-acp -- --nocapture` | `bash scripts/verify-acp-stdio.sh` (wire purity + protocol/session/stopReason on real stdio; streaming projection is white-box covered because real prompts are not deterministic tool fixtures) |

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
  stdin/stdout with a deterministic payload and a hermetic local
  Anthropic-compatible mock provider in an isolated temporary HOME. It
  asserts stdout carries only valid NDJSON JSON-RPC frames including both
  `cancelled` and `end_turn` stop reasons, and that stderr stays empty.
- The gate does not read or write the developer's real `.freehand` runtime
  home and does not depend on real provider credentials or upstream provider
  availability.

## Project Black-box

- daemon `acp` subcommand over real stdin/stdout returns only JSON-RPC
  frames on stdout and keeps stderr clean.
- end-to-end initialize + session/new + session/prompt returns a stop
  reason through the real runtime live-reason turn mainline using the
  isolated mock provider configuration.
- session/cancel then session/prompt returns stopReason cancelled without
  invoking the provider (next-prompt semantics) and mid-flight cancellation
  aborts the live turn with `RuntimeLiveBridgeError::Cancelled`, which the
  adapter surfaces as stopReason cancelled.
- After the cancelled turn returns, the cancel token is reset; the next
  prompt runs normally (stopReason end_turn) instead of being cancelled again.
  `scripts/verify-acp-stdio.sh` covers both the cancelled and end_turn
  follow-up frames plus the empty-stderr invariant.

## Known Test Gap

- `AcpBroadcaster.send_failed -> StopReason::Refusal` (streaming notification
  send failure) has no independent unit red test: the SDK `ConnectionTo`
  requires a full transport stack to construct, so a deterministic failing
  client is not feasible in a unit harness. The path is covered by review
  analysis and the e2e empty-stderr/wire-purity assertions. Add a red test
  when the SDK exposes a testable notification sink.
- ACP tool-call `ToolKind` projection follows the `tool.display` owner
  classifier (`classify_tool_display_kind`), so kinds that the old ACP-local
  classifier mapped to richer kinds (Delete/Move/Fetch/Think/SwitchMode) now
  collapse to the owner's display class (e.g. `Read`/`Other`). This is the
  intended de-duplicated semantics, not a regression; the e2e/white-box tests
  assert the owner-driven classification via `tool_kind_for_uses_display_owner_classification`.
