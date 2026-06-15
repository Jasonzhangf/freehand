# Function Map: `provider.anthropic-adapter`

- feature_id: `provider.anthropic-adapter`
- owner crate: `crates/freehand-provider-anthropic`
- owner module: `crates/freehand-provider-anthropic/src/lib.rs`
- owner entry symbols:
  - `AnthropicAdapter::new`
  - `AnthropicAdapter::render_request`
  - `AnthropicAdapter::parse_response`
  - `AnthropicAdapter::parse_stream_event`

## Request Mainline

- provider-neutral semantic request enters Anthropic adapter
- adapter renders Messages API request body with stateless conversation input

## Response Mainline

- Anthropic single-shot body or SSE event becomes provider-neutral semantic output
- partial tool-use input stays adapter-local until enough JSON exists to emit structured arguments

## Error Mainline

- unsupported protocol, invalid JSON body, invalid tool-use input, and stream-shape violations are explicit adapter errors
- Anthropic stop reasons are semantic metadata, not Freehand completion truth

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
| 01 | `AnthropicAdapter::render_request` | `crates/freehand-provider-anthropic/src/lib.rs` | render semantic request to Anthropic messages wire request | provider semantic request | Anthropic path + JSON body | runtime/provider caller | adapter renderer | bound |
| 02 | `AnthropicAdapter::parse_response` | `crates/freehand-provider-anthropic/src/lib.rs` | parse single-shot Anthropic response | raw response body | provider semantic outputs | runtime/provider caller | adapter parser | bound |
| 03 | `AnthropicAdapter::parse_stream_event` | `crates/freehand-provider-anthropic/src/lib.rs` | parse one Anthropic SSE event and update partial state | raw stream event | provider semantic outputs | runtime/provider caller | adapter stream parser | bound |

## Sync Status Against Code

- renderer and parser bindings now match `AnthropicAdapter`
