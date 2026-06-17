# Dev Gates

Freehand uses one gate stack locally and in CI.

## Required Local Gate

Canonical command:

```bash
make ci
```

Expanded command stack:

```bash
cargo build --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

`cargo test --workspace` is the mandatory test umbrella. As modules gain tests, it must cover:

- module white-box tests
- module black-box tests
- project black-box tests

No feature may claim regression-safe completion unless all three mapped layers pass where applicable.

## Commit And Push Rule

- commit requires format and architecture gate
- push requires `make ci`
- CI reruns `make ci`
- release jobs rerun `make ci` before building release binaries
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

## Mainline Manifest Gate

`xtask gates check` validates migrated mainline-call sources as deterministic manifests:

- `docs/mainline-calls/<feature_id>.json` path must match its internal `feature_id`
- `function_map_doc`, `test_design_doc`, and `generated_wiki_doc` must point to the canonical feature paths
- function map and test design must contain the same `feature_id`
- function map must reference the same mainline-call source
- feature map must link the mainline-call source and generated wiki path

This keeps generated wiki artifacts as compiled review surfaces over one machine-readable truth instead of independent hand-maintained docs.

## Feature-Map Uniqueness Gate

`xtask gates check` validates that `docs/architecture/feature-map.md` does not carry duplicate seed entries:

- each `### \`<feature_id>\`` seed entry may appear only once
- duplicate owner entries for the same `feature_id` fail the gate even if the duplicated docs are text-identical

This keeps owner routing as one truth instead of allowing silent drift through duplicated seed blocks.

## Mainline Call-Table Binding Gate

`xtask gates check` validates every migrated mainline-call row with `binding_status = "bound"`:

- each `file_path` segment must point to an existing repo file
- each `symbol_path` segment must resolve in one of the listed files
- Rust-style `Type::method` entries may resolve through the method tail, such as `method`, because source files define methods as `fn method`
- `binding_status = "pending"` remains explicit and is not treated as a bound symbol

This keeps mainline call maps code-bound instead of becoming stale review prose.

## CI/CD Command Alignment Gate

`xtask gates check` validates local and remote automation routes through the same full-gate truth:

- `Makefile` must provide a `mainlines` target that runs `cargo run -p xtask -- mainlines check`
- `Makefile` `ci` must include `build fmt clippy test mainlines gates`
- `.githooks/pre-push` must run `make ci`
- GitHub CI must run `make ci`
- release workflow must run `make ci` before release build/publish steps

This prevents pre-push, CI, and release from silently drifting into partial gate stacks.

## Metadata/Request Isolation Gate

`xtask gates check` validates one low-noise static boundary for metadata/request separation:

- request-node contract structs must not introduce metadata/debug owner types or obvious metadata/debug/cache payload fields
- metadata owner types must stay inside `crates/freehand-metadata`
- metadata owner structs must not introduce request payload fields such as prompt text, message arrays, or context payload content

This gate is intentionally narrow. It exists to fail obvious boundary regressions early without inventing fallback or runtime heuristics.
