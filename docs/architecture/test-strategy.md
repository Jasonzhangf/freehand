# Test Strategy

Freehand test strategy is locked around user behavior and owner truth, not ad hoc test placement.

## Test Design Record

Every module feature must maintain an inspectable test-design record before or alongside implementation work.

Required purpose:

- let another worker see where test coverage is supposed to live
- let another worker audit whether the logic path is closed-loop
- make test gaps visible before implementation drifts too far

Recommended location:

- feature-level records under `docs/testing/<feature-id>.md`
- if the repo is still early-stage, a temporary section in the owning design doc is acceptable only until the dedicated test doc exists

Required fields:

- `feature_id`
- owner
- lifecycle path under test
- white-box plan
- module black-box plan
- project black-box impact
- fixtures / replay inputs / runtime evidence paths
- known gaps
- sync status between design and implementation

## Test Layers

- module white-box
  - internal semantic behavior of the owning crate
  - covers builders, parsers, validators, projectors, state transitions, error classification, and edge conditions
- module black-box
  - standard caller-visible behavior at the owner crate boundary
  - proves the crate contract behaves correctly without reaching into internals
- project black-box
  - typical user-visible or operator-visible behavior across crate and app boundaries
  - proves runtime wiring, command flow, query flow, subscribe flow, and terminal projection

## Placement Rules

- white-box tests default to the owner crate
- module black-box tests live in the owner crate unless they need shared fixtures or harness support
- project black-box tests converge in `crates/freehand-testkit` and app/runtime smoke harnesses
- shared fixtures, replay inputs, mock providers, and protocol stream fixtures must be reused rather than duplicated

## Gate Rules

- every compile/regression cycle must run workspace build, lint, and all currently mapped test layers
- `cargo test --workspace` is the workspace umbrella and must include module white-box, module black-box, and project black-box coverage as those tests are added
- `cargo run -p xtask -- gates check` validates that the required docs and policy locks remain present
- commit is blocked on format plus architecture gate
- push is blocked on full local gate
- CI reruns the same gate before merge

## Mapping Rules

- every feature in `docs/architecture/feature-map.md` must declare:
  - `required_white_box_tests`
  - `required_module_black_box_tests`
  - `required_project_black_box_tests`
  - `test_design_doc`
- no feature may rely on an implicit test stack
- if a required layer does not exist yet, the gap must be explicit and tracked rather than hidden
- test-design records and implemented tests must be updated in the same change set when feature truth changes
- if implementation and test-design record disagree, the feature is not closed

## Behavioral Focus

- black-box tests should be written from standard user or caller behavior, not internal implementation shape
- module black-box tests cover feature-level standard behavior
- project black-box tests cover whole-project typical behavior and regression-sensitive journeys
- when state machine, stream, timeout, retry, error projection, or cleanup logic changes, positive and negative tests must both be present
- test-design records should explain why each test exists, not just list names
