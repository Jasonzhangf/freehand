# Debug And Observability Design

## Status

Confirmed discussion with partial implementation landed. Remaining missing details stay `TBD`.

## Confirmed

### Debug starting point

- debugging starts from function map
- first locate `feature_id`
- then confirm owner, `debug_artifacts`, `runtime_paths`
- only then move into code and runtime evidence

### Two-coordinate debug rule

Every important failure investigation should preserve:

- semantic position
  - `feature_id`
  - `session_id`
  - `turn_id`
  - pipeline node id
- scene position
  - crate
  - file
  - function
  - artifact path
  - raw exchange id

### Runtime evidence home

Runtime home is `~/.freehand`.

Confirmed standard directories:

- `~/.freehand/state`
- `~/.freehand/logs`
- `~/.freehand/ledgers`
- `~/.freehand/replays`
- `~/.freehand/cache`
- `~/.freehand/tmp`

### Evidence preference

- logs are hints
- replay fixtures and ledgers are stronger truth
- runtime debug paths should be documented before use
- if truth changes during debug, docs and skill workflow must be updated in same task

### Debug module boundary

- `debug.core` should be an independent module
- any module may import and emit debug/trace material through it
- `debug.core` does not own request truth
- `debug.core` does not own session truth
- `debug.core` does not own provider semantic truth
- UI may consume debug projections through protocol-owned wrappers, but UI does not own debug truth

### Landed baseline

- `crates/freehand-debug` now owns shared debug snapshot, trace envelope, hub, subscriber fanout, and stdout/file sink primitives
- `freehand-reason` now emits per-turn lifecycle debug observations into `debug.core`
- `freehand-ui-protocol` consumes `freehand-debug::DebugStateSnapshot` directly instead of defining a duplicate DTO

## Open Questions / TBD

- exact ledger format
- exact replay file format
- exact trace envelope schema
- exact retention and cleanup rules
- exact online/offline replay workflow

## Landed Follow-up

- sink-dispatch failures now surface through a dedicated observation-failure stream in `debug.core`
- producing modules may observe those failures without promoting them into request/session/reason truth

## Update trigger

Update this doc when:

- debug entry flow changes
- runtime evidence directories change
- ledger/replay design changes
- trace schema changes
