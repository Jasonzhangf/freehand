# Test Design: `provider.openai-adapter`

- feature_id: `provider.openai-adapter`
- owner: `crates/freehand-provider-openai`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `provider_request.render_hosted_search_wire`
  - `provider_request.render_openai_image_input_wire`
  - `provider_response.observe_hosted_search_call`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `provider_request.render_hosted_search_wire` | bound | `cargo test -p freehand-provider-openai web_search -- --nocapture` covers hosted `web_search` wire rendering for `ProviderHostedToolDefinition::WebSearch` | `cargo test -p freehand-provider-openai web_search -- --nocapture` verifies the rendered Responses body includes hosted `{"type":"web_search","external_web_access":true}` and keeps the hosted tool outside local function-tool schema | `node scripts/verify-provider-hosted-web-search-online.mjs` proves the live S-profile request truth declares hosted `web_search` and not a local Freehand function tool |
| `provider_request.render_openai_image_input_wire` | bound | `cargo test -p freehand-provider-openai image_input -- --nocapture` covers Responses and Chat Completions image wire rendering | `cargo test -p freehand-provider-openai image_input -- --nocapture` verifies data URLs contain media type/base64 while attachment id/name do not leak | `node scripts/verify-webui-image-attachment-online.mjs` proves the online WebUI current-submit contract reaches provider-neutral image input without persisting raw image history |
| `provider_response.observe_hosted_search_call` | bound | `cargo test -p freehand-provider-openai web_search -- --nocapture` covers `web_search_call` observation from both response and stream parsing | `cargo test -p freehand-provider-openai web_search -- --nocapture` verifies `web_search_call` becomes provider-hosted reasoning text and not a local tool execution | `node scripts/verify-provider-hosted-web-search-online.mjs` proves the ADP transcript retains provider-hosted web search observation in the current turn |

- lifecycle path under test:
  - semantic request renders typed input segments into `responses` or `chat completions`
  - single-shot and stream outputs normalize into shared semantic events
  - hosted `web_search` declarations render into OpenAI Responses tool wire when the semantic request carries provider-hosted search metadata
  - `web_search_call` output items re-enter as provider-hosted reasoning observations, not local tool executions
  - partial tool-call chunks accumulate until arguments become complete
  - live HTTP/SSE executor renders the selected OpenAI-compatible protocol, captures raw response/error/stream bodies, and returns only provider-neutral semantic outputs to runtime
- white-box plan:
  - request renderer, response parser, stream parser, partial tool accumulator, executor raw capture, executor HTTP status classification surface
  - Responses and Chat Completions success bodies carrying a wire-level `error: null` field must not emit `ProviderSemanticOutput::Error`
  - a non-null wire-level error object must still emit the typed provider error semantic output
  - hosted `web_search` request rendering must be adapter-owned and must not create a local `ToolCall`
- module black-box plan:
  - adapter emits provider-neutral text/tool/usage/terminal/error outputs for both OpenAI protocols
  - successful completed output with `error: null` remains semantically successful and cannot create a user-visible error projection, while a real error object remains observable
  - executor drives OpenAI-compatible responses and chat-completions requests through local mock HTTP/SSE endpoints without runtime owning wire paths
- project black-box impact:
  - reason layer can consume OpenAI semantic outputs without protocol leakage
- fixtures / replay inputs / runtime evidence paths:
  - OpenAI request/response fixtures
  - `~/.freehand/ledgers/providers/openai`
  - `~/.freehand/replays/providers/openai`
- known gaps:
  - real upstream OpenAI-compatible online behavior is verified through `provider.reason-live-bridge`; provider crate tests use local mock HTTP/SSE surfaces
- sync status between design and implementation:
  - `OpenAiAdapter` and `OpenAiExecutor` baseline implemented
  - request rendering covers `responses` and `chat completions` from typed input segments
  - single-shot and stream parsing cover text, tool calls, usage, terminal, and error paths
  - hosted `web_search` request rendering and `web_search_call` observation are now covered by focused web-search tests
- mainline/wiki sync:
  - wiki generated from mainline call must stay in sync with OpenAI adapter owner code and function map updates
