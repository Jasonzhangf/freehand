//! Shared mocks, fixtures, runtime harnesses, and replay helpers for Freehand tests.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use freehand_blocks::{
    CompletionDecision, CompletionSchemaRejection, RecoveryRewriteAction, RecoveryRewriteInput,
    RestoreStatus, RewritePolicyThresholds, completion_schema_guidance,
    completion_schema_rejection_feedback, parse_completion_submission_block,
    strip_completion_submission_block, validate_completion_submission,
};
use freehand_config::{
    ProviderProtocol as ConfigProviderProtocol, ProviderType, SelectedAgentConfig,
};
use freehand_contracts::{
    AgentId, ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
    ContextSegmentKind, ContextStability, FeatureId, ReasonReq04ToolCall,
    ReasonReq05ToolResultReentry, SessionId, TokenUsage, ToolResultContract, TraceId, TurnId,
};
use freehand_provider_anthropic::{
    AnthropicAdapterConfig, AnthropicExecutor, AnthropicExecutorConfig, AnthropicExecutorError,
};
use freehand_provider_core::ProviderSemanticOutput;
use freehand_provider_core::{
    ProviderCapabilities, ProviderDescriptor, ProviderFamily, ProviderProtocol, ProviderToolChoice,
    ProviderToolDefinition, ProviderToolExchange, build_semantic_request,
};
use freehand_reason::{
    CompactionPolicyOutcome, CompactionPolicyRequest, CompactionRewritePayload,
    ReasonBroadcastEvent, ReasonPersistence, ReasonPersistenceError, ReasonRewriteRuntime,
    ReasonTurnEngine, RecoveryPolicyOutcome, RecoveryPolicyRequest, ResumeRebuildPayload,
    RewriteRuntimeError, RewriteRuntimeState, SessionHistory, TurnRecord, TurnStartInput,
};
use serde_json::{Value, json};
use thiserror::Error;

