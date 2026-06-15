# Contracts Core Design

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

## Confirmed

### Scope

`contracts.core` covers cross-module shared semantic types.

Included:

- shared semantic request chain types
- typed context segment contracts
- shared semantic response/event chain types
- shared semantic tool-call and tool-result re-entry types
- structured JSON-capable tool arguments
- shared semantic reasoning-process node types
- shared cross-module error contracts
- module-level error base contracts
- shared ID types

Excluded:

- runtime config schema
- UI projection types
- debug/trace envelope

### Pipeline node naming

Pipeline node naming is locked to:

- direction + node number + node meaning

### Request-side chain

The request side should be explicitly split into nodes.

Confirmed request-side semantic stages:

- user raw input
- context-composed input
- provider payload
- tool call
- tool result re-entry
- error
- reasoning-process nodes

First-version context-planning contracts now also include:

- typed `ContextSegment`
- segment kind
- segment stability
- segment cache policy
- segment role
- provenance

Request-chain direction is now:

- raw user input
- typed context-composed input
- provider payload carrying typed input segments

### Response/event-side chain

The response/event side should be explicitly split into nodes.

Confirmed response/event stages:

- provider raw event
- semantic event

UI projection is intentionally outside `contracts.core` and stays in `freehand-ui-protocol`.

### ID system

The first version locks these shared IDs:

- `agent_id`
- `session_id`
- `turn_id`
- `trace_id`
- `feature_id`

### Error strategy

`contracts.core` defines:

- cross-module error contracts
- module-level error base contracts

### Serialization boundary

`contracts.core` types must default to all of:

- serializable
- replayable
- persistable

### Current shared tool and usage boundary

- tool arguments preserve structured JSON values instead of flattening everything to strings
- usage contracts may carry provider-supplied `total_tokens`, `reasoning_tokens`, and `finish_reason`
- cache counters remain explicit so replay and cache-hit calculations do not depend on provider-specific DTOs

### Metadata/request separation baseline

`contracts.core` now locks request-content-side truth further:

- request contracts use typed context segments instead of ad hoc `source/content` pairs
- provider payload semantic contract carries `input_segments`, not one mixed rendered string field
- metadata/debug envelope remains outside `contracts.core`

## Open Questions / TBD

- exact compatibility/versioning policy for contract evolution
- future cache-shape contract split if it must become cross-module truth

## Update trigger

Update this doc when:

- contract scope changes
- shared chains change
- ID system changes
- error contract policy changes
- serialization requirements change
