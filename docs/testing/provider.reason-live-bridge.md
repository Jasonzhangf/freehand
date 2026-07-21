# Test Design: `provider.reason-live-bridge`

- feature_id: `provider.reason-live-bridge`
- owner: `crates/freehand-runtime`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `request_context.build_provider_request`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `request_context.build_provider_request` | bound | `cargo test -p freehand-runtime live_bridge -- --nocapture` covers provider descriptor, request/tool/terminal metadata, schema retry, provider retry, provider retry backoff cancellation, tool re-entry, and master framework request-contract tests | `cargo test -p freehand-runtime live_bridge -- --nocapture` plus `cargo test -p freehand-runtime provider_retry_backoff_sleep_observes_live_cancel_token -- --nocapture` cover selected provider bridge smokes for text/tool/writable/checkpoint/schema/provider-retry/task-tool paths and interruptible retry wait | `scripts/verify-provider-retry-online.sh` covers CLI/daemon live bridge proof through provider -> reason -> tool -> persistence -> UI projection for provider retry behavior |

- lifecycle path under test:
  - selected config resolves one anthropic provider
  - runtime-owned live bridge restores existing session truth or explicitly creates a new session when recovery truth is missing
  - one or more reason rounds start and build provider payloads
  - provider semantic request is built from round truth
  - provider raw response/error/event bodies are captured through raw-capable executor callbacks and written into debug-only provider ledgers
  - first tool-capable request advertises implemented schemas from the Reasonix-aligned tool registry
  - anthropic executor runs single-shot or SSE request
  - provider-neutral outputs are written back into the active round and broadcast
  - completed implemented registry tool calls are executed, written as success or failed tool-result re-entry, persisted, and passed to the next provider request
  - completion schema is parsed from tagged text only after a terminal-candidate finish reason such as `stop` or `end_turn`, then either accepted, rejected with type-aware field feedback plus UI-visible retry waiting projection, or continued
  - terminal live turns are materialized through `ReasonPersistence`
  - runtime dispatch projects the final turn into shared `UiProtocolState`
