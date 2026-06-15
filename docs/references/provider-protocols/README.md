# Provider Protocol References

This directory stores local protocol-reference snapshots for provider development.

## Scope

Current first-version references cover:

- OpenAI Responses API
- Anthropic Messages API
- Anthropic streaming messages
- Anthropic tool use
- Anthropic fine-grained tool streaming

## Source Policy

- every file here is derived from official provider documentation
- each file must record the official source URL
- local summaries are for fast repo search and implementation comparison
- wire DTOs and live behavior must still be verified against official docs when ambiguity exists

## Files

- `openai-responses.md`
  - request model, items vs messages, tools, stateful context, migration notes
- `anthropic-messages.md`
  - messages API shape, statelessness, stop reasons, streaming, tool-use loop

## Use Rule

- when implementing or debugging provider adapters, read this directory before inventing protocol behavior
- if implementation and official reference disagree, update the code or escalate the ambiguity
- if official docs change in a meaningful way, refresh these snapshots in the same task that updates provider truth