pub struct ReasonRuntimeHarness {
    engine: ReasonTurnEngine,
    rewrite_runtime: ReasonRewriteRuntime,
    rewrite_state: RewriteRuntimeState,
    history: SessionHistory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveReasonTurnRequest {
    pub runtime_home: PathBuf,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub prompt: String,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveReasonTurnOutcome {
    pub turn: TurnRecord,
    pub turns: Vec<TurnRecord>,
    pub broadcasts: Vec<ReasonBroadcastEvent>,
    pub rounds: usize,
    pub schema_rejections: Vec<CompletionSchemaRejection>,
    pub tool_executions: usize,
    pub restore_status: LiveReasonRestoreStatus,
    pub restored_closed_turns: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveReasonRestoreStatus {
    CreatedNew,
    RestoredExisting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessTurnStart {
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub user_text: String,
    pub model: String,
    pub planned_context_segments: Vec<ContextSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageDrivenCompactionInput {
    pub context_window_tokens: Option<u32>,
    pub estimated_stale_reclaim_tokens: Option<u32>,
    pub compaction_payload: Option<CompactionRewritePayload>,
    pub thresholds: RewritePolicyThresholds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeRebuildHarnessInput {
    pub restore_status: RestoreStatus,
    pub resume_rebuild_payload: Option<ResumeRebuildPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTurnHarnessOutcome {
    pub turn: TurnRecord,
    pub latest_usage: Option<TokenUsage>,
    pub compaction_outcome: Option<CompactionPolicyOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonRuntimeSmokeScenario {
    UsageCompaction,
    RecoveryBlock,
}

impl ReasonRuntimeSmokeScenario {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UsageCompaction => "usage-compaction",
            Self::RecoveryBlock => "recovery-block",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "usage-compaction" => Some(Self::UsageCompaction),
            "recovery-block" => Some(Self::RecoveryBlock),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasonRuntimeSmokeReport {
    pub scenario: ReasonRuntimeSmokeScenario,
    pub rewrite_action: String,
    pub rewrite_version: u64,
    pub latest_usage_tokens: Option<u64>,
    pub blocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasonPersistenceSmokeReport {
    pub restored_terminal_summary: String,
    pub reason_seq: u64,
    pub ui_sidecar_exists: bool,
    pub session_index_entries: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReasonRuntimeHarnessError {
    #[error("turn start failed: {0}")]
    TurnStartFailed(String),
    #[error("rewrite runtime failed: {0}")]
    RewriteRuntimeFailed(String),
    #[error("live bridge provider `{provider}` with protocol `{protocol}` is not supported")]
    UnsupportedLiveProvider { provider: String, protocol: String },
    #[error("provider semantic request build failed: {0}")]
    ProviderRequestBuildFailed(String),
    #[error("anthropic live executor failed: {0}")]
    AnthropicExecutorFailed(String),
    #[error("reason persistence failed: {0}")]
    ReasonPersistenceFailed(String),
    #[error("live tool execution failed: {0}")]
    ToolExecutionFailed(String),
}

impl ReasonRuntimeHarness {
    pub fn new(
        session_id: SessionId,
        base_context_segments: Vec<ContextSegment>,
    ) -> Result<Self, ReasonRuntimeHarnessError> {
        let history = SessionHistory::new(session_id, base_context_segments)
            .map_err(|err| ReasonRuntimeHarnessError::RewriteRuntimeFailed(err.to_string()))?;
        Ok(Self {
            engine: ReasonTurnEngine::new(),
            rewrite_runtime: ReasonRewriteRuntime::new(),
            rewrite_state: RewriteRuntimeState::default(),
            history,
        })
    }

    pub fn run_provider_turn(
        &mut self,
        agent_id: AgentId,
        feature_id: FeatureId,
        start: HarnessTurnStart,
        provider_outputs: impl IntoIterator<Item = ProviderSemanticOutput>,
        compaction_input: Option<UsageDrivenCompactionInput>,
    ) -> Result<ProviderTurnHarnessOutcome, ReasonRuntimeHarnessError> {
        let session_id = self.history.session_id().clone();
        let mut turn = self
            .engine
            .start_turn(
                &mut self.history,
                TurnStartInput {
                    session_id,
                    turn_id: start.turn_id,
                    trace_id: start.trace_id,
                    feature_id,
                    agent_id,
                    user_text: start.user_text,
                    planned_context_segments: start.planned_context_segments,
                    model: start.model,
                },
            )
            .map_err(|err| ReasonRuntimeHarnessError::TurnStartFailed(err.to_string()))?;

        for output in provider_outputs {
            self.engine.apply_provider_output(&mut turn, output);
        }

        let latest_usage = turn.usage_events.last().map(|event| event.usage.clone());

        let compaction_outcome = match (latest_usage.clone(), compaction_input) {
            (Some(provider_usage), Some(input)) => Some(
                self.rewrite_runtime
                    .apply_compaction_policy(
                        &mut self.history,
                        &mut self.rewrite_state,
                        CompactionPolicyRequest {
                            context_window_tokens: input.context_window_tokens,
                            prompt_tokens: None,
                            provider_usage: Some(provider_usage),
                            estimated_stale_reclaim_tokens: input.estimated_stale_reclaim_tokens,
                            compaction_payload: input.compaction_payload,
                            thresholds: input.thresholds,
                        },
                    )
                    .map_err(map_rewrite_runtime_error)?,
            ),
            _ => None,
        };

        Ok(ProviderTurnHarnessOutcome {
            turn,
            latest_usage,
            compaction_outcome,
        })
    }

    pub fn apply_resume_rebuild(
        &mut self,
        input: ResumeRebuildHarnessInput,
    ) -> Result<RecoveryPolicyOutcome, ReasonRuntimeHarnessError> {
        self.rewrite_runtime
            .apply_recovery_policy(
                &mut self.history,
                RecoveryPolicyRequest {
                    decision_input: RecoveryRewriteInput {
                        restore_status: input.restore_status,
                        latest_rewrite_regression: None,
                        rollback_snapshot_available: false,
                        rebuild_source_available: input.resume_rebuild_payload.is_some(),
                    },
                    rollback_payload: None,
                    resume_rebuild_payload: input.resume_rebuild_payload,
                },
            )
            .map_err(map_rewrite_runtime_error)
    }

    pub fn history(&self) -> &SessionHistory {
        &self.history
    }

    pub fn rewrite_state(&self) -> &RewriteRuntimeState {
        &self.rewrite_state
    }
}

pub fn run_reason_runtime_smoke(
    agent_name: &str,
    scenario: ReasonRuntimeSmokeScenario,
) -> Result<ReasonRuntimeSmokeReport, ReasonRuntimeHarnessError> {
    let mut harness = ReasonRuntimeHarness::new(
        SessionId::new("session-smoke"),
        vec![stable_test_segment(
            "memory-smoke",
            ContextSegmentKind::SessionMemory,
            "remember the current smoke runtime state",
        )],
    )?;
    match scenario {
        ReasonRuntimeSmokeScenario::UsageCompaction => {
            let outcome = harness.run_provider_turn(
                AgentId::new(agent_name),
                FeatureId::new("app.cli-runtime-smoke"),
                HarnessTurnStart {
                    turn_id: TurnId::new("turn-smoke-1"),
                    trace_id: TraceId::new("trace-smoke-1"),
                    user_text: "continue".to_owned(),
                    model: "smoke-model".to_owned(),
                    planned_context_segments: Vec::new(),
                },
                [ProviderSemanticOutput::Usage(
                    freehand_contracts::ReasonResp02UsageEvent {
                        session_id: SessionId::new("session-smoke"),
                        turn_id: TurnId::new("turn-smoke-1"),
                        trace_id: TraceId::new("trace-smoke-1"),
                        feature_id: FeatureId::new("app.cli-runtime-smoke"),
                        agent_id: AgentId::new(agent_name),
                        usage: TokenUsage {
                            input_tokens: 80,
                            output_tokens: 4,
                            total_tokens: Some(84),
                            reasoning_tokens: None,
                            cache_creation_tokens: 10,
                            cache_read_tokens: 70,
                            finish_reason: Some("stop".to_owned()),
                        },
                    },
                )],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: Some(0),
                    compaction_payload: Some(CompactionRewritePayload {
                        rewritten_base_segments: vec![stable_test_segment(
                            "summary-smoke-1",
                            ContextSegmentKind::SessionSummary,
                            "compacted summary",
                        )],
                        rewrite_reason: "usage pressure compacted context".to_owned(),
                    }),
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )?;
            let decision = outcome.compaction_outcome.expect("compaction outcome");
            Ok(ReasonRuntimeSmokeReport {
                scenario,
                rewrite_action: format!("{:?}", decision.decision.action),
                rewrite_version: harness.history().rewrite_version(),
                latest_usage_tokens: outcome.latest_usage.map(|usage| usage.input_tokens),
                blocked: false,
            })
        }
        ReasonRuntimeSmokeScenario::RecoveryBlock => {
            let outcome = harness.apply_resume_rebuild(ResumeRebuildHarnessInput {
                restore_status: RestoreStatus::PersistedStateMissing,
                resume_rebuild_payload: None,
            })?;
            Ok(ReasonRuntimeSmokeReport {
                scenario,
                rewrite_action: format!("{:?}", outcome.decision.action),
                rewrite_version: harness.history().rewrite_version(),
                latest_usage_tokens: None,
                blocked: outcome.decision.action == RecoveryRewriteAction::Block,
            })
        }
    }
}

pub fn run_reason_persistence_smoke(
    agent_name: &str,
    runtime_home: impl AsRef<Path>,
) -> Result<ReasonPersistenceSmokeReport, ReasonRuntimeHarnessError> {
    let runtime_home = runtime_home.as_ref();
    let agent_id = AgentId::new(agent_name);
    let session_id = SessionId::new("session-persist-smoke");
    let turn_id = TurnId::new("turn-persist-smoke-1");
    let trace_id = TraceId::new("trace-persist-smoke-1");
    let feature_id = FeatureId::new("reason.persistence");
    let mut history = SessionHistory::new(
        session_id.clone(),
        vec![stable_test_segment(
            "memory-persist-smoke",
            ContextSegmentKind::SessionMemory,
            "remember persisted smoke state",
        )],
    )
    .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;
    let persistence = ReasonPersistence::new(runtime_home, agent_id.clone());
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: trace_id.clone(),
                feature_id: feature_id.clone(),
                agent_id: agent_id.clone(),
                user_text: "persist the latest terminal summary".to_owned(),
                planned_context_segments: Vec::new(),
                model: "smoke-model".to_owned(),
            },
        )
        .map_err(|err| ReasonRuntimeHarnessError::TurnStartFailed(err.to_string()))?;
    persistence
        .record_turn_started(&history, &turn, 0)
        .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;

    let output =
        ProviderSemanticOutput::SemanticEvent(freehand_contracts::ReasonResp01SemanticEvent {
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            trace_id: trace_id.clone(),
            feature_id: feature_id.clone(),
            agent_id: agent_id.clone(),
            kind: freehand_contracts::SemanticEventKind::Text,
            content: "persisted smoke output".to_owned(),
        });
    engine.apply_provider_output(&mut turn, output.clone());
    persistence
        .record_provider_output_applied(&history, &turn, &output, 0)
        .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;

    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id,
        feature_id,
        agent_id,
        status: freehand_contracts::TerminalStatus::Success,
        summary: "persisted smoke terminal".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;

    let restored = persistence
        .restore(&session_id)
        .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;
    let index_path = runtime_home
        .join("cache")
        .join("session-index")
        .join(format!("{agent_name}.json"));
    let index_entries: Vec<freehand_reason::PersistedSessionIndexEntry> =
        std::fs::read_to_string(&index_path)
            .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))
            .and_then(|payload| {
                serde_json::from_str(&payload).map_err(|err| {
                    ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string())
                })
            })?;

    Ok(ReasonPersistenceSmokeReport {
        restored_terminal_summary: restored.closed_turns[0]
            .terminal_event
            .as_ref()
            .expect("terminal")
            .summary
            .clone(),
        reason_seq: restored.cursor.last_applied_reason_seq,
        ui_sidecar_exists: runtime_home
            .join("state")
            .join("ui")
            .join(agent_name)
            .join("session-persist-smoke.json")
            .is_file(),
        session_index_entries: index_entries.len(),
    })
}

pub fn run_live_reason_turn(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
) -> Result<LiveReasonTurnOutcome, ReasonRuntimeHarnessError> {
    match (selected.provider.provider_type, selected.provider.protocol) {
        (ProviderType::Anthropic, ConfigProviderProtocol::Messages) => {
            run_live_anthropic_reason_turn(selected, request)
        }
        _ => Err(ReasonRuntimeHarnessError::UnsupportedLiveProvider {
            provider: selected.provider.provider_type.as_str().to_owned(),
            protocol: selected.provider.protocol.as_str().to_owned(),
        }),
    }
}

pub fn stable_test_segment(id: &str, kind: ContextSegmentKind, content: &str) -> ContextSegment {
    let (stability, cache_policy, role) = match kind {
        ContextSegmentKind::SystemAnchor => (
            ContextStability::Stable,
            ContextCachePolicy::CacheAnchor,
            ContextRole::System,
        ),
        ContextSegmentKind::DeveloperPolicy | ContextSegmentKind::CompletionContract => (
            ContextStability::Stable,
            ContextCachePolicy::CacheAnchor,
            ContextRole::Developer,
        ),
        ContextSegmentKind::SessionMemory | ContextSegmentKind::SessionSummary => (
            ContextStability::SessionStable,
            ContextCachePolicy::Cacheable,
            ContextRole::Developer,
        ),
        _ => panic!("stable_test_segment requires a stable/session-stable segment kind"),
    };
    ContextSegment {
        segment_id: ContextSegmentId::new(id),
        kind,
        stability,
        cache_policy,
        role,
        content: content.to_owned(),
        token_budget: 128,
        provenance: ContextProvenance {
            source: "freehand_testkit".to_owned(),
            reference: None,
        },
    }
}

fn map_rewrite_runtime_error(err: RewriteRuntimeError) -> ReasonRuntimeHarnessError {
    ReasonRuntimeHarnessError::RewriteRuntimeFailed(err.to_string())
}

fn run_live_anthropic_reason_turn(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
) -> Result<LiveReasonTurnOutcome, ReasonRuntimeHarnessError> {
    run_live_anthropic_reason_turn_with_hook(selected, request, |_| {})
}

fn run_live_anthropic_reason_turn_with_hook<F>(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
    mut on_broadcast: F,
) -> Result<LiveReasonTurnOutcome, ReasonRuntimeHarnessError>
where
    F: FnMut(&ReasonBroadcastEvent),
{
    let agent_id = AgentId::new(selected.name.clone());
    let persistence = ReasonPersistence::new(request.runtime_home.clone(), agent_id.clone());
    let (mut history, restore_status, restored_closed_turns) = match persistence
        .restore(&request.session_id)
    {
        Ok(restored) => {
            let count = restored.closed_turns.len();
            (
                restored.history,
                LiveReasonRestoreStatus::RestoredExisting,
                count,
            )
        }
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => (
            SessionHistory::new(request.session_id.clone(), Vec::new())
                .map_err(|err| ReasonRuntimeHarnessError::RewriteRuntimeFailed(err.to_string()))?,
            LiveReasonRestoreStatus::CreatedNew,
            0,
        ),
        Err(err) => {
            return Err(ReasonRuntimeHarnessError::ReasonPersistenceFailed(
                err.to_string(),
            ));
        }
    };
    let engine = ReasonTurnEngine::new();
    let receiver = engine.subscribe(64);
    let mut executor = AnthropicExecutor::new(AnthropicExecutorConfig {
        base_url: selected.provider.base_url.clone(),
        api_key: selected.provider.api_key.clone(),
        anthropic_version: "2023-06-01".to_owned(),
        adapter: AnthropicAdapterConfig { max_tokens: 512 },
    })
    .map_err(map_anthropic_executor_error)?;

    let mut broadcasts = Vec::new();
    let mut schema_rejections = Vec::new();
    let mut turns = Vec::new();
    let mut round = 0usize;
    let mut tool_executions = 0usize;
    let mut next_prompt = request.prompt.clone();
    let mut carryover_segments = vec![
        completion_contract_segment(),
        echo_json_tool_guidance_segment(),
        original_task_segment(&request.prompt),
    ];
    let mut tool_exchanges: Vec<ProviderToolExchange> = Vec::new();
    let mut executed_tool_call_ids = Vec::<String>::new();

    loop {
        round = round.saturating_add(1);
        let turn_id = derived_turn_id(&request.turn_id, round);
        let trace_id = derived_trace_id(&request.trace_id, round);
        let mut turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: request.session_id.clone(),
                    turn_id,
                    trace_id,
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: agent_id.clone(),
                    user_text: next_prompt.clone(),
                    planned_context_segments: carryover_segments.clone(),
                    model: selected.provider.default_model.clone(),
                },
            )
            .map_err(|err| ReasonRuntimeHarnessError::TurnStartFailed(err.to_string()))?;
        persistence
            .record_turn_started(&history, &turn, schema_rejections.len() as u32)
            .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;

        let mut semantic_request = build_semantic_request(
            provider_descriptor(selected),
            turn.provider_payload.clone(),
            false,
        )
        .map_err(|err| ReasonRuntimeHarnessError::ProviderRequestBuildFailed(err.to_string()))?;
        semantic_request.tools = vec![echo_json_tool_definition()];
        semantic_request.tool_choice = if tool_executions == 0 {
            Some(ProviderToolChoice::Required {
                name: "echo_json".to_owned(),
            })
        } else {
            None
        };
        semantic_request.tool_exchanges = tool_exchanges.clone();

        if request.stream {
            let mut stream_persistence_error = None::<ReasonRuntimeHarnessError>;
            executor
                .execute_stream_with(&provider_ctx(&turn), &semantic_request, |batch| {
                    let mut apply_ctx = LiveApplyContext {
                        engine: &engine,
                        persistence: &persistence,
                        history: &history,
                        receiver: &receiver,
                        broadcasts: &mut broadcasts,
                        on_broadcast: &mut on_broadcast,
                    };
                    if let Err(err) = apply_provider_outputs_persist_and_capture_broadcasts(
                        &mut apply_ctx,
                        &mut turn,
                        batch,
                        schema_rejections.len() as u32,
                    ) {
                        stream_persistence_error = Some(err);
                        return Err(AnthropicExecutorError::Callback(
                            "live bridge failed while persisting stream output".to_owned(),
                        ));
                    }
                    Ok(())
                })
                .map_err(map_anthropic_executor_error)?;
            if let Some(err) = stream_persistence_error {
                return Err(err);
            }
        } else {
            let outputs = executor
                .execute_once(&provider_ctx(&turn), &semantic_request)
                .map_err(map_anthropic_executor_error)?;
            let mut apply_ctx = LiveApplyContext {
                engine: &engine,
                persistence: &persistence,
                history: &history,
                receiver: &receiver,
                broadcasts: &mut broadcasts,
                on_broadcast: &mut on_broadcast,
            };
            apply_provider_outputs_persist_and_capture_broadcasts(
                &mut apply_ctx,
                &mut turn,
                &outputs,
                schema_rejections.len() as u32,
            )?;
        }
        drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);

        let completed_tool_calls = pending_completed_tool_calls(&turn, &executed_tool_call_ids);
        if !completed_tool_calls.is_empty() {
            for tool_call in completed_tool_calls {
                let tool_result = execute_echo_json_tool_call(&turn, &tool_call)?;
                let output = ProviderSemanticOutput::ToolResultReentry(tool_result.clone());
                engine.apply_provider_output(&mut turn, output.clone());
                persistence
                    .record_provider_output_applied(
                        &history,
                        &turn,
                        &output,
                        schema_rejections.len() as u32,
                    )
                    .map_err(|err| {
                        ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string())
                    })?;
                executed_tool_call_ids.push(tool_call.tool_call.tool_call_id.as_str().to_owned());
                tool_exchanges.push(ProviderToolExchange {
                    tool_call,
                    tool_result,
                });
                tool_executions = tool_executions.saturating_add(1);
            }
            next_prompt = "The tool result has been returned. Use it to continue the task, then provide the required Freehand completion schema when done.".to_owned();
            carryover_segments =
                next_round_segments(&request.prompt, &collect_turn_text(&turn), None);
            turns.push(turn);
            continue;
        }

        let provider_text = collect_turn_text(&turn);
        let visible_text = strip_completion_submission_block(&provider_text);
        match parse_completion_submission_block(&provider_text) {
            Ok(submission) => match validate_completion_submission(&submission)
                .expect("completion submission already validated")
            {
                CompletionDecision::Completed { .. } | CompletionDecision::Blocked { .. } => {
                    let _ = engine
                        .submit_completion(&mut turn, &submission)
                        .map_err(|err| {
                            ReasonRuntimeHarnessError::TurnStartFailed(err.to_string())
                        })?;
                    drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
                    persistence
                        .record_turn_closed(&history, &turn, schema_rejections.len() as u32)
                        .map_err(|err| {
                            ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string())
                        })?;
                    turns.push(turn.clone());
                    return Ok(LiveReasonTurnOutcome {
                        turn,
                        turns,
                        broadcasts,
                        rounds: round,
                        schema_rejections,
                        tool_executions,
                        restore_status,
                        restored_closed_turns,
                    });
                }
                CompletionDecision::ContinueWithNextStep { next_step } => {
                    next_prompt = next_step;
                    carryover_segments = next_round_segments(&request.prompt, &visible_text, None);
                    turns.push(turn);
                }
            },
            Err(rejection) => {
                let feedback = completion_schema_rejection_feedback(&rejection);
                schema_rejections.push(rejection.clone());
                persistence
                    .record_completion_rejected(
                        &history,
                        &turn,
                        &rejection,
                        schema_rejections.len() as u32,
                    )
                    .map_err(|err| {
                        ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string())
                    })?;
                if schema_rejections.len() >= 3 {
                    engine.fail_turn(
                        &mut turn,
                        format!(
                            "Failed after 3 invalid completion schema retries.\n{}",
                            feedback
                        ),
                    );
                    drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
                    persistence
                        .record_turn_closed(&history, &turn, schema_rejections.len() as u32)
                        .map_err(|err| {
                            ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string())
                        })?;
                    turns.push(turn.clone());
                    return Ok(LiveReasonTurnOutcome {
                        turn,
                        turns,
                        broadcasts,
                        rounds: round,
                        schema_rejections,
                        tool_executions,
                        restore_status,
                        restored_closed_turns,
                    });
                }
                next_prompt = feedback.clone();
                carryover_segments =
                    next_round_segments(&request.prompt, &visible_text, Some(feedback.as_str()));
                turns.push(turn);
            }
        }
    }
}