- white-box plan:
  - provider descriptor derivation
  - unsupported provider rejection
  - metadata center bootstrap from `~/.freehand/ledgers/metadata`
  - runtime-owned restore/request/tool/terminal lifecycle metadata writes
  - runtime-owned context-planning started/completed lifecycle metadata writes
    before turn-start/provider-request build, plus segment-level
    started/completed/failed metadata for completion, control, tool guidance,
    instruction capability, task-space snapshot, and original-task segments,
    without prompt or provider payload leakage
  - runtime-owned restore/request/tool/terminal lifecycle debug snapshot emission
  - context-planning and context-segment debug snapshots project to UI
    model-waiting activity so a live submit is not silent while request context
    is being prepared and the exact segment can be identified
  - runtime-owned metadata write failure is explicit and aborts the live bridge
  - single-shot live-bridge mock path
  - SSE live-bridge mock path
  - broadcast capture path
  - incremental stream apply path proving broadcast can happen before stream completion
  - cancellation before tool execution path
  - cancellation before terminal persistence path
  - cancellation during provider retry backoff path
  - missing completion-schema rejection then success path
  - invalid completion-schema rejection then success path
  - missing completion-schema feedback must be sent to the next provider request with the missing `<freehand_completion>` tag requirement
  - invalid-schema rejection feedback must be sent to the next provider request with concrete missing field names, not only a generic retry prompt
  - schema/no-schema response mismatch must remain `CompletionSchemaRejected` plus `error.center` polishing guidance (`repair_schema` recovery action internally); it must not be projected as provider failure or `fail_turn`
  - nullable optional fields in completion and control-status schemas are
    treated as absent, while non-null wrong types remain explicit field-level
    mismatches
  - invalid control-status schema is paired back to the model as a
    schema-polishing retry with observable waiting feedback; it must not be
    returned as `ProviderRequestBuildFailed` or terminate the live turn
  - terminal-candidate-only schema parsing path proving `tool_use` responses do not trigger schema retry
  - `claim=continue` next-round path
  - retry-exhausted failed terminal path
  - restore-before-turn path
  - restored same-session follow-up request includes effective historical turns in the next provider request
  - restored prompt context for repaired multi-round logical turns keeps only the latest repaired round and excludes superseded failed attempt text
  - long operator prompts for master task orchestration reach the provider request with their tail sentinel intact instead of failing the `original-task` segment with a fixed 128-token budget
  - long previous-round visible output reaches the next provider request with its tail sentinel intact instead of failing `previous-visible-output` with a fixed 512-token budget
  - every live round includes completion contract, control status contract, runtime tool guidance, and the task contract before volatile carryover
  - `live_context::base_live_context_segments` owns live segment construction in
    `crates/freehand-runtime/src/live_context.rs`; the `src/lib.rs`
    orchestrator must not duplicate those builders
  - first-round `live_context::base_live_context_segments_with_observer` emits
    owner-safe segment build observations and `run_live_provider_reason_turn`
    records them as metadata/debug without prompt text, provider payload, or
    segment content leakage
  - every Master live round includes TaskSpaceSnapshot from task owner
    `TaskRuntime::query_task_space_snapshot`, with configured Worker, known
    tasks, agents, newest recent events, valid status filters, and the explicit
    no-`status="all"` instruction before the original task; the live context
    path must not boot full TaskRuntime, replay scheduler facts, or page
    EventInbox just to build prompt context
  - master-task prompt contract, exposed `task` tool schema, tool field descriptions, dispatch conditions, workspace-boundary rule, concurrency/flow-control guidance, cross-workspace sample, and success/error/retry samples are present in the first provider request before any model task creation decision
  - master-task request contract tells the model every task call needs
    top-level `op`, shows valid create/assign examples, and says `target_cwd`
    should prefer an expanded absolute existing repository/workspace path while
    accepting leading-~/symlink aliases only when they resolve to an existing
    workspace
  - master-autonomous success path: mock provider emits `task` tool calls for create_agent/create/assign/claim_next/running/review_ready/approve/close, each tool result is paired back to the next provider request, and Task Center truth closes only after review approval
  - master-autonomous execution-error path: mock provider emits a worker `blocked` execution fact, runtime returns it as a normal tool result, and Task Center truth remains blocked without review approval or close
  - master-autonomous rejected-review path: mock provider emits review_ready, reject, recovering with retry_count, second review_ready, approve, and close; Task Center truth preserves the reject-before-retry-before-close event order
  - live turn start/provider-output/schema-rejection/terminal persistence writes
  - provider raw debug-ledger write path for single-shot response bodies and SSE event bodies
  - provider raw debug-ledger failure path is explicit and aborts the live bridge
  - registry-backed tool schema export path
  - registry-backed tool schema fingerprint reaches planner diagnostics
  - master-safe tool schema export is framework-only: it includes `task` and
    `timer`, and omits file/search/write tools, unsandboxed `bash`,
    `todo_write`, and `complete_step`
  - injected Master file/search/write, shell, `todo_write`, or `complete_step`
    calls return one paired failed capability-boundary tool result that
    instructs the model to use `task` with a worker; it does not execute, leak
    file content, fail, or terminalize the turn
  - `task` remains executable when the requested target cwd is outside the master
    runtime home so the model can create and assign delegated work
  - a hidden/forbidden `read_file` or `bash` call returns a paired failed tool
    result with worker delegation guidance instead of executing
  - implemented registry read-only tool execution path
  - implemented registry read-only tool execution failure path returns `ToolResultStatus::Failed`, passes `is_error=true` to Anthropic, and lets the model continue to a later terminal schema
  - incomplete `tool_use` path returns `ToolResultStatus::Failed`, passes `is_error=true` to Anthropic, keeps `schema_rejections=0`, and lets the model continue to a later terminal schema
  - unknown tool-name execution path returns `ToolResultStatus::Failed`, passes `is_error=true` to Anthropic, and does not materialize failed terminal/error truth by itself
  - registered but unimplemented tool-name execution path returns `ToolResultStatus::Failed`, passes `is_error=true` to Anthropic, and does not materialize failed terminal/error truth by itself
  - writable tool checkpoint creation and rewind-safe manifest/ledger path
  - previewless writable-tool rejection path
  - tool-result re-entry passed to Anthropic as `tool_result`
  - runtime dispatch submit-user-input path invokes the live bridge and updates UI projection
