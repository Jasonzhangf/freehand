# Function Map: `provider.reason-live-bridge`

- feature_id: `provider.reason-live-bridge`
- owner crate: `crates/freehand-testkit`
- owner module: `crates/freehand-testkit/src/lib.rs`
- owner entry symbols:
  - `run_live_reason_turn`

## Request Mainline

- selected agent config enters the live bridge with one bound provider
- bridge derives provider descriptor and executor config from selected provider truth
- `reason.turn` may start multiple rounds under one logical live request when completion schema says `continue` or when schema rejection requires same-task retry
- provider semantic request is built from each round's turn-owned provider payload
- Anthropic live executor runs the HTTP/SSE request and returns provider-neutral semantic outputs for each round
- stream mode applies outputs incrementally through the executor callback path before the provider response completes
- completion schema is parsed from tagged text, validated, and either accepted, rejected with field-level feedback, or used to schedule the next round

## Response Mainline

- provider-neutral outputs are applied back into the active round through `ReasonTurnEngine::apply_provider_output`
- completed/blocked schema writes terminal truth through `ReasonTurnEngine::submit_completion`
- retry exhaustion writes failed terminal truth through `ReasonTurnEngine::fail_turn`
- bridge returns final turn truth, captured broadcast events, schema rejection ledger, and live-output summary without leaking wire DTOs

## Error Mainline

- unsupported provider type/protocol is rejected at the bridge boundary
- provider execution failures are returned explicitly
- invalid or missing completion schema is rejected with field-level feedback and retried up to 3 times
- provider terminal metadata does not become final completion truth without accepted Freehand completion schema

## Shared Multi-Reference Functions

- `build_semantic_request`
  - owner: `crates/freehand-provider-core/src/lib.rs`
  - purpose: convert turn-owned provider payload plus provider descriptor into provider-neutral request truth
  - allowed callers: runtime bridges, tests
  - related tests: provider semantic request tests, live bridge request build tests
  - why shared: keeps provider-neutral request ownership centralized

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run_live_reason_turn` | `crates/freehand-testkit/src/lib.rs` | compose config-selected provider execution with one reason turn | selected agent config + prompt + stream mode | turn truth + broadcast capture + output summary | CLI/tests | live bridge owner | bound |
| 02 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | create one turn and provider payload | session history + prompt | initialized turn record | live bridge | reason owner | bound |
| 03 | `build_semantic_request` | `crates/freehand-provider-core/src/lib.rs` | build provider-neutral request | provider descriptor + provider payload | provider semantic request | live bridge | provider semantic owner | bound |
| 04 | `AnthropicExecutor::execute_once` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one non-stream Anthropic request | provider semantic request + auth/base URL | provider semantic outputs | live bridge | anthropic executor | bound |
| 05 | `AnthropicExecutor::execute_stream` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one stream Anthropic request and accumulate outputs | provider semantic request + auth/base URL | provider semantic outputs | live bridge | anthropic executor | bound |
| 06 | `AnthropicExecutor::execute_stream_with` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one stream Anthropic request and call back per semantic batch before completion | provider semantic request + auth/base URL + callback | incremental semantic output batches + accumulated outputs | live bridge | anthropic executor | bound |
| 07 | `parse_completion_submission_block` | `crates/freehand-blocks/src/lib.rs` | parse tagged completion schema from model text | model text | typed submission or schema rejection list | live bridge | blocks owner | bound |
| 08 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | write provider-neutral outputs into turn truth | provider semantic output | updated turn record + broadcast | live bridge | reason owner | bound |
| 09 | `ReasonTurnEngine::submit_completion` | `crates/freehand-reason/src/lib.rs` | write accepted completed/blocked terminal truth | validated completion submission | terminal event | live bridge | reason owner | bound |
| 10 | `ReasonTurnEngine::fail_turn` | `crates/freehand-reason/src/lib.rs` | write failed terminal truth after schema retry exhaustion | retry-exhausted failure summary | terminal event | live bridge | reason owner | bound |

## Sync Status Against Code

- live bridge binding is implemented in `freehand-testkit`
- current live path supports Anthropic `messages` only
- stream path now applies outputs incrementally through the executor callback path before the provider response completes
- completion schema loop now exists with tagged JSON parsing, field-level rejection feedback, `continue` next-round execution, and 3-retry failed terminal closeout
- production CLI/server multi-provider loop remains pending
