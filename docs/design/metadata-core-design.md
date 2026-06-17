# Metadata Core Design

## Status

Baseline owner and contract are implemented.

## Purpose

`metadata.core` is the central owner for internal control/provenance metadata.

It exists to make operational facts answerable at the failure site:

- who wrote the metadata
- which pipeline node wrote it
- which trace/session/turn it belongs to
- what control/provenance facts were written

## Owner

- feature_id: `metadata.core`
- owner crate: `crates/freehand-metadata`
- owner module: `crates/freehand-metadata/src/lib.rs`
- function map: `docs/function-maps/metadata.core.md`
- test design: `docs/testing/metadata.core.md`

## Boundary

Metadata is not request data.

Metadata may carry:

- control state
- routing provenance
- provider/model metadata
- cache metadata
- debug artifact links
- runtime state markers
- trace/session/turn identity
- writer owner and writer node identity

Metadata must not carry:

- user prompt text
- rewritten prompt text
- provider request payloads
- message arrays
- context segment content
- tool result content
- any field that becomes a fallback source for request-chain content

## Required Provenance

Every metadata envelope must include:

- `MetadataWriteOwner.feature_id`
- `MetadataWriteOwner.crate_name`
- `MetadataWriteOwner.module_path`
- `MetadataWriteOwner.symbol_path`
- `MetadataWriteNode.pipeline_node`
- `MetadataSubject.trace_id`

`MetadataWriteNode.runtime_node_id` is optional because some metadata is written before a runtime master/slave node exists. The pipeline node is mandatory.

## Request Separation

Request-chain content stays in request node contracts, for example:

- `ReasonReq01UserRawInput`
- `ReasonReq02ContextComposedInput`
- `ReasonReq03ProviderPayload`
- `ReasonReq04ToolCall`
- `ReasonReq05ToolResultReentry`

Metadata center validates against request-like metadata keys including:

- `request`
- `payload`
- `prompt`
- `message`
- `messages`
- `input`
- `content`
- `text`

The key guard is not a semantic payload scanner. It is the first hard gate against known request-content pollution. Runtime integration must still use typed request nodes and metadata envelopes as separate arguments.

## Debug Relationship

`debug.core` and `metadata.core` are separate:

- debug owns observation envelopes, snapshots, hubs, and sinks
- metadata owns internal control/provenance records
- debug may later point to metadata ids or artifacts
- metadata admission must not depend on debug sinks
- debug records must not become metadata write truth unless explicitly converted by the metadata owner

## Runtime Persistence Direction

Future durable metadata ledgers should live under:

- `~/.freehand/ledgers/metadata`
- `~/.freehand/replays/metadata`

This baseline implements only validated in-memory admission. Persistent ledger ownership is intentionally not claimed yet.

## Update Trigger

Update this doc when:

- metadata envelope fields change
- writer owner or write-node provenance changes
- metadata/request isolation policy changes
- runtime producers start writing metadata
- metadata ledger persistence is implemented
