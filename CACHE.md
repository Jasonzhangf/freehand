# CACHE

- 2026-06-15: repo scaffold initialized for Rust workspace, architecture docs, project AGENTS, and local `freehand-dev` skill.
- Current priority: foundation harness before feature implementation.
- 2026-06-15: workflow truth expanded
  - runtime home is `~/.freehand`
  - dev/debug starts from function map and owner
  - when feature truth changes, update map/docs/skills/memory in same task
- 2026-06-15: test workflow truth expanded
  - `ui.protocol` first-version scope and subscription/query rules are documented
  - feature map now declares white-box, module black-box, and project black-box test groups
  - `xtask` gate requires the test-strategy doc and related rule snippets
- 2026-06-15: function-map workflow expanded
  - feature map now points to per-feature `function_map_doc`
  - `docs/function-maps/` holds code-bound mainline docs and call-table stubs
  - `xtask` gate requires function-map routing and policy snippets
