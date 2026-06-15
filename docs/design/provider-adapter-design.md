# Provider Adapter Design

## Status

First baseline for provider adapters is now implemented as protocol renderers and parsers, not as live HTTP clients.

## Owner Boundaries

- `crates/freehand-provider-openai`
  - owns OpenAI `responses` and `chat completions` wire rendering/parsing through `OpenAiAdapter`
- `crates/freehand-provider-anthropic`
  - owns Anthropic Messages wire rendering/parsing through `AnthropicAdapter`
- `crates/freehand-provider-core`
  - owns provider-neutral semantic request/event/output contracts only

## First-Version Scope

- render semantic request into protocol-specific request body
- parse single-shot response bodies
- parse streaming event bodies
- normalize text, reasoning, tool call, usage, terminal, and error semantics

Out of scope for this baseline:

- live HTTP execution
- credential injection
- retry loop runtime
- persisted raw ledgers

## Shared Rules

- wire DTO structs stay private to each provider adapter crate
- partial streamed tool arguments may emit `arguments_complete=false`
- once accumulated JSON becomes valid, adapter emits structured tool arguments
- `finish_reason` and stop reasons are semantic metadata, not Freehand turn completion truth

## Update Trigger

Update this doc when:

- supported protocols change
- adapter ownership changes
- request rendering rules change
- stream event accumulation rules change
