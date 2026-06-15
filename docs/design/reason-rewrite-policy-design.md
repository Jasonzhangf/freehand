# Reason Rewrite Policy Design

## Status

Locked design truth for first baseline.

This document defines:

- when Freehand should consider compaction
- when Freehand should trigger rollback
- when Freehand should trigger resume rebuild
- how unexpected states are classified instead of silently tolerated

## Owner

- semantic owner crate: `crates/freehand-blocks`
- session-truth mutation owner crate: `crates/freehand-reason`

Ownership split:

- `freehand-blocks` owns pure trigger policy and abnormal-state classification
- `freehand-reason` owns `SessionHistory` mutation through explicit gate methods
- runtime wiring may call policy first, then call `SessionHistory::stage_compaction`, `SessionHistory::stage_rollback`, or `SessionHistory::stage_resume_rebuild`

## Reference Evidence

Reasonix provides direct evidence for compaction policy only.

Local evidence paths:

- `../Deepseek-reasonix/internal/agent/compact.go`
- `../Deepseek-reasonix/internal/agent/cache_shape.go`
- `../Deepseek-reasonix/internal/agent/prune.go`

Confirmed Reasonix compaction behavior:

- soft notice at 50% of context window
- auto compaction trigger at 80%
- force compaction at 90%
- target kept tail capped by `min(window * 0.5, 16384 tokens)`
- stale tool results are pruned before compaction if pruning alone can clear the threshold
- auto compaction pauses after two ineffective consecutive compactions

Important boundary:

- Reasonix does not provide a Freehand-style persisted `SessionHistory` rewrite ledger
- Reasonix therefore does not provide direct reference truth for Freehand rollback or resume-rebuild trigger policy
- rollback and resume-rebuild policy below are Freehand-specific design, not copied reference behavior

## Design Goal

Freehand must not have hidden rewrite behavior.

The trigger layer must answer, with explicit typed output:

1. do nothing
2. emit a soft warning
3. prefer stale volatile evidence pruning over rewrite
4. stage compaction
5. stage rollback
6. stage resume rebuild
7. block because recovery truth is insufficient

## Trigger Matrix

### Ordinary Turn Compaction Trigger

Inputs:

- current rewrite mode
- context window tokens
- last prompt token usage
- estimated stale volatile reclaim tokens
- auto-compaction pause state
- soft-notice latch
- compaction thresholds

Rules:

1. if a non-ordinary rewrite mode is already pending, do not trigger a second rewrite
2. if context window is missing or zero, do not auto-compact; return explicit hold reason
3. if prompt usage is missing or zero, do not auto-compact; return explicit hold reason
4. if prompt usage is below soft threshold, do nothing and keep stable prefix unchanged
5. if prompt usage is between soft threshold and auto threshold, emit one soft notice and do not rewrite
6. if auto compaction is paused, do not compact again automatically
7. if prompt usage is above auto threshold but estimated stale reclaim would bring it below the threshold, prefer stale-evidence pruning instead of rewrite
8. if prompt usage is above auto threshold, stage compaction
9. if prompt usage is above force threshold, compaction is forced even if fold economics are weak

Default thresholds:

- soft notice: `50%`
- auto compaction: `80%`
- force compaction: `90%`
- target tail cap: `50%` of context window
- max recent tail tokens: `16384`

### Compaction Follow-Up Trigger

Inputs:

- context window tokens
- post-compaction prompt tokens
- consecutive compaction count

Rules:

1. if post-compaction prompt usage drops below the auto threshold, clear the auto-compaction stuck state
2. if post-compaction prompt usage stays above the auto threshold, keep the consecutive-compaction count
3. if consecutive ineffective compactions reaches `2`, pause auto compaction and require explicit operator/runtime intervention

### Rollback Trigger

Rollback exists for regressions after a previously applied rewrite, not for ordinary provider failure.

Inputs:

- latest rewrite regression classification
- whether a known-good rollback snapshot exists
- whether a rebuild source exists

Rules:

1. if a rewrite regression is confirmed and a known-good rollback snapshot exists, stage rollback
2. rewrite regression means a rewrite-layer truth problem, for example:
   - explicit operator/debugger rollback request
   - latest rewrite reference no longer points to valid source truth
   - applied-turn evidence is inconsistent with rewrite ledger expectations
   - semantic regression is confirmed against replay or debug truth