fn provider_ctx(turn: &TurnRecord) -> freehand_provider_core::ProviderEventContext {
    freehand_provider_core::ProviderEventContext {
        agent_id: turn.request.agent_id.clone(),
        session_id: turn.request.session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: turn.request.feature_id.clone(),
    }
}

fn map_anthropic_executor_error(err: AnthropicExecutorError) -> ReasonRuntimeHarnessError {
    ReasonRuntimeHarnessError::AnthropicExecutorFailed(err.to_string())
}

fn provider_descriptor(selected: &SelectedAgentConfig) -> ProviderDescriptor {
    ProviderDescriptor {
        provider_name: selected.provider.id.clone(),
        family: ProviderFamily::Anthropic,
        protocol: ProviderProtocol::AnthropicMessages,
        model: selected.provider.default_model.clone(),
        capabilities: ProviderCapabilities {
            web_search: false,
            multimodal: false,
            vision: false,
            reasoning: true,
        },
    }
}

fn derived_turn_id(base: &TurnId, round: usize) -> TurnId {
    if round == 1 {
        base.clone()
    } else {
        TurnId::new(format!("{}-r{round}", base.as_str()))
    }
}

fn derived_trace_id(base: &TraceId, round: usize) -> TraceId {
    if round == 1 {
        base.clone()
    } else {
        TraceId::new(format!("{}-r{round}", base.as_str()))
    }
}

