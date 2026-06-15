//! Reasoning turn orchestration and event emission for Freehand.

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

use freehand_blocks::{
    CompletionDecision, CompletionSubmission, CompletionValidationError,
    validate_completion_submission,
};
use freehand_contracts::{
    AgentId, ErrorErr01RuntimeClassified, FeatureId, ReasonReq02ContextComposedInput,
    ReasonReq03ProviderPayload, ReasonReq04ToolCall, ReasonReq05ToolResultReentry,
    ReasonResp01SemanticEvent, ReasonResp02UsageEvent, ReasonResp03TerminalEvent,
    RequestContextItem, SessionId, TraceId, TurnId,
};
use freehand_provider_core::ProviderSemanticOutput;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnStartInput {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub user_text: String,
    pub context_items: Vec<RequestContextItem>,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReasonBroadcastEvent {
    Semantic(ReasonResp01SemanticEvent),
    Tool(ReasonReq04ToolCall),
    Usage(ReasonResp02UsageEvent),
    Terminal(ReasonResp03TerminalEvent),
    Error(ErrorErr01RuntimeClassified),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnProjection {
    pub turn_id: TurnId,
    pub user_text: String,
    pub terminal_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRecord {
    pub request: ReasonReq02ContextComposedInput,
    pub provider_payload: ReasonReq03ProviderPayload,
    pub semantic_events: Vec<ReasonResp01SemanticEvent>,
    pub tool_calls: Vec<ReasonReq04ToolCall>,
    pub tool_results: Vec<ReasonReq05ToolResultReentry>,
    pub usage_events: Vec<ReasonResp02UsageEvent>,
    pub terminal_event: Option<ReasonResp03TerminalEvent>,
    pub error_events: Vec<ErrorErr01RuntimeClassified>,
}

pub struct ReasonTurnEngine {
    subscribers: Mutex<Vec<SyncSender<ReasonBroadcastEvent>>>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReasonTurnError {
    #[error("turn input text must not be empty")]
    EmptyUserText,
    #[error("completion rejected: {0}")]
    CompletionRejected(String),
    #[error("completion requires next step: {0}")]
    CompletionRequiresNextStep(String),
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

    pub fn start_turn(&self, input: TurnStartInput) -> Result<TurnRecord, ReasonTurnError> {
        if input.user_text.trim().is_empty() {
            return Err(ReasonTurnError::EmptyUserText);
        }
        let request = ReasonReq02ContextComposedInput {
            session_id: input.session_id.clone(),
            turn_id: input.turn_id.clone(),
            trace_id: input.trace_id.clone(),
            feature_id: input.feature_id.clone(),
            agent_id: input.agent_id.clone(),
            user_text: input.user_text.clone(),
            context_items: input.context_items,
        };
        let provider_payload = ReasonReq03ProviderPayload {
            session_id: input.session_id,
            turn_id: input.turn_id,
            trace_id: input.trace_id,
            feature_id: input.feature_id,
            agent_id: input.agent_id,
            model: input.model,
            rendered_input: request.user_text.clone(),
        };
        Ok(TurnRecord {
            request,
            provider_payload,
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
        })
    }

    pub fn apply_provider_output(&self, turn: &mut TurnRecord, output: ProviderSemanticOutput) {
        match output {
            ProviderSemanticOutput::SemanticEvent(event) => {
                turn.semantic_events.push(event.clone());
                self.publish(ReasonBroadcastEvent::Semantic(event));
            }
            ProviderSemanticOutput::ToolCall(event) => {
                turn.tool_calls.push(event.clone());
                self.publish(ReasonBroadcastEvent::Tool(event));
            }
            ProviderSemanticOutput::ToolResultReentry(result) => {
                turn.tool_results.push(result);
            }
            ProviderSemanticOutput::Usage(event) => {
                turn.usage_events.push(event.clone());
                self.publish(ReasonBroadcastEvent::Usage(event));
            }
            ProviderSemanticOutput::Terminal(_) => {
                // provider terminal is not final truth; wait for completion schema validation
            }
            ProviderSemanticOutput::Error(event) => {
                turn.error_events.push(event.clone());
                self.publish(ReasonBroadcastEvent::Error(event));
            }
        }
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
                Ok(event)
            }
            Ok(CompletionDecision::ContinueWithNextStep { next_step }) => {
                Err(ReasonTurnError::CompletionRequiresNextStep(next_step))
            }
            Err(err) => Err(ReasonTurnError::CompletionRejected(
                completion_error_message(err),
            )),
        }
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
        TerminalStatus, TokenUsage, ToolArgument, ToolCallContract, ToolCallId,
    };
    use freehand_provider_core::ProviderAdapterEvent;
    use serde_json::json;

    fn start_input() -> TurnStartInput {
        TurnStartInput {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.turn"),
            agent_id: AgentId::new("agent-1"),
            user_text: "hello".to_owned(),
            context_items: vec![RequestContextItem {
                source: "memory".to_owned(),
                content: "ctx".to_owned(),
            }],
            model: "gpt-test".to_owned(),
        }
    }

    #[test]
    fn projects_session_from_per_turn_truth() {
        let engine = ReasonTurnEngine::new();
        let turn = engine.start_turn(start_input()).expect("turn");
        let projected = engine.project_session(&[turn]);
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].user_text, "hello");
        assert_eq!(projected[0].terminal_summary, None);
    }

    #[test]
    fn writes_tool_result_reentry_back_to_owning_turn() {
        let engine = ReasonTurnEngine::new();
        let mut turn = engine.start_turn(start_input()).expect("turn");
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
        engine.apply_provider_output(
            &mut turn,
            ProviderSemanticOutput::ToolResultReentry(result.clone()),
        );
        assert_eq!(turn.tool_results, vec![result]);
    }

    #[test]
    fn accepts_valid_completion_schema_and_emits_terminal() {
        let engine = ReasonTurnEngine::new();
        let receiver = engine.subscribe(4);
        let mut turn = engine.start_turn(start_input()).expect("turn");
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
        let mut turn = engine.start_turn(start_input()).expect("turn");
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
    fn slow_subscriber_does_not_block_main_path() {
        let engine = ReasonTurnEngine::new();
        let _receiver = engine.subscribe(1);
        let mut turn = engine.start_turn(start_input()).expect("turn");
        let ctx = freehand_provider_core::ProviderEventContext {
            agent_id: turn.request.agent_id.clone(),
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
        };
        engine.apply_provider_output(
            &mut turn,
            freehand_provider_core::map_adapter_event(
                &ctx,
                ProviderAdapterEvent::ReasoningDelta("step-1".to_owned()),
            ),
        );
        engine.apply_provider_output(
            &mut turn,
            freehand_provider_core::map_adapter_event(
                &ctx,
                ProviderAdapterEvent::TextDelta("step-2".to_owned()),
            ),
        );
        assert_eq!(turn.semantic_events.len(), 2);
    }

    #[test]
    fn broadcasts_semantic_and_usage_events() {
        let engine = ReasonTurnEngine::new();
        let receiver = engine.subscribe(4);
        let mut turn = engine.start_turn(start_input()).expect("turn");
        let ctx = freehand_provider_core::ProviderEventContext {
            agent_id: turn.request.agent_id.clone(),
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
        };
        engine.apply_provider_output(
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
        );
        engine.apply_provider_output(
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
        );

        let first = receiver.recv().expect("first");
        let second = receiver.recv().expect("second");
        assert!(matches!(first, ReasonBroadcastEvent::Tool(_)));
        assert!(matches!(second, ReasonBroadcastEvent::Usage(_)));
    }
}
