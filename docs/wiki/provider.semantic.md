# Wiki: `provider.semantic`

Generated from `docs/mainline-calls/provider.semantic.json`. Do not edit by hand.

- owner crate: `crates/freehand-provider-core`
- owner module: `crates/freehand-provider-core/src/lib.rs`
- function map: `docs/function-maps/provider.semantic.md`
- generated wiki: `docs/wiki/provider.semantic.md`
- test design: `docs/testing/provider.semantic.md`

## Request Mainline

- normalized provider request enters provider semantic boundary
- provider semantic request must validate the typed provider payload contract before adapter rendering
- OpenAI-compatible request path explicitly supports `responses`
- OpenAI-compatible request path explicitly supports `chat completions`
- provider-specific adapters render wire payloads without leaking adapter DTOs outside adapter crates
- provider semantic request must stay provider-neutral
- provider metadata and request content must stay separate types
- provider semantic request may carry provider-neutral tool metadata as `ProviderToolDefinition`, `ProviderToolChoice`, and `ProviderToolExchange`; these are not request text and must be rendered only by adapter owners
- `freehand-provider-core` may bridge reason to provider, but must not import `freehand-reason` implementation truth

## Response Mainline

- provider raw stream or single-shot output becomes unified semantic events
- semantic output carries text, reasoning, tool, usage, terminal, and error semantics
- tool-use output maps to shared `ReasonReq04ToolCall`; tool-result continuation maps to shared `ReasonReq05ToolResultReentry`
- provider stop/finish signals remain metadata/usage signals until `freehand-reason` decides terminal truth

## Error Mainline

- provider errors are classified into unified error contracts
- periodic-recoverable errors preserve recovery windows in seconds
- debug/raw retention stays separate from normal semantic output
- metadata/request boundary confusion is architecture-invalid and should be blocked by future gate work

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
| 01 | `build_semantic_request` | `crates/freehand-provider-core/src/lib.rs` | build semantic provider request and retention policy | typed provider payload plus debug flag | provider semantic request | reason/orchestrator | provider core boundary | bound |
| 02 | `ProviderToolDefinition` | `crates/freehand-provider-core/src/lib.rs` | carry provider-neutral tool schema metadata outside request text | tool name/description/input schema | adapter-renderable tool metadata | live bridge/tests | provider semantic request | bound |
| 03 | `ProviderToolExchange` | `crates/freehand-provider-core/src/lib.rs` | carry provider-neutral tool call/result continuation outside request text | tool call plus tool result re-entry | adapter-renderable tool continuation | live bridge/tests | provider semantic request | bound |
| 04 | `map_adapter_event` | `crates/freehand-provider-core/src/lib.rs` | map normalized adapter event into shared semantic output | normalized adapter event | semantic output | adapter runtime | semantic mapper | bound |
| 05 | `map_adapter_events` | `crates/freehand-provider-core/src/lib.rs` | map normalized adapter event batch into shared semantic outputs | normalized adapter event batch | semantic output batch | adapter runtime | semantic mapper | bound |
| 06 | `classify_provider_error` | `crates/freehand-provider-core/src/lib.rs` | classify provider failure into shared error contract | provider error hint | unified error contract | adapter/runtime | error classifier | bound |

## Sync Status Against Mainline Call

- semantic request builder, single-event mapper, batch mapper, and error classifier are bound in code
- semantic request builder now consumes validated `input_segments` payload contract before adapter rendering
- provider-neutral tool schema, tool choice, and tool exchange metadata are bound on `ProviderSemanticRequest`
- provider semantic layer is independent from provider adapter implementation details and from `freehand-reason` implementation crate
- metadata/request hard isolation is required architecture truth but still needs dedicated type/gate closeout
- generated wiki must be regenerated from `docs/mainline-calls/provider.semantic.json` when this function-map truth changes
