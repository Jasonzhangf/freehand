# Provider Semantic Design

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

## Confirmed

### Provider scope

First version formally supports:

- OpenAI-compatible providers
- Anthropic providers

OpenAI-compatible first-version protocol support must explicitly include:

- `responses`
- `chat completions`

### Provider interface shape

First version requires both:

- single request/response
- streaming event interface

### Minimum unified semantic outputs

The unified provider semantic layer must cover:

- text output
- reasoning progress
- tool call
- tool result re-entry
- usage and token accounting
- terminal status
- error
- model/provider metadata

### Reasoning progress semantics

Reasoning progress uses two layers:

- raw provider reasoning/thinking events preserved when available
- unified semantic reasoning events for the rest of the system

### Tool call contract

First version requires all of:

- tool call id
- tool name
- structured args
- partial tool-call stream
- tool result re-entry into later request flow

### Usage semantics

First version usage must cover:

- input tokens
- output tokens
- total tokens
- reasoning tokens
- cache hit
- cache miss
- cache hit rate
- finish reason

### Error contract

First version error contract must classify:

- auth
- rate limit
- upstream unavailable
- malformed payload / protocol
- stream interrupted
- unsupported capability
- user/config error

Errors are also classified by recovery type:

- recoverable
- unrecoverable
- periodic-recoverable

For periodic-recoverable errors:

- period unit is seconds
- provider-supplied period takes priority when present
- otherwise use configurable default windows

Confirmed default periodic windows:

- half hour
- five hours
- daily midnight

### Capability model

Default provider expectation is broad support.

First version must explicitly declare these capabilities:

- web search
- multimodal
- vision
- reasoning

### Model selection responsibility

- model/provider selection belongs to `freehand-config`
- provider semantic layer validates and executes, but is not the selection source of truth

### Raw event retention

- raw provider events are retained in debug mode
- outside debug mode, raw provider events are not retained long-term

### Payload boundary

- provider payload wire DTOs stay private to provider adapters
- `contracts.core` holds semantic request nodes, not provider wire payload structs
- `responses` protocol wire events and DTOs remain adapter-private even when their semantic output is supported system-wide
- provider semantic request now consumes typed `input_segments` from `contracts.core` rather than a pre-mixed rendered prompt string

### Provider / reason independence boundary

- provider core owns provider-neutral semantic request/output contracts
- provider adapters own protocol rendering/parsing only
- provider adapters must not depend on `freehand-reason`
- `freehand-reason` must not depend on provider adapter crates
- provider adapters must not write turn truth, session truth, completion truth, or UI projection truth
- provider stop reasons and finish reasons are metadata/usage signals, not Freehand terminal decision truth

### Metadata/request hard isolation

- provider metadata belongs in explicit provider descriptor, config, event context, usage, or debug envelope types
- request content belongs in request-chain nodes only
- provider renderers must not infer user/task content from metadata
- provider renderers must not write provider/model/debug metadata into prompt text unless an explicit context builder produced that text as request data
- raw provider events are debug ledger material, not request-chain material
- adapter renderers may render typed context segments into provider-specific wire text, but they do not own segment admission or ordering truth

### Provider registration

First version target direction is:

- runtime plugin/provider loading

## Open Questions / TBD

- exact provider interface signatures
- exact capability declaration schema
- exact raw-event retention trigger and storage policy
- exact mapping from provider-specific recovery hints into periodic-recoverable defaults
- exact runtime plugin loading protocol and trust boundary
- exact provider metadata envelope type names

## Update trigger

Update this doc when:

- supported provider families change
- unified semantic outputs change
- capability model changes
- error recovery policy changes
- raw retention policy changes
- provider registration model changes
