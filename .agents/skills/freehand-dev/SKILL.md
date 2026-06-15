---
name: freehand-dev
description: Use when working inside the Freehand repo on architecture, harness, config, provider, reasoning, node topology, UI protocol, gates, or test infrastructure. Enforces Freehand's contracts-plus-blocks-plus-orchestrators architecture, feature map ownership, directory locks, replay-first debugging, and required validation workflow.
---

# Freehand Dev

Use this skill for any non-trivial work in this repo.

## Start

1. Read `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`.
2. Read `docs/architecture/feature-map.md`.
3. Read the feature's bound function-map doc before non-trivial implementation or debug.
4. Identify the target `feature_id`, owning crate, allowed paths, forbidden paths, required checks, debug artifacts, runtime paths, `test_design_doc`, `function_map_doc`, and `lifecycle_checks`.
5. If ownership is unclear, fix the map first or stop and ask.
6. Before coding, ask three questions:
   - is the information sufficient
   - is the logic closed-loop
   - is lifecycle management complete
7. If any answer is no, do read-only tracing and source search first. Ask the user only after read-only search cannot close the gap.
8. Before implementation for each module feature, write or update its test-design record first.
9. Test-design record must capture:
   - target feature and owner
   - lifecycle and logic path
   - white-box coverage plan
   - module black-box coverage plan
   - project black-box coverage impact
   - known gaps and non-goals
10. Function-map record must capture:
   - owner crate and owner module
   - code-bound entry symbols
   - request mainline
   - response mainline
   - error mainline
   - shared multi-reference functions and why they are reused
   - call table bound to code paths
11. If another worker cannot read the test design and function map and understand where coverage lives, where the mainline runs, and what remains risky, the design is incomplete.

## Runtime Home

