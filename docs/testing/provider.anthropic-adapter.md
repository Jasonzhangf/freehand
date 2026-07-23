# Test Design: `provider.anthropic-adapter`

- feature_id: `provider.anthropic-adapter`
- owner: `crates/freehand-provider-anthropic`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `provider_request.render_anthropic_hosted_search_wire`
  - `provider_response.observe_anthropic_hosted_search_call`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `provider_request.render_anthropic_hosted_search_wire` | bound | `cargo test -p freehand-provider-anthropic web_search -- --nocapture` covers `anthropic_messages_hosted_tool` rendering from `ProviderHostedToolDefinition::WebSearch` | `cargo test -p freehand-provider-anthropic web_search -- --nocapture` verifies the rendered Messages body includes `web_search_20250305` as a server tool and not a local `input_schema` function tool | `freehand-cliS adp-provider-web-search-test --url ws://127.0.0.1:4042/adp --provider minimax --query "Use web_search to find the current UTC date and one current news headline from openai.com today. Do not answer from memory."` live-tests S-profile Anthropic-compatible Messages provider acceptance or explicit provider rejection |
| `provider_response.observe_anthropic_hosted_search_call` | bound | `cargo test -p freehand-provider-anthropic web_search -- --nocapture` covers `server_tool_use` and `web_search_tool_result` parsing into hosted-search observations | `cargo test -p freehand-provider-anthropic web_search -- --nocapture` verifies hosted-search blocks become provider-neutral reasoning events and never `ProviderSemanticOutput::ToolCall` | `freehand-cliS adp-provider-web-search-test --url ws://127.0.0.1:4042/adp --provider minimax --query "Use web_search to find the current UTC date and one current news headline from openai.com today. Do not answer from memory."` proves online observation when the provider emits hosted-search blocks, or returns the exact provider failure as the visible test result |

- lifecycle path under test:
  - semantic request renders typed input segments into Messages API request
  - semantic request renders provider-neutral tool schema/choice/exchange into Anthropic tool wire shape
  - semantic request renders provider-neutral hosted web_search metadata into Anthropic server-tool wire without creating local Freehand tool truth
  - executor posts rendered Messages API request with explicit auth/version/content headers
  - single-shot and SSE outputs normalize into shared semantic events
  - hosted `server_tool_use` / `web_search_tool_result` blocks normalize into reasoning observations rather than local tool calls
  - partial tool-use input accumulates until arguments become complete
- white-box plan:
  - request renderer, response parser, SSE parser, partial tool accumulator
  - indexed Anthropic streaming tool-use path where `content_block_start` provides id/name but later `input_json_delta` and `content_block_stop` events only provide `index`
  - Anthropic `tools` / `tool_choice` / `tool_result` request rendering
  - Anthropic hosted web_search request rendering and hosted block observation
  - executor URL joining, header emission, non-success status handling, SSE event-boundary parsing, and incremental callback delivery
  - raw-capable executor callback coverage for response bodies, HTTP error bodies, and SSE event bodies before semantic parse
- module black-box plan:
  - adapter emits provider-neutral text/tool/usage/terminal/error outputs for Anthropic messages
  - adapter emits provider-neutral hosted-search observations from Anthropic server-tool response blocks without routing through local tool execution
  - adapter replays live `minimonth` Anthropic-compatible single-shot response fixture
  - adapter replays live `minimonth` Anthropic-compatible SSE stream fixture
  - executor emits provider-neutral outputs from local single-shot and SSE mock servers
  - executor proves first semantic batch can be observed before the stream is released to completion
  - executor raw callbacks surface parse-failing bodies and HTTP error bodies for runtime debug-ledger retention
- project black-box impact:
  - reason layer can consume Anthropic semantic outputs without protocol leakage
- fixtures / replay inputs / runtime evidence paths:
  - `crates/freehand-provider-anthropic/fixtures/minimonth_messages_single.json`
  - `crates/freehand-provider-anthropic/fixtures/minimonth_messages_stream.sse`
  - `~/.freehand/ledgers/providers/anthropic`
  - `~/.freehand/replays/providers/anthropic`
- sync status between design and implementation:
  - `AnthropicAdapter` baseline implemented
  - request rendering covers Messages API with default `max_tokens=8192`, explicit adapter config validation, and typed input segments
  - request rendering now covers Anthropic `tools`, `tool_choice`, assistant `tool_use`, and user `tool_result`
  - request rendering now covers Anthropic hosted `web_search_20250305` server-tool declarations from provider-neutral hosted tool metadata
  - single-shot and stream parsing cover text, tool use, usage, terminal, and error paths
  - single-shot and stream parsing now cover hosted `server_tool_use` / `web_search_tool_result` observations without creating local tool calls
  - stream parsing covers indexed tool-use deltas without repeated tool id/name, matching live Anthropic-compatible provider SSE evidence
  - live `minimonth` fixtures now cover thinking/text/usage/cache/terminal replay for single-shot and SSE
  - HTTP executor now supports incremental SSE callback delivery via `AnthropicExecutor::execute_stream_with`
  - raw-capable executor variants now support debug retention of single-shot response bodies, HTTP error bodies, and SSE event bodies before semantic parse
  - HTTP executor tests use local mock servers and do not require live provider credentials
- mainline/wiki sync:
  - wiki generated from mainline call must stay in sync with Anthropic adapter owner code and function map updates
