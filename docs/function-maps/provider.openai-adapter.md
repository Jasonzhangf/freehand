# Function Map: `provider.openai-adapter`

- feature_id: `provider.openai-adapter`
- owner crate: `crates/freehand-provider-openai`
- owner module: `crates/freehand-provider-openai/src/lib.rs`
- mainline call source: `docs/mainline-calls/provider.openai-adapter.json`
- generated wiki: `docs/wiki/provider.openai-adapter.md`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `provider_request.render_hosted_search_wire`
  - `provider_request.render_openai_image_input_wire`
  - `provider_response.observe_hosted_search_call`
- owner entry symbols:
  - `OpenAiAdapter::new`
  - `OpenAiAdapter::render_request`
  - `openai_responses_hosted_tool`
  - `OpenAiAdapter::parse_response`
  - `OpenAiAdapter::parse_stream_event`
  - `parse_openai_usage`
  - `provider_hosted_web_search_observation`
  - `OpenAiExecutor::execute_once_with_raw`
  - `OpenAiExecutor::execute_stream_with_raw`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - OpenAI adapter wire renderer/parser operations for `provider_request` and `provider_response`
- touched resources:
  - `provider_request`
  - `provider_response`
  - `provider_hosted_search`
  - `input_attachment`
- resource operations:
  - `provider_request.render_hosted_search_wire` (`provider_request` -> `provider_hosted_search`)
  - `provider_request.render_openai_image_input_wire` (`provider_request` -> `input_attachment`)
  - `provider_response.observe_hosted_search_call` (`provider_response` -> `provider_hosted_search`)
- forbidden shortcuts:
  - OpenAI hosted `web_search` wire must be rendered only from `ProviderHostedToolDefinition`, never from a local Freehand function tool named `web_search`.
  - `web_search_call` output items must become provider-neutral observations, not `ToolCall` values that runtime would execute locally.

## Request Mainline

- provider-neutral semantic request enters OpenAI adapter
- adapter renders either `responses` or `chat completions` request body based on selected protocol
- adapter consumes typed `input_segments` and renders them to OpenAI wire text without owning segment admission truth
- adapter renders provider-neutral tool definitions and tool-result re-entry into the selected OpenAI wire shape so runtime never hardcodes protocol-specific tool wire
- adapter renders provider-neutral image attachments as Responses `input_image` data URLs or Chat Completions `image_url` data URLs; attachment id/name metadata never leaks into provider wire
- adapter renders provider-neutral `ProviderHostedToolDefinition::WebSearch` into OpenAI Responses hosted `{"type":"web_search","external_web_access":true}` wire when the live bridge declares it

## Response Mainline

- OpenAI single-shot body or stream chunk becomes provider-neutral semantic output
- optional wire `error` fields are absent when missing or JSON null; only a non-null error object becomes a provider semantic error
- OpenAI Responses `web_search_call` output items are observed as provider-hosted reasoning events so the search stays provider-native and never enters local tool execution
- partial tool calls stay adapter-local until enough JSON exists to emit structured arguments
- usage parsing keeps OpenAI `input_tokens`/`prompt_tokens` as normalized total input, writes it explicitly to `normalized_input_tokens`, and maps cache-read/cache-write aliases from `input_tokens_details` or `prompt_tokens_details`; cache categories never inflate the input denominator
- OpenAI executor owns HTTP endpoint selection, bearer auth, status/body capture, SSE reading, and callback mapping before returning provider-neutral semantic outputs

## Error Mainline

