# Function Map: `provider.semantic`

- feature_id: `provider.semantic`
- owner crate: `crates/freehand-provider-core`
- owner module: `crates/freehand-provider-core/src/lib.rs`
- owner entry symbols:
  - `build_semantic_request`
  - `map_adapter_event`
  - `map_adapter_events`
  - `classify_provider_error`

## Request Mainline

- normalized provider request enters provider semantic boundary
- OpenAI-compatible request path explicitly supports `responses`
- OpenAI-compatible request path explicitly supports `chat completions`
- provider-specific adapters render wire payloads without leaking adapter DTOs outside adapter crates

## Response Mainline

- provider raw stream or single-shot output becomes unified semantic events
- semantic output carries text, reasoning, tool, usage, terminal, and error semantics

## Error Mainline

- provider errors are classified into unified error contracts
- periodic-recoverable errors preserve recovery windows in seconds
- debug/raw retention stays separate from normal semantic output

## Shared Multi-Reference Functions

- `classify_provider_error`
  - owner: `crates/freehand-provider-core/src/lib.rs`
  - purpose: unify provider failures into shared recovery/error contract
  - allowed callers: provider adapters, tests
  - related tests: periodic recovery classification tests
  - why shared: keeps recovery policy centralized instead of duplicated per adapter
- `map_adapter_events`
  - owner: `crates/freehand-provider-core/src/lib.rs`
  - purpose: map one provider-parser output batch into shared semantic outputs
  - allowed callers: provider adapters, tests
  - related tests: openai/anthropic adapter parser tests
  - why shared: keeps event-batch normalization centralized instead of each adapter hand-looping output conversion

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `build_semantic_request` | `crates/freehand-provider-core/src/lib.rs` | build semantic provider request and retention policy | semantic request + debug flag | provider semantic request | reason/orchestrator | provider core boundary | bound |
| 02 | `map_adapter_event` | `crates/freehand-provider-core/src/lib.rs` | map normalized adapter event into shared semantic output | normalized adapter event | semantic output | adapter runtime | semantic mapper | bound |
| 03 | `map_adapter_events` | `crates/freehand-provider-core/src/lib.rs` | map normalized adapter event batch into shared semantic outputs | normalized adapter event batch | semantic output batch | adapter runtime | semantic mapper | bound |
| 04 | `classify_provider_error` | `crates/freehand-provider-core/src/lib.rs` | classify provider failure into shared error contract | provider error hint | unified error contract | adapter/runtime | error classifier | bound |

## Sync Status Against Code

- semantic request builder, single-event mapper, batch mapper, and error classifier are bound in code
