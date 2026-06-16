# Runtime Command Dispatch Design

## Status

First implementation baseline.

## Purpose

`runtime.ui-command-dispatch` is the runtime wiring owner for UI command ingress.

It exists so:

- apps can stay protocol-only
- `freehand-ui-protocol` can stay contract-only
- real command dispatch can still reach owner modules such as `freehand-reason` and `freehand-node`

## Ownership

- owner crate: `crates/freehand-runtime`
- feature id: `runtime.ui-command-dispatch`

`runtime.ui-command-dispatch` owns:

- UI command dispatch port implementations
- command envelope to owner-adapter wiring
- runtime-owned in-memory composition of reason/node owners for command execution smoke
- config-selected bootstrap from the default config path into one runtime dispatcher
- runtime-owned provider live bridge for config-selected Anthropic `messages` turns
- derived UI projection updates caused by runtime-owned dispatch

It does not own:

- UI command/query/subscribe contracts
- reason turn truth semantics
- node master/slave semantics
- provider semantics

## First-Version Scope

First baseline dispatches:

- `SubmitUserInput` -> runtime-owned provider live bridge -> `reason.turn`
- `CancelTurn` -> `reason.turn`
- `ResumeTurn` -> explicit unsupported runtime error
- `SendDirectMessageToSlave` -> `node.master-slave`

## Boundary Rules

- apps must call a dispatch port, not reason/node owners directly
- `freehand-ui-protocol` declares owner routing but does not execute owner semantics
- `freehand-runtime` may compose reason/node owners, but must not redefine their semantics
- turn truth remains inside `freehand-reason`
- node direct-message semantics remain inside `freehand-node`
- provider wire DTOs remain inside provider adapter crates
- runtime may compose provider executor + reason owner + persistence, but must not redefine provider wire semantics or reason truth semantics

## First Runtime Model

The first baseline is in-memory only:

- one `SessionHistory`
- one `ReasonTurnEngine`
- one `LocalNodeRuntime`
- one derived `UiProtocolState`

Current extension:

- runtime may now bootstrap from one selected agent in `~/.freehand/config.toml`
- runtime consumes config-selected peer topology from `config.core`
- local master node id, paired slave node id, paired allowed IP, and paired token env are selected from config instead of being derived synthetically
- runtime submit-user-input now runs provider-backed live execution with completion schema, tool re-entry, persistence, and UI projection
- live bootstrap restores persisted turn projections into runtime-owned `UiProtocolState` and resumes turn-id allocation from persisted runtime turn ordinals

This is a runtime wiring baseline, not a production daemon design.

## Update Trigger

Update this doc when:

- command-to-owner routing changes
- runtime wiring moves from in-memory to daemon/process boundaries
- runtime dispatch failure classification changes
- app/runtime injection boundary changes
