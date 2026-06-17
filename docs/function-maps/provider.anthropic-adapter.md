# Function Map: `provider.anthropic-adapter`

- feature_id: `provider.anthropic-adapter`
- owner crate: `crates/freehand-provider-anthropic`
- owner module: `crates/freehand-provider-anthropic/src/lib.rs`
- mainline call source: `docs/mainline-calls/provider.anthropic-adapter.json`
- generated wiki: `docs/wiki/provider.anthropic-adapter.md`
- owner entry symbols:
  - `AnthropicAdapter::new`
  - `AnthropicAdapter::render_request`
  - `AnthropicAdapter::parse_response`
  - `AnthropicAdapter::parse_stream_event`
  - `AnthropicExecutor::new`
  - `AnthropicExecutor::execute_once`
  - `AnthropicExecutor::execute_once_with_raw`
  - `AnthropicExecutor::execute_stream`
  - `AnthropicExecutor::execute_stream_with`
  - `AnthropicExecutor::execute_stream_with_raw`

## Request Mainline

- provider-neutral semantic request enters Anthropic adapter
- adapter renders Messages API request body with stateless conversation input
- adapter consumes typed `input_segments` and renders them to Anthropic wire text without owning segment admission truth
- adapter renders provider-neutral tool schema metadata into Anthropic `tools` and `tool_choice`
- adapter renders provider-neutral tool call/result exchanges into Anthropic assistant `tool_use` and user `tool_result` message content
- executor posts rendered requests to configured Anthropic-compatible base URL with explicit `x-api-key`, `anthropic-version`, and JSON headers

## Response Mainline

- Anthropic single-shot body or SSE event becomes provider-neutral semantic output
- raw-capable executor paths expose response bodies, HTTP error bodies, and SSE event bodies before semantic parsing so runtime can retain debug-only ledgers even when parsing fails
- partial tool-use input stays adapter-local until enough JSON exists to emit structured arguments
- live `minimonth` single-shot and SSE fixtures replay through the same parser entrypoints as synthetic tests
- executor single-shot path parses response body through `AnthropicAdapter::parse_response`
- executor stream path reads SSE event boundaries incrementally, parses `data:` payloads through `AnthropicAdapter::parse_stream_event`, and can notify callers before the HTTP response finishes

## Error Mainline

- unsupported protocol, invalid JSON body, invalid tool-use input, and stream-shape violations are explicit adapter errors
- HTTP transport failures and non-success HTTP statuses are explicit executor errors
- raw-callback failures from `execute_*_with_raw` are explicit executor errors and do not become semantic success
- Anthropic stop reasons are semantic metadata, not Freehand completion truth

## Shared Multi-Reference Functions

- `parse_tool_arguments_json`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: convert tool argument JSON string into shared structured tool arguments
  - allowed callers: provider adapters, tests
  - related tests: OpenAI tool-call parser tests, Anthropic tool-use parser tests
  - why shared: keeps tool-argument parsing centralized instead of duplicated per adapter
- `render_tool_arguments_json`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: render shared structured tool arguments back to JSON for provider wire requests
  - allowed callers: provider adapters, tests
  - related tests: Anthropic tool_result exchange renderer tests
  - why shared: avoids adapter-local second implementations of shared tool argument JSON

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `AnthropicAdapter::render_request` | `crates/freehand-provider-anthropic/src/lib.rs` | render semantic request to Anthropic messages wire request | provider semantic request | Anthropic path + JSON body | runtime/provider caller | adapter renderer | bound |
| 02 | `AnthropicAdapter::parse_response` | `crates/freehand-provider-anthropic/src/lib.rs` | parse single-shot Anthropic response | raw response body | provider semantic outputs | runtime/provider caller | adapter parser | bound |
| 03 | `AnthropicAdapter::parse_stream_event` | `crates/freehand-provider-anthropic/src/lib.rs` | parse one Anthropic SSE event and update partial state | raw stream event | provider semantic outputs | runtime/provider caller | adapter stream parser | bound |
| 04 | `AnthropicExecutor::execute_once` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic messages HTTP request through the raw-capable single-shot path | semantic request + auth/base URL | provider semantic outputs | runtime/provider caller | `execute_once_with_raw` + adapter parser | bound |
| 05 | `AnthropicExecutor::execute_once_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic messages HTTP request and expose raw response/error body before semantic parsing | semantic request + auth/base URL + raw callback | provider semantic outputs plus callback-visible raw body/error body | runtime/provider caller | HTTP executor + adapter parser | bound |
| 06 | `AnthropicExecutor::execute_stream` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic SSE request and return accumulated semantic outputs | semantic request + auth/base URL | provider semantic outputs | runtime/provider caller | `execute_stream_with` + adapter stream parser | bound |
| 07 | `AnthropicExecutor::execute_stream_with` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic SSE request and call back for each parsed semantic batch before stream completion | semantic request + auth/base URL + callback | incremental provider semantic output batches plus final accumulated outputs | runtime/provider caller | `execute_stream_with_raw` + adapter stream parser | bound |
| 08 | `AnthropicExecutor::execute_stream_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic SSE request and expose each raw SSE event body before semantic parsing | semantic request + auth/base URL + raw callback + output callback | incremental raw event bodies plus incremental provider semantic output batches | runtime/provider caller | HTTP executor + adapter stream parser | bound |

## Sync Status Against Code

- renderer and parser bindings now match `AnthropicAdapter`
- fixture replay bindings now cover `crates/freehand-provider-anthropic/fixtures/minimonth_messages_single.json`
- fixture replay bindings now cover `crates/freehand-provider-anthropic/fixtures/minimonth_messages_stream.sse`
- HTTP executor bindings now cover single-shot and incremental-SSE execution against local mock servers
- incremental stream regression proves callback delivery can happen before the provider response completes
- raw-capable executor bindings now preserve single-shot response bodies, HTTP error bodies, and SSE event bodies before semantic parsing
- request renderer now binds Anthropic `tools`, `tool_choice`, assistant `tool_use`, and user `tool_result` message rendering from provider-neutral metadata
- the generated wiki must be regenerated from `docs/mainline-calls/provider.anthropic-adapter.json` when this function-map truth changes
