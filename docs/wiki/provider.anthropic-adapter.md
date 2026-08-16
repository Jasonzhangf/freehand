# Wiki: `provider.anthropic-adapter`

Generated from `docs/mainline-calls/provider.anthropic-adapter.json`. Do not edit by hand.

- owner crate: `crates/freehand-provider-anthropic`
- owner module: `crates/freehand-provider-anthropic/src/lib.rs`
- function map: `docs/function-maps/provider.anthropic-adapter.md`
- generated wiki: `docs/wiki/provider.anthropic-adapter.md`
- test design: `docs/testing/provider.anthropic-adapter.md`

## Resource Operation Backlinks

- provider_request.render_anthropic_hosted_search_wire
- provider_request.render_anthropic_image_input_wire
- provider_response.observe_anthropic_hosted_search_call

## Request Mainline

- provider-neutral semantic request enters Anthropic adapter
- adapter renders Messages API request body with stateless conversation input
- adapter consumes typed `input_segments` and renders them to Anthropic wire text without owning segment admission truth
- adapter renders provider-neutral tool schema metadata into Anthropic `tools` and `tool_choice`
- adapter renders provider-neutral hosted web_search metadata into Anthropic Messages server-tool wire without creating a local Freehand function tool
- adapter renders current-submit provider-neutral image attachments into Anthropic Messages image source blocks without leaking attachment ids or filenames
- adapter renders provider-neutral tool call/result exchanges into Anthropic assistant `tool_use` and user `tool_result` message content
- executor posts rendered requests to configured Anthropic-compatible base URL with explicit `x-api-key`, `anthropic-version`, and JSON headers

## Response Mainline

