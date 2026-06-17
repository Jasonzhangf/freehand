# Reason Context Planner Design

## Status

Locked design truth for first implementation.

This document defines:

- how Freehand builds model-visible context
- how cache-stable and volatile context are separated
- how subagent search results may enter parent context
- how request content and metadata stay hard-isolated

Planner baseline is landed.

Current landed baseline includes:

- typed segment admission in `freehand-blocks`
- user-turn segment ownership in planner
- stable-before-volatile ordering
- raw subagent transcript rejection by provenance
- token-budget rejection
- metadata-side cache diagnostics
- rewrite-base validation for session-history gates
- rewrite mode/version sourced from session-history truth
- explicit compaction / rollback / resume rebuild ledger diagnostics

Remaining gap:

- final CLI/server runtime loop wiring for real usage metrics and persisted recovery payloads is not yet bound
- planner diagnostics now accept runtime-supplied tool-schema fingerprint truth; broader runtime metrics/recovery wiring remains the remaining gap

## Owner

- semantic owner crate: `crates/freehand-blocks`
- orchestration owner crate: `crates/freehand-reason`

Ownership split:

- `freehand-blocks` owns pure context planning, validation, cache-shape calculation, and allowed context-addition rules
- `freehand-reason` owns when planning runs, which turn/session truth is read, which turn consumes the result, and which session rewrite version applies

## Design Goal

Freehand context planning combines:

- Reasonix cache-first session discipline
- Codex typed fragment discipline

The target is:

1. stable prefix stays stable across turns
2. new context enters through typed segments only
3. subagent work does not dump raw transcript into parent context
4. provider adapters receive already planned request content, not mixed metadata
5. cache drift is explainable from typed planner output

## Context Locking Model

### Lock 1: Stable Prefix Lock

The stable prefix is session truth and must not change on ordinary turns.

Stable prefix includes only:

- system-level operating rules
- stable project memory and durable policy material
- stable completion contract instructions
- stable tool contract instructions

It does not include:

- current user turn text
- volatile tool outputs
- raw provider events
- trace/debug metadata
- subagent transcript bodies

### Lock 2: Append-Only Tail Lock

Ordinary turns may only add volatile context at the tail.

Allowed tail additions:

- current user turn input
- current turn tool-result evidence
- validated learned material promoted for this session
- validated subagent conclusion segments

Ordinary turns must not mutate the stable prefix in place.

### Lock 3: Rewrite Gate Lock

Context rewrite is allowed only through explicit rewrite events.

First version allowed rewrite events:

- compaction
- rollback
- explicit session rebuild after restart/resume load

Every rewrite must:

- bump a session/history rewrite version
- emit debug/cache diagnostics evidence
- remain visible in replay/ledger

Current implementation status:

- planner diagnostics now carry both `rewrite_mode` and `rewrite_version`
- turn startup reads rewrite mode/version from `reason.session-history`
- explicit compaction / rollback / resume rewrite gating is landed in `freehand-reason`
- trigger policy for compaction / rollback / resume rebuild is now owned by `reason.rewrite-policy` in `freehand-blocks`
- `ReasonRewriteRuntime` now consumes trigger-policy output before mutating session history

### Lock 4: Subagent Conclusion Lock

Subagent output enters parent context only as a typed final conclusion segment.

The parent must not ingest:

- subagent raw reasoning stream
- subagent tool call history
- subagent full transcript body
- subagent intermediate partial answers

The parent may ingest only:

- concise final answer
- subagent reference id
- declared task/scope label
- optional evidence summary already reduced by the subagent

This follows Reasonix `task` behavior: the parent sees only the subagent's self-contained final answer while the transcript persists separately.

## Typed Context Model

### Segment Classes

Each model-visible fragment must become one typed context segment before provider rendering.

First-version segment classes:

- `SystemAnchor`
  - cache role: `CacheAnchor`
  - stability: `Stable`
