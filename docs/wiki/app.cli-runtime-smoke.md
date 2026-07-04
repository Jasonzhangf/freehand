# Wiki: `app.cli-runtime-smoke`

Generated from `docs/mainline-calls/app.cli-runtime-smoke.json`. Do not edit by hand.

- owner crate: `apps/freehand-cli`
- owner module: `apps/freehand-cli/src/main.rs`
- function map: `docs/function-maps/app.cli-runtime-smoke.md`
- generated wiki: `docs/wiki/app.cli-runtime-smoke.md`
- test design: `docs/testing/app.cli-runtime-smoke.md`

## Request Mainline

- operator invokes `freehand-cli`
- CLI parses the command shape and selects one agent plus its bound provider from `~/.freehand/config.toml`
- for reason E2E smoke, CLI builds one scripted runtime harness request
- provider semantic outputs enter the harness, then reason turn truth, then rewrite runtime, then terminal reporting
- for ADP smoke, CLI connects to a caller-provided daemon `/adp` WebSocket URL and sends protocol-owned subscribe, query, and query-as-command frames
- for ADP turn samples, CLI connects to the same daemon `/adp`, creates an isolated sample session, subscribes to latest-turn updates, submits a success sample prompt or a tool-result-failure recovery prompt, verifies the matching terminal projection, then queries the sample session transcript to prove round/tool evidence
- for ADP session manage, CLI connects to the same daemon `/adp` and sends protocol-owned create, rename, archive, restore, delete-as-archive, or rollback command frames for no-UI session lifecycle diagnosis
- for ADP task query, CLI connects to the same daemon `/adp` and sends protocol-owned task list/history query frames for no-UI task truth diagnosis
- for ADP task subscribe, CLI connects to the same daemon `/adp` and sends protocol-owned task list subscription frames for no-UI task push diagnosis
- for ADP error query, CLI connects to the same daemon `/adp` and sends protocol-owned error-center query frames for no-UI metadata diagnosis

## Response Mainline

- config startup path prints selected-agent summary plus selected-provider metadata without exposing provider secret values
- reason E2E smoke prints scenario name, selected agent, rewrite outcome, rewrite version, and latest usage summary
- CLI output remains a terminal-facing projection, not debug ledger raw payload
- ADP smoke prints the observed subscription_accepted, subscription_event, query_result, and explicit failure frame sequence for no-UI diagnosis
- ADP turn samples print the observed command outcome plus the matching latest-turn projection and transcript evidence; the failure sample proves a failed tool result can continue to a successful terminal turn with rounds>=2, one or more unique tool executions, and one or more unique failed tool results instead of becoming an ADP/system failure
- ADP session manage prints command receipt status for session CRUD and rollback commands without creating a second source of session truth
- ADP task query prints task list count/task ids or task history event counts from protocol-owned query results
- ADP task subscribe prints accepted state plus task list count/task ids from the initial protocol-owned subscription event
- ADP error query prints error-center event count and domain/class/recovery/code/raw-hash summary without exposing raw metadata payloads

## Error Mainline

- invalid command shape returns explicit usage
- missing config or missing agent selection returns explicit config errors
- smoke runtime failures return explicit reason or runtime errors
- ADP connect, send, receive, and decode timeouts return explicit terminal errors
- ADP turn sample timeout, wrong terminal status, missing isolated-session transcript evidence, missing failed tool activity for the failure sample, or system/provider terminal failure returns explicit terminal errors
- ADP session manage failures print explicit ADP failure code/message instead of treating session mutation errors as empty success
- rewrite recovery block is reported as explicit blocked outcome, not disguised as success
- ADP query-as-command must return ingress_command_kind_mismatch, proving command/query separation without mutation
- ADP task query failures print explicit ADP failure code/message instead of treating missing task truth as an empty success
- ADP task subscribe failures print explicit ADP failure code/message instead of treating missing task truth as an empty success
- ADP error query failures print explicit ADP failure code/message instead of treating metadata read failure as an empty success

## Shared Multi-Reference Functions

- `ReasonRuntimeHarness::run_provider_turn`
  - owner: `crates/freehand-testkit/src/lib.rs`
  - purpose: route provider semantic outputs through turn truth into usage-driven rewrite policy
  - allowed callers: CLI smoke command, project tests
  - related tests: CLI reason E2E smoke tests
  - why shared: app and project tests must reuse one runtime harness path
