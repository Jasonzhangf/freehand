# Contracts Core Design

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

## Confirmed

### Scope

`contracts.core` covers cross-module shared semantic types.

Included:

- shared semantic request chain types
- shared semantic response/event chain types
- shared semantic tool-call and tool-result re-entry types
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

## Open Questions / TBD

- exact field expansion beyond the current minimal shared request/response/error/tool contracts
- exact relationship between semantic error nodes and module error base contracts
- exact serialization format policy
- exact compatibility/versioning policy for contract evolution

## Update trigger

Update this doc when:

- contract scope changes
- shared chains change
- ID system changes
- error contract policy changes
- serialization requirements change
