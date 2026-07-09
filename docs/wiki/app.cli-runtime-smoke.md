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
- for ADP turn samples, CLI connects to the same daemon `/adp`, creates an isolated sample session, subscribes to latest-turn updates, submits a success sample prompt, a tool-result-failure recovery prompt, a schema-mismatch polishing prompt, or a provider-retry prompt, verifies the matching terminal projection, then queries the sample session transcript to prove round/tool/schema/provider evidence
- for provider-retry online proof, scripts/verify-provider-retry-online.sh temporarily points S-profile provider config at a local Anthropic-compatible 500 fixture, runs the provider-retry ADP sample, verifies five upstream /v1/messages attempts plus error-center provider rows, then restores runtime config and S env
- for same-session continuation sample, CLI submits two prompts into one isolated session and verifies the second terminal answer uses a unique token from prior effective history
- for task lifecycle sample, CLI sends protocol-owned task mutation commands (`CreateTask`, `SubmitTaskReview`, `ApproveTaskReview`, `CloseTask`) through ADP, then verifies task owner truth through ADP task list/history queries
- for Phase 1 foundation sample, CLI drives protocol-owned TaskBoard, AgentBoard, ExecutionFact, SchedulerTick, and same-id verification commands/queries through ADP without UI or model prose
- for Phase 2A master-worker foundation sample, CLI drives protocol-owned worker agent creation, task assignment, claim-next with execution id, progress/blocked/recovering/review facts, reject, retry, approve, close, and same-id verification through ADP without UI or model prose
- for Phase 2B master poll foundation sample, CLI drives protocol-owned EventInbox query and MasterPoll command through ADP, verifies compact classifications and cursor persistence, then verifies the same cursor after daemon restart without UI or model prose
- Phase 2B sample create mode sends replay_from_start=true plus omitted EventInbox/MasterPoll limits to ignore stale persisted cursors and drain all pending rows before recording the cursor; finite limits are pagination and are not accepted as same-cursor closeout proof
- Phase 2B sample create mode reads the final persisted cursor with a non-replay owner-backed poll after the command poll, then uses that cursor for same-cursor verification
- for Phase 2C worker-control foundation sample, CLI drives protocol-owned WorkerControl commands through ADP against an already-running worker execution, then verifies control ledger query truth plus Task Center pause/resume/cancel consequences without UI or model prose
- for ADP session manage, CLI connects to the same daemon `/adp` and sends protocol-owned create, rename, archive, restore, delete-as-archive, or rollback command frames for no-UI session lifecycle diagnosis
- for ADP task query, CLI connects to the same daemon `/adp` and sends protocol-owned task list/history query frames for no-UI task truth diagnosis
- for ADP task subscribe, CLI connects to the same daemon `/adp` and sends protocol-owned task list subscription frames for no-UI task push diagnosis
- for ADP error query, CLI connects to the same daemon `/adp` and sends protocol-owned error-center query frames for no-UI metadata diagnosis

## Response Mainline

- config startup path prints selected-agent summary plus selected-provider metadata without exposing provider secret values
- reason E2E smoke prints scenario name, selected agent, rewrite outcome, rewrite version, and latest usage summary
- CLI output remains a terminal-facing projection, not debug ledger raw payload
- ADP smoke prints the observed subscription_accepted, subscription_event, query_result, and explicit failure frame sequence for no-UI diagnosis
- ADP turn samples print the observed command outcome plus the matching latest-turn projection and transcript evidence; the failure sample proves a failed tool result can continue to a successful terminal turn with rounds>=2, one or more unique tool executions, and one or more unique failed tool results instead of becoming an ADP/system failure; the schema-mismatch sample proves schema polishing was visible before terminal success; the provider-retry sample proves provider-domain retry/error evidence instead of schema/tool failure
- provider-retry online verifier prints the sample output, error-center projection, provider attempt count, and session id after proving config restoration is wired through the S-profile daemon lifecycle
- same-session continuation sample prints both turn ids, transcript count, and token recovery evidence
- task lifecycle sample prints task id, closed status, and required history event types
- Phase 1 foundation sample prints blocked task id, review task id, execution id, agent id, blocked/review/stale counts, lifecycle query evidence, and recovering-event evidence
- Phase 2A master-worker foundation sample prints task id, worker id, execution id, final closed status, ordered lifecycle events, review retry evidence, and same-id restart verification arguments
- Phase 2B master poll foundation sample prints task id, worker id, execution id, EventInbox cursor, persisted master cursor, classification kinds, and same-cursor restart verification arguments
- Phase 2C worker-control foundation sample prints task id, worker id, execution id, cancel control id, control event count/statuses, and Task Center consequence events for same-id restart verification
- ADP session manage prints command receipt status for session CRUD and rollback commands without creating a second source of session truth
- ADP task query prints task list count/task ids or task history event counts from protocol-owned query results
- ADP task subscribe prints accepted state plus task list count/task ids from the initial protocol-owned subscription event
- ADP error query prints error-center event count and domain/class/recovery/code/raw-hash summary without exposing raw metadata payloads

