# Wiki: `instruction.capability-loader`

Generated from `docs/mainline-calls/instruction.capability-loader.json`. Do not edit by hand.

- owner crate: `crates/freehand-instructions`
- owner module: `crates/freehand-instructions/src/lib.rs`
- function map: `docs/function-maps/instruction.capability-loader.md`
- generated wiki: `docs/wiki/instruction.capability-loader.md`
- test design: `docs/testing/instruction.capability-loader.md`

## Resource Operation Backlinks

- config.compile_instruction_capability
- instruction_capability.admit_request_context

## Request Mainline

- caller supplies Freehand runtime home and current working directory
- loader resolves global instruction authoring inputs from `~/.freehand/AGENTS.md` and `~/.freehand/skills`
- loader resolves local instruction authoring inputs from every directory between project root and cwd using `AGENTS.md` and `.agents/skills`
- cwd and project root are normalized before local directory traversal so symlink aliases cannot add alias-parent instruction roots outside the canonical project path
- skill roots are scanned with deterministic max depth, visited canonical directories, hidden-entry skip, and symlink traversal only when the target stays inside the same skill root
- discovered authoring files are validated and normalized into typed manifest entries

## Response Mainline

- output is a deterministic `InstructionCapabilityManifest`
- manifest entries carry scope, precedence, normalized path, content byte count, and content hash
- skill entries also carry parsed `name` and `description`
- instruction owner renders manifest entries into typed instruction capability context content
- runtime admits rendered instruction capability content as `ContextSegmentKind::InstructionCapability` before context planning
- optional writer persists the compiled manifest to `~/.freehand/state/instructions/capability-manifest.json`

## Error Mainline

- cwd that is not a directory is rejected
- unreadable AGENTS or skill files become explicit read errors
- symlinked skill entries that resolve outside the current skill root become explicit manifest errors and are not traversed
- malformed skill frontmatter becomes an explicit manifest error while valid capability entries remain visible
- manifest write failure returns an explicit error

## Shared Multi-Reference Functions

- `compile_instruction_capability_manifest`
  - owner: `crates/freehand-instructions/src/lib.rs`
  - purpose: discover, validate, sort, and fingerprint AGENTS.md and skill authoring surfaces into one manifest
  - allowed callers: runtime/context planner startup, CLI diagnostics, owner-crate tests
  - related tests: global/local AGENTS and skills indexing tests
  - why shared: instruction capability discovery must not be duplicated in runtime, UI, or provider crates
- `write_instruction_capability_manifest`
  - owner: `crates/freehand-instructions/src/lib.rs`
  - purpose: persist the compiled manifest under runtime state
  - allowed callers: runtime startup, CLI diagnostics, owner-crate tests
  - related tests: manifest writer test
  - why shared: manifest output path and JSON shape must stay single-sourced
- `render_instruction_capability_context`
  - owner: `crates/freehand-instructions/src/lib.rs`
  - purpose: render compiled AGENTS.md and skill manifest entries into typed provider-visible instruction capability context content
  - allowed callers: runtime live context admission, owner-crate tests
  - related tests: instruction capability context rendering tests, runtime live bridge typed instruction admission tests
  - why shared: runtime and provider crates must not rescan or independently render instruction authoring surfaces

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `InstructionCapabilityCompileInput::new` | `crates/freehand-instructions/src/lib.rs` | build compile input with default project-root markers | runtime home plus cwd | typed compile input | runtime/CLI/tests | input builder |  |  |  | bound |
| 02 | `compile_instruction_capability_manifest` | `crates/freehand-instructions/src/lib.rs` | discover global and local AGENTS.md plus skill roots | compile input | manifest candidates | runtime/CLI/tests | discovery planner | config | instruction_capability | config.compile_instruction_capability | bound |
| 02a | `dirs_between / normalize_path` | `crates/freehand-instructions/src/lib.rs` | normalize cwd and project root before deriving local instruction directories so symlink aliases do not add parent instruction roots | project root plus cwd, possibly through symlink alias | canonical local directory chain from project root to cwd only | compile_instruction_capability_manifest | path normalizer |  |  |  | bound |
| 03 | `agents_md_capability` | `crates/freehand-instructions/src/lib.rs` | normalize one AGENTS.md source into a manifest entry | AGENTS.md path plus scope plus precedence | AGENTS manifest entry or error | manifest compiler | AGENTS parser |  |  |  | bound |
| 04 | `collect_skills` | `crates/freehand-instructions/src/lib.rs` | scan a skill root deterministically with bounded symlink-safe traversal | skill root plus scope plus precedence | skill entries and error records | manifest compiler | skill scanner |  |  |  | bound |
| 05 | `parse_skill_frontmatter` | `crates/freehand-instructions/src/lib.rs` | validate required skill frontmatter fields | SKILL.md content | skill name and description or manifest error | skill scanner | skill parser |  |  |  | bound |
| 06 | `write_instruction_capability_manifest` | `crates/freehand-instructions/src/lib.rs` | persist manifest JSON under runtime state | compiled manifest plus target path | manifest file or write error | runtime/CLI/tests | manifest writer |  |  |  | bound |
| 07 | `instruction_capability_segment` | `crates/freehand-runtime/src/live_context.rs` | compile and render instruction capability truth, then admit it as a typed context-planner segment | runtime home plus cwd plus compiled instruction capability manifest | ContextSegmentKind::InstructionCapability request-context segment | runtime live bridge | freehand-instructions::render_instruction_capability_context | instruction_capability | request_context | instruction_capability.admit_request_context | bound |

## Sync Status Against Mainline Call

- manifest compiler and typed context renderer are implemented in `crates/freehand-instructions`
- current support covers global `~/.freehand/AGENTS.md`, local `AGENTS.md`, global `~/.freehand/skills`, and local `.agents/skills`
- runtime live bridge consumes this owner output through `ContextSegmentKind::InstructionCapability`, not by provider payload patching