- Anthropic single-shot body or SSE event becomes provider-neutral semantic output
- raw-capable executor paths expose response bodies, HTTP error bodies, and SSE event bodies before semantic parsing so runtime can retain debug-only ledgers even when parsing fails
- partial tool-use input stays adapter-local until enough JSON exists to emit structured arguments
- streamed tool-use blocks may carry the tool id/name only on `content_block_start`; subsequent `input_json_delta` and `content_block_stop` events may carry only the stream `index`, so the adapter owns index-to-tool-call state until the block closes
- server-side hosted web_search blocks become provider-neutral reasoning observations and never local `ToolCall` values
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

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `AnthropicAdapter::render_request` | `crates/freehand-provider-anthropic/src/lib.rs` | render semantic request to Anthropic messages wire request | provider semantic request | Anthropic path plus JSON body | runtime/provider caller | adapter renderer |  |  |  | bound |
| 01a | `anthropic_messages_hosted_tool` | `crates/freehand-provider-anthropic/src/lib.rs` | render provider-neutral hosted search declarations into Anthropic Messages server-tool wire | provider semantic request hosted tool metadata | Anthropic Messages hosted web_search tool JSON | AnthropicAdapter::render_request | adapter hosted-tool renderer | provider_request | provider_hosted_search | provider_request.render_anthropic_hosted_search_wire | bound |
| 01b | `anthropic_attachment_content` | `crates/freehand-provider-anthropic/src/lib.rs` | render provider-neutral current-submit image attachments into Anthropic Messages image content blocks | ProviderInputAttachment image metadata plus base64 payload | Anthropic Messages image block with base64 source and media type without attachment id/name leakage | AnthropicAdapter::render_request | adapter image renderer | provider_request | input_attachment | provider_request.render_anthropic_image_input_wire | bound |
| 02 | `AnthropicAdapter::parse_response` | `crates/freehand-provider-anthropic/src/lib.rs` | parse single-shot Anthropic response | raw response body | provider semantic outputs | runtime/provider caller | adapter parser |  |  |  | bound |
| 02a | `anthropic_hosted_search_discovery` | `crates/freehand-provider-anthropic/src/lib.rs` | map Anthropic Messages hosted web_search blocks into provider-neutral reasoning observations | raw Anthropic server_tool_use or web_search_tool_result block | provider semantic reasoning event | AnthropicAdapter::parse_response / AnthropicAdapter::parse_stream_event | adapter hosted-search parser | provider_response | provider_hosted_search | provider_response.observe_anthropic_hosted_search_call | bound |
| 03 | `AnthropicAdapter::parse_stream_event` | `crates/freehand-provider-anthropic/src/lib.rs` | parse one Anthropic SSE event and update partial state | raw stream event | provider semantic outputs | runtime/provider caller | adapter stream parser |  |  |  | bound |
| 04 | `AnthropicExecutor::execute_once` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic messages HTTP request through the raw-capable single-shot path | semantic request plus auth/base URL | provider semantic outputs | runtime/provider caller | execute_once_with_raw + adapter parser |  |  |  | bound |
| 05 | `AnthropicExecutor::execute_once_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic messages HTTP request and expose raw response/error body before semantic parsing | semantic request plus auth/base URL plus raw callback | provider semantic outputs plus callback-visible raw body/error body | runtime/provider caller | HTTP executor plus adapter parser |  |  |  | bound |
| 06 | `AnthropicExecutor::execute_stream` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic SSE request and return accumulated semantic outputs | semantic request plus auth/base URL | provider semantic outputs | runtime/provider caller | execute_stream_with plus adapter stream parser |  |  |  | bound |
| 07 | `AnthropicExecutor::execute_stream_with` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic SSE request and call back for each parsed semantic batch before stream completion | semantic request plus auth/base URL plus callback | incremental provider semantic output batches plus final accumulated outputs | runtime/provider caller | execute_stream_with_raw + adapter stream parser |  |  |  | bound |
| 08 | `AnthropicExecutor::execute_stream_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic SSE request and expose each raw SSE event body before semantic parsing | semantic request plus auth/base URL plus raw callback plus output callback | incremental raw event bodies plus incremental provider semantic output batches | runtime/provider caller | HTTP executor plus adapter stream parser |  |  |  | bound |
| 09 | `AnthropicExecutorFactory::build_executor` | `crates/freehand-provider-anthropic/src/lib.rs` | adapt provider-core executor config into an Anthropic executor without exposing Anthropic wire DTOs to runtime | provider descriptor plus auth/base URL | boxed provider-core live executor or explicit unsupported/build failure | freehand-provider-executors assembly | Anthropic executor factory |  |  |  | bound |
| 10 | `classify_anthropic_executor_error` | `crates/freehand-provider-anthropic/src/lib.rs` | classify Anthropic executor build/transport/adapter/callback failures into provider-core retry/failover error info | Anthropic executor error | provider-core executor error info with code/message/retry/failover flags | Anthropic executor factory and ProviderLiveExecutor impl | Anthropic error classifier |  |  |  | bound |

## Sync Status Against Mainline Call

- renderer and parser bindings now match `AnthropicAdapter`
- fixture replay bindings now cover `crates/freehand-provider-anthropic/fixtures/minimonth_messages_single.json`
- fixture replay bindings now cover `crates/freehand-provider-anthropic/fixtures/minimonth_messages_stream.sse`
- HTTP executor bindings now cover single-shot and incremental-SSE execution against local mock servers
- incremental stream regression proves callback delivery can happen before the provider response completes
- raw-capable executor bindings now preserve single-shot response bodies, HTTP error bodies, and SSE event bodies before semantic parsing
- request renderer now binds Anthropic `tools`, `tool_choice`, assistant `tool_use`, and user `tool_result` message rendering from provider-neutral metadata
- request renderer default output budget is locked by `DEFAULT_ANTHROPIC_MAX_TOKENS=8192`
- stream parser now binds indexed Anthropic `tool_use` events where `content_block_start` has id/name and later `input_json_delta` / `content_block_stop` events reference only `index`
- hosted Anthropic Messages web_search request rendering and hosted-search response observations are adapter-owned and covered by focused web-search tests
- generated wiki must be regenerated from `docs/mainline-calls/provider.anthropic-adapter.json` when this function-map truth changes
- AnthropicExecutorFactory implements provider-core `ProviderExecutorFactory`, and AnthropicExecutor implements provider-core `ProviderLiveExecutor`, so runtime consumes only provider-core traits while Anthropic wire execution stays adapter-owned.
