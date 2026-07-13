# Wiki: `provider.openai-adapter`

Generated from `docs/mainline-calls/provider.openai-adapter.json`. Do not edit by hand.

- owner crate: `crates/freehand-provider-openai`
- owner module: `crates/freehand-provider-openai/src/lib.rs`
- function map: `docs/function-maps/provider.openai-adapter.md`
- generated wiki: `docs/wiki/provider.openai-adapter.md`
- test design: `docs/testing/provider.openai-adapter.md`

## Request Mainline

- provider-neutral semantic request enters OpenAI adapter
- adapter renders either `responses` or `chat completions` request body based on selected protocol
- adapter consumes typed `input_segments` and renders them to OpenAI wire text without owning segment admission truth
- adapter renders provider-neutral tool definitions and tool-result re-entry into the selected OpenAI wire shape so runtime never hardcodes protocol-specific tool wire

## Response Mainline

- OpenAI single-shot body or stream chunk becomes provider-neutral semantic output
- partial tool calls stay adapter-local until enough JSON exists to emit structured arguments
- OpenAI executor owns HTTP endpoint selection, bearer auth, status/body capture, SSE reading, and callback mapping before returning provider-neutral semantic outputs

## Error Mainline

- unsupported protocol, invalid JSON body, and invalid tool-argument payload are explicit adapter errors
- OpenAI finish reasons are semantic metadata, not Freehand completion truth

## Shared Multi-Reference Functions

- `parse_tool_arguments_json`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: convert tool argument JSON string into shared structured tool arguments
  - allowed callers: provider adapters, tests
  - related tests: OpenAI tool-call parser tests, Anthropic tool-use parser tests
  - why shared: keeps tool-argument parsing centralized instead of duplicated per adapter

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `OpenAiAdapter::render_request` | `crates/freehand-provider-openai/src/lib.rs` | render semantic request, tool definitions, and tool-result re-entry to OpenAI wire request | provider semantic request | OpenAI path plus JSON body | runtime/provider caller | adapter renderer |  |  |  | bound |
| 02 | `OpenAiAdapter::parse_response` | `crates/freehand-provider-openai/src/lib.rs` | parse single-shot OpenAI response | raw response body | provider semantic outputs | runtime/provider caller | adapter parser |  |  |  | bound |
| 03 | `OpenAiAdapter::parse_stream_event` | `crates/freehand-provider-openai/src/lib.rs` | parse one OpenAI stream event and update partial state | raw stream event | provider semantic outputs | runtime/provider caller | adapter stream parser |  |  |  | bound |
| 04 | `OpenAiExecutor::execute_once_with_raw` | `crates/freehand-provider-openai/src/lib.rs` | render and execute one non-stream OpenAI-compatible request without leaking wire DTOs to runtime | provider semantic request plus auth/base URL plus raw callback | provider semantic outputs plus callback-visible raw body/error body | runtime provider driver | OpenAI executor |  |  |  | bound |
| 05 | `OpenAiExecutor::execute_stream_with_raw` | `crates/freehand-provider-openai/src/lib.rs` | render and execute one streaming OpenAI-compatible request, collecting SSE events through adapter-owned parsing | provider semantic request plus auth/base URL plus raw callback plus semantic callback | incremental raw event bodies plus incremental semantic output batches plus accumulated outputs | runtime provider driver | OpenAI executor |  |  |  | bound |

## Sync Status Against Mainline Call

- renderer/parser bindings match `OpenAiAdapter`, and live HTTP/SSE executor bindings match `OpenAiExecutor`
- generated wiki must be regenerated from `docs/mainline-calls/provider.openai-adapter.json` when this function-map truth changes
