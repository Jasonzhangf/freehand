//! Reasoning turn orchestration and event emission for Freehand.

mod persistence;
mod rewrite_runtime;
mod session_history;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_blocks::{
    CompletionClaim, CompletionDecision, CompletionSchemaRejection, CompletionSubmission,
    CompletionValidationError, ContextPlannerInput, PlannedContext, SearchEvidenceSchemaRejection,
    SearchEvidenceValidationError, build_search_evidence_turn_delivery, plan_context,
    project_search_evidence_stage_status, validate_completion_submission,
};
use freehand_contracts::{
    AgentId, ContextProvenance, ContextSegment, ContextSegmentId, ErrorErr01RuntimeClassified,
    FeatureId, InputAttachmentMetadata, ReasonReq02ContextComposedInput,
    ReasonReq03ProviderPayload, ReasonReq04ToolCall, ReasonReq05ToolResultReentry,
    ReasonResp01SemanticEvent, ReasonResp02UsageEvent, ReasonResp03TerminalEvent,
    SearchEvidenceDelivery, SearchEvidenceTurnDelivery, SearchEvidenceTurnStatus,
    SearchFinalDelivery, SessionId, TerminalStatus, TraceId, TurnId, validate_reason_req02,
};
use freehand_debug::{
    DebugEvent, DebugHub, DebugScenePosition, DebugSemanticPosition, DebugStateSnapshot,
    DebugTraceEnvelope,
};
use freehand_metadata::{
    MetadataCenter, MetadataEntry, MetadataEnvelope, MetadataId, MetadataKind, MetadataSubject,
    MetadataWriteNode, MetadataWriteOwner,
};
use freehand_provider_core::ProviderSemanticOutput;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub use persistence::{
    ActiveTurnSnapshot, MAX_SESSION_LIST_PAGE_LIMIT, PersistedSessionIndexEntry,
    PersistedSessionMetadataEntry, PersistedSessionSummary, PersistedSessionSummaryIndex,
    PersistedSessionView, ProviderRawLedgerRow, ProviderRawLedgerWrite, ProviderRawScenePosition,
    ReasonLedgerPayload, ReasonLedgerRow, ReasonPersistence, ReasonPersistenceCursor,
    ReasonPersistenceError, ReasonSessionLatestStatus, ReasonSessionListCursor,
    ReasonSessionListPage, ReasonSessionListPageRequest, ReasonTurnPage, ReasonTurnPageDirection,
    ReasonTurnPageRequest, RestoredReasonSession, SessionRollbackMarker,
};
pub use rewrite_runtime::{
    CompactionPolicyOutcome, CompactionPolicyRequest, CompactionRewritePayload,
    ReasonRewriteRuntime, RecoveryPolicyOutcome, RecoveryPolicyRequest, ResumeRebuildPayload,
    RewriteRuntimeError, RewriteRuntimeState, RollbackRewritePayload,
};
pub use session_history::{
    RewriteDiagnosticsSnapshot, SessionHistory, SessionHistoryError, SessionRewriteRecord,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnStartInput {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub user_text: String,
    pub planned_context_segments: Vec<ContextSegment>,
    pub tool_schema_fingerprint: Option<String>,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasonBroadcastEvent {
    Semantic(ReasonResp01SemanticEvent),
    SearchEvidence(SearchEvidenceTurnDelivery),
    SearchEvidenceSchemaRejected(ReasonResp06SearchEvidenceSchemaRejected),
    Tool(ReasonReq04ToolCall),
    ToolResult(ReasonReq05ToolResultReentry),
    Usage(ReasonResp02UsageEvent),
    CompletionSchemaRejected(ReasonResp04CompletionSchemaRejected),
    ModelContinuationWaiting(ReasonResp05ModelContinuationWaiting),
    Terminal(ReasonResp03TerminalEvent),
    Error(ErrorErr01RuntimeClassified),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonResp04CompletionSchemaRejected {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub retry_index: u32,
    pub rejection: CompletionSchemaRejection,
    pub feedback: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonResp05ModelContinuationWaiting {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonResp06SearchEvidenceSchemaRejected {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub retry_index: u32,
    pub rejection: SearchEvidenceSchemaRejection,
    pub feedback: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnProjection {
    pub turn_id: TurnId,
    pub user_text: String,
    pub terminal_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    #[serde(default)]
    pub created_at: u64,
    #[serde(default, skip_serializing_if = "TurnTiming::is_empty")]
    pub timing: TurnTiming,
    pub request: ReasonReq02ContextComposedInput,
    pub provider_payload: ReasonReq03ProviderPayload,
    pub planned_context: PlannedContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<InputAttachmentMetadata>,
    pub semantic_events: Vec<ReasonResp01SemanticEvent>,
    pub tool_calls: Vec<ReasonReq04ToolCall>,
    pub tool_results: Vec<ReasonReq05ToolResultReentry>,
    pub usage_events: Vec<ReasonResp02UsageEvent>,
    pub terminal_event: Option<ReasonResp03TerminalEvent>,
    pub error_events: Vec<ErrorErr01RuntimeClassified>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_evidence: Option<SearchEvidenceTurnDelivery>,
}

fn append_search_evidence_delivery(
    turn: &mut TurnRecord,
    delivery: SearchEvidenceDelivery,
    status: freehand_contracts::SearchEvidenceTurnStatus,
) {
    match turn.search_evidence.as_mut() {
        Some(evidence) => {
            if let SearchEvidenceDelivery::Verification(verification) = &delivery
                && verification.access_status == freehand_contracts::SearchAccessStatus::Verified
            {
                evidence.verified_sources.push(verification.clone());
            }
            evidence.status = status;
            evidence.deliveries.push(delivery);
        }
        None => {
            match delivery {
                SearchEvidenceDelivery::DomainPlan(domain_plan) => {
                    turn.search_evidence = Some(SearchEvidenceTurnDelivery {
                        schema: "search_evidence.turn.v1".to_owned(),
                        session_id: turn.request.session_id.clone(),
                        turn_id: turn.request.turn_id.clone(),
                        domain_plan: Some(domain_plan.clone()),
                        deliveries: vec![SearchEvidenceDelivery::DomainPlan(domain_plan)],
                        verified_sources: Vec::new(),
                        unconfirmed: Vec::new(),
                        claims: Vec::new(),
                        status: freehand_contracts::SearchEvidenceTurnStatus::DomainPlanValidated,
                        summary_ready: false,
                        summary: None,
                        blocked_reason: None,
                        terminal: None,
                    });
                }
                SearchEvidenceDelivery::Discovery(discovery) => {
                    // Non-sourced hosted search: a provider-hosted discovery may be
                    // emitted without a domain plan (clean_search profile). Represent it
                    // as an observation-only turn delivery so the UI can project the
                    // tool activity without blocking completion on a sourced final.
                    turn.search_evidence = Some(SearchEvidenceTurnDelivery {
                        schema: "search_evidence.turn.v1".to_owned(),
                        session_id: turn.request.session_id.clone(),
                        turn_id: turn.request.turn_id.clone(),
                        domain_plan: None,
                        deliveries: vec![SearchEvidenceDelivery::Discovery(discovery)],
                        verified_sources: Vec::new(),
                        unconfirmed: Vec::new(),
                        claims: Vec::new(),
                        status,
                        summary_ready: false,
                        summary: None,
                        blocked_reason: None,
                        terminal: None,
                    });
                }
                // The stage validator guarantees a first delivery is either a domain
                // plan (sourced) or a non-sourced hosted discovery; verification,
                // supplement, and final cannot open the evidence stream.
                _ => unreachable!(
                    "stage validator requires first delivery to be a domain plan or hosted discovery"
                ),
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnTiming {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_response_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_to_first_response_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_elapsed_ms: Option<u64>,
}

impl TurnTiming {
    pub fn is_empty(&self) -> bool {
        self.turn_started_at_ms.is_none()
            && self.first_response_at_ms.is_none()
            && self.completed_at_ms.is_none()
            && self.time_to_first_response_ms.is_none()
            && self.total_elapsed_ms.is_none()
    }

    pub fn mark_first_response(&mut self, timestamp_ms: u64) {
        if self.first_response_at_ms.is_none() {
            self.first_response_at_ms = Some(timestamp_ms);
        }
        if self.time_to_first_response_ms.is_none()
            && let (Some(started_at), Some(first_at)) =
                (self.turn_started_at_ms, self.first_response_at_ms)
        {
            self.time_to_first_response_ms = first_at.checked_sub(started_at);
        }
    }

    pub fn mark_completed(&mut self, timestamp_ms: u64) {
        if self.completed_at_ms.is_none() {
            self.completed_at_ms = Some(timestamp_ms);
        }
        if self.total_elapsed_ms.is_none()
            && let (Some(started_at), Some(completed_at)) =
                (self.turn_started_at_ms, self.completed_at_ms)
        {
            self.total_elapsed_ms = completed_at.checked_sub(started_at);
        }
    }
}

pub struct ReasonTurnEngine {
    subscribers: Mutex<Vec<SyncSender<ReasonBroadcastEvent>>>,
    debug_hub: Option<Arc<DebugHub>>,
    metadata_center: Option<Arc<Mutex<MetadataCenter>>>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReasonTurnError {
    #[error("turn input text must not be empty")]
    EmptyUserText,
    #[error("session history does not match turn session `{0}`")]
    SessionMismatch(String),
    #[error("context planning failed: {0}")]
    ContextPlanningFailed(String),
    #[error("completion rejected: {0}")]
    CompletionRejected(String),
    #[error("completion requires next step: {0}")]
    CompletionRequiresNextStep(String),
    #[error("metadata write failed: {0}")]
    MetadataWriteFailed(String),
    #[error("search evidence rejected: {0}")]
    SearchEvidenceRejected(String),
}

impl Default for ReasonTurnEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ReasonTurnEngine {
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
            debug_hub: None,
            metadata_center: None,
        }
    }

    pub fn with_debug_hub(debug_hub: Arc<DebugHub>) -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
            debug_hub: Some(debug_hub),
            metadata_center: None,
        }
    }

    pub fn with_metadata_center(metadata_center: Arc<Mutex<MetadataCenter>>) -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
            debug_hub: None,
            metadata_center: Some(metadata_center),
        }
    }

    pub fn with_debug_hub_and_metadata_center(
        debug_hub: Arc<DebugHub>,
        metadata_center: Arc<Mutex<MetadataCenter>>,
    ) -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
            debug_hub: Some(debug_hub),
            metadata_center: Some(metadata_center),
        }
    }

    pub fn subscribe(&self, capacity: usize) -> Receiver<ReasonBroadcastEvent> {
        let (sender, receiver) = mpsc::sync_channel(capacity.max(1));
        self.subscribers
            .lock()
            .expect("lock subscribers")
            .push(sender);
        receiver
    }

    pub fn start_turn(
        &self,
        history: &mut SessionHistory,
        input: TurnStartInput,
    ) -> Result<TurnRecord, ReasonTurnError> {
        if input.user_text.trim().is_empty() {
            return Err(ReasonTurnError::EmptyUserText);
        }
        if history.session_id() != &input.session_id {
            return Err(ReasonTurnError::SessionMismatch(
                input.session_id.as_str().to_owned(),
            ));
        }
        let mut candidate_segments = history.base_context_segments().to_vec();
        candidate_segments.extend(input.planned_context_segments);
        let planned_context = plan_context(ContextPlannerInput {
            candidate_segments,
            current_user_text: input.user_text.clone(),
            user_segment_id: ContextSegmentId::new(format!("{}-user", input.turn_id.as_str())),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: history.current_rewrite_mode(),
            rewrite_version: history.rewrite_version(),
            tool_schema_fingerprint: input.tool_schema_fingerprint.clone(),
        })
        .map_err(|err| ReasonTurnError::ContextPlanningFailed(err.to_string()))?;
        let request = ReasonReq02ContextComposedInput {
            session_id: input.session_id.clone(),
            turn_id: input.turn_id.clone(),
            trace_id: input.trace_id.clone(),
            feature_id: input.feature_id.clone(),
            agent_id: input.agent_id.clone(),
            user_text: input.user_text.clone(),
            context_segments: planned_context.ordered_segments.clone(),
        };
        validate_reason_req02(&request).map_err(|_| ReasonTurnError::EmptyUserText)?;
        let provider_payload = ReasonReq03ProviderPayload {
            session_id: input.session_id,
            turn_id: input.turn_id,
            trace_id: input.trace_id,
            feature_id: input.feature_id,
            agent_id: input.agent_id,
            model: input.model,
            input_segments: planned_context.ordered_segments.clone(),
        };
        let started_at_ms = unix_millis_now();
        let turn = TurnRecord {
            created_at: started_at_ms / 1000,
            timing: TurnTiming {
                turn_started_at_ms: Some(started_at_ms),
                ..TurnTiming::default()
            },
            request,
            provider_payload,
            planned_context,
            cwd: None,
            attachments: Vec::new(),
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            search_evidence: None,
        };
        self.write_metadata(
            &turn,
            MetadataKind::RuntimeState,
            "ReasonReq02ContextComposedInput",
            "start_turn",
            "ReasonTurnEngine::start_turn",
            vec![
                MetadataEntry {
                    key: "reason.model".to_owned(),
                    value: json!(turn.provider_payload.model),
                },
                MetadataEntry {
                    key: "context.rewrite_mode".to_owned(),
                    value: json!(format!(
                        "{:?}",
                        turn.planned_context.diagnostics.rewrite_mode
                    )),
                },
                MetadataEntry {
                    key: "context.rewrite_version".to_owned(),
                    value: json!(turn.planned_context.diagnostics.rewrite_version),
                },
                MetadataEntry {
                    key: "context.segment_count".to_owned(),
                    value: json!(turn.planned_context.ordered_segments.len()),
                },
            ],
        )?;
        history.commit_turn_start(&turn.request.turn_id);
        self.emit_debug(
            &turn,
            "ReasonTurnEngine::start_turn",
            "reason turn started",
            vec![
                format!("model={}", turn.provider_payload.model),
                format!(
                    "rewrite_mode={:?}",
                    turn.planned_context.diagnostics.rewrite_mode
                ),
                format!(
                    "rewrite_version={}",
                    turn.planned_context.diagnostics.rewrite_version
                ),
            ],
        );
        Ok(turn)
    }

    pub fn apply_provider_output(
        &self,
        turn: &mut TurnRecord,
        output: ProviderSemanticOutput,
    ) -> Result<(), ReasonTurnError> {
        self.write_provider_output_metadata(turn, &output)?;
        let output_started = !matches!(output, ProviderSemanticOutput::ToolResultReentry(_));
        if output_started {
            turn.timing.mark_first_response(unix_millis_now());
        }
        match output {
            ProviderSemanticOutput::SemanticEvent(event) => {
                turn.semantic_events.push(event.clone());
                self.publish(ReasonBroadcastEvent::Semantic(event));
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::apply_provider_output",
                    "provider semantic event applied",
                    vec![format!(
                        "kind={:?}",
                        turn.semantic_events.last().map(|it| &it.kind)
                    )],
                );
            }
            ProviderSemanticOutput::SearchDiscovery(delivery) => {
                let evidence = SearchEvidenceDelivery::Discovery(delivery);
                self.apply_search_evidence_stage_delivery(turn, evidence)?;
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::apply_provider_output",
                    "hosted search discovery applied",
                    vec!["search_discovery=true".to_owned()],
                );
            }
            ProviderSemanticOutput::ToolCall(event) => {
                turn.tool_calls.push(event.clone());
                self.publish(ReasonBroadcastEvent::Tool(event));
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::apply_provider_output",
                    "provider tool call applied",
                    vec![format!(
                        "tool_name={}",
                        turn.tool_calls
                            .last()
                            .map(|it| it.tool_call.tool_name.as_str())
                            .unwrap_or("")
                    )],
                );
            }
            ProviderSemanticOutput::ToolResultReentry(result) => {
                if let Some(delivery) = result.tool_result.search_evidence.clone() {
                    self.apply_search_evidence_stage_delivery(turn, delivery)?;
                }
                turn.tool_results.push(result.clone());
                self.publish(ReasonBroadcastEvent::ToolResult(result));
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::apply_provider_output",
                    "tool result re-entry applied",
                    vec![format!("tool_results={}", turn.tool_results.len())],
                );
            }
            ProviderSemanticOutput::Usage(event) => {
                turn.usage_events.push(event.clone());
                self.publish(ReasonBroadcastEvent::Usage(event));
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::apply_provider_output",
                    "provider usage applied",
                    vec![format!("usage_events={}", turn.usage_events.len())],
                );
            }
            ProviderSemanticOutput::Terminal(event) => {
                if event.status != TerminalStatus::ToolPending {
                    // Only apply raw provider terminal if the turn has not already received
                    // a completion-schema-driven terminal.  Completion schema is the
                    // authoritative semantic summary; the raw stop_reason (e.g. "end_turn")
                    // is not meaningful presentation text and must not shadow a
                    // harness-parsed completion terminal.
                    if turn.terminal_event.is_none() {
                        turn.terminal_event = Some(event.clone());
                        turn.timing.mark_completed(unix_millis_now());
                        self.publish(ReasonBroadcastEvent::Terminal(event.clone()));
                        self.emit_debug(
                            turn,
                            "ReasonTurnEngine::apply_provider_output",
                            "provider terminal applied",
                            vec![format!("terminal_status={:?}", event.status)],
                        );
                    } else {
                        self.emit_debug(
                            turn,
                            "ReasonTurnEngine::apply_provider_output",
                            "provider terminal skipped (completion terminal already set)",
                            vec![format!(
                                "terminal_status={:?} kept_summary={:?}",
                                event.status,
                                turn.terminal_event.as_ref().map(|e| &e.summary)
                            )],
                        );
                    }
                } else {
                    self.emit_debug(
                        turn,
                        "ReasonTurnEngine::apply_provider_output",
                        "provider tool-pending observed",
                        vec!["terminal_waits_for_tool_round=true".to_owned()],
                    );
                }
            }
            ProviderSemanticOutput::Error(event) => {
                turn.error_events.push(event.clone());
                self.publish(ReasonBroadcastEvent::Error(event));
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::apply_provider_output",
                    "provider error applied",
                    vec![format!("error_events={}", turn.error_events.len())],
                );
            }
        }
        Ok(())
    }

    pub fn discard_provider_terminal(&self, turn: &mut TurnRecord, summary: impl Into<String>) {
        turn.terminal_event = None;
        turn.timing.completed_at_ms = None;
        turn.timing.total_elapsed_ms = None;
        self.emit_debug(
            turn,
            "ReasonTurnEngine::discard_provider_terminal",
            "provider terminal invalidated before persistence",
            vec![summary.into()],
        );
    }

    pub fn submit_completion(
        &self,
        turn: &mut TurnRecord,
        submission: &CompletionSubmission,
    ) -> Result<ReasonResp03TerminalEvent, ReasonTurnError> {
        // Non-sourced hosted search (no domain plan) is observation-only and does not
        // gate completion on a sourced final delivery; only sourced evidence applies
        // the claim/summary final gate.
        if let Some(search_evidence) = turn.search_evidence.as_ref()
            && search_evidence.domain_plan.is_some()
        {
            let expected = match search_evidence.terminal {
                None if search_evidence.status
                    == freehand_contracts::SearchEvidenceTurnStatus::FinalValidated =>
                {
                    Some((
                        CompletionClaim::Complete,
                        search_evidence.summary.as_deref(),
                    ))
                }
                Some(freehand_contracts::SearchEvidenceTerminal::Blocked) => Some((
                    CompletionClaim::Blocked,
                    search_evidence.blocked_reason.as_deref(),
                )),
                _ => None,
            };
            let Some((expected_claim, expected_text)) = expected else {
                return Err(ReasonTurnError::SearchEvidenceRejected(
                    "search completion requires a validated SearchFinalDelivery".to_owned(),
                ));
            };
            if submission.claim != expected_claim {
                return Err(ReasonTurnError::SearchEvidenceRejected(
                    "completion claim must match validated search final claim".to_owned(),
                ));
            }
            let submitted_text = match submission.claim {
                CompletionClaim::Complete => submission.summary.as_deref(),
                CompletionClaim::Blocked => submission.blocked_reason.as_deref(),
                _ => None,
            };
            if submitted_text != expected_text {
                return Err(ReasonTurnError::SearchEvidenceRejected(
                    "completion text must match validated search final delivery".to_owned(),
                ));
            }
        }
        match validate_completion_submission(submission) {
            Ok(CompletionDecision::Completed {
                status,
                terminal_text,
            })
            | Ok(CompletionDecision::Blocked {
                status,
                terminal_text,
            }) => {
                let user_options: Option<Vec<String>> = None;
                self.handle_completion_close(turn, status, terminal_text, user_options)
            }
            Ok(CompletionDecision::Waiting {
                status,
                terminal_text,
                user_options,
            }) => self.handle_completion_close(turn, status, terminal_text, user_options),
            Ok(CompletionDecision::ContinueWithNextStep { next_step }) => {
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::submit_completion",
                    "completion requested continuation",
                    vec![format!("next_step={next_step}")],
                );
                Err(ReasonTurnError::CompletionRequiresNextStep(next_step))
            }
            Err(err) => {
                let message = completion_error_message(err);
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::submit_completion",
                    "completion rejected",
                    vec![message.clone()],
                );
                Err(ReasonTurnError::CompletionRejected(message))
            }
        }
    }

    pub fn apply_search_evidence_delivery<'a>(
        &self,
        turn: &'a mut TurnRecord,
        final_delivery: SearchFinalDelivery,
    ) -> Result<&'a SearchEvidenceTurnDelivery, ReasonTurnError> {
        let deliveries = turn
            .search_evidence
            .as_ref()
            .map(|evidence| evidence.deliveries.clone())
            .unwrap_or_default();
        let search_evidence = build_search_evidence_turn_delivery(
            turn.request.session_id.clone(),
            turn.request.turn_id.clone(),
            deliveries,
            final_delivery,
        )
        .map_err(|error: SearchEvidenceValidationError| {
            ReasonTurnError::SearchEvidenceRejected(error.to_string())
        })?;
        turn.search_evidence = Some(search_evidence);
        self.publish(ReasonBroadcastEvent::SearchEvidence(
            turn.search_evidence
                .as_ref()
                .expect("search evidence was just assigned")
                .clone(),
        ));
        Ok(turn
            .search_evidence
            .as_ref()
            .expect("search evidence was just assigned"))
    }

    pub fn apply_search_evidence_stage_delivery<'a>(
        &self,
        turn: &'a mut TurnRecord,
        delivery: SearchEvidenceDelivery,
    ) -> Result<&'a SearchEvidenceTurnDelivery, ReasonTurnError> {
        if matches!(delivery, SearchEvidenceDelivery::Final(_)) {
            return Err(ReasonTurnError::SearchEvidenceRejected(
                "final delivery must enter the final-delivery owner gate".to_owned(),
            ));
        }
        let existing = turn
            .search_evidence
            .as_ref()
            .map(|evidence| evidence.deliveries.as_slice())
            .unwrap_or(&[]);
        let status = project_search_evidence_stage_status(existing, &delivery)
            .map_err(|error| ReasonTurnError::SearchEvidenceRejected(error.to_string()))?;
        append_search_evidence_delivery(turn, delivery, status);
        self.publish(ReasonBroadcastEvent::SearchEvidence(
            turn.search_evidence
                .as_ref()
                .expect("validated stage delivery created search evidence truth")
                .clone(),
        ));
        Ok(turn
            .search_evidence
            .as_ref()
            .expect("validated stage delivery created search evidence truth"))
    }

    pub fn carry_search_evidence(
        &self,
        previous: &TurnRecord,
        next: &mut TurnRecord,
    ) -> Result<(), ReasonTurnError> {
        let Some(evidence) = previous.search_evidence.as_ref() else {
            return Ok(());
        };
        if evidence.status == SearchEvidenceTurnStatus::TurnTerminalSuccess {
            return Ok(());
        }
        next.search_evidence = Some(evidence.clone());
        Ok(())
    }

    fn handle_completion_close(
        &self,
        turn: &mut TurnRecord,
        status: TerminalStatus,
        terminal_text: String,
        user_options: Option<Vec<String>>,
    ) -> Result<ReasonResp03TerminalEvent, ReasonTurnError> {
        let event = ReasonResp03TerminalEvent {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            status,
            summary: terminal_text,
            user_options,
        };
        if let Some(search_evidence) = turn.search_evidence.as_mut()
            && search_evidence.status
                == freehand_contracts::SearchEvidenceTurnStatus::FinalValidated
            && event.status == TerminalStatus::Success
        {
            search_evidence.status =
                freehand_contracts::SearchEvidenceTurnStatus::TurnTerminalSuccess;
            search_evidence.terminal = Some(freehand_contracts::SearchEvidenceTerminal::Success);
        }
        if let Some(search_evidence) = turn.search_evidence.as_ref() {
            self.publish(ReasonBroadcastEvent::SearchEvidence(
                search_evidence.clone(),
            ));
        }
        turn.terminal_event = Some(event.clone());
        turn.timing.mark_completed(unix_millis_now());
        self.publish(ReasonBroadcastEvent::Terminal(event.clone()));
        self.emit_debug(
            turn,
            "ReasonTurnEngine::submit_completion",
            "completion accepted",
            vec![format!("terminal_status={:?}", event.status)],
        );
        Ok(event)
    }

    pub fn fail_turn(
        &self,
        turn: &mut TurnRecord,
        summary: impl Into<String>,
    ) -> ReasonResp03TerminalEvent {
        let event = ReasonResp03TerminalEvent {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            status: TerminalStatus::Failed,
            summary: summary.into(),
            user_options: None,
        };
        turn.terminal_event = Some(event.clone());
        turn.timing.mark_completed(unix_millis_now());
        self.publish(ReasonBroadcastEvent::Terminal(event.clone()));
        self.emit_debug(
            turn,
            "ReasonTurnEngine::fail_turn",
            "turn failed",
            vec![event.summary.clone()],
        );
        event
    }

    pub fn interrupt_turn(
        &self,
        turn: &mut TurnRecord,
        summary: impl Into<String>,
    ) -> ReasonResp03TerminalEvent {
        self.close_turn_with_status(turn, TerminalStatus::Interrupted, summary)
    }

    pub fn block_turn(
        &self,
        turn: &mut TurnRecord,
        summary: impl Into<String>,
    ) -> ReasonResp03TerminalEvent {
        self.close_turn_with_status(turn, TerminalStatus::Blocked, summary)
    }

    fn close_turn_with_status(
        &self,
        turn: &mut TurnRecord,
        status: TerminalStatus,
        summary: impl Into<String>,
    ) -> ReasonResp03TerminalEvent {
        let event = ReasonResp03TerminalEvent {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            status,
            summary: summary.into(),
            user_options: None,
        };
        turn.terminal_event = Some(event.clone());
        turn.timing.mark_completed(unix_millis_now());
        self.publish(ReasonBroadcastEvent::Terminal(event.clone()));
        self.emit_debug(
            turn,
            "ReasonTurnEngine::close_turn_with_status",
            format!("turn closed as {:?}", event.status),
            vec![event.summary.clone()],
        );
        event
    }

    pub fn cancel_turn(
        &self,
        turn: &mut TurnRecord,
        summary: impl Into<String>,
    ) -> ReasonResp03TerminalEvent {
        if turn.terminal_event.is_some() {
            return turn
                .terminal_event
                .clone()
                .expect("terminal event already set");
        }
        let event = ReasonResp03TerminalEvent {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            status: TerminalStatus::Cancelled,
            summary: summary.into(),
            user_options: None,
        };
        turn.terminal_event = Some(event.clone());
        turn.timing.mark_completed(unix_millis_now());
        self.publish(ReasonBroadcastEvent::Terminal(event.clone()));
        self.emit_debug(
            turn,
            "ReasonTurnEngine::cancel_turn",
            "turn cancelled",
            vec![event.summary.clone()],
        );
        event
    }

    pub fn project_session(&self, turns: &[TurnRecord]) -> Vec<TurnProjection> {
        turns
            .iter()
            .map(|turn| TurnProjection {
                turn_id: turn.request.turn_id.clone(),
                user_text: turn.request.user_text.clone(),
                terminal_summary: turn
                    .terminal_event
                    .as_ref()
                    .map(|event| event.summary.clone()),
            })
            .collect()
    }

    fn publish(&self, event: ReasonBroadcastEvent) {
        let mut subscribers = self.subscribers.lock().expect("lock subscribers");
        subscribers.retain(|sender| match sender.try_send(event.clone()) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }

    fn emit_debug(
        &self,
        turn: &TurnRecord,
        function: &str,
        status_text: impl Into<String>,
        detail_lines: Vec<String>,
    ) {
        let Some(hub) = &self.debug_hub else {
            return;
        };
        let snapshot = DebugStateSnapshot::new(
            DebugSemanticPosition {
                feature_id: turn.request.feature_id.clone(),
                session_id: turn.request.session_id.clone(),
                turn_id: turn.request.turn_id.clone(),
                trace_id: turn.request.trace_id.clone(),
                agent_id: Some(turn.request.agent_id.clone()),
                pipeline_node: Some("reason.turn".to_owned()),
            },
            DebugScenePosition {
                crate_name: "freehand-reason".to_owned(),
                file: "src/lib.rs".to_owned(),
                function: function.to_owned(),
                line: None,
                artifact_path: None,
                raw_exchange_id: None,
            },
            status_text,
            detail_lines,
        );
        let event = DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic: snapshot.semantic.clone(),
                scene: snapshot.scene.clone(),
                input_hash: None,
                output_hash: None,
                artifact_path: snapshot.scene.artifact_path.clone(),
                timestamp: unix_timestamp_string(),
            },
            snapshot: Some(snapshot),
        };
        let _ = hub.emit(event);
    }

    fn write_provider_output_metadata(
        &self,
        turn: &TurnRecord,
        output: &ProviderSemanticOutput,
    ) -> Result<(), ReasonTurnError> {
        let (kind, pipeline_node, output_kind, extra_entries) = match output {
            ProviderSemanticOutput::SemanticEvent(event) => (
                MetadataKind::RuntimeState,
                "ReasonResp01SemanticEvent",
                "semantic_event",
                vec![MetadataEntry {
                    key: "provider_output.semantic_kind".to_owned(),
                    value: json!(format!("{:?}", event.kind)),
                }],
            ),
            ProviderSemanticOutput::SearchDiscovery(delivery) => (
                MetadataKind::Provider,
                "SearchDiscoveryDelivery",
                "search_discovery",
                vec![
                    MetadataEntry {
                        key: "search.discovery_channel".to_owned(),
                        value: json!(format!("{:?}", delivery.discovery_channel)),
                    },
                    MetadataEntry {
                        key: "search.candidate_count".to_owned(),
                        value: json!(delivery.candidates.len()),
                    },
                ],
            ),
            ProviderSemanticOutput::ToolCall(event) => (
                MetadataKind::Routing,
                "ReasonReq04ToolCall",
                "tool_call",
                vec![MetadataEntry {
                    key: "tool.name".to_owned(),
                    value: json!(event.tool_call.tool_name),
                }],
            ),
            ProviderSemanticOutput::ToolResultReentry(event) => (
                MetadataKind::Routing,
                "ReasonReq05ToolResultReentry",
                "tool_result_reentry",
                vec![MetadataEntry {
                    key: "tool.call_id".to_owned(),
                    value: json!(event.tool_result.tool_call_id.as_str()),
                }],
            ),
            ProviderSemanticOutput::Usage(event) => (
                MetadataKind::Cache,
                "ReasonResp02UsageEvent",
                "usage",
                vec![
                    MetadataEntry {
                        key: "usage.input_tokens".to_owned(),
                        value: json!(event.usage.total_input_tokens()),
                    },
                    MetadataEntry {
                        key: "usage.output_tokens".to_owned(),
                        value: json!(event.usage.output_tokens),
                    },
                    MetadataEntry {
                        key: "usage.cache_hit_rate".to_owned(),
                        value: json!(event.usage.cache_hit_rate()),
                    },
                ],
            ),
            ProviderSemanticOutput::Terminal(event) => (
                MetadataKind::Provider,
                "ReasonProviderTerminalObserved",
                "provider_terminal_observed",
                vec![
                    MetadataEntry {
                        key: "provider_terminal.status".to_owned(),
                        value: json!(format!("{:?}", event.status)),
                    },
                    MetadataEntry {
                        key: "provider_terminal.final_truth".to_owned(),
                        value: json!(false),
                    },
                ],
            ),
            ProviderSemanticOutput::Error(event) => (
                MetadataKind::RuntimeState,
                "ErrorErr01RuntimeClassified",
                "provider_error",
                vec![
                    MetadataEntry {
                        key: "error.class".to_owned(),
                        value: json!(format!("{:?}", event.error.class)),
                    },
                    MetadataEntry {
                        key: "error.recovery".to_owned(),
                        value: json!(format!("{:?}", event.error.recovery)),
                    },
                ],
            ),
        };
        let mut entries = vec![MetadataEntry {
            key: "provider_output.kind".to_owned(),
            value: json!(output_kind),
        }];
        entries.extend(extra_entries);
        self.write_metadata(
            turn,
            kind,
            pipeline_node,
            output_kind,
            "ReasonTurnEngine::apply_provider_output",
            entries,
        )
    }

    fn write_metadata(
        &self,
        turn: &TurnRecord,
        kind: MetadataKind,
        pipeline_node: &str,
        metadata_suffix: &str,
        symbol_path: &str,
        entries: Vec<MetadataEntry>,
    ) -> Result<(), ReasonTurnError> {
        let Some(center) = &self.metadata_center else {
            return Ok(());
        };
        let envelope = MetadataEnvelope::new(
            MetadataId::new(format!(
                "{}:{}:{}",
                turn.request.trace_id.as_str(),
                pipeline_node,
                metadata_suffix
            )),
            kind,
            MetadataWriteOwner {
                feature_id: FeatureId::new("reason.turn"),
                crate_name: "freehand-reason".to_owned(),
                module_path: "freehand_reason".to_owned(),
                symbol_path: symbol_path.to_owned(),
            },
            MetadataWriteNode {
                pipeline_node: pipeline_node.to_owned(),
                runtime_node_id: None,
            },
            MetadataSubject {
                agent_id: Some(turn.request.agent_id.clone()),
                session_id: Some(turn.request.session_id.clone()),
                turn_id: Some(turn.request.turn_id.clone()),
                trace_id: turn.request.trace_id.clone(),
            },
            entries,
        )
        .map_err(|err| ReasonTurnError::MetadataWriteFailed(err.to_string()))?;
        center
            .lock()
            .map_err(|err| ReasonTurnError::MetadataWriteFailed(err.to_string()))?
            .write(envelope)
            .map_err(|err| ReasonTurnError::MetadataWriteFailed(err.to_string()))
    }
}

