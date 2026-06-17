//! Reasoning turn orchestration and event emission for Freehand.

mod persistence;
mod rewrite_runtime;
mod session_history;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_blocks::{
    CompletionDecision, CompletionSubmission, CompletionValidationError, ContextPlannerInput,
    PlannedContext, plan_context, validate_completion_submission,
};
use freehand_contracts::{
    AgentId, ContextProvenance, ContextSegment, ContextSegmentId, ErrorErr01RuntimeClassified,
    FeatureId, ReasonReq02ContextComposedInput, ReasonReq03ProviderPayload, ReasonReq04ToolCall,
    ReasonReq05ToolResultReentry, ReasonResp01SemanticEvent, ReasonResp02UsageEvent,
    ReasonResp03TerminalEvent, SessionId, TerminalStatus, TraceId, TurnId, validate_reason_req02,
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
    ActiveTurnSnapshot, PersistedSessionIndexEntry, PersistedSessionView, ReasonLedgerPayload,
    ReasonLedgerRow, ReasonPersistence, ReasonPersistenceCursor, ReasonPersistenceError,
    RestoredReasonSession,
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
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasonBroadcastEvent {
    Semantic(ReasonResp01SemanticEvent),
    Tool(ReasonReq04ToolCall),
    Usage(ReasonResp02UsageEvent),
    Terminal(ReasonResp03TerminalEvent),
    Error(ErrorErr01RuntimeClassified),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnProjection {
    pub turn_id: TurnId,
    pub user_text: String,
    pub terminal_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    pub request: ReasonReq02ContextComposedInput,
    pub provider_payload: ReasonReq03ProviderPayload,
    pub planned_context: PlannedContext,
    pub semantic_events: Vec<ReasonResp01SemanticEvent>,
    pub tool_calls: Vec<ReasonReq04ToolCall>,
    pub tool_results: Vec<ReasonReq05ToolResultReentry>,
    pub usage_events: Vec<ReasonResp02UsageEvent>,
    pub terminal_event: Option<ReasonResp03TerminalEvent>,
    pub error_events: Vec<ErrorErr01RuntimeClassified>,
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
            tool_schema_fingerprint: None,
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
        let turn = TurnRecord {
            request,
            provider_payload,
            planned_context,
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
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
                turn.tool_results.push(result);
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
            ProviderSemanticOutput::Terminal(_) => {
                // provider terminal is not final truth; wait for completion schema validation
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::apply_provider_output",
                    "provider terminal observed but not accepted",
                    vec!["terminal_waits_for_completion_schema=true".to_owned()],
                );
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

    pub fn submit_completion(
        &self,
        turn: &mut TurnRecord,
        submission: &CompletionSubmission,
    ) -> Result<ReasonResp03TerminalEvent, ReasonTurnError> {
        match validate_completion_submission(submission) {
            Ok(CompletionDecision::Completed {
                status,
                terminal_text,
            })
            | Ok(CompletionDecision::Blocked {
                status,
                terminal_text,
            }) => {
                let event = ReasonResp03TerminalEvent {
                    session_id: turn.request.session_id.clone(),
                    turn_id: turn.request.turn_id.clone(),
                    trace_id: turn.request.trace_id.clone(),
                    feature_id: turn.request.feature_id.clone(),
                    agent_id: turn.request.agent_id.clone(),
                    status,
                    summary: terminal_text,
                };
                turn.terminal_event = Some(event.clone());
                self.publish(ReasonBroadcastEvent::Terminal(event.clone()));
                self.emit_debug(
                    turn,
                    "ReasonTurnEngine::submit_completion",
                    "completion accepted",
                    vec![format!("terminal_status={:?}", event.status)],
                );
                Ok(event)
            }
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
        };
        turn.terminal_event = Some(event.clone());
        self.publish(ReasonBroadcastEvent::Terminal(event.clone()));
        self.emit_debug(
            turn,
            "ReasonTurnEngine::fail_turn",
            "turn failed",
            vec![event.summary.clone()],
        );
        event
    }

    pub fn cancel_turn(
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
            status: TerminalStatus::Cancelled,
            summary: summary.into(),
        };
        turn.terminal_event = Some(event.clone());
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
                        value: json!(event.usage.input_tokens),
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

fn unix_timestamp_string() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_owned(),
    }
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
    use freehand_debug::DebugHub;
    use freehand_provider_core::ProviderAdapterEvent;
    use serde_json::json;
    use std::sync::Arc;

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
    fn writes_tool_result_reentry_back_to_owning_turn() {
        let engine = ReasonTurnEngine::new();
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
                output: "done".to_owned(),
            },
        };
        engine
            .apply_provider_output(
                &mut turn,
                ProviderSemanticOutput::ToolResultReentry(result.clone()),
            )
            .expect("apply provider output");
        assert_eq!(turn.tool_results, vec![result]);
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
}
