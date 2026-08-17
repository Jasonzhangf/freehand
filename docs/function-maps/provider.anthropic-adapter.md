# Function Map: `provider.anthropic-adapter`

- feature_id: `provider.anthropic-adapter`
- owner crate: `crates/freehand-provider-anthropic`
- owner module: `crates/freehand-provider-anthropic/src/lib.rs`
- mainline call source: `docs/mainline-calls/provider.anthropic-adapter.json`
- generated wiki: `docs/wiki/provider.anthropic-adapter.md`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `provider_request.render_anthropic_hosted_search_wire`
  - `provider_request.render_anthropic_image_input_wire`
  - `provider_response.observe_anthropic_hosted_search_call`
- owner entry symbols:
  - `AnthropicAdapter::new`
  - `AnthropicAdapter::render_request`
  - `anthropic_messages_hosted_tool`
  - `AnthropicAdapter::parse_response`
  - `AnthropicAdapter::parse_stream_event`
  - `parse_anthropic_usage`
  - `anthropic_hosted_search_discovery`
  - `AnthropicExecutor::new`
  - `AnthropicExecutor::execute_once`
  - `AnthropicExecutor::execute_once_with_raw`
  - `AnthropicExecutor::execute_stream`
  - `AnthropicExecutor::execute_stream_with`
  - `AnthropicExecutor::execute_stream_with_raw`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - Anthropic adapter wire renderer/parser operations for `provider_request` and `provider_response`
- touched resources:
  - `provider_request`
  - `provider_response`
  - `provider_hosted_search`
  - `input_attachment`
- resource operations:
  - `provider_request.render_anthropic_hosted_search_wire` (`provider_request` -> `provider_hosted_search`)
  - `provider_request.render_anthropic_image_input_wire` (`provider_request` -> `input_attachment`)
  - `provider_response.observe_anthropic_hosted_search_call` (`provider_response` -> `provider_hosted_search`)
- forbidden shortcuts:
  - Anthropic hosted `web_search` wire must be rendered only from `ProviderHostedToolDefinition`, never from a local Freehand function tool named `web_search`.
  - `server_tool_use` and `web_search_tool_result` hosted-search blocks must become provider-neutral observations, not `ToolCall` values that runtime would execute locally.

## Request Mainline

- provider-neutral semantic request enters Anthropic adapter
- adapter renders Messages API request body with stateless conversation input
- adapter consumes typed `input_segments` and renders them to Anthropic wire text without owning segment admission truth
- adapter renders provider-neutral tool schema metadata into Anthropic `tools` and `tool_choice`
- adapter renders provider-neutral hosted web_search metadata into Anthropic Messages server-tool wire without creating a local Freehand function tool
- adapter renders provider-neutral image attachments as Anthropic base64 image source blocks; attachment id/name metadata never leaks into provider wire
- adapter renders provider-neutral tool call/result exchanges into Anthropic assistant `tool_use` and user `tool_result` message content
- executor posts rendered requests to configured Anthropic-compatible base URL with explicit `x-api-key`, `anthropic-version`, and JSON headers

## Response Mainline