fn unix_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn unix_millis_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| {
            let millis = duration.as_millis();
            u64::try_from(millis).unwrap_or(u64::MAX)
        })
        .unwrap_or(0)
}

fn unix_timestamp_string() -> String {
    unix_seconds_now().to_string()
}

fn completion_error_message(err: CompletionValidationError) -> String {
    match err {
        CompletionValidationError::MissingField(field) => {
            format!("missing required completion field `{field}`")
        }
        CompletionValidationError::EmptyField(field) => {
            format!("completion field `{field}` must not be empty")
        }
        CompletionValidationError::MissingNextStep => {
            "completion requires valid `next_step` when not complete".to_owned()
        }
        CompletionValidationError::MissingBlockedReason => {
            "completion requires valid `blocked_reason` when blocked".to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_blocks::CompletionClaim;
    use freehand_contracts::{
        ContextCachePolicy, ContextRewriteMode, ContextRole, ContextSegmentKind, ContextStability,
        TerminalStatus, TokenUsage, ToolArgument, ToolCallContract, ToolCallId,
    };
    use freehand_debug::{DebugHub, DebugSink, DebugSinkError, DebugSinkKind};
    use freehand_provider_core::ProviderAdapterEvent;
    use serde_json::json;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct FailingDebugSink;

    impl DebugSink for FailingDebugSink {
        fn kind(&self) -> DebugSinkKind {
            DebugSinkKind::ReplayCapture
        }

        fn handle(&self, _event: &freehand_debug::DebugEvent) -> Result<(), DebugSinkError> {
            Err(DebugSinkError::Io("reason debug sink failed".to_owned()))
        }
    }

    fn session_history() -> SessionHistory {
        SessionHistory::new(
            SessionId::new("session-1"),
            vec![ContextSegment {
                segment_id: ContextSegmentId::new("segment-memory"),
                kind: ContextSegmentKind::SessionMemory,
                stability: ContextStability::SessionStable,
                cache_policy: ContextCachePolicy::Cacheable,
                role: ContextRole::Developer,
                content: "ctx".to_owned(),
                token_budget: 64,
                provenance: ContextProvenance {
                    source: "memory".to_owned(),
                    reference: None,
                },
            }],
        )
        .expect("history")
    }

    fn start_input() -> TurnStartInput {
        TurnStartInput {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.turn"),
            agent_id: AgentId::new("agent-1"),
            user_text: "hello".to_owned(),
            planned_context_segments: Vec::new(),
            tool_schema_fingerprint: None,
            model: "gpt-test".to_owned(),
        }
    }

    #[test]
    fn projects_session_from_per_turn_truth() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let projected = engine.project_session(&[turn]);
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].user_text, "hello");
        assert_eq!(projected[0].terminal_summary, None);
    }

    #[test]
    fn provider_terminal_event_closes_turn_without_completion_schema() {
        let engine = ReasonTurnEngine::new();
        let receiver = engine.subscribe(4);
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let provider_terminal = ReasonResp03TerminalEvent {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            status: TerminalStatus::Success,
            summary: "end_turn".to_owned(),
            user_options: None,
        };
        engine
            .apply_provider_output(
                &mut turn,
                ProviderSemanticOutput::Terminal(provider_terminal.clone()),
            )
            .expect("provider terminal must close turn truth");
        assert_eq!(
            turn.terminal_event.as_ref(),
            Some(&provider_terminal),
            "provider terminal is objective turn truth and must not wait for a completion schema"
        );
        assert!(turn.timing.completed_at_ms.is_some());
        assert!(matches!(
            receiver.recv().expect("terminal broadcast"),
            ReasonBroadcastEvent::Terminal(event)
                if event == provider_terminal
        ));
    }

    #[test]
    fn provider_terminal_failure_preserves_objective_status() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let provider_terminal = ReasonResp03TerminalEvent {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            status: TerminalStatus::Interrupted,
            summary: "max_tokens".to_owned(),
            user_options: None,
        };
        engine
            .apply_provider_output(
                &mut turn,
                ProviderSemanticOutput::Terminal(provider_terminal.clone()),
            )
            .expect("provider terminal must close turn truth");
        assert_eq!(
            turn.terminal_event.as_ref(),
            Some(&provider_terminal),
            "provider-observed interruption must remain an objective terminal status"
        );
    }

    #[test]
    fn writes_tool_result_reentry_back_to_owning_turn() {
        let engine = ReasonTurnEngine::new();
        let receiver = engine.subscribe(4);
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let result = ReasonReq05ToolResultReentry {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            tool_result: freehand_contracts::ToolResultContract {
                tool_call_id: ToolCallId::new("tool-1"),
                status: freehand_contracts::ToolResultStatus::Success,
                output: "done".to_owned(),
                search_evidence: None,
            },
        };
        engine
            .apply_provider_output(
                &mut turn,
                ProviderSemanticOutput::ToolResultReentry(result.clone()),
            )
            .expect("apply provider output");
        assert_eq!(turn.tool_results, vec![result]);
        assert!(matches!(
            receiver.recv().expect("tool result broadcast"),
            ReasonBroadcastEvent::ToolResult(_)
        ));
    }

    #[test]
    fn search_evidence_is_persisted_only_after_verified_source_binding() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let plan = freehand_contracts::SearchDomainPlanDelivery {
            schema: "search_evidence.domain_plan.v1".to_owned(),
            delivery_id: "domain-1".to_owned(),
            domain: freehand_contracts::SearchDomain::News,
            preferred_source_kinds: vec!["official_publication".to_owned()],
            social_platform_priority: vec![freehand_contracts::SearchSocialPlatform::Weibo],
            minimum_verified_sources: 1,
            policy_version: "2026-08-15".to_owned(),
        };
        let discovery = freehand_contracts::SearchDiscoveryDelivery {
            schema: "search_evidence.discovery.v1".to_owned(),
            delivery_id: "hosted-1".to_owned(),
            discovery_channel: freehand_contracts::SearchDiscoveryChannel::HostedWebSearch,
            domain_plan_ref: Some("domain-1".to_owned()),
            hosted_search_attempt: Some(freehand_contracts::SearchHostedAttempt {
                tool_call_id: None,
                status: None,
                result_count: None,
                query: "news".to_owned(),
                provider: "openai_responses".to_owned(),
            }),
            candidates: vec![freehand_contracts::SearchDiscoveryCandidate {
                candidate_id: "c1".to_owned(),
                status: freehand_contracts::SearchCandidateStatus::Usable,
                original_url: Some("https://example.com/news".to_owned()),
                title: "News".to_owned(),
                snippet: "search snippet".to_owned(),
                discovered_by: Some(freehand_contracts::SearchDiscoveryChannel::HostedWebSearch),
                platform: Some(freehand_contracts::SearchSocialPlatform::Web),
                source_weight: Some(90),
                reason: None,
            }],
        };
        let source = freehand_contracts::SearchVerificationDelivery {
            schema: "search_evidence.verification.v1".to_owned(),
            delivery_id: "verify-c1".to_owned(),
            source_id: "c1".to_owned(),
            original_url: "https://example.com/news".to_owned(),
            camo_profile: "default".to_owned(),
            accessed_at: "2026-08-15T12:00:00Z".to_owned(),
            access_status: freehand_contracts::SearchAccessStatus::Verified,
            page_title: Some("News".to_owned()),
            evidence_excerpt: Some("Evidence".to_owned()),
            verified_by: Some("camo".to_owned()),
            access_attempts: vec![freehand_contracts::SearchAccessAttempt {
                attempt_id: "attempt-1".to_owned(),
                channel: "camo".to_owned(),
                status: freehand_contracts::SearchAccessStatus::Verified,
                accessed_at: "2026-08-15T12:00:00Z".to_owned(),
                error: None,
            }],
            error: None,
        };
        let supplement = freehand_contracts::SocialSupplementDecisionDelivery {
            schema: "search_evidence.supplement_decision.v1".to_owned(),
            delivery_id: "supplement-1".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            required: false,
            reasons: Vec::new(),
            platforms: Vec::new(),
        };
        let final_delivery = freehand_contracts::SearchFinalDelivery {
            schema: "search_evidence.final.v1".to_owned(),
            delivery_id: "final-1".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            claim: freehand_contracts::SearchFinalClaimStatus::Complete,
            summary: Some("Summary".to_owned()),
            claims: vec![freehand_contracts::SearchClaimDelivery {
                claim_id: "claim-1".to_owned(),
                text: "Claim".to_owned(),
                source_ids: vec!["c1".to_owned()],
            }],
            unconfirmed: Vec::new(),
            blocked_reason: None,
        };
        for (index, delivery) in [
            SearchEvidenceDelivery::DomainPlan(plan),
            SearchEvidenceDelivery::Discovery(discovery),
            SearchEvidenceDelivery::Verification(source),
            SearchEvidenceDelivery::SupplementDecision(supplement),
        ]
        .into_iter()
        .enumerate()
        {
            let result = ReasonReq05ToolResultReentry {
                session_id: turn.request.session_id.clone(),
                turn_id: turn.request.turn_id.clone(),
                trace_id: turn.request.trace_id.clone(),
                feature_id: turn.request.feature_id.clone(),
                agent_id: turn.request.agent_id.clone(),
                tool_result: freehand_contracts::ToolResultContract {
                    tool_call_id: ToolCallId::new(format!("search-stage-{index}")),
                    status: freehand_contracts::ToolResultStatus::Success,
                    output: "typed search stage".to_owned(),
                    search_evidence: Some(delivery),
                },
            };
            engine
                .apply_provider_output(&mut turn, ProviderSemanticOutput::ToolResultReentry(result))
                .expect("append search stage");
        }
        engine
            .apply_search_evidence_delivery(&mut turn, final_delivery)
            .expect("valid search evidence");
        assert!(turn.search_evidence.as_ref().is_some_and(|evidence| {
            evidence.summary_ready
                && evidence.status == freehand_contracts::SearchEvidenceTurnStatus::FinalValidated
                && evidence.terminal.is_none()
        }));

        let submission = CompletionSubmission {
            claim: CompletionClaim::Complete,
            completion_reason: Some("search evidence validated".to_owned()),
            evidence: Some("camo source bound".to_owned()),
            summary: Some("Summary".to_owned()),
            learned: Some("typed search evidence".to_owned()),
            next_step: None,
            blocked_reason: None,
            user_options: None,
        };
        engine
            .submit_completion(&mut turn, &submission)
            .expect("terminal completion");
        assert!(turn.search_evidence.as_ref().is_some_and(|evidence| {
            evidence.status == freehand_contracts::SearchEvidenceTurnStatus::TurnTerminalSuccess
                && evidence.terminal == Some(freehand_contracts::SearchEvidenceTerminal::Success)
        }));
    }

    #[test]
    fn search_evidence_rejects_unverified_final_without_mutating_turn() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let final_delivery = freehand_contracts::SearchFinalDelivery {
            schema: "search_evidence.final.v1".to_owned(),
            delivery_id: "final-1".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            claim: freehand_contracts::SearchFinalClaimStatus::Complete,
            summary: Some("Unsupported summary".to_owned()),
            claims: vec![freehand_contracts::SearchClaimDelivery {
                claim_id: "claim-1".to_owned(),
                text: "Claim".to_owned(),
                source_ids: vec!["missing".to_owned()],
            }],
            unconfirmed: Vec::new(),
            blocked_reason: None,
        };
        assert!(
            engine
                .apply_search_evidence_delivery(&mut turn, final_delivery)
                .is_err()
        );
        assert!(turn.search_evidence.is_none());
    }

    #[test]
    fn search_stage_owner_rejects_final_and_invalid_stage_without_mutation() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let final_delivery = freehand_contracts::SearchFinalDelivery {
            schema: "search_evidence.final.v1".to_owned(),
            delivery_id: "final-1".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            claim: freehand_contracts::SearchFinalClaimStatus::Blocked,
            summary: None,
            claims: Vec::new(),
            unconfirmed: Vec::new(),
            blocked_reason: Some("no_verified_source".to_owned()),
        };
        assert!(
            engine
                .apply_search_evidence_stage_delivery(
                    &mut turn,
                    SearchEvidenceDelivery::Final(final_delivery),
                )
                .is_err()
        );
        assert!(turn.search_evidence.is_none());

        let discovery = freehand_contracts::SearchDiscoveryDelivery {
            schema: "search_evidence.discovery.v1".to_owned(),
            delivery_id: "hosted-1".to_owned(),
            discovery_channel: freehand_contracts::SearchDiscoveryChannel::HostedWebSearch,
            domain_plan_ref: Some("domain-1".to_owned()),
            hosted_search_attempt: Some(freehand_contracts::SearchHostedAttempt {
                tool_call_id: None,
                status: None,
                result_count: None,
                query: "news".to_owned(),
                provider: "openai_responses".to_owned(),
            }),
            candidates: vec![freehand_contracts::SearchDiscoveryCandidate {
                candidate_id: "c1".to_owned(),
                status: freehand_contracts::SearchCandidateStatus::Usable,
                original_url: Some("https://example.com/news".to_owned()),
                title: "News".to_owned(),
                snippet: "snippet".to_owned(),
                discovered_by: Some(freehand_contracts::SearchDiscoveryChannel::HostedWebSearch),
                platform: Some(freehand_contracts::SearchSocialPlatform::Web),
                source_weight: Some(90),
                reason: None,
            }],
        };
        assert!(
            engine
                .apply_search_evidence_stage_delivery(
                    &mut turn,
                    SearchEvidenceDelivery::Discovery(discovery),
                )
                .is_err()
        );
        assert!(turn.search_evidence.is_none());
    }

    #[test]
    fn search_completion_rejects_generic_complete_before_final_delivery() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let plan = freehand_contracts::SearchDomainPlanDelivery {
            schema: "search_evidence.domain_plan.v1".to_owned(),
            delivery_id: "domain-1".to_owned(),
            domain: freehand_contracts::SearchDomain::News,
            preferred_source_kinds: vec!["official_publication".to_owned()],
            social_platform_priority: vec![freehand_contracts::SearchSocialPlatform::Weibo],
            minimum_verified_sources: 1,
            policy_version: "2026-08-15".to_owned(),
        };
        engine
            .apply_search_evidence_stage_delivery(
                &mut turn,
                SearchEvidenceDelivery::DomainPlan(plan),
            )
            .expect("domain plan");
        let submission = CompletionSubmission {
            claim: CompletionClaim::Complete,
            completion_reason: Some("done".to_owned()),
            evidence: Some("none".to_owned()),
            summary: Some("unsupported summary".to_owned()),
            learned: Some("none".to_owned()),
            next_step: None,
            blocked_reason: None,
            user_options: None,
        };
        assert!(engine.submit_completion(&mut turn, &submission).is_err());
        assert!(turn.terminal_event.is_none());
    }

    #[test]
    fn non_sourced_hosted_discovery_streams_observation_only_and_completion_unblocks() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let discovery = freehand_contracts::SearchDiscoveryDelivery {
            schema: "search_evidence.discovery.v1".to_owned(),
            delivery_id: "hosted-non-sourced".to_owned(),
            discovery_channel: freehand_contracts::SearchDiscoveryChannel::HostedWebSearch,
            domain_plan_ref: None,
            hosted_search_attempt: Some(freehand_contracts::SearchHostedAttempt {
                tool_call_id: Some("srv-1".to_owned()),
                status: Some("completed".to_owned()),
                result_count: Some(3),
                query: "shenzhen".to_owned(),
                provider: "anthropic_messages".to_owned(),
            }),
            candidates: Vec::new(),
        };
        engine
            .apply_search_evidence_stage_delivery(
                &mut turn,
                SearchEvidenceDelivery::Discovery(discovery),
            )
            .expect("non-sourced hosted discovery accepted");
        let evidence = turn.search_evidence.as_ref().expect("evidence");
        assert!(
            evidence.domain_plan.is_none(),
            "non-sourced hosted discovery must not synthesize a domain plan"
        );
        assert_eq!(evidence.deliveries.len(), 1);

        // Without a domain plan, completion is unblocked and accepted.
        let submission = CompletionSubmission {
            claim: CompletionClaim::Complete,
            completion_reason: Some("hosted_search_observation_only".to_owned()),
            evidence: Some("hosted search observation".to_owned()),
            summary: Some("observation-only summary".to_owned()),
            learned: Some("non-sourced completion is unblocked".to_owned()),
            next_step: None,
            blocked_reason: None,
            user_options: None,
        };
        engine
            .submit_completion(&mut turn, &submission)
            .expect("completion must not be blocked by sourced final gate");
        assert!(turn.terminal_event.is_some());
    }

    #[test]
    fn non_sourced_stage_appends_still_reject_verification_and_final() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let discovery = freehand_contracts::SearchDiscoveryDelivery {
            schema: "search_evidence.discovery.v1".to_owned(),
            delivery_id: "hosted-non-sourced".to_owned(),
            discovery_channel: freehand_contracts::SearchDiscoveryChannel::HostedWebSearch,
            domain_plan_ref: None,
            hosted_search_attempt: Some(freehand_contracts::SearchHostedAttempt {
                tool_call_id: None,
                status: Some("completed".to_owned()),
                result_count: Some(0),
                query: "shenzhen".to_owned(),
                provider: "anthropic_messages".to_owned(),
            }),
            candidates: Vec::new(),
        };
        engine
            .apply_search_evidence_stage_delivery(
                &mut turn,
                SearchEvidenceDelivery::Discovery(discovery),
            )
            .expect("non-sourced hosted discovery accepted");

        // Reverse guard: a verification or final delivery cannot follow an
        // observation-only hosted discovery; the sourced stage gate stays strict.
        let final_delivery = freehand_contracts::SearchFinalDelivery {
            schema: "search_evidence.final.v1".to_owned(),
            delivery_id: "final-illegal".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            claim: freehand_contracts::SearchFinalClaimStatus::Complete,
            summary: Some("Should not validate".to_owned()),
            claims: Vec::new(),
            unconfirmed: Vec::new(),
            blocked_reason: None,
        };
        assert!(
            engine
                .apply_search_evidence_delivery(&mut turn, final_delivery)
                .is_err()
        );
    }

    #[test]
    fn turn_timing_records_first_provider_response_and_terminal_elapsed() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let started_at = turn.timing.turn_started_at_ms.expect("turn start timing");
        assert!(turn.timing.first_response_at_ms.is_none());
        assert!(turn.timing.time_to_first_response_ms.is_none());
        assert!(turn.timing.total_elapsed_ms.is_none());

        let result = ReasonReq05ToolResultReentry {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            tool_result: freehand_contracts::ToolResultContract {
                tool_call_id: ToolCallId::new("tool-1"),
                status: freehand_contracts::ToolResultStatus::Success,
                output: "done".to_owned(),
                search_evidence: None,
            },
        };
        engine
            .apply_provider_output(&mut turn, ProviderSemanticOutput::ToolResultReentry(result))
            .expect("tool result reentry");
        assert!(
            turn.timing.first_response_at_ms.is_none(),
            "tool result re-entry is not a provider first response"
        );

        let ctx = freehand_provider_core::ProviderEventContext {
            agent_id: turn.request.agent_id.clone(),
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            search_domain_plan_ref: None,
        };
        engine
            .apply_provider_output(
                &mut turn,
                freehand_provider_core::map_adapter_event(
                    &ctx,
                    ProviderAdapterEvent::TextDelta("first byte".to_owned()),
                ),
            )
            .expect("first provider output");
        let first_response_at = turn
            .timing
            .first_response_at_ms
            .expect("first response timing");
        assert!(first_response_at >= started_at);
        assert_eq!(
            turn.timing.time_to_first_response_ms,
            Some(first_response_at - started_at)
        );

        engine
            .apply_provider_output(
                &mut turn,
                freehand_provider_core::map_adapter_event(
                    &ctx,
                    ProviderAdapterEvent::TextDelta("second byte".to_owned()),
                ),
            )
            .expect("second provider output");
        assert_eq!(turn.timing.first_response_at_ms, Some(first_response_at));

        engine
            .submit_completion(
                &mut turn,
                &CompletionSubmission {
                    claim: CompletionClaim::Complete,
                    completion_reason: Some("done".to_owned()),
                    evidence: Some("tests passed".to_owned()),
                    summary: Some("completed task".to_owned()),
                    learned: Some("keep schema strict".to_owned()),
                    next_step: None,
                    blocked_reason: None,
                    user_options: None,
                },
            )
            .expect("terminal");
        let completed_at = turn.timing.completed_at_ms.expect("completed timing");
        assert!(completed_at >= started_at);
        assert_eq!(
            turn.timing.total_elapsed_ms,
            Some(completed_at - started_at)
        );
    }

    #[test]
    fn accepts_valid_completion_schema_and_emits_terminal() {
        let engine = ReasonTurnEngine::new();
        let receiver = engine.subscribe(4);
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let terminal = engine
            .submit_completion(
                &mut turn,
                &CompletionSubmission {
                    claim: CompletionClaim::Complete,
                    completion_reason: Some("done".to_owned()),
                    evidence: Some("tests passed".to_owned()),
                    summary: Some("completed task".to_owned()),
                    learned: Some("keep schema strict".to_owned()),
                    next_step: None,
                    blocked_reason: None,
                    user_options: None,
                },
            )
            .expect("terminal");
        assert_eq!(terminal.status, TerminalStatus::Success);
        let broadcast = receiver.recv().expect("broadcast");
        match broadcast {
            ReasonBroadcastEvent::Terminal(event) => {
                assert_eq!(event.status, TerminalStatus::Success)
            }
            other => panic!("unexpected broadcast: {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_completion_schema() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let err = engine
            .submit_completion(
                &mut turn,
                &CompletionSubmission {
                    claim: CompletionClaim::Complete,
                    completion_reason: Some("done".to_owned()),
                    evidence: None,
                    summary: Some("completed task".to_owned()),
                    learned: Some("keep schema strict".to_owned()),
                    next_step: None,
                    blocked_reason: None,
                    user_options: None,
                },
            )
            .expect_err("should fail");
        assert!(matches!(err, ReasonTurnError::CompletionRejected(_)));
    }

    #[test]
    fn writes_failed_terminal_when_requested() {
        let engine = ReasonTurnEngine::new();
        let receiver = engine.subscribe(4);
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let terminal = engine.fail_turn(&mut turn, "schema retry limit exhausted");
        assert_eq!(terminal.status, TerminalStatus::Failed);
        let broadcast = receiver.recv().expect("broadcast");
        match broadcast {
            ReasonBroadcastEvent::Terminal(event) => {
                assert_eq!(event.status, TerminalStatus::Failed);
                assert!(event.summary.contains("schema retry limit exhausted"));
            }
            other => panic!("unexpected broadcast: {other:?}"),
        }
    }

    #[test]
    fn writes_cancelled_terminal_when_requested() {
        let engine = ReasonTurnEngine::new();
        let receiver = engine.subscribe(4);
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let terminal = engine.cancel_turn(&mut turn, "cancelled by ui command");
        assert_eq!(terminal.status, TerminalStatus::Cancelled);
        let broadcast = receiver.recv().expect("broadcast");
        match broadcast {
            ReasonBroadcastEvent::Terminal(event) => {
                assert_eq!(event.status, TerminalStatus::Cancelled);
                assert!(event.summary.contains("cancelled by ui command"));
            }
            other => panic!("unexpected broadcast: {other:?}"),
        }
    }
    #[test]
    fn cancel_already_terminal_turn_does_not_overwrite_terminal() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let submission = CompletionSubmission {
            claim: CompletionClaim::Complete,
            completion_reason: Some("done".to_owned()),
            evidence: Some("tests passed".to_owned()),
            summary: Some("completed task".to_owned()),
            learned: Some("keep schema strict".to_owned()),
            next_step: None,
            blocked_reason: None,
            user_options: None,
        };
        let terminal = engine
            .submit_completion(&mut turn, &submission)
            .expect("terminal");
        assert_eq!(terminal.status, TerminalStatus::Success);

        let second = engine.cancel_turn(&mut turn, "cancelled after terminal");
        assert_eq!(second.status, TerminalStatus::Success);
        assert_eq!(second.summary, terminal.summary);
        assert!(
            turn.terminal_event
                .as_ref()
                .expect("terminal")
                .summary
                .contains("completed task")
        );
    }
    #[test]
    fn slow_subscriber_does_not_block_main_path() {
        let engine = ReasonTurnEngine::new();
        let _receiver = engine.subscribe(1);
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let ctx = freehand_provider_core::ProviderEventContext {
            agent_id: turn.request.agent_id.clone(),
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            search_domain_plan_ref: None,
        };
        engine
            .apply_provider_output(
                &mut turn,
                freehand_provider_core::map_adapter_event(
                    &ctx,
                    ProviderAdapterEvent::ReasoningDelta("step-1".to_owned()),
                ),
            )
            .expect("apply first provider output");
        engine
            .apply_provider_output(
                &mut turn,
                freehand_provider_core::map_adapter_event(
                    &ctx,
                    ProviderAdapterEvent::TextDelta("step-2".to_owned()),
                ),
            )
            .expect("apply second provider output");
        assert_eq!(turn.semantic_events.len(), 2);
    }

    #[test]
    fn broadcasts_semantic_and_usage_events() {
        let engine = ReasonTurnEngine::new();
        let receiver = engine.subscribe(4);
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let ctx = freehand_provider_core::ProviderEventContext {
            agent_id: turn.request.agent_id.clone(),
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            search_domain_plan_ref: None,
        };
        engine
            .apply_provider_output(
                &mut turn,
                freehand_provider_core::map_adapter_event(
                    &ctx,
                    ProviderAdapterEvent::ToolCall(ToolCallContract {
                        tool_call_id: ToolCallId::new("tool-1"),
                        tool_name: "search".to_owned(),
                        arguments: vec![ToolArgument {
                            name: "query".to_owned(),
                            value: json!("rust"),
                        }],
                        arguments_complete: true,
                    }),
                ),
            )
            .expect("apply tool call output");
        engine
            .apply_provider_output(
                &mut turn,
                freehand_provider_core::map_adapter_event(
                    &ctx,
                    ProviderAdapterEvent::Usage(TokenUsage {
                        input_tokens: 10,
                        output_tokens: 5,
                        total_tokens: Some(15),
                        reasoning_tokens: Some(4),
                        cache_creation_tokens: 0,
                        cache_read_tokens: 0,
                        normalized_input_tokens: Some(10),
                        finish_reason: Some("stop".to_owned()),
                    }),
                ),
            )
            .expect("apply usage output");

        let first = receiver.recv().expect("first");
        let second = receiver.recv().expect("second");
        assert!(matches!(first, ReasonBroadcastEvent::Tool(_)));
        assert!(matches!(second, ReasonBroadcastEvent::Usage(_)));
    }

    #[test]
    fn ordinary_turn_keeps_rewrite_version_and_mode_from_session_truth() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let turn_a = engine
            .start_turn(&mut history, start_input())
            .expect("turn a");
        let turn_b = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    turn_id: TurnId::new("turn-2"),
                    trace_id: TraceId::new("trace-2"),
                    user_text: "hello again".to_owned(),
                    ..start_input()
                },
            )
            .expect("turn b");

        assert_eq!(
            turn_a.planned_context.diagnostics.rewrite_mode,
            ContextRewriteMode::OrdinaryTurn
        );
        assert_eq!(turn_a.planned_context.diagnostics.rewrite_version, 0);
        assert_eq!(
            turn_a.planned_context.diagnostics.stable_prefix_hash,
            turn_b.planned_context.diagnostics.stable_prefix_hash
        );
        assert_eq!(history.rewrite_version(), 0);
        assert_eq!(
            history.current_rewrite_mode(),
            ContextRewriteMode::OrdinaryTurn
        );
    }

    #[test]
    fn start_turn_forwards_tool_schema_fingerprint_to_planner_diagnostics() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let turn_without = engine
            .start_turn(&mut history, start_input())
            .expect("turn without fingerprint");

        let mut history_with = session_history();
        let turn_with = engine
            .start_turn(
                &mut history_with,
                TurnStartInput {
                    turn_id: TurnId::new("turn-2"),
                    trace_id: TraceId::new("trace-2"),
                    tool_schema_fingerprint: Some("tool-registry-v1".to_owned()),
                    ..start_input()
                },
            )
            .expect("turn with fingerprint");

        assert_ne!(
            turn_without.planned_context.diagnostics.tool_schema_hash,
            turn_with.planned_context.diagnostics.tool_schema_hash
        );
        assert_eq!(
            turn_without.planned_context.diagnostics.stable_prefix_hash,
            turn_with.planned_context.diagnostics.stable_prefix_hash
        );
    }

    #[test]
    fn start_turn_rejects_session_history_mismatch_explicitly() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();

        let err = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-other"),
                    ..start_input()
                },
            )
            .expect_err("session mismatch must fail");

        assert_eq!(
            err,
            ReasonTurnError::SessionMismatch("session-other".to_owned())
        );
    }

    #[test]
    fn explicit_rewrite_gate_bumps_version_and_is_consumed_by_next_turn() {
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        history
            .stage_compaction(
                vec![ContextSegment {
                    segment_id: ContextSegmentId::new("segment-summary"),
                    kind: ContextSegmentKind::SessionSummary,
                    stability: ContextStability::SessionStable,
                    cache_policy: ContextCachePolicy::Cacheable,
                    role: ContextRole::Developer,
                    content: "compacted".to_owned(),
                    token_budget: 64,
                    provenance: ContextProvenance {
                        source: "compaction".to_owned(),
                        reference: None,
                    },
                }],
                "compact stale context",
            )
            .expect("rewrite");

        let turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        assert_eq!(
            turn.planned_context.diagnostics.rewrite_mode,
            ContextRewriteMode::Compaction
        );
        assert_eq!(turn.planned_context.diagnostics.rewrite_version, 1);
        assert_eq!(
            history.current_rewrite_mode(),
            ContextRewriteMode::OrdinaryTurn
        );
        assert_eq!(history.rewrite_version(), 1);
        assert_eq!(
            history
                .rewrite_ledger()
                .last()
                .and_then(|record| record.applied_turn_id.clone()),
            Some(TurnId::new("turn-1"))
        );

        let ordinary_after = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    turn_id: TurnId::new("turn-2"),
                    trace_id: TraceId::new("trace-2"),
                    user_text: "ordinary again".to_owned(),
                    ..start_input()
                },
            )
            .expect("turn");
        assert_eq!(
            ordinary_after.planned_context.diagnostics.rewrite_mode,
            ContextRewriteMode::OrdinaryTurn
        );
        assert_eq!(
            ordinary_after.planned_context.diagnostics.rewrite_version,
            1
        );
    }

    #[test]
    fn emits_debug_event_without_mutating_turn_truth() {
        let debug_hub = Arc::new(DebugHub::new(true));
        let debug_receiver = debug_hub.subscribe(4);
        let engine = ReasonTurnEngine::with_debug_hub(debug_hub);
        let mut history = session_history();
        let turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");

        let event = debug_receiver.recv().expect("debug event");
        let snapshot = event.snapshot.expect("snapshot");
        assert_eq!(snapshot.status_text, "reason turn started");
        assert_eq!(snapshot.semantic.turn_id, TurnId::new("turn-1"));
        assert!(
            snapshot
                .detail_lines
                .iter()
                .any(|line| line == "model=gpt-test")
        );
        assert!(turn.semantic_events.is_empty());
        assert!(turn.terminal_event.is_none());
    }

    #[test]
    fn surfaces_debug_sink_failure_without_mutating_turn_truth() {
        let debug_hub = Arc::new(DebugHub::new(true));
        debug_hub.add_sink(FailingDebugSink);
        let failure_receiver = debug_hub.subscribe_failures(1);
        let engine = ReasonTurnEngine::with_debug_hub(debug_hub);
        let mut history = session_history();

        let turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn should still start");

        let failure = failure_receiver.recv().expect("failure event");
        assert_eq!(failure.sink_kind, DebugSinkKind::ReplayCapture);
        assert_eq!(
            failure.event_envelope.semantic.feature_id,
            FeatureId::new("reason.turn")
        );
        assert_eq!(
            failure.event_envelope.scene.function,
            "ReasonTurnEngine::start_turn"
        );
        assert_eq!(failure.message, "io failure: reason debug sink failed");
        assert!(turn.semantic_events.is_empty());
        assert!(turn.error_events.is_empty());
        assert!(turn.terminal_event.is_none());
    }

    #[test]
    fn writes_start_turn_metadata_with_owner_node_and_without_request_text() {
        let center = Arc::new(Mutex::new(MetadataCenter::new()));
        let engine = ReasonTurnEngine::with_metadata_center(Arc::clone(&center));
        let mut history = session_history();
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    user_text: "secret operator prompt".to_owned(),
                    ..start_input()
                },
            )
            .expect("turn");

        let center = center.lock().expect("metadata center");
        assert_eq!(center.records().len(), 1);
        let record = &center.records()[0];
        assert_eq!(record.owner.feature_id, FeatureId::new("reason.turn"));
        assert_eq!(
            record.owner.symbol_path,
            "ReasonTurnEngine::start_turn".to_owned()
        );
        assert_eq!(
            record.write_node.pipeline_node,
            "ReasonReq02ContextComposedInput".to_owned()
        );
        assert_eq!(record.subject.turn_id, Some(TurnId::new("turn-1")));
        let encoded = serde_json::to_string(record).expect("metadata json");
        assert!(!encoded.contains("secret operator prompt"));
        assert_eq!(turn.request.user_text, "secret operator prompt");
    }

    #[test]
    fn writes_provider_output_metadata_for_usage_without_request_payload() {
        let center = Arc::new(Mutex::new(MetadataCenter::new()));
        let engine = ReasonTurnEngine::with_metadata_center(Arc::clone(&center));
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let usage = ProviderSemanticOutput::Usage(freehand_contracts::ReasonResp02UsageEvent {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            usage: TokenUsage {
                input_tokens: 10,
                output_tokens: 5,
                total_tokens: Some(15),
                reasoning_tokens: Some(3),
                cache_creation_tokens: 2,
                cache_read_tokens: 8,
                normalized_input_tokens: Some(10),
                finish_reason: Some("stop".to_owned()),
            },
        });

        engine
            .apply_provider_output(&mut turn, usage)
            .expect("apply usage");

        let center = center.lock().expect("metadata center");
        let usage_record = center
            .records()
            .iter()
            .find(|record| record.write_node.pipeline_node == "ReasonResp02UsageEvent")
            .expect("usage metadata");
        assert_eq!(usage_record.kind, MetadataKind::Cache);
        assert_eq!(
            usage_record.owner.symbol_path,
            "ReasonTurnEngine::apply_provider_output".to_owned()
        );
        assert!(
            usage_record
                .entries
                .iter()
                .any(|entry| entry.key == "usage.cache_hit_rate")
        );
        let encoded = serde_json::to_string(usage_record).expect("metadata json");
        assert!(!encoded.contains("hello"));
    }

    #[test]
    fn persists_reason_turn_metadata_to_durable_ledger_without_request_text() {
        let ledger_path = temp_metadata_ledger_path("reason-turn-metadata");
        let center = Arc::new(Mutex::new(
            MetadataCenter::with_ledger_path(&ledger_path).expect("metadata ledger center"),
        ));
        let engine = ReasonTurnEngine::with_metadata_center(Arc::clone(&center));
        let mut history = session_history();
        let mut turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    user_text: "secret operator prompt".to_owned(),
                    ..start_input()
                },
            )
            .expect("turn");

        let usage = ProviderSemanticOutput::Usage(freehand_contracts::ReasonResp02UsageEvent {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            usage: TokenUsage {
                input_tokens: 10,
                output_tokens: 5,
                total_tokens: Some(15),
                reasoning_tokens: Some(3),
                cache_creation_tokens: 2,
                cache_read_tokens: 8,
                normalized_input_tokens: Some(10),
                finish_reason: Some("stop".to_owned()),
            },
        });
        engine
            .apply_provider_output(&mut turn, usage)
            .expect("apply usage");

        let restored = MetadataCenter::with_ledger_path(&ledger_path).expect("restore ledger");
        assert_eq!(restored.records().len(), 2);
        assert!(
            restored
                .records()
                .iter()
                .any(|record| record.write_node.pipeline_node == "ReasonReq02ContextComposedInput")
        );
        assert!(
            restored
                .records()
                .iter()
                .any(|record| record.write_node.pipeline_node == "ReasonResp02UsageEvent")
        );
        let raw = std::fs::read_to_string(&ledger_path).expect("read metadata ledger");
        assert!(!raw.contains("secret operator prompt"));

        let _ = std::fs::remove_file(&ledger_path);
    }

    #[test]
    fn metadata_write_failure_does_not_commit_start_turn_history() {
        let center = Arc::new(Mutex::new(MetadataCenter::new()));
        let poison_center = Arc::clone(&center);
        let _ = std::thread::spawn(move || {
            let _guard = poison_center.lock().expect("metadata center");
            panic!("poison metadata center");
        })
        .join();
        let engine = ReasonTurnEngine::with_metadata_center(center);
        let mut history = session_history();
        history
            .stage_compaction(
                vec![ContextSegment {
                    segment_id: ContextSegmentId::new("segment-summary"),
                    kind: ContextSegmentKind::SessionSummary,
                    stability: ContextStability::SessionStable,
                    cache_policy: ContextCachePolicy::Cacheable,
                    role: ContextRole::Developer,
                    content: "compacted".to_owned(),
                    token_budget: 64,
                    provenance: ContextProvenance {
                        source: "compaction".to_owned(),
                        reference: None,
                    },
                }],
                "compact stale context",
            )
            .expect("rewrite");

        let err = engine
            .start_turn(&mut history, start_input())
            .expect_err("metadata write failure must fail start_turn");

        assert!(matches!(err, ReasonTurnError::MetadataWriteFailed(_)));
        assert_eq!(
            history.current_rewrite_mode(),
            ContextRewriteMode::Compaction
        );
        assert_eq!(
            history
                .rewrite_ledger()
                .last()
                .and_then(|record| record.applied_turn_id.clone()),
            None
        );
    }

    #[test]
    fn metadata_write_failure_does_not_mutate_provider_output_turn_truth() {
        let center = Arc::new(Mutex::new(MetadataCenter::new()));
        let engine = ReasonTurnEngine::with_metadata_center(Arc::clone(&center));
        let mut history = session_history();
        let mut turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn");
        let poison_center = Arc::clone(&center);
        let _ = std::thread::spawn(move || {
            let _guard = poison_center.lock().expect("metadata center");
            panic!("poison metadata center");
        })
        .join();

        let err = engine
            .apply_provider_output(
                &mut turn,
                ProviderSemanticOutput::SemanticEvent(
                    freehand_contracts::ReasonResp01SemanticEvent {
                        session_id: SessionId::new("session-1"),
                        turn_id: TurnId::new("turn-1"),
                        trace_id: TraceId::new("trace-1"),
                        feature_id: FeatureId::new("reason.turn"),
                        agent_id: AgentId::new("agent-1"),
                        kind: freehand_contracts::SemanticEventKind::Text,
                        content: "provider text".to_owned(),
                    },
                ),
            )
            .expect_err("metadata write failure must stop provider mutation");

        assert!(matches!(err, ReasonTurnError::MetadataWriteFailed(_)));
        assert!(turn.semantic_events.is_empty());
    }

    fn temp_metadata_ledger_path(prefix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{unique}.jsonl"))
    }
}