- Runtime home is `~/.freehand`.
- Use standard runtime paths:
  - `~/.freehand/state`
  - `~/.freehand/state/config`
  - `~/.freehand/state/turns`
  - `~/.freehand/state/ui`
  - `~/.freehand/logs`
  - `~/.freehand/ledgers`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/ledgers/providers`
  - `~/.freehand/replays`
  - `~/.freehand/cache`
  - `~/.freehand/cache/session-index`
  - `~/.freehand/tmp`
- Runtime evidence belongs there, not in random ad hoc paths.
- Directory routes:
  - debug docs: `docs/debug/`
  - runtime docs: `docs/runtime/`
  - config docs: `docs/config/`
  - design docs: `docs/design/`
  - provider protocol references: `docs/references/provider-protocols/`
- Config source:
  - `~/.freehand/config.toml`
  - multi-agent layout uses `[agents.<name>]`

## Architecture Rules

- Global semantic types live in `crates/freehand-contracts`.
- `crates/freehand-contracts` owns cross-module shared semantic types, shared IDs, cross-module error contracts, and module-level error base contracts.
- `crates/freehand-contracts` does not own config schema, UI projection, or debug/trace envelope.
- Shared pure semantic logic lives in `crates/freehand-blocks`.
- Before adding any function, inspect existing blocks and owner crates first.
- Do not add temporary helpers to `crates/freehand-reason` or `crates/freehand-node`.
- If logic smells reusable, semantic, parser-like, builder-like, validator-like, or projector-like, put it in `crates/freehand-blocks`.
- Provider wire DTOs stay inside `crates/freehand-provider-*`.
- Provider semantic layer supports OpenAI-compatible and Anthropic first.
- Provider payload wire DTOs stay private to provider adapters.
- Turn semantics stay inside `crates/freehand-reason`.
- Turn truth is stored per turn and projected into conversation view.
- Only `crates/freehand-reason` may write session truth.
- Master/slave runtime stays inside `crates/freehand-node`.
- master/slave is input-permission configuration.
- local multiple agents are managed by `config.toml`, and one `config.toml` may define multiple local agents.
- config source path is only `~/.freehand/config.toml`.
- one process starts one agent, chosen by CLI agent name.
- current first version master/slave scope is local one-master one-slave only.
- pairing transport is WebSocket handshake.
- each agent has a startup configuration file that decides its startup mode.
- whichever side is configured as `master` accepts user input and dispatches to local sub-agents or paired remote slaves.
- paired `slave` mode accepts input only from its paired source, which may be a user or another master.
- slave startup config includes at least `name`, `mode`, and `pair_token`.
- `allowed_pair_ip` is optional. If omitted, source IP is not filtered.
- `pair_token` must be configured as an environment variable reference.
- slave pairing source is fixed by config and changing it requires restart.
- if slave loses pairing, it keeps listening for later re-pairing.
- master may send task, query progress, directly talk, and subscribe to slave turn stream.
- UI code must consume `crates/freehand-ui-protocol`, never provider crates directly.
- First version UI scope is CLI plus WebUI.
- CLI and WebUI may render different views, but they must share one `freehand-ui-protocol` truth.
- No fallback, no silent downgrade, no duplicate semantic logic in orchestrators.
- Start development and debugging from the function map owner, never from random grep alone.
- Request/response/error mainlines must have logic descriptions in the function map, not only crate names.
- Any function used from multiple call sites must have one shared semantic description in the function map.
- function-call tables must bind to code symbols or explicitly say implementation binding is still pending.
- New features and bug fixes both require lifecycle thinking, not just local code patches.
- In provider work, preserve raw provider events in debug mode and rely on unified semantic events for normal operation.
- In provider work, read local official protocol snapshots under `docs/references/provider-protocols/` before inventing wire behavior.
- In reason-turn work, provider `finish_reason=stop/end_turn` is not enough to stop. Completion schema decides stop.
- Reason context planning follows locked Reasonix/Codex direction:
  - stable prefix stays stable across ordinary turns
  - only explicit rewrite events may change prefix layout
  - prefer subagent search final-report enrichment over injecting raw exploration transcripts
  - admit subagent context into parent turns only as typed final conclusion segments
- `reason.rewrite-policy` in `freehand-blocks` owns when compaction / rollback / resume rebuild should trigger; `freehand-reason` only owns `SessionHistory` mutation after that decision
- `ReasonRewriteRuntime` in `freehand-reason` is the baseline consumer that may call `SessionHistory::stage_*` from policy-approved decisions
- Provider `TokenUsage` enters rewrite policy only through `freehand-blocks::prompt_tokens_from_usage`; do not hand-roll provider usage interpretation in runtime or UI
- `freehand-testkit` may host project black-box runtime harnesses before production CLI/server loops exist; keep harness behavior aligned with function maps and test design
- `reason.session-history` inside `freehand-reason` owns base context, rewrite mode/version, rewrite ledger, and persisted session-history snapshots.
- `reason.persistence` inside `freehand-reason` owns authoritative snapshot and reason-ledger persistence; UI sidecars and provider raw ledgers remain derived or debug-only.
- Non-ordinary rewrite modes may enter planner only through explicit session-history gate methods for compaction, rollback, or resume rebuild.
- `freehand-reason` and provider adapter crates must remain independent; neither side may depend on the other's implementation crate.
- Metadata/debug/provider/cache fields and request-chain content fields must stay hard-isolated by type and builder ownership.
- Metadata must not be smuggled into request text, and request content must not be recovered from metadata/debug fields.
- Restart recovery must use authoritative snapshots plus reason-ledger replay; UI sidecars and provider raw ledgers are never recovery truth.
- In UI protocol work, query and subscribe must stay separate, and source identity fields must remain explicit.
- Shared contract types should default to serializable, replayable, and persistable unless a higher-priority truth source says otherwise.

## Debug Workflow

- Start from `feature_id`, owner, `debug_artifacts`, and runtime paths in the function map.
- Use repo routes first:
  - `docs/debug/debug-playbook.md`
  - `docs/runtime/runtime-directories.md`
- When debugging, capture both semantic and scene position.
- Prefer replayable fixtures and event ledger evidence over plain logs.
- Check `~/.freehand` evidence paths before inventing new debug output locations.
- If a failure repeats twice, search externally for 3-5 candidate fixes before continuing to grind on one path.
- Keep asking during debug:
  - do I have enough information
  - is the logic path closed-loop
  - is lifecycle management complete
- If not, continue read-only source tracing first. Ask the user only when repo truth and runtime evidence cannot answer.

## Validation Workflow

- Test design and test implementation must evolve together in the same task when feature truth changes.
- Function-map logic description and code binding must evolve together with implementation in the same task when feature truth changes.
- Do not add implementation without first making the test-design path inspectable in docs.
- Before claiming completion, run the feature's required checks.
- Before claiming completion, satisfy the feature's `lifecycle_checks`.
- Before claiming completion, run the feature's mapped test stack:
  - module white-box tests
  - module black-box tests
  - project black-box tests
- Minimum baseline:
  - `cargo build --workspace`
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo run -p xtask -- gates check`
- For state machine, stream, timeout, retry, error projection, or resource cleanup changes, add both positive and negative tests.
- For provider recovery logic, classify errors as recoverable, unrecoverable, or periodic-recoverable. Periodic windows use provider-supplied seconds first, otherwise configured defaults.
- For reason-turn stop logic, validate completion schema before terminal acceptance. Reject and explain invalid terminal submissions.
- UI protocol black-box tests must cover standard user-visible flows, not only internal event wiring.
- `cargo test --workspace` is the regression umbrella and must carry white-box plus module/project black-box coverage as those tests are added.
- When tests are added, changed, or found incomplete, update the module's test-design record in the same change set.
- When request/response/error mainlines or shared function usage change, update the function-map doc in the same change set.
- When context-segment admission, cache-shape policy, or subagent context flow changes, update `reason.context-planner` design, test design, function map, and memory in the same task.

## Memory Workflow

- Record exploration in `note.md`.
- Promote only verified, durable conclusions into `MEMORY.md`.
- Keep `CACHE.md` short and current for the next session.
- If feature truth changed, update function map, architecture docs, skill workflow, and memory files in the same task.

## Closure Checklist

Use this checklist for both new features and bug fixes:

- information sufficient
- logic closed-loop
- lifecycle management complete
- owner and function map updated if truth changed
- function-map call table and symbol binding still match code
- metadata/request isolation still holds for cross-module calls
- test-design record updated and still matches implementation
- runtime/debug evidence path still valid

If any line is not true, do not claim completion.