fn completion_contract_segment() -> ContextSegment {
    ContextSegment {
        segment_id: ContextSegmentId::new("completion-contract"),
        kind: ContextSegmentKind::CompletionContract,
        stability: ContextStability::Stable,
        cache_policy: ContextCachePolicy::CacheAnchor,
        role: ContextRole::Developer,
        content: completion_schema_guidance().prompt,
        token_budget: 1024,
        provenance: ContextProvenance {
            source: "freehand_testkit".to_owned(),
            reference: Some("completion_schema_guidance".to_owned()),
        },
    }
}

fn echo_json_tool_guidance_segment() -> ContextSegment {
    ContextSegment {
        segment_id: ContextSegmentId::new("echo-json-tool-guidance"),
        kind: ContextSegmentKind::DeveloperPolicy,
        stability: ContextStability::Stable,
        cache_policy: ContextCachePolicy::CacheAnchor,
        role: ContextRole::Developer,
        content: "Use the `echo_json` tool exactly once before final completion when it is available. After receiving the tool result, continue from the returned JSON and then provide the required Freehand completion schema.".to_owned(),
        token_budget: 128,
        provenance: ContextProvenance {
            source: "freehand_testkit".to_owned(),
            reference: Some("echo_json_tool_guidance".to_owned()),
        },
    }
}

