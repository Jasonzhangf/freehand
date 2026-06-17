# Stability And Engineering Quality Plan

## Goal

Improve Freehand stability and engineering quality without adding product features.

This goal focuses on making existing behavior safer to change, easier to debug, and harder to regress.

## Acceptance Criteria

- Repository has a current stability baseline from the full local gate stack.
- High-risk runtime surfaces have explicit owner, test, and debug routes.
- Existing flaky, slow, under-specified, or weakly verified paths are identified and either fixed or filed in docs with owner and next gate.
- No fallback, silent downgrade, or duplicate semantic implementation is introduced.
- Function maps, test designs, mainline call JSON, generated wiki, and skills/memory stay synchronized when truth changes.

## Scope

In scope:

- Audit current build/test/gate health.
- Strengthen deterministic tests around runtime, UI protocol, live turn cancellation, checkpoint/rewind, tool execution, provider live bridge, and persistence.
- Add or tighten gates that catch stale wiki, owner drift, missing tests, duplicate truth, fallback wording, metadata/request leaks, and manifest/compiled-directory drift when applicable.
- Improve debug traceability from `feature_id` to owner to runtime evidence.
- Remove confirmed dead/duplicate/error implementations after verifying dependencies.
- Document gaps as owner-bound follow-up work when the current slice cannot safely close them.

Out of scope:

- New user-facing product features.
- New provider families beyond current planned provider work.
- Broad UI redesign.
- Unapproved destructive migrations.
- Runtime fallback or best-effort behavior.

## Design Principles

- Owner first: every fix starts from `docs/architecture/feature-map.md` and the relevant function map.
- Test design first: stability fixes must update test-design docs before or with test implementation.
- Compiled directory discipline: authoring directories are not runtime truth unless compiled and validated into deterministic manifests.
- Runtime truth stays separate from UI projection, debug snapshots, provider raw events, and metadata.
- Positive and negative tests must pair for state machines, streams, cancellation, timeouts, retries, error projection, and cleanup.
- Prefer deterministic local fixtures and replay over live services; use real provider smoke only when claiming real provider behavior.

## Target Docs

- `AGENTS.md`
- `.agents/skills/freehand-dev/SKILL.md`
- `docs/architecture/feature-map.md`
- `docs/architecture/dev-gates.md`
- `docs/function-maps/README.md`
- `docs/testing/**`
- `docs/mainline-calls/**`
- `docs/wiki/**`
- `docs/debug/debug-playbook.md`
- `docs/runtime/runtime-directories.md`
- `docs/goals/stability-quality-plan.md`

## Work Plan

1. Baseline audit
   - Run full local gate stack.
   - Capture current test counts and failures, if any.
   - Inspect `git status --short` before changes.

2. Owner and gate coverage audit
   - Check feature-map entries for required docs, required checks, and lifecycle checks.
   - Check function maps for request/response/error mainlines and code-bound call tables.
   - Check mainline JSON and generated wiki freshness.

3. Runtime risk audit
   - Review live turn, cancellation, persistence restore, checkpoint/rewind, tool execution, and daemon command ingress.
   - Confirm each high-risk path has positive and negative tests.
   - Add missing tests only through the owning feature.

4. Debuggability audit
   - Confirm failures can be located by `feature_id -> owner -> runtime evidence`.
   - Check debug docs and runtime paths remain accurate.
   - Add targeted debug evidence only when it has a clear owner and replay path.

5. Cleanup audit
   - Identify confirmed dead code, stale docs, duplicate semantics, old chains, and fallback wording.
   - Remove only after dependency check and verification.

6. Documentation and memory sync
   - Update function maps, test designs, mainline JSON, generated wiki, skills, `note.md`, `MEMORY.md`, and `CACHE.md` only when truth changes.

## Validation Matrix

- Formatting: `cargo fmt --all --check`
- Build: `cargo build --workspace`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Tests: `cargo test --workspace`
- Mainline wiki freshness: `cargo run -p xtask -- mainlines check`
- Architecture gates: `cargo run -p xtask -- gates check`
- Targeted tests: run feature-specific crate tests for every touched owner.
- Real smoke: run daemon/provider/WebUI smoke only when the change claims real runtime/provider/UI behavior.

## Risk Points

- Large stability sweeps can become unfocused. Keep each fix owner-bound and commit in small slices.
- Gate additions can create noisy false positives. Start with deterministic, low-noise checks.
- Test hardening can accidentally encode implementation details. Prefer semantic assertions at module boundaries.
- Cleanup can delete useful but hidden dependencies. Verify references and run full gates before removal.
- Live provider tests can be unstable. Use local mock fixtures unless real provider behavior is the acceptance target.

## Completion Definition

- Full gate stack passes.
- All touched features have updated function map, test design, mainline JSON, and generated wiki when truth changed.
- Stability fixes include positive and negative tests where required.
- Confirmed dead/duplicate/error implementations are physically removed or documented with owner-bound blocker.
- Remaining gaps are listed with owner, risk, and next verification gate.
- Changes are committed and pushed unless the user explicitly says not to.