- Anthropic single-shot body or SSE event becomes provider-neutral semantic output
- raw-capable executor paths expose response bodies, HTTP error bodies, and SSE event bodies before semantic parsing so runtime can retain debug-only ledgers even when parsing fails
- partial tool-use input stays adapter-local until enough JSON exists to emit structured arguments
- streamed tool-use blocks may carry the tool id/name only on `content_block_start`; subsequent `input_json_delta` and `content_block_stop` events may carry only the stream `index`, so the adapter owns index-to-tool-call state until the block closes
- server-side hosted web_search blocks become provider-neutral reasoning observations and never local `ToolCall` values
- usage parsing normalizes Anthropic's uncached `input_tokens` plus cache creation/read counters into one total input before projecting cache hit rate and total tokens
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
| 01a | `anthropic_messages_hosted_tool` | `crates/freehand-provider-anthropic/src/lib.rs` | render provider-neutral hosted search declarations into Anthropic Messages server-tool wire | provider semantic request hosted tool metadata | Anthropic Messages hosted web_search tool JSON | `AnthropicAdapter::render_request` | adapter hosted-tool renderer | bound |
| 01b | `anthropic_attachment_content` | `crates/freehand-provider-anthropic/src/lib.rs` | render provider-neutral image attachment bytes as an Anthropic base64 image source block | provider image attachment | Anthropic Messages image content block | `AnthropicAdapter::render_request` | adapter renderer | bound |
| 02 | `AnthropicAdapter::parse_response` | `crates/freehand-provider-anthropic/src/lib.rs` | parse single-shot Anthropic response | raw response body | provider semantic outputs | runtime/provider caller | adapter parser | bound |
| 02a | `anthropic_hosted_search_discovery` | `crates/freehand-provider-anthropic/src/lib.rs` | map Anthropic Messages hosted web_search result blocks into provider-neutral typed search discovery | domain plan, tracked query, and raw Anthropic web_search_tool_result block | `SearchDiscoveryDelivery` | `AnthropicAdapter::parse_response` / `AnthropicAdapter::parse_stream_event` | adapter hosted-search parser | bound |
| 02b | `parse_anthropic_usage` | `crates/freehand-provider-anthropic/src/lib.rs` | normalize uncached input plus cache creation/read counters into provider-neutral total-input usage | raw Anthropic usage object | provider-neutral `TokenUsage` | `AnthropicAdapter::parse_response` / `AnthropicAdapter::parse_stream_event` | adapter usage parser | bound |
| 03 | `AnthropicAdapter::parse_stream_event` | `crates/freehand-provider-anthropic/src/lib.rs` | parse one Anthropic SSE event and update partial state | raw stream event | provider semantic outputs | runtime/provider caller | adapter stream parser | bound |
| 04 | `AnthropicExecutor::execute_once` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic messages HTTP request through the raw-capable single-shot path | semantic request + auth/base URL | provider semantic outputs | runtime/provider caller | `execute_once_with_raw` + adapter parser | bound |
| 05 | `AnthropicExecutor::execute_once_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic messages HTTP request and expose raw response/error body before semantic parsing | semantic request + auth/base URL + raw callback | provider semantic outputs plus callback-visible raw body/error body | runtime/provider caller | HTTP executor + adapter parser | bound |
| 06 | `AnthropicExecutor::execute_stream` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic SSE request and return accumulated semantic outputs | semantic request + auth/base URL | provider semantic outputs | runtime/provider caller | `execute_stream_with` + adapter stream parser | bound |
| 07 | `AnthropicExecutor::execute_stream_with` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic SSE request and call back for each parsed semantic batch before stream completion | semantic request + auth/base URL + callback | incremental provider semantic output batches plus final accumulated outputs | runtime/provider caller | `execute_stream_with_raw` + adapter stream parser | bound |
| 08 | `AnthropicExecutor::execute_stream_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one Anthropic SSE request and expose each raw SSE event body before semantic parsing | semantic request + auth/base URL + raw callback + output callback | incremental raw event bodies plus incremental provider semantic output batches | runtime/provider caller | HTTP executor + adapter stream parser | bound |
| 09 | `AnthropicExecutorFactory::build_executor` | `crates/freehand-provider-anthropic/src/lib.rs` | adapt provider-core executor config into an Anthropic executor without exposing Anthropic wire DTOs to runtime | provider descriptor + auth/base URL | boxed provider-core live executor or explicit unsupported/build failure | freehand-provider-executors assembly | Anthropic executor factory | bound |
| 10 | `classify_anthropic_executor_error` | `crates/freehand-provider-anthropic/src/lib.rs` | classify Anthropic executor build/transport/adapter/callback failures into provider-core retry/failover error info | Anthropic executor error | provider-core executor error info with code/message/retry/failover flags | Anthropic executor factory and ProviderLiveExecutor impl | Anthropic error classifier | bound |

## Sync Status Against Code

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
- `AnthropicExecutorFactory` implements provider-core `ProviderExecutorFactory`, and `AnthropicExecutor` implements provider-core `ProviderLiveExecutor`
- the generated wiki must be regenerated from `docs/mainline-calls/provider.anthropic-adapter.json` when this function-map truth changes
