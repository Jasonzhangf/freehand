# Dev And Debug Workflow

## Development Flow

1. read `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`
2. open `feature-map.md`
3. use `Owner Routing Index` to map the problem to exactly one `feature_id`
4. open the feature's function-map doc
5. locate request/response/error mainlines and entry symbols
6. confirm single owner and allowed paths
7. open the feature's `test_design_doc` and locate white-box, module black-box, and project black-box coverage
8. ask:
   - is information sufficient
   - is logic closed-loop
   - is lifecycle management complete
9. if not, do read-only tracing and source search first
10. search existing blocks and owner crates before writing any function
11. implement in owner or `freehand-blocks`
12. write or update the feature's test-design record
13. write or update the feature's function-map mainlines and call table
14. locate mapped tests:
   - module white-box
   - module black-box
   - project black-box
15. run required checks and the mapped tests
16. verify feature `lifecycle_checks`
17. if truth changed, update map/docs/skill/memory in same task

## Debug Flow

1. start from `feature_id`
2. if the `feature_id` is unclear, use `Owner Routing Index` and do not patch code until ownership is clear
3. inspect owner and `debug_artifacts`
4. inspect the feature's function-map doc for request/response/error mainlines, entry symbols, and shared function ownership
5. inspect the feature's `test_design_doc` to see the existing white-box, module black-box, and project black-box coverage
6. inspect runtime evidence under `docs/runtime/runtime-directories.md` declared paths
7. identify semantic position and scene position together
8. ask:
   - do I have enough information
   - is the logic path closed-loop
   - is lifecycle management complete
9. if not, continue read-only tracing first
10. reproduce with replay or fixture when possible
11. fix owner truth, not symptom branch
12. update the test-design record if the bug changes expected behavior or reveals a coverage gap
13. update the function-map doc if the bug changes mainline behavior, shared function usage, or code bindings
14. rerun mapped white-box, module black-box, and project black-box tests for the feature
15. verify feature `lifecycle_checks`
16. update function map or docs if debug entry or truth changed

## Problem Location Rule

- A problem area maps to one `feature_id` through the `Owner Routing Index`.
- The `feature_id` maps to one owner module/crate through `docs/architecture/feature-map.md`.
- The owner module maps to exact entry symbols and mainlines through `docs/function-maps/<feature-id>.md`.
- The verification stack maps through `docs/testing/<feature-id>.md`.
- If any mapping is missing, update the map before implementation.
- Do not use grep as the first owner decision; grep is only evidence after routing.

## Required Update When Truth Changes

- update `docs/architecture/feature-map.md`
- update relevant architecture docs
- update `.agents/skills/freehand-dev/SKILL.md` if workflow changed
- update `MEMORY.md` with verified durable truth
- update `note.md` with exploration trail
