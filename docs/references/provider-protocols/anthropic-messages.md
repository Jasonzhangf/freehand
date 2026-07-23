# Anthropic Messages API Snapshot

- snapshot_date: `2026-06-15`
- provider: `Anthropic`
- protocol_family: `Messages API`
- official_sources:
  - `https://docs.anthropic.com/en/api/messages`
  - `https://docs.anthropic.com/en/api/messages-streaming`
  - `https://docs.anthropic.com/en/docs/build-with-claude/tool-use/overview`
  - `https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/web-search-tool`
  - `https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/fine-grained-tool-streaming`
  - `https://docs.anthropic.com/en/docs/intro-to-claude`

## Why It Matters For Freehand

- Anthropic side of `provider.semantic` must normalize Messages API semantics into the same shared contracts used for OpenAI `responses`
- tool-use and streaming behaviors must be mapped without leaking Anthropic block shapes to upper layers

## Confirmed Protocol Points

### Core Model

- Messages API is Anthropic's direct prompting API for custom agent loops
- Messages API is stateless; caller sends full conversational history on each request
- docs describe message structure, system prompts, and stop reasons as core concepts

### Streaming

- set `stream: true` to receive server-sent events
- SDKs can accumulate a final message while streaming
- stream path and final accumulated message are both supported modes

### Tool Use

- Claude can return structured tool-use blocks
- client tools run in caller infrastructure
- server tools run in Anthropic infrastructure
- hosted web search is a server tool, not a caller-executed function tool
- Freehand renders provider-neutral hosted search as Anthropic Messages
  `{"type":"web_search_20250305","name":"web_search","max_uses":5}` in
  the adapter-owned `tools` array
- hosted search response blocks use server-side shapes such as
  `server_tool_use` and `web_search_tool_result`; these are observations of
  provider-side work, not local `tool_use` calls for Freehand to execute
- for client-side tool loops, docs explicitly describe:
  - Claude returns `stop_reason: "tool_use"`
  - application executes the operation
  - application sends back a `tool_result`

### Tool Choice

- default tool choice is auto
- prompting can influence tool usage
- explicit `tool_choice` can harden behavior when needed

### Fine-Grained Tool Streaming

- `eager_input_streaming: true` enables earlier streaming of tool parameters
- partial or invalid JSON may appear in streamed tool input
- when `max_tokens` truncates output, tool input may end incomplete
- `provider.semantic` must therefore support partial tool-call semantics

## Freehand Mapping Notes

- map normal text deltas to `SemanticEventKind::Text`
- map thinking/reasoning-like progress to `SemanticEventKind::Reasoning` when exposed semantically
- map structured `tool_use` blocks to `ToolCallContract`
- map hosted `server_tool_use` / `web_search_tool_result` blocks to
  provider-hosted reasoning observations
- map caller-supplied `tool_result` continuation to `ReasonReq05ToolResultReentry`
- treat SSE event shapes and block DTOs as adapter-private

## Watchpoints

- do not assume Anthropic is stateful like OpenAI conversations
- do not assume streamed tool input is always valid complete JSON
- do not collapse `tool_use` stop reasons into final task completion
- do not expose hosted `web_search` as a Freehand function tool; adapter-owned
  Messages wire must be rendered only from provider-neutral hosted tool
  metadata
