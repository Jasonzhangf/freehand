# Test Design: `instruction.capability-loader`

- feature_id: `instruction.capability-loader`
- owner: `crates/freehand-instructions`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `config.compile_instruction_capability`
  - `instruction_capability.admit_request_context`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `config.compile_instruction_capability` | bound | `cargo test -p freehand-instructions` covers global/local AGENTS and skill discovery, malformed frontmatter, ordering, fingerprint, and manifest write tests | `cargo test -p freehand-instructions` covers fixture tree compile plus runtime-state manifest writer smokes | `cargo run -p xtask -- gates check` covers project-level manifest boundary; provider/UI consumption belongs to `instruction_capability.admit_request_context` |
| `instruction_capability.admit_request_context` | bound | `cargo test -p freehand-instructions renders_manifest_entries_as_instruction_capability_context -- --nocapture` covers owner-rendered instruction capability content and `cargo test -p freehand-blocks instruction_capability_segment_is_stable_typed_context -- --nocapture` covers typed planner segment admission | `cargo test -p freehand-runtime live_bridge_admits_instruction_capability_manifest_as_typed_context -- --nocapture` covers runtime live bridge manifest consumption and provider request admission as `ContextSegmentKind::InstructionCapability` | `cargo test -p freehand-runtime live_bridge -- --nocapture` covers live bridge request-context admission paths, and `cargo run -p xtask -- gates check` covers resource-map/source-edge binding |

- lifecycle path under test:
  - discover global Freehand instruction surfaces from `~/.freehand`
  - discover local project instruction surfaces from project root to cwd
  - validate skill frontmatter
  - compile deterministic manifest entries
  - write manifest JSON under runtime state
- white-box plan:
  - global `~/.freehand/AGENTS.md` creates a global AGENTS manifest entry
  - local `AGENTS.md` files from project root through nested cwd create ordered local entries
- symlinked cwd aliases are normalized before local directory traversal and do not scan alias-parent `.agents/skills`
- global `~/.freehand/skills/**/SKILL.md` creates global skill entries
- local `.agents/skills/**/SKILL.md` creates local skill entries
- symlink cycles under `.agents/skills` terminate through visited canonical
  directory tracking, while symlinks pointing outside the skill root create
  explicit manifest errors and do not import external skills
- malformed skill frontmatter creates an explicit error record while valid skills remain indexed
- stable input order produces stable manifest fingerprint
- module black-box plan:
  - fixture tree compile returns a manifest with expected scope and precedence
  - manifest writer creates `state/instructions/capability-manifest.json`
  - manifest entries expose path/hash/size metadata
  - typed instruction admission renders AGENTS.md and skill content only through instruction owner output and runtime `ContextSegmentKind::InstructionCapability`
- project black-box impact:
  - runtime live bridge consumes this compiled manifest rather than scanning authoring directories from provider code
  - provider requests receive instruction capability content only after typed context-planner admission
- fixtures / replay inputs / runtime evidence paths:
  - temp fixture trees inside `cargo test -p freehand-instructions`
  - `~/.freehand/state/instructions/capability-manifest.json`
- known gaps:
  - UI/CLI diagnostics for manifest errors are pending
- sync status between design and implementation:
  - white-box and module black-box tests are implemented in `crates/freehand-instructions`, `crates/freehand-blocks`, and `crates/freehand-runtime`
  - `cargo test -p freehand-instructions --lib -- --nocapture` includes
    `skill_scan_stays_inside_root_and_does_not_follow_symlink_cycles`
    and `symlink_cwd_does_not_scan_alias_parent_instruction_roots`
  - function map and mainline call map bind the manifest compiler, context renderer, and runtime typed context admission symbols
