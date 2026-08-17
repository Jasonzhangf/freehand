# OpenAI Responses API Snapshot

- snapshot_date: `2026-06-15`
- provider: `OpenAI`
- protocol: `Responses API`
- official_sources:
  - `https://developers.openai.com/api/reference/resources/responses/methods/create/`
  - `https://developers.openai.com/api/docs/guides/migrate-to-responses`
  - `https://developers.openai.com/api/reference/responses/overview/`

## Why It Matters For Freehand

- OpenAI side of `provider.semantic` must explicitly support `responses`, not assume legacy chat-completions shape
- `responses` uses item-oriented input/output semantics
- tool use, reasoning, multimodal input, and stateful continuation are first-class in this protocol

## Confirmed Protocol Points

### Endpoint

- create response uses `POST /responses`

### Input Model

- request input may be:
  - a plain string
  - an item list
- item content may include:
  - text
  - image
  - file-linked content
- role hierarchy includes `developer`, `system`, `user`, `assistant`

### Stateful Flow

- conversations can be attached so prior items are prepended automatically
- responses can be used as input to later responses for multi-turn workflows
- migration guide positions `responses` as the recommended API for new projects

### Tools

- protocol supports:
  - custom function calling
  - built-in tools such as web search and file search
  - remote MCP-related integrations according to the migration guide
- one request can contain an agentic loop with multiple tool interactions
- local Codex source reference confirms hosted web search is a Responses API
  tool, not a normal function tool:
  - `/Users/fanzhang/code/codex/codex-rs/tools/src/tool_spec.rs`
    serializes `ToolSpec::WebSearch` as `{"type":"web_search", ...}`
  - `/Users/fanzhang/code/codex/codex-rs/core/src/tools/hosted_spec.rs`
    maps `WebSearchMode::Live` to `external_web_access=true`, `Cached` to
    `external_web_access=false`, and `Indexed` to
    `external_web_access=true` plus `indexed_web_access=true`
  - `/Users/fanzhang/code/codex/codex-rs/tools/src/tool_spec_tests.rs`
    locks optional fields: `indexed_web_access`, `filters`,
    `user_location`, `search_context_size`, and `search_content_types`

### Hosted Web Search Output

- Responses emits hosted search activity as output items with
  `type="web_search_call"`, not as local function calls.
- Local Codex source reference:
  - `/Users/fanzhang/code/codex/codex-rs/core/tests/common/responses.rs`
    uses SSE events `response.output_item.added` and
    `response.output_item.done` carrying item type `web_search_call`
  - `/Users/fanzhang/code/codex/codex-rs/protocol/src/models.rs`
    documents `web_search_call` with `id`, optional `status`, and optional
    `action`
  - `/Users/fanzhang/code/codex/codex-rs/core/src/event_mapping_tests.rs`
    maps action variants `search`, `open_page`, `find_in_page`, and partial
    no-action calls into web-search observation items
- Freehand mapping rule: `web_search_call` must become provider-hosted
  reasoning/observation semantics. It must not become
  `ProviderSemanticOutput::ToolCall`, because there is no local Freehand
  `web_search` tool to execute.

### Semantic Difference From Chat Completions

- `responses` uses `items`, not only message arrays
- function/tool actions are distinct items instead of being glued into a single chat message structure
- migration guide explicitly says `responses` is the new API primitive and recommended for new projects

### Reasoning / Context

- migration guide highlights better support for reasoning models
- stateful context can preserve reasoning and tool context across turns
- encrypted reasoning is mentioned as an opt-out statefulness path

### Usage And Cache

- Responses usage reports `input_tokens` as the total input count and cache detail inside `input_tokens_details`
- OpenAI-compatible providers may name read/write cache details `cached_tokens`, `cached_read_tokens`, `cached_write_tokens`, `cache_read_tokens`, or `cache_write_tokens`; the adapter normalizes these aliases without adding them again to total input

### Streaming

- response retrieval docs indicate streaming support through event sequences when enabled
- `provider.semantic` should treat raw stream events as adapter-private and only emit unified semantic events outward

## Freehand Mapping Notes

- map text-bearing response output into `SemanticEventKind::Text`
- map reasoning-bearing output into `SemanticEventKind::Reasoning`
- map tool invocation items into `ToolCallContract`
- map tool outputs re-entering later turns into `ReasonReq05ToolResultReentry`
- treat an optional `error` member with JSON null as absent; only a non-null error object maps to provider error semantics
- preserve raw events only in debug-mode retention

## Watchpoints

- do not model OpenAI `responses` as plain chat messages only
- completed responses may carry `error: null`; field presence alone is not failure evidence
- do not leak item-level or wire-level DTOs outside the OpenAI adapter
- do not expose hosted `web_search` as a Freehand function tool; adapter-owned
  Responses wire must be rendered only from provider-neutral hosted tool
  metadata
- when docs and observed payloads diverge, keep raw evidence and update adapter references
