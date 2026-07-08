//! UI-facing commands, events, and projections for Freehand.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, TryRecvError};

use freehand_blocks::{
    ToolDisplayOutcome, ToolDisplayProjection, project_tool_call_display,
    project_tool_result_display, strip_completion_submission_block,
};
use freehand_contracts::{
    AgentId, ErrorErr01RuntimeClassified, ReasonReq04ToolCall, ReasonReq05ToolResultReentry,
    ReasonResp01SemanticEvent, ReasonResp02UsageEvent, ReasonResp03TerminalEvent,
    SemanticEventKind, SessionId, TerminalStatus, ToolResultContract, ToolResultStatus, TurnId,
};
use freehand_control::strip_control_status_block;
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
    TaskList,
    ErrorCenter,
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
    CreateSession {
        session_id: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
    RenameSession {
        session_id: SessionId,
        title: String,
    },
    ArchiveSession {
        session_id: SessionId,
    },
    RestoreSession {
        session_id: SessionId,
    },
    DeleteSession {
        session_id: SessionId,
    },
    RollbackLatestSessionTurn {
        session_id: SessionId,
    },
    SubmitUserInput {
        text: String,
        #[serde(default)]
        session_id: Option<SessionId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
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
    SubscribeTaskList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<AgentId>,
    },
    SubscribeErrorCenterEvents {
        session_id: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trace_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn_id: Option<TurnId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        domain: Option<String>,
    },
    SubscribeDebugState {
        client: UiClientKind,
        turn_id: TurnId,
    },
    QueryLatestActiveTurn,
    QueryTurn {
        turn_id: TurnId,
    },
    QuerySessionList,
    QueryArchivedSessionList,
    QuerySessionTurns {
        session_id: SessionId,
    },
    QueryConfigStatus,
    QueryTaskList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<AgentId>,
    },
    QueryTaskHistory {
        task_id: String,
    },
    QueryErrorCenterEvents {
        session_id: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trace_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn_id: Option<TurnId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        domain: Option<String>,
    },
    UpdateProviderConfig {
        update: UiProviderConfigUpdate,
    },
    CreateTask {
        task: UiTaskCreateCommand,
    },
    SubmitTaskReview {
        review: UiTaskReviewCommand,
    },
    ApproveTaskReview {
        task_id: String,
    },
    CloseTask {
        task_id: String,
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
pub struct UiProviderConfigUpdate {
    pub agent_name: String,
    pub provider_id: String,
    pub provider_type: String,
    pub provider_protocol: String,
    pub base_url: String,
    pub default_model: String,
    pub api_key_env: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTurnProjection {
    pub source: UiSource,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub user_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_request: Option<UiModelRequestActivity>,
    pub reasoning: Vec<String>,
    pub text: Vec<String>,
    pub tool_calls: Vec<String>,
    pub tool_activities: Vec<UiToolActivity>,
    pub usage: Vec<String>,
    pub terminal_status: Option<TerminalStatus>,
    pub terminal_text: Option<String>,
    pub errors: Vec<String>,
    pub slave_substream_card: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiModelRequestActivity {
    pub status: UiModelRequestStatus,
    #[serde(default)]
    pub kind: UiModelRequestKind,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiCompletionSchemaRetryWaiting {
    pub source_agent_id: AgentId,
    pub source_node_id: String,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub retry_index: u32,
    pub issue_summary: String,
    pub slave_substream_card: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiModelRequestWaiting {
    pub source_agent_id: AgentId,
    pub source_node_id: String,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub kind: UiModelRequestKind,
    pub detail: Option<String>,
    pub slave_substream_card: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiModelRequestStatus {
    Waiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UiModelRequestKind {
    #[default]
    Thinking,
    SchemaRetry,
    ToolResultContinuation,
}

impl UiModelRequestKind {
    pub fn as_str(self) -> &'static str {
        match self {
            UiModelRequestKind::Thinking => "thinking",
            UiModelRequestKind::SchemaRetry => "schema_retry",
            UiModelRequestKind::ToolResultContinuation => "tool_result_continuation",
        }
    }
}

impl UiModelRequestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            UiModelRequestStatus::Waiting => "waiting",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiToolActivityStatus {
    Waiting,
    Completed,
    Failed,
}

impl UiToolActivityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            UiToolActivityStatus::Waiting => "waiting",
            UiToolActivityStatus::Completed => "completed",
            UiToolActivityStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiToolActivity {
    pub tool_call_id: String,
    pub tool_name: String,
    pub status: UiToolActivityStatus,
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<ToolDisplayProjection>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<ToolDisplayProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiPublicTurnProjection {
    pub turn: UiTurnProjection,
    pub public_conversation: Vec<UiConversationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionSummary {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub latest_turn_id: Option<TurnId>,
    pub active_turn_id: Option<TurnId>,
    pub turn_count: usize,
    pub latest_status: String,
    pub latest_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionListProjection {
    pub sessions: Vec<UiSessionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionTranscriptProjection {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub turns: Vec<UiTurnProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionMetadataProjection {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
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
pub struct UiTaskSnapshotProjection {
    pub task_id: String,
    pub status: String,
    pub title: String,
    pub goal: String,
    pub priority: i64,
    pub target_cwd: Option<String>,
    pub assignee_agent_id: Option<AgentId>,
    pub updated_at: u64,
    pub last_progress_at: Option<u64>,
    pub last_event_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskListProjection {
    pub source_agent_id: AgentId,
    pub status_filter: Option<String>,
    pub agent_filter: Option<AgentId>,
    pub tasks: Vec<UiTaskSnapshotProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskLedgerEventProjection {
    pub seq: u64,
    pub event_id: String,
    pub event_type: String,
    pub from_status: Option<String>,
    pub to_status: String,
    pub timestamp: u64,
    pub actor_agent_id: AgentId,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskHistoryProjection {
    pub source_agent_id: AgentId,
    pub task_id: String,
    pub events: Vec<UiTaskLedgerEventProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiErrorCenterEventProjection {
    pub metadata_id: String,
    pub source_agent_id: Option<AgentId>,
    pub session_id: Option<SessionId>,
    pub turn_id: Option<TurnId>,
    pub trace_id: String,
    pub writer_feature_id: String,
    pub writer_crate: String,
    pub writer_symbol: String,
    pub pipeline_node: String,
    pub domain: String,
    pub class: String,
    pub code: String,
    pub source_owner: String,
    pub source_pipeline_node: String,
    pub recovery_action: String,
    pub retry_index: u64,
    pub retry_cap: u64,
    pub public_visibility: String,
    pub owner_target: String,
    pub repair_fields: Vec<String>,
    pub raw_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiErrorCenterEventListProjection {
    pub source_agent_id: AgentId,
    pub session_id: SessionId,
    pub trace_filter: Option<String>,
    pub turn_filter: Option<TurnId>,
    pub domain_filter: Option<String>,
    pub events: Vec<UiErrorCenterEventProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConfigStatusProjection {
    pub agent_name: String,
    pub agent_mode: String,
    pub node_id: String,
    pub paired_agent_name: String,
    pub paired_agent_mode: String,
    pub paired_node_id: String,
    pub provider_id: String,
    pub provider_type: String,
    pub provider_protocol: String,
    pub provider_base_url_host: String,
    pub default_model: String,
    pub provider_auth_type: String,
    pub provider_auth_source: String,
    pub restart_required_on_change: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskCreateCommand {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub title: String,
    pub content: String,
    pub goal: String,
    #[serde(default)]
    pub deliverables: Vec<String>,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub priority: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<TurnId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskReviewCommand {
    pub task_id: String,
    pub summary: String,
    #[serde(default)]
    pub deliverables: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiProjection {
    Turn(UiTurnProjection),
    NodeStatus(NodeStatusSnapshot),
    Progress(TaskProgressSnapshot),
    Debug(DebugStateSnapshot),
    Checkpoints(UiCheckpointSnapshot),
    TaskList(UiTaskListProjection),
    ErrorCenterEvents(UiErrorCenterEventListProjection),
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
    pub cwd: Option<String>,
    pub user_text: Option<String>,
    pub semantic_events: Vec<ReasonResp01SemanticEvent>,
    pub tool_calls: Vec<ReasonReq04ToolCall>,
    pub tool_results: Vec<ReasonReq05ToolResultReentry>,
    pub usage_events: Vec<ReasonResp02UsageEvent>,
    pub terminal_event: Option<ReasonResp03TerminalEvent>,
    pub error_events: Vec<ErrorErr01RuntimeClassified>,
    pub slave_substream_card: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiQueryResult {
    Turn(Option<UiTurnProjection>),
    SessionList(UiSessionListProjection),
    SessionTurns(UiSessionTranscriptProjection),
    NodeStatus(Option<NodeStatusSnapshot>),
    Progress(Option<TaskProgressSnapshot>),
    Debug(Option<DebugStateSnapshot>),
    Checkpoints(UiCheckpointSnapshot),
    TaskList(UiTaskListProjection),
    TaskHistory(UiTaskHistoryProjection),
    ErrorCenterEvents(UiErrorCenterEventListProjection),
    ConfigStatus(UiConfigStatusProjection),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAdpFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiAdpRequest {
    Command {
        request_id: String,
        command: UiCommand,
    },
    Query {
        request_id: String,
        query: UiCommand,
    },
    Subscribe {
        request_id: String,
        subscription: UiCommand,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiAdpResponse {
    CommandReceipt {
        request_id: String,
        receipt: UiCommandDispatchReceipt,
    },
    QueryResult {
        request_id: String,
        result: UiQueryResult,
    },
    SubscriptionEvent {
        request_id: String,
        event: UiSubscriptionEvent,
    },
    SubscriptionAccepted {
        request_id: String,
        selector: SubscriptionSelector,
    },
    Failure {
        request_id: String,
        failure: UiAdpFailure,
    },
}

pub trait UiCommandDispatchPort: Send + Sync {
    fn dispatch(
        &self,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError>;
}

pub trait UiRuntimeQueryPort: Send + Sync {
    fn query_runtime(
        &self,
        command: &UiCommand,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError>;
}

pub struct UiProtocolOnlyQueryPort;

impl UiRuntimeQueryPort for UiProtocolOnlyQueryPort {
    fn query_runtime(
        &self,
        _command: &UiCommand,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError> {
        Ok(None)
    }
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
    session_cwds: BTreeMap<SessionId, String>,
    session_metadata: BTreeMap<SessionId, UiSessionMetadataProjection>,
    node_status: BTreeMap<String, NodeStatusSnapshot>,
    progress: BTreeMap<TurnId, TaskProgressSnapshot>,
    debug: BTreeMap<TurnId, DebugStateSnapshot>,
    checkpoints: Option<UiCheckpointSnapshot>,
    subscription_tx: broadcast::Sender<UiSubscriptionEvent>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UiProtocolError {
    #[error("session command requires non-empty session id")]
    EmptySessionId,
    #[error("session title must be non-empty when provided")]
    EmptySessionTitle,
    #[error("submit user input command requires non-empty text")]
    EmptyUserInput,
    #[error("session cwd must be non-empty when provided")]
    EmptySessionCwd,
    #[error("direct slave message requires non-empty text")]
    EmptySlaveMessage,
    #[error("rewind checkpoint command requires non-empty checkpoint id")]
    EmptyCheckpointId,
    #[error("task query requires non-empty task id")]
    EmptyTaskId,
    #[error("task create requires non-empty title")]
    EmptyTaskTitle,
    #[error("task create requires non-empty content")]
    EmptyTaskContent,
    #[error("task create requires non-empty goal")]
    EmptyTaskGoal,
    #[error("task review requires non-empty summary")]
    EmptyTaskReviewSummary,
    #[error("config update requires non-empty agent name")]
    EmptyConfigAgentName,
    #[error("config update requires non-empty provider id")]
    EmptyProviderId,
    #[error("config update requires non-empty provider type")]
    EmptyProviderType,
    #[error("config update requires non-empty provider protocol")]
    EmptyProviderProtocol,
    #[error("config update requires non-empty provider base URL")]
    EmptyProviderBaseUrl,
    #[error("config update requires non-empty default model")]
    EmptyProviderDefaultModel,
    #[error("config update requires non-empty API key environment variable name")]
    EmptyProviderApiKeyEnv,
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
            session_cwds: BTreeMap::new(),
            session_metadata: BTreeMap::new(),
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

    pub fn publish_task_list_projection(&self, projection: UiTaskListProjection) {
        self.publish_projection(UiProjection::TaskList(projection));
    }

    pub fn publish_error_center_events_projection(
        &self,
        projection: UiErrorCenterEventListProjection,
    ) {
        self.publish_projection(UiProjection::ErrorCenterEvents(projection));
    }

    pub fn apply_turn_projection(&mut self, projection: UiTurnProjection) {
        self.latest_active_turn_id = Some(projection.turn_id.clone());
        if let Some(cwd) = projection.cwd.clone() {
            self.session_cwds.insert(projection.session_id.clone(), cwd);
        }
        self.turns
            .insert(projection.turn_id.clone(), projection.clone());
        self.publish_projection(UiProjection::Turn(projection));
    }

    pub fn replace_session_turn_projections(
        &mut self,
        session_id: &SessionId,
        projections: impl IntoIterator<Item = UiTurnProjection>,
    ) {
        self.turns
            .retain(|_, projection| &projection.session_id != session_id);
        let mut latest_session_turn_id = None;
        for projection in projections {
            latest_session_turn_id = Some(projection.turn_id.clone());
            if let Some(cwd) = projection.cwd.clone() {
                self.session_cwds.insert(projection.session_id.clone(), cwd);
            }
            self.turns
                .insert(projection.turn_id.clone(), projection.clone());
            self.publish_projection(UiProjection::Turn(projection));
        }
        if self
            .latest_active_turn_id
            .as_ref()
            .is_some_and(|turn_id| !self.turns.contains_key(turn_id))
        {
            self.latest_active_turn_id = latest_session_turn_id.or_else(|| {
                self.turns
                    .values()
                    .last()
                    .map(|projection| projection.turn_id.clone())
            });
        }
    }

    pub fn set_session_cwd(&mut self, session_id: SessionId, cwd: impl Into<String>) {
        let cwd = cwd.into();
        self.session_cwds.insert(session_id.clone(), cwd.clone());
        self.session_metadata
            .entry(session_id.clone())
            .and_modify(|metadata| metadata.cwd = Some(cwd.clone()))
            .or_insert_with(|| UiSessionMetadataProjection {
                session_id: session_id.clone(),
                title: None,
                archived: false,
                cwd: Some(cwd.clone()),
            });
        for projection in self.turns.values_mut() {
            if projection.session_id == session_id {
                projection.cwd = Some(cwd.clone());
            }
        }
    }

    pub fn set_session_metadata(&mut self, metadata: UiSessionMetadataProjection) {
        if let Some(cwd) = metadata.cwd.clone() {
            self.session_cwds
                .insert(metadata.session_id.clone(), cwd.clone());
            for projection in self.turns.values_mut() {
                if projection.session_id == metadata.session_id && projection.cwd.is_none() {
                    projection.cwd = Some(cwd.clone());
                }
            }
        }
        self.session_metadata
            .insert(metadata.session_id.clone(), metadata);
    }

    pub fn set_session_metadata_entries(
        &mut self,
        entries: impl IntoIterator<Item = UiSessionMetadataProjection>,
    ) {
        for entry in entries {
            self.set_session_metadata(entry);
        }
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
            projection.model_request = None;
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
            upsert_tool_activity(
                &mut projection.tool_activities,
                event.tool_call.tool_call_id.as_str().to_owned(),
                event.tool_call.tool_name.clone(),
                UiToolActivityStatus::Waiting,
                Some("waiting for tool execution".to_owned()),
                Some(project_tool_call_display(
                    &event.tool_call.tool_name,
                    &event.tool_call.arguments,
                )),
            );
            projection.model_request = None;
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_model_request_waiting(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        session_id: &SessionId,
        turn_id: &TurnId,
        detail: Option<String>,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        self.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id,
            source_node_id,
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail,
            slave_substream_card,
        })
    }

    pub fn apply_model_request_waiting_kind(
        &mut self,
        waiting: UiModelRequestWaiting,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                waiting.source_agent_id,
                waiting.source_node_id,
                &waiting.session_id,
                &waiting.turn_id,
                waiting.slave_substream_card,
            );
            projection.model_request = Some(UiModelRequestActivity {
                status: UiModelRequestStatus::Waiting,
                kind: waiting.kind,
                detail: waiting.detail,
            });
            projection.clone()
        };
        self.latest_active_turn_id = Some(waiting.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_completion_schema_retry_waiting(
        &mut self,
        waiting: UiCompletionSchemaRetryWaiting,
    ) -> UiTurnProjection {
        let detail = format!(
            "schema polishing #{}: {}",
            waiting.retry_index, waiting.issue_summary
        );
        self.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: waiting.source_agent_id,
            source_node_id: waiting.source_node_id,
            session_id: waiting.session_id,
            turn_id: waiting.turn_id,
            kind: UiModelRequestKind::SchemaRetry,
            detail: Some(detail),
            slave_substream_card: waiting.slave_substream_card,
        })
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
            projection.model_request = None;
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_tool_result(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonReq05ToolResultReentry,
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
            let tool_call_id = event.tool_result.tool_call_id.as_str().to_owned();
            let tool_name = projection
                .tool_activities
                .iter()
                .find(|activity| activity.tool_call_id == tool_call_id)
                .map(|activity| activity.tool_name.clone())
                .unwrap_or_else(|| "tool".to_owned());
            let display = projection
                .tool_activities
                .iter()
                .find(|activity| activity.tool_call_id == tool_call_id)
                .and_then(|activity| activity.display.clone())
                .map(|display| project_tool_result_display(display, &event.tool_result));
            upsert_tool_activity(
                &mut projection.tool_activities,
                tool_call_id,
                tool_name,
                tool_activity_status_from_result(event.tool_result.status),
                Some(tool_activity_detail_from_result(&event.tool_result)),
                display,
            );
            projection.model_request = None;
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
            projection.model_request = None;
            if event.status == TerminalStatus::Failed {
                fail_waiting_tool_activities(
                    &mut projection.tool_activities,
                    Some(event.summary.clone()),
                );
            }
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
            projection.model_request = None;
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
            UiCommand::QuerySessionList => Ok(UiQueryResult::SessionList(session_list_projection(
                &self.turns,
                &self.session_cwds,
                &self.session_metadata,
                self.latest_active_turn_id.as_ref(),
                false,
            ))),
            UiCommand::QueryArchivedSessionList => {
                Ok(UiQueryResult::SessionList(session_list_projection(
                    &self.turns,
                    &self.session_cwds,
                    &self.session_metadata,
                    self.latest_active_turn_id.as_ref(),
                    true,
                )))
            }
            UiCommand::QuerySessionTurns { session_id } => {
                Ok(UiQueryResult::SessionTurns(session_transcript_projection(
                    session_id,
                    &self.turns,
                    &self.session_cwds,
                    &self.session_metadata,
                )))
            }
            UiCommand::QueryTaskList { .. }
            | UiCommand::QueryTaskHistory { .. }
            | UiCommand::QueryConfigStatus
            | UiCommand::QueryErrorCenterEvents { .. } => Err(UiProtocolError::StreamKindMismatch),
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
                cwd: self.session_cwds.get(session_id).cloned(),
                user_text: None,
                model_request: None,
                reasoning: Vec::new(),
                text: Vec::new(),
                tool_calls: Vec::new(),
                tool_activities: Vec::new(),
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
        UiCommand::CreateSession {
            session_id,
            title,
            cwd,
        } if session_id.as_str().trim().is_empty()
            || title.as_ref().is_some_and(|title| title.trim().is_empty())
            || cwd.as_ref().is_some_and(|cwd| cwd.trim().is_empty()) =>
        {
            if session_id.as_str().trim().is_empty() {
                Err(UiProtocolError::EmptySessionId)
            } else if cwd.as_ref().is_some_and(|cwd| cwd.trim().is_empty()) {
                Err(UiProtocolError::EmptySessionCwd)
            } else {
                Err(UiProtocolError::EmptySessionTitle)
            }
        }
        UiCommand::RenameSession { session_id, title }
            if session_id.as_str().trim().is_empty() || title.trim().is_empty() =>
        {
            if session_id.as_str().trim().is_empty() {
                Err(UiProtocolError::EmptySessionId)
            } else {
                Err(UiProtocolError::EmptySessionTitle)
            }
        }
        UiCommand::ArchiveSession { session_id }
        | UiCommand::RestoreSession { session_id }
        | UiCommand::DeleteSession { session_id }
        | UiCommand::RollbackLatestSessionTurn { session_id }
            if session_id.as_str().trim().is_empty() =>
        {
            Err(UiProtocolError::EmptySessionId)
        }
        UiCommand::SubmitUserInput { text, .. } if text.trim().is_empty() => {
            Err(UiProtocolError::EmptyUserInput)
        }
        UiCommand::SubmitUserInput { cwd: Some(cwd), .. } if cwd.trim().is_empty() => {
            Err(UiProtocolError::EmptySessionCwd)
        }
        UiCommand::SendDirectMessageToSlave { text, .. } if text.trim().is_empty() => {
            Err(UiProtocolError::EmptySlaveMessage)
        }
        UiCommand::RewindCheckpoint { checkpoint_id } if checkpoint_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyCheckpointId)
        }
        UiCommand::QueryTaskHistory { task_id } if task_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::CreateTask { task } if task.title.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskTitle)
        }
        UiCommand::CreateTask { task } if task.content.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskContent)
        }
        UiCommand::CreateTask { task } if task.goal.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskGoal)
        }
        UiCommand::CreateTask { task }
            if task
                .task_id
                .as_ref()
                .is_some_and(|task_id| task_id.trim().is_empty()) =>
        {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::CreateTask { task }
            if task
                .target_cwd
                .as_ref()
                .is_some_and(|cwd| cwd.trim().is_empty()) =>
        {
            Err(UiProtocolError::EmptySessionCwd)
        }
        UiCommand::SubmitTaskReview { review } if review.task_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::SubmitTaskReview { review } if review.summary.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskReviewSummary)
        }
        UiCommand::ApproveTaskReview { task_id } | UiCommand::CloseTask { task_id }
            if task_id.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::UpdateProviderConfig { update } if update.agent_name.trim().is_empty() => {
            Err(UiProtocolError::EmptyConfigAgentName)
        }
        UiCommand::UpdateProviderConfig { update } if update.provider_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyProviderId)
        }
        UiCommand::UpdateProviderConfig { update } if update.provider_type.trim().is_empty() => {
            Err(UiProtocolError::EmptyProviderType)
        }
        UiCommand::UpdateProviderConfig { update }
            if update.provider_protocol.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyProviderProtocol)
        }
        UiCommand::UpdateProviderConfig { update } if update.base_url.trim().is_empty() => {
            Err(UiProtocolError::EmptyProviderBaseUrl)
        }
        UiCommand::UpdateProviderConfig { update } if update.default_model.trim().is_empty() => {
            Err(UiProtocolError::EmptyProviderDefaultModel)
        }
        UiCommand::UpdateProviderConfig { update } if update.api_key_env.trim().is_empty() => {
            Err(UiProtocolError::EmptyProviderApiKeyEnv)
        }
        UiCommand::QueryErrorCenterEvents { session_id, .. }
        | UiCommand::SubscribeErrorCenterEvents { session_id, .. }
            if session_id.as_str().trim().is_empty() =>
        {
            Err(UiProtocolError::EmptySessionId)
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
        UiProtocolError::EmptySessionId => "empty_session_id",
        UiProtocolError::EmptySessionTitle => "empty_session_title",
        UiProtocolError::EmptyUserInput => "empty_user_input",
        UiProtocolError::EmptySessionCwd => "empty_session_cwd",
        UiProtocolError::EmptySlaveMessage => "empty_slave_message",
        UiProtocolError::EmptyCheckpointId => "empty_checkpoint_id",
        UiProtocolError::EmptyTaskId => "empty_task_id",
        UiProtocolError::EmptyTaskTitle => "empty_task_title",
        UiProtocolError::EmptyTaskContent => "empty_task_content",
        UiProtocolError::EmptyTaskGoal => "empty_task_goal",
        UiProtocolError::EmptyTaskReviewSummary => "empty_task_review_summary",
        UiProtocolError::EmptyConfigAgentName => "empty_config_agent_name",
        UiProtocolError::EmptyProviderId => "empty_provider_id",
        UiProtocolError::EmptyProviderType => "empty_provider_type",
        UiProtocolError::EmptyProviderProtocol => "empty_provider_protocol",
        UiProtocolError::EmptyProviderBaseUrl => "empty_provider_base_url",
        UiProtocolError::EmptyProviderDefaultModel => "empty_provider_default_model",
        UiProtocolError::EmptyProviderApiKeyEnv => "empty_provider_api_key_env",
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
        UiCommand::SubscribeTaskList { .. } => Some(SubscriptionSelector {
            client: UiClientKind::WebUi,
            stream_kind: UiStreamKind::TaskList,
            target_turn_id: None,
        }),
        UiCommand::SubscribeErrorCenterEvents { .. } => Some(SubscriptionSelector {
            client: UiClientKind::WebUi,
            stream_kind: UiStreamKind::ErrorCenter,
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
        (UiStreamKind::TaskList, UiProjection::TaskList(_)) => true,
        (UiStreamKind::ErrorCenter, UiProjection::ErrorCenterEvents(_)) => true,
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
            tool_call_id: None,
            display: None,
        });
    }
    for text in &projection.text {
        let public_text = strip_control_status_block(&strip_completion_submission_block(text));
        if !public_text.trim().is_empty() {
            items.push(UiConversationItem {
                kind: UiConversationItemKind::AssistantText,
                title: "Assistant".to_owned(),
                body: public_text,
                status: "streaming".to_owned(),
                tool_call_id: None,
                display: None,
            });
        }
    }
    for activity in &projection.tool_activities {
        let title = activity
            .display
            .as_ref()
            .map(|display| display.action.clone())
            .unwrap_or_else(|| activity.tool_name.clone());
        let body = tool_public_body(activity);
        items.push(UiConversationItem {
            kind: UiConversationItemKind::ToolSummary,
            title,
            body,
            status: activity.status.as_str().to_owned(),
            tool_call_id: Some(activity.tool_call_id.clone()),
            display: activity.display.clone(),
        });
    }
    if let Some(terminal_text) = &projection.terminal_text {
        let public_text =
            strip_control_status_block(&strip_completion_submission_block(terminal_text));
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
                tool_call_id: None,
                display: None,
            });
        }
    }
    for error in &projection.errors {
        items.push(UiConversationItem {
            kind: UiConversationItemKind::Error,
            title: "Error".to_owned(),
            body: error.clone(),
            status: "failed".to_owned(),
            tool_call_id: None,
            display: None,
        });
    }
    items
}

fn tool_public_body(activity: &UiToolActivity) -> String {
    let semantic_body = activity.display.as_ref().and_then(tool_display_public_body);
    match activity.status {
        UiToolActivityStatus::Waiting => semantic_body
            .or_else(|| activity.detail.clone())
            .unwrap_or_else(|| "waiting".to_owned()),
        UiToolActivityStatus::Completed => semantic_body
            .or_else(|| activity.detail.clone())
            .unwrap_or_else(|| "completed".to_owned()),
        UiToolActivityStatus::Failed => semantic_body
            .or_else(|| activity.detail.clone())
            .unwrap_or_else(|| "failed".to_owned()),
    }
}

fn tool_display_public_body(display: &ToolDisplayProjection) -> Option<String> {
    if let Some(diff) = &display.diff {
        return Some(format!(
            "diff: {}\n- {}\n+ {}",
            diff.target, diff.before, diff.after
        ));
    }
    if let Some(parameter_summary) = &display.parameter_summary
        && !parameter_summary.trim().is_empty()
    {
        return Some(parameter_summary.clone());
    }
    if !display.summary.trim().is_empty() {
        return Some(display.summary.clone());
    }
    if !display.fields.is_empty() {
        let compact_fields = display
            .fields
            .iter()
            .take(4)
            .map(|field| format!("{}: {}", field.label, field.value))
            .collect::<Vec<_>>()
            .join(" · ");
        if !compact_fields.trim().is_empty() {
            return Some(compact_fields);
        }
    }
    None
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

fn session_list_projection(
    turns: &BTreeMap<TurnId, UiTurnProjection>,
    session_cwds: &BTreeMap<SessionId, String>,
    session_metadata: &BTreeMap<SessionId, UiSessionMetadataProjection>,
    latest_active_turn_id: Option<&TurnId>,
    archived: bool,
) -> UiSessionListProjection {
    let mut grouped: Vec<(SessionId, Vec<&UiTurnProjection>)> = Vec::new();
    for turn in turns.values() {
        match grouped
            .iter_mut()
            .find(|(session_id, _)| session_id == &turn.session_id)
        {
            Some((_, session_turns)) => session_turns.push(turn),
            None => grouped.push((turn.session_id.clone(), vec![turn])),
        }
    }

    let mut sessions = grouped
        .into_iter()
        .map(|(session_id, mut session_turns)| {
            session_turns.sort_by(|left, right| {
                turn_order_key(&left.turn_id).cmp(&turn_order_key(&right.turn_id))
            });
            let latest = session_turns.last().copied();
            let active_turn_id = latest_active_turn_id.and_then(|turn_id| {
                session_turns
                    .iter()
                    .any(|turn| &turn.turn_id == turn_id)
                    .then(|| turn_id.clone())
            });
            let metadata = session_metadata.get(&session_id);
            let cwd = session_cwds
                .get(&session_id)
                .cloned()
                .or_else(|| metadata.and_then(|metadata| metadata.cwd.clone()))
                .or_else(|| latest.and_then(|turn| turn.cwd.clone()));
            UiSessionSummary {
                session_id,
                title: metadata.and_then(|metadata| metadata.title.clone()),
                archived: metadata.is_some_and(|metadata| metadata.archived),
                cwd,
                latest_turn_id: latest.map(|turn| turn.turn_id.clone()),
                active_turn_id,
                turn_count: session_turns.len(),
                latest_status: latest
                    .map(session_latest_status)
                    .unwrap_or_else(|| "empty".to_owned()),
                latest_summary: latest.and_then(session_latest_summary),
            }
        })
        .collect::<Vec<_>>();
    for metadata in session_metadata.values() {
        if metadata.archived != archived
            || sessions
                .iter()
                .any(|session| session.session_id == metadata.session_id)
        {
            continue;
        }
        sessions.push(UiSessionSummary {
            session_id: metadata.session_id.clone(),
            title: metadata.title.clone(),
            archived: metadata.archived,
            cwd: metadata
                .cwd
                .clone()
                .or_else(|| session_cwds.get(&metadata.session_id).cloned()),
            latest_turn_id: None,
            active_turn_id: None,
            turn_count: 0,
            latest_status: "empty".to_owned(),
            latest_summary: metadata.title.clone(),
        });
    }
    sessions.retain(|session| session.archived == archived);
    sessions.sort_by(|left, right| left.session_id.as_str().cmp(right.session_id.as_str()));
    UiSessionListProjection { sessions }
}

fn session_transcript_projection(
    session_id: &SessionId,
    turns: &BTreeMap<TurnId, UiTurnProjection>,
    session_cwds: &BTreeMap<SessionId, String>,
    session_metadata: &BTreeMap<SessionId, UiSessionMetadataProjection>,
) -> UiSessionTranscriptProjection {
    let metadata = session_metadata.get(session_id);
    let cwd = session_cwds
        .get(session_id)
        .cloned()
        .or_else(|| metadata.and_then(|metadata| metadata.cwd.clone()));
    let mut session_turns = turns
        .values()
        .filter(|turn| &turn.session_id == session_id)
        .cloned()
        .collect::<Vec<_>>();
    for turn in &mut session_turns {
        if turn.cwd.is_none() {
            turn.cwd = cwd.clone();
        }
    }
    session_turns
        .sort_by(|left, right| turn_order_key(&left.turn_id).cmp(&turn_order_key(&right.turn_id)));
    UiSessionTranscriptProjection {
        session_id: session_id.clone(),
        title: metadata.and_then(|metadata| metadata.title.clone()),
        archived: metadata.is_some_and(|metadata| metadata.archived),
        cwd,
        turns: session_turns,
    }
}

fn session_latest_status(turn: &UiTurnProjection) -> String {
    if let Some(status) = &turn.terminal_status {
        return format!("{status:?}").to_lowercase();
    }
    if turn
        .tool_activities
        .iter()
        .any(|activity| activity.status == UiToolActivityStatus::Waiting)
    {
        return "tool_running".to_owned();
    }
    if turn.model_request.is_some() {
        return "waiting_model".to_owned();
    }
    if !turn.text.is_empty() || !turn.reasoning.is_empty() {
        return "active".to_owned();
    }
    "submitted".to_owned()
}

fn session_latest_summary(turn: &UiTurnProjection) -> Option<String> {
    turn.terminal_text
        .clone()
        .or_else(|| turn.text.last().cloned())
        .or_else(|| turn.user_text.clone())
}

fn turn_order_key(turn_id: &TurnId) -> (String, u64, u64, String) {
    let raw = turn_id.as_str();
    if let Some(rest) = raw.strip_prefix("runtime-turn-") {
        let (ordinal_part, round_part) = match rest.split_once("-r") {
            Some((ordinal, round)) => (ordinal, round),
            None => (rest, "1"),
        };
        let ordinal = ordinal_part.parse::<u64>().unwrap_or(u64::MAX);
        let round = round_part.parse::<u64>().unwrap_or(1);
        return ("runtime-turn-".to_owned(), ordinal, round, raw.to_owned());
    }
    let split_at = raw
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_ascii_digit())
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let (prefix, digits) = raw.split_at(split_at);
    let ordinal = if digits.is_empty() {
        0
    } else {
        digits.parse::<u64>().unwrap_or(u64::MAX)
    };
    (prefix.to_owned(), ordinal, 1, raw.to_owned())
}

fn tool_activities_from_input(
    tool_calls: &[ReasonReq04ToolCall],
    tool_results: &[ReasonReq05ToolResultReentry],
) -> Vec<UiToolActivity> {
    let mut activities = Vec::new();
    for call in tool_calls {
        upsert_tool_activity(
            &mut activities,
            call.tool_call.tool_call_id.as_str().to_owned(),
            call.tool_call.tool_name.clone(),
            UiToolActivityStatus::Waiting,
            Some("waiting for tool execution".to_owned()),
            Some(project_tool_call_display(
                &call.tool_call.tool_name,
                &call.tool_call.arguments,
            )),
        );
    }
    for result in tool_results {
        let tool_call_id = result.tool_result.tool_call_id.as_str().to_owned();
        let tool_name = tool_calls
            .iter()
            .find(|call| call.tool_call.tool_call_id.as_str() == tool_call_id)
            .map(|call| call.tool_call.tool_name.clone())
            .unwrap_or_else(|| "tool".to_owned());
        let display = activities
            .iter()
            .find(|activity| activity.tool_call_id == tool_call_id)
            .and_then(|activity| activity.display.clone())
            .map(|display| project_tool_result_display(display, &result.tool_result));
        upsert_tool_activity(
            &mut activities,
            tool_call_id,
            tool_name,
            tool_activity_status_from_result(result.tool_result.status),
            Some(tool_activity_detail_from_result(&result.tool_result)),
            display,
        );
    }
    activities
}

fn upsert_tool_activity(
    activities: &mut Vec<UiToolActivity>,
    tool_call_id: String,
    tool_name: String,
    status: UiToolActivityStatus,
    detail: Option<String>,
    display: Option<ToolDisplayProjection>,
) {
    if let Some(activity) = activities
        .iter_mut()
        .find(|activity| activity.tool_call_id == tool_call_id)
    {
        activity.tool_name = tool_name;
        activity.status = match (activity.status, status) {
            (UiToolActivityStatus::Completed, UiToolActivityStatus::Waiting) => {
                UiToolActivityStatus::Completed
            }
            (UiToolActivityStatus::Completed, UiToolActivityStatus::Failed) => {
                UiToolActivityStatus::Completed
            }
            _ => status,
        };
        activity.detail = detail;
        if display.is_some() {
            activity.display = display;
        }
        return;
    }

    activities.push(UiToolActivity {
        tool_call_id,
        tool_name,
        status,
        detail,
        display,
    });
}

fn tool_activity_status_from_result(status: ToolResultStatus) -> UiToolActivityStatus {
    match status {
        ToolResultStatus::Success => UiToolActivityStatus::Completed,
        ToolResultStatus::Failed => UiToolActivityStatus::Failed,
    }
}

fn tool_activity_detail_from_result(result: &ToolResultContract) -> String {
    let prefix = match result.status {
        ToolResultStatus::Success => "result",
        ToolResultStatus::Failed => "failure",
    };
    if result.output.trim().is_empty() {
        return match result.status {
            ToolResultStatus::Success => "result: <empty>".to_owned(),
            ToolResultStatus::Failed => "failure: <empty>".to_owned(),
        };
    }
    format!("{prefix}: {}", result.output)
}

fn fail_waiting_tool_activities(activities: &mut [UiToolActivity], detail: Option<String>) {
    for activity in activities {
        if activity.status == UiToolActivityStatus::Waiting {
            activity.status = UiToolActivityStatus::Failed;
            activity.detail = detail.clone();
            if let Some(display) = &mut activity.display {
                display.outcome = ToolDisplayOutcome::Failed;
                display.result_summary = detail.clone();
            }
        }
    }
}

fn command_kind(command: &UiCommand) -> &'static str {
    match command {
        UiCommand::CreateSession { .. } => "create_session",
        UiCommand::RenameSession { .. } => "rename_session",
        UiCommand::ArchiveSession { .. } => "archive_session",
        UiCommand::RestoreSession { .. } => "restore_session",
        UiCommand::DeleteSession { .. } => "delete_session",
        UiCommand::RollbackLatestSessionTurn { .. } => "rollback_latest_session_turn",
        UiCommand::SubmitUserInput { .. } => "submit_user_input",
        UiCommand::SubscribeLatestActiveTurn { .. } => "subscribe_latest_active_turn",
        UiCommand::SubscribeTurn { .. } => "subscribe_turn",
        UiCommand::SubscribeNodeStatus => "subscribe_node_status",
        UiCommand::SubscribeProgress => "subscribe_progress",
        UiCommand::SubscribeTaskList { .. } => "subscribe_task_list",
        UiCommand::SubscribeErrorCenterEvents { .. } => "subscribe_error_center_events",
        UiCommand::SubscribeDebugState { .. } => "subscribe_debug_state",
        UiCommand::QueryLatestActiveTurn => "query_latest_active_turn",
        UiCommand::QueryTurn { .. } => "query_turn",
        UiCommand::QuerySessionList => "query_session_list",
        UiCommand::QueryArchivedSessionList => "query_archived_session_list",
        UiCommand::QuerySessionTurns { .. } => "query_session_turns",
        UiCommand::QueryConfigStatus => "query_config_status",
        UiCommand::QueryTaskList { .. } => "query_task_list",
        UiCommand::QueryTaskHistory { .. } => "query_task_history",
        UiCommand::QueryErrorCenterEvents { .. } => "query_error_center_events",
        UiCommand::UpdateProviderConfig { .. } => "update_provider_config",
        UiCommand::CreateTask { .. } => "create_task",
        UiCommand::SubmitTaskReview { .. } => "submit_task_review",
        UiCommand::ApproveTaskReview { .. } => "approve_task_review",
        UiCommand::CloseTask { .. } => "close_task",
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
        UiCommand::CreateSession { .. }
            | UiCommand::RenameSession { .. }
            | UiCommand::ArchiveSession { .. }
            | UiCommand::RestoreSession { .. }
            | UiCommand::DeleteSession { .. }
            | UiCommand::RollbackLatestSessionTurn { .. }
            | UiCommand::SubmitUserInput { .. }
            | UiCommand::UpdateProviderConfig { .. }
            | UiCommand::CreateTask { .. }
            | UiCommand::SubmitTaskReview { .. }
            | UiCommand::ApproveTaskReview { .. }
            | UiCommand::CloseTask { .. }
            | UiCommand::SendDirectMessageToSlave { .. }
            | UiCommand::RewindCheckpoint { .. }
            | UiCommand::CancelTurn { .. }
            | UiCommand::CancelLatestActiveTurn { .. }
            | UiCommand::ResumeTurn { .. }
    )
}

fn command_dispatch_target(command: &UiCommand) -> (&'static str, &'static str) {
    match command {
        UiCommand::CreateSession { .. }
        | UiCommand::RenameSession { .. }
        | UiCommand::ArchiveSession { .. }
        | UiCommand::RestoreSession { .. }
        | UiCommand::DeleteSession { .. }
        | UiCommand::RollbackLatestSessionTurn { .. } => {
            ("reason.persistence", "crates/freehand-reason")
        }
        UiCommand::SubmitUserInput { .. }
        | UiCommand::CancelTurn { .. }
        | UiCommand::CancelLatestActiveTurn { .. }
        | UiCommand::ResumeTurn { .. } => ("reason.turn", "crates/freehand-reason"),
        UiCommand::RewindCheckpoint { .. } => {
            ("runtime.checkpoint-rewind", "crates/freehand-runtime")
        }
        UiCommand::UpdateProviderConfig { .. } => ("config.core", "crates/freehand-config"),
        UiCommand::CreateTask { .. }
        | UiCommand::SubmitTaskReview { .. }
        | UiCommand::ApproveTaskReview { .. }
        | UiCommand::CloseTask { .. } => ("task.orchestration", "crates/freehand-task"),
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
    let mut tool_activities = tool_activities_from_input(&input.tool_calls, &input.tool_results);
    if matches!(
        input.terminal_event.as_ref().map(|event| &event.status),
        Some(TerminalStatus::Failed)
    ) {
        let detail = input
            .terminal_event
            .as_ref()
            .map(|event| event.summary.clone())
            .or_else(|| {
                input
                    .error_events
                    .first()
                    .map(|event| event.error.message.clone())
            });
        fail_waiting_tool_activities(&mut tool_activities, detail);
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
        cwd: input.cwd,
        user_text: input.user_text,
        model_request: None,
        reasoning,
        text,
        tool_calls: input
            .tool_calls
            .iter()
            .map(|call| call.tool_call.tool_name.clone())
            .collect(),
        tool_activities,
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
            cwd: None,
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
            tool_results: Vec::new(),
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
            session_id: None,
            cwd: None,
        })
        .expect("valid");

        let projection = sample_turn_projection(false);
        assert_eq!(projection.reasoning, vec!["thinking"]);
        assert_eq!(projection.text, vec!["answer"]);
        assert_eq!(projection.tool_activities.len(), 1);
        assert_eq!(
            projection.tool_activities[0].status,
            UiToolActivityStatus::Waiting
        );
    }

    #[test]
    fn submit_user_input_accepts_optional_session_id() {
        let command = UiCommand::SubmitUserInput {
            text: "hello new session".to_owned(),
            session_id: Some(SessionId::new("webui-session-test")),
            cwd: None,
        };
        validate_command(&command).expect("valid command");
        let encoded = serde_json::to_string(&command).expect("json");
        assert!(encoded.contains("webui-session-test"));
        let decoded: UiCommand = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, command);
    }

    #[test]
    fn submit_user_input_carries_session_cwd_and_rejects_empty_cwd() {
        let command = UiCommand::SubmitUserInput {
            text: "hello cwd session".to_owned(),
            session_id: Some(SessionId::new("webui-session-cwd")),
            cwd: Some("/tmp/freehand-cwd".to_owned()),
        };
        validate_command(&command).expect("valid cwd command");
        let encoded = serde_json::to_string(&command).expect("json");
        assert!(encoded.contains("/tmp/freehand-cwd"));
        let decoded: UiCommand = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, command);

        let err = validate_command(&UiCommand::SubmitUserInput {
            text: "bad cwd".to_owned(),
            session_id: None,
            cwd: Some("   ".to_owned()),
        })
        .expect_err("blank cwd must be rejected");
        assert_eq!(err, UiProtocolError::EmptySessionCwd);
        assert_eq!(protocol_rejection(err).code, "empty_session_cwd");
    }

    #[test]
    fn create_session_rejects_empty_cwd() {
        let command = UiCommand::CreateSession {
            session_id: SessionId::new("webui-task-session"),
            title: Some("Task".to_owned()),
            cwd: Some("/tmp/freehand-cwd".to_owned()),
        };
        validate_command(&command).expect("valid task cwd command");
        let encoded = serde_json::to_string(&command).expect("json");
        assert!(encoded.contains("/tmp/freehand-cwd"));

        let err = validate_command(&UiCommand::CreateSession {
            session_id: SessionId::new("webui-task-empty-cwd"),
            title: Some("Task".to_owned()),
            cwd: Some("   ".to_owned()),
        })
        .expect_err("blank task cwd must be rejected");
        assert_eq!(err, UiProtocolError::EmptySessionCwd);
        assert_eq!(protocol_rejection(err).code, "empty_session_cwd");
    }

    #[test]
    fn session_list_and_transcript_project_session_cwd() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("webui-session-cwd");
        state.apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: TurnId::new("turn-cwd-1"),
            cwd: Some("/tmp/freehand-cwd".to_owned()),
            user_text: Some("run in cwd".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        }));

        match state.query(&UiCommand::QuerySessionList).expect("list") {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions[0].cwd.as_deref(), Some("/tmp/freehand-cwd"));
            }
            other => panic!("unexpected list result: {other:?}"),
        }
        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.cwd.as_deref(), Some("/tmp/freehand-cwd"));
                assert_eq!(
                    transcript.turns[0].cwd.as_deref(),
                    Some("/tmp/freehand-cwd")
                );
            }
            other => panic!("unexpected transcript result: {other:?}"),
        }
    }

    #[test]
    fn tool_activity_waits_until_matching_result_reentry() {
        let mut projection = sample_turn_projection(false);
        projection.terminal_text = None;
        projection.terminal_status = None;
        let items = public_conversation_items(&projection);
        let tool = items
            .iter()
            .find(|item| item.kind == UiConversationItemKind::ToolSummary)
            .expect("tool item");
        assert_eq!(tool.status, "waiting");
        assert_eq!(tool.title, "Run tool");
        assert_eq!(tool.body, "Run tool: search");
        assert_eq!(
            tool.display.as_ref().map(|display| display.kind.as_str()),
            Some("generic")
        );

        let completed = turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            cwd: None,
            user_text: Some("run the task".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: vec![ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_call: freehand_contracts::ToolCallContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    tool_name: "grep".to_owned(),
                    arguments: vec![freehand_contracts::ToolArgument {
                        name: "pattern".to_owned(),
                        value: serde_json::json!("needle"),
                    }],
                    arguments_complete: true,
                },
            }],
            tool_results: vec![ReasonReq05ToolResultReentry {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_result: freehand_contracts::ToolResultContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    status: freehand_contracts::ToolResultStatus::Success,
                    output: "result body rendered in public summary".to_owned(),
                },
            }],
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        });
        let completed_tool = public_conversation_items(&completed)
            .into_iter()
            .find(|item| item.kind == UiConversationItemKind::ToolSummary)
            .expect("completed tool item");
        assert_eq!(completed_tool.status, "completed");
        assert_eq!(completed_tool.title, "Search text");
        assert_eq!(completed_tool.body, "pattern=needle");
        assert_eq!(
            completed_tool
                .display
                .as_ref()
                .map(|display| display.kind.as_str()),
            Some("search")
        );
    }

    #[test]
    fn public_conversation_strips_hidden_control_status_blocks() {
        let mut projection = sample_turn_projection(false);
        projection.text = vec![
            concat!(
                "answer\n",
                "<<<freehand_status>>>\n",
                "{\"schema_version\":1,\"status\":{\"simple_request\":true}}\n",
                "<</freehand_status>>>"
            )
            .to_owned(),
        ];
        projection.terminal_text = Some(
            concat!(
                "final\n",
                "<<<freehand_status>>>\n",
                "{\"schema_version\":1,\"status\":{\"simple_request\":true}}\n",
                "<</freehand_status>>>"
            )
            .to_owned(),
        );

        let items = public_conversation_items(&projection);
        let encoded = serde_json::to_string(&items).expect("items json");

        assert!(!encoded.contains("freehand_status"));
        assert!(encoded.contains("answer"));
        assert!(encoded.contains("final"));
    }

    #[test]
    fn failed_tool_result_updates_same_activity_without_error_projection() {
        let projection = turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            cwd: None,
            user_text: Some("run the task".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: vec![ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_call: freehand_contracts::ToolCallContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    tool_name: "read_file".to_owned(),
                    arguments: vec![freehand_contracts::ToolArgument {
                        name: "path".to_owned(),
                        value: serde_json::json!("missing.txt"),
                    }],
                    arguments_complete: true,
                },
            }],
            tool_results: vec![ReasonReq05ToolResultReentry {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_result: freehand_contracts::ToolResultContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    status: freehand_contracts::ToolResultStatus::Failed,
                    output: "failure body rendered in public summary".to_owned(),
                },
            }],
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        });

        assert_eq!(projection.tool_activities.len(), 1);
        assert_eq!(
            projection.tool_activities[0].status,
            UiToolActivityStatus::Failed
        );
        assert_eq!(projection.terminal_status, None);
        assert!(projection.errors.is_empty());
        let cards = public_conversation_items(&projection);
        let tool_cards = cards
            .iter()
            .filter(|item| item.kind == UiConversationItemKind::ToolSummary)
            .collect::<Vec<_>>();
        assert_eq!(tool_cards.len(), 1);
        assert_eq!(tool_cards[0].status, "failed");
        assert_eq!(tool_cards[0].title, "Read file");
        assert_eq!(tool_cards[0].body, "path=missing.txt");
        assert!(
            cards
                .iter()
                .all(|item| item.kind != UiConversationItemKind::Error)
        );
    }

    #[test]
    fn failed_terminal_marks_waiting_tool_activity_failed() {
        let projection = turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            cwd: None,
            user_text: Some("run the task".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: vec![ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_call: freehand_contracts::ToolCallContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    tool_name: "ls".to_owned(),
                    arguments: vec![],
                    arguments_complete: true,
                },
            }],
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: Some(ReasonResp03TerminalEvent {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                status: TerminalStatus::Failed,
                summary: "tool failed explicitly".to_owned(),
            }),
            error_events: Vec::new(),
            slave_substream_card: false,
        });

        assert_eq!(
            projection.tool_activities[0].status,
            UiToolActivityStatus::Failed
        );
        let tool = public_conversation_items(&projection)
            .into_iter()
            .find(|item| item.kind == UiConversationItemKind::ToolSummary)
            .expect("tool item");
        assert_eq!(tool.status, "failed");
        assert_eq!(tool.title, "List directory");
        assert_eq!(tool.body, "path=.");
    }

    #[test]
    fn session_latest_status_does_not_call_text_only_turn_streaming() {
        let projection = UiTurnProjection {
            source: base_source(UiStreamKind::Turn),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            cwd: None,
            user_text: Some("run the task".to_owned()),
            model_request: None,
            reasoning: vec!["thinking".to_owned()],
            text: vec!["answer".to_owned()],
            tool_calls: Vec::new(),
            tool_activities: Vec::new(),
            usage: Vec::new(),
            terminal_status: None,
            terminal_text: None,
            errors: Vec::new(),
            slave_substream_card: false,
        };

        assert_eq!(session_latest_status(&projection), "active");
    }

    #[test]
    fn duplicate_tool_call_projection_updates_one_activity_card() {
        let tool_call = ReasonReq04ToolCall {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("ui.protocol"),
            agent_id: AgentId::new("agent-1"),
            tool_call: freehand_contracts::ToolCallContract {
                tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                tool_name: "ls".to_owned(),
                arguments: vec![],
                arguments_complete: true,
            },
        };
        let projection = turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            cwd: None,
            user_text: Some("run the task".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: vec![tool_call.clone(), tool_call],
            tool_results: vec![ReasonReq05ToolResultReentry {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_result: freehand_contracts::ToolResultContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    status: freehand_contracts::ToolResultStatus::Success,
                    output: "private output".to_owned(),
                },
            }],
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        });

        assert_eq!(projection.tool_activities.len(), 1);
        assert_eq!(
            projection.tool_activities[0].status,
            UiToolActivityStatus::Completed
        );
        let tool_cards = public_conversation_items(&projection)
            .into_iter()
            .filter(|item| item.kind == UiConversationItemKind::ToolSummary)
            .collect::<Vec<_>>();
        assert_eq!(tool_cards.len(), 1);
        assert_eq!(tool_cards[0].status, "completed");
        assert_eq!(tool_cards[0].tool_call_id.as_deref(), Some("tool-1"));
    }

    #[test]
    fn model_request_waiting_projection_clears_on_response_event() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-model-wait");
        let turn_id = TurnId::new("turn-model-wait");
        let waiting = state.apply_model_request_waiting(
            AgentId::new("agent-1"),
            "node-1".to_owned(),
            &session_id,
            &turn_id,
            Some("provider request built".to_owned()),
            false,
        );
        assert_eq!(
            waiting
                .model_request
                .as_ref()
                .map(|activity| activity.status),
            Some(UiModelRequestStatus::Waiting)
        );
        assert_eq!(
            waiting.model_request.as_ref().map(|activity| activity.kind),
            Some(UiModelRequestKind::Thinking)
        );
        assert_eq!(
            waiting
                .model_request
                .as_ref()
                .and_then(|activity| activity.detail.as_deref()),
            Some("provider request built")
        );

        let responded = state.apply_semantic_event(
            AgentId::new("agent-1"),
            "node-1".to_owned(),
            &ReasonResp01SemanticEvent {
                session_id,
                turn_id,
                trace_id: TraceId::new("trace-model-wait"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                kind: SemanticEventKind::Text,
                content: "model response arrived".to_owned(),
            },
            false,
        );
        assert_eq!(responded.model_request, None);
        assert_eq!(responded.text, vec!["model response arrived".to_owned()]);
    }

    #[test]
    fn schema_mismatch_projects_as_model_polishing_activity() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-schema-retry");
        let turn_id = TurnId::new("turn-schema-retry");

        let waiting = state.apply_completion_schema_retry_waiting(UiCompletionSchemaRetryWaiting {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id,
            turn_id,
            retry_index: 2,
            issue_summary: "evidence must be a string, got array".to_owned(),
            slave_substream_card: false,
        });

        let activity = waiting.model_request.expect("model request activity");
        assert_eq!(activity.status, UiModelRequestStatus::Waiting);
        assert_eq!(activity.kind, UiModelRequestKind::SchemaRetry);
        let detail = activity.detail.expect("detail");
        assert!(detail.contains("schema polishing #2"));
        assert!(detail.contains("evidence must be a string"));
        assert!(!detail.contains("Feedback sent to the model"));
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
    fn session_queries_return_ordered_transcript_without_cross_session_leakage() {
        let mut state = UiProtocolState::default();
        let mut first = sample_turn_projection(false);
        first.session_id = SessionId::new("session-a");
        first.turn_id = TurnId::new("runtime-turn-1-r2");
        first.source.source_turn_id = Some(first.turn_id.clone());
        first.user_text = Some("first prompt".to_owned());
        first.terminal_text = Some("first answer".to_owned());

        let mut second = sample_turn_projection(false);
        second.session_id = SessionId::new("session-a");
        second.turn_id = TurnId::new("runtime-turn-2-r2");
        second.source.source_turn_id = Some(second.turn_id.clone());
        second.user_text = Some("second prompt".to_owned());
        second.terminal_text = Some("second answer".to_owned());

        let mut tenth = sample_turn_projection(false);
        tenth.session_id = SessionId::new("session-a");
        tenth.turn_id = TurnId::new("runtime-turn-10-r2");
        tenth.source.source_turn_id = Some(tenth.turn_id.clone());
        tenth.user_text = Some("tenth prompt".to_owned());
        tenth.terminal_text = Some("tenth answer".to_owned());

        let mut other = sample_turn_projection(false);
        other.session_id = SessionId::new("session-b");
        other.turn_id = TurnId::new("runtime-turn-3");
        other.source.source_turn_id = Some(other.turn_id.clone());
        other.user_text = Some("other prompt".to_owned());

        state.apply_turn_projection(second.clone());
        state.apply_turn_projection(other);
        state.apply_turn_projection(tenth.clone());
        state.apply_turn_projection(first.clone());

        let list = state
            .query(&UiCommand::QuerySessionList)
            .expect("session list query");
        match list {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 2);
                let session_a = list
                    .sessions
                    .iter()
                    .find(|session| session.session_id.as_str() == "session-a")
                    .expect("session-a summary");
                assert_eq!(session_a.turn_count, 3);
                assert_eq!(
                    session_a.latest_turn_id.as_ref(),
                    Some(&TurnId::new("runtime-turn-10-r2"))
                );
            }
            other => panic!("unexpected query result: {other:?}"),
        }

        let transcript = state
            .query(&UiCommand::QuerySessionTurns {
                session_id: SessionId::new("session-a"),
            })
            .expect("session turns query");
        match transcript {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.session_id, SessionId::new("session-a"));
                assert_eq!(transcript.turns.len(), 3);
                assert_eq!(
                    transcript.turns[0].turn_id,
                    TurnId::new("runtime-turn-1-r2")
                );
                assert_eq!(
                    transcript.turns[1].turn_id,
                    TurnId::new("runtime-turn-2-r2")
                );
                assert_eq!(
                    transcript.turns[2].turn_id,
                    TurnId::new("runtime-turn-10-r2")
                );
                assert!(
                    transcript
                        .turns
                        .iter()
                        .all(|turn| turn.session_id.as_str() == "session-a")
                );
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
    fn tool_summary_carries_tool_call_identity() {
        let projection = sample_turn_projection(false);
        let tool = public_conversation_items(&projection)
            .into_iter()
            .find(|item| item.kind == UiConversationItemKind::ToolSummary)
            .expect("tool item");
        assert_eq!(tool.tool_call_id.as_deref(), Some("tool-1"));
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
            session_id: None,
            cwd: None,
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
            session_id: None,
            cwd: None,
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
    fn command_dispatch_envelope_routes_session_crud_to_persistence_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::RenameSession {
            session_id: SessionId::new("session-crud"),
            title: "Renamed".to_owned(),
        })
        .expect("envelope");
        assert_eq!(envelope.ingress.command_kind, "rename_session");
        assert_eq!(envelope.target_feature_id, "reason.persistence");
        assert_eq!(envelope.target_owner_module, "crates/freehand-reason");
    }

    #[test]
    fn command_dispatch_envelope_routes_session_rollback_to_persistence_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::RollbackLatestSessionTurn {
            session_id: SessionId::new("session-rollback"),
        })
        .expect("envelope");
        assert_eq!(
            envelope.ingress.command_kind,
            "rollback_latest_session_turn"
        );
        assert_eq!(envelope.target_feature_id, "reason.persistence");
        assert_eq!(envelope.target_owner_module, "crates/freehand-reason");

        let err = accept_command_ingress(&UiCommand::RollbackLatestSessionTurn {
            session_id: SessionId::new("   "),
        })
        .expect_err("blank session must be rejected");
        assert_eq!(err, UiProtocolError::EmptySessionId);
    }

    #[test]
    fn session_crud_validation_rejects_empty_title() {
        let err = accept_command_ingress(&UiCommand::RenameSession {
            session_id: SessionId::new("session-crud"),
            title: "   ".to_owned(),
        })
        .expect_err("empty title must fail");
        assert_eq!(err, UiProtocolError::EmptySessionTitle);
        assert_eq!(protocol_rejection(err).code, "empty_session_title");
    }

    #[test]
    fn session_metadata_projection_includes_empty_and_archived_sessions() {
        let mut state = UiProtocolState::default();
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: SessionId::new("session-empty"),
            title: Some("Empty session".to_owned()),
            archived: false,
            cwd: Some("/tmp".to_owned()),
        });
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: SessionId::new("session-archived"),
            title: Some("Archived session".to_owned()),
            archived: true,
            cwd: None,
        });

        match state
            .query(&UiCommand::QuerySessionList)
            .expect("active list")
        {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 1);
                assert_eq!(list.sessions[0].session_id, SessionId::new("session-empty"));
                assert_eq!(list.sessions[0].title.as_deref(), Some("Empty session"));
                assert!(!list.sessions[0].archived);
                assert_eq!(list.sessions[0].turn_count, 0);
            }
            other => panic!("unexpected query result: {other:?}"),
        }

        match state
            .query(&UiCommand::QueryArchivedSessionList)
            .expect("archived list")
        {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 1);
                assert_eq!(
                    list.sessions[0].session_id,
                    SessionId::new("session-archived")
                );
                assert!(list.sessions[0].archived);
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }

    #[test]
    fn static_dispatch_port_returns_dispatch_receipt() {
        let envelope = build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
            text: "run task".to_owned(),
            session_id: None,
            cwd: None,
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
    fn task_list_subscription_matches_runtime_projection_only() {
        let selector = subscription_selector(&UiCommand::SubscribeTaskList {
            status: Some("waiting_agent".to_owned()),
            agent_id: Some(AgentId::new("worker-1")),
        })
        .expect("task list selector");
        assert_eq!(selector.stream_kind, UiStreamKind::TaskList);
        assert_eq!(selector.target_turn_id, None);

        let projection = UiTaskListProjection {
            source_agent_id: AgentId::new("master"),
            status_filter: Some("waiting_agent".to_owned()),
            agent_filter: Some(AgentId::new("worker-1")),
            tasks: Vec::new(),
        };
        assert!(subscription_matches(
            &selector,
            &UiProjection::TaskList(projection),
            None,
        ));
        assert!(!subscription_matches(
            &selector,
            &UiProjection::Progress(TaskProgressSnapshot {
                source: UiSource {
                    source_agent_id: AgentId::new("master"),
                    source_node_id: "master-node".to_owned(),
                    source_turn_id: Some(TurnId::new("turn-1")),
                    stream_kind: UiStreamKind::Progress,
                },
                turn_id: TurnId::new("turn-1"),
                status_text: "running".to_owned(),
            }),
            None,
        ));

        let err = UiProtocolState::default()
            .query(&UiCommand::QueryTaskList {
                status: None,
                agent_id: None,
            })
            .expect_err("task query must stay runtime-owned");
        assert_eq!(err, UiProtocolError::StreamKindMismatch);
    }

    #[test]
    fn config_status_query_stays_runtime_owned_and_secret_free() {
        validate_command(&UiCommand::QueryConfigStatus).expect("valid query");
        let ingress_err = accept_command_ingress(&UiCommand::QueryConfigStatus)
            .expect_err("config status query must not enter command ingress");
        assert_eq!(ingress_err, UiProtocolError::IngressCommandKindMismatch);

        let query_err = UiProtocolState::default()
            .query(&UiCommand::QueryConfigStatus)
            .expect_err("config status must stay runtime-owned");
        assert_eq!(query_err, UiProtocolError::StreamKindMismatch);

        let result = UiQueryResult::ConfigStatus(UiConfigStatusProjection {
            agent_name: "master".to_owned(),
            agent_mode: "master".to_owned(),
            node_id: "master-node".to_owned(),
            paired_agent_name: "worker".to_owned(),
            paired_agent_mode: "slave".to_owned(),
            paired_node_id: "worker-node".to_owned(),
            provider_id: "minimonth".to_owned(),
            provider_type: "anthropic".to_owned(),
            provider_protocol: "messages".to_owned(),
            provider_base_url_host: "api.example.test".to_owned(),
            default_model: "MiniMax-M2".to_owned(),
            provider_auth_type: "apikey".to_owned(),
            provider_auth_source: "env".to_owned(),
            restart_required_on_change: true,
        });
        let encoded = serde_json::to_string(&result).expect("config status json");
        assert!(encoded.contains("ConfigStatus"));
        assert!(encoded.contains("provider_auth_source"));
        assert!(!encoded.contains("api_key"));
        assert!(!encoded.contains("pair_token"));
        assert!(!encoded.contains("secret"));
    }

    #[test]
    fn provider_config_update_routes_to_config_owner_and_rejects_empty_fields() {
        let command = UiCommand::UpdateProviderConfig {
            update: UiProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "minimax".to_owned(),
                provider_type: "openai".to_owned(),
                provider_protocol: "responses".to_owned(),
                base_url: "https://api.minimaxi.com/v1".to_owned(),
                default_model: "MiniMax-M3".to_owned(),
                api_key_env: "MINIMAX_API_KEY".to_owned(),
            },
        };
        validate_command(&command).expect("valid provider update command");
        let envelope = build_command_dispatch_envelope(&command).expect("dispatch envelope");
        assert_eq!(envelope.target_feature_id, "config.core");
        assert_eq!(envelope.target_owner_module, "crates/freehand-config");
        assert_eq!(envelope.ingress.command_kind, "update_provider_config");

        let err = validate_command(&UiCommand::UpdateProviderConfig {
            update: UiProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "minimax".to_owned(),
                provider_type: "openai".to_owned(),
                provider_protocol: "responses".to_owned(),
                base_url: "https://api.minimaxi.com/v1".to_owned(),
                default_model: String::new(),
                api_key_env: "MINIMAX_API_KEY".to_owned(),
            },
        })
        .expect_err("empty model rejected");
        assert_eq!(err, UiProtocolError::EmptyProviderDefaultModel);
        assert_eq!(protocol_rejection(err).code, "empty_provider_default_model");
    }

    #[test]
    fn provider_config_update_serialization_does_not_include_secret_field() {
        let command = UiCommand::UpdateProviderConfig {
            update: UiProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "minimax".to_owned(),
                provider_type: "openai".to_owned(),
                provider_protocol: "responses".to_owned(),
                base_url: "https://api.minimaxi.com/v1".to_owned(),
                default_model: "MiniMax-M3".to_owned(),
                api_key_env: "MINIMAX_API_KEY".to_owned(),
            },
        };
        let encoded = serde_json::to_string(&command).expect("update command json");
        assert!(encoded.contains("UpdateProviderConfig"));
        assert!(encoded.contains("api_key_env"));
        assert!(!encoded.contains("api_key\""));
        assert!(!encoded.contains("secret"));
        assert!(!encoded.contains("sk-"));
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

    #[test]
    fn adp_request_and_response_frames_roundtrip() {
        let request = UiAdpRequest::Query {
            request_id: "req-1".to_owned(),
            query: UiCommand::QueryConfigStatus,
        };
        let request_json = serde_json::to_string(&request).expect("request json");
        assert!(request_json.contains("\"kind\":\"query\""));
        assert!(request_json.contains("QueryConfigStatus"));
        let decoded_request: UiAdpRequest =
            serde_json::from_str(&request_json).expect("decoded request");
        assert_eq!(decoded_request, request);

        let response = UiAdpResponse::Failure {
            request_id: "req-1".to_owned(),
            failure: UiAdpFailure {
                code: "protocol_mismatch".to_owned(),
                message: "query frame rejected".to_owned(),
                retryable: false,
            },
        };
        let response_json = serde_json::to_string(&response).expect("response json");
        assert!(response_json.contains("\"kind\":\"failure\""));
        let decoded_response: UiAdpResponse =
            serde_json::from_str(&response_json).expect("decoded response");
        assert_eq!(decoded_response, response);
    }
}