- module black-box plan:
  - one selected anthropic provider drives text/reasoning/usage into turn truth through the bridge and closes via accepted completion schema
  - one selected anthropic provider writes provider raw response body or stream event bodies into `~/.freehand/ledgers/providers/anthropic`
  - one selected anthropic provider emits an implemented registered tool call, receives tool result, then closes via accepted completion schema
  - one selected anthropic provider emits a writable file tool call, gets checkpointed before execute, and can be rewound by runtime owner truth
  - one runtime dispatcher submit-user-input command drives an anthropic mock provider, materializes persistence, and exposes terminal projection through `UiProtocolState`
  - dispatcher failure recovery refreshes only the failed session transcript and does not clobber previously restored other-session transcripts in `UiProtocolState`
  - invalid completion schema retries exactly 3 consecutive terminal-candidate responses and closes blocked terminal without early success or failed status
  - missing completion schema polishing request includes the required tag guidance needed by the model to align the response to the contract
  - invalid completion schema polishing request includes the missing schema fields required by the model to align the response to the contract
  - non-string completion fields produce explicit type feedback instead of being reported as missing
  - non-string control-status fields produce explicit type feedback in the
    next provider request, then a corrected status can close the same logical
    turn successfully
  - non-terminal completion-schema rejection retries emit a UI waiting projection showing feedback was sent back to the model
  - recoverable non-stream provider HTTP/executor failure retries ten attempts with exponential backoff starting at 1 second before explicit dispatch failure, materializes failed terminal/error truth with a concrete provider error code, leaves no active turn hanging, and interrupts backoff immediately when the live cancel token is set
  - recoverable non-stream provider HTTP/executor failure can succeed after earlier attempts; metadata records `retry_same_step` attempts without terminal `fail_turn`
  - each actual retry and accepted fallback switch updates the same turn's model-request transport substate (`provider_retry` / `provider_failover`) before sleeping, retrying, or entering fallback; the next semantic response or terminal event clears that transient activity, and recovered turns contain no provider error event
  - OpenAI Responses HTTP 402 on the primary route immediately activates the configured Anthropic fallback; fallback success persists the fallback model and explicit route-switch metadata while retaining the primary error code, and the error-center recovery action is `failover_provider` rather than the contradictory `fail_turn`
  - retryable primary HTTP 500 exhausts ten attempts before activating the configured fallback
  - primary success does not call fallback, and adapter/callback/local invalid-config errors do not activate fallback
  - fallback retry exhaustion materializes exactly one failed closed turn with no active turn
  - tool execution result failure returns a paired failed tool result to the model, emits runtime-owned model-continuation waiting status, and can still end with a successful terminal schema
  - repaired failed-tool logical turn remains fully visible in persisted/UI truth while future prompt context admits only the final repaired round by default
  - long master-task prompt admission preserves semantic payload while still reporting planner token diagnostics
  - long multi-round carryover admission preserves semantic payload while still reporting planner token diagnostics
  - multi-round provider requests retain status schema, fresh TaskSpaceSnapshot,
    and task-tool guidance after tool-result continuation, `continue`, or
    schema-polishing re-entry
  - master-autonomy mock-provider scenarios prove model-output tool chains route through the live bridge and Task Center owner truth for success, execution-error blocked, and rejected-review retry outcomes
  - cross-workspace master fixture first attempts direct workspace access, receives
    boundary guidance, then calls `task` to create and assign worker work without
    any direct master-side filesystem execution
  - first provider request carries the master role, dispatch/no-dispatch boundary, workspace-boundary rule, multi-agent dispatch guidance, concurrency/flow-control guidance, task tool workflow, and the Codex-vs-Deepseek-reasonix cross-workspace sample without adding extra task/deep-research tools
  - structured task execution fact results are rendered from Task Center event semantics before re-entering provider context
  - provider raw ledger path poisoning returns explicit `RuntimeLiveBridgeError::ReasonPersistenceFailed`
  - reason-turn provider-output apply failure returns explicit dispatch failure when the reason owner rejects mutation
