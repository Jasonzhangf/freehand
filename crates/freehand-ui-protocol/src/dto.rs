use freehand_blocks::ToolDisplayProjection;
use freehand_contracts::{
    AgentId, SearchClaimDelivery, SearchDomainPlanDelivery, SearchEvidenceDelivery,
    SearchEvidenceTerminal, SearchEvidenceTurnDelivery, SearchEvidenceTurnStatus,
    SearchUnconfirmedDelivery, SearchVerificationDelivery, SessionId, TerminalStatus, TurnId,
};
use serde::{Deserialize, Serialize};

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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<UiSubmitMetadata>,
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
    QuerySessionListPage {
        archived: bool,
        page: UiSessionListPageRequest,
    },
    QuerySessionTurns {
        session_id: SessionId,
    },
    QuerySessionTurnsPage {
        session_id: SessionId,
        page: UiSessionTurnsPageRequest,
    },
    QuerySessionSearch {
        query: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
    },
    QueryMemory {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        query: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<SessionId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sort: Option<UiMemorySort>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<usize>,
    },
    QueryConfigStatus,
    QueryTaskList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<AgentId>,
    },
    QueryTaskBoard {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<AgentId>,
        #[serde(default)]
        include_terminal: bool,
    },
    QueryEventInbox {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after_cursor: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
    },
    QueryAgentBoard,
    QueryAgentLifecycle {
        agent_id: AgentId,
    },
    QueryTaskHistory {
        task_id: String,
    },
    QueryWorkerControl {
        task_id: String,
        execution_id: String,
    },
    QueryTimerList {
        #[serde(default)]
        include_terminal: bool,
    },
    QueryToolRegistry,
    QueryDiagnostics,
    PullAccountConfig,
    PushAccountConfig,
    AddToMemory {
        session_id: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        turn_id: Option<TurnId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
        content: String,
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
    UpsertProviderConfig {
        update: UiProviderConfigUpdate,
    },
    UpsertModelGroupConfig {
        group: UiModelGroupConfigUpdate,
    },
    UpdateAgentModelGroupSelection {
        selection: UiAgentModelGroupSelectionUpdate,
    },
    TestProviderWebSearch {
        provider_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        query: Option<String>,
    },
    ScheduleTimer {
        timer: UiTimerScheduleCommand,
    },
    CancelTimer {
        timer_id: String,
    },
    UpdateAgentProviderSelection {
        selection: UiAgentProviderSelectionUpdate,
    },
    UpdateAgentResourceConfig {
        update: UiAgentResourceConfigUpdate,
    },
    CreateTask {
        task: UiTaskCreateCommand,
    },
    CreateTaskAgent {
        agent: UiTaskAgentCreateCommand,
    },
    AssignTask {
        assignment: UiTaskAssignCommand,
    },
    ClaimNextTask {
        claim: UiTaskClaimCommand,
    },
    SubmitTaskReview {
        review: UiTaskReviewCommand,
    },
    RejectTaskReview {
        rejection: UiTaskReviewRejectionCommand,
    },
    ApproveTaskReview {
        task_id: String,
    },
    CloseTask {
        task_id: String,
    },
    ApplyExecutionFact {
        fact: UiExecutionFactCommand,
    },
    RunSchedulerTick {
        tick: UiSchedulerTickCommand,
    },
    RunMasterPoll {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after_cursor: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
        #[serde(default)]
        include_terminal: bool,
        #[serde(default)]
        replay_from_start: bool,
    },
    QueryMasterPoll {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after_cursor: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
        #[serde(default)]
        include_terminal: bool,
        #[serde(default)]
        replay_from_start: bool,
    },
    WorkerControl {
        control: UiWorkerControlCommand,
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
    CompactSessionContext {
        session_id: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
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
    #[serde(default)]
    pub web_search: String,
    pub api_key_env: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAgentProviderSelectionUpdate {
    pub agent_name: String,
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_provider_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiModelRouteUpdate {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiModelWeightedRouteUpdate {
    pub provider_id: String,
    pub model: String,
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiModelGroupConfigUpdate {
    pub agent_name: String,
    pub group_id: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub label: String,
    pub primary: UiModelRouteUpdate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<UiModelRouteUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<UiModelRouteUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<UiModelRouteUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<UiModelRouteUpdate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_balance: Vec<UiModelWeightedRouteUpdate>,
    pub context_window_tokens: u32,
    pub compaction_threshold_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAgentModelGroupSelectionUpdate {
    pub agent_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAgentResourceConfigUpdate {
    pub agent_name: String,
    pub resource_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UiSubmitMetadata {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<UiInputAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiInputAttachment {
    pub attachment_id: String,
    pub kind: UiInputAttachmentKind,
    pub media_type: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_base64: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiInputAttachmentKind {
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAttachmentMetadataProjection {
    pub attachment_id: String,
    pub kind: UiInputAttachmentKind,
    pub media_type: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

impl UiInputAttachment {
    pub fn metadata_projection(&self) -> UiAttachmentMetadataProjection {
        UiAttachmentMetadataProjection {
            attachment_id: self.attachment_id.clone(),
            kind: self.kind,
            media_type: self.media_type.clone(),
            name: self.name.clone(),
            size_bytes: self.size_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTurnProjection {
    pub source: UiSource,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<UiTurnTimingProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub user_text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<UiAttachmentMetadataProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_request: Option<UiModelRequestActivity>,
    pub reasoning: Vec<String>,
    pub text: Vec<String>,
    pub tool_calls: Vec<String>,
    pub tool_activities: Vec<UiToolActivity>,
    pub usage: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_projection: Option<UiUsageProjection>,
    pub terminal_status: Option<TerminalStatus>,
    pub terminal_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_options: Option<Vec<String>>,
    pub errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_evidence: Option<UiSearchEvidenceProjection>,
    pub slave_substream_card: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSearchEvidenceProjection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_plan: Option<SearchDomainPlanDelivery>,
    pub deliveries: Vec<SearchEvidenceDelivery>,
    pub verified_sources: Vec<SearchVerificationDelivery>,
    pub unconfirmed: Vec<SearchUnconfirmedDelivery>,
    pub claims: Vec<SearchClaimDelivery>,
    pub status: SearchEvidenceTurnStatus,
    pub summary_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<SearchEvidenceTerminal>,
}

impl From<&SearchEvidenceTurnDelivery> for UiSearchEvidenceProjection {
    fn from(delivery: &SearchEvidenceTurnDelivery) -> Self {
        Self {
            domain_plan: delivery.domain_plan.clone(),
            deliveries: delivery.deliveries.clone(),
            verified_sources: delivery.verified_sources.clone(),
            unconfirmed: delivery.unconfirmed.clone(),
            claims: delivery.claims.clone(),
            status: delivery.status,
            summary_ready: delivery.summary_ready,
            summary: delivery.summary.clone(),
            blocked_reason: delivery.blocked_reason.clone(),
            terminal: delivery.terminal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiUsageProjection {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    /// Cache hit rate in basis points (0-10000; 8000 == 80%).
    pub cache_hit_rate_bps: u64,
    /// Provider-normalized total input tokens for this turn.
    pub context_tokens: u64,
    /// Context compaction applied before this turn, in tokens.
    #[serde(default)]
    pub compacted_tokens: u64,
    /// Raw model-name or provider label if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTurnTimingProjection {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiModelRequestActivity {
    pub status: UiModelRequestStatus,
    #[serde(default)]
    pub kind: UiModelRequestKind,
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<UiModelTransportActivity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiModelTransportActivity {
    pub kind: UiModelTransportKind,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiModelTransportKind {
    ProviderRetry,
    ProviderFailover,
}

impl UiModelTransportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            UiModelTransportKind::ProviderRetry => "provider_retry",
            UiModelTransportKind::ProviderFailover => "provider_failover",
        }
    }
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
    pub transport: Option<UiModelTransportActivity>,
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
    pub activity_unix_seconds: u64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiSessionListPageDirection {
    Latest,
    Older,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionListPageRequest {
    pub direction: UiSessionListPageDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionListPageInfo {
    pub has_older: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub unavailable_sessions: Vec<SessionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionListPageProjection {
    pub sessions: Vec<UiSessionSummary>,
    pub page: UiSessionListPageInfo,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiSessionTurnsPageDirection {
    Latest,
    Older,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionTurnsPageRequest {
    pub direction: UiSessionTurnsPageDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_turn_id: Option<TurnId>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionTurnsPageInfo {
    pub has_older: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oldest_turn_id: Option<TurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest_turn_id: Option<TurnId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionTranscriptPageProjection {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub turns: Vec<UiTurnProjection>,
    pub page: UiSessionTurnsPageInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionSearchProjection {
    pub query: String,
    pub results: Vec<UiSessionSearchResultProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionSearchResultProjection {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_turn_id: Option<TurnId>,
    pub latest_status: String,
    pub snippet: String,
    pub matched_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_matches: Vec<UiSessionSearchChildProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSessionSearchChildProjection {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_turn_id: Option<TurnId>,
    pub latest_status: String,
    pub snippet: String,
    pub matched_fields: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiMemorySort {
    Recent,
    Oldest,
    Relevance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiMemoryProjection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub sort: UiMemorySort,
    pub entries: Vec<UiMemoryEntryProjection>,
    pub total_matching: u64,
    pub has_older: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiMemoryEntryProjection {
    pub id: u64,
    pub created_at_unix_seconds: u64,
    pub agent_id: AgentId,
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<TurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    pub content: String,
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub execution_profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attached_session_ids: Vec<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_session_id: Option<SessionId>,
    pub assignee_agent_id: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_execution_id: Option<String>,
    pub created_at: u64,
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
pub struct UiTaskBoardProjection {
    pub source_agent_id: AgentId,
    pub status_filter: Option<String>,
    pub agent_filter: Option<AgentId>,
    pub include_terminal: bool,
    pub tasks: Vec<UiTaskSnapshotProjection>,
    pub agents: Vec<UiAgentSnapshotProjection>,
    pub blocked: Vec<UiTaskSnapshotProjection>,
    pub review_ready: Vec<UiTaskSnapshotProjection>,
    pub stale: Vec<UiTaskSnapshotProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAgentSnapshotProjection {
    pub agent_id: AgentId,
    pub status: String,
    pub current_task_id: Option<String>,
    pub current_cwd: Option<String>,
    pub running_tasks: u32,
    pub queued_tasks: u32,
    pub last_seen_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAgentLifecycleProjection {
    pub agent_id: AgentId,
    pub role: String,
    pub alive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<Box<UiAgentProcessProjection>>,
    pub state: String,
    pub current_task_id: Option<String>,
    pub current_execution_id: Option<String>,
    pub current_turn_id: Option<TurnId>,
    pub current_activity: Option<UiAgentLifecycleActivityProjection>,
    pub last_activity: Option<UiAgentLifecycleActivityProjection>,
    pub model_request_count: u64,
    pub model_retry_count: u64,
    pub tool_call_count: u64,
    pub tool_failure_count: u64,
    pub schema_polish_count: u64,
    pub provider_error_count: u64,
    pub blocked_count: u64,
    pub current_model: Option<String>,
    pub last_seen_at: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAgentProcessProjection {
    pub process_id: Option<u32>,
    pub process_instance_id: Option<String>,
    pub started_at: Option<u64>,
    pub heartbeat_at: Option<u64>,
    pub restart_count: u64,
    pub next_check_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAgentLifecycleActivityProjection {
    pub kind: String,
    pub semantic_summary: String,
    pub target: Option<String>,
    pub elapsed_ms: u64,
    pub tool_name: Option<String>,
    pub model: Option<String>,
    pub retry_count: Option<u32>,
    pub visibility: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAgentBoardProjection {
    pub source_agent_id: AgentId,
    pub agents: Vec<UiAgentLifecycleProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskEventInboxEntryProjection {
    pub cursor: String,
    pub event_id: String,
    pub kind: String,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub created_at: u64,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskEventInboxProjection {
    pub source_agent_id: AgentId,
    pub generated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub events: Vec<UiTaskEventInboxEntryProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiMasterPollClassificationProjection {
    pub kind: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiMasterPollProjection {
    pub source_agent_id: AgentId,
    pub generated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persisted_cursor: Option<String>,
    pub event_inbox: UiTaskEventInboxProjection,
    pub task_board: UiTaskBoardProjection,
    pub agent_board: UiAgentBoardProjection,
    pub classifications: Vec<UiMasterPollClassificationProjection>,
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
    #[serde(default)]
    pub public_message: Option<String>,
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
    pub paired_agents: Vec<UiConfigPeerProjection>,
    #[serde(default)]
    pub local_agent_directory: Vec<UiLocalAgentProjection>,
    #[serde(default)]
    pub provider_registry: Vec<UiProviderConfigSummaryProjection>,
    #[serde(default)]
    pub model_group_registry: Vec<UiModelGroupConfigProjection>,
    pub agent_resource_count: usize,
    pub agent_resource_limit: usize,
    pub agent_resource_provider_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_resource_provider_id: Option<String>,
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_group_id: Option<String>,
    #[serde(default)]
    pub route_source: String,
    pub provider_type: String,
    pub provider_protocol: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_base_url: String,
    pub provider_base_url_host: String,
    pub default_model: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_web_search: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_web_search_effective: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_web_search_reason: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_web_search_route_summary: String,
    pub provider_auth_type: String,
    pub provider_auth_source: String,
    pub restart_required_on_change: bool,
    #[serde(default)]
    pub account_config_sync: UiAccountConfigSyncProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiAccountConfigSyncProjection {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_document: Option<UiAccountConfigDocumentSummaryProjection>,
}

impl Default for UiAccountConfigSyncProjection {
    fn default() -> Self {
        Self {
            status: "not_configured".to_owned(),
            account_id: None,
            revision: None,
            etag: None,
            updated_at: None,
            error_message: None,
            server_document: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiAccountConfigDocumentSummaryProjection {
    pub provider_count: usize,
    pub model_group_count: usize,
    pub relay_endpoint_count: usize,
    pub remote_daemon_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiLocalAgentProjection {
    pub agent_name: String,
    pub agent_mode: String,
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_web_url: Option<String>,
    #[serde(default)]
    pub is_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiProviderConfigSummaryProjection {
    pub provider_id: String,
    pub enabled: bool,
    pub provider_type: String,
    pub provider_protocol: String,
    pub provider_base_url: String,
    pub provider_base_url_host: String,
    pub default_model: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_web_search: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_web_search_effective: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_web_search_reason: String,
    pub provider_auth_type: String,
    pub provider_auth_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiModelRouteProjection {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiModelWeightedRouteProjection {
    pub provider_id: String,
    pub model: String,
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiModelGroupConfigProjection {
    pub group_id: String,
    pub enabled: bool,
    pub label: String,
    pub primary: UiModelRouteProjection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<UiModelRouteProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<UiModelRouteProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<UiModelRouteProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<UiModelRouteProjection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_balance: Vec<UiModelWeightedRouteProjection>,
    pub context_window_tokens: u32,
    pub compaction_threshold_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConfigPeerProjection {
    pub agent_name: String,
    pub agent_mode: String,
    pub node_id: String,
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_group_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_web_url: Option<String>,
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub execution_profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<TurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<UiTaskDispatchCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum UiTaskDispatchCommand {
    None,
    SelfAgent,
    Agent { agent_id: AgentId },
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
pub struct UiTaskAgentCreateCommand {
    pub agent_id: AgentId,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskAssignCommand {
    pub task_id: String,
    pub agent_id: AgentId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskClaimCommand {
    pub agent_id: AgentId,
    pub execution_id: String,
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTaskReviewRejectionCommand {
    pub task_id: String,
    pub reject_reason: String,
    #[serde(default)]
    pub next_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiExecutionFactKind {
    Running {
        phase: String,
        summary: String,
        evidence: Vec<String>,
    },
    Recovering {
        summary: String,
        evidence: Vec<String>,
        retry_count: u32,
    },
    Blocked {
        reason: String,
        evidence: Vec<String>,
    },
    Interrupted {
        reason: String,
        evidence: Vec<String>,
    },
    Failed {
        reason: String,
        evidence: Vec<String>,
    },
    ReviewReady {
        summary: String,
        deliverables: Vec<String>,
        evidence: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiExecutionFactCommand {
    pub execution_id: String,
    pub task_id: String,
    pub agent_id: AgentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<TurnId>,
    pub kind: UiExecutionFactKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSchedulerTickCommand {
    pub stale_after_seconds: u64,
    pub soft_timeout_seconds: u64,
    pub hard_timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTimerScheduleCommand {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_id: Option<String>,
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_at_unix_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<UiTimerRepeatCommand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runs: Option<u32>,
    pub reason: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_session_id: Option<SessionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiTimerRepeatCommand {
    Interval {
        interval_seconds: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_runs: Option<u32>,
    },
    Daily {
        time_of_day_seconds_local: u32,
        #[serde(default)]
        skip_weekends: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_runs: Option<u32>,
    },
    Weekly {
        time_of_day_seconds_local: u32,
        weekdays: Vec<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_runs: Option<u32>,
    },
    Cron {
        expression: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_runs: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTimerProjection {
    pub timer_id: String,
    pub agent_id: AgentId,
    pub status: String,
    pub reason: String,
    pub prompt: String,
    pub next_due_at: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub fired_count: u32,
    pub max_runs: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repeat_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repeat_summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_turn_id: Option<TurnId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTimerEventProjection {
    pub event_id: String,
    pub timer_id: String,
    pub event_type: String,
    pub occurred_at: u64,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTimerListProjection {
    pub source_agent_id: AgentId,
    pub generated_at: u64,
    pub include_terminal: bool,
    #[serde(default)]
    pub timers: Vec<UiTimerProjection>,
    #[serde(default)]
    pub events: Vec<UiTimerEventProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiToolRegistryProjection {
    pub source_agent_id: AgentId,
    pub generated_at: u64,
    pub registry_version: String,
    #[serde(default)]
    pub guidance: Vec<String>,
    #[serde(default)]
    pub tools: Vec<UiToolRegistryToolProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiToolRegistryToolProjection {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub read_only: bool,
    pub implemented: bool,
    pub execution_scope: String,
    pub exposed_to_master: bool,
    pub exposed_to_worker: bool,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub guidance: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiDiagnosticsProjection {
    pub source_agent_id: AgentId,
    pub generated_at: u64,
    pub runtime_home: String,
    pub logs_dir: String,
    #[serde(default)]
    pub files: Vec<UiDiagnosticLogFileProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiDiagnosticLogFileProjection {
    pub name: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: Option<u64>,
    #[serde(default)]
    pub tail_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiWorkerControlCommand {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_id: Option<String>,
    pub task_id: String,
    pub execution_id: String,
    pub agent_id: AgentId,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiWorkerControlEventProjection {
    pub control_id: String,
    pub op: String,
    pub status: String,
    pub task_id: String,
    pub execution_id: String,
    pub agent_id: AgentId,
    pub created_at: u64,
    pub summary: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiWorkerControlProjection {
    pub source_agent_id: AgentId,
    pub generated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<UiWorkerControlEventProjection>,
    #[serde(default)]
    pub events: Vec<UiWorkerControlEventProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<UiTaskSnapshotProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<UiAgentSnapshotProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<UiAgentLifecycleProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_event: Option<UiTaskLedgerEventProjection>,
}