- `DeveloperPolicy`
  - cache role: `CacheAnchor`
  - stability: `Stable`
- `SessionMemory`
  - cache role: `Cacheable`
  - stability: `SessionStable`
- `SessionSummary`
  - cache role: `Cacheable`
  - stability: `SessionStable`
- `SubagentConclusion`
  - cache role: `NoCache`
  - stability: `TurnVolatile`
- `ToolResultEvidence`
  - cache role: `NoCache`
  - stability: `TurnVolatile`
- `UserTurnInput`
  - cache role: `NoCache`
  - stability: `TurnVolatile`
- `CompletionContract`
  - cache role: `CacheAnchor`
  - stability: `Stable`

### Segment Admission Rules

Every segment must have:

- unique `segment_id`
- explicit `segment_kind`
- explicit `stability`
- explicit `cache_policy`
- explicit `role`
- model-visible `content`
- bounded token budget
- provenance that explains where the content came from

No segment may contain hidden debug fields.

## Preferred Context Addition Method

When the system needs to enlarge context from external search or broad exploration, the preferred path is:

1. dispatch a focused subagent search/investigation task
2. let the subagent work in its own session/truth
3. receive one concise final report
4. admit only that report as `SubagentConclusion`

This is preferred over:

- copying raw search logs into parent context
- copying many `grep`/`read` outputs directly into parent context
- replaying the child transcript in the parent turn

Reason:

- parent context stays compact
- cache-stable head is preserved
- search noise is isolated
- debug still has the subagent transcript reference

## Context Ordering

First-version planned request order is locked as:

1. `SystemAnchor`
2. `DeveloperPolicy`
3. `CompletionContract`
4. `SessionMemory`
5. `SessionSummary`
6. `SubagentConclusion`
7. `ToolResultEvidence`
8. `UserTurnInput`

Ordering rule:

- stable segments must remain ahead of volatile segments
- subagent conclusion must come before current user turn only when it is supporting context for this turn
- current user turn stays last among model-visible natural-language context

## Cache Diagnostics

The planner must produce a cache-shape view separate from request text.

First-version cache diagnostics should explain drift from:

- stable prefix hash
- stable segment hash list
- tool schema hash
- session/history rewrite version
- per-segment estimated token cost

These diagnostics belong in debug/ledger metadata, not in request text.

## Metadata / Request Hard Isolation

### Request Content Side

Request content types may carry only:

- system/developer/user/tool visible content
- planned context segments
- provider-target request content

### Metadata Side

Metadata types may carry only:

- writer owner and write-node provenance owned by `metadata.core`
- trace ids
- source ids
- provider/model/protocol selectors
- cache diagnostics
- debug policy
- scene/semantic location
- replay/ledger references

Forbidden:

- metadata values embedded into request text to steer behavior
- request content reconstructed from metadata/debug fields
- one mixed DTO that contains both prompt content and debug/trace/provider state

## Provider Boundary

Provider crates do not own context decisions.

They may:

- receive planned request content
- map it to provider wire shape
- parse raw provider events

They may not:

- decide which context segments exist
- reorder segment classes
- upgrade raw metadata into prompt content

## Subagent Persistence Rule

Subagent transcript truth stays outside parent request context.

The debug/runtime side should persist:

- subagent transcript/session truth
- subagent reference id
- subagent owner parent session/turn
- subagent status

The parent request may carry only:

- `SubagentConclusion`
- transcript/reference pointer in metadata/debug envelope

## Required Gates

Architecture gates for this feature must eventually reject:

- request nodes containing debug/trace/provider/cache metadata fields
- metadata types containing request text payload
- provider adapter context planning logic
- raw subagent transcript ingestion into parent request context
- ad hoc prompt rendering outside the planner owner path
- non-explicit session rewrite without version bump

## Open Items

- exact Rust type names
- exact segment serialization shape
- exact context compaction format
- exact runtime ledger file names

Those are implementation details and do not reopen the locked semantic design above.