## Error Mainline

- invalid command shape returns explicit usage
- missing config or missing agent selection returns explicit config errors
- smoke runtime failures return explicit reason or runtime errors
- ADP connect, send, receive, and decode timeouts return explicit terminal errors
- ADP turn sample timeout, wrong terminal status, missing isolated-session transcript evidence, missing failed tool activity for the failure sample, missing schema retry evidence for the schema-mismatch sample, missing provider retry evidence for the provider-retry sample, or wrong error domain returns explicit terminal errors
- same-session continuation timeout, missing transcript, missing second turn, or missing token evidence returns explicit terminal errors
- task lifecycle timeout, missing task, non-closed status, or missing create/review/approve/close history event returns explicit terminal errors
- Phase 1 foundation sample timeout, missing TaskBoard/AgentBoard/Lifecycle evidence, missing blocked/review/stale projection, missing recovering history event, or same-id restart mismatch returns explicit terminal errors
- Phase 2A master-worker foundation sample timeout, no claimed task, missing execution id, missing blocked/recovering/review/reject/retry/approve/close history event, missing lifecycle state, or same-id restart mismatch returns explicit terminal errors
- Phase 2B master poll foundation sample timeout, missing EventInbox events, missing classification kinds, unexpected task status mutation, or same-cursor restart mismatch returns explicit terminal errors
- Phase 2B verification fails if replay after the persisted cursor returns events, because that means the create path reused a stale cursor or paginated instead of replaying and draining the backlog
- Phase 2C worker-control foundation sample timeout, missing worker-control events, missing pause/resume/cancel task consequences, task/execution/agent mismatch, or same-id restart mismatch returns explicit terminal errors
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
| 08 | `run_adp_turn_sample` | `apps/freehand-cli/src/main.rs` | parse ADP sample URL and sample kind | --url ws://.../adp --sample success|failure|schema-mismatch|provider-retry | terminal-facing sample result | CLI dispatcher | ADP sample runner | bound |
| 09 | `run_adp_turn_sample_async` | `apps/freehand-cli/src/main.rs` | submit sample prompts over ADP into an isolated sample session, verify matching terminal projection, query transcript evidence, and reject wrong-domain failures explicitly | ADP WebSocket URL plus sample kind | observed success, recovered failed-tool-result, schema-polishing, or provider-retry evidence with round/tool/schema/provider counts | ADP sample runner | daemon /adp | bound |
| 09a | `run_verify_provider_retry_online` | `scripts/verify-provider-retry-online.sh` | run a local provider-error fixture, temporarily update S-profile provider config, execute provider-retry ADP sample, verify error-center retry truth, and restore config/env | S-profile daemon on 127.0.0.1:4042 | terminal-facing provider retry proof with five mock attempts and provider error-center rows | operator/agent verifier | daemon /adp plus local mock provider | bound |
| 09b | `run_session_continue_sample / run_session_continue_sample_async` | `apps/freehand-cli/src/main.rs` | submit two prompts into one isolated session and query transcript evidence that the second answer used prior effective history | ADP WebSocket URL | terminal-facing two-turn continuation result | CLI dispatcher | daemon /adp | bound |
| 09c | `run_task_lifecycle_sample / run_task_lifecycle_sample_async` | `apps/freehand-cli/src/main.rs` | send protocol-owned task create/review/approve/close commands and query task list/history evidence that the task closed through owner truth | ADP WebSocket URL | terminal-facing task lifecycle result | CLI dispatcher | daemon /adp | bound |
| 09d | `run_phase1_foundation_sample / run_phase1_foundation_sample_async / run_phase1_foundation_verify_async` | `apps/freehand-cli/src/main.rs` | drive Phase 1 TaskBoard, AgentBoard, ExecutionFact, SchedulerTick, and same-id restart verification through ADP | ADP WebSocket URL plus optional verify ids | terminal-facing Phase 1 foundation evidence or explicit ADP/query failure | CLI dispatcher | daemon /adp | bound |
| 09e | `run_master_worker_foundation_sample / run_master_worker_foundation_sample_async / verify_master_worker_foundation_truth` | `apps/freehand-cli/src/main.rs` | drive Phase 2A master/worker task execution loop and same-id restart verification through ADP | ADP WebSocket URL plus optional verify ids | terminal-facing worker lifecycle evidence or explicit ADP/query failure | CLI dispatcher | daemon /adp | bound |
| 09f | `run_master_poll_foundation_sample / run_master_poll_foundation_sample_async / verify_master_poll_foundation_truth` | `apps/freehand-cli/src/main.rs` | drive Phase 2B EventInbox and MasterPoll loop, reread owner-backed final cursor after command poll, and perform same-cursor restart verification through ADP | ADP WebSocket URL plus optional verify cursor/task/execution/agent ids | terminal-facing master poll evidence or explicit ADP/query failure | CLI dispatcher | daemon /adp | bound |
| 10 | `run_adp_task_query` | `apps/freehand-cli/src/main.rs` | parse ADP task query URL and optional list/history filters | --url ws://.../adp plus optional task filters | selected task query command | CLI dispatcher | ADP task query runner | bound |
| 11 | `run_adp_task_query_async` | `apps/freehand-cli/src/main.rs` | send task list/history query over ADP and summarize the task projection | ADP WebSocket URL plus task query command | terminal-facing task list/history summary or explicit ADP failure | ADP task query runner | daemon /adp | bound |
| 12 | `run_adp_task_subscribe` | `apps/freehand-cli/src/main.rs` | parse ADP task subscribe URL and optional list filters | --url ws://.../adp [--status <status>] [--agent <id>] | selected task subscribe command | CLI dispatcher | ADP task subscribe runner | bound |
| 13 | `run_adp_task_subscribe_async` | `apps/freehand-cli/src/main.rs` | send task list subscription over ADP and summarize the first task list event | ADP WebSocket URL plus task subscription filters | terminal-facing task list subscription summary or explicit ADP failure | ADP task subscribe runner | daemon /adp | bound |
| 14 | `run_adp_error_query` | `apps/freehand-cli/src/main.rs` | parse ADP error query URL and optional session/trace/turn/domain filters | --url ws://.../adp --session <id> plus optional error-center filters | selected error-center query command | CLI dispatcher | ADP error query runner | bound |
| 15 | `run_adp_error_query_async` | `apps/freehand-cli/src/main.rs` | send error-center query over ADP and summarize UI-safe event rows | ADP WebSocket URL plus error-center query command | terminal-facing error-center event summary or explicit ADP failure | ADP error query runner | daemon /adp | bound |
| 16 | `run_adp_session_manage / run_adp_session_manage_async` | `apps/freehand-cli/src/main.rs` | send protocol-owned session CRUD or rollback command over ADP and summarize the command receipt | --url ws://.../adp --action create|rename|archive|restore|delete|rollback --session <id> plus optional title/cwd | terminal-facing session manage receipt or explicit ADP failure | CLI dispatcher | daemon /adp | bound |
| 09g | `run_worker_control_foundation_sample / run_worker_control_foundation_sample_async / verify_worker_control_foundation_truth` | `apps/freehand-cli/src/main.rs` | drive Phase 2C worker-control query/safe-point/pause/resume/cancel loop and same-id restart verification through ADP | ADP URL plus optional verify control/task/execution/agent ids | terminal-facing worker-control evidence or explicit ADP/query failure | CLI dispatcher | daemon /adp | bound |