3. ordinary provider errors, tool errors, or terminal-schema rejection do not by themselves trigger rollback
4. if regression exists but no rollback snapshot exists, prefer resume rebuild if rebuild source exists
5. if neither rollback snapshot nor rebuild source exists, block explicitly

### Resume Rebuild Trigger

Resume rebuild exists for startup or recovery paths where persisted session truth cannot safely continue.

Inputs:

- restore status
- rebuild source availability

Rules:

1. if persisted session history restores cleanly and no rewrite regression exists, do nothing
2. if persisted session history is missing but rebuild source exists, stage resume rebuild
3. if persisted session history is invalid or incoherent and rebuild source exists, stage resume rebuild
4. if restore state is missing or invalid and no rebuild source exists, block explicitly

## Unexpected-Case Strategy

Unexpected cases must not silently fall through.

### Missing Usage Metrics

- auto compaction is not triggered
- stable prefix remains append-only
- runtime should surface the hold reason in debug or node status

### Missing Context Window

- auto compaction is not triggered
- this is treated as unsupported or incomplete runtime truth, not as permission to guess

### Overlapping Rewrite Gates

- if `SessionHistory` is already in non-ordinary rewrite mode, policy refuses a second concurrent rewrite trigger

### Stale Volatile Evidence Unknown

- if reclaim estimate is unavailable, policy may still compact by threshold
- absence of reclaim estimate is not treated as proof that reclaim is impossible

### Repeated Ineffective Compaction

- after two consecutive ineffective compactions, auto compaction pauses
- runtime must surface that stable prefix plus minimal tail is already too large
- operator action is then required: shrink anchors, reduce tool output, or change model/window

### Rewrite Regression Without Recovery Truth

- if regression exists and neither rollback snapshot nor rebuild source exists, policy returns explicit block
- no fallback rewrite and no silent continuation are allowed

### Invalid Persisted Session Truth

- invalid persisted state does not get auto-repaired in place
- if authoritative rebuild source exists, use resume rebuild
- otherwise block explicitly

## Runtime Binding Contract

Pure policy output must remain separate from session mutation.

Runtime consumer contract:

1. evaluate rewrite policy in `freehand-blocks`
2. if the action is rewrite-bearing, call the corresponding `SessionHistory` gate in `freehand-reason`
3. persist ledger evidence through existing session-history paths
4. never let runtime invent a fourth rewrite mode

Prompt usage source contract:

- provider adapters emit unified `TokenUsage`
- `freehand-blocks::prompt_tokens_from_usage` maps `TokenUsage.input_tokens` to rewrite-policy prompt usage
- `ReasonRewriteRuntime` may receive either explicit `prompt_tokens` or provider `TokenUsage`, but not both
- conflicting usage sources are explicit runtime errors, not silently reconciled

## Testing Direction

White-box tests must cover:

- soft notice threshold
- auto compaction threshold
- force compaction threshold
- stale-prune-preferred threshold case
- compaction pause after repeated ineffective folds
- rollback preferred over rebuild when safe snapshot exists
- rebuild preferred when restore is missing or invalid
- block returned when recovery truth is insufficient

Module black-box tests must cover:

- pure policy input to decision smoke
- restore/recovery decision smoke

Project black-box impact:

- runtime reason orchestration must eventually prove that session-history gates are reached only through policy-approved trigger paths

## Current Implementation Status

Current baseline:

- pure trigger policy lives in `freehand-blocks`
- runtime session-history mutation remains in `freehand-reason`
- `ReasonRewriteRuntime` consumes policy decisions and is the single baseline path that calls `SessionHistory::stage_compaction`, `SessionHistory::stage_rollback`, or `SessionHistory::stage_resume_rebuild`
- provider `TokenUsage` is accepted by `ReasonRewriteRuntime` through the shared `prompt_tokens_from_usage` conversion
- `freehand-testkit::ReasonRuntimeHarness` provides the project black-box path from provider semantic output to turn truth to usage-driven rewrite policy

Known remaining gaps after this design:

- stale volatile evidence pruning executor is still pending even though the trigger policy is now explicit
- production CLI/server runtime loop that supplies provider usage events and persisted recovery sources is still pending
