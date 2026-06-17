# Wiki: `reason.rewrite-policy`

Generated from `docs/mainline-calls/reason.rewrite-policy.json`. Do not edit by hand.

- owner crate: `crates/freehand-blocks`
- owner module: `crates/freehand-blocks/src/rewrite_policy.rs`
- function map: `docs/function-maps/reason.rewrite-policy.md`
- generated wiki: `docs/wiki/reason.rewrite-policy.md`
- test design: `docs/testing/reason.rewrite-policy.md`

## Request Mainline

- runtime or orchestrator gathers rewrite trigger facts
- provider usage enters as shared `TokenUsage`, then `prompt_tokens_from_usage` extracts prompt usage from `input_tokens`
- project black-box harness can feed provider semantic outputs into reason turn truth before evaluating rewrite policy
- facts stay on the metadata/debug/runtime side, not in request text
- rewrite policy classifies whether the next action is hold, soft notice, prune-only, compaction, rollback, resume rebuild, or explicit block
- if the policy selects a rewrite-bearing action, runtime later calls the matching `SessionHistory` gate in `freehand-reason`

## Response Mainline

- compaction trigger decision returns one typed decision with explicit thresholds and reason
- compaction follow-up decision returns whether auto compaction should reset, remain active, or pause
- recovery decision returns whether to do nothing, rollback, resume rebuild, or block
- runtime consumer returns a staged rewrite ledger record only when a policy-approved rewrite gate was actually called
- usage-source conflicts return explicit errors before policy mutation is attempted

## Error Mainline

- missing usage or context-window truth does not silently compact
- conflicting prompt usage sources are rejected
- overlapping non-ordinary rewrite mode does not admit a second rewrite
- rewrite regression without rollback or rebuild truth returns explicit block
- invalid restore truth must resolve to resume rebuild or block, not hidden repair

## Shared Multi-Reference Functions

- `decide_compaction_trigger`
  - owner: `crates/freehand-blocks`
  - purpose: classify ordinary-turn compaction trigger state from prompt usage, window size, stale reclaim estimate, and rewrite guardrails
  - allowed callers: runtime/orchestrator, tests
  - related tests: compaction threshold tests, prune-preferred tests, paused-auto-compaction tests
  - why shared: compaction trigger semantics must stay out of orchestrator glue and aligned across runtime consumers
- `assess_compaction_follow_up`
  - owner: `crates/freehand-blocks`
  - purpose: decide whether compaction reset or compaction pause should happen after observing post-compaction usage
  - allowed callers: runtime/orchestrator, tests
  - related tests: repeated ineffective compaction tests
  - why shared: compaction loop-stop semantics must not be duplicated
- `decide_recovery_rewrite`
  - owner: `crates/freehand-blocks`
  - purpose: classify rollback, resume rebuild, or block on restore/rewrite-regression paths
  - allowed callers: runtime/orchestrator, tests
  - related tests: restore-invalid rebuild tests, rollback-vs-rebuild tests, explicit-block tests
  - why shared: recovery/rewrite trigger semantics must stay separate from session-history mutation code
