# Function Map: `reason.context-planner`

- feature_id: `reason.context-planner`
- owner crate: `crates/freehand-blocks`
- owner module: `crates/freehand-blocks/src/lib.rs`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `turn.plan_request_context`
- owner entry symbols:
  - `plan_context`
  - `render_context_segments_as_text`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `request_context`
- touched resources:
  - `turn`
- resource operations:
  - `turn.plan_request_context`
- forbidden shortcuts:
  - Metadata/control/debug state must not be smuggled into request context as ordinary model-visible payload.
  - Turns must not build provider requests without this typed context-planner resource.

## Request Mainline

- `freehand-reason` reads session truth and current turn inputs
- `reason.session-history` provides stable base context plus session-owned `rewrite_mode` and `rewrite_version`
- upstream restore/context rebuild callers may pre-prune superseded repaired-failure rounds before passing session-memory segments to the planner; raw failure truth remains outside request content in ledgers/UI projections
- it asks the planner owner path to classify context into stable and volatile segments
- task contract segments are session-stable/cacheable and task-space snapshots are turn-volatile/no-cache, so task state can be visible without poisoning the stable cache prefix
- the planner admits additional context only through typed segment rules
- instruction capability content enters only as `ContextSegmentKind::InstructionCapability`, with instruction owner provenance and session-stable cacheable semantics
- preferred context expansion path is subagent search final report -> `SubagentConclusion`
- the planner returns request-content-only output; metadata/cache/debug stay outside this mainline

## Response Mainline

- planner output becomes provider-neutral request content for the current turn
- planner also returns cache-shape diagnostics through metadata-side outputs
- cache-shape diagnostics now include tool-schema hash derived from tool owner truth when a runtime owner supplies a fingerprint
- provider renderers consume only planned request content plus explicit provider config
- downstream response handling does not mutate the stable prefix except through explicit rewrite events

## Error Mainline

- raw subagent transcript attempted as parent context is rejected as an architecture error
- metadata/request mixing is rejected as an architecture error
- unbounded or over-budget context segment admission is rejected
- task-space snapshots are rejected from rewrite-base gates because they are volatile turn state
- prefix rewrite without explicit rewrite gate is rejected

## Shared Multi-Reference Functions

- `plan_context`
  - owner: `crates/freehand-blocks`
  - purpose: classify, validate, order, and project typed context segments while emitting metadata-side cache diagnostics
  - allowed callers: `freehand-reason`, owner-crate tests, replay/debug tools
  - related tests: context segment admission, subagent conclusion admission, cache-shape drift tests
  - why shared: context semantics must not be duplicated in orchestrator or provider crates
- `validate_rewrite_base_segments`
  - owner: `crates/freehand-blocks`
  - purpose: validate and order session rewrite base segments before `reason.session-history` mutates stable prefix truth
  - allowed callers: `freehand-reason`, owner-crate tests
  - related tests: rewrite-base rejection tests
  - why shared: rewrite-base segment semantics must stay aligned with ordinary-turn planner semantics
- `inspect_context_cache_diagnostics`
  - owner: `crates/freehand-blocks`
  - purpose: compute metadata-side cache diagnostics for explicit rewrite ledger events
  - allowed callers: `freehand-reason`, owner-crate tests, replay/debug tools
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
| 01 | `plan_context` | `crates/freehand-blocks/src/lib.rs` | classify stable and volatile context sources into typed segments and append the owning user-turn segment | candidate segments + current turn input + rewrite metadata | ordered typed context segment set + cache diagnostics | `freehand-reason` | planner builder | bound |
| 02 | `plan_context` | `crates/freehand-blocks/src/lib.rs` | validate segment admission and token caps | typed context segment candidates | admitted/rejected segment set | planner builder | planner validator | bound |
| 03 | `plan_context` | `crates/freehand-blocks/src/lib.rs` | calculate cache-shape diagnostics including rewrite mode/version | admitted stable prefix + rewrite version + tool schema shape | metadata-side cache diagnostics | planner builder | cache diagnostics block | bound |
| 04 | `validate_rewrite_base_segments` | `crates/freehand-blocks/src/lib.rs` | validate stable/session-stable base segments for explicit rewrite gates | rewritten stable prefix candidates | admitted/rejected rewrite base segment set | `reason.session-history` | rewrite validator | bound |
| 05 | `inspect_context_cache_diagnostics` | `crates/freehand-blocks/src/lib.rs` | calculate rewrite-ledger diagnostics outside request content | admitted rewrite base segments + rewrite mode/version | metadata-side cache diagnostics | `reason.session-history` | cache diagnostics block | bound |
| 06 | `render_context_segments_as_text` | `crates/freehand-blocks/src/lib.rs` | materialize provider-neutral planned request content | admitted ordered segments | provider-neutral request content string | provider adapters | planner projector | bound |

## Metadata / Request Isolation Notes

- request-side output contains only ordered typed context content
- cache diagnostics, trace ids, provider selectors, and replay pointers stay on metadata side
- `SubagentConclusion` is the only allowed parent-visible projection from subagent transcript truth
- provider renderers must not recover content from metadata or child transcript storage

## Sync Status Against Code

- semantic design is locked
- planner baseline is landed in `freehand-blocks`
- current `freehand-reason` baseline now routes turn startup through `plan_context`
- current baseline enforces segment ordering, segment-contract validation, token-budget rejection, user-turn append ownership, raw-subagent-transcript rejection by provenance, and rewrite-base validation for session history
- current baseline includes first-class `TaskContract` and `TaskSpaceSnapshot` segment kinds with cache-shape coverage
- current baseline includes first-class `InstructionCapability` segment kind for instruction owner output admitted into request context
- current baseline emits cache diagnostics separated from request content for both ordinary turns and explicit rewrite ledger events
- rewrite-mode and rewrite-version are now sourced from persistent `SessionHistory` truth instead of turn-local constants
- runtime live bridge now wires deterministic tool-schema fingerprint truth from `tool.registry` into planner diagnostics without moving tool schema semantics into reason owners
- runtime live bridge now rebuilds restored same-session context with repaired logical-turn economy before the planner sees session-memory segments, keeping only the latest repaired round in future prompt context
- migrated mainline-call source now lives at `docs/mainline-calls/reason.context-planner.json` and generated wiki lives at `docs/wiki/reason.context-planner.md`
