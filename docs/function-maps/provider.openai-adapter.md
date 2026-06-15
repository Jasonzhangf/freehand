# Function Map: `provider.openai-adapter`

- feature_id: `provider.openai-adapter`
- owner crate: `crates/freehand-provider-openai`
- owner module: `crates/freehand-provider-openai/src/lib.rs`
- owner entry symbols:
  - `OpenAiAdapter::new`
  - `OpenAiAdapter::render_request`
  - `OpenAiAdapter::parse_response`
  - `OpenAiAdapter::parse_stream_event`

## Request Mainline

- provider-neutral semantic request enters OpenAI adapter
- adapter renders either `responses` or `chat completions` request body based on selected protocol

## Response Mainline

- OpenAI single-shot body or stream chunk becomes provider-neutral semantic output
- partial tool calls stay adapter-local until enough JSON exists to emit structured arguments

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

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `OpenAiAdapter::render_request` | `crates/freehand-provider-openai/src/lib.rs` | render semantic request to OpenAI wire request | provider semantic request | OpenAI path + JSON body | runtime/provider caller | adapter renderer | bound |
| 02 | `OpenAiAdapter::parse_response` | `crates/freehand-provider-openai/src/lib.rs` | parse single-shot OpenAI response | raw response body | provider semantic outputs | runtime/provider caller | adapter parser | bound |
| 03 | `OpenAiAdapter::parse_stream_event` | `crates/freehand-provider-openai/src/lib.rs` | parse one OpenAI stream event and update partial state | raw stream event | provider semantic outputs | runtime/provider caller | adapter stream parser | bound |

## Sync Status Against Code

- renderer and parser bindings now match `OpenAiAdapter`
