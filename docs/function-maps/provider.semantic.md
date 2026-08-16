# Function Map: `provider.semantic`

- feature_id: `provider.semantic`
- owner crate: `crates/freehand-provider-core`
- owner module: `crates/freehand-provider-core/src/lib.rs`
- mainline call source: `docs/mainline-calls/provider.semantic.json`
- generated wiki: `docs/wiki/provider.semantic.md`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `provider_hosted_search.declare`
  - `provider_hosted_search.project_candidate`
- owner entry symbols:
  - `build_semantic_request`
  - `ProviderToolDefinition`
  - `ProviderHostedToolDefinition`
  - `ProviderWebSearchCapability`
  - `ProviderToolChoice`
  - `ProviderToolExchange`
  - `map_adapter_event`
  - `map_adapter_events`
  - `project_hosted_search_discovery`
  - `classify_provider_error`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `provider_response`
  - `provider_hosted_search`
- touched resources:
  - `provider_request`
  - `search_evidence`
- resource operations:
  - `provider_hosted_search.declare` (`provider_hosted_search` -> `provider_request`)
  - `provider_hosted_search.project_candidate` (`provider_hosted_search` -> `search_evidence`)
- forbidden shortcuts:
  - Provider-hosted search must stay provider-neutral metadata on `ProviderSemanticRequest.hosted_tools`; it must not be exposed as a local Freehand function tool or executed by `tool.registry`.
  - Adapter-specific hosted search wire DTOs must not leak into `freehand-provider-core`.

## Request Mainline

- normalized provider request enters provider semantic boundary
- provider semantic request must validate the typed provider payload contract before adapter rendering
- OpenAI-compatible request path explicitly supports `responses`
- OpenAI-compatible request path explicitly supports `chat completions`
- provider-specific adapters render wire payloads without leaking adapter DTOs outside adapter crates
- provider semantic request must stay provider-neutral
- provider metadata and request content must stay separate types
- provider semantic request may carry provider-neutral tool metadata as `ProviderToolDefinition`, `ProviderToolChoice`, and `ProviderToolExchange`; these are not request text and must be rendered only by adapter owners
- provider semantic request may carry provider-neutral hosted tool metadata as `ProviderHostedToolDefinition`; OpenAI Responses and Anthropic Messages adapters can render that as provider-hosted `web_search`, while protocol-unsupported providers leave it absent
- `freehand-provider-core` may bridge reason to provider, but must not import `freehand-reason` implementation truth

## Response Mainline

- provider raw stream or single-shot output becomes unified semantic events
- semantic output carries text, reasoning, tool, usage, terminal, and error semantics
- provider-hosted search observations enter normal semantic reasoning output, not local `ToolCall` execution truth
- hosted result candidates with original URLs additionally enter typed `SearchDiscoveryDelivery`; hosted snippets never enter verified truth
- tool-use output maps to shared `ReasonReq04ToolCall`; tool-result continuation maps to shared `ReasonReq05ToolResultReentry`
- provider stop/finish signals remain metadata/usage signals until `freehand-reason` decides terminal truth

## Error Mainline

- provider errors are classified into unified error contracts
- periodic-recoverable errors preserve recovery windows in seconds
- debug/raw retention stays separate from normal semantic output
- metadata/request boundary confusion is architecture-invalid and should be blocked by future gate work

## Shared Multi-Reference Functions

- `classify_provider_error`
  - owner: `crates/freehand-provider-core/src/lib.rs`
  - purpose: unify provider failures into shared recovery/error contract
  - allowed callers: provider adapters, tests
  - related tests: periodic recovery classification tests
  - why shared: keeps recovery policy centralized instead of duplicated per adapter
- `map_adapter_events`
  - owner: `crates/freehand-provider-core/src/lib.rs`
  - purpose: map one provider-parser output batch into shared semantic outputs
  - allowed callers: provider adapters, tests
  - related tests: openai/anthropic adapter parser tests
  - why shared: keeps event-batch normalization centralized instead of each adapter hand-looping output conversion

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `build_semantic_request` | `crates/freehand-provider-core/src/lib.rs` | build semantic provider request and retention policy | typed provider payload + debug flag | provider semantic request | reason/orchestrator | provider core boundary | bound |
| 02 | `ProviderToolDefinition` | `crates/freehand-provider-core/src/lib.rs` | carry provider-neutral tool schema metadata outside request text | tool name/description/input schema | adapter-renderable tool metadata | live bridge/tests | provider semantic request | bound |
| 03 | `ProviderToolExchange` | `crates/freehand-provider-core/src/lib.rs` | carry provider-neutral tool call/result continuation outside request text | tool call + tool result re-entry | adapter-renderable tool continuation | live bridge/tests | provider semantic request | bound |
| 04 | `map_adapter_event` | `crates/freehand-provider-core/src/lib.rs` | map normalized adapter event into shared semantic output | normalized adapter event | semantic output | adapter runtime | semantic mapper | bound |
| 05 | `map_adapter_events` | `crates/freehand-provider-core/src/lib.rs` | map normalized adapter event batch into shared semantic outputs | normalized adapter event batch | semantic output batch | adapter runtime | semantic mapper | bound |
| 06 | `classify_provider_error` | `crates/freehand-provider-core/src/lib.rs` | classify provider failure into shared error contract | provider error hint | unified error contract | adapter/runtime | error classifier | bound |
| 07 | `ProviderHostedToolDefinition` | `crates/freehand-provider-core/src/lib.rs` | carry provider-neutral hosted tool declarations outside request text and outside local tool execution | provider hosted capability selection | adapter-renderable hosted tool metadata | live bridge/tests | provider semantic request | bound |
| 08 | `project_hosted_search_discovery` | `crates/freehand-provider-core/src/lib.rs` | project normalized provider-hosted search observation into a typed `SearchDiscoveryDelivery` side-channel | `ProviderHostedSearchDiscovery` | typed `SearchDiscoveryDelivery` | provider adapters | provider semantic discovery | bound |

## Sync Status Against Code

- semantic request builder, single-event mapper, batch mapper, and error classifier are bound in code
- semantic request builder now consumes validated `input_segments` payload contract before adapter rendering
- provider-neutral tool schema, tool choice, and tool exchange metadata are bound on `ProviderSemanticRequest`
- provider-neutral hosted tool metadata is bound on `ProviderSemanticRequest.hosted_tools`
- provider semantic layer is independent from provider adapter implementation details and from `freehand-reason` implementation crate
- metadata/request hard isolation is required architecture truth but still needs dedicated type/gate closeout
- the generated wiki must be regenerated from `docs/mainline-calls/provider.semantic.json` when this function-map truth changes
