# Tool Preview Design

## Scope

This doc locks the preview lifecycle for writable built-in tools.

- feature_id: `tool.preview`
- owner: `crates/freehand-tools`
- consumer owners:
  - `crates/freehand-runtime`
  - `crates/freehand-debug`
  - future UI/read-only projections through runtime-owned projection paths
- non-owners:
  - app crates
  - provider adapter crates
  - `crates/freehand-reason`

## Reference Evidence

Reasonix evidence:

- `../Deepseek-reasonix/internal/tool/builtin/preview.go`
  - writable tools compute a no-write preview before permission and execution
  - preview reuses the same argument and transform rules as execute
  - preview/execute equality is guarded by red tests

Current Freehand evidence:

- `crates/freehand-tools/src/lib.rs`
  - writable tool execution exists for `write_file`, `edit_file`, and `multi_edit`
  - current execution returns text summary only and has no structured preview owner path

## Core Truth

- `freehand-tools` owns writable-tool preview truth.
- Preview is not a UI helper and not a debug-only rendering.
- Preview must compute the same semantic file transformation that execute would persist, but without writing.
- Preview must validate:
  - arguments
  - workspace path lock
  - uniqueness rules
  - existing-parent rules
  - tool-specific transform rules
- Preview and execute must share one semantic transform path. They may differ only at the final side effect boundary.

## First-Version Coverage

Preview is required before live exposure for these writable tools:

- `write_file`
- `edit_file`
- `multi_edit`
- `delete_range`

Preview is required before implementation for these still-unimplemented writable tools:

- `delete_symbol`
- `notebook_edit`

Out of scope for v1:

- foreground `bash`
- read-only tools
- speculative patch languages or app-side synthetic diff generation

## Preview Contract Direction

Canonical preview truth should be structured and replayable.

First-version canonical shape direction:

- one preview per tool call
- one or more file changes per preview
- each change carries:
  - locked path
  - change kind: create / modify / delete
  - before text or explicit absent-before marker
  - after text or explicit absent-after marker

Derived renderings such as unified diff, colorized cards, or compact text are downstream projections. They are not the semantic truth.

If shared cross-module preview contracts are added in code, they must land in `crates/freehand-contracts`. `tool.preview` still owns the lifecycle and execution parity.

## Preview/Execute Parity Rule

- Preview must fail where execute would fail.
- Preview must succeed where execute would succeed.
- The previewed post-image must equal the text eventually persisted by execute.
- If execute semantics change, preview must change in the same patch.
- No writable tool may use one parser for preview and another parser for execution.

## Separation Rules

- `freehand-tools` owns path resolution, transform calculation, and preview truth.
- `freehand-runtime` may consume preview truth, but may not invent preview semantics.
- UI may render preview projections, but UI must not compute or alter preview truth.
- `freehand-reason` may mention tool failure/success semantically, but it must not preview file mutations.

## Error Policy

- malformed arguments -> explicit preview failure
- path escape or parent-directory violation -> explicit preview failure
- exact-match ambiguity or anchor-not-found -> explicit preview failure
- no fallback text summary in place of a structured preview
- no live writable execution path that silently skips preview

## Test Direction

- white-box:
  - preview argument validation parity
  - preview path-lock parity
  - preview create / modify / delete kind classification
  - preview post-image equals execute post-image
  - invalid edit and delete-range preview rejection parity
- module black-box:
  - runtime can request preview from the tool owner before writable execution
  - runtime rejects writable live execution when preview is unavailable
- project black-box:
  - provider live writable-tool loop can emit checkpointable preview truth before execution

## Non-Goals For This Design Lock

- conversation rewind
- app-owned approval UX
- notebook or symbol-aware editing implementation
- rewrite of session truth from preview alone

## Update Rule

If writable tool lifecycle changes, update in the same change set:

- `docs/architecture/feature-map.md`
- `docs/function-maps/tool.preview.md`
- `docs/testing/tool.preview.md`
- this design doc
- `docs/mainline-calls/tool.preview.json`
- generated wiki from `xtask mainlines generate`
