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

### Semantic Difference From Chat Completions

- `responses` uses `items`, not only message arrays
- function/tool actions are distinct items instead of being glued into a single chat message structure
- migration guide explicitly says `responses` is the new API primitive and recommended for new projects

### Reasoning / Context

- migration guide highlights better support for reasoning models
- stateful context can preserve reasoning and tool context across turns
- encrypted reasoning is mentioned as an opt-out statefulness path

### Streaming

- response retrieval docs indicate streaming support through event sequences when enabled
- `provider.semantic` should treat raw stream events as adapter-private and only emit unified semantic events outward

## Freehand Mapping Notes

- map text-bearing response output into `SemanticEventKind::Text`
- map reasoning-bearing output into `SemanticEventKind::Reasoning`
- map tool invocation items into `ToolCallContract`
- map tool outputs re-entering later turns into `ReasonReq05ToolResultReentry`
- preserve raw events only in debug-mode retention

## Watchpoints

- do not model OpenAI `responses` as plain chat messages only
- do not leak item-level or wire-level DTOs outside the OpenAI adapter
- when docs and observed payloads diverge, keep raw evidence and update adapter references
