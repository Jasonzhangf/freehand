# Dev And Debug Workflow

## Development Flow

1. read `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`
2. open `feature-map.md`
3. open the feature's function-map doc
4. locate `feature_id`
5. confirm single owner and allowed paths
6. ask:
   - is information sufficient
   - is logic closed-loop
   - is lifecycle management complete
7. if not, do read-only tracing and source search first
8. search existing blocks and owner crates before writing any function
9. implement in owner or `freehand-blocks`
10. write or update the feature's test-design record
11. write or update the feature's function-map mainlines and call table
12. locate mapped tests:
   - module white-box
   - module black-box
   - project black-box
13. run required checks and the mapped tests
14. verify feature `lifecycle_checks`
15. if truth changed, update map/docs/skill/memory in same task

## Debug Flow

1. start from `feature_id`
2. inspect owner and `debug_artifacts`
3. inspect the feature's function-map doc for request/response/error mainlines
4. inspect runtime evidence under `docs/runtime/runtime-directories.md` declared paths
5. identify semantic position and scene position together
6. ask:
   - do I have enough information
   - is the logic path closed-loop
   - is lifecycle management complete
7. if not, continue read-only tracing first
8. reproduce with replay or fixture when possible
9. fix owner truth, not symptom branch
10. update the test-design record if the bug changes expected behavior or reveals a coverage gap
11. update the function-map doc if the bug changes mainline behavior, shared function usage, or code bindings
12. rerun mapped white-box, module black-box, and project black-box tests for the feature
13. verify feature `lifecycle_checks`
14. update function map or docs if debug entry or truth changed

## Required Update When Truth Changes

- update `docs/architecture/feature-map.md`
- update relevant architecture docs
- update `.agents/skills/freehand-dev/SKILL.md` if workflow changed
- update `MEMORY.md` with verified durable truth
- update `note.md` with exploration trail
