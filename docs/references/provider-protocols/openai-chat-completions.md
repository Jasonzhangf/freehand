# OpenAI Chat Completions API Snapshot

- snapshot_date: `2026-06-15`
- provider: `OpenAI`
- protocol: `Chat Completions API`
- official_sources:
  - `https://platform.openai.com/docs/api-reference/chat/create`
  - `https://platform.openai.com/docs/guides/text-generation`
  - `https://platform.openai.com/docs/guides/function-calling`

## Why It Matters For Freehand

- some OpenAI-compatible upstreams still expose chat-completions rather than `responses`
- Freehand must normalize chat-completions semantics into the same shared contracts as `responses`
- tool calls and stream deltas must remain adapter-private until mapped into shared semantic events

## Confirmed Protocol Points

### Endpoint

- create chat completion uses `POST /chat/completions`

### Input Model

- request uses `messages`
- each message has a `role`
- user text commonly appears in a `user` message
- tool declarations are passed separately from messages

### Output Model

- non-streaming response returns `choices`
- final assistant content is usually under `choices[*].message.content`
- tool invocations appear under `choices[*].message.tool_calls`
- `finish_reason` is carried per choice

### Streaming

- stream responses emit incremental `choices[*].delta`
- text deltas arrive under `delta.content`
- tool call chunks may stream through `delta.tool_calls`
- terminal chunk may carry `finish_reason`
- `[DONE]` closes the stream

### Tool Use

- tool calls carry an id, function name, and JSON arguments string
- tool argument chunks may arrive incrementally during streaming
- adapter must handle incomplete streamed arguments before final parse succeeds

## Freehand Mapping Notes

- map assistant text deltas to `SemanticEventKind::Text`
- map streamed or final tool calls to `ToolCallContract`
- keep `arguments_complete=false` until the accumulated JSON is complete and parseable
- map choice `finish_reason` into usage/terminal metadata, not final task completion truth

## Watchpoints

- do not assume chat-completions and `responses` share the same wire DTOs
- do not treat `finish_reason=stop` as Freehand turn completion
- do not leak message or chunk DTOs outside the OpenAI adapter