fn echo_json_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition {
        name: "echo_json".to_owned(),
        description:
            "Return the provided JSON object with deterministic Freehand E2E wrapper metadata."
                .to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Short message to echo."
                },
                "step": {
                    "type": "string",
                    "description": "Current reasoning step name."
                }
            },
            "additionalProperties": true
        }),
    }
}

fn original_task_segment(prompt: &str) -> ContextSegment {
    stable_test_segment(
        "original-task",
        ContextSegmentKind::SessionMemory,
        &format!("Original operator task:\n{prompt}"),
    )
}

fn next_round_segments(
    original_prompt: &str,
    visible_text: &str,
    rejection_feedback: Option<&str>,
) -> Vec<ContextSegment> {
    let mut segments = vec![
        completion_contract_segment(),
        original_task_segment(original_prompt),
    ];
    if !visible_text.trim().is_empty() {
        segments.push(ContextSegment {
            segment_id: ContextSegmentId::new("previous-visible-output"),
            kind: ContextSegmentKind::SubagentConclusion,
            stability: ContextStability::TurnVolatile,
            cache_policy: ContextCachePolicy::NoCache,
            role: ContextRole::Developer,
            content: format!("Previous round visible output:\n{visible_text}"),
            token_budget: 512,
            provenance: ContextProvenance {
                source: "live_reason_turn".to_owned(),
                reference: Some("previous_visible_output".to_owned()),
            },
        });
    }
    if let Some(feedback) = rejection_feedback {
        segments.push(ContextSegment {
            segment_id: ContextSegmentId::new("completion-schema-feedback"),
            kind: ContextSegmentKind::SubagentConclusion,
            stability: ContextStability::TurnVolatile,
            cache_policy: ContextCachePolicy::NoCache,
            role: ContextRole::Developer,
            content: format!("Completion schema rejection feedback:\n{feedback}"),
            token_budget: 1024,
            provenance: ContextProvenance {
                source: "live_reason_turn".to_owned(),
                reference: Some("completion_schema_feedback".to_owned()),
            },
        });
    }
    segments
}

