---
name: provider-protocols
description: Use when working on Freehand provider adapters, Responses API support, Anthropic Messages API support, streaming/tool-call mapping, or protocol-spec comparisons. Reads local official-reference snapshots before implementation or debugging provider behavior.
---

# Provider Protocols

Use this skill when implementing or debugging provider protocol behavior in Freehand.

## Start

1. Read `docs/references/provider-protocols/README.md`.
2. Choose the matching local reference:
   - OpenAI Responses: `docs/references/provider-protocols/openai-responses.md`
   - OpenAI Chat Completions: `docs/references/provider-protocols/openai-chat-completions.md`
   - Anthropic Messages: `docs/references/provider-protocols/anthropic-messages.md`
3. Compare implementation against the local snapshot before inventing protocol behavior.
4. If behavior is still ambiguous, verify against the official source URL listed in the snapshot.

## Rules

- treat local reference files as the fast search surface, not as a replacement for official docs
- keep provider wire DTOs inside provider adapters
- map only unified semantic events into shared contracts
- if protocol behavior changes implementation truth, update:
  - provider design docs
  - provider test design
  - provider function map
  - local reference snapshot

## What To Check

- request shape:
  - stateful vs stateless
  - items/messages/content structure
  - tool declaration shape
- stream shape:
  - text deltas
  - reasoning progress
  - tool-call chunks
  - terminal events
- tool loop:
  - tool call emission
  - tool result re-entry
  - partial/incomplete tool input handling
- error and retention:
  - retry hints
  - stream interruption
  - raw-event retention boundaries

## Validation Reminder

- after changing provider behavior, run mapped provider tests and the repo gate
- when local snapshots lag official docs, update the snapshot in the same task
