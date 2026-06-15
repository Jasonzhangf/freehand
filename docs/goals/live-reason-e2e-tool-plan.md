# Live Reason E2E Tool Plan

## Goal

Deliver one minimal but real end-to-end path that proves:

- config-selected live provider can execute a real reasoning turn
- the turn can span multiple rounds under Freehand completion-schema control
- at least one tool call is executed and re-entered into the same turn
- final turn truth is persisted and queryable from `~/.freehand`

This plan targets the smallest closed-loop runtime that can be validated with a real provider. It is not the full production runtime closeout.

## Acceptance

The goal is accepted only when all of the following are true:

1. `freehand-cli reason-live --agent <name> --prompt <text>` can drive a real Anthropic-compatible `messages` provider through at least two rounds.
2. The model can emit one tool call, Freehand executes it, and the tool result is re-entered into the same logical live request.
3. The model closes through valid `<freehand_completion>...</freehand_completion>` tagged JSON, not provider `finish_reason`.
4. Live execution writes authoritative session/turn truth through `ReasonPersistence`.
5. Restart or second invocation can restore the session and continue from persisted truth.
6. Targeted white-box, module black-box, project black-box, workspace build/lint/test/gate, and one live smoke all pass.

## Scope

### In Scope

- Anthropic-compatible `messages` live path only
- one simple deterministic read-only tool
- multi-round `continue` loop
- tool-result re-entry
- persistence wiring for live turns
- CLI app-boundary E2E
- mock-based regression plus one real-provider smoke

### Out of Scope

- OpenAI-compatible live executor
- multi-tool parallel orchestration
- WebUI rendering changes beyond already-existing protocol truth
- generalized production node runtime
- cross-host recovery or distributed persistence
- provider raw ledger as recovery truth

## Design Principles

1. No fallback. Unsupported provider or invalid live state must fail explicitly.
2. `freehand-reason` stays the only writer of session truth.
3. Provider adapters remain protocol render/parse/execute owners only; they do not own turn semantics.
4. Metadata/debug/provider raw evidence stays hard-isolated from request-chain content.
5. Orchestrators remain orchestration-only. Shared helpers or validators go to existing owner crates first, then `freehand-blocks` if truly shared semantic logic is needed.
6. The E2E path must be replayable and debuggable from `feature_id`, function map, ledgers, and persisted runtime state.

## Existing Truth To Reuse

- `crates/freehand-testkit/src/lib.rs`
  - `run_live_reason_turn`
  - `run_live_anthropic_reason_turn_with_hook`
- `crates/freehand-reason/src/persistence.rs`
  - `ReasonPersistence`
- `crates/freehand-reason/src/lib.rs`
  - `ReasonTurnEngine`
- `crates/freehand-provider-anthropic/src/lib.rs`
  - `AnthropicExecutor`
- `apps/freehand-cli/src/main.rs`
  - `reason-live`
- `docs/function-maps/provider.reason-live-bridge.md`
- `docs/testing/provider.reason-live-bridge.md`
- `docs/design/reason-persistence-design.md`

## Technical Plan

### 1. Live session identity and restore

Replace smoke-grade fixed live IDs with runtime-generated or resumable session identity at the CLI/testkit boundary.

Required behavior:

- CLI selects one agent from `~/.freehand/config.toml`
- live runtime resolves or creates one session id for the invocation
- before starting a new round, live path attempts `ReasonPersistence::restore`
- restored `SessionHistory` becomes the input truth for `ReasonTurnEngine`

Likely files:

- `apps/freehand-cli/src/main.rs`
- `crates/freehand-testkit/src/lib.rs`
- `crates/freehand-reason/src/persistence.rs`

### 2. Live persistence wiring

The live path must write the same authoritative truth already owned by `ReasonPersistence`.

Required behavior:

- turn start writes reason ledger + `active-turn.json`
- streamed semantic output updates `active-turn.json`
- terminal close writes closed turn snapshot and clears active-turn snapshot
- provider raw debug retention, if enabled, stays outside recovery truth

Likely files:

- `crates/freehand-testkit/src/lib.rs`
- `crates/freehand-reason/src/persistence.rs`
- `crates/freehand-reason/src/lib.rs`

### 3. Minimal live tool loop

Add one minimal deterministic tool path that proves real tool calling without broadening architecture.

Required behavior:

- the live bridge exposes one fixed tool schema to the provider request
- the provider may emit a tool call
- Freehand validates the tool call contract already present in shared types
- Freehand executes the tool through one minimal read-only executor
- tool result re-enters the same live request through turn-owned re-entry
- after re-entry, the model can continue and emit final completion schema

Recommended minimal tool:

- `echo_json`
  - input: arbitrary JSON object
  - output: same JSON object plus deterministic wrapper fields

Reason:

- deterministic
- easy to assert in tests
- no external IO
- still exercises full tool-call and tool-result semantics

Likely files:

