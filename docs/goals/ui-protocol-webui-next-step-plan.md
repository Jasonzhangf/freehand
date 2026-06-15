# UI Protocol and WebUI Next Step Plan

## Goal

Lock `ui.protocol` as the single truth source for CLI and WebUI, then use it to drive a minimal WebUI that can query and subscribe to live turn truth without duplicating reason or provider logic.

This next step is protocol-first, not presentation-first.

## Acceptance

The goal is accepted only when all of the following are true:

1. `ui.protocol` remains the single shared truth for query/subscribe/projection behavior.
2. CLI and WebUI both consume the same protocol contracts and source identity fields.
3. Query and subscribe stay separate.
4. WebUI can display:
   - current terminal text projection
   - latest active turn
   - slave turn as a separate WebUI card
5. At least one black-box path proves protocol query + subscribe behavior from the UI boundary.
6. Workspace gates and `ui.protocol`-mapped tests pass.

## Scope

### In Scope

- `ui.protocol` schema and routing truth
- CLI projection through `ui.protocol`
- minimal WebUI consumption of `ui.protocol`
- source identity fields and stream kind routing
- latest active turn and explicit turn subscription
- slave turn separate-card projection in WebUI

### Out of Scope

- reason engine changes
- provider adapter changes
- master/slave runtime protocol redesign
- visual polish beyond a minimal functional page
- multi-client sync semantics beyond existing query/subscribe truth

## Design Principles

1. Protocol first. UI must follow `ui.protocol`, not invent new truth.
2. Query and subscribe remain separate primitives.
3. Source identity stays explicit on every path.
4. WebUI may render differently from CLI, but must not change protocol truth.
5. No fallback. Missing or invalid projection must fail explicitly.
6. Shared projection logic must remain in `freehand-ui-protocol`, not duplicated in UI adapters.

## Existing Truth To Reuse

- `crates/freehand-ui-protocol/src/lib.rs`
  - command validation
  - query
  - subscription selection/matching
  - turn projection
  - terminal text projection
- `docs/function-maps/ui.protocol.md`
- `docs/testing/ui.protocol.md`
- `crates/freehand-node/src/lib.rs`
- `crates/freehand-reason/src/lib.rs`
- `apps/freehand-cli/src/main.rs`

## Technical Plan

### 1. Lock protocol truth

Re-read and update `ui.protocol` so it remains the single truth for:

- query command shape
- subscribe command shape
- stream kinds
- source identity fields
- terminal text projection
- slave turn card projection

Likely files:

- `crates/freehand-ui-protocol/src/lib.rs`
- `docs/function-maps/ui.protocol.md`
- `docs/testing/ui.protocol.md`

### 2. Keep CLI as a protocol consumer

Make sure CLI consumes `ui.protocol` for the same projection truth that WebUI uses.

Likely files:

- `apps/freehand-cli/src/main.rs`
- `apps/freehand-cli/tests/config_startup.rs`
- `docs/function-maps/app.cli-live-turn.md`
- `docs/testing/app.cli-live-turn.md`

### 3. Add minimal WebUI consumption

Add a minimal WebUI that can:

- query the latest active turn
- subscribe to incremental updates
- render terminal text projection
- render slave turn as a separate card

Likely files:

- `crates/freehand-ui-protocol/src/lib.rs`
- `apps/freehand-webui/**` if the repo already has or will add a web UI boundary
- `docs/function-maps/ui.protocol.md`
- `docs/testing/ui.protocol.md`

### 4. Preserve explicit identity and stream kinds

The UI boundary must keep these fields explicit:

- `source_agent_id`
- `source_node_id`
- `source_turn_id`
- `stream_kind`

Likely files:

- `crates/freehand-ui-protocol/src/lib.rs`
- `crates/freehand-node/src/lib.rs`

## File List To Expect

Implementation is likely to touch:

- `crates/freehand-ui-protocol/src/lib.rs`
- `docs/function-maps/ui.protocol.md`
- `docs/testing/ui.protocol.md`
- `apps/freehand-cli/src/main.rs`
- `apps/freehand-cli/tests/config_startup.rs`
- possibly `apps/freehand-webui/**` if the WebUI boundary exists or is added

## Risks And Avoidance

### Risk: WebUI invents its own projection semantics

Avoidance:

- keep query/subscribe/projection in `ui.protocol`
- make WebUI a thin renderer over protocol truth

### Risk: CLI and WebUI drift apart

Avoidance:

- route both through the same protocol helpers and shared fixtures

### Risk: protocol truth gets mixed with reason/provider logic

Avoidance:

- keep `ui.protocol` independent from provider adapters and reason turn ownership

## Verification Matrix

### White-box

- `cargo test -p freehand-ui-protocol`
- projection and subscription routing tests
- terminal text projection tests

### Module black-box

- CLI protocol projection smoke
- WebUI protocol query smoke
- WebUI subscribe smoke
- slave turn separate-card smoke

### Project black-box

- one UI boundary consumes query/subscribe truth and renders both active turn and slave turn without duplicating reason/provider logic

### Workspace gates

- `cargo build --workspace`
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -p xtask -- gates check`

## Implementation Steps

1. Confirm the current `ui.protocol` truth and update its function map/test design if needed.
2. Add or adjust protocol projections for terminal text and slave-turn card truth.
3. Keep CLI projections aligned with protocol truth.
4. Add the minimal WebUI boundary on top of the same protocol.
5. Add black-box tests for query/subscribe separation and slave turn rendering.
6. Run workspace gates.
7. Update docs, memory, and notes if truth changes.

## Definition of Done

Done means:

- CLI and WebUI share the same `ui.protocol` truth
- query and subscribe remain separate
- WebUI can show terminal text, latest active turn, and slave turn as a separate card
- protocol tests and workspace gates pass
