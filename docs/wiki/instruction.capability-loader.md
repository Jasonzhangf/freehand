# Wiki: `instruction.capability-loader`

Generated from `docs/mainline-calls/instruction.capability-loader.json`. Do not edit by hand.

- owner crate: `crates/freehand-instructions`
- owner module: `crates/freehand-instructions/src/lib.rs`
- function map: `docs/function-maps/instruction.capability-loader.md`
- generated wiki: `docs/wiki/instruction.capability-loader.md`
- test design: `docs/testing/instruction.capability-loader.md`

## Request Mainline

- caller supplies Freehand runtime home and current working directory
- loader resolves global instruction authoring inputs from `~/.freehand/AGENTS.md` and `~/.freehand/skills`
- loader resolves local instruction authoring inputs from every directory between project root and cwd using `AGENTS.md` and `.agents/skills`
- discovered authoring files are validated and normalized into typed manifest entries

## Response Mainline

- output is a deterministic `InstructionCapabilityManifest`
- manifest entries carry scope, precedence, normalized path, content byte count, and content hash
- skill entries also carry parsed `name` and `description`
- optional writer persists the compiled manifest to `~/.freehand/state/instructions/capability-manifest.json`

## Error Mainline

- cwd that is not a directory is rejected
- unreadable AGENTS or skill files become explicit read errors
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

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `InstructionCapabilityCompileInput::new` | `crates/freehand-instructions/src/lib.rs` | build compile input with default project-root markers | runtime home plus cwd | typed compile input | runtime/CLI/tests | input builder | bound |
| 02 | `compile_instruction_capability_manifest` | `crates/freehand-instructions/src/lib.rs` | discover global and local AGENTS.md plus skill roots | compile input | manifest candidates | runtime/CLI/tests | discovery planner | bound |
| 03 | `agents_md_capability` | `crates/freehand-instructions/src/lib.rs` | normalize one AGENTS.md source into a manifest entry | AGENTS.md path plus scope plus precedence | AGENTS manifest entry or error | manifest compiler | AGENTS parser | bound |
| 04 | `collect_skills` | `crates/freehand-instructions/src/lib.rs` | scan a skill root deterministically | skill root plus scope plus precedence | skill entries and error records | manifest compiler | skill scanner | bound |
| 05 | `parse_skill_frontmatter` | `crates/freehand-instructions/src/lib.rs` | validate required skill frontmatter fields | SKILL.md content | skill name and description or manifest error | skill scanner | skill parser | bound |
| 06 | `write_instruction_capability_manifest` | `crates/freehand-instructions/src/lib.rs` | persist manifest JSON under runtime state | compiled manifest plus target path | manifest file or write error | runtime/CLI/tests | manifest writer | bound |

## Sync Status Against Mainline Call

- first manifest compiler is implemented in `crates/freehand-instructions`
- current support covers global `~/.freehand/AGENTS.md`, local `AGENTS.md`, global `~/.freehand/skills`, and local `.agents/skills`
- runtime/context planner consumption is pending and must be added through this manifest owner
