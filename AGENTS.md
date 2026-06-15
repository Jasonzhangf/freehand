# Freehand Project AGENTS

## Purpose

This file is the repo entry router.

Freehand is a Rust-first agent system with:

- master/slave node topology
- reasoning and UI split
- multi-UI access on one truth source
- contracts + blocks + orchestrators isolation
- function-map-first development and debugging

Do not put long detailed rules here. Detailed truth belongs in `docs/`.

## Read Order

1. `AGENTS.md`
2. `CACHE.md`
3. `MEMORY.md`
4. `note.md`
5. route into `docs/` based on task

## Route Map

- feature/function owner lookup:
  - `docs/architecture/feature-map.md`
  - `docs/architecture/function-map-spec.md`
  - `docs/function-maps/README.md`
- workspace and module boundaries:
  - `docs/architecture/workspace-layout.md`
- dev and debug workflow:
  - `docs/architecture/dev-debug-workflow.md`
  - `docs/debug/README.md`
  - `docs/debug/debug-directories.md`
  - `docs/debug/debug-playbook.md`
- runtime home and runtime directories:
  - `docs/runtime/runtime-home.md`
  - `docs/runtime/runtime-directories.md`
- config directories and config truth:
  - `docs/config/config-directories.md`
- design docs and design truth:
  - `docs/design/design-doc-index.md`
- validation and gates:
  - `docs/architecture/dev-gates.md`
- local workflow skill:
  - `.agents/skills/freehand-dev/SKILL.md`

## Core Router Rules

1. No owner from function map, no edit.
2. No new function before checking existing blocks and owner crates.
3. Orchestrator crates stay pure orchestration; helper or semantic logic goes to `freehand-blocks`.
4. debug starts from `feature_id`, owner, debug artifacts, and runtime directories.
5. If truth changes, update docs, function map, skill workflow, and memory in same task.
6. For both features and bug fixes, check information sufficiency, logic closure, and lifecycle completeness before coding or closing work.
7. If those checks fail, do read-only tracing first. Ask the user only after local repo truth and runtime evidence cannot close the gap.

## Validation Baseline

```bash
cargo build --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- gates check
```