- `ReasonRuntimeHarness::apply_resume_rebuild`
  - owner: `crates/freehand-testkit/src/lib.rs`
  - purpose: route restore status into resume-rebuild or block decision
  - allowed callers: CLI smoke command, project tests
  - related tests: CLI recovery-block smoke tests
  - why shared: recovery smoke must reuse one runtime harness path

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run` | `apps/freehand-cli/src/main.rs` | parse CLI command and dispatch config startup or reason E2E smoke | CLI args | selected command path | shell/operator | CLI dispatcher | bound |
| 02 | `load_default_config` | `crates/freehand-config/src/lib.rs` | load runtime config from `~/.freehand/config.toml` | runtime home config path | selected config truth | CLI dispatcher | config owner | bound |
| 03 | `run_reason_e2e_smoke` | `apps/freehand-cli/src/main.rs` | build scripted E2E runtime harness request from selected agent | selected agent plus scenario | terminal-facing smoke summary | CLI dispatcher | app smoke runner | bound |
| 04 | `ReasonRuntimeHarness::run_provider_turn` | `crates/freehand-testkit/src/lib.rs` | route provider usage into turn truth and rewrite policy | scripted provider outputs plus compaction scenario | turn truth plus optional compaction outcome | app smoke runner | testkit harness | bound |
| 05 | `ReasonRuntimeHarness::apply_resume_rebuild` | `crates/freehand-testkit/src/lib.rs` | route restore state into recovery policy | restore status plus optional rebuild payload | recovery outcome | app smoke runner | testkit harness | bound |
| 06 | `run_adp_smoke` | `apps/freehand-cli/src/main.rs` | parse ADP smoke URL and run a bounded no-UI WebSocket smoke | --url ws://.../adp | terminal-facing ADP smoke summary | CLI dispatcher | ADP smoke runner | bound |
| 07 | `run_adp_smoke_async` | `apps/freehand-cli/src/main.rs` | connect to daemon ADP, send subscribe/query/query-as-command frames, and collect required responses | ADP WebSocket URL | observed frame sequence or explicit error | ADP smoke runner | daemon /adp | bound |
| 08 | `run_adp_turn_sample` | `apps/freehand-cli/src/main.rs` | parse ADP sample URL and sample kind | --url ws://.../adp --sample success|failure | terminal-facing sample result | CLI dispatcher | ADP sample runner | bound |
| 09 | `run_adp_turn_sample_async` | `apps/freehand-cli/src/main.rs` | submit success/failure sample prompts over ADP into an isolated sample session, verify matching terminal projection, query transcript evidence, and reject system/provider terminal failure explicitly | ADP WebSocket URL plus sample kind | observed success projection or recovered failed-tool-result projection with round/tool counts | ADP sample runner | daemon /adp | bound |
| 10 | `run_adp_task_query` | `apps/freehand-cli/src/main.rs` | parse ADP task query URL and optional list/history filters | --url ws://.../adp plus optional task filters | selected task query command | CLI dispatcher | ADP task query runner | bound |
| 11 | `run_adp_task_query_async` | `apps/freehand-cli/src/main.rs` | send task list/history query over ADP and summarize the task projection | ADP WebSocket URL plus task query command | terminal-facing task list/history summary or explicit ADP failure | ADP task query runner | daemon /adp | bound |
| 12 | `run_adp_task_subscribe` | `apps/freehand-cli/src/main.rs` | parse ADP task subscribe URL and optional list filters | --url ws://.../adp [--status <status>] [--agent <id>] | selected task subscribe command | CLI dispatcher | ADP task subscribe runner | bound |
| 13 | `run_adp_task_subscribe_async` | `apps/freehand-cli/src/main.rs` | send task list subscription over ADP and summarize the first task list event | ADP WebSocket URL plus task subscription filters | terminal-facing task list subscription summary or explicit ADP failure | ADP task subscribe runner | daemon /adp | bound |
| 14 | `run_adp_error_query` | `apps/freehand-cli/src/main.rs` | parse ADP error query URL and optional session/trace/turn/domain filters | --url ws://.../adp --session <id> plus optional error-center filters | selected error-center query command | CLI dispatcher | ADP error query runner | bound |
| 15 | `run_adp_error_query_async` | `apps/freehand-cli/src/main.rs` | send error-center query over ADP and summarize UI-safe event rows | ADP WebSocket URL plus error-center query command | terminal-facing error-center event summary or explicit ADP failure | ADP error query runner | daemon /adp | bound |
| 16 | `run_adp_session_manage / run_adp_session_manage_async` | `apps/freehand-cli/src/main.rs` | send protocol-owned session CRUD or rollback command over ADP and summarize the command receipt | --url ws://.../adp --action create|rename|archive|restore|delete|rollback --session <id> plus optional title/cwd | terminal-facing session manage receipt or explicit ADP failure | CLI dispatcher | daemon /adp | bound |

## Sync Status Against Mainline Call

- CLI config startup path is implemented
- CLI reason E2E smoke path is implemented
- CLI ADP no-UI smoke path is implemented
- CLI ADP success/failure turn sample path is implemented; the failure sample requires isolated-session terminal success plus transcript evidence for rounds>=2, unique failed tool-result activity, and no system/provider terminal failure
- CLI ADP task list/history query path is implemented for no-UI task truth diagnosis
- harness-backed app E2E smoke now exists before production CLI or server runtime loop
- remaining gap: production non-smoke command loop is still pending
- generated wiki must be regenerated from `docs/mainline-calls/app.cli-runtime-smoke.json` when this function-map truth changes
- CLI ADP task list subscribe path is implemented for no-UI task push diagnosis
- CLI ADP error-center query path is implemented for no-UI metadata diagnosis
