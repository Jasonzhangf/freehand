# Function Map: `instruction.capability-loader`

- feature_id: `instruction.capability-loader`
- owner crate: `crates/freehand-instructions`
- owner module: `crates/freehand-instructions/src/lib.rs`
- owner entry symbols:
  - `InstructionCapabilityCompileInput::new`
  - `compile_instruction_capability_manifest`
  - `write_instruction_capability_manifest`

## Request Mainline

- caller supplies the Freehand runtime home and current working directory
- loader resolves global instruction authoring inputs from `~/.freehand/AGENTS.md` and `~/.freehand/skills`
- loader resolves local instruction authoring inputs from every directory between project root and cwd using `AGENTS.md` and `.agents/skills`
- every discovered authoring file is validated and normalized into a typed manifest entry
- invalid skill frontmatter becomes an explicit manifest error entry instead of being silently ignored

## Response Mainline

- output is a deterministic `InstructionCapabilityManifest`
- manifest entries carry scope, precedence, normalized path, content byte count, and content hash
- skill entries also carry parsed `name` and `description`
- manifest carries one deterministic fingerprint over agents, skills, and compile errors
- optional writer persists the compiled manifest to `~/.freehand/state/instructions/capability-manifest.json`

## Error Mainline

- missing optional roots are skipped because no authoring input exists
- cwd that is not a directory is rejected
- unreadable AGENTS or skill files become explicit read errors
- malformed skill frontmatter becomes an explicit manifest error while valid capability entries remain visible
- manifest write failure returns an explicit error

## Shared Multi-Reference Functions

- `compile_instruction_capability_manifest`
  - owner: `crates/freehand-instructions`
  - purpose: discover, validate, sort, and fingerprint AGENTS.md and skill authoring surfaces into one manifest
  - allowed callers: runtime/context planner startup, CLI diagnostics, owner-crate tests
  - related tests: global/local AGENTS and skills indexing tests
  - why shared: instruction capability discovery must not be duplicated in runtime, UI, or provider crates
- `write_instruction_capability_manifest`
  - owner: `crates/freehand-instructions`
  - purpose: persist the compiled manifest under runtime state
  - allowed callers: runtime startup, CLI diagnostics, owner-crate tests
  - related tests: manifest writer test
  - why shared: manifest output path and JSON shape must stay single-sourced

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `InstructionCapabilityCompileInput::new` | `crates/freehand-instructions/src/lib.rs` | build compile input with default project-root markers | runtime home + cwd | typed compile input | runtime/CLI/tests | input builder | bound |
| 02 | `compile_instruction_capability_manifest` | `crates/freehand-instructions/src/lib.rs` | discover global and local AGENTS.md plus skill roots | compile input | manifest candidates | runtime/CLI/tests | discovery planner | bound |
| 03 | `agents_md_capability` | `crates/freehand-instructions/src/lib.rs` | normalize one AGENTS.md source into a manifest entry | AGENTS.md path + scope + precedence | AGENTS manifest entry or error | manifest compiler | AGENTS parser | bound |
| 04 | `collect_skills` | `crates/freehand-instructions/src/lib.rs` | scan a skill root deterministically | skill root + scope + precedence | skill entries and error records | manifest compiler | skill scanner | bound |
| 05 | `parse_skill_frontmatter` | `crates/freehand-instructions/src/lib.rs` | validate required skill frontmatter fields | SKILL.md content | skill name and description or manifest error | skill scanner | skill parser | bound |
| 06 | `write_instruction_capability_manifest` | `crates/freehand-instructions/src/lib.rs` | persist manifest JSON under runtime state | compiled manifest + target path | manifest file or write error | runtime/CLI/tests | manifest writer | bound |

## Metadata / Request Isolation Notes

- authoring directories are only discovery inputs
- this owner does not mutate provider payloads or request text
- current slice indexes content hashes and byte sizes; provider-visible injection is a later context-planner consumption step
- compile errors are data about capability loading and must not be converted into successful capability entries

## Sync Status Against Code

- first manifest compiler is implemented in `crates/freehand-instructions`
- current support covers global `~/.freehand/AGENTS.md`, local `AGENTS.md`, global `~/.freehand/skills`, and local `.agents/skills`
- runtime/context planner consumption is pending and must be added through this manifest owner, not by scanning directories from runtime code
- migrated mainline-call source lives at `docs/mainline-calls/instruction.capability-loader.json` and generated wiki lives at `docs/wiki/instruction.capability-loader.md`