- `prompt_tokens_from_usage`
  - owner: `crates/freehand-blocks`
  - purpose: convert provider-normalized `TokenUsage.input_tokens` into rewrite-policy prompt token pressure
  - allowed callers: freehand-reason, runtime/orchestrator, tests
  - related tests: provider-usage prompt-token conversion tests, conflicting usage source tests
  - why shared: all provider families must feed compaction pressure through one usage interpretation path

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `decide_compaction_trigger` | `crates/freehand-blocks/src/rewrite_policy.rs` | decide whether ordinary-turn pressure should remain append-only, warn, prune stale evidence, or stage compaction | prompt usage plus context window plus stale reclaim estimate plus rewrite guard state | typed compaction trigger decision | runtime/orchestrator | rewrite-policy block | bound |
| 02 | `assess_compaction_follow_up` | `crates/freehand-blocks/src/rewrite_policy.rs` | decide whether auto-compaction state resets or pauses after a compaction attempt | post-compaction prompt usage plus context window plus consecutive compaction count | typed compaction follow-up decision | runtime/orchestrator | rewrite-policy block | bound |
| 03 | `decide_recovery_rewrite` | `crates/freehand-blocks/src/rewrite_policy.rs` | decide whether restore/rewrite-regression path should rollback, resume rebuild, or block | restore status plus rewrite regression state plus rollback/rebuild truth availability | typed recovery rewrite decision | runtime/orchestrator | rewrite-policy block | bound |
| 04 | `prompt_tokens_from_usage` | `crates/freehand-blocks/src/rewrite_policy.rs` | convert provider-normalized usage into compaction prompt pressure | `TokenUsage.input_tokens` | `prompt_tokens` or usage error | `ReasonRewriteRuntime` | usage policy block | bound |
| 05 | `ReasonRewriteRuntime::apply_compaction_policy` | `crates/freehand-reason/src/rewrite_runtime.rs` | consume compaction trigger decision and call session-history compaction gate only when approved | session history plus runtime compaction facts plus optional provider usage plus optional rewrite payload | typed decision plus optional rewrite ledger record | runtime/orchestrator | usage policy plus rewrite policy plus session-history gate | bound |
| 06 | `ReasonRewriteRuntime::record_compaction_follow_up` | `crates/freehand-reason/src/rewrite_runtime.rs` | update soft-notice, consecutive-compaction, and auto-pause state after compaction observation | runtime rewrite state plus post-compaction usage | typed follow-up decision plus updated runtime state | runtime/orchestrator | rewrite policy | bound |
| 07 | `ReasonRewriteRuntime::apply_recovery_policy` | `crates/freehand-reason/src/rewrite_runtime.rs` | consume recovery decision and call rollback/resume-rebuild gate only when approved | session history plus recovery facts plus optional rewrite payloads | typed decision plus optional rewrite ledger record | runtime/orchestrator | rewrite policy plus session-history gate | bound |
| 08 | `SessionHistory::stage_compaction` | `crates/freehand-reason/src/session_history.rs` | mutate session rewrite truth after policy-approved compaction | compacted base context plus reason | updated session history plus rewrite ledger record | `ReasonRewriteRuntime` | session-history gate | bound |
| 09 | `SessionHistory::stage_rollback` | `crates/freehand-reason/src/session_history.rs` | mutate session rewrite truth after policy-approved rollback | rollback base context plus reason plus reference turn id | updated session history plus rewrite ledger record | `ReasonRewriteRuntime` | session-history gate | bound |
| 10 | `SessionHistory::stage_resume_rebuild` | `crates/freehand-reason/src/session_history.rs` | mutate session rewrite truth after policy-approved resume rebuild | rebuilt base context plus reason plus resume source | updated session history plus rewrite ledger record | `ReasonRewriteRuntime` | session-history gate | bound |
| 11 | `ReasonRuntimeHarness::run_provider_turn` | `crates/freehand-testkit/src/lib.rs` | black-box route provider semantic outputs through reason turn truth into usage-driven rewrite policy | provider semantic outputs plus compaction input | turn truth plus latest usage plus optional compaction outcome | project tests | reason engine plus rewrite runtime | bound |
| 12 | `ReasonRuntimeHarness::apply_resume_rebuild` | `crates/freehand-testkit/src/lib.rs` | black-box route recovery restore status into resume-rebuild/block policy | restore status plus optional rebuild payload | recovery decision plus optional rewrite ledger record | project tests | rewrite runtime | bound |

## Sync Status Against Mainline Call

- pure rewrite trigger policy baseline is implemented in `freehand-blocks`
- compaction trigger, compaction follow-up, and recovery/rewrite decisions are bound to concrete symbols
- runtime consumer wiring through `ReasonRewriteRuntime` is implemented in `freehand-reason`
- provider usage to prompt-pressure conversion is bound through `prompt_tokens_from_usage`
- project black-box harness is implemented in `freehand-testkit`
- remaining gap: production CLI/server loop must supply real provider usage events, stale-prune payloads, rollback snapshots, and rebuild sources
- generated wiki must be regenerated from `docs/mainline-calls/reason.rewrite-policy.json` when this function-map truth changes
