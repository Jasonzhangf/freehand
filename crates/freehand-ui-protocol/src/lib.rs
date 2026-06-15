//! UI-facing commands, events, and projections for Freehand.

use std::collections::BTreeMap;

use freehand_contracts::{
    AgentId, ErrorErr01RuntimeClassified, ReasonReq04ToolCall, ReasonResp01SemanticEvent,
    ReasonResp02UsageEvent, ReasonResp03TerminalEvent, SemanticEventKind, SessionId, TurnId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiClientKind {
    Cli,
    WebUi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiStreamKind {
    Turn,
    Progress,
    NodeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSource {
    pub source_agent_id: AgentId,
    pub source_node_id: String,
    pub source_turn_id: Option<TurnId>,
    pub stream_kind: UiStreamKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiCommand {
    SubmitUserInput {
        text: String,
    },
    SubscribeLatestActiveTurn {
        client: UiClientKind,
    },
    SubscribeTurn {
        client: UiClientKind,
        turn_id: TurnId,
    },
    SubscribeNodeStatus,
    SubscribeProgress,
    QueryLatestActiveTurn,
    QueryTurn {
        turn_id: TurnId,
    },
    QueryNodeStatus {
        node_id: String,
    },
    QueryTaskProgress {
        turn_id: TurnId,
    },
    SendDirectMessageToSlave {
        node_id: String,
        text: String,
    },
    CancelTurn {
        turn_id: TurnId,
    },
    ResumeTurn {
        turn_id: TurnId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTurnProjection {
    pub source: UiSource,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub reasoning: Vec<String>,
    pub text: Vec<String>,
    pub tool_calls: Vec<String>,
    pub usage: Vec<String>,
    pub terminal_text: Option<String>,
    pub errors: Vec<String>,
    pub slave_substream_card: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeStatusSnapshot {
    pub source: UiSource,
    pub node_id: String,
    pub healthy: bool,
    pub pairing_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProgressSnapshot {
    pub source: UiSource,
    pub turn_id: TurnId,
    pub status_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiProjection {
    Turn(UiTurnProjection),
    NodeStatus(NodeStatusSnapshot),
    Progress(TaskProgressSnapshot),
}

#[derive(Debug, Clone)]
pub struct TurnProjectionInput {
    pub source_agent_id: AgentId,
    pub source_node_id: String,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub semantic_events: Vec<ReasonResp01SemanticEvent>,
    pub tool_calls: Vec<ReasonReq04ToolCall>,
    pub usage_events: Vec<ReasonResp02UsageEvent>,
    pub terminal_event: Option<ReasonResp03TerminalEvent>,
    pub error_events: Vec<ErrorErr01RuntimeClassified>,
    pub slave_substream_card: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiQueryResult {
    Turn(Option<UiTurnProjection>),
    NodeStatus(Option<NodeStatusSnapshot>),
    Progress(Option<TaskProgressSnapshot>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionSelector {
    pub client: UiClientKind,
    pub stream_kind: UiStreamKind,
    pub target_turn_id: Option<TurnId>,
}

#[derive(Debug, Default)]
pub struct UiProtocolState {
    latest_active_turn_id: Option<TurnId>,
    turns: BTreeMap<TurnId, UiTurnProjection>,
    node_status: BTreeMap<String, NodeStatusSnapshot>,
    progress: BTreeMap<TurnId, TaskProgressSnapshot>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UiProtocolError {
    #[error("submit user input command requires non-empty text")]
    EmptyUserInput,
    #[error("direct slave message requires non-empty text")]
    EmptySlaveMessage,
    #[error("stream kind mismatch for requested projection")]
    StreamKindMismatch,
}

impl UiProtocolState {
    pub fn apply_turn_projection(&mut self, projection: UiTurnProjection) {
        self.latest_active_turn_id = Some(projection.turn_id.clone());
        self.turns.insert(projection.turn_id.clone(), projection);
    }

    pub fn set_node_status(&mut self, snapshot: NodeStatusSnapshot) {
        self.node_status.insert(snapshot.node_id.clone(), snapshot);
    }

    pub fn set_progress(&mut self, snapshot: TaskProgressSnapshot) {
        self.progress.insert(snapshot.turn_id.clone(), snapshot);
    }

    pub fn query(&self, command: &UiCommand) -> Result<UiQueryResult, UiProtocolError> {
        match command {
            UiCommand::QueryLatestActiveTurn => {
                let result = self
                    .latest_active_turn_id
                    .as_ref()
                    .and_then(|turn_id| self.turns.get(turn_id).cloned());
                Ok(UiQueryResult::Turn(result))
            }
            UiCommand::QueryTurn { turn_id } => {
                Ok(UiQueryResult::Turn(self.turns.get(turn_id).cloned()))
            }
            UiCommand::QueryNodeStatus { node_id } => Ok(UiQueryResult::NodeStatus(
                self.node_status.get(node_id).cloned(),
            )),
            UiCommand::QueryTaskProgress { turn_id } => {
                Ok(UiQueryResult::Progress(self.progress.get(turn_id).cloned()))
            }
            _ => Err(UiProtocolError::StreamKindMismatch),
        }
    }
}

pub fn validate_command(command: &UiCommand) -> Result<(), UiProtocolError> {
    match command {
        UiCommand::SubmitUserInput { text } if text.trim().is_empty() => {
            Err(UiProtocolError::EmptyUserInput)
        }
        UiCommand::SendDirectMessageToSlave { text, .. } if text.trim().is_empty() => {
            Err(UiProtocolError::EmptySlaveMessage)
        }
        _ => Ok(()),
    }
}

pub fn subscription_selector(command: &UiCommand) -> Option<SubscriptionSelector> {
    match command {
        UiCommand::SubscribeLatestActiveTurn { client } => Some(SubscriptionSelector {
            client: *client,
            stream_kind: UiStreamKind::Turn,
            target_turn_id: None,
        }),
        UiCommand::SubscribeTurn { client, turn_id } => Some(SubscriptionSelector {
            client: *client,
            stream_kind: UiStreamKind::Turn,
            target_turn_id: Some(turn_id.clone()),
        }),
        UiCommand::SubscribeNodeStatus => Some(SubscriptionSelector {
            client: UiClientKind::WebUi,
            stream_kind: UiStreamKind::NodeStatus,
            target_turn_id: None,
        }),
        UiCommand::SubscribeProgress => Some(SubscriptionSelector {
            client: UiClientKind::WebUi,
            stream_kind: UiStreamKind::Progress,
            target_turn_id: None,
        }),
        _ => None,
    }
}

pub fn subscription_matches(
    selector: &SubscriptionSelector,
    projection: &UiProjection,
    latest_active_turn_id: Option<&TurnId>,
) -> bool {
    match (selector.stream_kind, projection) {
        (UiStreamKind::Turn, UiProjection::Turn(turn)) => match selector.target_turn_id.as_ref() {
            Some(target) => target == &turn.turn_id,
            None => latest_active_turn_id == Some(&turn.turn_id),
        },
        (UiStreamKind::Progress, UiProjection::Progress(_)) => true,
        (UiStreamKind::NodeStatus, UiProjection::NodeStatus(_)) => true,
        _ => false,
    }
}

pub fn terminal_text_projection(event: &ReasonResp03TerminalEvent) -> String {
    event.summary.clone()
}

pub fn turn_projection_from_events(input: TurnProjectionInput) -> UiTurnProjection {
    let mut reasoning = Vec::new();
    let mut text = Vec::new();
    for event in &input.semantic_events {
        match event.kind {
            SemanticEventKind::Reasoning => reasoning.push(event.content.clone()),
            SemanticEventKind::Text => text.push(event.content.clone()),
            _ => {}
        }
    }
    UiTurnProjection {
        source: UiSource {
            source_agent_id: input.source_agent_id,
            source_node_id: input.source_node_id,
            source_turn_id: Some(input.turn_id.clone()),
            stream_kind: UiStreamKind::Turn,
        },
        session_id: input.session_id,
        turn_id: input.turn_id,
        reasoning,
        text,
        tool_calls: input
            .tool_calls
            .iter()
            .map(|call| call.tool_call.tool_name.clone())
            .collect(),
        usage: input
            .usage_events
            .iter()
            .map(|usage| {
                format!(
                    "input={} output={}",
                    usage.usage.input_tokens, usage.usage.output_tokens
                )
            })
            .collect(),
        terminal_text: input.terminal_event.as_ref().map(terminal_text_projection),
        errors: input
            .error_events
            .iter()
            .map(|error| error.error.message.clone())
            .collect(),
        slave_substream_card: input.slave_substream_card,
    }
}

pub fn turn_projection_for_client(
    projection: UiTurnProjection,
    client: UiClientKind,
) -> UiTurnProjection {
    if client == UiClientKind::Cli && projection.slave_substream_card {
        UiTurnProjection {
            slave_substream_card: false,
            ..projection
        }
    } else {
        projection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        ErrorClass, ErrorContract, FeatureId, RecoveryPolicy, TerminalStatus, TraceId,
    };

    fn base_source(stream_kind: UiStreamKind) -> UiSource {
        UiSource {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            source_turn_id: Some(TurnId::new("turn-1")),
            stream_kind,
        }
    }

    fn sample_turn_projection(slave_substream_card: bool) -> UiTurnProjection {
        turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            semantic_events: vec![
                ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    trace_id: TraceId::new("trace-1"),
                    feature_id: FeatureId::new("ui.protocol"),
                    agent_id: AgentId::new("agent-1"),
                    kind: SemanticEventKind::Reasoning,
                    content: "thinking".to_owned(),
                },
                ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    trace_id: TraceId::new("trace-1"),
                    feature_id: FeatureId::new("ui.protocol"),
                    agent_id: AgentId::new("agent-1"),
                    kind: SemanticEventKind::Text,
                    content: "answer".to_owned(),
                },
            ],
            tool_calls: vec![ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_call: freehand_contracts::ToolCallContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    tool_name: "search".to_owned(),
                    arguments: vec![],
                    arguments_complete: true,
                },
            }],
            usage_events: vec![ReasonResp02UsageEvent {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                usage: freehand_contracts::TokenUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: Some(15),
                    reasoning_tokens: Some(3),
                    cache_creation_tokens: 0,
                    cache_read_tokens: 0,
                    finish_reason: Some("stop".to_owned()),
                },
            }],
            terminal_event: Some(ReasonResp03TerminalEvent {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                status: TerminalStatus::Success,
                summary: "final text".to_owned(),
            }),
            error_events: vec![ErrorErr01RuntimeClassified {
                session_id: Some(SessionId::new("session-1")),
                turn_id: Some(TurnId::new("turn-1")),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: Some(AgentId::new("agent-1")),
                error: ErrorContract {
                    code: "warn".to_owned(),
                    class: ErrorClass::Protocol,
                    recovery: RecoveryPolicy::Recoverable,
                    message: "minor".to_owned(),
                },
            }],
            slave_substream_card,
        })
    }

    #[test]
    fn command_to_projection_smoke() {
        validate_command(&UiCommand::SubmitUserInput {
            text: "hello".to_owned(),
        })
        .expect("valid");

        let projection = sample_turn_projection(false);
        assert_eq!(projection.reasoning, vec!["thinking"]);
        assert_eq!(projection.text, vec!["answer"]);
    }

    #[test]
    fn slave_turn_subscription_smoke() {
        let projection = sample_turn_projection(true);
        let selector = subscription_selector(&UiCommand::SubscribeTurn {
            client: UiClientKind::WebUi,
            turn_id: TurnId::new("turn-1"),
        })
        .expect("selector");
        let event = UiProjection::Turn(projection.clone());
        assert!(subscription_matches(
            &selector,
            &event,
            Some(&TurnId::new("turn-1"))
        ));
        let cli_projection = turn_projection_for_client(projection, UiClientKind::Cli);
        assert!(!cli_projection.slave_substream_card);
    }

    #[test]
    fn node_status_query_smoke() {
        let mut state = UiProtocolState::default();
        state.set_node_status(NodeStatusSnapshot {
            source: base_source(UiStreamKind::NodeStatus),
            node_id: "node-1".to_owned(),
            healthy: true,
            pairing_state: "paired".to_owned(),
        });
        let result = state
            .query(&UiCommand::QueryNodeStatus {
                node_id: "node-1".to_owned(),
            })
            .expect("query");
        match result {
            UiQueryResult::NodeStatus(Some(snapshot)) => {
                assert!(snapshot.healthy);
                assert_eq!(snapshot.pairing_state, "paired");
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }

    #[test]
    fn terminal_result_projection_smoke() {
        let event = ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("ui.protocol"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "only final text".to_owned(),
        };
        assert_eq!(terminal_text_projection(&event), "only final text");
    }

    #[test]
    fn latest_active_turn_and_stream_kind_routing() {
        let mut state = UiProtocolState::default();
        let projection = sample_turn_projection(false);
        state.apply_turn_projection(projection.clone());
        let result = state
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match result {
            UiQueryResult::Turn(Some(snapshot)) => assert_eq!(snapshot.turn_id, projection.turn_id),
            other => panic!("unexpected query result: {other:?}"),
        }

        let selector = subscription_selector(&UiCommand::SubscribeLatestActiveTurn {
            client: UiClientKind::Cli,
        })
        .expect("selector");
        assert!(subscription_matches(
            &selector,
            &UiProjection::Turn(projection),
            state.latest_active_turn_id.as_ref()
        ));
    }
}
