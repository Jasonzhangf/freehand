# Reason Turn Design

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

## Confirmed

### Turn truth granularity

- turn truth is stored per turn
- conversation view is projected from per-turn records

### Single writer rule

- only `freehand-reason` may write session truth

### Event broadcast granularity

First version semantic broadcast includes:

- reasoning
- text
- tool
- usage
- terminal
- error

### Tool result re-entry

- `freehand-reason` writes tool result re-entry back into turn truth

### Context orchestration and cache direction

Freehand context orchestration direction is now locked. Planner baseline is landed.

Locked behavior:

- stable context material should remain stable across turns to improve provider cache reuse
- volatile per-turn user input should be isolated from stable system/developer/context segments
- tool results and learned material should enter explicitly named context segments
- context composition must expose segment boundaries for debug and cache analysis
- provider-specific renderers may map segments to protocol wire shape, but cannot own context orchestration truth
- preferred context expansion path is subagent search final answer, not raw child transcript injection
- subagent transcript truth remains outside parent request context
- only explicit rewrite events may change stable-prefix history layout

Current implementation baseline:

- `ReasonTurnEngine::start_turn` now delegates request-content planning to `freehand-blocks::plan_context`
- typed segment ordering and admission rules are enforced before provider payload is built
- planner diagnostics are stored separately from request content
- `reason.session-history` now owns base context, rewrite mode, rewrite version, rewrite ledger, and persisted session-history snapshots
- ordinary turns read rewrite state from session truth without bumping rewrite version
- explicit compaction / rollback / resume rebuild paths are now staged through dedicated session-history gate methods

Current implementation gap:

- final CLI/server runtime loop wiring for `reason.rewrite-policy` remains outside this baseline
- tool-schema fingerprint is still not wired into planner diagnostics

### Multi-subscriber policy

- slow subscribers may drop frames
- slow subscribers must not back-pressure the main reasoning path
- debug and replay truth use ledger/replay paths instead of subscriber delivery guarantees

### Provider raw event placement

- provider raw events do not enter session truth
- provider raw events go to debug ledger paths

### Terminal classes

First version terminal classes are explicitly distinct:

- success
- tool_pending / needs_tool_result
- blocked
- interrupted
- failed
- cancelled

### Stop decision rule

- provider `finish_reason=stop` or `finish_reason=end_turn` does not by itself stop Freehand turn execution
- stop is controlled by Freehand completion schema validation

### Completion schema rule

The system prompt requires a completion schema wrapped in a fixed tagged JSON block:

```text
<freehand_completion>
{
  "claim": "complete" | "continue" | "blocked",
  "completion_reason": "...",
  "evidence": "...",
  "summary": "...",
  "learned": "...",
  "next_step": "...",
  "blocked_reason": "..."
}
</freehand_completion>
```

Confirmed terminal evaluation rules:

1. completed
   - if the model claims task completion, it must provide:
     - completion reason
     - evidence
     - summary
     - learned
   - if completion is claimed and evidence field has content, Freehand treats it as completed
   - Freehand extracts summary and evidence and composes terminal text output
   - if completion is claimed but required completion schema is missing or invalid, Freehand rejects it and asks again

2. not completed with next step
   - if not completed and `next_step` is present, Freehand uses `next_step` as the next execution signal and enters the next round instead of stopping

3. blocked
   - if not completed and blocked, `blocked_reason` must be present
   - if blocked data is valid, Freehand may stop in blocked state

4. invalid or missing schema
   - if schema is missing, invalid, or conditions are not satisfied, Freehand rejects it
   - Freehand must explain the exact invalid schema entries and how the schema should be provided
   - invalid schema is rejected within the same turn
   - invalid schema retry limit is 3
   - after 3 invalid schema retries, Freehand marks the turn as failed

## Open Questions / TBD

- exact per-turn storage schema
- exact mapping between terminal classes and persisted turn outcome records
- exact cache-boundary debug fields

## Update trigger

Update this doc when:

- turn truth ownership changes
- event broadcast granularity changes
- subscriber policy changes
- stop/completion schema rules change
- terminal class policy changes