## Sync Status Against Mainline Call

- CLI config startup path is implemented
- CLI reason E2E smoke path is implemented
- CLI ADP no-UI smoke path is implemented
- CLI ADP success/failure/schema-mismatch/provider-retry turn sample path is implemented; the failure sample requires isolated-session terminal success plus transcript evidence for rounds>=2, unique failed tool-result activity, and no system/provider terminal failure; schema-mismatch requires schema-polishing evidence; provider-retry requires provider-domain retry/error evidence
- provider retry online proof is repeatable through scripts/verify-provider-retry-online.sh, which rejects model prose by requiring five fixture provider attempts plus error.center provider rows
- CLI same-session continuation sample is implemented and verifies second-turn token recovery from prior effective history
- CLI task lifecycle sample is implemented and verifies closed task plus create/review/approve/close history evidence after protocol-owned task mutation commands
- CLI Phase 1 foundation sample is implemented and verifies TaskBoard, AgentBoard, ExecutionFact, SchedulerTick, and restart same-id evidence through protocol-owned ADP frames
- CLI Phase 2A master-worker foundation sample is implemented and verifies assign/claim/progress/blocked/recovering/review/reject/retry/approve/close and restart same-id proof through protocol-owned ADP frames
- CLI Phase 2B master poll foundation sample is implemented and verifies EventInbox, classifications, persisted cursor, and restart same-cursor evidence through protocol-owned ADP frames; S-profile online closeout is still required before claiming the phase complete
- CLI ADP task list/history query path is implemented for no-UI task truth diagnosis
- harness-backed app E2E smoke now exists before production CLI or server runtime loop
- remaining gap: production non-smoke command loop is still pending
- generated wiki must be regenerated from `docs/mainline-calls/app.cli-runtime-smoke.json` when this function-map truth changes
- CLI ADP task list subscribe path is implemented for no-UI task push diagnosis
- CLI ADP error-center query path is implemented for no-UI metadata diagnosis
- CLI Phase 2C worker-control foundation sample is implemented in integration tests and verifies control ledger plus pause/resume/cancel consequence evidence through protocol-owned ADP frames
