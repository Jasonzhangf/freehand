# System Architecture Overview

## Status

This document records only design points already confirmed in discussion.

Unknown details stay `TBD`.

## Confirmed

### Core goals

- Rust-first implementation
- reasoning and UI split
- multi-UI access against one truth source
- master/slave topology is part of target architecture
- module isolation uses `contracts + blocks + orchestrators`
- development and debugging start from function map and owner lookup

### Layering

- `freehand-contracts`
  - global semantic type truth
- `freehand-blocks`
  - reusable pure builders, parsers, validators, projectors
- `freehand-provider-*`
  - provider adapters and provider wire DTOs only
- `freehand-reason`
  - turn orchestration and reasoning event emission
- `freehand-node`
  - master/slave runtime and node protocol
- `freehand-ui-protocol`
  - UI-facing commands, projections, event surface
- `freehand-gates`
  - architecture and workflow gate enforcement
- `freehand-testkit`
  - replay helpers, fixtures, mocks

### Boundary rules

- new helper or semantic logic should be searched in existing libraries first
- orchestrator crates are pure orchestration, not helper libraries
- if reusable logic is missing, add it to `freehand-blocks`
- UI must not depend directly on provider crates
- provider-specific wire DTOs must not leak outside provider crates
- no fallback, no silent downgrade

### Source-of-truth routing

- project entry router: `AGENTS.md`
- feature owner truth: `docs/architecture/feature-map.md`
- detailed workflow truth: `docs/architecture/` and `docs/design/`
- runtime scene evidence: `~/.freehand`

## Open Questions / TBD

- exact master/slave transport protocol
- exact multi-agent `config.toml` schema
- exact persistence format for session truth
- exact API surface for multi-UI command submission
- exact crate-level public API boundaries beyond current scaffold

## Confirmed Master/Slave Meaning

- master/slave is an input-permission configuration problem
- local agents are managed through `config.toml`
- one `config.toml` may define multiple local agents
- each agent has its own startup configuration entry
- config source path is `~/.freehand/config.toml`
- multi-agent layout uses `[agents.<name>]`
- startup configuration decides how that agent starts
- whichever side is configured as `master` is the side that receives user input and dispatches work
- `master` dispatches work to:
  - local sub-agents
  - remote slave agents
- `slave` is a task-receiving mode
- if startup configuration selects `slave` mode, the config includes:
  - `name`
  - `mode`
  - pairing token
- `allowed_pair_ip` is optional, and when omitted no source IP filter is applied
- after successful pairing, `slave` executes paired input only
- paired input may come from:
  - a user
  - another `master`
- `slave` does not accept unrelated direct input from other sources while in paired slave mode

## Update trigger

Update this doc when:

- crate responsibility changes
- boundary rules change
- orchestration ownership changes
- source-of-truth routing changes
