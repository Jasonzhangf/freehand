//! UI-facing commands, events, and projections for Freehand.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, TryRecvError};

use freehand_blocks::strip_completion_submission_block;
use freehand_contracts::{
    AgentId, ErrorErr01RuntimeClassified, ReasonReq04ToolCall, ReasonResp01SemanticEvent,
    ReasonResp02UsageEvent, ReasonResp03TerminalEvent, SemanticEventKind, SessionId,
    TerminalStatus, TurnId,
};
pub use freehand_debug::{
    DebugEvent, DebugScenePosition, DebugSemanticPosition, DebugStateSnapshot, DebugTraceEnvelope,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;

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
    Debug,
    Checkpoint,
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
    SubscribeDebugState {
        client: UiClientKind,
        turn_id: TurnId,
    },
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
    QueryDebugState {
        turn_id: TurnId,
    },
    QueryCheckpoints,
    SendDirectMessageToSlave {
        node_id: String,
        text: String,
    },
    RewindCheckpoint {
        checkpoint_id: String,
    },
    CancelTurn {
        turn_id: TurnId,
    },
    CancelLatestActiveTurn {},
    ResumeTurn {
        turn_id: TurnId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTurnProjection {
    pub source: UiSource,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub user_text: Option<String>,
    pub reasoning: Vec<String>,
    pub text: Vec<String>,
    pub tool_calls: Vec<String>,
    pub usage: Vec<String>,
    pub terminal_status: Option<TerminalStatus>,
    pub terminal_text: Option<String>,
    pub errors: Vec<String>,
    pub slave_substream_card: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiConversationItemKind {
    UserText,
    AssistantText,
    ToolSummary,
    Terminal,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConversationItem {
    pub kind: UiConversationItemKind,
    pub title: String,
    pub body: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiPublicTurnProjection {
    pub turn: UiTurnProjection,
    pub public_conversation: Vec<UiConversationItem>,
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
pub struct UiCheckpointSummary {
    pub checkpoint_id: String,
    pub agent_id: AgentId,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub tool_call_id: String,
    pub changed_paths: Vec<String>,
    pub latest_status: String,
    pub latest_detail: Option<String>,
    pub updated_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCheckpointSnapshot {
    pub source: UiSource,
    pub checkpoints: Vec<UiCheckpointSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiProjection {
    Turn(UiTurnProjection),
    NodeStatus(NodeStatusSnapshot),
    Progress(TaskProgressSnapshot),
    Debug(DebugStateSnapshot),
    Checkpoints(UiCheckpointSnapshot),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSubscriptionEvent {
    pub projection: UiProjection,
    pub latest_active_turn_id: Option<TurnId>,
}

#[derive(Debug, Clone)]
pub struct TurnProjectionInput {
    pub source_agent_id: AgentId,
    pub source_node_id: String,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub user_text: Option<String>,
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
    Debug(Option<DebugStateSnapshot>),
    Checkpoints(UiCheckpointSnapshot),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCommandIngressAck {
    pub command_kind: String,
    pub accepted: bool,
    pub status_text: String,
    pub mutation_authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiProtocolRejection {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCommandDispatchEnvelope {
    pub ingress: UiCommandIngressAck,
    pub command: UiCommand,
    pub target_feature_id: String,
    pub target_owner_module: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCommandDispatchReceipt {
    pub ingress: UiCommandIngressAck,
    pub target_feature_id: String,
    pub target_owner_module: String,
    pub dispatch_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCommandDispatchFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

pub trait UiCommandDispatchPort: Send + Sync {
    fn dispatch(
        &self,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError>;
}

#[derive(Debug, Clone)]
pub struct StaticUiCommandDispatchPort {
    dispatch_status: String,
}

impl Default for StaticUiCommandDispatchPort {
    fn default() -> Self {
        Self {
            dispatch_status: "queued_by_static_dispatch_port".to_owned(),
        }
    }
}

impl StaticUiCommandDispatchPort {
    pub fn new(dispatch_status: impl Into<String>) -> Self {
        Self {
            dispatch_status: dispatch_status.into(),
        }
    }
}

impl UiCommandDispatchPort for StaticUiCommandDispatchPort {
    fn dispatch(
        &self,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: self.dispatch_status.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionSelector {
    pub client: UiClientKind,
    pub stream_kind: UiStreamKind,
    pub target_turn_id: Option<TurnId>,
}

#[derive(Debug)]
pub struct UiProtocolState {
    latest_active_turn_id: Option<TurnId>,
    turns: BTreeMap<TurnId, UiTurnProjection>,
    node_status: BTreeMap<String, NodeStatusSnapshot>,
    progress: BTreeMap<TurnId, TaskProgressSnapshot>,
    debug: BTreeMap<TurnId, DebugStateSnapshot>,
    checkpoints: Option<UiCheckpointSnapshot>,
    subscription_tx: broadcast::Sender<UiSubscriptionEvent>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UiProtocolError {
    #[error("submit user input command requires non-empty text")]
    EmptyUserInput,
    #[error("direct slave message requires non-empty text")]
    EmptySlaveMessage,
    #[error("rewind checkpoint command requires non-empty checkpoint id")]
    EmptyCheckpointId,
    #[error("command ingress route only accepts mutation-intent commands")]
    IngressCommandKindMismatch,
    #[error("stream kind mismatch for requested projection")]
    StreamKindMismatch,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UiCommandDispatchPortError {
    #[error("dispatch port failure: {0}")]
    DispatchFailed(String),
    #[error("dispatch target not found: {0}")]
    TargetNotFound(String),
    #[error("dispatch path unsupported: {0}")]
    Unsupported(String),
}

impl UiProtocolState {
    pub fn new() -> Self {
        let (subscription_tx, _subscription_rx) = broadcast::channel(256);
        Self {
            latest_active_turn_id: None,
            turns: BTreeMap::new(),
            node_status: BTreeMap::new(),
            progress: BTreeMap::new(),
            debug: BTreeMap::new(),
            checkpoints: None,
            subscription_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<UiSubscriptionEvent> {
        self.subscription_tx.subscribe()
    }

    pub fn apply_turn_projection(&mut self, projection: UiTurnProjection) {
        self.latest_active_turn_id = Some(projection.turn_id.clone());
        self.turns
            .insert(projection.turn_id.clone(), projection.clone());
        self.publish_projection(UiProjection::Turn(projection));
    }

    pub fn apply_semantic_event(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonResp01SemanticEvent,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &event.session_id,
                &event.turn_id,
                slave_substream_card,
            );
            match event.kind {
                SemanticEventKind::Reasoning => projection.reasoning.push(event.content.clone()),
                SemanticEventKind::Text => projection.text.push(event.content.clone()),
                _ => {}
            }
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_tool_call(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonReq04ToolCall,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &event.session_id,
                &event.turn_id,
                slave_substream_card,
            );
            projection
                .tool_calls
                .push(event.tool_call.tool_name.clone());
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_usage_event(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonResp02UsageEvent,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &event.session_id,
                &event.turn_id,
                slave_substream_card,
            );
            projection.usage.push(format!(
                "input={} output={}",
                event.usage.input_tokens, event.usage.output_tokens
            ));
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_terminal_event(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonResp03TerminalEvent,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &event.session_id,
                &event.turn_id,
                slave_substream_card,
            );
            projection.terminal_status = Some(event.status.clone());
            projection.terminal_text = Some(terminal_text_projection(event));
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_error_event(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ErrorErr01RuntimeClassified,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let session_id = event
            .session_id
            .clone()
            .expect("ui turn error projection requires session_id");
        let turn_id = event
            .turn_id
            .clone()
            .expect("ui turn error projection requires turn_id");
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &session_id,
                &turn_id,
                slave_substream_card,
            );
            projection.errors.push(event.error.message.clone());
            projection.clone()
        };
        self.latest_active_turn_id = Some(turn_id);
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn set_node_status(&mut self, snapshot: NodeStatusSnapshot) {
        self.node_status
            .insert(snapshot.node_id.clone(), snapshot.clone());
        self.publish_projection(UiProjection::NodeStatus(snapshot));
    }

    pub fn set_progress(&mut self, snapshot: TaskProgressSnapshot) {
        self.progress
            .insert(snapshot.turn_id.clone(), snapshot.clone());
        self.publish_projection(UiProjection::Progress(snapshot));
    }

    pub fn set_debug_state(&mut self, snapshot: DebugStateSnapshot) {
        self.debug
            .insert(snapshot.semantic.turn_id.clone(), snapshot.clone());
        self.publish_projection(UiProjection::Debug(snapshot));
    }

    pub fn set_checkpoint_snapshot(&mut self, snapshot: UiCheckpointSnapshot) {
        self.checkpoints = Some(snapshot.clone());
        self.publish_projection(UiProjection::Checkpoints(snapshot));
    }

    pub fn apply_debug_event(&mut self, event: &DebugEvent) -> bool {
        let Some(snapshot) = event.snapshot.clone() else {
            return false;
        };
        self.set_debug_state(snapshot);
        true
    }

    pub fn drain_debug_receiver(&mut self, receiver: &Receiver<DebugEvent>) -> usize {
        let mut applied = 0;
        loop {
            match receiver.try_recv() {
                Ok(event) => {
                    if self.apply_debug_event(&event) {
                        applied += 1;
                    }
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return applied,
            }
        }
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
            UiCommand::QueryDebugState { turn_id } => {
                Ok(UiQueryResult::Debug(self.debug.get(turn_id).cloned()))
            }
            UiCommand::QueryCheckpoints => Ok(UiQueryResult::Checkpoints(
                self.checkpoints
                    .clone()
                    .unwrap_or_else(empty_checkpoint_snapshot),
            )),
            _ => Err(UiProtocolError::StreamKindMismatch),
        }
    }

    fn ensure_turn_projection(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        session_id: &SessionId,
        turn_id: &TurnId,
        slave_substream_card: bool,
    ) -> &mut UiTurnProjection {
        self.turns
            .entry(turn_id.clone())
            .or_insert_with(|| UiTurnProjection {
                source: UiSource {
                    source_agent_id,
                    source_node_id,
                    source_turn_id: Some(turn_id.clone()),
                    stream_kind: UiStreamKind::Turn,
                },
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                user_text: None,
                reasoning: Vec::new(),
                text: Vec::new(),
                tool_calls: Vec::new(),
                usage: Vec::new(),
                terminal_status: None,
                terminal_text: None,
                errors: Vec::new(),
                slave_substream_card,
            })
    }

    fn publish_projection(&self, projection: UiProjection) {
        let _ = self.subscription_tx.send(UiSubscriptionEvent {
            projection,
            latest_active_turn_id: self.latest_active_turn_id.clone(),
        });
    }
}

impl Default for UiProtocolState {
    fn default() -> Self {
        Self::new()
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
        UiCommand::RewindCheckpoint { checkpoint_id } if checkpoint_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyCheckpointId)
        }
        _ => Ok(()),
    }
}

pub fn accept_command_ingress(command: &UiCommand) -> Result<UiCommandIngressAck, UiProtocolError> {
    validate_command(command)?;
    if !is_command_ingress_kind(command) {
        return Err(UiProtocolError::IngressCommandKindMismatch);
    }
    Ok(UiCommandIngressAck {
        command_kind: command_kind(command).to_owned(),
        accepted: true,
        status_text: "command accepted for owner-module handling".to_owned(),
        mutation_authority: "owner_modules".to_owned(),
    })
}

pub fn protocol_rejection(err: UiProtocolError) -> UiProtocolRejection {
    let code = match err {
        UiProtocolError::EmptyUserInput => "empty_user_input",
        UiProtocolError::EmptySlaveMessage => "empty_slave_message",
        UiProtocolError::EmptyCheckpointId => "empty_checkpoint_id",
        UiProtocolError::IngressCommandKindMismatch => "ingress_command_kind_mismatch",
        UiProtocolError::StreamKindMismatch => "stream_kind_mismatch",
    };
    UiProtocolRejection {
        code: code.to_owned(),
        message: err.to_string(),
    }
}

pub fn build_command_dispatch_envelope(
    command: &UiCommand,
) -> Result<UiCommandDispatchEnvelope, UiProtocolError> {
    let ingress = accept_command_ingress(command)?;
    let (target_feature_id, target_owner_module) = command_dispatch_target(command);
    Ok(UiCommandDispatchEnvelope {
        ingress,
        command: command.clone(),
        target_feature_id: target_feature_id.to_owned(),
        target_owner_module: target_owner_module.to_owned(),
    })
}

pub fn dispatch_port_failure(err: UiCommandDispatchPortError) -> UiCommandDispatchFailure {
    match err {
        UiCommandDispatchPortError::DispatchFailed(message) => UiCommandDispatchFailure {
            code: "command_dispatch_port_failure".to_owned(),
            message: format!("dispatch port failure: {message}"),
            retryable: true,
        },
        UiCommandDispatchPortError::TargetNotFound(message) => UiCommandDispatchFailure {
            code: "command_dispatch_target_not_found".to_owned(),
            message: format!("dispatch target not found: {message}"),
            retryable: false,
        },
        UiCommandDispatchPortError::Unsupported(message) => UiCommandDispatchFailure {
            code: "command_dispatch_unsupported".to_owned(),
            message: format!("dispatch path unsupported: {message}"),
            retryable: false,
        },
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
        UiCommand::SubscribeDebugState { client, turn_id } => Some(SubscriptionSelector {
            client: *client,
            stream_kind: UiStreamKind::Debug,
            target_turn_id: Some(turn_id.clone()),
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
        (UiStreamKind::Debug, UiProjection::Debug(debug)) => {
            selector.target_turn_id.as_ref() == Some(&debug.semantic.turn_id)
        }
        (UiStreamKind::Checkpoint, UiProjection::Checkpoints(_)) => true,
        _ => false,
    }
}

pub fn terminal_text_projection(event: &ReasonResp03TerminalEvent) -> String {
    event.summary.clone()
}

pub fn public_conversation_items(projection: &UiTurnProjection) -> Vec<UiConversationItem> {
    let mut items = Vec::new();
    if let Some(user_text) = &projection.user_text
        && !user_text.trim().is_empty()
    {
        items.push(UiConversationItem {
            kind: UiConversationItemKind::UserText,
            title: "User".to_owned(),
            body: user_text.clone(),
            status: "submitted".to_owned(),
        });
    }
    for text in &projection.text {
        let public_text = strip_completion_submission_block(text);
        if !public_text.trim().is_empty() {
            items.push(UiConversationItem {
                kind: UiConversationItemKind::AssistantText,
                title: "Assistant".to_owned(),
                body: public_text,
                status: "streaming".to_owned(),
            });
        }
    }
    for tool_name in &projection.tool_calls {
        items.push(UiConversationItem {
            kind: UiConversationItemKind::ToolSummary,
            title: "Tool".to_owned(),
            body: format!("Tool call requested: {tool_name}"),
            status: "running".to_owned(),
        });
    }
    if let Some(terminal_text) = &projection.terminal_text {
        let public_text = strip_completion_submission_block(terminal_text);
        if !public_text.trim().is_empty() {
            let status = match projection.terminal_status {
                Some(TerminalStatus::Cancelled) => "cancelled",
                Some(TerminalStatus::Failed) => "failed",
                Some(TerminalStatus::Blocked) => "blocked",
                Some(TerminalStatus::Interrupted) => "interrupted",
                Some(TerminalStatus::ToolPending) => "running",
                Some(TerminalStatus::Success) | None => "completed",
            };
            items.push(UiConversationItem {
                kind: UiConversationItemKind::Terminal,
                title: "Final".to_owned(),
                body: public_text,
                status: status.to_owned(),
            });
        }
    }
    for error in &projection.errors {
        items.push(UiConversationItem {
            kind: UiConversationItemKind::Error,
            title: "Error".to_owned(),
            body: error.clone(),
            status: "failed".to_owned(),
        });
    }
    items
}

pub fn public_turn_projection(projection: UiTurnProjection) -> UiPublicTurnProjection {
    let public_conversation = public_conversation_items(&projection);
    UiPublicTurnProjection {
        turn: projection,
        public_conversation,
    }
}

pub fn checkpoint_projection_from_runtime_summary(
    source_agent_id: AgentId,
    source_node_id: String,
    summaries: Vec<UiCheckpointSummary>,
) -> UiCheckpointSnapshot {
    UiCheckpointSnapshot {
        source: UiSource {
            source_agent_id,
            source_node_id,
            source_turn_id: None,
            stream_kind: UiStreamKind::Checkpoint,
        },
        checkpoints: summaries,
    }
}

pub fn debug_projection_from_event(event: &DebugEvent) -> Option<UiProjection> {
    event.snapshot.clone().map(UiProjection::Debug)
}

fn empty_checkpoint_snapshot() -> UiCheckpointSnapshot {
    UiCheckpointSnapshot {
        source: UiSource {
            source_agent_id: AgentId::new("unknown"),
            source_node_id: "unknown".to_owned(),
            source_turn_id: None,
            stream_kind: UiStreamKind::Checkpoint,
        },
        checkpoints: Vec::new(),
    }
}

fn command_kind(command: &UiCommand) -> &'static str {
    match command {
        UiCommand::SubmitUserInput { .. } => "submit_user_input",
        UiCommand::SubscribeLatestActiveTurn { .. } => "subscribe_latest_active_turn",
        UiCommand::SubscribeTurn { .. } => "subscribe_turn",
        UiCommand::SubscribeNodeStatus => "subscribe_node_status",
        UiCommand::SubscribeProgress => "subscribe_progress",
        UiCommand::SubscribeDebugState { .. } => "subscribe_debug_state",
        UiCommand::QueryLatestActiveTurn => "query_latest_active_turn",
        UiCommand::QueryTurn { .. } => "query_turn",
        UiCommand::QueryNodeStatus { .. } => "query_node_status",
        UiCommand::QueryTaskProgress { .. } => "query_task_progress",
        UiCommand::QueryDebugState { .. } => "query_debug_state",
        UiCommand::QueryCheckpoints => "query_checkpoints",
        UiCommand::SendDirectMessageToSlave { .. } => "send_direct_message_to_slave",
        UiCommand::RewindCheckpoint { .. } => "rewind_checkpoint",
        UiCommand::CancelTurn { .. } => "cancel_turn",
        UiCommand::CancelLatestActiveTurn { .. } => "cancel_latest_active_turn",
        UiCommand::ResumeTurn { .. } => "resume_turn",
    }
}

fn is_command_ingress_kind(command: &UiCommand) -> bool {
    matches!(
        command,
        UiCommand::SubmitUserInput { .. }
            | UiCommand::SendDirectMessageToSlave { .. }
            | UiCommand::RewindCheckpoint { .. }
            | UiCommand::CancelTurn { .. }
            | UiCommand::CancelLatestActiveTurn { .. }
            | UiCommand::ResumeTurn { .. }
    )
}

fn command_dispatch_target(command: &UiCommand) -> (&'static str, &'static str) {
    match command {
        UiCommand::SubmitUserInput { .. }
        | UiCommand::CancelTurn { .. }
        | UiCommand::CancelLatestActiveTurn { .. }
        | UiCommand::ResumeTurn { .. } => ("reason.turn", "crates/freehand-reason"),
        UiCommand::RewindCheckpoint { .. } => {
            ("runtime.checkpoint-rewind", "crates/freehand-runtime")
        }
        UiCommand::SendDirectMessageToSlave { .. } => ("node.master-slave", "crates/freehand-node"),
        _ => ("ui.protocol", "crates/freehand-ui-protocol"),
    }
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
        user_text: input.user_text,
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
        terminal_status: input
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
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
    use freehand_debug::DebugHub;

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
            user_text: Some("run the task".to_owned()),
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

    fn sample_debug_snapshot() -> DebugStateSnapshot {
        DebugStateSnapshot::new(
            freehand_debug::DebugSemanticPosition {
                feature_id: FeatureId::new("ui.protocol"),
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                agent_id: Some(AgentId::new("agent-1")),
                pipeline_node: Some("UiDebugState".to_owned()),
            },
            freehand_debug::DebugScenePosition {
                crate_name: "freehand-ui-protocol".to_owned(),
                file: "src/lib.rs".to_owned(),
                function: "sample_debug_snapshot".to_owned(),
                line: None,
                artifact_path: None,
                raw_exchange_id: None,
            },
            "planner locked stable prefix",
            vec![
                "rewrite_mode=ordinary".to_owned(),
                "rewrite_version=0".to_owned(),
            ],
        )
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
    fn cancelled_terminal_status_projects_to_public_conversation() {
        let mut projection = sample_turn_projection(false);
        projection.terminal_status = Some(TerminalStatus::Cancelled);
        projection.terminal_text = Some("cancelled by ui command".to_owned());

        let items = public_conversation_items(&projection);
        let terminal = items
            .iter()
            .find(|item| item.kind == UiConversationItemKind::Terminal)
            .expect("terminal item");

        assert_eq!(terminal.status, "cancelled");
        assert_eq!(terminal.body, "cancelled by ui command");
    }

    #[test]
    fn public_conversation_projection_hides_internal_reasoning_usage_and_completion_schema() {
        let mut projection = sample_turn_projection(false);
        projection.text = vec![concat!(
            "Visible answer\n",
            "<freehand_completion>",
            "{\"claim\":\"complete\",\"completion_reason\":\"done\",\"evidence\":\"proof\",\"summary\":\"summary\",\"learned\":\"lesson\"}",
            "</freehand_completion>"
        )
        .to_owned()];
        projection.reasoning = vec!["private chain".to_owned()];
        projection.usage = vec!["input=10 output=5".to_owned()];

        let items = public_conversation_items(&projection);
        let rendered = items
            .iter()
            .map(|item| item.body.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(items[0].kind, UiConversationItemKind::UserText);
        assert_eq!(items[0].body, "run the task");
        assert!(rendered.contains("Visible answer"));
        assert!(rendered.contains("run the task"));
        assert!(!rendered.contains("freehand_completion"));
        assert!(!rendered.contains("private chain"));
        assert!(!rendered.contains("input=10"));

        let public_turn = public_turn_projection(projection);
        assert_eq!(public_turn.public_conversation, items);
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

    #[test]
    fn debug_state_query_and_subscription_smoke() {
        let mut state = UiProtocolState::default();
        let debug = sample_debug_snapshot();
        state.set_debug_state(debug.clone());

        let result = state
            .query(&UiCommand::QueryDebugState {
                turn_id: TurnId::new("turn-1"),
            })
            .expect("query");
        match result {
            UiQueryResult::Debug(Some(snapshot)) => {
                assert_eq!(snapshot.status_text, "planner locked stable prefix");
                assert_eq!(
                    snapshot.detail_lines,
                    vec!["rewrite_mode=ordinary", "rewrite_version=0"]
                );
            }
            other => panic!("unexpected query result: {other:?}"),
        }

        let selector = subscription_selector(&UiCommand::SubscribeDebugState {
            client: UiClientKind::Cli,
            turn_id: TurnId::new("turn-1"),
        })
        .expect("selector");
        assert!(subscription_matches(
            &selector,
            &UiProjection::Debug(debug),
            state.latest_active_turn_id.as_ref()
        ));
    }

    #[test]
    fn checkpoint_summary_query_smoke() {
        let mut state = UiProtocolState::default();
        let snapshot = checkpoint_projection_from_runtime_summary(
            AgentId::new("agent-1"),
            "node-1".to_owned(),
            vec![UiCheckpointSummary {
                checkpoint_id: "checkpoint-1".to_owned(),
                agent_id: AgentId::new("agent-1"),
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                tool_call_id: "tool-1".to_owned(),
                changed_paths: vec!["scratch/file.txt".to_owned()],
                latest_status: "restored".to_owned(),
                latest_detail: None,
                updated_unix_seconds: 42,
            }],
        );
        state.set_checkpoint_snapshot(snapshot.clone());

        let result = state
            .query(&UiCommand::QueryCheckpoints)
            .expect("checkpoint query");
        match result {
            UiQueryResult::Checkpoints(returned) => assert_eq!(returned, snapshot),
            other => panic!("unexpected checkpoint query result: {other:?}"),
        }
    }

    #[test]
    fn command_ingress_rejects_checkpoint_query_route_misuse() {
        let err = accept_command_ingress(&UiCommand::QueryCheckpoints).expect_err("must reject");
        assert_eq!(err, UiProtocolError::IngressCommandKindMismatch);
    }

    #[test]
    fn debug_subscription_rejects_other_turns() {
        let selector = subscription_selector(&UiCommand::SubscribeDebugState {
            client: UiClientKind::WebUi,
            turn_id: TurnId::new("turn-1"),
        })
        .expect("selector");
        let other = DebugStateSnapshot::new(
            freehand_debug::DebugSemanticPosition {
                turn_id: TurnId::new("turn-2"),
                ..sample_debug_snapshot().semantic
            },
            sample_debug_snapshot().scene,
            "planner locked stable prefix",
            vec![
                "rewrite_mode=ordinary".to_owned(),
                "rewrite_version=0".to_owned(),
            ],
        );
        assert!(!subscription_matches(
            &selector,
            &UiProjection::Debug(other),
            None
        ));
    }

    #[test]
    fn debug_receiver_drain_updates_queryable_state() {
        let hub = DebugHub::new(true);
        let receiver = hub.subscribe(4);
        let snapshot = sample_debug_snapshot();
        let event = DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic: snapshot.semantic.clone(),
                scene: snapshot.scene.clone(),
                input_hash: None,
                output_hash: None,
                artifact_path: None,
                timestamp: "2026-06-16T00:00:00Z".to_owned(),
            },
            snapshot: Some(snapshot),
        };
        hub.emit(event).expect("emit");

        let mut state = UiProtocolState::default();
        let applied = state.drain_debug_receiver(&receiver);
        assert_eq!(applied, 1);

        let result = state
            .query(&UiCommand::QueryDebugState {
                turn_id: TurnId::new("turn-1"),
            })
            .expect("query");
        match result {
            UiQueryResult::Debug(Some(snapshot)) => {
                assert_eq!(snapshot.status_text, "planner locked stable prefix");
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }

    #[test]
    fn debug_event_without_snapshot_does_not_update_state() {
        let snapshot = sample_debug_snapshot();
        let event = DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic: snapshot.semantic,
                scene: snapshot.scene,
                input_hash: None,
                output_hash: None,
                artifact_path: None,
                timestamp: "2026-06-16T00:00:00Z".to_owned(),
            },
            snapshot: None,
        };

        let mut state = UiProtocolState::default();
        assert!(!state.apply_debug_event(&event));
        let result = state
            .query(&UiCommand::QueryDebugState {
                turn_id: TurnId::new("turn-1"),
            })
            .expect("query");
        assert_eq!(result, UiQueryResult::Debug(None));
        assert!(debug_projection_from_event(&event).is_none());
    }

    #[test]
    fn command_ingress_accepts_mutation_intent_without_writing_truth() {
        let ack = accept_command_ingress(&UiCommand::SubmitUserInput {
            text: "ship it".to_owned(),
        })
        .expect("ack");
        assert!(ack.accepted);
        assert_eq!(ack.command_kind, "submit_user_input");
        assert_eq!(ack.mutation_authority, "owner_modules");
    }

    #[test]
    fn command_ingress_accepts_rewind_checkpoint() {
        let ack = accept_command_ingress(&UiCommand::RewindCheckpoint {
            checkpoint_id: "checkpoint-1".to_owned(),
        })
        .expect("ack");
        assert!(ack.accepted);
        assert_eq!(ack.command_kind, "rewind_checkpoint");
    }

    #[test]
    fn command_ingress_rejects_empty_checkpoint_id() {
        let err = accept_command_ingress(&UiCommand::RewindCheckpoint {
            checkpoint_id: "   ".to_owned(),
        })
        .expect_err("must reject");
        assert_eq!(err, UiProtocolError::EmptyCheckpointId);
        let rejection = protocol_rejection(err);
        assert_eq!(rejection.code, "empty_checkpoint_id");
    }

    #[test]
    fn command_ingress_rejects_query_commands() {
        let err =
            accept_command_ingress(&UiCommand::QueryLatestActiveTurn).expect_err("must reject");
        assert_eq!(err, UiProtocolError::IngressCommandKindMismatch);
        let rejection = protocol_rejection(err);
        assert_eq!(rejection.code, "ingress_command_kind_mismatch");
    }

    #[test]
    fn command_dispatch_envelope_routes_submit_input_to_reason_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
            text: "run task".to_owned(),
        })
        .expect("envelope");
        assert_eq!(envelope.ingress.command_kind, "submit_user_input");
        assert_eq!(envelope.target_feature_id, "reason.turn");
        assert_eq!(envelope.target_owner_module, "crates/freehand-reason");
    }

    #[test]
    fn command_dispatch_envelope_routes_slave_message_to_node_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::SendDirectMessageToSlave {
            node_id: "node-1".to_owned(),
            text: "ping".to_owned(),
        })
        .expect("envelope");
        assert_eq!(envelope.target_feature_id, "node.master-slave");
        assert_eq!(envelope.target_owner_module, "crates/freehand-node");
    }

    #[test]
    fn command_dispatch_envelope_routes_rewind_checkpoint_to_runtime_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::RewindCheckpoint {
            checkpoint_id: "checkpoint-1".to_owned(),
        })
        .expect("envelope");
        assert_eq!(envelope.ingress.command_kind, "rewind_checkpoint");
        assert_eq!(envelope.target_feature_id, "runtime.checkpoint-rewind");
        assert_eq!(envelope.target_owner_module, "crates/freehand-runtime");
    }

    #[test]
    fn static_dispatch_port_returns_dispatch_receipt() {
        let envelope = build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
            text: "run task".to_owned(),
        })
        .expect("envelope");
        let port = StaticUiCommandDispatchPort::new("queued_by_test_port");
        let receipt = port.dispatch(envelope).expect("receipt");
        assert_eq!(receipt.dispatch_status, "queued_by_test_port");
        assert_eq!(receipt.target_feature_id, "reason.turn");
    }

    #[test]
    fn dispatch_failure_mapping_preserves_retryability() {
        let not_found = dispatch_port_failure(UiCommandDispatchPortError::TargetNotFound(
            "turn-404".to_owned(),
        ));
        assert_eq!(not_found.code, "command_dispatch_target_not_found");
        assert!(!not_found.retryable);

        let unsupported =
            dispatch_port_failure(UiCommandDispatchPortError::Unsupported("resume".to_owned()));
        assert_eq!(unsupported.code, "command_dispatch_unsupported");
        assert!(!unsupported.retryable);
    }

    #[test]
    fn state_subscription_receives_turn_and_debug_updates() {
        let mut state = UiProtocolState::default();
        let mut receiver = state.subscribe();

        let projection = sample_turn_projection(false);
        state.apply_turn_projection(projection.clone());
        let event = receiver.try_recv().expect("turn event");
        assert_eq!(
            event,
            UiSubscriptionEvent {
                projection: UiProjection::Turn(projection.clone()),
                latest_active_turn_id: Some(projection.turn_id.clone()),
            }
        );

        let debug = sample_debug_snapshot();
        state.set_debug_state(debug.clone());
        let event = receiver.try_recv().expect("debug event");
        assert_eq!(
            event,
            UiSubscriptionEvent {
                projection: UiProjection::Debug(debug),
                latest_active_turn_id: Some(projection.turn_id),
            }
        );
    }

    #[test]
    fn incremental_turn_projection_updates_from_shared_contract_events() {
        let mut state = UiProtocolState::default();
        let mut receiver = state.subscribe();

        let semantic = ReasonResp01SemanticEvent {
            session_id: SessionId::new("session-2"),
            turn_id: TurnId::new("turn-2"),
            trace_id: TraceId::new("trace-2"),
            feature_id: FeatureId::new("reason.turn"),
            agent_id: AgentId::new("agent-2"),
            kind: SemanticEventKind::Reasoning,
            content: "step one".to_owned(),
        };
        let projection = state.apply_semantic_event(
            AgentId::new("agent-2"),
            "node-2".to_owned(),
            &semantic,
            false,
        );
        assert_eq!(projection.reasoning, vec!["step one"]);
        let event = receiver.try_recv().expect("semantic publish");
        assert_eq!(event.latest_active_turn_id, Some(TurnId::new("turn-2")));

        let terminal = ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-2"),
            turn_id: TurnId::new("turn-2"),
            trace_id: TraceId::new("trace-2"),
            feature_id: FeatureId::new("reason.turn"),
            agent_id: AgentId::new("agent-2"),
            status: TerminalStatus::Success,
            summary: "done".to_owned(),
        };
        let projection = state.apply_terminal_event(
            AgentId::new("agent-2"),
            "node-2".to_owned(),
            &terminal,
            false,
        );
        assert_eq!(projection.terminal_text.as_deref(), Some("done"));
        let event = receiver.try_recv().expect("terminal publish");
        match event.projection {
            UiProjection::Turn(turn) => {
                assert_eq!(turn.turn_id, TurnId::new("turn-2"));
                assert_eq!(turn.terminal_text.as_deref(), Some("done"));
            }
            other => panic!("unexpected projection: {other:?}"),
        }
    }
}
