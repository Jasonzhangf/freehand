# Instruction Capability Loader Design

## Scope

- feature_id: `instruction.capability-loader`
- owner: `crates/freehand-instructions`
- first slice: compile an index manifest for AGENTS.md and skills
- current typed-admission slice: render compiled manifest entries into instruction capability context content and admit that content through `ContextSegmentKind::InstructionCapability`
- non-goal: provider adapters or UI apps directly scanning authoring directories or patching provider payloads

## Authoring Inputs

- global AGENTS.md: `~/.freehand/AGENTS.md`
- global skills: `~/.freehand/skills/**/SKILL.md`
- local AGENTS.md: every `AGENTS.md` from project root to cwd
- local skills: every `.agents/skills/**/SKILL.md` from project root to cwd

Local project root detection uses deterministic markers supplied by the compile input. The default markers are `.git` and `Cargo.toml`.

## Manifest Truth

Runtime consumers must consume the compiled manifest. Runtime/provider/UI code must not scan authoring directories directly.

The manifest contains:

- schema version
- Freehand home, cwd, and project root
- AGENTS.md entries with scope, precedence, normalized path, byte count, and content hash
- skill entries with scope, precedence, normalized path, root, parsed name, parsed description, byte count, and content hash
- explicit compile errors
- deterministic manifest fingerprint

The typed context renderer consumes the manifest and reads the listed AGENTS.md and skill files through the instruction owner. Runtime/provider code must not reimplement this scan or rendering logic.

## Ordering

- global entries use precedence `0`
- local entries start at `10` and increase from project root toward cwd
- entries with equal precedence sort by stable name/path ordering

This preserves Codex-style global plus repo-local layering while keeping Freehand runtime home as `~/.freehand`.

## Error Policy

- missing optional roots mean there was no authoring input and do not create fallback truth
- malformed skill frontmatter records a compile error
- valid entries remain indexed when unrelated entries fail
- write failures are explicit errors

## Context Boundary

The loader is not a provider adapter and does not patch provider payloads. It produces a bounded, typed, deterministic manifest and renders instruction capability content for context admission.

Runtime live bridge may consume this owner output only by constructing a `ContextSegmentKind::InstructionCapability` segment. The context planner validates the segment contract and admits it into request context before provider rendering.

Provider adapters and UI apps must never own this discovery or injection behavior.