- project black-box impact:
  - CLI can reuse the runtime-owned bridge for a live-turn smoke path without importing provider DTOs into app code
  - CLI can prove real provider -> reason -> tool -> reason -> persistence from the app boundary
  - daemon can prove HTTP command ingress -> runtime live bridge -> provider -> reason -> persistence -> UI query/SSE projection
- fixtures / replay inputs / runtime evidence paths:
  - `crates/freehand-provider-anthropic/fixtures/minimonth_messages_single.json`
  - `crates/freehand-provider-anthropic/fixtures/minimonth_messages_stream.sse`
  - local mock transcript fixtures when added
  - `~/.freehand/ledgers/metadata`
  - `~/.freehand/ledgers/providers/anthropic`
  - `~/.freehand/ledgers/reason`
- known gaps:
  - streaming provider failover is intentionally unsupported until partial semantic output/tool-call rollback or resume is a typed contract; current production Master/Worker/lifecycle requests use `stream=false`
- sync status between design and implementation:
  - provider-selected live bridge owner is now `freehand-runtime`; provider-specific wire execution stays inside provider driver implementations backed by provider crates
  - runtime white-box coverage includes single-shot, SSE, invalid-schema retry, retry exhaustion, unsupported provider, registry-backed tool loop, persistence restore, runtime metadata producer wiring, and provider raw debug-ledger wiring
  - runtime white-box coverage now also proves runtime-owned debug snapshots for restore/request/tool/terminal lifecycle boundaries without prompt or tool-result leakage
  - runtime live bridge now injects tool owner schema fingerprint into reason planner diagnostics before provider request build
  - runtime dispatch and daemon black-box coverage are landed against local mock providers
  - runtime live bridge now writes restore/request/tool/terminal lifecycle metadata through `metadata.core` and fails explicitly on metadata write errors
  - runtime live bridge now writes provider raw response/error/event bodies through `reason.persistence` and fails explicitly on provider raw ledger write errors
  - runtime live bridge cancellation checkpoint coverage before tool execution, terminal persistence, and provider retry backoff is landed
  - runtime white-box coverage now explicitly locks failed tool-result multi-round continuation, including incomplete `tool_use` as paired failed tool-result truth with zero schema retries, proving execution failures become paired `ToolResultStatus::Failed` re-entry truth and provider/system errors remain explicit bridge failures
  - runtime white-box coverage now explicitly locks provider executor failure materialization: transport failure writes concrete provider error codes such as `anthropic_http_status_500` or `openai_http_status_500`, retries recoverable non-stream failures up to ten attempts, closes the active turn as failed only after exhaustion, and restores with no active turn
  - runtime white-box coverage locks configured primary/fallback routing: retryable primary provider exhaustion switches once to the configured fallback using the same provider-neutral round input, fallback success owns the persisted model/provider metadata, fallback exhaustion materializes one failed turn, and non-retryable adapter/callback/content failures do not switch providers
  - runtime white-box coverage locks OpenAI-compatible `responses` and `chat_completions` descriptor mapping through the same provider-neutral live bridge abstraction
  - runtime white-box coverage now explicitly locks context economy for repaired logical turns: superseded failed repair attempts do not leak into rebuilt future prompt context
  - runtime white-box coverage now locks long operator task admission through the live bridge: `original-task` budget scales with actual prompt content and the provider request preserves the prompt tail sentinel
  - runtime white-box coverage now locks `original-task` as a `TaskContract` segment and proves second-round requests still carry control status and runtime tool guidance
  - runtime white-box coverage now locks master-autonomy tool-loop outcomes with `live_bridge_master_autonomy_success_dispatches_worker_and_closes_task`, `live_bridge_master_autonomy_execution_error_blocks_without_success_close`, and `live_bridge_master_autonomy_rejected_review_retries_and_closes`
- mainline/wiki sync:
  - wiki generated from mainline call must stay in sync with runtime live bridge owner code and function map updates
