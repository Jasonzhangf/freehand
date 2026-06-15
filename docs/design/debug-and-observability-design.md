# Debug And Observability Design

## Status

Confirmed discussion only. Missing implementation details remain `TBD`.

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

## Open Questions / TBD

- exact ledger format
- exact replay file format
- exact trace envelope schema
- exact retention and cleanup rules
- exact online/offline replay workflow

## Update trigger

Update this doc when:

- debug entry flow changes
- runtime evidence directories change
- ledger/replay design changes
- trace schema changes