- unsupported protocol, invalid JSON body, and invalid tool-argument payload are explicit adapter errors
- a successful OpenAI-compatible response carrying `error: null` is not a provider error and must not enter turn/UI error truth
- non-null wire error objects remain typed provider semantic errors
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
| 01 | `OpenAiAdapter::render_request` | `crates/freehand-provider-openai/src/lib.rs` | render semantic request, tool definitions, and tool-result re-entry to OpenAI wire request | provider semantic request | OpenAI path + JSON body | runtime/provider caller | adapter renderer | bound |
| 01a | `openai_responses_hosted_tool` | `crates/freehand-provider-openai/src/lib.rs` | render provider-neutral hosted search declarations into OpenAI Responses hosted tool wire | provider semantic request hosted tool metadata | OpenAI Responses hosted tool JSON | `OpenAiAdapter::render_request` | adapter renderer | bound |
| 01b | `openai_responses_attachment_content` / `openai_chat_attachment_content` | `crates/freehand-provider-openai/src/lib.rs` | render provider-neutral image attachment bytes into the selected OpenAI protocol image content block | provider image attachment | Responses `input_image` or Chat Completions `image_url` data URL | `OpenAiAdapter::render_request` | adapter renderer | bound |
| 02 | `OpenAiAdapter::parse_response` | `crates/freehand-provider-openai/src/lib.rs` | parse single-shot OpenAI response | raw response body | provider semantic outputs | runtime/provider caller | adapter parser | bound |
| 02b | `parse_openai_usage` | `crates/freehand-provider-openai/src/lib.rs` | normalize Responses and Chat Completions usage totals plus compatible cache-read/cache-write detail aliases | raw OpenAI-compatible usage object | provider-neutral `TokenUsage` | `OpenAiAdapter::parse_response` / `OpenAiAdapter::parse_stream_event` | adapter usage parser | bound |
| 02a | `provider_hosted_web_search_observation` | `crates/freehand-provider-openai/src/lib.rs` | map OpenAI Responses `web_search_call` items into provider-neutral reasoning observations | raw OpenAI response item | provider semantic reasoning event | `OpenAiAdapter::parse_response` / `OpenAiAdapter::parse_stream_event` | adapter parser | bound |
| 03 | `OpenAiAdapter::parse_stream_event` | `crates/freehand-provider-openai/src/lib.rs` | parse one OpenAI stream event and update partial state | raw stream event | provider semantic outputs | runtime/provider caller | adapter stream parser | bound |
| 04 | `OpenAiExecutor::execute_once_with_raw` | `crates/freehand-provider-openai/src/lib.rs` | render and execute one non-stream OpenAI-compatible request without leaking wire DTOs to runtime | provider semantic request + auth/base URL + raw callback | provider semantic outputs plus callback-visible raw body/error body | runtime provider driver | OpenAI executor | bound |
| 05 | `OpenAiExecutor::execute_stream_with_raw` | `crates/freehand-provider-openai/src/lib.rs` | render and execute one streaming OpenAI-compatible request, collecting SSE events through adapter-owned parsing | provider semantic request + auth/base URL + raw callback + semantic callback | incremental raw event bodies plus incremental semantic output batches plus accumulated outputs | runtime provider driver | OpenAI executor | bound |
| 05a | `OpenAiExecutorFactory::build_executor` | `crates/freehand-provider-openai/src/lib.rs` | adapt provider-core executor config into an OpenAI executor without exposing OpenAI wire DTOs to runtime | provider descriptor + auth/base URL | boxed provider-core live executor or explicit unsupported/build failure | freehand-provider-executors assembly | OpenAI executor factory | bound |
| 05b | `classify_openai_executor_error` | `crates/freehand-provider-openai/src/lib.rs` | classify OpenAI executor build/transport/adapter/callback failures into provider-core retry/failover error info | OpenAI executor error | provider-core executor error info with code/message/retry/failover flags | OpenAI executor factory and ProviderLiveExecutor impl | OpenAI error classifier | bound |

## Sync Status Against Code

- renderer/parser bindings match `OpenAiAdapter`, and live HTTP/SSE executor bindings match `OpenAiExecutor`
- `OpenAiExecutorFactory` implements provider-core `ProviderExecutorFactory`, and `OpenAiExecutor` implements provider-core `ProviderLiveExecutor`
- hosted OpenAI Responses web_search request rendering and `web_search_call` observation are adapter-owned and covered by focused adapter tests
- the generated wiki must be regenerated from `docs/mainline-calls/provider.openai-adapter.json` when this function-map truth changes
