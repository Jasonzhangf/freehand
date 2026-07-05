# Instruction Capability Loader Design

## Scope

- feature_id: `instruction.capability-loader`
- owner: `crates/freehand-instructions`
- first slice: compile an index manifest for AGENTS.md and skills
- non-goal in this slice: injecting instruction content into provider requests

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

The loader is not a prompt builder. It produces a bounded, typed, deterministic index. A later context-planner slice may consume the manifest and convert selected entries into typed context segments with explicit token budgets.

Provider adapters and UI apps must never own this discovery or injection behavior.
