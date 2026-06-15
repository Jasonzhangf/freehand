# Dev Gates

Freehand uses one gate stack locally and in CI.

## Required Local Gate

```bash
cargo build --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- gates check
```

`cargo test --workspace` is the mandatory test umbrella. As modules gain tests, it must cover:

- module white-box tests
- module black-box tests
- project black-box tests

No feature may claim regression-safe completion unless all three mapped layers pass where applicable.

## Commit And Push Rule

- commit requires format and architecture gate
- push requires full local gate
- CI reruns the same gate
- release jobs only run after CI success
- gate failures block commit and push; no bypass-by-default workflow exists

## Test Taxonomy

- module white-box: internal semantic behavior of the owner crate, including validator/builder/parser/projector edge cases
- module black-box: standard user-visible or caller-visible behavior at the module contract boundary
- project black-box: typical end-to-end application behavior across crate boundaries

Every feature map entry must state its required tests in this taxonomy rather than as an unstructured list.

## Per-Change Expectation

For every feature change:

- identify the owner feature in `docs/architecture/feature-map.md`
- run its mapped white-box tests
- run its mapped module black-box tests
- run its mapped project black-box tests
- run workspace build, lint, and architecture gates

If a layer is intentionally not yet present for a feature, that absence must be explicit in the function map or test strategy docs rather than assumed.

## Architecture Rule

- search existing blocks and owner crates before adding a function
- orchestrator crates are not helper libraries
- reusable or semantic logic must land in `freehand-blocks`
- start development and debug from function map and owner
- runtime home is `~/.freehand`
- truth change requires same-task updates to map, docs, skill, and memory
- `AGENTS.md` is router only; detailed truth must live in `docs/`