- `crates/freehand-testkit/src/lib.rs`
- `crates/freehand-provider-core/src/lib.rs`
- `crates/freehand-blocks/src/lib.rs` only if shared schema/render helpers are truly needed
- `apps/freehand-cli/src/main.rs`

### 4. Multi-round completion loop closure

Keep the already-landed tagged completion schema loop as the only stop authority.

Required behavior:

- `claim=continue` triggers the next round immediately
- invalid or missing tagged schema is rejected in the same logical turn
- schema rejection feedback names the exact failing fields/items
- retry limit remains `3`
- terminal success/blocked/failed is written only through `ReasonTurnEngine`

Likely files:

- `crates/freehand-testkit/src/lib.rs`
- `crates/freehand-blocks/src/lib.rs`
- `crates/freehand-reason/src/lib.rs`

### 5. CLI app-boundary E2E shape

Keep CLI as a thin boundary over shared harness/runtime owners.

Required behavior:

- CLI does not duplicate provider or reason semantics
- CLI prints visible text projection, tool activity summary, round count, schema rejection count, and final terminal projection
- CLI can optionally resume an existing session or use a deterministic session label rule if one is defined during implementation

Likely files:

- `apps/freehand-cli/src/main.rs`
- `docs/function-maps/app.cli-live-turn.md`
- `docs/testing/app.cli-live-turn.md`

## File List To Expect

Implementation is likely to touch:

- `apps/freehand-cli/src/main.rs`
- `crates/freehand-testkit/src/lib.rs`
- `crates/freehand-reason/src/lib.rs`
- `crates/freehand-reason/src/persistence.rs`
- `crates/freehand-provider-core/src/lib.rs`
- `docs/function-maps/provider.reason-live-bridge.md`
- `docs/testing/provider.reason-live-bridge.md`
- `docs/function-maps/app.cli-live-turn.md`
- `docs/testing/app.cli-live-turn.md`
- `docs/testing/reason.persistence.md`

If a new feature boundary is introduced during implementation, update:

- `docs/architecture/feature-map.md`
- matching `docs/function-maps/*.md`
- matching `docs/testing/*.md`

## Risks And Avoidance

### Risk: tool loop leaks business semantics into orchestrator glue

Avoidance:

- keep tool execution contract minimal
- put shared schema/render helpers in existing shared owner crates only when reuse is real
- keep `freehand-testkit` as the E2E harness owner for this milestone

### Risk: provider `finish_reason` accidentally closes the turn

Avoidance:

- continue to require accepted Freehand completion schema for terminal truth
- add negative tests where provider says stop/end_turn without valid tagged completion

### Risk: persistence writes drift from real streamed execution

Avoidance:

- persist at live event-apply boundaries, not only after the full response
- add recovery tests from mid-stream snapshots and ledger tail

### Risk: live provider smoke becomes flaky

Avoidance:

- use mock-server tests as mandatory regression truth
- keep one real-provider smoke as explicitly credential-dependent validation, not the only evidence

## Verification Matrix

### White-box

- `cargo test -p freehand-testkit`
  - live bridge tool-call loop
  - multi-round continue loop
  - invalid-schema retry exhaustion
  - restore-before-turn path
  - persistence writes during streamed apply
- `cargo test -p freehand-reason`
  - persistence restore and terminal materialization regressions
- `cargo test -p freehand-provider-anthropic`
  - executor/tool-related request rendering or stream parsing regressions if touched

### Module black-box

- CLI live-turn mock smoke with tool call
- CLI live-turn mock smoke with `continue` then final complete
- CLI live-turn restore/resume smoke

### Project black-box

- app boundary: config-selected live Anthropic provider -> reason turn -> tool call -> tool result re-entry -> final completion -> persistence restore

### Workspace gates

- `cargo build --workspace`
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -p xtask -- gates check`

### Live smoke

- one real-provider run against the configured Anthropic-compatible provider in `~/.freehand/config.toml`
- prompt must force a tool call and at least one `claim=continue` round before final completion

## Implementation Steps

1. Lock or update feature docs before code:
   - refresh function map and test design for `provider.reason-live-bridge`
   - refresh CLI live-turn docs
   - add a new feature-map entry only if a new owner boundary appears
2. Replace fixed live IDs with resumable session identity at CLI/testkit boundary.
3. Wire `ReasonPersistence::restore` into live startup.
4. Wire persistence writes into live turn lifecycle.
5. Add one minimal live tool schema and executor path.
6. Re-enter tool results into the same live request and preserve completion-schema loop behavior.
7. Add white-box and module black-box tests for tool call, continue, restore, and terminal rules.
8. Run workspace gates.
9. Run one real-provider smoke with the configured Anthropic-compatible provider.
10. If truth changed, update docs, function maps, test design, `CACHE.md`, `MEMORY.md`, and `note.md` in the same change set.

## Definition of Done

Done means:

- a real provider can complete one simple multi-round task with at least one tool call
- the task closes only through valid Freehand completion schema
- turn/session truth survives through `ReasonPersistence`
- CLI remains a thin boundary over shared owners
- docs, function maps, test design, and gates are all in sync with the shipped behavior