fn collect_turn_text(turn: &TurnRecord) -> String {
    turn.semantic_events
        .iter()
        .filter_map(|event| {
            if event.kind == freehand_contracts::SemanticEventKind::Text {
                Some(event.content.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

fn pending_completed_tool_calls(
    turn: &TurnRecord,
    executed_tool_call_ids: &[String],
) -> Vec<ReasonReq04ToolCall> {
    turn.tool_calls
        .iter()
        .filter(|call| {
            call.tool_call.arguments_complete
                && !executed_tool_call_ids
                    .iter()
                    .any(|id| id == call.tool_call.tool_call_id.as_str())
        })
        .cloned()
        .collect()
}

fn execute_echo_json_tool_call(
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<ReasonReq05ToolResultReentry, ReasonRuntimeHarnessError> {
    if tool_call.tool_call.tool_name != "echo_json" {
        return Err(ReasonRuntimeHarnessError::ToolExecutionFailed(format!(
            "unsupported tool `{}`",
            tool_call.tool_call.tool_name
        )));
    }
    if !tool_call.tool_call.arguments_complete {
        return Err(ReasonRuntimeHarnessError::ToolExecutionFailed(
            "cannot execute incomplete tool arguments".to_owned(),
        ));
    }
    let mut input = serde_json::Map::new();
    for argument in &tool_call.tool_call.arguments {
        input.insert(argument.name.clone(), argument.value.clone());
    }
    let output = json!({
        "tool": "echo_json",
        "status": "ok",
        "input": Value::Object(input),
    })
    .to_string();
    Ok(ReasonReq05ToolResultReentry {
        session_id: turn.request.session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: turn.request.feature_id.clone(),
        agent_id: turn.request.agent_id.clone(),
        tool_result: ToolResultContract {
            tool_call_id: tool_call.tool_call.tool_call_id.clone(),
            output,
        },
    })
}

struct LiveApplyContext<'a, F>
where
    F: FnMut(&ReasonBroadcastEvent),
{
    engine: &'a ReasonTurnEngine,
    persistence: &'a ReasonPersistence,
    history: &'a SessionHistory,
    receiver: &'a Receiver<ReasonBroadcastEvent>,
    broadcasts: &'a mut Vec<ReasonBroadcastEvent>,
    on_broadcast: &'a mut F,
}

fn apply_provider_outputs_persist_and_capture_broadcasts<F>(
    ctx: &mut LiveApplyContext<'_, F>,
    turn: &mut TurnRecord,
    outputs: &[ProviderSemanticOutput],
    schema_rejections: u32,
) -> Result<(), ReasonRuntimeHarnessError>
where
    F: FnMut(&ReasonBroadcastEvent),
{
    for output in outputs {
        ctx.engine.apply_provider_output(turn, output.clone());
        ctx.persistence
            .record_provider_output_applied(ctx.history, turn, output, schema_rejections)
            .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;
    }
    drain_broadcasts(ctx.receiver, ctx.broadcasts, ctx.on_broadcast);
    Ok(())
}

fn drain_broadcasts<F>(
    receiver: &Receiver<ReasonBroadcastEvent>,
    broadcasts: &mut Vec<ReasonBroadcastEvent>,
    on_broadcast: &mut F,
) where
    F: FnMut(&ReasonBroadcastEvent),
{
    while let Ok(event) = receiver.try_recv() {
        on_broadcast(&event);
        broadcasts.push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_blocks::{CompactionTriggerAction, RecoveryRewriteAction};
    use freehand_config::{AgentMode, ProviderAuthType, ProviderType, SelectedProviderConfig};
    use freehand_contracts::{
        ReasonResp02UsageEvent, SemanticEventKind, TerminalStatus, ToolCallContract, ToolCallId,
    };
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_runtime_home() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("freehand-testkit-runtime-{stamp}-{counter}"))
    }

    fn harness() -> ReasonRuntimeHarness {
        ReasonRuntimeHarness::new(
            SessionId::new("session-1"),
            vec![stable_test_segment(
                "memory-1",
                ContextSegmentKind::SessionMemory,
                "remember workspace state",
            )],
        )
        .expect("harness")
    }

    fn turn_start(turn_id: &str) -> HarnessTurnStart {
        HarnessTurnStart {
            turn_id: TurnId::new(turn_id),
            trace_id: TraceId::new(format!("{turn_id}-trace")),
            user_text: "continue".to_owned(),
            model: "model-a".to_owned(),
            planned_context_segments: Vec::new(),
        }
    }

    fn usage_output(input_tokens: u64) -> ProviderSemanticOutput {
        ProviderSemanticOutput::Usage(ReasonResp02UsageEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("turn-1-trace"),
            feature_id: FeatureId::new("reason.rewrite-policy"),
            agent_id: AgentId::new("agent-1"),
            usage: TokenUsage {
                input_tokens,
                output_tokens: 4,
                total_tokens: Some(input_tokens + 4),
                reasoning_tokens: None,
                cache_creation_tokens: 10,
                cache_read_tokens: input_tokens.saturating_sub(10),
                finish_reason: Some("stop".to_owned()),
            },
        })
    }

    fn live_selected_agent(base_url: String, provider_type: ProviderType) -> SelectedAgentConfig {
        let protocol = match provider_type {
            ProviderType::Anthropic => ConfigProviderProtocol::Messages,
            ProviderType::OpenAi => ConfigProviderProtocol::ChatCompletions,
        };
        SelectedAgentConfig {
            name: "agent-live".to_owned(),
            mode: AgentMode::Master,
            allowed_pair_ip: None,
            pair_token_env: "FREEHAND_MASTER_TOKEN".to_owned(),
            pair_token: "pair-token".to_owned(),
            provider: SelectedProviderConfig {
                id: "provider-live".to_owned(),
                provider_type,
                protocol,
                base_url,
                default_model: "MiniMax-M2.7".to_owned(),
                auth_type: ProviderAuthType::ApiKey,
                api_key: "test-api-key".to_owned(),
            },
            restart_required_on_change: true,
        }
    }

    fn live_request(stream: bool) -> LiveReasonTurnRequest {
        LiveReasonTurnRequest {
            runtime_home: temp_runtime_home(),
            session_id: SessionId::new("session-live"),
            turn_id: TurnId::new("turn-live"),
            trace_id: TraceId::new("trace-live"),
            prompt: "reply exactly pong".to_owned(),
            stream,
        }
    }

    fn spawn_mock_server(
        status: u16,
        content_type: &'static str,
        response_body: String,
    ) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("timeout");
            let mut raw = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read");
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..read]);
                if request_is_complete(&raw) {
                    break;
                }
            }
            let request = String::from_utf8(raw).expect("utf8");
            tx.send(request).expect("send");
            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
                response_body.len()
            );
            stream.write_all(response.as_bytes()).expect("write");
        });
        (base_url, rx, handle)
    }

    fn spawn_incremental_stream_server(
        first_chunk: String,
        remaining_chunks: String,
    ) -> (
        String,
        mpsc::Receiver<String>,
        mpsc::Receiver<bool>,
        mpsc::Sender<()>,
        thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let (request_tx, request_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (continue_tx, continue_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("timeout");
            let mut raw = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read");
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..read]);
                if request_is_complete(&raw) {
                    break;
                }
            }
            request_tx
                .send(String::from_utf8(raw).expect("utf8"))
                .expect("send");
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
                )
                .expect("write headers");
            stream
                .write_all(first_chunk.as_bytes())
                .expect("write first chunk");
            stream.flush().expect("flush first chunk");

            let released = continue_rx.recv_timeout(Duration::from_secs(2)).is_ok();
            release_tx.send(released).expect("send release");
            if released {
                stream
                    .write_all(remaining_chunks.as_bytes())
                    .expect("write remaining chunks");
                stream.flush().expect("flush remaining chunks");
            }
        });
        (base_url, request_rx, release_rx, continue_tx, handle)
    }

    fn spawn_sequence_server(
        content_type: &'static str,
        response_bodies: Vec<String>,
    ) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            for response_body in response_bodies {
                let (mut stream, _) = listener.accept().expect("accept");
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("timeout");
                let mut raw = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let read = stream.read(&mut buffer).expect("read");
                    if read == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buffer[..read]);
                    if request_is_complete(&raw) {
                        break;
                    }
                }
                tx.send(String::from_utf8(raw).expect("utf8"))
                    .expect("send");
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
                    response_body.len()
                );
                stream.write_all(response.as_bytes()).expect("write");
            }
        });
        (base_url, rx, handle)
    }

    fn request_is_complete(raw: &[u8]) -> bool {
        let text = String::from_utf8_lossy(raw);
        let Some(header_end) = text.find("\r\n\r\n") else {
            return false;
        };
        let content_length = text[..header_end]
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length: ")
                    .or_else(|| line.strip_prefix("Content-Length: "))
            })
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        raw.len() >= header_end + 4 + content_length
    }

    fn tagged_completion_json(body: &str) -> String {
        format!("<freehand_completion>\n{body}\n</freehand_completion>")
    }

    fn complete_single_response(visible_text: &str) -> String {
        let tagged = tagged_completion_json(
            r#"{"claim":"complete","completion_reason":"done","evidence":"provider returned pong","summary":"pong","learned":"keep tagged completion strict"}"#,
        );
        format!(
            r#"{{"content":[{{"type":"text","text":"{visible}\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":82}},"stop_reason":"end_turn"}}"#,
            visible = visible_text,
            tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
        )
    }

    fn continue_single_response(next_step: &str) -> String {
        let tagged = tagged_completion_json(&format!(
            r#"{{"claim":"continue","next_step":"{next_step}"}}"#
        ));
        format!(
            r#"{{"content":[{{"type":"text","text":"working\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":40}},"stop_reason":"end_turn"}}"#,
            tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
        )
    }

    fn invalid_complete_response() -> String {
        let tagged = tagged_completion_json(r#"{"claim":"complete","summary":"pong"}"#);
        format!(
            r#"{{"content":[{{"type":"text","text":"draft\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":40}},"stop_reason":"end_turn"}}"#,
            tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
        )
    }

    fn tool_use_single_response() -> String {
        r#"{"content":[{"type":"tool_use","id":"toolu_echo_1","name":"echo_json","input":{"message":"pong","step":"tool-loop"}}],"usage":{"input_tokens":20,"output_tokens":16},"stop_reason":"tool_use"}"#.to_owned()
    }

    fn complete_stream_response(visible_text: &str) -> String {
        let tagged = tagged_completion_json(
            r#"{"claim":"complete","completion_reason":"done","evidence":"provider returned pong","summary":"pong","learned":"keep tagged completion strict"}"#,
        );
        format!(
            concat!(
                "event: content_block_start\n",
                "data: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"thinking\",\"thinking\":\"\"}}}}\n\n",
                "event: content_block_delta\n",
                "data: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"thinking_delta\",\"thinking\":\"thinking\"}}}}\n\n",
                "event: content_block_stop\n",
                "data: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n",
                "event: content_block_start\n",
                "data: {{\"type\":\"content_block_start\",\"index\":1,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n",
                "event: content_block_delta\n",
                "data: {{\"type\":\"content_block_delta\",\"index\":1,\"delta\":{{\"type\":\"text_delta\",\"text\":\"{text}\"}}}}\n\n",
                "event: content_block_stop\n",
                "data: {{\"type\":\"content_block_stop\",\"index\":1}}\n\n",
                "event: message_delta\n",
                "data: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"input_tokens\":14,\"output_tokens\":82}}}}\n\n",
                "event: message_stop\n",
                "data: {{\"type\":\"message_stop\"}}\n\n"
            ),
            text = format!("{visible_text}\\n{tagged}")
                .replace('\n', "\\n")
                .replace('"', "\\\"")
        )
    }

    #[test]
    fn provider_usage_drives_compaction_in_project_harness() {
        let mut harness = harness();
        let outcome = harness
            .run_provider_turn(
                AgentId::new("agent-1"),
                FeatureId::new("reason.rewrite-policy"),
                turn_start("turn-1"),
                [usage_output(80)],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: Some(0),
                    compaction_payload: Some(CompactionRewritePayload {
                        rewritten_base_segments: vec![stable_test_segment(
                            "summary-1",
                            ContextSegmentKind::SessionSummary,
                            "compacted summary",
                        )],
                        rewrite_reason: "usage pressure compacted context".to_owned(),
                    }),
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )
            .expect("run turn");

        assert_eq!(
            outcome
                .compaction_outcome
                .expect("compaction")
                .decision
                .action,
            CompactionTriggerAction::StageCompaction { force: false }
        );
        assert_eq!(harness.history().rewrite_version(), 1);
    }

    #[test]
    fn provider_usage_can_prefer_stale_prune_without_rewrite() {
        let mut harness = harness();
        let outcome = harness
            .run_provider_turn(
                AgentId::new("agent-1"),
                FeatureId::new("reason.rewrite-policy"),
                turn_start("turn-1"),
                [usage_output(81)],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: Some(5),
                    compaction_payload: None,
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )
            .expect("run turn");

        assert_eq!(
            outcome
                .compaction_outcome
                .expect("compaction")
                .decision
                .action,
            CompactionTriggerAction::PruneStaleOnly
        );
        assert_eq!(harness.history().rewrite_version(), 0);
    }

    #[test]
    fn resume_rebuild_blocks_when_source_is_missing() {
        let mut harness = harness();
        let outcome = harness
            .apply_resume_rebuild(ResumeRebuildHarnessInput {
                restore_status: RestoreStatus::PersistedStateMissing,
                resume_rebuild_payload: None,
            })
            .expect("block outcome");

        assert_eq!(outcome.decision.action, RecoveryRewriteAction::Block);
        assert_eq!(harness.history().rewrite_version(), 0);
    }

    #[test]
    fn provider_text_event_does_not_trigger_rewrite_without_usage() {
        let mut harness = harness();
        let outcome = harness
            .run_provider_turn(
                AgentId::new("agent-1"),
                FeatureId::new("reason.rewrite-policy"),
                turn_start("turn-1"),
                [ProviderSemanticOutput::SemanticEvent(
                    freehand_contracts::ReasonResp01SemanticEvent {
                        session_id: SessionId::new("session-1"),
                        turn_id: TurnId::new("turn-1"),
                        trace_id: TraceId::new("turn-1-trace"),
                        feature_id: FeatureId::new("reason.rewrite-policy"),
                        agent_id: AgentId::new("agent-1"),
                        kind: SemanticEventKind::Text,
                        content: "answer".to_owned(),
                    },
                )],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: None,
                    compaction_payload: None,
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )
            .expect("run turn");

        assert!(outcome.compaction_outcome.is_none());
        assert_eq!(harness.history().rewrite_version(), 0);
    }

    #[test]
    fn tool_call_without_usage_does_not_trigger_rewrite() {
        let mut harness = harness();
        let outcome = harness
            .run_provider_turn(
                AgentId::new("agent-1"),
                FeatureId::new("reason.rewrite-policy"),
                turn_start("turn-1"),
                [ProviderSemanticOutput::ToolCall(
                    freehand_contracts::ReasonReq04ToolCall {
                        session_id: SessionId::new("session-1"),
                        turn_id: TurnId::new("turn-1"),
                        trace_id: TraceId::new("turn-1-trace"),
                        feature_id: FeatureId::new("reason.rewrite-policy"),
                        agent_id: AgentId::new("agent-1"),
                        tool_call: ToolCallContract {
                            tool_call_id: ToolCallId::new("tool-1"),
                            tool_name: "search".to_owned(),
                            arguments: Vec::new(),
                            arguments_complete: true,
                        },
                    },
                )],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: None,
                    compaction_payload: None,
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )
            .expect("run turn");

        assert!(outcome.compaction_outcome.is_none());
        assert_eq!(harness.history().rewrite_version(), 0);
    }

    #[test]
    fn live_bridge_runs_single_shot_anthropic_provider_into_turn_truth() {
        let (base_url, rx, handle) =
            spawn_mock_server(200, "application/json", complete_single_response("pong"));

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, ProviderType::Anthropic),
            live_request(false),
        )
        .expect("live bridge");
        let raw_request = rx.recv().expect("request");
        handle.join().expect("join");

        assert!(raw_request.starts_with("POST /v1/messages HTTP/1.1"));
        assert!(raw_request.contains("x-api-key: test-api-key"));
        assert!(raw_request.contains("\"stream\":false"));
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|e| e.status.clone()),
            Some(TerminalStatus::Success)
        );
        assert!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .is_some_and(|event| event.summary.contains("Summary: pong"))
        );
        assert_eq!(
            strip_completion_submission_block(&collect_turn_text(&outcome.turn)),
            "pong"
        );
        assert!(
            outcome
                .broadcasts
                .iter()
                .any(|event| matches!(event, ReasonBroadcastEvent::Usage(_)))
        );
    }

    #[test]
    fn live_bridge_runs_streaming_anthropic_provider_into_broadcasts() {
        let (base_url, rx, handle) =
            spawn_mock_server(200, "text/event-stream", complete_stream_response("pong"));

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, ProviderType::Anthropic),
            live_request(true),
        )
        .expect("live bridge");
        let raw_request = rx.recv().expect("request");
        handle.join().expect("join");

        assert!(raw_request.contains("\"stream\":true"));
        assert_eq!(outcome.rounds, 1);
        let text = strip_completion_submission_block(&collect_turn_text(&outcome.turn));
        assert_eq!(text.trim(), "pong");
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|e| e.status.clone()),
            Some(TerminalStatus::Success)
        );
        assert!(outcome
            .broadcasts
            .iter()
            .any(|event| matches!(event, ReasonBroadcastEvent::Semantic(event) if event.kind == SemanticEventKind::Reasoning)));
    }

    #[test]
    fn live_bridge_applies_stream_outputs_before_provider_finishes() {
        let tagged = tagged_completion_json(
            r#"{"claim":"complete","completion_reason":"done","evidence":"provider returned pong","summary":"pong","learned":"keep tagged completion strict"}"#,
        );
        let first_chunk = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"pong\"}}\n\n"
        )
        .to_owned();
        let streamed_text = format!("pong\\n{tagged}")
            .replace('\n', "\\n")
            .replace('"', "\\\"");
        let remaining_chunks = format!(
            "event: content_block_start\n\
data: {{\"type\":\"content_block_start\",\"index\":1,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n\
event: content_block_delta\n\
data: {{\"type\":\"content_block_delta\",\"index\":1,\"delta\":{{\"type\":\"text_delta\",\"text\":\"{streamed_text}\"}}}}\n\n\
event: content_block_stop\n\
data: {{\"type\":\"content_block_stop\",\"index\":1}}\n\n\
event: message_delta\n\
data: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"input_tokens\":14,\"output_tokens\":82}}}}\n\n\
event: message_stop\n\
data: {{\"type\":\"message_stop\"}}\n\n"
        );
        let (base_url, rx, released_rx, continue_tx, handle) =
            spawn_incremental_stream_server(first_chunk, remaining_chunks);

        let mut seen_reasoning_before_release = false;
        let outcome = run_live_anthropic_reason_turn_with_hook(
            &live_selected_agent(base_url, ProviderType::Anthropic),
            live_request(true),
            |event| {
                if matches!(
                    event,
                    ReasonBroadcastEvent::Semantic(semantic)
                        if semantic.kind == SemanticEventKind::Reasoning
                ) {
                    seen_reasoning_before_release = true;
                    let _ = continue_tx.send(());
                }
            },
        )
        .expect("live bridge");
        let raw_request = rx.recv().expect("request");
        let released = released_rx.recv().expect("release");
        handle.join().expect("join");

        assert!(raw_request.contains("\"stream\":true"));
        assert!(
            released,
            "bridge did not apply reasoning output before stream end"
        );
        assert!(seen_reasoning_before_release);
        assert_eq!(
            strip_completion_submission_block(&collect_turn_text(&outcome.turn)),
            "pong"
        );
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|e| e.status.clone()),
            Some(TerminalStatus::Success)
        );
    }

    #[test]
    fn live_bridge_retries_invalid_schema_then_completes() {
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                invalid_complete_response(),
                complete_single_response("pong"),
            ],
        );

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, ProviderType::Anthropic),
            live_request(false),
        )
        .expect("live bridge");
        let first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        handle.join().expect("join");

        assert!(first_request.contains("reply exactly pong"));
        assert!(second_request.contains("Fix these schema entries"));
        assert_eq!(outcome.rounds, 2);
        assert_eq!(outcome.schema_rejections.len(), 1);
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );
    }

    #[test]
    fn live_bridge_uses_continue_next_step_for_next_round() {
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                continue_single_response("open the file and confirm pong"),
                complete_single_response("pong"),
            ],
        );

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, ProviderType::Anthropic),
            live_request(false),
        )
        .expect("live bridge");
        let _first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        handle.join().expect("join");

        assert!(second_request.contains("open the file and confirm pong"));
        assert_eq!(outcome.rounds, 2);
        assert!(outcome.schema_rejections.is_empty());
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );
    }

    #[test]
    fn live_bridge_executes_tool_reenters_result_and_persists_terminal_turn() {
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("tool done"),
            ],
        );
        let request = live_request(false);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, ProviderType::Anthropic),
            request,
        )
        .expect("live bridge");
        let first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        handle.join().expect("join");

        assert!(first_request.contains("\"tools\""));
        assert!(
            first_request.contains("\"tool_choice\":{\"name\":\"echo_json\",\"type\":\"tool\"}")
        );
        assert!(second_request.contains("\"type\":\"tool_result\""));
        assert!(second_request.contains("toolu_echo_1"));
        assert_eq!(outcome.rounds, 2);
        assert_eq!(outcome.tool_executions, 1);
        assert_eq!(outcome.restore_status, LiveReasonRestoreStatus::CreatedNew);
        assert!(
            outcome
                .turns
                .iter()
                .any(|turn| !turn.tool_results.is_empty())
        );
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );

        let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
            .restore(&session_id)
            .expect("restore persisted live session");
        assert_eq!(
            restored
                .closed_turns
                .last()
                .and_then(|turn| turn.terminal_event.as_ref())
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );
        assert!(restored.cursor.last_applied_reason_seq >= 4);
    }

    #[test]
    fn live_bridge_fails_after_three_invalid_schema_retries() {
        let (base_url, _rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                invalid_complete_response(),
                invalid_complete_response(),
                invalid_complete_response(),
            ],
        );

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, ProviderType::Anthropic),
            live_request(false),
        )
        .expect("live bridge");
        handle.join().expect("join");

        assert_eq!(outcome.rounds, 3);
        assert_eq!(outcome.schema_rejections.len(), 3);
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Failed)
        );
    }

    #[test]
    fn live_bridge_rejects_unsupported_provider_selection() {
        let err = run_live_reason_turn(
            &live_selected_agent("http://127.0.0.1:1".to_owned(), ProviderType::OpenAi),
            live_request(false),
        )
        .expect_err("must fail");

        assert!(matches!(
            err,
            ReasonRuntimeHarnessError::UnsupportedLiveProvider { provider, protocol }
                if provider == "openai" && protocol == "chat_completions"
        ));
    }

    #[test]
    fn reason_persistence_smoke_recovers_terminal_turn() {
        let runtime_home = temp_runtime_home();
        let report =
            run_reason_persistence_smoke("agent-1", &runtime_home).expect("persistence smoke");

        assert_eq!(report.restored_terminal_summary, "persisted smoke terminal");
        assert_eq!(report.reason_seq, 3);
        assert!(report.ui_sidecar_exists);
        assert_eq!(report.session_index_entries, 1);

        std::fs::remove_dir_all(runtime_home).expect("cleanup");
    }
}
