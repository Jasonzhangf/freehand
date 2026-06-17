# Wiki: `reason.context-planner`

Generated from `docs/mainline-calls/reason.context-planner.json`. Do not edit by hand.

- owner crate: `crates/freehand-blocks`
- owner module: `crates/freehand-blocks/src/lib.rs`
- function map: `docs/function-maps/reason.context-planner.md`
- generated wiki: `docs/wiki/reason.context-planner.md`
- test design: `docs/testing/reason.context-planner.md`

## Request Mainline

- `freehand-reason` reads session truth and current turn inputs
- `reason.session-history` provides stable base context plus session-owned `rewrite_mode` and `rewrite_version`
- it asks the planner owner path to classify context into stable and volatile segments
- the planner admits additional context only through typed segment rules
- preferred context expansion path is subagent search final report -> `SubagentConclusion`
- the planner returns request-content-only output; metadata/cache/debug stay outside this mainline

## Response Mainline

- planner output becomes provider-neutral request content for the current turn
- planner also returns cache-shape diagnostics through metadata-side outputs
- provider renderers consume only planned request content plus explicit provider config
- downstream response handling does not mutate the stable prefix except through explicit rewrite events

## Error Mainline

- raw subagent transcript attempted as parent context is rejected as an architecture error
- metadata/request mixing is rejected as an architecture error
- unbounded or over-budget context segment admission is rejected
- prefix rewrite without explicit rewrite gate is rejected

## Shared Multi-Reference Functions

- `plan_context`
  - owner: `crates/freehand-blocks`
  - purpose: classify, validate, order, and project typed context segments while emitting metadata-side cache diagnostics
  - allowed callers: freehand-reason, owner-crate tests, replay/debug tools
  - related tests: context segment admission, subagent conclusion admission, cache-shape drift tests
  - why shared: context semantics must not be duplicated in orchestrator or provider crates
- `validate_rewrite_base_segments`
  - owner: `crates/freehand-blocks`
  - purpose: validate and order session rewrite base segments before `reason.session-history` mutates stable prefix truth
  - allowed callers: freehand-reason, owner-crate tests
  - related tests: rewrite-base rejection tests
  - why shared: rewrite-base segment semantics must stay aligned with ordinary-turn planner semantics
- `inspect_context_cache_diagnostics`
  - owner: `crates/freehand-blocks`
  - purpose: compute metadata-side cache diagnostics for explicit rewrite ledger events
  - allowed callers: freehand-reason, owner-crate tests, replay/debug tools
  - related tests: rewrite diagnostics snapshot tests
  - why shared: rewrite and ordinary-turn cache evidence must use one semantic calculator
- `render_context_segments_as_text`
  - owner: `crates/freehand-blocks`
  - purpose: single renderer from typed request-side segments into provider-consumable text
  - allowed callers: provider adapters, tests
  - related tests: planned request render smoke
  - why shared: provider adapters must consume a single context rendering path and must not own segment interpretation

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `plan_context` | `crates/freehand-blocks/src/lib.rs` | classify stable and volatile context sources into typed segments and append the owning user-turn segment | candidate segments plus current turn input plus rewrite metadata | ordered typed context segment set plus cache diagnostics | freehand-reason | planner builder | bound |
| 02 | `plan_context` | `crates/freehand-blocks/src/lib.rs` | validate segment admission and token caps | typed context segment candidates | admitted/rejected segment set | planner builder | planner validator | bound |
| 03 | `plan_context` | `crates/freehand-blocks/src/lib.rs` | calculate cache-shape diagnostics including rewrite mode/version | admitted stable prefix plus rewrite version plus tool schema shape | metadata-side cache diagnostics | planner builder | cache diagnostics block | bound |
| 04 | `validate_rewrite_base_segments` | `crates/freehand-blocks/src/lib.rs` | validate stable/session-stable base segments for explicit rewrite gates | rewritten stable prefix candidates | admitted/rejected rewrite base segment set | reason.session-history | rewrite validator | bound |
| 05 | `inspect_context_cache_diagnostics` | `crates/freehand-blocks/src/lib.rs` | calculate rewrite-ledger diagnostics outside request content | admitted rewrite base segments plus rewrite mode/version | metadata-side cache diagnostics | reason.session-history | cache diagnostics block | bound |
| 06 | `render_context_segments_as_text` | `crates/freehand-blocks/src/lib.rs` | materialize provider-neutral planned request content | admitted ordered segments | provider-neutral request content string | provider adapters | planner projector | bound |

## Sync Status Against Mainline Call

- semantic design is locked
- planner baseline is landed in `freehand-blocks`
- current `freehand-reason` baseline now routes turn startup through `plan_context`
- current baseline enforces segment ordering, segment-contract validation, token-budget rejection, user-turn append ownership, raw-subagent-transcript rejection by provenance, and rewrite-base validation for session history
- current baseline emits cache diagnostics separated from request content for both ordinary turns and explicit rewrite ledger events
- rewrite-mode and rewrite-version are now sourced from persistent `SessionHistory` truth instead of turn-local constants
- remaining gap: tool-schema fingerprint is still not wired from runtime tool truth into planner diagnostics
- generated wiki must be regenerated from `docs/mainline-calls/reason.context-planner.json` when this function-map truth changes
