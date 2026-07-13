//! Task orchestration truth, persistence, startup recovery, and agent registry skeleton.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_contracts::{AgentId, SessionId, TraceId, TurnId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Created,
    WaitingAgent,
    Assigned,
    Running,
    Interrupted,
    Paused,
    Blocked,
    ReviewSubmitted,
    Approved,
    Rejected,
    Failed,
    Cancelled,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Available,
    Busy,
    Paused,
    Offline,
    Closing,
    Closed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAssignee {
    pub agent_id: AgentId,
    pub assignment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskReviewState {
    pub status: String,
    pub submitted_at: Option<u64>,
    pub reviewer_agent_id: Option<AgentId>,
    pub decision: Option<String>,
    pub reject_reason: Option<String>,
    pub next_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskParentRef {
    pub session_id: Option<SessionId>,
    pub turn_id: Option<TurnId>,
    pub trace_id: Option<TraceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub schema_version: u32,
    pub task_id: TaskId,
    pub status: TaskStatus,
    pub title: String,
    pub content: String,
    pub goal: String,
    pub deliverables: Vec<String>,
    pub acceptance: Vec<String>,
    pub priority: i64,
    pub target_cwd: Option<String>,
    pub assignee: Option<TaskAssignee>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_execution_id: Option<String>,
    pub review: TaskReviewState,
    pub parent: TaskParentRef,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_progress_at: Option<u64>,
    pub last_event_seq: u64,
    pub last_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub schema_version: u32,
    pub agent_id: AgentId,
    pub status: AgentStatus,
    pub current_task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_execution_id: Option<String>,
    pub current_cwd: Option<String>,
    pub capabilities: Vec<String>,
    pub last_seen_at: u64,
    pub running_tasks: u32,
    pub queued_tasks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLifecycleState {
    Idle,
    Assigned,
    Running,
    Progressing,
    ModelThinking,
    ToolRunning,
    Recovering,
    Blocked,
    WaitingReview,
    Retrying,
    Approved,
    Closed,
    Failed,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLifecycleActivity {
    pub kind: String,
    pub semantic_summary: String,
    pub target: Option<String>,
    pub started_at: u64,
    pub elapsed_ms: u64,
    pub tool_name: Option<String>,
    pub model: Option<String>,
    pub retry_count: Option<u32>,
    pub visibility: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLifecycleStats {
    pub turn_count: u64,
    pub model_request_count: u64,
    pub model_retry_count: u64,
    pub tool_call_count: u64,
    pub tool_failure_count: u64,
    pub schema_polish_count: u64,
    pub provider_error_count: u64,
    pub blocked_count: u64,
    pub current_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLifecycleSnapshot {
    pub schema_version: u32,
    pub agent_id: AgentId,
    pub role: String,
    pub alive: bool,
    pub state: AgentLifecycleState,
    pub state_entered_at: u64,
    pub elapsed_ms: u64,
    pub current_task_id: Option<TaskId>,
    pub current_execution_id: Option<String>,
    pub current_turn_id: Option<TurnId>,
    pub current_activity: Option<AgentLifecycleActivity>,
    pub last_activity: Option<AgentLifecycleActivity>,
    pub stats: AgentLifecycleStats,
    pub last_seen_at: u64,
    pub next_check_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentBoardProjection {
    pub schema_version: u32,
    pub generated_at: u64,
    pub agents: Vec<AgentLifecycleSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentLifecycleEvent {
    ModelThinking {
        agent_id: AgentId,
        task_id: Option<TaskId>,
        turn_id: Option<TurnId>,
        model: String,
    },
    ToolRunning {
        agent_id: AgentId,
        task_id: Option<TaskId>,
        turn_id: Option<TurnId>,
        tool_name: String,
        target: Option<String>,
    },
    Recovering {
        agent_id: AgentId,
        task_id: Option<TaskId>,
        turn_id: Option<TurnId>,
        summary: String,
    },
    Blocked {
        agent_id: AgentId,
        task_id: Option<TaskId>,
        turn_id: Option<TurnId>,
        reason: String,
    },
}

impl AgentLifecycleEvent {
    pub fn agent_id(&self) -> &AgentId {
        match self {
            Self::ModelThinking { agent_id, .. }
            | Self::ToolRunning { agent_id, .. }
            | Self::Recovering { agent_id, .. }
            | Self::Blocked { agent_id, .. } => agent_id,
        }
    }

    pub fn task_id(&self) -> Option<&TaskId> {
        match self {
            Self::ModelThinking { task_id, .. }
            | Self::ToolRunning { task_id, .. }
            | Self::Recovering { task_id, .. }
            | Self::Blocked { task_id, .. } => task_id.as_ref(),
        }
    }

    pub fn turn_id(&self) -> Option<&TurnId> {
        match self {
            Self::ModelThinking { turn_id, .. }
            | Self::ToolRunning { turn_id, .. }
            | Self::Recovering { turn_id, .. }
            | Self::Blocked { turn_id, .. } => turn_id.as_ref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskBoardQuery {
    pub status: Option<TaskStatus>,
    pub assignee: Option<AgentId>,
    pub include_terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskBoardProjection {
    pub schema_version: u32,
    pub generated_at: u64,
    pub tasks: Vec<TaskSnapshot>,
    pub agents: Vec<AgentSnapshot>,
    pub blocked: Vec<TaskSnapshot>,
    pub review_ready: Vec<TaskSnapshot>,
    pub stale: Vec<TaskSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionFactKind {
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
    ReviewReady {
        summary: String,
        deliverables: Vec<String>,
        evidence: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionFact {
    pub execution_id: String,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub turn_id: Option<TurnId>,
    pub occurred_at: u64,
    pub kind: ExecutionFactKind,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SchedulerFactKind {
    Stale { idle_seconds: u64 },
    SoftTimeout { elapsed_seconds: u64 },
    HardTimeout { elapsed_seconds: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerFact {
    pub task_id: TaskId,
    pub agent_id: Option<AgentId>,
    pub observed_at: u64,
    pub fact: SchedulerFactKind,
    pub recommendation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerTickRequest {
    pub now: u64,
    pub stale_after_seconds: u64,
    pub soft_timeout_seconds: u64,
    pub hard_timeout_seconds: u64,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerTickOutcome {
    pub facts: Vec<SchedulerFact>,
    pub events: Vec<TaskLedgerEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLease {
    pub schema_version: u32,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub lease_id: String,
    pub status: String,
    pub acquired_at: u64,
    pub heartbeat_at: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLedgerEvent {
    pub schema_version: u32,
    pub task_id: TaskId,
    pub seq: u64,
    pub event_id: String,
    pub event_type: String,
    pub from_status: Option<TaskStatus>,
    pub to_status: TaskStatus,
    pub timestamp: u64,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEventInboxQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_cursor: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEventInboxEntry {
    pub schema_version: u32,
    pub cursor: String,
    pub event_id: String,
    pub kind: String,
    pub task_id: TaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub created_at: u64,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEventInboxProjection {
    pub schema_version: u32,
    pub generated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub events: Vec<TaskEventInboxEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MasterPollRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_cursor: Option<String>,
    pub limit: usize,
    pub include_terminal: bool,
    pub replay_from_start: bool,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MasterPollClassification {
    pub kind: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MasterPollOutcome {
    pub schema_version: u32,
    pub generated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persisted_cursor: Option<String>,
    pub include_terminal: bool,
    pub event_inbox: TaskEventInboxProjection,
    pub task_board: TaskBoardProjection,
    pub agent_board: AgentBoardProjection,
    pub classifications: Vec<MasterPollClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerControlOp {
    QueryStatus,
    AskAtSafePoint,
    AddConstraint,
    RequestCheckpoint,
    RequestSubmissionNow,
    Pause,
    Resume,
    Cancel,
}

impl WorkerControlOp {
    fn event_type(&self) -> &'static str {
        match self {
            Self::QueryStatus => "WorkerControlStatusQueried",
            Self::AskAtSafePoint => "WorkerControlSafePointQuestionQueued",
            Self::AddConstraint => "WorkerControlConstraintQueued",
            Self::RequestCheckpoint => "WorkerControlCheckpointRequested",
            Self::RequestSubmissionNow => "WorkerControlSubmissionRequested",
            Self::Pause => "WorkerControlPauseRequested",
            Self::Resume => "WorkerControlResumeRequested",
            Self::Cancel => "WorkerControlCancelRequested",
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::QueryStatus => "query_status",
            Self::AskAtSafePoint => "ask_at_safe_point",
            Self::AddConstraint => "add_constraint",
            Self::RequestCheckpoint => "request_checkpoint",
            Self::RequestSubmissionNow => "request_submission_now",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerControlRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_id: Option<String>,
    pub task_id: TaskId,
    pub execution_id: String,
    pub agent_id: AgentId,
    pub op: WorkerControlOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerControlEvent {
    pub schema_version: u32,
    pub control_id: String,
    pub op: WorkerControlOp,
    pub status: String,
    pub task_id: TaskId,
    pub execution_id: String,
    pub agent_id: AgentId,
    pub created_at: u64,
    pub summary: String,
    pub payload: Value,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerControlProjection {
    pub schema_version: u32,
    pub generated_at: u64,
    pub event: WorkerControlEvent,
    pub task: TaskSnapshot,
    pub agent: AgentSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<AgentLifecycleSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_event: Option<TaskLedgerEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActor {
    pub agent_id: AgentId,
    pub source: String,
    pub session_id: Option<SessionId>,
    pub turn_id: Option<TurnId>,
    pub trace_id: Option<TraceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskWatermark {
    pub metadata_id: Option<String>,
    pub hook: Option<String>,
    pub action_tool_call_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCreateRequest {
    pub task_id: Option<TaskId>,
    pub title: String,
    pub content: String,
    pub goal: String,
    pub deliverables: Vec<String>,
    pub acceptance: Vec<String>,
    pub priority: i64,
    pub target_cwd: Option<String>,
    pub dispatch: TaskDispatchRequest,
    pub parent: TaskParentRef,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TaskDispatchRequest {
    None,
    SelfAgent,
    Agent { agent_id: AgentId },
    Auto { allow_create_agent: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCreateOutcome {
    pub task: TaskSnapshot,
    pub events: Vec<TaskLedgerEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskListQuery {
    pub status: Option<TaskStatus>,
    pub assignee: Option<AgentId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMutationRequest {
    pub task_id: TaskId,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskHeartbeatRequest {
    pub task_id: TaskId,
    pub ttl_seconds: u64,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAssignRequest {
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCreateRequest {
    pub agent_id: AgentId,
    pub capabilities: Vec<String>,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMutationRequest {
    pub agent_id: AgentId,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskClaimRequest {
    pub agent_id: AgentId,
    pub execution_id: String,
    pub ttl_seconds: u64,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskExecutionRecordRequest {
    pub task_id: TaskId,
    pub phase: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskReviewSubmission {
    pub task_id: TaskId,
    pub summary: String,
    pub deliverables: Vec<String>,
    pub evidence: Vec<String>,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskReviewRejection {
    pub task_id: TaskId,
    pub reject_reason: String,
    pub next_requirements: Vec<String>,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAppendRequest {
    pub task_id: TaskId,
    pub note: String,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMutationOutcome {
    pub task: TaskSnapshot,
    pub event: TaskLedgerEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMutationOutcome {
    pub agent: AgentSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskClaimOutcome {
    pub task: Option<TaskSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    pub event: Option<TaskLedgerEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRuntimeSnapshot {
    pub status: String,
    pub tasks: Vec<TaskSnapshot>,
    pub agents: Vec<AgentSnapshot>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TaskError {
    #[error("task field `{0}` is required")]
    MissingField(&'static str),
    #[error("task `{0}` already exists")]
    TaskAlreadyExists(String),
    #[error("task `{0}` not found")]
    TaskNotFound(String),
    #[error("agent `{0}` not found")]
    AgentNotFound(String),
    #[error("agent `{0}` already exists")]
    AgentAlreadyExists(String),
    #[error("agent `{0}` is not available")]
    AgentUnavailable(String),
    #[error("invalid agent transition from `{from:?}` using `{event_type}`")]
    InvalidAgentTransition {
        from: AgentStatus,
        event_type: &'static str,
    },
    #[error("invalid task transition from `{from:?}` using `{event_type}`")]
    InvalidTransition {
        from: TaskStatus,
        event_type: &'static str,
    },
    #[error("task persistence failed: {0}")]
    Persistence(String),
    #[error("task ledger replay failed: {0}")]
    Replay(String),
    #[error("task event cursor `{0}` not found")]
    CursorNotFound(String),
    #[error("invalid task event cursor mode: {0}")]
    InvalidCursorMode(String),
}

pub struct TaskRuntime {
    store: TaskStore,
    state: Mutex<TaskRuntimeState>,
}

#[derive(Debug, Clone, Default)]
struct TaskRuntimeState {
    tasks: BTreeMap<TaskId, TaskSnapshot>,
    agents: BTreeMap<AgentId, AgentSnapshot>,
    lifecycle: BTreeMap<AgentId, AgentLifecycleSnapshot>,
    leases: BTreeMap<TaskId, TaskLease>,
    scheduler_facts: BTreeMap<TaskId, Vec<SchedulerFact>>,
}

impl TaskRuntime {
    pub fn boot(
        runtime_home: impl Into<PathBuf>,
        owner_agent_id: AgentId,
    ) -> Result<Self, TaskError> {
        let store = TaskStore::new(runtime_home, owner_agent_id.clone());
        let mut state = TaskRuntimeState::default();
        let self_agent = store.load_or_create_self_agent(&owner_agent_id)?;
        for agent in store.load_agent_snapshots()? {
            state.lifecycle.insert(
                agent.agent_id.clone(),
                lifecycle_from_agent_snapshot(&agent, now_unix_seconds()),
            );
            state.agents.insert(agent.agent_id.clone(), agent);
        }
        for lifecycle in store.load_agent_lifecycle_snapshots()? {
            state
                .lifecycle
                .insert(lifecycle.agent_id.clone(), lifecycle);
        }
        state.lifecycle.insert(
            self_agent.agent_id.clone(),
            lifecycle_from_agent_snapshot(&self_agent, now_unix_seconds()),
        );
        if let Some(lifecycle) = store.load_agent_lifecycle_snapshot(&self_agent.agent_id)? {
            state
                .lifecycle
                .insert(self_agent.agent_id.clone(), lifecycle);
        }
        state.agents.insert(owner_agent_id, self_agent);
        for task in store.load_task_snapshots()? {
            state.tasks.insert(task.task_id.clone(), task);
        }
        state.leases = store.load_leases()?;
        reconcile_running_leases(&store, &mut state, now_unix_seconds())?;
        reconcile_paused_agents(&store, &mut state, now_unix_seconds())?;
        state.scheduler_facts = store.load_scheduler_facts(state.tasks.keys())?;
        Ok(Self {
            store,
            state: Mutex::new(state),
        })
    }

    pub fn create_task(&self, request: TaskCreateRequest) -> Result<TaskCreateOutcome, TaskError> {
        validate_create_request(&request)?;
        let mut state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let task_id = request
            .task_id
            .clone()
            .unwrap_or_else(|| TaskId::new(format!("task-{}", now_unix_seconds())));
        if state.tasks.contains_key(&task_id) || self.store.task_snapshot_path(&task_id).is_file() {
            return Err(TaskError::TaskAlreadyExists(task_id.as_str().to_owned()));
        }
        let created_at = now_unix_seconds();
        let mut task = TaskSnapshot {
            schema_version: 1,
            task_id: task_id.clone(),
            status: TaskStatus::Created,
            title: request.title,
            content: request.content,
            goal: request.goal,
            deliverables: request.deliverables,
            acceptance: request.acceptance,
            priority: request.priority,
            target_cwd: request.target_cwd,
            assignee: None,
            active_execution_id: None,
            review: TaskReviewState {
                status: "none".to_owned(),
                submitted_at: None,
                reviewer_agent_id: None,
                decision: None,
                reject_reason: None,
                next_requirements: Vec::new(),
            },
            parent: request.parent,
            created_at,
            updated_at: created_at,
            last_progress_at: None,
            last_event_seq: 0,
            last_event_id: String::new(),
        };
        let mut events = Vec::new();
        let created = build_event(
            &task,
            None,
            TaskStatus::Created,
            "TaskCreated",
            &request.actor,
            &request.watermark,
            json!({}),
        );
        apply_event(&mut task, &created);
        self.store.append_event_and_snapshot(&task, &created)?;
        events.push(created);

        match resolve_dispatch(&request.dispatch, &state.agents)? {
            Some(agent_id) => {
                let from = task.status.clone();
                task.assignee = Some(TaskAssignee {
                    agent_id: agent_id.clone(),
                    assignment_id: format!("assign-{}-{}", task.task_id.as_str(), events.len() + 1),
                });
                task.status = TaskStatus::Assigned;
                let assigned = build_event(
                    &task,
                    Some(from),
                    TaskStatus::Assigned,
                    "TaskAssigned",
                    &request.actor,
                    &request.watermark,
                    json!({"agent_id": agent_id.as_str()}),
                );
                apply_event(&mut task, &assigned);
                self.store.append_event_and_snapshot(&task, &assigned)?;
                events.push(assigned);
                if let Some(agent) = state.agents.get_mut(&agent_id) {
                    agent.status = AgentStatus::Busy;
                    agent.current_task_id = Some(task.task_id.clone());
                    agent.current_execution_id = None;
                    agent.current_cwd = task.target_cwd.clone();
                    agent.running_tasks = 0;
                    agent.queued_tasks = agent.queued_tasks.saturating_add(1);
                    agent.last_seen_at = now_unix_seconds();
                    self.store.write_agent_snapshot(agent)?;
                }
            }
            None => {
                let from = task.status.clone();
                task.status = TaskStatus::WaitingAgent;
                let waiting = build_event(
                    &task,
                    Some(from),
                    TaskStatus::WaitingAgent,
                    "TaskWaitingAgent",
                    &request.actor,
                    &request.watermark,
                    json!({}),
                );
                apply_event(&mut task, &waiting);
                self.store.append_event_and_snapshot(&task, &waiting)?;
                events.push(waiting);
            }
        }
        state.tasks.insert(task.task_id.clone(), task.clone());
        Ok(TaskCreateOutcome { task, events })
    }

    pub fn query_task(&self, task_id: &TaskId) -> Result<TaskSnapshot, TaskError> {
        self.state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| TaskError::TaskNotFound(task_id.as_str().to_owned()))
    }

    pub fn list_tasks(&self, query: TaskListQuery) -> Result<Vec<TaskSnapshot>, TaskError> {
        let state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let mut tasks = state
            .tasks
            .values()
            .filter(|task| {
                query
                    .status
                    .as_ref()
                    .map(|status| &task.status == status)
                    .unwrap_or(true)
            })
            .filter(|task| {
                query
                    .assignee
                    .as_ref()
                    .map(|agent_id| {
                        task.assignee
                            .as_ref()
                            .map(|assignee| &assignee.agent_id == agent_id)
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.created_at.cmp(&right.created_at))
                .then_with(|| left.task_id.cmp(&right.task_id))
        });
        Ok(tasks)
    }

    pub fn query_task_board(
        &self,
        query: TaskBoardQuery,
    ) -> Result<TaskBoardProjection, TaskError> {
        let state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let mut tasks = state
            .tasks
            .values()
            .filter(|task| {
                query
                    .status
                    .as_ref()
                    .map(|status| &task.status == status)
                    .unwrap_or(true)
            })
            .filter(|task| {
                query
                    .assignee
                    .as_ref()
                    .map(|agent_id| {
                        task.assignee
                            .as_ref()
                            .map(|assignee| &assignee.agent_id == agent_id)
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .filter(|task| query.include_terminal || !is_terminal_status(&task.status))
            .cloned()
            .collect::<Vec<_>>();
        sort_task_snapshots(&mut tasks);
        let blocked = tasks
            .iter()
            .filter(|task| matches!(task.status, TaskStatus::Blocked))
            .cloned()
            .collect();
        let review_ready = tasks
            .iter()
            .filter(|task| matches!(task.status, TaskStatus::ReviewSubmitted))
            .cloned()
            .collect();
        let stale = tasks
            .iter()
            .filter(|task| {
                state
                    .scheduler_facts
                    .get(&task.task_id)
                    .map(|facts| {
                        facts
                            .iter()
                            .any(|fact| matches!(fact.fact, SchedulerFactKind::Stale { .. }))
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        Ok(TaskBoardProjection {
            schema_version: 1,
            generated_at: now_unix_seconds(),
            tasks,
            agents: state.agents.values().cloned().collect(),
            blocked,
            review_ready,
            stale,
        })
    }

    pub fn query_event_inbox(
        &self,
        query: TaskEventInboxQuery,
    ) -> Result<TaskEventInboxProjection, TaskError> {
        let source_cursor = normalize_cursor(query.after_cursor)?;
        let events = self.master_visible_events_after(source_cursor.as_deref(), query.limit)?;
        Ok(TaskEventInboxProjection {
            schema_version: 1,
            generated_at: now_unix_seconds(),
            source_cursor,
            next_cursor: events.last().map(|event| event.cursor.clone()),
            events,
        })
    }

    pub fn run_master_poll(
        &self,
        request: MasterPollRequest,
    ) -> Result<MasterPollOutcome, TaskError> {
        let explicit_cursor = normalize_cursor(request.after_cursor)?;
        if request.replay_from_start && explicit_cursor.is_some() {
            return Err(TaskError::InvalidCursorMode(
                "replay_from_start cannot be combined with after_cursor".to_owned(),
            ));
        }
        let source_cursor = match explicit_cursor {
            Some(cursor) => Some(cursor),
            None if request.replay_from_start => None,
            None => self
                .store
                .load_master_event_cursor(&request.actor.agent_id)?,
        };
        let event_inbox = self.query_event_inbox(TaskEventInboxQuery {
            after_cursor: source_cursor.clone(),
            limit: request.limit,
        })?;
        let task_board = self.query_task_board(TaskBoardQuery {
            status: None,
            assignee: None,
            include_terminal: request.include_terminal,
        })?;
        let agent_board = self.query_agent_board()?;
        let classifications = master_poll_classifications(&event_inbox, &task_board, &agent_board);
        if let Some(cursor) = event_inbox.next_cursor.as_ref() {
            self.store
                .write_master_event_cursor(&request.actor.agent_id, cursor)?;
        }
        let persisted_cursor = self
            .store
            .load_master_event_cursor(&request.actor.agent_id)?;
        Ok(MasterPollOutcome {
            schema_version: 1,
            generated_at: now_unix_seconds(),
            source_cursor,
            next_cursor: event_inbox.next_cursor.clone(),
            persisted_cursor,
            include_terminal: request.include_terminal,
            event_inbox,
            task_board,
            agent_board,
            classifications,
        })
    }

    pub fn apply_worker_control(
        &self,
        request: WorkerControlRequest,
    ) -> Result<WorkerControlProjection, TaskError> {
        validate_worker_control_request(&request)?;
        let (task, _agent, _lifecycle) = self.validate_worker_control_target(&request)?;
        validate_worker_control_transition(&request.op, &task.status)?;

        let task_event = match request.op {
            WorkerControlOp::Pause => Some(
                self.pause_task(TaskMutationRequest {
                    task_id: request.task_id.clone(),
                    actor: worker_control_task_actor(&request),
                    watermark: request.watermark.clone(),
                })?
                .event,
            ),
            WorkerControlOp::Resume => Some(
                self.resume_task(TaskMutationRequest {
                    task_id: request.task_id.clone(),
                    actor: worker_control_task_actor(&request),
                    watermark: request.watermark.clone(),
                })?
                .event,
            ),
            WorkerControlOp::Cancel => Some(
                self.cancel_task(TaskMutationRequest {
                    task_id: request.task_id.clone(),
                    actor: worker_control_task_actor(&request),
                    watermark: request.watermark.clone(),
                })?
                .event,
            ),
            WorkerControlOp::QueryStatus
            | WorkerControlOp::AskAtSafePoint
            | WorkerControlOp::AddConstraint
            | WorkerControlOp::RequestCheckpoint
            | WorkerControlOp::RequestSubmissionNow => None,
        };

        let task = self.query_task(&request.task_id)?;
        let agent = self.query_agent(&request.agent_id)?;
        let lifecycle = self.query_agent_lifecycle(&request.agent_id).ok();
        let event = build_worker_control_event(&request, &task, &agent, lifecycle.as_ref());
        self.store.append_worker_control_event(&event)?;
        let projection = WorkerControlProjection {
            schema_version: 1,
            generated_at: now_unix_seconds(),
            event,
            task,
            agent,
            lifecycle,
            task_event,
        };
        self.store.write_worker_control_snapshot(&projection)?;
        Ok(projection)
    }

    pub fn query_worker_control_events(
        &self,
        task_id: &TaskId,
        execution_id: &str,
    ) -> Result<Vec<WorkerControlEvent>, TaskError> {
        require_text(task_id.as_str(), "task_id")?;
        require_text(execution_id, "execution_id")?;
        let exists = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .tasks
            .contains_key(task_id);
        if !exists {
            return Err(TaskError::TaskNotFound(task_id.as_str().to_owned()));
        }
        self.store
            .load_worker_control_events(task_id, Some(execution_id))
    }

    pub fn task_history(&self, task_id: &TaskId) -> Result<Vec<TaskLedgerEvent>, TaskError> {
        let exists = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .tasks
            .contains_key(task_id);
        if !exists {
            return Err(TaskError::TaskNotFound(task_id.as_str().to_owned()));
        }
        self.store.load_task_ledger(task_id)
    }

    pub fn append_task(
        &self,
        request: TaskAppendRequest,
    ) -> Result<TaskMutationOutcome, TaskError> {
        require_text(&request.note, "note")?;
        self.mutate_task(
            &request.task_id,
            "TaskProgressed",
            None,
            &request.actor,
            &request.watermark,
            json!({"note": request.note}),
        )
    }

    pub fn pause_task(
        &self,
        request: TaskMutationRequest,
    ) -> Result<TaskMutationOutcome, TaskError> {
        self.mutate_task(
            &request.task_id,
            "TaskPaused",
            Some(TaskStatus::Paused),
            &request.actor,
            &request.watermark,
            json!({}),
        )
    }

    pub fn resume_task(
        &self,
        request: TaskMutationRequest,
    ) -> Result<TaskMutationOutcome, TaskError> {
        let outcome = self.mutate_task(
            &request.task_id,
            "TaskResumed",
            Some(TaskStatus::Running),
            &request.actor,
            &request.watermark,
            json!({}),
        )?;
        self.acquire_or_refresh_lease(
            &outcome.task.task_id,
            &request.actor.agent_id,
            &request.actor,
            &request.watermark,
            DEFAULT_TASK_LEASE_TTL_SECONDS,
        )?;
        Ok(outcome)
    }

    pub fn heartbeat_task(
        &self,
        request: TaskHeartbeatRequest,
    ) -> Result<TaskMutationOutcome, TaskError> {
        if request.ttl_seconds == 0 {
            return Err(TaskError::MissingField("ttl_seconds"));
        }
        self.acquire_or_refresh_lease(
            &request.task_id,
            &request.actor.agent_id,
            &request.actor,
            &request.watermark,
            request.ttl_seconds,
        )
    }

    pub fn assign_task(
        &self,
        request: TaskAssignRequest,
    ) -> Result<TaskMutationOutcome, TaskError> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let mut task = self.refresh_task_from_store_locked(&mut state, &request.task_id)?;
        if !matches!(
            task.status,
            TaskStatus::WaitingAgent
                | TaskStatus::Created
                | TaskStatus::Interrupted
                | TaskStatus::Blocked
                | TaskStatus::Rejected
        ) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                event_type: "TaskAssigned",
            });
        }
        let reactivating_same_agent = matches!(
            task.status,
            TaskStatus::Interrupted | TaskStatus::Blocked | TaskStatus::Rejected
        ) && task
            .assignee
            .as_ref()
            .map(|assignee| assignee.agent_id == request.agent_id)
            .unwrap_or(false);
        let agent = state
            .agents
            .get_mut(&request.agent_id)
            .ok_or_else(|| TaskError::AgentNotFound(request.agent_id.as_str().to_owned()))?;
        let agent_can_accept_assignment =
            matches!(agent.status, AgentStatus::Available | AgentStatus::Busy)
                || reactivating_same_agent && matches!(agent.status, AgentStatus::Paused);
        if !agent_can_accept_assignment {
            return Err(TaskError::AgentUnavailable(
                request.agent_id.as_str().to_owned(),
            ));
        }
        let from = task.status.clone();
        task.active_execution_id = None;
        task.assignee = Some(TaskAssignee {
            agent_id: request.agent_id.clone(),
            assignment_id: format!(
                "assign-{}-{}",
                task.task_id.as_str(),
                task.last_event_seq.saturating_add(1)
            ),
        });
        task.status = TaskStatus::Assigned;
        let event = build_event(
            &task,
            Some(from),
            TaskStatus::Assigned,
            "TaskAssigned",
            &request.actor,
            &request.watermark,
            json!({"agent_id": request.agent_id.as_str()}),
        );
        apply_event(&mut task, &event);
        self.store.append_event_and_snapshot(&task, &event)?;
        agent.status = AgentStatus::Busy;
        agent.current_task_id = Some(task.task_id.clone());
        agent.current_execution_id = None;
        agent.current_cwd = task.target_cwd.clone();
        agent.running_tasks = 0;
        agent.queued_tasks = agent.queued_tasks.saturating_add(1);
        agent.last_seen_at = now_unix_seconds();
        self.store.write_agent_snapshot(agent)?;
        state.tasks.insert(task.task_id.clone(), task.clone());
        Ok(TaskMutationOutcome { task, event })
    }

    pub fn claim_next_task(
        &self,
        request: TaskClaimRequest,
    ) -> Result<TaskClaimOutcome, TaskError> {
        let task_id = {
            let state = self
                .state
                .lock()
                .map_err(|err| TaskError::Persistence(err.to_string()))?;
            state
                .agents
                .get(&request.agent_id)
                .ok_or_else(|| TaskError::AgentNotFound(request.agent_id.as_str().to_owned()))?;
            state
                .tasks
                .values()
                .filter(|task| {
                    matches!(task.status, TaskStatus::Assigned)
                        && task
                            .assignee
                            .as_ref()
                            .map(|assignee| assignee.agent_id == request.agent_id)
                            .unwrap_or(false)
                })
                .max_by(|left, right| {
                    left.priority
                        .cmp(&right.priority)
                        .then_with(|| right.created_at.cmp(&left.created_at))
                        .then_with(|| right.task_id.cmp(&left.task_id))
                })
                .map(|task| task.task_id.clone())
        };
        let Some(task_id) = task_id else {
            return Ok(TaskClaimOutcome {
                task: None,
                execution_id: None,
                event: None,
            });
        };
        require_text(&request.execution_id, "execution_id")?;
        let execution_id = request.execution_id.clone();
        let resumed = self.mutate_task(
            &task_id,
            "TaskResumed",
            Some(TaskStatus::Running),
            &request.actor,
            &request.watermark,
            json!({
                "claim_agent_id": request.agent_id.as_str(),
                "execution_id": execution_id
            }),
        )?;
        let heartbeat = self.acquire_or_refresh_lease(
            &task_id,
            &request.agent_id,
            &request.actor,
            &request.watermark,
            request.ttl_seconds,
        )?;
        Ok(TaskClaimOutcome {
            task: Some(heartbeat.task),
            execution_id: Some(request.execution_id),
            event: Some(resumed.event),
        })
    }

    pub fn record_execution(
        &self,
        request: TaskExecutionRecordRequest,
    ) -> Result<TaskMutationOutcome, TaskError> {
        require_text(&request.phase, "phase")?;
        require_text(&request.summary, "summary")?;
        if request.evidence.is_empty() {
            return Err(TaskError::MissingField("evidence"));
        }
        self.mutate_task(
            &request.task_id,
            "TaskExecutionRecorded",
            None,
            &request.actor,
            &request.watermark,
            json!({
                "phase": request.phase,
                "summary": request.summary,
                "evidence": request.evidence
            }),
        )
    }

    pub fn apply_execution_fact(
        &self,
        fact: ExecutionFact,
    ) -> Result<TaskMutationOutcome, TaskError> {
        validate_execution_fact(&fact)?;
        self.ensure_execution_binding(&fact)?;
        let actor = TaskActor {
            agent_id: fact.agent_id.clone(),
            source: "task.orchestration.execution_fact".to_owned(),
            session_id: None,
            turn_id: fact.turn_id.clone(),
            trace_id: None,
        };
        match fact.kind {
            ExecutionFactKind::Running {
                phase,
                summary,
                evidence,
            } => {
                self.ensure_running_from_execution_fact(&fact.task_id, &actor, &fact.watermark)?;
                self.mutate_task(
                    &fact.task_id,
                    "TaskExecutionRecorded",
                    None,
                    &actor,
                    &fact.watermark,
                    json!({
                        "execution_id": fact.execution_id,
                        "agent_id": fact.agent_id.as_str(),
                        "turn_id": fact.turn_id.as_ref().map(TurnId::as_str),
                        "occurred_at": fact.occurred_at,
                        "phase": phase,
                        "summary": summary,
                        "evidence": evidence
                    }),
                )
            }
            ExecutionFactKind::Recovering {
                summary,
                evidence,
                retry_count,
            } => {
                self.ensure_running_from_execution_fact(&fact.task_id, &actor, &fact.watermark)?;
                self.mutate_task(
                    &fact.task_id,
                    "TaskExecutionRecovering",
                    None,
                    &actor,
                    &fact.watermark,
                    json!({
                        "execution_id": fact.execution_id,
                        "agent_id": fact.agent_id.as_str(),
                        "turn_id": fact.turn_id.as_ref().map(TurnId::as_str),
                        "occurred_at": fact.occurred_at,
                        "summary": summary,
                        "evidence": evidence,
                        "retry_count": retry_count
                    }),
                )
            }
            ExecutionFactKind::Blocked { reason, evidence } => self.mutate_task(
                &fact.task_id,
                "TaskBlocked",
                Some(TaskStatus::Blocked),
                &actor,
                &fact.watermark,
                json!({
                    "execution_id": fact.execution_id,
                    "agent_id": fact.agent_id.as_str(),
                    "turn_id": fact.turn_id.as_ref().map(TurnId::as_str),
                    "occurred_at": fact.occurred_at,
                    "reason": reason,
                    "evidence": evidence
                }),
            ),
            ExecutionFactKind::Interrupted { reason, evidence } => self.mutate_task(
                &fact.task_id,
                "TaskInterrupted",
                Some(TaskStatus::Interrupted),
                &actor,
                &fact.watermark,
                json!({
                    "execution_id": fact.execution_id,
                    "agent_id": fact.agent_id.as_str(),
                    "turn_id": fact.turn_id.as_ref().map(TurnId::as_str),
                    "occurred_at": fact.occurred_at,
                    "reason": reason,
                    "evidence": evidence
                }),
            ),
            ExecutionFactKind::ReviewReady {
                summary,
                deliverables,
                evidence,
            } => {
                self.ensure_running_from_execution_fact(&fact.task_id, &actor, &fact.watermark)?;
                self.mutate_task(
                    &fact.task_id,
                    "TaskReviewSubmitted",
                    Some(TaskStatus::ReviewSubmitted),
                    &actor,
                    &fact.watermark,
                    json!({
                        "execution_id": fact.execution_id,
                        "agent_id": fact.agent_id.as_str(),
                        "turn_id": fact.turn_id.as_ref().map(TurnId::as_str),
                        "occurred_at": fact.occurred_at,
                        "summary": summary,
                        "deliverables": deliverables,
                        "evidence": evidence
                    }),
                )
            }
        }
    }

    pub fn run_scheduler_tick(
        &self,
        request: SchedulerTickRequest,
    ) -> Result<SchedulerTickOutcome, TaskError> {
        validate_scheduler_tick_request(&request)?;
        let mut state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let candidates = state
            .tasks
            .values()
            .filter(|task| !is_terminal_status(&task.status))
            .cloned()
            .collect::<Vec<_>>();
        let mut facts = Vec::new();
        let mut events = Vec::new();
        for task in candidates {
            let task_facts = scheduler_facts_for_task(&task, &state.leases, &request);
            for fact in task_facts {
                let event = build_event(
                    &task,
                    Some(task.status.clone()),
                    task.status.clone(),
                    "TaskSchedulerTick",
                    &request.actor,
                    &request.watermark,
                    json!({"scheduler_fact": fact}),
                );
                self.store.append_event_and_snapshot(&task, &event)?;
                state
                    .scheduler_facts
                    .entry(task.task_id.clone())
                    .or_default()
                    .push(fact.clone());
                events.push(event);
                facts.push(fact);
            }
        }
        Ok(SchedulerTickOutcome { facts, events })
    }

    pub fn cancel_task(
        &self,
        request: TaskMutationRequest,
    ) -> Result<TaskMutationOutcome, TaskError> {
        self.mutate_task(
            &request.task_id,
            "TaskCancelled",
            Some(TaskStatus::Cancelled),
            &request.actor,
            &request.watermark,
            json!({}),
        )
    }

    pub fn submit_review(
        &self,
        request: TaskReviewSubmission,
    ) -> Result<TaskMutationOutcome, TaskError> {
        require_text(&request.summary, "summary")?;
        if request.deliverables.is_empty() {
            return Err(TaskError::MissingField("deliverables"));
        }
        if request.evidence.is_empty() {
            return Err(TaskError::MissingField("evidence"));
        }
        self.mutate_task(
            &request.task_id,
            "TaskReviewSubmitted",
            Some(TaskStatus::ReviewSubmitted),
            &request.actor,
            &request.watermark,
            json!({
                "summary": request.summary,
                "deliverables": request.deliverables,
                "evidence": request.evidence
            }),
        )
    }

    pub fn approve_review(
        &self,
        request: TaskMutationRequest,
    ) -> Result<TaskMutationOutcome, TaskError> {
        self.mutate_task(
            &request.task_id,
            "TaskReviewApproved",
            Some(TaskStatus::Approved),
            &request.actor,
            &request.watermark,
            json!({}),
        )
    }

    pub fn reject_review(
        &self,
        request: TaskReviewRejection,
    ) -> Result<TaskMutationOutcome, TaskError> {
        require_text(&request.reject_reason, "reject_reason")?;
        if request.next_requirements.is_empty() {
            return Err(TaskError::MissingField("next_requirements"));
        }
        self.mutate_task(
            &request.task_id,
            "TaskReviewRejected",
            Some(TaskStatus::Rejected),
            &request.actor,
            &request.watermark,
            json!({
                "reject_reason": request.reject_reason,
                "next_requirements": request.next_requirements
            }),
        )
    }

    pub fn close_task(
        &self,
        request: TaskMutationRequest,
    ) -> Result<TaskMutationOutcome, TaskError> {
        self.mutate_task(
            &request.task_id,
            "TaskClosed",
            Some(TaskStatus::Closed),
            &request.actor,
            &request.watermark,
            json!({}),
        )
    }

    pub fn create_agent(
        &self,
        request: AgentCreateRequest,
    ) -> Result<AgentMutationOutcome, TaskError> {
        if request.capabilities.is_empty() {
            return Err(TaskError::MissingField("capabilities"));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        if state.agents.contains_key(&request.agent_id)
            || self.store.agent_snapshot_path(&request.agent_id).is_file()
        {
            return Err(TaskError::AgentAlreadyExists(
                request.agent_id.as_str().to_owned(),
            ));
        }
        let now = now_unix_seconds();
        let agent = AgentSnapshot {
            schema_version: 1,
            agent_id: request.agent_id.clone(),
            status: AgentStatus::Available,
            current_task_id: None,
            current_execution_id: None,
            current_cwd: None,
            capabilities: request.capabilities,
            last_seen_at: now,
            running_tasks: 0,
            queued_tasks: 0,
        };
        self.store.write_agent_snapshot(&agent)?;
        let lifecycle = lifecycle_from_agent_snapshot(&agent, now);
        self.store.write_agent_lifecycle_snapshot(&lifecycle)?;
        state.lifecycle.insert(agent.agent_id.clone(), lifecycle);
        state.agents.insert(agent.agent_id.clone(), agent.clone());
        Ok(AgentMutationOutcome { agent })
    }

    pub fn close_agent(
        &self,
        request: AgentMutationRequest,
    ) -> Result<AgentMutationOutcome, TaskError> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let agent = state
            .agents
            .get_mut(&request.agent_id)
            .ok_or_else(|| TaskError::AgentNotFound(request.agent_id.as_str().to_owned()))?;
        if !matches!(
            agent.status,
            AgentStatus::Available | AgentStatus::Paused | AgentStatus::Offline
        ) || agent.current_task_id.is_some()
            || agent.running_tasks > 0
            || agent.queued_tasks > 0
        {
            return Err(TaskError::InvalidAgentTransition {
                from: agent.status.clone(),
                event_type: "AgentClosed",
            });
        }
        agent.status = AgentStatus::Closed;
        agent.last_seen_at = now_unix_seconds();
        self.store.write_agent_snapshot(agent)?;
        Ok(AgentMutationOutcome {
            agent: agent.clone(),
        })
    }

    pub fn list_agents(&self) -> Result<Vec<AgentSnapshot>, TaskError> {
        Ok(self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .agents
            .values()
            .cloned()
            .collect())
    }

    pub fn query_agent(&self, agent_id: &AgentId) -> Result<AgentSnapshot, TaskError> {
        self.state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .agents
            .get(agent_id)
            .cloned()
            .ok_or_else(|| TaskError::AgentNotFound(agent_id.as_str().to_owned()))
    }

    pub fn apply_agent_lifecycle_event(
        &self,
        event: AgentLifecycleEvent,
    ) -> Result<AgentLifecycleSnapshot, TaskError> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let agent_id = event.agent_id().clone();
        let agent = state
            .agents
            .get(&agent_id)
            .ok_or_else(|| TaskError::AgentNotFound(agent_id.as_str().to_owned()))?;
        let now = now_unix_seconds();
        let mut snapshot = state
            .lifecycle
            .get(&agent_id)
            .cloned()
            .unwrap_or_else(|| lifecycle_from_agent_snapshot(agent, now));
        let previous = snapshot.current_activity.clone();
        let (next_state, activity, model, stats_update) = lifecycle_event_projection(&event, now);
        snapshot.state = next_state;
        snapshot.state_entered_at = now;
        snapshot.elapsed_ms = 0;
        snapshot.current_task_id = event
            .task_id()
            .cloned()
            .or_else(|| agent.current_task_id.clone());
        snapshot.current_turn_id = event.turn_id().cloned();
        snapshot.current_activity = Some(activity);
        snapshot.last_activity = previous;
        snapshot.last_seen_at = now;
        if let Some(model) = model {
            snapshot.stats.current_model = Some(model);
        }
        stats_update(&mut snapshot.stats);
        self.store.write_agent_lifecycle_snapshot(&snapshot)?;
        state.lifecycle.insert(agent_id, snapshot.clone());
        Ok(snapshot)
    }

    pub fn query_agent_lifecycle(
        &self,
        agent_id: &AgentId,
    ) -> Result<AgentLifecycleSnapshot, TaskError> {
        self.state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .lifecycle
            .get(agent_id)
            .cloned()
            .ok_or_else(|| TaskError::AgentNotFound(agent_id.as_str().to_owned()))
    }

    pub fn query_agent_board(&self) -> Result<AgentBoardProjection, TaskError> {
        let mut agents = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .lifecycle
            .values()
            .cloned()
            .collect::<Vec<_>>();
        agents.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
        Ok(AgentBoardProjection {
            schema_version: 1,
            generated_at: now_unix_seconds(),
            agents,
        })
    }

    pub fn snapshot(&self) -> Result<TaskRuntimeSnapshot, TaskError> {
        let state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        Ok(TaskRuntimeSnapshot {
            status: "ready".to_owned(),
            tasks: state.tasks.values().cloned().collect(),
            agents: state.agents.values().cloned().collect(),
        })
    }

    fn mutate_task(
        &self,
        task_id: &TaskId,
        event_type: &'static str,
        to_status: Option<TaskStatus>,
        actor: &TaskActor,
        watermark: &TaskWatermark,
        payload: Value,
    ) -> Result<TaskMutationOutcome, TaskError> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let mut task = self.refresh_task_from_store_locked(&mut state, task_id)?;
        let from = task.status.clone();
        let target = to_status.unwrap_or_else(|| task.status.clone());
        validate_transition(&from, &target, event_type)?;
        if event_type == "TaskReviewSubmitted" {
            task.review.status = "submitted".to_owned();
            task.review.submitted_at = Some(now_unix_seconds());
            task.review.decision = None;
        } else if event_type == "TaskReviewApproved" {
            task.review.status = "approved".to_owned();
            task.review.decision = Some("approved".to_owned());
        } else if event_type == "TaskReviewRejected" {
            task.review.status = "rejected".to_owned();
            task.review.decision = Some("rejected".to_owned());
            task.review.reject_reason = payload
                .get("reject_reason")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            task.review.next_requirements = payload
                .get("next_requirements")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect()
                })
                .unwrap_or_default();
        }
        if let Some(execution_id) = payload.get("execution_id").and_then(Value::as_str)
            && !execution_id.trim().is_empty()
        {
            task.active_execution_id = Some(execution_id.to_owned());
        }
        task.status = target.clone();
        if matches!(event_type, "TaskProgressed" | "TaskExecutionRecorded") {
            task.last_progress_at = Some(now_unix_seconds());
        }
        let payload_for_lifecycle = payload.clone();
        let event = build_event(
            &task,
            Some(from),
            target,
            event_type,
            actor,
            watermark,
            payload,
        );
        apply_event(&mut task, &event);
        self.store.append_event_and_snapshot(&task, &event)?;
        if !matches!(task.status, TaskStatus::Running) {
            self.store.remove_lease(task_id)?;
            state.leases.remove(task_id);
        }
        if matches!(
            task.status,
            TaskStatus::Closed
                | TaskStatus::Cancelled
                | TaskStatus::Failed
                | TaskStatus::Paused
                | TaskStatus::Blocked
                | TaskStatus::ReviewSubmitted
                | TaskStatus::Approved
                | TaskStatus::Interrupted
        ) && let Some(assignee) = task.assignee.as_ref()
            && let Some(agent) = state.agents.get_mut(&assignee.agent_id)
        {
            release_agent_task(agent, &task.status);
            agent.last_seen_at = now_unix_seconds();
            self.store.write_agent_snapshot(agent)?;
        }
        project_task_event_to_lifecycle(&mut state, &task, event_type, &payload_for_lifecycle);
        if let Some(assignee) = task.assignee.as_ref()
            && let Some(snapshot) = state.lifecycle.get(&assignee.agent_id)
        {
            self.store.write_agent_lifecycle_snapshot(snapshot)?;
        }
        state.tasks.insert(task.task_id.clone(), task.clone());
        Ok(TaskMutationOutcome { task, event })
    }

    fn acquire_or_refresh_lease(
        &self,
        task_id: &TaskId,
        agent_id: &AgentId,
        actor: &TaskActor,
        watermark: &TaskWatermark,
        ttl_seconds: u64,
    ) -> Result<TaskMutationOutcome, TaskError> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let mut task = self.refresh_task_from_store_locked(&mut state, task_id)?;
        state.leases = self.store.load_leases()?;
        if !matches!(task.status, TaskStatus::Running) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                event_type: "TaskHeartbeat",
            });
        }
        let Some(assignee) = task.assignee.as_ref() else {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                event_type: "TaskHeartbeat",
            });
        };
        if &assignee.agent_id != agent_id {
            return Err(TaskError::AgentNotFound(agent_id.as_str().to_owned()));
        }
        let now = now_unix_seconds();
        let lease = TaskLease {
            schema_version: 1,
            task_id: task_id.clone(),
            agent_id: agent_id.clone(),
            lease_id: format!("lease-{}-{}", task_id.as_str(), assignee.assignment_id),
            status: "active".to_owned(),
            acquired_at: state
                .leases
                .get(task_id)
                .map(|lease| lease.acquired_at)
                .unwrap_or(now),
            heartbeat_at: now,
            expires_at: now.saturating_add(ttl_seconds),
        };
        let event = build_event(
            &task,
            Some(task.status.clone()),
            TaskStatus::Running,
            "TaskHeartbeat",
            actor,
            watermark,
            json!({
                "agent_id": agent_id.as_str(),
                "lease_id": lease.lease_id,
                "heartbeat_at": lease.heartbeat_at,
                "expires_at": lease.expires_at
            }),
        );
        apply_event(&mut task, &event);
        self.store.append_event_and_snapshot(&task, &event)?;
        self.store.write_lease(&lease)?;
        if let Some(agent) = state.agents.get_mut(agent_id) {
            agent.status = AgentStatus::Busy;
            agent.current_task_id = Some(task_id.clone());
            agent.current_execution_id = task.active_execution_id.clone();
            agent.current_cwd = task.target_cwd.clone();
            agent.running_tasks = 1;
            agent.queued_tasks = agent.queued_tasks.saturating_sub(1);
            agent.last_seen_at = now;
            self.store.write_agent_snapshot(agent)?;
        }
        state.leases.insert(task_id.clone(), lease);
        state.tasks.insert(task_id.clone(), task.clone());
        Ok(TaskMutationOutcome { task, event })
    }

    fn refresh_task_from_store_locked(
        &self,
        state: &mut TaskRuntimeState,
        task_id: &TaskId,
    ) -> Result<TaskSnapshot, TaskError> {
        let task = self.store.load_task_snapshot(task_id)?;
        state.tasks.insert(task_id.clone(), task.clone());
        Ok(task)
    }

    fn ensure_execution_binding(&self, fact: &ExecutionFact) -> Result<(), TaskError> {
        let state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        state
            .agents
            .get(&fact.agent_id)
            .ok_or_else(|| TaskError::AgentNotFound(fact.agent_id.as_str().to_owned()))?;
        let task = state
            .tasks
            .get(&fact.task_id)
            .ok_or_else(|| TaskError::TaskNotFound(fact.task_id.as_str().to_owned()))?;
        if task
            .assignee
            .as_ref()
            .map(|assignee| assignee.agent_id == fact.agent_id)
            .unwrap_or(false)
        {
            Ok(())
        } else {
            Err(TaskError::AgentUnavailable(
                fact.agent_id.as_str().to_owned(),
            ))
        }
    }

    fn ensure_running_from_execution_fact(
        &self,
        task_id: &TaskId,
        actor: &TaskActor,
        watermark: &TaskWatermark,
    ) -> Result<(), TaskError> {
        let status = self.query_task(task_id)?.status;
        if matches!(status, TaskStatus::Running) {
            return Ok(());
        }
        if matches!(
            status,
            TaskStatus::Assigned
                | TaskStatus::Interrupted
                | TaskStatus::Paused
                | TaskStatus::Blocked
                | TaskStatus::Rejected
        ) {
            self.mutate_task(
                task_id,
                "TaskResumed",
                Some(TaskStatus::Running),
                actor,
                watermark,
                json!({"source": "execution_fact"}),
            )?;
            self.acquire_or_refresh_lease(
                task_id,
                &actor.agent_id,
                actor,
                watermark,
                DEFAULT_TASK_LEASE_TTL_SECONDS,
            )?;
            Ok(())
        } else {
            Err(TaskError::InvalidTransition {
                from: status,
                event_type: "TaskExecutionRecorded",
            })
        }
    }

    fn master_visible_events_after(
        &self,
        after_cursor: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TaskEventInboxEntry>, TaskError> {
        let limit = if limit == 0 { usize::MAX } else { limit };
        let mut entries = self
            .store
            .load_all_task_ledger_events()?
            .into_iter()
            .filter_map(|event| task_event_inbox_entry(&event))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.cursor.cmp(&right.cursor));
        let start_index = if let Some(cursor) = after_cursor {
            event_inbox_start_index_after_cursor(&entries, cursor)?
        } else {
            0
        };
        Ok(entries.into_iter().skip(start_index).take(limit).collect())
    }

    fn validate_worker_control_target(
        &self,
        request: &WorkerControlRequest,
    ) -> Result<(TaskSnapshot, AgentSnapshot, Option<AgentLifecycleSnapshot>), TaskError> {
        let state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let task = state
            .tasks
            .get(&request.task_id)
            .cloned()
            .ok_or_else(|| TaskError::TaskNotFound(request.task_id.as_str().to_owned()))?;
        let agent = state
            .agents
            .get(&request.agent_id)
            .cloned()
            .ok_or_else(|| TaskError::AgentNotFound(request.agent_id.as_str().to_owned()))?;
        if is_worker_control_terminal_status(&task.status) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                event_type: request.op.event_type(),
            });
        }
        let assignee_matches = task
            .assignee
            .as_ref()
            .map(|assignee| assignee.agent_id == request.agent_id)
            .unwrap_or(false);
        if !assignee_matches {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                event_type: request.op.event_type(),
            });
        }
        if task.active_execution_id.as_deref() != Some(request.execution_id.as_str()) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                event_type: request.op.event_type(),
            });
        }
        let lifecycle = state.lifecycle.get(&request.agent_id).cloned();
        Ok((task, agent, lifecycle))
    }
}

const DEFAULT_TASK_LEASE_TTL_SECONDS: u64 = 300;
const RUNNING_LEASE_ACQUISITION_GRACE_SECONDS: u64 = 5;

#[derive(Debug, Clone)]
struct TaskStore {
    runtime_home: PathBuf,
    owner_agent_id: AgentId,
}

impl TaskStore {
    fn new(runtime_home: impl Into<PathBuf>, owner_agent_id: AgentId) -> Self {
        Self {
            runtime_home: runtime_home.into(),
            owner_agent_id,
        }
    }

    fn load_or_create_self_agent(
        &self,
        owner_agent_id: &AgentId,
    ) -> Result<AgentSnapshot, TaskError> {
        let path = self.agent_snapshot_path(owner_agent_id);
        if path.is_file() {
            return read_json(&path);
        }
        let now = now_unix_seconds();
        let snapshot = AgentSnapshot {
            schema_version: 1,
            agent_id: owner_agent_id.clone(),
            status: AgentStatus::Available,
            current_task_id: None,
            current_execution_id: None,
            current_cwd: None,
            capabilities: vec![
                "code_edit".to_owned(),
                "test_run".to_owned(),
                "docs".to_owned(),
            ],
            last_seen_at: now,
            running_tasks: 0,
            queued_tasks: 0,
        };
        self.write_agent_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn load_task_snapshots(&self) -> Result<Vec<TaskSnapshot>, TaskError> {
        let task_dir = self.task_state_dir();
        if !task_dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut tasks = Vec::new();
        for entry in fs::read_dir(&task_dir).map_err(io_err)? {
            let path = entry.map_err(io_err)?.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && path.file_name().and_then(|name| name.to_str()) != Some("index.json")
            {
                tasks.push(read_json(&path)?);
            }
        }
        Ok(tasks)
    }

    fn load_task_snapshot(&self, task_id: &TaskId) -> Result<TaskSnapshot, TaskError> {
        let path = self.task_snapshot_path(task_id);
        if !path.is_file() {
            return Err(TaskError::TaskNotFound(task_id.as_str().to_owned()));
        }
        read_json(&path)
    }

    fn load_task_ledger(&self, task_id: &TaskId) -> Result<Vec<TaskLedgerEvent>, TaskError> {
        let path = self.task_ledger_path(task_id);
        if !path.is_file() {
            return Err(TaskError::TaskNotFound(task_id.as_str().to_owned()));
        }
        let file = fs::File::open(&path).map_err(io_err)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(io_err)?;
            if line.trim().is_empty() {
                continue;
            }
            events.push(serde_json::from_str(&line).map_err(json_err)?);
        }
        events.sort_by_key(|event: &TaskLedgerEvent| event.seq);
        Ok(events)
    }

    fn load_all_task_ledger_events(&self) -> Result<Vec<TaskLedgerEvent>, TaskError> {
        let mut events = Vec::new();
        for task in self.load_task_snapshots()? {
            events.extend(self.load_task_ledger(&task.task_id)?);
        }
        events.sort_by(|left, right| {
            task_event_cursor(left)
                .cmp(&task_event_cursor(right))
                .then_with(|| left.event_id.cmp(&right.event_id))
        });
        Ok(events)
    }

    fn load_scheduler_facts<'a>(
        &self,
        task_ids: impl Iterator<Item = &'a TaskId>,
    ) -> Result<BTreeMap<TaskId, Vec<SchedulerFact>>, TaskError> {
        let mut facts = BTreeMap::<TaskId, Vec<SchedulerFact>>::new();
        for task_id in task_ids {
            let path = self.task_ledger_path(task_id);
            if !path.is_file() {
                continue;
            }
            for event in self.load_task_ledger(task_id)? {
                if event.event_type != "TaskSchedulerTick" {
                    continue;
                }
                let Some(value) = event.payload.get("scheduler_fact") else {
                    continue;
                };
                let fact: SchedulerFact =
                    serde_json::from_value(value.clone()).map_err(json_err)?;
                facts.entry(task_id.clone()).or_default().push(fact);
            }
        }
        Ok(facts)
    }

    fn load_agent_snapshots(&self) -> Result<Vec<AgentSnapshot>, TaskError> {
        let dir = self.agent_state_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut agents = Vec::new();
        for entry in fs::read_dir(&dir).map_err(io_err)? {
            let path = entry.map_err(io_err)?.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && path.file_name().and_then(|name| name.to_str()) != Some("index.json")
            {
                agents.push(read_json(&path)?);
            }
        }
        Ok(agents)
    }

    fn load_agent_lifecycle_snapshots(&self) -> Result<Vec<AgentLifecycleSnapshot>, TaskError> {
        let dir = self.agent_lifecycle_state_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut snapshots = Vec::new();
        for entry in fs::read_dir(&dir).map_err(io_err)? {
            let path = entry.map_err(io_err)?.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && path.file_name().and_then(|name| name.to_str()) != Some("index.json")
            {
                snapshots.push(read_json(&path)?);
            }
        }
        Ok(snapshots)
    }

    fn load_agent_lifecycle_snapshot(
        &self,
        agent_id: &AgentId,
    ) -> Result<Option<AgentLifecycleSnapshot>, TaskError> {
        let path = self.agent_lifecycle_snapshot_path(agent_id);
        if path.is_file() {
            Ok(Some(read_json(&path)?))
        } else {
            Ok(None)
        }
    }

    fn append_event_and_snapshot(
        &self,
        snapshot: &TaskSnapshot,
        event: &TaskLedgerEvent,
    ) -> Result<(), TaskError> {
        let ledger_path = self.task_ledger_path(&snapshot.task_id);
        ensure_parent_dir(&ledger_path)?;
        let line = serde_json::to_string(event).map_err(json_err)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&ledger_path)
            .map_err(io_err)?;
        writeln!(file, "{line}").map_err(io_err)?;
        write_json_atomic(&self.task_snapshot_path(&snapshot.task_id), snapshot)?;
        self.write_task_index()
    }

    fn write_task_index(&self) -> Result<(), TaskError> {
        let tasks = self
            .load_task_snapshots()?
            .into_iter()
            .map(|task| task.task_id)
            .collect::<Vec<_>>();
        write_json_atomic(&self.task_state_dir().join("index.json"), &tasks)
    }

    fn write_agent_snapshot(&self, snapshot: &AgentSnapshot) -> Result<(), TaskError> {
        write_json_atomic(&self.agent_snapshot_path(&snapshot.agent_id), snapshot)?;
        self.write_agent_index()
    }

    fn write_agent_lifecycle_snapshot(
        &self,
        snapshot: &AgentLifecycleSnapshot,
    ) -> Result<(), TaskError> {
        write_json_atomic(
            &self.agent_lifecycle_snapshot_path(&snapshot.agent_id),
            snapshot,
        )?;
        self.write_agent_lifecycle_index()
    }

    fn write_agent_lifecycle_index(&self) -> Result<(), TaskError> {
        let dir = self.agent_lifecycle_state_dir();
        let mut agents = Vec::<AgentId>::new();
        if dir.is_dir() {
            for entry in fs::read_dir(&dir).map_err(io_err)? {
                let path = entry.map_err(io_err)?.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                    && path.file_name().and_then(|name| name.to_str()) != Some("index.json")
                {
                    let snapshot: AgentLifecycleSnapshot = read_json(&path)?;
                    agents.push(snapshot.agent_id);
                }
            }
        }
        write_json_atomic(&dir.join("index.json"), &agents)
    }

    fn write_agent_index(&self) -> Result<(), TaskError> {
        let dir = self.agent_state_dir();
        let mut agents = Vec::<AgentId>::new();
        if dir.is_dir() {
            for entry in fs::read_dir(&dir).map_err(io_err)? {
                let path = entry.map_err(io_err)?.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                    && path.file_name().and_then(|name| name.to_str()) != Some("index.json")
                {
                    let agent: AgentSnapshot = read_json(&path)?;
                    agents.push(agent.agent_id);
                }
            }
        }
        write_json_atomic(&dir.join("index.json"), &agents)
    }

    fn load_leases(&self) -> Result<BTreeMap<TaskId, TaskLease>, TaskError> {
        let path = self.lease_state_path();
        if !path.is_file() {
            return Ok(BTreeMap::new());
        }
        let leases: Vec<TaskLease> = read_json(&path)?;
        Ok(leases
            .into_iter()
            .map(|lease| (lease.task_id.clone(), lease))
            .collect())
    }

    fn write_leases(&self, leases: &BTreeMap<TaskId, TaskLease>) -> Result<(), TaskError> {
        let values = leases.values().cloned().collect::<Vec<_>>();
        write_json_atomic(&self.lease_state_path(), &values)
    }

    fn write_lease(&self, lease: &TaskLease) -> Result<(), TaskError> {
        let mut leases = self.load_leases()?;
        leases.insert(lease.task_id.clone(), lease.clone());
        self.write_leases(&leases)
    }

    fn remove_lease(&self, task_id: &TaskId) -> Result<(), TaskError> {
        let mut leases = self.load_leases()?;
        leases.remove(task_id);
        self.write_leases(&leases)
    }

    fn load_master_event_cursor(
        &self,
        master_agent_id: &AgentId,
    ) -> Result<Option<String>, TaskError> {
        let path = self.master_event_cursor_path(master_agent_id);
        if path.is_file() {
            Ok(Some(read_json(&path)?))
        } else {
            Ok(None)
        }
    }

    fn write_master_event_cursor(
        &self,
        master_agent_id: &AgentId,
        cursor: &str,
    ) -> Result<(), TaskError> {
        require_text(cursor, "event_cursor")?;
        write_json_atomic(
            &self.master_event_cursor_path(master_agent_id),
            &cursor.to_owned(),
        )
    }

    fn append_worker_control_event(&self, event: &WorkerControlEvent) -> Result<(), TaskError> {
        let ledger_path = self.worker_control_ledger_path(&event.task_id);
        ensure_parent_dir(&ledger_path)?;
        let line = serde_json::to_string(event).map_err(json_err)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&ledger_path)
            .map_err(io_err)?;
        writeln!(file, "{line}").map_err(io_err)
    }

    fn load_worker_control_events(
        &self,
        task_id: &TaskId,
        execution_id: Option<&str>,
    ) -> Result<Vec<WorkerControlEvent>, TaskError> {
        let path = self.worker_control_ledger_path(task_id);
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(&path).map_err(io_err)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(io_err)?;
            if line.trim().is_empty() {
                continue;
            }
            let event: WorkerControlEvent = serde_json::from_str(&line).map_err(json_err)?;
            if execution_id
                .map(|execution_id| event.execution_id == execution_id)
                .unwrap_or(true)
            {
                events.push(event);
            }
        }
        Ok(events)
    }

    fn write_worker_control_snapshot(
        &self,
        projection: &WorkerControlProjection,
    ) -> Result<(), TaskError> {
        write_json_atomic(
            &self.worker_control_snapshot_path(
                &projection.event.task_id,
                &projection.event.execution_id,
            ),
            projection,
        )
    }

    fn task_state_dir(&self) -> PathBuf {
        self.runtime_home
            .join("state")
            .join("tasks")
            .join(self.owner_agent_id.as_str())
    }

    fn task_ledger_dir(&self) -> PathBuf {
        self.runtime_home
            .join("ledgers")
            .join("tasks")
            .join(self.owner_agent_id.as_str())
    }

    fn agent_state_dir(&self) -> PathBuf {
        self.runtime_home.join("state").join("agents")
    }

    fn agent_lifecycle_state_dir(&self) -> PathBuf {
        self.runtime_home
            .join("state")
            .join("agent-lifecycle")
            .join(self.owner_agent_id.as_str())
    }

    fn task_runtime_state_dir(&self) -> PathBuf {
        self.runtime_home
            .join("state")
            .join("task-runtime")
            .join(self.owner_agent_id.as_str())
    }

    fn lease_state_path(&self) -> PathBuf {
        self.task_runtime_state_dir().join("leases.json")
    }

    fn master_event_cursor_path(&self, master_agent_id: &AgentId) -> PathBuf {
        self.task_runtime_state_dir()
            .join("master-event-cursors")
            .join(format!("{}.json", master_agent_id.as_str()))
    }

    fn worker_control_state_dir(&self) -> PathBuf {
        self.task_runtime_state_dir().join("worker-control")
    }

    fn worker_control_ledger_dir(&self) -> PathBuf {
        self.runtime_home
            .join("ledgers")
            .join("worker-control")
            .join(self.owner_agent_id.as_str())
    }

    fn worker_control_snapshot_path(&self, task_id: &TaskId, execution_id: &str) -> PathBuf {
        self.worker_control_state_dir()
            .join(task_id.as_str())
            .join(format!("{execution_id}.json"))
    }

    fn worker_control_ledger_path(&self, task_id: &TaskId) -> PathBuf {
        self.worker_control_ledger_dir()
            .join(format!("{}.jsonl", task_id.as_str()))
    }

    fn task_snapshot_path(&self, task_id: &TaskId) -> PathBuf {
        self.task_state_dir()
            .join(format!("{}.json", task_id.as_str()))
    }

    fn task_ledger_path(&self, task_id: &TaskId) -> PathBuf {
        self.task_ledger_dir()
            .join(format!("{}.jsonl", task_id.as_str()))
    }

    fn agent_snapshot_path(&self, agent_id: &AgentId) -> PathBuf {
        self.agent_state_dir()
            .join(format!("{}.json", agent_id.as_str()))
    }

    fn agent_lifecycle_snapshot_path(&self, agent_id: &AgentId) -> PathBuf {
        self.agent_lifecycle_state_dir()
            .join(format!("{}.json", agent_id.as_str()))
    }
}

fn resolve_dispatch(
    dispatch: &TaskDispatchRequest,
    agents: &BTreeMap<AgentId, AgentSnapshot>,
) -> Result<Option<AgentId>, TaskError> {
    match dispatch {
        TaskDispatchRequest::None => Ok(None),
        TaskDispatchRequest::SelfAgent => Ok(agents
            .values()
            .find(|agent| matches!(agent.status, AgentStatus::Available))
            .map(|agent| agent.agent_id.clone())),
        TaskDispatchRequest::Agent { agent_id } => {
            let agent = agents
                .get(agent_id)
                .ok_or_else(|| TaskError::AgentNotFound(agent_id.as_str().to_owned()))?;
            if matches!(agent.status, AgentStatus::Available) {
                Ok(Some(agent_id.clone()))
            } else {
                Ok(None)
            }
        }
        TaskDispatchRequest::Auto { .. } => Ok(agents
            .values()
            .find(|agent| matches!(agent.status, AgentStatus::Available))
            .map(|agent| agent.agent_id.clone())),
    }
}

fn reconcile_running_leases(
    store: &TaskStore,
    state: &mut TaskRuntimeState,
    now: u64,
) -> Result<(), TaskError> {
    let task_ids = state.tasks.keys().cloned().collect::<Vec<_>>();
    let mut leases_changed = false;
    for task_id in task_ids {
        let Some(task) = state.tasks.get(&task_id).cloned() else {
            continue;
        };
        if !matches!(task.status, TaskStatus::Running) {
            if state.leases.remove(&task_id).is_some() {
                leases_changed = true;
            }
            continue;
        }
        let lease = state.leases.get(&task_id);
        if lease.is_none()
            && now
                <= task
                    .updated_at
                    .saturating_add(RUNNING_LEASE_ACQUISITION_GRACE_SECONDS)
        {
            continue;
        }
        let lease_valid = lease
            .map(|lease| {
                lease.status == "active"
                    && lease.task_id == task_id
                    && lease.expires_at > now
                    && task
                        .assignee
                        .as_ref()
                        .map(|assignee| assignee.agent_id == lease.agent_id)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        if lease_valid {
            continue;
        }

        let mut interrupted = task.clone();
        let event = build_event(
            &interrupted,
            Some(TaskStatus::Running),
            TaskStatus::Interrupted,
            "TaskInterrupted",
            &TaskActor {
                agent_id: store.owner_agent_id.clone(),
                source: "task.orchestration.recovery".to_owned(),
                session_id: interrupted.parent.session_id.clone(),
                turn_id: interrupted.parent.turn_id.clone(),
                trace_id: interrupted.parent.trace_id.clone(),
            },
            &TaskWatermark {
                metadata_id: None,
                hook: Some("TaskRuntime::boot".to_owned()),
                action_tool_call_id: None,
            },
            json!({"reason": "missing_or_expired_lease"}),
        );
        apply_event(&mut interrupted, &event);
        store.append_event_and_snapshot(&interrupted, &event)?;
        if let Some(assignee) = interrupted.assignee.as_ref()
            && let Some(agent) = state.agents.get_mut(&assignee.agent_id)
        {
            release_agent_task(agent, &TaskStatus::Interrupted);
            agent.last_seen_at = now;
            store.write_agent_snapshot(agent)?;
        }
        state.tasks.insert(task_id.clone(), interrupted);
        if state.leases.remove(&task_id).is_some() {
            leases_changed = true;
        }
    }
    if leases_changed {
        store.write_leases(&state.leases)?;
    }
    Ok(())
}

fn release_agent_task(agent: &mut AgentSnapshot, task_status: &TaskStatus) {
    agent.current_task_id = None;
    agent.current_execution_id = None;
    agent.current_cwd = None;
    match task_status {
        TaskStatus::Paused => {
            agent.status = AgentStatus::Paused;
            agent.running_tasks = 0;
            agent.queued_tasks = 0;
        }
        TaskStatus::Blocked | TaskStatus::ReviewSubmitted | TaskStatus::Approved => {
            agent.status = AgentStatus::Available;
            agent.running_tasks = 0;
            agent.queued_tasks = 0;
        }
        TaskStatus::Interrupted
        | TaskStatus::Cancelled
        | TaskStatus::Failed
        | TaskStatus::Closed => {
            agent.status = AgentStatus::Available;
            agent.running_tasks = 0;
            agent.queued_tasks = 0;
        }
        _ => {}
    }
}

fn reconcile_paused_agents(
    store: &TaskStore,
    state: &mut TaskRuntimeState,
    now: u64,
) -> Result<(), TaskError> {
    let paused_agent_ids = state
        .agents
        .values()
        .filter(|agent| matches!(agent.status, AgentStatus::Paused))
        .map(|agent| agent.agent_id.clone())
        .collect::<Vec<_>>();

    for agent_id in paused_agent_ids {
        let has_explicitly_paused_task = state.tasks.values().any(|task| {
            matches!(task.status, TaskStatus::Paused)
                && task
                    .assignee
                    .as_ref()
                    .map(|assignee| assignee.agent_id == agent_id)
                    .unwrap_or(false)
        });
        if has_explicitly_paused_task {
            continue;
        }

        let lifecycle = {
            let agent = state
                .agents
                .get_mut(&agent_id)
                .expect("paused agent id came from agent registry");
            agent.status = AgentStatus::Available;
            agent.current_task_id = None;
            agent.current_execution_id = None;
            agent.current_cwd = None;
            agent.running_tasks = 0;
            agent.queued_tasks = 0;
            agent.last_seen_at = now;
            store.write_agent_snapshot(agent)?;

            let mut lifecycle = state
                .lifecycle
                .get(&agent_id)
                .cloned()
                .unwrap_or_else(|| lifecycle_from_agent_snapshot(agent, now));
            lifecycle.state = AgentLifecycleState::Idle;
            lifecycle.state_entered_at = now;
            lifecycle.elapsed_ms = 0;
            lifecycle.current_task_id = None;
            lifecycle.current_turn_id = None;
            lifecycle.last_activity = lifecycle.current_activity.take();
            lifecycle.last_seen_at = now;
            lifecycle
        };
        store.write_agent_lifecycle_snapshot(&lifecycle)?;
        state.lifecycle.insert(agent_id, lifecycle);
    }
    Ok(())
}

fn validate_worker_control_request(request: &WorkerControlRequest) -> Result<(), TaskError> {
    require_text(request.task_id.as_str(), "task_id")?;
    require_text(&request.execution_id, "execution_id")?;
    require_text(request.agent_id.as_str(), "agent_id")?;
    if let Some(control_id) = request.control_id.as_ref() {
        require_text(control_id, "control_id")?;
    }
    match request.op {
        WorkerControlOp::AskAtSafePoint => {
            require_text(request.question.as_deref().unwrap_or_default(), "question")
        }
        WorkerControlOp::AddConstraint => require_text(
            request.constraint.as_deref().unwrap_or_default(),
            "constraint",
        ),
        WorkerControlOp::QueryStatus
        | WorkerControlOp::RequestCheckpoint
        | WorkerControlOp::RequestSubmissionNow
        | WorkerControlOp::Pause
        | WorkerControlOp::Resume
        | WorkerControlOp::Cancel => Ok(()),
    }
}

fn validate_worker_control_transition(
    op: &WorkerControlOp,
    status: &TaskStatus,
) -> Result<(), TaskError> {
    let valid = match op {
        WorkerControlOp::QueryStatus
        | WorkerControlOp::AskAtSafePoint
        | WorkerControlOp::AddConstraint
        | WorkerControlOp::RequestCheckpoint
        | WorkerControlOp::RequestSubmissionNow => !is_worker_control_terminal_status(status),
        WorkerControlOp::Pause => matches!(
            status,
            TaskStatus::Assigned | TaskStatus::Running | TaskStatus::Rejected
        ),
        WorkerControlOp::Resume => matches!(
            status,
            TaskStatus::Paused
                | TaskStatus::Blocked
                | TaskStatus::Rejected
                | TaskStatus::Assigned
                | TaskStatus::Interrupted
        ),
        WorkerControlOp::Cancel => !matches!(
            status,
            TaskStatus::Closed | TaskStatus::Cancelled | TaskStatus::Approved
        ),
    };
    if valid {
        Ok(())
    } else {
        Err(TaskError::InvalidTransition {
            from: status.clone(),
            event_type: op.event_type(),
        })
    }
}

fn is_worker_control_terminal_status(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Approved | TaskStatus::Closed | TaskStatus::Cancelled | TaskStatus::Failed
    )
}

fn build_worker_control_event(
    request: &WorkerControlRequest,
    task: &TaskSnapshot,
    agent: &AgentSnapshot,
    lifecycle: Option<&AgentLifecycleSnapshot>,
) -> WorkerControlEvent {
    let status = worker_control_event_status(&request.op);
    let payload = worker_control_payload(request, task, agent, lifecycle);
    WorkerControlEvent {
        schema_version: 1,
        control_id: request
            .control_id
            .clone()
            .unwrap_or_else(|| generated_worker_control_id(task, &request.execution_id)),
        op: request.op.clone(),
        status: status.to_owned(),
        task_id: request.task_id.clone(),
        execution_id: request.execution_id.clone(),
        agent_id: request.agent_id.clone(),
        created_at: now_unix_seconds(),
        summary: worker_control_summary(request),
        payload,
        actor: request.actor.clone(),
        watermark: request.watermark.clone(),
    }
}

fn worker_control_event_status(op: &WorkerControlOp) -> &'static str {
    match op {
        WorkerControlOp::QueryStatus => "observed",
        WorkerControlOp::AskAtSafePoint
        | WorkerControlOp::AddConstraint
        | WorkerControlOp::RequestCheckpoint
        | WorkerControlOp::RequestSubmissionNow => "queued",
        WorkerControlOp::Pause | WorkerControlOp::Resume | WorkerControlOp::Cancel => "applied",
    }
}

fn worker_control_summary(request: &WorkerControlRequest) -> String {
    match request.op {
        WorkerControlOp::QueryStatus => format!(
            "queried status for task {} execution {}",
            request.task_id.as_str(),
            request.execution_id
        ),
        WorkerControlOp::AskAtSafePoint => format!(
            "queued safe-point question for task {}",
            request.task_id.as_str()
        ),
        WorkerControlOp::AddConstraint => {
            format!("queued constraint for task {}", request.task_id.as_str())
        }
        WorkerControlOp::RequestCheckpoint => {
            format!("requested checkpoint for task {}", request.task_id.as_str())
        }
        WorkerControlOp::RequestSubmissionNow => {
            format!("requested submission for task {}", request.task_id.as_str())
        }
        WorkerControlOp::Pause => format!("requested pause for task {}", request.task_id.as_str()),
        WorkerControlOp::Resume => {
            format!("requested resume for task {}", request.task_id.as_str())
        }
        WorkerControlOp::Cancel => {
            format!("requested cancel for task {}", request.task_id.as_str())
        }
    }
}

fn worker_control_payload(
    request: &WorkerControlRequest,
    task: &TaskSnapshot,
    agent: &AgentSnapshot,
    lifecycle: Option<&AgentLifecycleSnapshot>,
) -> Value {
    let mut payload = json!({
        "op": request.op.as_str(),
        "task_status": task.status.clone(),
        "active_execution_id": task.active_execution_id.clone(),
        "agent_status": agent.status.clone(),
        "agent_current_task_id": agent.current_task_id.as_ref().map(TaskId::as_str),
        "agent_current_execution_id": agent.current_execution_id.clone(),
    });
    if let Some(lifecycle) = lifecycle
        && let Some(map) = payload.as_object_mut()
    {
        map.insert("lifecycle_state".to_owned(), json!(lifecycle.state));
        map.insert(
            "lifecycle_elapsed_ms".to_owned(),
            json!(lifecycle.elapsed_ms),
        );
        map.insert(
            "lifecycle_activity".to_owned(),
            lifecycle
                .current_activity
                .as_ref()
                .map(|activity| json!(activity))
                .unwrap_or(Value::Null),
        );
    }
    if let Some(map) = payload.as_object_mut() {
        if let Some(question) = request.question.as_ref() {
            map.insert("question".to_owned(), json!(question));
        }
        if let Some(constraint) = request.constraint.as_ref() {
            map.insert("constraint".to_owned(), json!(constraint));
        }
        if let Some(note) = request.note.as_ref() {
            map.insert("note".to_owned(), json!(note));
        }
    }
    payload
}

fn generated_worker_control_id(task: &TaskSnapshot, execution_id: &str) -> String {
    format!(
        "wctl-{}-{}-{}",
        task.task_id.as_str(),
        execution_id,
        now_unix_nanos()
    )
}

fn worker_control_task_actor(request: &WorkerControlRequest) -> TaskActor {
    TaskActor {
        agent_id: request.agent_id.clone(),
        source: "worker.control".to_owned(),
        session_id: request.actor.session_id.clone(),
        turn_id: request.actor.turn_id.clone(),
        trace_id: request.actor.trace_id.clone(),
    }
}

fn normalize_cursor(cursor: Option<String>) -> Result<Option<String>, TaskError> {
    match cursor {
        Some(cursor) if cursor.trim().is_empty() => Err(TaskError::MissingField("event_cursor")),
        Some(cursor) => Ok(Some(cursor)),
        None => Ok(None),
    }
}

fn task_event_cursor(event: &TaskLedgerEvent) -> String {
    format!(
        "{:020}:{}:{:020}:{}",
        event.timestamp,
        event.task_id.as_str(),
        event.seq,
        event.event_id
    )
}

#[cfg(test)]
fn task_event_legacy_cursor_prefix(event: &TaskLedgerEvent) -> String {
    format!(
        "{:020}:{}:{:020}",
        event.timestamp,
        event.task_id.as_str(),
        event.seq
    )
}

fn event_inbox_start_index_after_cursor(
    entries: &[TaskEventInboxEntry],
    cursor: &str,
) -> Result<usize, TaskError> {
    if let Some(index) = entries.iter().rposition(|event| event.cursor == cursor) {
        return Ok(index.saturating_add(1));
    }
    let legacy_prefix = format!("{cursor}:");
    if let Some(index) = entries
        .iter()
        .rposition(|event| event.cursor.starts_with(&legacy_prefix))
    {
        return Ok(index.saturating_add(1));
    }
    Err(TaskError::CursorNotFound(cursor.to_owned()))
}

fn task_event_inbox_entry(event: &TaskLedgerEvent) -> Option<TaskEventInboxEntry> {
    let kind = master_visible_event_kind(&event.event_type)?;
    Some(TaskEventInboxEntry {
        schema_version: 1,
        cursor: task_event_cursor(event),
        event_id: event.event_id.clone(),
        kind: kind.to_owned(),
        task_id: event.task_id.clone(),
        execution_id: event
            .payload
            .get("execution_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        agent_id: event
            .payload
            .get("agent_id")
            .and_then(Value::as_str)
            .map(AgentId::new)
            .or_else(|| {
                event
                    .payload
                    .get("claim_agent_id")
                    .and_then(Value::as_str)
                    .map(AgentId::new)
            }),
        created_at: event.timestamp,
        payload: event.payload.clone(),
    })
}

fn master_visible_event_kind(event_type: &str) -> Option<&'static str> {
    match event_type {
        "TaskCreated" => Some("task_created"),
        "TaskWaitingAgent" => Some("task_waiting_agent"),
        "TaskAssigned" => Some("task_assigned"),
        "TaskResumed" => Some("execution_started"),
        "TaskHeartbeat" => Some("execution_heartbeat"),
        "TaskExecutionRecorded" => Some("progress_reported"),
        "TaskExecutionRecovering" => Some("execution_recovering"),
        "TaskBlocked" => Some("execution_blocked"),
        "TaskSchedulerTick" => Some("scheduler_tick"),
        "TaskReviewSubmitted" => Some("review_ready"),
        "TaskReviewRejected" => Some("review_rejected"),
        "TaskReviewApproved" => Some("review_approved"),
        "TaskClosed" => Some("task_closed"),
        "TaskInterrupted" => Some("execution_interrupted"),
        "TaskCancelled" => Some("task_cancelled"),
        _ => None,
    }
}

fn master_poll_classifications(
    event_inbox: &TaskEventInboxProjection,
    task_board: &TaskBoardProjection,
    agent_board: &AgentBoardProjection,
) -> Vec<MasterPollClassification> {
    let mut classifications = Vec::new();
    for event in &event_inbox.events {
        match event.kind.as_str() {
            "review_ready" => classifications.push(MasterPollClassification {
                kind: "review_ready".to_owned(),
                summary: format!("task {} is ready for review", event.task_id.as_str()),
                task_id: Some(event.task_id.clone()),
                execution_id: event.execution_id.clone(),
                agent_id: event.agent_id.clone(),
                recommended_actions: vec![
                    "inspect_submission".to_owned(),
                    "approve_submission".to_owned(),
                    "reject_submission".to_owned(),
                ],
            }),
            "execution_blocked" => classifications.push(MasterPollClassification {
                kind: "blocked".to_owned(),
                summary: format!("task {} is blocked", event.task_id.as_str()),
                task_id: Some(event.task_id.clone()),
                execution_id: event.execution_id.clone(),
                agent_id: event.agent_id.clone(),
                recommended_actions: vec![
                    "inspect_blocker".to_owned(),
                    "split_unblock_task".to_owned(),
                    "ask_user".to_owned(),
                ],
            }),
            "scheduler_tick" => {
                if let Some(fact) = event.payload.get("scheduler_fact") {
                    if scheduler_fact_kind_name(fact) == Some("stale") {
                        classifications.push(MasterPollClassification {
                            kind: "stale".to_owned(),
                            summary: format!("task {} is stale", event.task_id.as_str()),
                            task_id: Some(event.task_id.clone()),
                            execution_id: event.execution_id.clone(),
                            agent_id: event.agent_id.clone(),
                            recommended_actions: vec![
                                "query_agent_lifecycle".to_owned(),
                                "wait_with_next_check".to_owned(),
                            ],
                        });
                    }
                    if scheduler_fact_kind_name(fact) == Some("soft_timeout") {
                        classifications.push(MasterPollClassification {
                            kind: "soft_timeout".to_owned(),
                            summary: format!("task {} hit soft timeout", event.task_id.as_str()),
                            task_id: Some(event.task_id.clone()),
                            execution_id: event.execution_id.clone(),
                            agent_id: event.agent_id.clone(),
                            recommended_actions: vec![
                                "query_agent_lifecycle".to_owned(),
                                "wait_with_next_check".to_owned(),
                            ],
                        });
                    }
                    if scheduler_fact_kind_name(fact) == Some("hard_timeout") {
                        classifications.push(MasterPollClassification {
                            kind: "hard_timeout".to_owned(),
                            summary: format!("task {} hit hard timeout", event.task_id.as_str()),
                            task_id: Some(event.task_id.clone()),
                            execution_id: event.execution_id.clone(),
                            agent_id: event.agent_id.clone(),
                            recommended_actions: vec![
                                "query_agent_lifecycle".to_owned(),
                                "cancel_or_reassign_execution".to_owned(),
                            ],
                        });
                    }
                }
            }
            _ => {}
        }
    }
    for task in &task_board.blocked {
        push_unique_classification(
            &mut classifications,
            "blocked",
            Some(task.task_id.clone()),
            task.active_execution_id.clone(),
            task.assignee
                .as_ref()
                .map(|assignee| assignee.agent_id.clone()),
            format!("task {} remains blocked", task.task_id.as_str()),
            vec![
                "inspect_blocker".to_owned(),
                "split_unblock_task".to_owned(),
                "ask_user".to_owned(),
            ],
        );
    }
    for task in &task_board.review_ready {
        push_unique_classification(
            &mut classifications,
            "review_ready",
            Some(task.task_id.clone()),
            task.active_execution_id.clone(),
            task.assignee
                .as_ref()
                .map(|assignee| assignee.agent_id.clone()),
            format!("task {} remains ready for review", task.task_id.as_str()),
            vec![
                "inspect_submission".to_owned(),
                "approve_submission".to_owned(),
                "reject_submission".to_owned(),
            ],
        );
    }
    for task in &task_board.stale {
        push_unique_classification(
            &mut classifications,
            "stale",
            Some(task.task_id.clone()),
            task.active_execution_id.clone(),
            task.assignee
                .as_ref()
                .map(|assignee| assignee.agent_id.clone()),
            format!("task {} has stale scheduler facts", task.task_id.as_str()),
            vec![
                "query_agent_lifecycle".to_owned(),
                "wait_with_next_check".to_owned(),
            ],
        );
    }
    for agent in &agent_board.agents {
        if matches!(agent.state, AgentLifecycleState::Idle) {
            push_unique_classification(
                &mut classifications,
                "idle_worker",
                None,
                None,
                Some(agent.agent_id.clone()),
                format!("agent {} is idle", agent.agent_id.as_str()),
                vec![
                    "dispatch_ready_task".to_owned(),
                    "wait_with_next_check".to_owned(),
                ],
            );
        }
    }
    if task_board
        .tasks
        .iter()
        .any(|task| matches!(task.status, TaskStatus::WaitingAgent | TaskStatus::Assigned))
    {
        push_unique_classification(
            &mut classifications,
            "ready_task",
            None,
            None,
            None,
            "ready task is waiting for worker progress".to_owned(),
            vec![
                "dispatch_ready_task".to_owned(),
                "wait_with_next_check".to_owned(),
            ],
        );
    }
    if !task_board.tasks.is_empty()
        && task_board
            .tasks
            .iter()
            .all(|task| matches!(task.status, TaskStatus::Closed))
    {
        push_unique_classification(
            &mut classifications,
            "all_closed",
            None,
            None,
            None,
            "all visible tasks are closed".to_owned(),
            vec!["close_big_task".to_owned(), "report_to_user".to_owned()],
        );
    }
    classifications
}

fn scheduler_fact_kind_name(fact: &Value) -> Option<&str> {
    fact.get("fact")
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
}

fn push_unique_classification(
    classifications: &mut Vec<MasterPollClassification>,
    kind: &str,
    task_id: Option<TaskId>,
    execution_id: Option<String>,
    agent_id: Option<AgentId>,
    summary: String,
    recommended_actions: Vec<String>,
) {
    let duplicate = classifications.iter().any(|existing| {
        existing.kind == kind
            && existing.task_id == task_id
            && existing.execution_id == execution_id
            && existing.agent_id == agent_id
    });
    if duplicate {
        return;
    }
    classifications.push(MasterPollClassification {
        kind: kind.to_owned(),
        summary,
        task_id,
        execution_id,
        agent_id,
        recommended_actions,
    });
}

fn project_task_event_to_lifecycle(
    state: &mut TaskRuntimeState,
    task: &TaskSnapshot,
    event_type: &str,
    payload: &Value,
) {
    let Some(assignee) = task.assignee.as_ref() else {
        return;
    };
    let Some(agent) = state.agents.get(&assignee.agent_id) else {
        return;
    };
    let now = now_unix_seconds();
    let mut snapshot = state
        .lifecycle
        .get(&assignee.agent_id)
        .cloned()
        .unwrap_or_else(|| lifecycle_from_agent_snapshot(agent, now));
    let previous = snapshot.current_activity.clone();
    let execution_id = task.active_execution_id.clone().or_else(|| {
        payload
            .get("execution_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    });
    let (state_kind, activity_kind, summary, retry_count) = match event_type {
        "TaskResumed" if matches!(task.status, TaskStatus::Rejected) => (
            AgentLifecycleState::Retrying,
            "retrying",
            "retrying rejected task".to_owned(),
            None,
        ),
        "TaskResumed" => (
            AgentLifecycleState::Running,
            "running",
            "claimed task execution".to_owned(),
            None,
        ),
        "TaskExecutionRecorded" => (
            AgentLifecycleState::Progressing,
            "progressing",
            payload
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("recorded execution progress")
                .to_owned(),
            None,
        ),
        "TaskExecutionRecovering" => (
            AgentLifecycleState::Recovering,
            "recovering",
            payload
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("recovering task execution")
                .to_owned(),
            payload
                .get("retry_count")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
        ),
        "TaskBlocked" => (
            AgentLifecycleState::Blocked,
            "blocked",
            payload
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("blocked")
                .to_owned(),
            None,
        ),
        "TaskReviewSubmitted" => (
            AgentLifecycleState::WaitingReview,
            "review_ready",
            payload
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("submitted for review")
                .to_owned(),
            None,
        ),
        "TaskReviewRejected" => (
            AgentLifecycleState::Retrying,
            "retrying",
            payload
                .get("reject_reason")
                .and_then(Value::as_str)
                .unwrap_or("review rejected")
                .to_owned(),
            None,
        ),
        "TaskReviewApproved" => (
            AgentLifecycleState::Approved,
            "approved",
            "review approved".to_owned(),
            None,
        ),
        "TaskClosed" => (
            AgentLifecycleState::Closed,
            "closed",
            "task closed".to_owned(),
            None,
        ),
        _ => return,
    };
    snapshot.state = state_kind;
    snapshot.state_entered_at = now;
    snapshot.elapsed_ms = 0;
    snapshot.current_task_id = Some(task.task_id.clone());
    snapshot.current_execution_id = execution_id;
    snapshot.current_turn_id = payload
        .get("turn_id")
        .and_then(Value::as_str)
        .map(TurnId::new);
    snapshot.last_activity = previous;
    snapshot.current_activity = Some(AgentLifecycleActivity {
        kind: activity_kind.to_owned(),
        semantic_summary: summary,
        target: Some(task.task_id.as_str().to_owned()),
        started_at: now,
        elapsed_ms: 0,
        tool_name: None,
        model: None,
        retry_count,
        visibility: "compact".to_owned(),
    });
    snapshot.last_seen_at = now;
    if event_type == "TaskExecutionRecovering" {
        snapshot.stats.tool_failure_count = snapshot.stats.tool_failure_count.saturating_add(1);
    }
    if event_type == "TaskBlocked" {
        snapshot.stats.blocked_count = snapshot.stats.blocked_count.saturating_add(1);
    }
    state.lifecycle.insert(assignee.agent_id.clone(), snapshot);
}

fn lifecycle_from_agent_snapshot(agent: &AgentSnapshot, now: u64) -> AgentLifecycleSnapshot {
    let state = match agent.status {
        AgentStatus::Available => AgentLifecycleState::Idle,
        AgentStatus::Busy => {
            if agent.running_tasks > 0 {
                AgentLifecycleState::Running
            } else {
                AgentLifecycleState::Assigned
            }
        }
        AgentStatus::Paused => AgentLifecycleState::Blocked,
        AgentStatus::Offline | AgentStatus::Closed => AgentLifecycleState::Offline,
        AgentStatus::Closing => AgentLifecycleState::Offline,
        AgentStatus::Failed => AgentLifecycleState::Failed,
    };
    AgentLifecycleSnapshot {
        schema_version: 1,
        agent_id: agent.agent_id.clone(),
        role: "worker".to_owned(),
        alive: !matches!(agent.status, AgentStatus::Closed | AgentStatus::Offline),
        state,
        state_entered_at: agent.last_seen_at,
        elapsed_ms: now.saturating_sub(agent.last_seen_at).saturating_mul(1000),
        current_task_id: agent.current_task_id.clone(),
        current_execution_id: agent.current_execution_id.clone(),
        current_turn_id: None,
        current_activity: Some(AgentLifecycleActivity {
            kind: if agent.current_task_id.is_some() {
                "assigned".to_owned()
            } else {
                "idle".to_owned()
            },
            semantic_summary: if let Some(task_id) = agent.current_task_id.as_ref() {
                format!("assigned to {}", task_id.as_str())
            } else {
                "idle".to_owned()
            },
            target: agent
                .current_task_id
                .as_ref()
                .map(|task_id| task_id.as_str().to_owned()),
            started_at: agent.last_seen_at,
            elapsed_ms: now.saturating_sub(agent.last_seen_at).saturating_mul(1000),
            tool_name: None,
            model: None,
            retry_count: None,
            visibility: "compact".to_owned(),
        }),
        last_activity: None,
        stats: AgentLifecycleStats {
            turn_count: 0,
            model_request_count: 0,
            model_retry_count: 0,
            tool_call_count: 0,
            tool_failure_count: 0,
            schema_polish_count: 0,
            provider_error_count: 0,
            blocked_count: 0,
            current_model: None,
        },
        last_seen_at: agent.last_seen_at,
        next_check_at: None,
    }
}

type StatsUpdate = fn(&mut AgentLifecycleStats);

fn lifecycle_event_projection(
    event: &AgentLifecycleEvent,
    now: u64,
) -> (
    AgentLifecycleState,
    AgentLifecycleActivity,
    Option<String>,
    StatsUpdate,
) {
    match event {
        AgentLifecycleEvent::ModelThinking { model, .. } => (
            AgentLifecycleState::ModelThinking,
            AgentLifecycleActivity {
                kind: "model_thinking".to_owned(),
                semantic_summary: "waiting for model response".to_owned(),
                target: None,
                started_at: now,
                elapsed_ms: 0,
                tool_name: None,
                model: Some(model.clone()),
                retry_count: None,
                visibility: "compact".to_owned(),
            },
            Some(model.clone()),
            |stats| {
                stats.model_request_count = stats.model_request_count.saturating_add(1);
            },
        ),
        AgentLifecycleEvent::ToolRunning {
            tool_name, target, ..
        } => (
            AgentLifecycleState::ToolRunning,
            AgentLifecycleActivity {
                kind: "tool_running".to_owned(),
                semantic_summary: target
                    .as_ref()
                    .map(|target| format!("running {tool_name} on {target}"))
                    .unwrap_or_else(|| format!("running {tool_name}")),
                target: target.clone(),
                started_at: now,
                elapsed_ms: 0,
                tool_name: Some(tool_name.clone()),
                model: None,
                retry_count: None,
                visibility: "compact".to_owned(),
            },
            None,
            |stats| {
                stats.tool_call_count = stats.tool_call_count.saturating_add(1);
            },
        ),
        AgentLifecycleEvent::Recovering { summary, .. } => (
            AgentLifecycleState::Recovering,
            AgentLifecycleActivity {
                kind: "recovering".to_owned(),
                semantic_summary: summary.clone(),
                target: None,
                started_at: now,
                elapsed_ms: 0,
                tool_name: None,
                model: None,
                retry_count: None,
                visibility: "compact".to_owned(),
            },
            None,
            |stats| {
                stats.tool_failure_count = stats.tool_failure_count.saturating_add(1);
            },
        ),
        AgentLifecycleEvent::Blocked { reason, .. } => (
            AgentLifecycleState::Blocked,
            AgentLifecycleActivity {
                kind: "blocked".to_owned(),
                semantic_summary: reason.clone(),
                target: None,
                started_at: now,
                elapsed_ms: 0,
                tool_name: None,
                model: None,
                retry_count: None,
                visibility: "compact".to_owned(),
            },
            None,
            |stats| {
                stats.blocked_count = stats.blocked_count.saturating_add(1);
            },
        ),
    }
}

fn is_terminal_status(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Closed | TaskStatus::Cancelled | TaskStatus::Failed | TaskStatus::Approved
    )
}

fn sort_task_snapshots(tasks: &mut [TaskSnapshot]) {
    tasks.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.created_at.cmp(&right.created_at))
            .then_with(|| left.task_id.cmp(&right.task_id))
    });
}

fn validate_create_request(request: &TaskCreateRequest) -> Result<(), TaskError> {
    require_text(&request.title, "title")?;
    require_text(&request.content, "content")?;
    require_text(&request.goal, "goal")?;
    if request.deliverables.is_empty() {
        return Err(TaskError::MissingField("deliverables"));
    }
    if request.acceptance.is_empty() {
        return Err(TaskError::MissingField("acceptance"));
    }
    Ok(())
}

fn validate_execution_fact(fact: &ExecutionFact) -> Result<(), TaskError> {
    require_text(&fact.execution_id, "execution_id")?;
    match &fact.kind {
        ExecutionFactKind::Running {
            phase,
            summary,
            evidence,
        } => {
            require_text(phase, "phase")?;
            require_text(summary, "summary")?;
            require_non_empty(evidence, "evidence")
        }
        ExecutionFactKind::Recovering {
            summary,
            evidence,
            retry_count: _,
        } => {
            require_text(summary, "summary")?;
            require_non_empty(evidence, "evidence")
        }
        ExecutionFactKind::Blocked { reason, evidence } => {
            require_text(reason, "reason")?;
            require_non_empty(evidence, "evidence")
        }
        ExecutionFactKind::Interrupted { reason, evidence } => {
            require_text(reason, "reason")?;
            require_non_empty(evidence, "evidence")
        }
        ExecutionFactKind::ReviewReady {
            summary,
            deliverables,
            evidence,
        } => {
            require_text(summary, "summary")?;
            require_non_empty(deliverables, "deliverables")?;
            require_non_empty(evidence, "evidence")
        }
    }
}

fn validate_scheduler_tick_request(request: &SchedulerTickRequest) -> Result<(), TaskError> {
    if request.stale_after_seconds == 0 {
        return Err(TaskError::MissingField("stale_after_seconds"));
    }
    if request.soft_timeout_seconds == 0 {
        return Err(TaskError::MissingField("soft_timeout_seconds"));
    }
    if request.hard_timeout_seconds == 0 {
        return Err(TaskError::MissingField("hard_timeout_seconds"));
    }
    Ok(())
}

fn scheduler_facts_for_task(
    task: &TaskSnapshot,
    leases: &BTreeMap<TaskId, TaskLease>,
    request: &SchedulerTickRequest,
) -> Vec<SchedulerFact> {
    let mut facts = Vec::new();
    let agent_id = task
        .assignee
        .as_ref()
        .map(|assignee| assignee.agent_id.clone());
    let last_progress = task
        .last_progress_at
        .or_else(|| leases.get(&task.task_id).map(|lease| lease.heartbeat_at))
        .unwrap_or(task.updated_at);
    let idle_seconds = request.now.saturating_sub(last_progress);
    if idle_seconds >= request.stale_after_seconds {
        facts.push(SchedulerFact {
            task_id: task.task_id.clone(),
            agent_id: agent_id.clone(),
            observed_at: request.now,
            fact: SchedulerFactKind::Stale { idle_seconds },
            recommendation: "query_agent_status".to_owned(),
        });
    }
    let elapsed_seconds = request.now.saturating_sub(task.created_at);
    if elapsed_seconds >= request.hard_timeout_seconds {
        facts.push(SchedulerFact {
            task_id: task.task_id.clone(),
            agent_id: agent_id.clone(),
            observed_at: request.now,
            fact: SchedulerFactKind::HardTimeout { elapsed_seconds },
            recommendation: "ask_master_for_decision".to_owned(),
        });
    } else if elapsed_seconds >= request.soft_timeout_seconds {
        facts.push(SchedulerFact {
            task_id: task.task_id.clone(),
            agent_id,
            observed_at: request.now,
            fact: SchedulerFactKind::SoftTimeout { elapsed_seconds },
            recommendation: "prioritize_status_check".to_owned(),
        });
    }
    facts
}

fn validate_transition(
    from: &TaskStatus,
    to: &TaskStatus,
    event_type: &'static str,
) -> Result<(), TaskError> {
    let valid = match event_type {
        "TaskProgressed" => !matches!(from, TaskStatus::Closed | TaskStatus::Cancelled),
        "TaskExecutionRecorded" => matches!(from, TaskStatus::Running),
        "TaskExecutionRecovering" => matches!(from, TaskStatus::Running),
        "TaskBlocked" => matches!(
            from,
            TaskStatus::Assigned
                | TaskStatus::Running
                | TaskStatus::Paused
                | TaskStatus::Rejected
                | TaskStatus::Interrupted
        ),
        "TaskPaused" => matches!(
            from,
            TaskStatus::Assigned | TaskStatus::Running | TaskStatus::Rejected
        ),
        "TaskResumed" => matches!(
            from,
            TaskStatus::Paused
                | TaskStatus::Blocked
                | TaskStatus::Rejected
                | TaskStatus::Assigned
                | TaskStatus::Interrupted
        ),
        "TaskReviewSubmitted" => matches!(from, TaskStatus::Running | TaskStatus::Assigned),
        "TaskReviewApproved" => matches!(from, TaskStatus::ReviewSubmitted),
        "TaskReviewRejected" => matches!(from, TaskStatus::ReviewSubmitted),
        "TaskClosed" => matches!(from, TaskStatus::Approved),
        "TaskCancelled" => !matches!(
            from,
            TaskStatus::Closed | TaskStatus::Cancelled | TaskStatus::Approved
        ),
        "TaskHeartbeat" => matches!(from, TaskStatus::Running),
        "TaskInterrupted" => matches!(from, TaskStatus::Running),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(TaskError::InvalidTransition {
            from: from.clone(),
            event_type,
        })
    }?;
    let status_matches = match event_type {
        "TaskProgressed" | "TaskExecutionRecovering" => from == to,
        "TaskReviewRejected" => matches!(to, TaskStatus::Rejected),
        _ => true,
    };
    if status_matches {
        Ok(())
    } else {
        Err(TaskError::InvalidTransition {
            from: from.clone(),
            event_type,
        })
    }
}

fn require_text(value: &str, field: &'static str) -> Result<(), TaskError> {
    if value.trim().is_empty() {
        Err(TaskError::MissingField(field))
    } else {
        Ok(())
    }
}

fn require_non_empty<T>(value: &[T], field: &'static str) -> Result<(), TaskError> {
    if value.is_empty() {
        Err(TaskError::MissingField(field))
    } else {
        Ok(())
    }
}

fn build_event(
    snapshot: &TaskSnapshot,
    from_status: Option<TaskStatus>,
    to_status: TaskStatus,
    event_type: &str,
    actor: &TaskActor,
    watermark: &TaskWatermark,
    payload: Value,
) -> TaskLedgerEvent {
    let seq = snapshot.last_event_seq.saturating_add(1);
    TaskLedgerEvent {
        schema_version: 1,
        task_id: snapshot.task_id.clone(),
        seq,
        event_id: format!("{}:{seq}", snapshot.task_id.as_str()),
        event_type: event_type.to_owned(),
        from_status,
        to_status,
        timestamp: now_unix_seconds(),
        actor: actor.clone(),
        watermark: watermark.clone(),
        payload,
    }
}

fn apply_event(snapshot: &mut TaskSnapshot, event: &TaskLedgerEvent) {
    snapshot.status = event.to_status.clone();
    snapshot.updated_at = event.timestamp;
    snapshot.last_event_seq = event.seq;
    snapshot.last_event_id = event.event_id.clone();
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, TaskError> {
    let raw = fs::read_to_string(path).map_err(io_err)?;
    serde_json::from_str(&raw).map_err(json_err)
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), TaskError> {
    ensure_parent_dir(path)?;
    let temp = path.with_extension(format!("tmp-{}", now_unix_seconds()));
    let raw = serde_json::to_string_pretty(value).map_err(json_err)?;
    fs::write(&temp, raw).map_err(io_err)?;
    fs::rename(&temp, path).map_err(io_err)
}

fn ensure_parent_dir(path: &Path) -> Result<(), TaskError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    Ok(())
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn io_err(err: std::io::Error) -> TaskError {
    TaskError::Persistence(err.to_string())
}

fn json_err(err: serde_json::Error) -> TaskError {
    TaskError::Persistence(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_task_writes_ledger_snapshot_and_recovers_on_boot() {
        let runtime_home = temp_runtime_home("task-create-recover");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");

        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");

        assert_eq!(outcome.task.status, TaskStatus::Assigned);
        assert_eq!(outcome.events.len(), 2);
        assert!(
            runtime_home
                .join("ledgers/tasks/master")
                .join(format!("{}.jsonl", outcome.task.task_id.as_str()))
                .is_file()
        );

        let recovered = TaskRuntime::boot(&runtime_home, agent_id).expect("recover");
        let task = recovered
            .query_task(&outcome.task.task_id)
            .expect("query recovered task");

        assert_eq!(task.status, TaskStatus::Assigned);
        assert_eq!(task.last_event_seq, 2);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn create_with_no_dispatch_waits_for_agent() {
        let runtime_home = temp_runtime_home("task-waiting-agent");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let mut request = sample_create_request(agent_id);
        request.dispatch = TaskDispatchRequest::None;

        let outcome = runtime.create_task(request).expect("create");

        assert_eq!(outcome.task.status, TaskStatus::WaitingAgent);
        assert!(outcome.task.assignee.is_none());
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn boot_registers_self_agent_as_available() {
        let runtime_home = temp_runtime_home("task-self-agent");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");

        let agent = runtime.query_agent(&agent_id).expect("self agent");

        assert_eq!(agent.status, AgentStatus::Available);
        assert!(agent.capabilities.contains(&"code_edit".to_owned()));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn review_reject_resume_submit_approve_close_lifecycle_persists() {
        let runtime_home = temp_runtime_home("task-review-lifecycle");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");
        let task_id = outcome.task.task_id.clone();
        let actor = sample_actor(agent_id);
        let watermark = sample_watermark();

        let running = runtime
            .resume_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect("resume to running");
        assert_eq!(running.task.status, TaskStatus::Running);

        let submitted = runtime
            .submit_review(TaskReviewSubmission {
                task_id: task_id.clone(),
                summary: "ready for review".to_owned(),
                deliverables: vec!["code".to_owned()],
                evidence: vec!["tests passed".to_owned()],
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect("submit review");
        assert_eq!(submitted.task.status, TaskStatus::ReviewSubmitted);

        let rejected = runtime
            .reject_review(TaskReviewRejection {
                task_id: task_id.clone(),
                reject_reason: "needs online proof".to_owned(),
                next_requirements: vec!["run browser proof".to_owned()],
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect("reject");
        assert_eq!(rejected.task.status, TaskStatus::Rejected);

        let running_again = runtime
            .resume_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect("resume rejected");
        assert_eq!(running_again.task.status, TaskStatus::Running);

        runtime
            .submit_review(TaskReviewSubmission {
                task_id: task_id.clone(),
                summary: "ready again".to_owned(),
                deliverables: vec!["code".to_owned()],
                evidence: vec!["online proof passed".to_owned()],
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect("submit second review");
        let approved = runtime
            .approve_review(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect("approve");
        assert_eq!(approved.task.status, TaskStatus::Approved);
        let closed = runtime
            .close_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor,
                watermark,
            })
            .expect("close");
        assert_eq!(closed.task.status, TaskStatus::Closed);

        let recovered = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("recover");
        let task = recovered.query_task(&task_id).expect("query recovered");
        assert_eq!(task.status, TaskStatus::Closed);
        assert_eq!(task.last_event_seq, 11);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn close_before_review_approval_is_rejected() {
        let runtime_home = temp_runtime_home("task-close-rejected");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");

        let err = runtime
            .close_task(TaskMutationRequest {
                task_id: outcome.task.task_id,
                actor: sample_actor(agent_id),
                watermark: sample_watermark(),
            })
            .expect_err("assigned task cannot close");

        assert!(matches!(err, TaskError::InvalidTransition { .. }));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn phase2a_close_requires_approved_review_for_blocked_and_rejected() {
        let runtime_home = temp_runtime_home("phase2a-close-requires-approved");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let actor = sample_actor(agent_id.clone());
        let watermark = sample_watermark();

        let mut blocked_request = sample_create_request(agent_id.clone());
        blocked_request.task_id = Some(TaskId::new("task-phase2a-close-blocked"));
        let blocked_task = runtime
            .create_task(blocked_request)
            .expect("create blocked candidate")
            .task;
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: "exec-phase2a-close-blocked".to_owned(),
                task_id: blocked_task.task_id.clone(),
                agent_id: agent_id.clone(),
                turn_id: Some(TurnId::new("turn-phase2a-close-blocked")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Blocked {
                    reason: "still blocked".to_owned(),
                    evidence: vec!["blocker evidence".to_owned()],
                },
                watermark: watermark.clone(),
            })
            .expect("block task");
        let blocked_close = runtime
            .close_task(TaskMutationRequest {
                task_id: blocked_task.task_id,
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect_err("blocked task cannot close");
        assert!(matches!(
            blocked_close,
            TaskError::InvalidTransition {
                from: TaskStatus::Blocked,
                ..
            }
        ));

        let _ = fs::remove_dir_all(runtime_home);

        let runtime_home = temp_runtime_home("phase2a-close-requires-approved-rejected");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot rejected");
        let actor = sample_actor(agent_id.clone());
        let watermark = sample_watermark();
        let mut rejected_request = sample_create_request(agent_id.clone());
        rejected_request.task_id = Some(TaskId::new("task-phase2a-close-rejected"));
        let rejected_task = runtime
            .create_task(rejected_request)
            .expect("create rejected candidate")
            .task;
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: "exec-phase2a-close-rejected".to_owned(),
                task_id: rejected_task.task_id.clone(),
                agent_id: agent_id.clone(),
                turn_id: Some(TurnId::new("turn-phase2a-close-rejected")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::ReviewReady {
                    summary: "ready but not acceptable".to_owned(),
                    deliverables: vec!["draft".to_owned()],
                    evidence: vec!["draft evidence".to_owned()],
                },
                watermark: watermark.clone(),
            })
            .expect("submit review");
        runtime
            .reject_review(TaskReviewRejection {
                task_id: rejected_task.task_id.clone(),
                reject_reason: "needs retry".to_owned(),
                next_requirements: vec!["retry".to_owned()],
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect("reject review");
        let rejected_close = runtime
            .close_task(TaskMutationRequest {
                task_id: rejected_task.task_id,
                actor,
                watermark,
            })
            .expect_err("rejected task cannot close");
        assert!(matches!(
            rejected_close,
            TaskError::InvalidTransition {
                from: TaskStatus::Rejected,
                ..
            }
        ));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn resume_creates_lease_and_heartbeat_extends_it() {
        let runtime_home = temp_runtime_home("task-lease-heartbeat");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");
        let task_id = outcome.task.task_id.clone();
        let actor = sample_actor(agent_id.clone());
        let watermark = sample_watermark();

        let running = runtime
            .resume_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect("resume");
        assert_eq!(running.task.status, TaskStatus::Running);

        let leases_path = runtime_home.join("state/task-runtime/master/leases.json");
        let leases: Vec<TaskLease> = read_json(&leases_path).expect("leases");
        assert_eq!(leases.len(), 1);
        assert_eq!(leases[0].task_id, task_id);
        assert_eq!(leases[0].agent_id, agent_id);

        let heartbeat = runtime
            .heartbeat_task(TaskHeartbeatRequest {
                task_id: leases[0].task_id.clone(),
                ttl_seconds: 900,
                actor,
                watermark,
            })
            .expect("heartbeat");
        assert_eq!(heartbeat.task.status, TaskStatus::Running);
        assert_eq!(heartbeat.event.event_type, "TaskHeartbeat");
        let updated_leases: Vec<TaskLease> = read_json(&leases_path).expect("updated leases");
        assert!(updated_leases[0].expires_at >= leases[0].expires_at);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn boot_interrupts_running_task_with_expired_lease() {
        let runtime_home = temp_runtime_home("task-expired-lease-recovery");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");
        let task_id = outcome.task.task_id.clone();
        let actor = sample_actor(agent_id.clone());
        runtime
            .resume_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: actor.clone(),
                watermark: sample_watermark(),
            })
            .expect("resume");
        let expired = vec![TaskLease {
            schema_version: 1,
            task_id: task_id.clone(),
            agent_id: agent_id.clone(),
            lease_id: "expired-lease".to_owned(),
            status: "active".to_owned(),
            acquired_at: 1,
            heartbeat_at: 1,
            expires_at: 1,
        }];
        write_json_atomic(
            &runtime_home.join("state/task-runtime/master/leases.json"),
            &expired,
        )
        .expect("force expired lease");

        let recovered = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("recover");
        let task = recovered.query_task(&task_id).expect("query");
        let agent = recovered.query_agent(&agent_id).expect("agent");

        assert_eq!(task.status, TaskStatus::Interrupted);
        assert_eq!(task.last_event_seq, 5);
        assert_eq!(agent.status, AgentStatus::Available);
        let leases: Vec<TaskLease> =
            read_json(&runtime_home.join("state/task-runtime/master/leases.json")).expect("leases");
        assert!(leases.is_empty());
        assert_eq!(actor.source, "control.center");
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn boot_preserves_fresh_running_task_during_lease_acquisition_grace() {
        let runtime_home = temp_runtime_home("task-fresh-missing-lease-grace");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");
        let task_id = outcome.task.task_id.clone();
        runtime
            .resume_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("resume");
        write_json_atomic(
            &runtime_home.join("state/task-runtime/master/leases.json"),
            &Vec::<TaskLease>::new(),
        )
        .expect("simulate lease acquisition window");
        let before = runtime.query_task(&task_id).expect("query before boot");

        let concurrent = TaskRuntime::boot(&runtime_home, agent_id).expect("concurrent boot");
        let after = concurrent.query_task(&task_id).expect("query after boot");

        assert_eq!(after.status, TaskStatus::Running);
        assert_eq!(after.last_event_seq, before.last_event_seq);
        assert_eq!(after.last_event_id, before.last_event_id);
        assert!(
            !concurrent
                .task_history(&task_id)
                .expect("history")
                .iter()
                .any(|event| event.event_type == "TaskInterrupted")
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn boot_interrupts_running_task_with_missing_lease_after_acquisition_grace() {
        let runtime_home = temp_runtime_home("task-stale-missing-lease");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");
        let task_id = outcome.task.task_id.clone();
        runtime
            .resume_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("resume");
        write_json_atomic(
            &runtime_home.join("state/task-runtime/master/leases.json"),
            &Vec::<TaskLease>::new(),
        )
        .expect("remove lease");
        let snapshot_path = runtime_home
            .join("state/tasks/master")
            .join(format!("{}.json", task_id.as_str()));
        let mut stale: TaskSnapshot = read_json(&snapshot_path).expect("read task snapshot");
        stale.updated_at =
            now_unix_seconds().saturating_sub(RUNNING_LEASE_ACQUISITION_GRACE_SECONDS + 1);
        write_json_atomic(&snapshot_path, &stale).expect("age task snapshot");

        let recovered = TaskRuntime::boot(&runtime_home, agent_id).expect("recover");
        let task = recovered.query_task(&task_id).expect("query");

        assert_eq!(task.status, TaskStatus::Interrupted);
        assert_eq!(task.last_event_seq, stale.last_event_seq + 1);
        assert_eq!(
            recovered
                .task_history(&task_id)
                .expect("history")
                .last()
                .map(|event| event.event_type.as_str()),
            Some("TaskInterrupted")
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn heartbeat_for_assigned_task_is_rejected_without_lease_write() {
        let runtime_home = temp_runtime_home("task-heartbeat-rejected");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");

        let err = runtime
            .heartbeat_task(TaskHeartbeatRequest {
                task_id: outcome.task.task_id.clone(),
                ttl_seconds: 300,
                actor: sample_actor(agent_id),
                watermark: sample_watermark(),
            })
            .expect_err("assigned heartbeat must fail");

        assert!(matches!(err, TaskError::InvalidTransition { .. }));
        assert!(
            !runtime_home
                .join("state/task-runtime/master/leases.json")
                .is_file()
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn create_agent_persists_recovers_and_closes_when_idle() {
        let runtime_home = temp_runtime_home("task-agent-create-close");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-a");
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");

        let created = runtime
            .create_agent(AgentCreateRequest {
                agent_id: worker_id.clone(),
                capabilities: vec!["code_edit".to_owned(), "test_run".to_owned()],
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("create agent");
        assert_eq!(created.agent.status, AgentStatus::Available);

        let recovered = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("recover");
        let worker = recovered.query_agent(&worker_id).expect("worker");
        assert_eq!(worker.capabilities, vec!["code_edit", "test_run"]);

        let closed = recovered
            .close_agent(AgentMutationRequest {
                agent_id: worker_id.clone(),
                actor: sample_actor(owner_id),
                watermark: sample_watermark(),
            })
            .expect("close idle worker");
        assert_eq!(closed.agent.status, AgentStatus::Closed);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn waiting_task_assigns_to_available_agent_and_recovers() {
        let runtime_home = temp_runtime_home("task-assign-waiting");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-b");
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        runtime
            .create_agent(AgentCreateRequest {
                agent_id: worker_id.clone(),
                capabilities: vec!["code_edit".to_owned()],
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("create agent");
        let mut request = sample_create_request(owner_id.clone());
        request.task_id = Some(TaskId::new("task-waiting-assign"));
        request.dispatch = TaskDispatchRequest::None;
        let created = runtime.create_task(request).expect("create waiting");
        assert_eq!(created.task.status, TaskStatus::WaitingAgent);

        let assigned = runtime
            .assign_task(TaskAssignRequest {
                task_id: created.task.task_id.clone(),
                agent_id: worker_id.clone(),
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("assign");
        assert_eq!(assigned.task.status, TaskStatus::Assigned);
        assert_eq!(
            assigned.task.assignee.as_ref().map(|a| a.agent_id.clone()),
            Some(worker_id.clone())
        );
        let worker = runtime.query_agent(&worker_id).expect("worker");
        assert_eq!(worker.status, AgentStatus::Busy);
        assert_eq!(worker.queued_tasks, 1);

        let recovered = TaskRuntime::boot(&runtime_home, owner_id).expect("recover");
        let task = recovered.query_task(&created.task.task_id).expect("task");
        assert_eq!(task.status, TaskStatus::Assigned);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn cancel_assigned_task_releases_agent_and_rejects_resume() {
        let runtime_home = temp_runtime_home("task-cancel-release");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");
        let task_id = outcome.task.task_id.clone();

        let cancelled = runtime
            .cancel_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("cancel assigned");
        assert_eq!(cancelled.task.status, TaskStatus::Cancelled);
        let agent = runtime.query_agent(&agent_id).expect("agent");
        assert_eq!(agent.status, AgentStatus::Available);
        assert_eq!(agent.queued_tasks, 0);

        let err = runtime
            .resume_task(TaskMutationRequest {
                task_id,
                actor: sample_actor(agent_id),
                watermark: sample_watermark(),
            })
            .expect_err("cancelled task cannot resume");
        assert!(matches!(err, TaskError::InvalidTransition { .. }));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn stale_runtime_heartbeat_after_cancel_is_rejected_without_terminal_overwrite() {
        let runtime_home = temp_runtime_home("stale-heartbeat-after-cancel");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-stale-heartbeat");
        let execution_id = "exec-stale-heartbeat";
        let stale_runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot stale");
        create_worker_agent(&stale_runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &stale_runtime,
            &owner_id,
            &worker_id,
            "task-stale-heartbeat",
            execution_id,
            80,
        );

        let cancelling_runtime =
            TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot canceller");
        cancelling_runtime
            .cancel_task(TaskMutationRequest {
                task_id: task.task_id.clone(),
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("cancel from second runtime");
        let cancelled_history_len = cancelling_runtime
            .task_history(&task.task_id)
            .expect("cancelled history")
            .len();

        let error = stale_runtime
            .heartbeat_task(TaskHeartbeatRequest {
                task_id: task.task_id.clone(),
                ttl_seconds: 300,
                actor: sample_actor(worker_id.clone()),
                watermark: sample_watermark(),
            })
            .expect_err("stale heartbeat after cancel must fail");

        assert!(matches!(
            error,
            TaskError::InvalidTransition {
                from: TaskStatus::Cancelled,
                event_type: "TaskHeartbeat",
            }
        ));
        let recovered = TaskRuntime::boot(&runtime_home, owner_id).expect("recover");
        let recovered_task = recovered.query_task(&task.task_id).expect("query");
        assert_eq!(recovered_task.status, TaskStatus::Cancelled);
        assert_eq!(recovered_task.last_event_seq, cancelled_history_len as u64);
        let history = recovered.task_history(&task.task_id).expect("history");
        assert_eq!(history.len(), cancelled_history_len);
        assert_eq!(
            history.last().map(|event| event.event_type.as_str()),
            Some("TaskCancelled")
        );
        assert!(
            !runtime_home
                .join("state/task-runtime/master/leases.json")
                .is_file()
                || read_json::<Vec<TaskLease>>(
                    &runtime_home.join("state/task-runtime/master/leases.json")
                )
                .expect("leases")
                .is_empty()
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn stale_runtime_execution_fact_after_cancel_is_rejected_without_terminal_overwrite() {
        let runtime_home = temp_runtime_home("stale-execution-fact-after-cancel");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-stale-execution-fact");
        let execution_id = "exec-stale-execution-fact";
        let stale_runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot stale");
        create_worker_agent(&stale_runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &stale_runtime,
            &owner_id,
            &worker_id,
            "task-stale-execution-fact",
            execution_id,
            80,
        );

        let cancelling_runtime =
            TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot canceller");
        cancelling_runtime
            .cancel_task(TaskMutationRequest {
                task_id: task.task_id.clone(),
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("cancel from second runtime");
        let cancelled_history_len = cancelling_runtime
            .task_history(&task.task_id)
            .expect("cancelled history")
            .len();

        let error = stale_runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.to_owned(),
                task_id: task.task_id.clone(),
                agent_id: worker_id,
                turn_id: Some(TurnId::new("turn-stale-execution-fact")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::ReviewReady {
                    summary: "stale success".to_owned(),
                    deliverables: vec!["stale artifact".to_owned()],
                    evidence: vec!["stale evidence".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect_err("stale execution fact after cancel must fail");

        assert!(matches!(
            error,
            TaskError::InvalidTransition {
                from: TaskStatus::Cancelled,
                event_type: "TaskReviewSubmitted" | "TaskExecutionRecorded",
            }
        ));
        let recovered = TaskRuntime::boot(&runtime_home, owner_id).expect("recover");
        let recovered_task = recovered.query_task(&task.task_id).expect("query");
        assert_eq!(recovered_task.status, TaskStatus::Cancelled);
        assert_eq!(recovered_task.last_event_seq, cancelled_history_len as u64);
        let history = recovered.task_history(&task.task_id).expect("history");
        assert_eq!(history.len(), cancelled_history_len);
        assert!(
            !history
                .iter()
                .any(|event| event.event_type == "TaskReviewSubmitted")
        );
        assert_eq!(
            history.last().map(|event| event.event_type.as_str()),
            Some("TaskCancelled")
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn close_busy_agent_is_rejected() {
        let runtime_home = temp_runtime_home("task-agent-close-busy");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");

        let err = runtime
            .close_agent(AgentMutationRequest {
                agent_id: agent_id.clone(),
                actor: sample_actor(agent_id),
                watermark: sample_watermark(),
            })
            .expect_err("busy agent cannot close");

        assert!(matches!(err, TaskError::InvalidAgentTransition { .. }));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn claim_next_runs_highest_priority_assigned_task_with_lease() {
        let runtime_home = temp_runtime_home("task-claim-priority");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let mut low = sample_create_request(agent_id.clone());
        low.task_id = Some(TaskId::new("task-low"));
        low.priority = 10;
        low.dispatch = TaskDispatchRequest::None;
        let mut high = sample_create_request(agent_id.clone());
        high.task_id = Some(TaskId::new("task-high"));
        high.priority = 90;
        high.dispatch = TaskDispatchRequest::None;
        let low = runtime.create_task(low).expect("low");
        let high = runtime.create_task(high).expect("high");
        runtime
            .assign_task(TaskAssignRequest {
                task_id: low.task.task_id,
                agent_id: agent_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("assign low");
        runtime
            .assign_task(TaskAssignRequest {
                task_id: high.task.task_id,
                agent_id: agent_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("assign high");

        let claimed = runtime
            .claim_next_task(TaskClaimRequest {
                agent_id: agent_id.clone(),
                execution_id: "exec-task-high".to_owned(),
                ttl_seconds: 600,
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("claim");
        let task = claimed.task.expect("claimed task");
        assert_eq!(task.task_id, TaskId::new("task-high"));
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.active_execution_id.as_deref(), Some("exec-task-high"));
        assert_eq!(claimed.execution_id.as_deref(), Some("exec-task-high"));
        let low_task = runtime
            .query_task(&TaskId::new("task-low"))
            .expect("low task");
        assert_eq!(low_task.status, TaskStatus::Assigned);
        let agent = runtime.query_agent(&agent_id).expect("agent");
        assert_eq!(agent.status, AgentStatus::Busy);
        assert_eq!(agent.running_tasks, 1);
        assert_eq!(agent.queued_tasks, 1);
        let leases: Vec<TaskLease> =
            read_json(&runtime_home.join("state/task-runtime/master/leases.json")).expect("leases");
        assert_eq!(leases.len(), 1);
        assert_eq!(leases[0].task_id, TaskId::new("task-high"));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn claim_next_empty_queue_returns_none_without_mutation() {
        let runtime_home = temp_runtime_home("task-claim-empty");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");

        let outcome = runtime
            .claim_next_task(TaskClaimRequest {
                agent_id: agent_id.clone(),
                execution_id: "exec-empty".to_owned(),
                ttl_seconds: 300,
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("claim empty");

        assert!(outcome.task.is_none());
        assert!(outcome.event.is_none());
        let agent = runtime.query_agent(&agent_id).expect("agent");
        assert_eq!(agent.status, AgentStatus::Available);
        assert!(
            !runtime_home
                .join("state/task-runtime/master/leases.json")
                .is_file()
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn record_execution_writes_progress_for_running_task() {
        let runtime_home = temp_runtime_home("task-record-execution");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");
        let task_id = outcome.task.task_id.clone();
        runtime
            .resume_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("resume");

        let recorded = runtime
            .record_execution(TaskExecutionRecordRequest {
                task_id: task_id.clone(),
                phase: "debug".to_owned(),
                summary: "read function map".to_owned(),
                evidence: vec!["docs/function-maps/task.orchestration.md".to_owned()],
                actor: sample_actor(agent_id),
                watermark: sample_watermark(),
            })
            .expect("record execution");

        assert_eq!(recorded.task.status, TaskStatus::Running);
        assert_eq!(recorded.event.event_type, "TaskExecutionRecorded");
        assert_eq!(
            recorded.event.payload.get("phase").and_then(Value::as_str),
            Some("debug")
        );
        assert!(recorded.task.last_progress_at.is_some());
        let recovered = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("recover");
        let task = recovered.query_task(&task_id).expect("query");
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.last_progress_at.is_some());
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn record_execution_rejects_non_running_task_without_sequence_advance() {
        let runtime_home = temp_runtime_home("task-record-execution-reject");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");
        let before = runtime.query_task(&outcome.task.task_id).expect("before");

        let err = runtime
            .record_execution(TaskExecutionRecordRequest {
                task_id: outcome.task.task_id.clone(),
                phase: "debug".to_owned(),
                summary: "should fail".to_owned(),
                evidence: vec!["not running".to_owned()],
                actor: sample_actor(agent_id),
                watermark: sample_watermark(),
            })
            .expect_err("assigned task cannot record execution");

        assert!(matches!(err, TaskError::InvalidTransition { .. }));
        let after = runtime.query_task(&outcome.task.task_id).expect("after");
        assert_eq!(after.last_event_seq, before.last_event_seq);
        assert_eq!(after.status, TaskStatus::Assigned);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_history_returns_ordered_ledger_events() {
        let runtime_home = temp_runtime_home("task-history");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");
        let task_id = outcome.task.task_id.clone();
        runtime
            .resume_task(TaskMutationRequest {
                task_id: task_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("resume");
        runtime
            .record_execution(TaskExecutionRecordRequest {
                task_id: task_id.clone(),
                phase: "debug".to_owned(),
                summary: "inspect ledger".to_owned(),
                evidence: vec!["ledger event".to_owned()],
                actor: sample_actor(agent_id),
                watermark: sample_watermark(),
            })
            .expect("record");

        let history = runtime.task_history(&task_id).expect("history");

        assert_eq!(
            history.iter().map(|event| event.seq).collect::<Vec<_>>(),
            (1..=history.len() as u64).collect::<Vec<_>>()
        );
        assert!(
            history
                .iter()
                .any(|event| event.event_type == "TaskCreated")
        );
        assert!(
            history
                .iter()
                .any(|event| event.event_type == "TaskExecutionRecorded")
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_history_unknown_task_is_explicit_not_found() {
        let runtime_home = temp_runtime_home("task-history-missing");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id).expect("boot");

        let err = runtime
            .task_history(&TaskId::new("missing-task"))
            .expect_err("missing task");

        assert_eq!(err, TaskError::TaskNotFound("missing-task".to_owned()));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn list_tasks_filters_by_status_and_assignee_in_priority_order() {
        let runtime_home = temp_runtime_home("task-list-filter");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let mut low = sample_create_request(agent_id.clone());
        low.task_id = Some(TaskId::new("task-list-low"));
        low.priority = 10;
        low.dispatch = TaskDispatchRequest::None;
        let mut high = sample_create_request(agent_id.clone());
        high.task_id = Some(TaskId::new("task-list-high"));
        high.priority = 90;
        high.dispatch = TaskDispatchRequest::None;
        let low = runtime.create_task(low).expect("low").task;
        let high = runtime.create_task(high).expect("high").task;
        for task in [&low, &high] {
            runtime
                .assign_task(TaskAssignRequest {
                    task_id: task.task_id.clone(),
                    agent_id: agent_id.clone(),
                    actor: sample_actor(agent_id.clone()),
                    watermark: sample_watermark(),
                })
                .expect("assign");
        }

        let tasks = runtime
            .list_tasks(TaskListQuery {
                status: Some(TaskStatus::Assigned),
                assignee: Some(agent_id),
            })
            .expect("list");

        assert_eq!(
            tasks
                .iter()
                .map(|task| task.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["task-list-high", "task-list-low"]
        );
        assert!(tasks.iter().all(|task| task.status == TaskStatus::Assigned));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_board_projects_owner_truth_with_filtered_views() {
        let runtime_home = temp_runtime_home("task-board-projection");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let actor = sample_actor(agent_id.clone());
        let watermark = sample_watermark();
        let running = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create running seed")
            .task;
        runtime
            .resume_task(TaskMutationRequest {
                task_id: running.task_id.clone(),
                actor: actor.clone(),
                watermark: watermark.clone(),
            })
            .expect("resume running");

        let mut waiting_request = sample_create_request(agent_id.clone());
        waiting_request.task_id = Some(TaskId::new("waiting-board-task"));
        waiting_request.priority = 99;
        waiting_request.dispatch = TaskDispatchRequest::None;
        runtime
            .create_task(waiting_request)
            .expect("create waiting task");

        runtime
            .submit_review(TaskReviewSubmission {
                task_id: running.task_id.clone(),
                summary: "ready".to_owned(),
                deliverables: vec!["artifact".to_owned()],
                evidence: vec!["test".to_owned()],
                actor,
                watermark,
            })
            .expect("submit review");

        let board = runtime
            .query_task_board(TaskBoardQuery {
                status: None,
                assignee: None,
                include_terminal: false,
            })
            .expect("task board");

        assert_eq!(board.tasks.len(), 2);
        assert_eq!(board.review_ready.len(), 1);
        assert_eq!(board.review_ready[0].task_id, running.task_id);
        assert!(board.blocked.is_empty());
        assert!(board.agents.iter().any(|agent| agent.agent_id == agent_id));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn execution_fact_recovering_keeps_running_and_writes_event() {
        let runtime_home = temp_runtime_home("execution-fact-recovering");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let task = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create")
            .task;

        let recovering = runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: "exec-1".to_owned(),
                task_id: task.task_id.clone(),
                agent_id: agent_id.clone(),
                turn_id: Some(TurnId::new("turn-1")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Recovering {
                    summary: "repairing tool schema".to_owned(),
                    evidence: vec!["paired failed tool result".to_owned()],
                    retry_count: 1,
                },
                watermark: sample_watermark(),
            })
            .expect("recovering fact");

        assert_eq!(recovering.task.status, TaskStatus::Running);
        assert_eq!(recovering.event.event_type, "TaskExecutionRecovering");
        assert_eq!(
            recovering
                .event
                .payload
                .get("retry_count")
                .and_then(Value::as_u64),
            Some(1)
        );
        let history = runtime.task_history(&task.task_id).expect("history");
        assert!(
            history
                .iter()
                .any(|event| event.event_type == "TaskExecutionRecovering")
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn phase2a_worker_claim_reject_retry_approve_close_recovers_same_execution_id() {
        let runtime_home = temp_runtime_home("phase2a-worker-loop");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-phase2a");
        let execution_id = "exec-phase2a-1".to_owned();
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        runtime
            .create_agent(AgentCreateRequest {
                agent_id: worker_id.clone(),
                capabilities: vec!["code_edit".to_owned(), "test_run".to_owned()],
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("create worker");
        let mut request = sample_create_request(owner_id.clone());
        request.task_id = Some(TaskId::new("task-phase2a-worker"));
        request.dispatch = TaskDispatchRequest::None;
        let task = runtime.create_task(request).expect("create task").task;
        runtime
            .assign_task(TaskAssignRequest {
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("assign worker");

        let claimed = runtime
            .claim_next_task(TaskClaimRequest {
                agent_id: worker_id.clone(),
                execution_id: execution_id.clone(),
                ttl_seconds: 300,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("claim");
        assert_eq!(claimed.execution_id.as_deref(), Some(execution_id.as_str()));
        assert_eq!(
            claimed
                .task
                .as_ref()
                .and_then(|task| task.active_execution_id.as_deref()),
            Some(execution_id.as_str())
        );

        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.clone(),
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(TurnId::new("turn-phase2a")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Running {
                    phase: "progress".to_owned(),
                    summary: "worker made progress".to_owned(),
                    evidence: vec!["progress evidence".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("progress fact");
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.clone(),
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(TurnId::new("turn-phase2a")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Blocked {
                    reason: "needs master input".to_owned(),
                    evidence: vec!["blocker evidence".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("blocked fact");
        assert_eq!(
            runtime.query_task(&task.task_id).expect("blocked").status,
            TaskStatus::Blocked
        );
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.clone(),
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(TurnId::new("turn-phase2a")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Recovering {
                    summary: "master provided unblock guidance".to_owned(),
                    evidence: vec!["recovering evidence".to_owned()],
                    retry_count: 1,
                },
                watermark: sample_watermark(),
            })
            .expect("recovering fact");
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.clone(),
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(TurnId::new("turn-phase2a")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::ReviewReady {
                    summary: "first submission".to_owned(),
                    deliverables: vec!["patch".to_owned()],
                    evidence: vec!["first evidence".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("review ready");
        runtime
            .reject_review(TaskReviewRejection {
                task_id: task.task_id.clone(),
                reject_reason: "needs retry".to_owned(),
                next_requirements: vec!["retry with evidence".to_owned()],
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("reject");
        assert_eq!(
            runtime.query_task(&task.task_id).expect("rejected").status,
            TaskStatus::Rejected
        );
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.clone(),
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(TurnId::new("turn-phase2a-retry")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Running {
                    phase: "retry".to_owned(),
                    summary: "retrying rejected review".to_owned(),
                    evidence: vec!["retry evidence".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("retry progress");
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.clone(),
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(TurnId::new("turn-phase2a-retry")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::ReviewReady {
                    summary: "second submission".to_owned(),
                    deliverables: vec!["accepted patch".to_owned()],
                    evidence: vec!["second evidence".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("second review ready");
        runtime
            .approve_review(TaskMutationRequest {
                task_id: task.task_id.clone(),
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("approve");
        runtime
            .close_task(TaskMutationRequest {
                task_id: task.task_id.clone(),
                actor: sample_actor(owner_id),
                watermark: sample_watermark(),
            })
            .expect("close");

        let recovered = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("recover");
        let recovered_task = recovered.query_task(&task.task_id).expect("task");
        assert_eq!(recovered_task.status, TaskStatus::Closed);
        assert_eq!(
            recovered_task.active_execution_id.as_deref(),
            Some(execution_id.as_str())
        );
        let history = recovered.task_history(&task.task_id).expect("history");
        for required in [
            "TaskAssigned",
            "TaskResumed",
            "TaskExecutionRecorded",
            "TaskBlocked",
            "TaskExecutionRecovering",
            "TaskReviewSubmitted",
            "TaskReviewRejected",
            "TaskReviewApproved",
            "TaskClosed",
        ] {
            assert!(
                history.iter().any(|event| event.event_type == required),
                "missing {required}"
            );
        }
        assert!(history.iter().any(|event| {
            event.payload.get("execution_id").and_then(Value::as_str) == Some(execution_id.as_str())
        }));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn execution_fact_blocked_and_review_ready_update_board_truth() {
        let runtime_home = temp_runtime_home("execution-fact-board");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let review_task = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create review task")
            .task;

        let review_ready = runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: "exec-review".to_owned(),
                task_id: review_task.task_id.clone(),
                agent_id: agent_id.clone(),
                turn_id: None,
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::ReviewReady {
                    summary: "ready for master review".to_owned(),
                    deliverables: vec!["patch".to_owned()],
                    evidence: vec!["cargo test".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("review fact");
        assert_eq!(review_ready.task.status, TaskStatus::ReviewSubmitted);

        let mut blocked_request = sample_create_request(agent_id.clone());
        blocked_request.task_id = Some(TaskId::new("execution-fact-blocked"));
        let blocked_task = runtime
            .create_task(blocked_request)
            .expect("create blocked task")
            .task;
        let blocked = runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: "exec-blocked".to_owned(),
                task_id: blocked_task.task_id.clone(),
                agent_id: agent_id.clone(),
                turn_id: None,
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Blocked {
                    reason: "permission denied".to_owned(),
                    evidence: vec!["os error 1".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("blocked fact");
        assert_eq!(blocked.task.status, TaskStatus::Blocked);

        let board = runtime
            .query_task_board(TaskBoardQuery {
                include_terminal: false,
                ..TaskBoardQuery::default()
            })
            .expect("board");
        assert_eq!(board.blocked.len(), 1);
        assert_eq!(board.blocked[0].task_id, blocked_task.task_id);
        assert_eq!(board.review_ready.len(), 1);
        assert_eq!(board.review_ready[0].task_id, review_task.task_id);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn execution_fact_interrupted_marks_task_retryable_without_blocked_truth() {
        let runtime_home = temp_runtime_home("execution-fact-interrupted");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-interrupted");
        let execution_id = "exec-worker-interrupted";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-interrupted",
            execution_id,
            80,
        );

        let interrupted = runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.to_owned(),
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: None,
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Interrupted {
                    reason: "provider retry exhausted".to_owned(),
                    evidence: vec!["anthropic_http_request_failed".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("interrupted fact");
        assert_eq!(interrupted.task.status, TaskStatus::Interrupted);
        let history = runtime.task_history(&task.task_id).expect("history");
        assert!(
            history
                .iter()
                .any(|event| event.event_type == "TaskInterrupted")
        );
        assert!(
            !history
                .iter()
                .any(|event| event.event_type == "TaskBlocked")
        );

        let reassigned = runtime
            .assign_task(TaskAssignRequest {
                task_id: task.task_id.clone(),
                agent_id: worker_id,
                actor: sample_actor(owner_id),
                watermark: sample_watermark(),
            })
            .expect("reassign interrupted task");
        assert_eq!(reassigned.task.status, TaskStatus::Assigned);
        assert_eq!(reassigned.task.task_id, task.task_id);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn blocked_execution_releases_worker_for_new_assignment() {
        let runtime_home = temp_runtime_home("blocked-worker-release");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-blocked-release");
        let execution_id = "exec-worker-blocked-release";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let blocked_task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-blocked-release",
            execution_id,
            80,
        );

        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.to_owned(),
                task_id: blocked_task.task_id,
                agent_id: worker_id.clone(),
                turn_id: Some(TurnId::new("turn-worker-blocked-release")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Blocked {
                    reason: "provider unavailable".to_owned(),
                    evidence: vec!["provider error".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("block worker task");

        assert_eq!(
            runtime.query_agent(&worker_id).expect("worker").status,
            AgentStatus::Available
        );
        let mut next_request = sample_create_request(owner_id.clone());
        next_request.task_id = Some(TaskId::new("task-worker-after-block"));
        next_request.dispatch = TaskDispatchRequest::None;
        let next_task = runtime
            .create_task(next_request)
            .expect("create next task")
            .task;
        let assigned = runtime
            .assign_task(TaskAssignRequest {
                task_id: next_task.task_id,
                agent_id: worker_id,
                actor: sample_actor(owner_id),
                watermark: sample_watermark(),
            })
            .expect("assign released worker");
        assert_eq!(assigned.task.status, TaskStatus::Assigned);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn explicit_pause_keeps_worker_unavailable_for_unrelated_assignment() {
        let runtime_home = temp_runtime_home("explicit-worker-pause");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-explicit-pause");
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let paused_task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-explicit-pause",
            "exec-worker-explicit-pause",
            80,
        );
        runtime
            .pause_task(TaskMutationRequest {
                task_id: paused_task.task_id,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("pause task");
        assert_eq!(
            runtime.query_agent(&worker_id).expect("worker").status,
            AgentStatus::Paused
        );

        let mut next_request = sample_create_request(owner_id.clone());
        next_request.task_id = Some(TaskId::new("task-worker-while-paused"));
        next_request.dispatch = TaskDispatchRequest::None;
        let next_task = runtime
            .create_task(next_request)
            .expect("create next task")
            .task;
        let error = runtime
            .assign_task(TaskAssignRequest {
                task_id: next_task.task_id,
                agent_id: worker_id.clone(),
                actor: sample_actor(owner_id),
                watermark: sample_watermark(),
            })
            .expect_err("paused worker must reject unrelated assignment");
        assert!(matches!(
            error,
            TaskError::AgentUnavailable(ref agent_id) if agent_id == worker_id.as_str()
        ));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn boot_repairs_legacy_blocked_pause_and_preserves_explicit_pause() {
        let runtime_home = temp_runtime_home("legacy-blocked-worker-pause");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-legacy-blocked-pause");
        let execution_id = "exec-worker-legacy-blocked-pause";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let blocked_task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-legacy-blocked-pause",
            execution_id,
            80,
        );
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.to_owned(),
                task_id: blocked_task.task_id,
                agent_id: worker_id.clone(),
                turn_id: Some(TurnId::new("turn-worker-legacy-blocked-pause")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Blocked {
                    reason: "legacy blocked state".to_owned(),
                    evidence: vec!["legacy evidence".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("block task");
        let mut legacy_snapshot = runtime.query_agent(&worker_id).expect("worker");
        legacy_snapshot.status = AgentStatus::Paused;
        runtime
            .store
            .write_agent_snapshot(&legacy_snapshot)
            .expect("write legacy paused snapshot");
        drop(runtime);

        let recovered =
            TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("recover legacy state");
        assert_eq!(
            recovered.query_agent(&worker_id).expect("worker").status,
            AgentStatus::Available
        );
        drop(recovered);
        let _ = fs::remove_dir_all(runtime_home);

        let runtime_home = temp_runtime_home("preserve-explicit-worker-pause");
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let paused_task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-preserve-explicit-pause",
            "exec-worker-preserve-explicit-pause",
            80,
        );
        runtime
            .pause_task(TaskMutationRequest {
                task_id: paused_task.task_id,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("pause task");
        drop(runtime);

        let recovered = TaskRuntime::boot(&runtime_home, owner_id).expect("recover explicit pause");
        assert_eq!(
            recovered.query_agent(&worker_id).expect("worker").status,
            AgentStatus::Paused
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn execution_fact_validation_failure_writes_no_truth() {
        let runtime_home = temp_runtime_home("execution-fact-validation");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let task = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create")
            .task;
        let before = runtime.query_task(&task.task_id).expect("before");

        let err = runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: "exec-invalid".to_owned(),
                task_id: task.task_id.clone(),
                agent_id,
                turn_id: None,
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Running {
                    phase: "debug".to_owned(),
                    summary: String::new(),
                    evidence: vec!["missing summary".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect_err("invalid fact");

        assert_eq!(err, TaskError::MissingField("summary"));
        let after = runtime.query_task(&task.task_id).expect("after");
        assert_eq!(after.last_event_seq, before.last_event_seq);
        assert_eq!(after.status, before.status);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn scheduler_tick_emits_stale_and_timeout_facts_without_decisions() {
        let runtime_home = temp_runtime_home("scheduler-tick-facts");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let task = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create")
            .task;
        let running = runtime
            .resume_task(TaskMutationRequest {
                task_id: task.task_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("resume");

        let tick = runtime
            .run_scheduler_tick(SchedulerTickRequest {
                now: running.task.created_at.saturating_add(1_000),
                stale_after_seconds: 1,
                soft_timeout_seconds: 10,
                hard_timeout_seconds: 100,
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("tick");

        assert_eq!(tick.facts.len(), 2);
        assert!(
            tick.facts
                .iter()
                .any(|fact| matches!(fact.fact, SchedulerFactKind::Stale { .. }))
        );
        assert!(
            tick.facts
                .iter()
                .any(|fact| matches!(fact.fact, SchedulerFactKind::HardTimeout { .. }))
        );
        assert!(
            tick.facts
                .iter()
                .any(|fact| fact.recommendation == "ask_master_for_decision")
        );
        let after = runtime.query_task(&task.task_id).expect("after tick");
        assert_eq!(after.status, TaskStatus::Running);
        let history = runtime.task_history(&task.task_id).expect("history");
        assert_eq!(
            history
                .iter()
                .filter(|event| event.event_type == "TaskSchedulerTick")
                .count(),
            2
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn scheduler_tick_soft_timeout_does_not_fail_task() {
        let runtime_home = temp_runtime_home("scheduler-soft-timeout");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let task = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create")
            .task;
        runtime
            .resume_task(TaskMutationRequest {
                task_id: task.task_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("resume");

        let tick = runtime
            .run_scheduler_tick(SchedulerTickRequest {
                now: task.created_at.saturating_add(20),
                stale_after_seconds: 1_000,
                soft_timeout_seconds: 10,
                hard_timeout_seconds: 100,
                actor: sample_actor(agent_id),
                watermark: sample_watermark(),
            })
            .expect("tick");

        assert_eq!(tick.facts.len(), 1);
        assert!(matches!(
            tick.facts[0].fact,
            SchedulerFactKind::SoftTimeout { .. }
        ));
        assert_eq!(
            runtime.query_task(&task.task_id).expect("after").status,
            TaskStatus::Running
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn scheduler_tick_recent_progress_is_not_stale() {
        let runtime_home = temp_runtime_home("scheduler-not-stale");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let task = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create")
            .task;
        let progressed = runtime
            .resume_task(TaskMutationRequest {
                task_id: task.task_id.clone(),
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("resume");
        runtime
            .record_execution(TaskExecutionRecordRequest {
                task_id: task.task_id.clone(),
                phase: "work".to_owned(),
                summary: "made progress".to_owned(),
                evidence: vec!["progress event".to_owned()],
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("progress");

        let tick = runtime
            .run_scheduler_tick(SchedulerTickRequest {
                now: progressed.task.updated_at,
                stale_after_seconds: 60,
                soft_timeout_seconds: 1_000,
                hard_timeout_seconds: 2_000,
                actor: sample_actor(agent_id),
                watermark: sample_watermark(),
            })
            .expect("tick");

        assert!(tick.facts.is_empty());
        assert_eq!(
            runtime.query_task(&task.task_id).expect("after").status,
            TaskStatus::Running
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn scheduler_tick_facts_recover_after_boot() {
        let runtime_home = temp_runtime_home("scheduler-recover");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let task = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create")
            .task;
        runtime
            .run_scheduler_tick(SchedulerTickRequest {
                now: task.created_at.saturating_add(500),
                stale_after_seconds: 1,
                soft_timeout_seconds: 10,
                hard_timeout_seconds: 1_000,
                actor: sample_actor(agent_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("tick");

        let recovered = TaskRuntime::boot(&runtime_home, agent_id).expect("recover");
        let board = recovered
            .query_task_board(TaskBoardQuery {
                include_terminal: false,
                ..TaskBoardQuery::default()
            })
            .expect("board");

        assert_eq!(board.stale.len(), 1);
        assert_eq!(board.stale[0].task_id, task.task_id);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn phase2b_event_inbox_projects_events_and_recovers_master_cursor() {
        let runtime_home = temp_runtime_home("phase2b-event-inbox");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-phase2b-inbox");
        let execution_id = "exec-phase2b-inbox";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-phase2b-inbox",
            execution_id,
            50,
        );
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.to_owned(),
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(TurnId::new("turn-phase2b-inbox")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Blocked {
                    reason: "needs master review".to_owned(),
                    evidence: vec!["blocked evidence".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("blocked fact");

        let first_page = runtime
            .query_event_inbox(TaskEventInboxQuery {
                after_cursor: None,
                limit: 3,
            })
            .expect("first inbox page");
        assert_eq!(first_page.source_cursor, None);
        assert_eq!(first_page.events.len(), 3);
        assert_eq!(
            first_page
                .events
                .iter()
                .map(|event| event.kind.as_str())
                .collect::<Vec<_>>(),
            vec!["task_created", "task_waiting_agent", "task_assigned"]
        );
        let page_cursor = first_page.next_cursor.clone().expect("page cursor");

        let second_page = runtime
            .query_event_inbox(TaskEventInboxQuery {
                after_cursor: Some(page_cursor.clone()),
                limit: 100,
            })
            .expect("second inbox page");
        assert_eq!(second_page.source_cursor, Some(page_cursor));
        assert!(second_page.events.iter().any(|event| {
            event.kind == "execution_started"
                && event.execution_id.as_deref() == Some(execution_id)
                && event.agent_id == Some(worker_id.clone())
        }));
        assert!(second_page.events.iter().any(|event| {
            event.kind == "execution_blocked"
                && event.execution_id.as_deref() == Some(execution_id)
                && event.agent_id == Some(worker_id.clone())
        }));

        let first_poll = runtime
            .run_master_poll(MasterPollRequest {
                after_cursor: None,
                limit: 100,
                include_terminal: false,
                replay_from_start: false,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("master poll");
        let persisted_cursor = first_poll
            .persisted_cursor
            .clone()
            .expect("persisted cursor");
        assert_eq!(first_poll.next_cursor, Some(persisted_cursor.clone()));

        let recovered = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("recover");
        let recovered_poll = recovered
            .run_master_poll(MasterPollRequest {
                after_cursor: None,
                limit: 100,
                include_terminal: false,
                replay_from_start: false,
                actor: sample_actor(owner_id),
                watermark: sample_watermark(),
            })
            .expect("recovered poll");
        assert_eq!(recovered_poll.source_cursor, Some(persisted_cursor.clone()));
        assert_eq!(recovered_poll.persisted_cursor, Some(persisted_cursor));
        assert!(recovered_poll.event_inbox.events.is_empty());
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn phase2b_master_poll_classifies_board_and_does_not_mutate_tasks() {
        let runtime_home = temp_runtime_home("phase2b-master-poll");
        let owner_id = AgentId::new("master");
        let blocked_worker = AgentId::new("worker-phase2b-blocked");
        let review_worker = AgentId::new("worker-phase2b-review");
        let stale_worker = AgentId::new("worker-phase2b-stale");
        let idle_worker = AgentId::new("worker-phase2b-idle");
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        for worker_id in [&blocked_worker, &review_worker, &stale_worker, &idle_worker] {
            create_worker_agent(&runtime, &owner_id, worker_id);
        }

        let blocked_task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &blocked_worker,
            "task-phase2b-blocked",
            "exec-phase2b-blocked",
            80,
        );
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: "exec-phase2b-blocked".to_owned(),
                task_id: blocked_task.task_id.clone(),
                agent_id: blocked_worker.clone(),
                turn_id: Some(TurnId::new("turn-phase2b-blocked")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Blocked {
                    reason: "missing permission".to_owned(),
                    evidence: vec!["permission error".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("blocked fact");

        let review_task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &review_worker,
            "task-phase2b-review",
            "exec-phase2b-review",
            70,
        );
        runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: "exec-phase2b-review".to_owned(),
                task_id: review_task.task_id.clone(),
                agent_id: review_worker.clone(),
                turn_id: Some(TurnId::new("turn-phase2b-review")),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::ReviewReady {
                    summary: "ready for review".to_owned(),
                    deliverables: vec!["patch".to_owned()],
                    evidence: vec!["cargo test".to_owned()],
                },
                watermark: sample_watermark(),
            })
            .expect("review fact");

        let stale_task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &stale_worker,
            "task-phase2b-stale",
            "exec-phase2b-stale",
            60,
        );
        let stale_snapshot = runtime.query_task(&stale_task.task_id).expect("stale task");
        runtime
            .run_scheduler_tick(SchedulerTickRequest {
                now: stale_snapshot.created_at.saturating_add(1_000),
                stale_after_seconds: 1,
                soft_timeout_seconds: 10,
                hard_timeout_seconds: 100,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("scheduler tick");

        let before = runtime
            .list_tasks(TaskListQuery {
                status: None,
                assignee: None,
            })
            .expect("before statuses")
            .into_iter()
            .map(|task| (task.task_id, task.status))
            .collect::<BTreeMap<_, _>>();

        let poll = runtime
            .run_master_poll(MasterPollRequest {
                after_cursor: None,
                limit: 100,
                include_terminal: false,
                replay_from_start: false,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("master poll");
        let kinds = poll
            .classifications
            .iter()
            .map(|classification| classification.kind.as_str())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&"blocked"));
        assert!(kinds.contains(&"review_ready"));
        assert!(kinds.contains(&"stale"));
        assert!(kinds.contains(&"hard_timeout"));
        assert!(kinds.contains(&"idle_worker"));
        assert!(poll.classifications.iter().any(|classification| {
            classification.kind == "review_ready"
                && classification
                    .recommended_actions
                    .contains(&"inspect_submission".to_owned())
                && classification
                    .recommended_actions
                    .contains(&"approve_submission".to_owned())
                && classification
                    .recommended_actions
                    .contains(&"reject_submission".to_owned())
        }));
        assert!(poll.persisted_cursor.is_some());

        let after = runtime
            .list_tasks(TaskListQuery {
                status: None,
                assignee: None,
            })
            .expect("after statuses")
            .into_iter()
            .map(|task| (task.task_id, task.status))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(after, before);
        assert_eq!(
            runtime
                .query_task(&blocked_task.task_id)
                .expect("blocked after poll")
                .status,
            TaskStatus::Blocked
        );
        assert_eq!(
            runtime
                .query_task(&review_task.task_id)
                .expect("review after poll")
                .status,
            TaskStatus::ReviewSubmitted
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn phase2b_event_inbox_rejects_unknown_cursor_without_advancing_master_cursor() {
        let runtime_home = temp_runtime_home("phase2b-unknown-cursor");
        let owner_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        let mut request = sample_create_request(owner_id.clone());
        request.task_id = Some(TaskId::new("task-phase2b-cursor"));
        runtime.create_task(request).expect("create task");

        let first_poll = runtime
            .run_master_poll(MasterPollRequest {
                after_cursor: None,
                limit: 1,
                include_terminal: false,
                replay_from_start: false,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("first poll");
        let before_cursor = runtime
            .store
            .load_master_event_cursor(&owner_id)
            .expect("load cursor before");
        assert_eq!(before_cursor, first_poll.persisted_cursor);

        let err = runtime
            .run_master_poll(MasterPollRequest {
                after_cursor: Some("cursor-does-not-exist".to_owned()),
                limit: 100,
                include_terminal: false,
                replay_from_start: false,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect_err("unknown cursor must fail");

        assert_eq!(
            err,
            TaskError::CursorNotFound("cursor-does-not-exist".to_owned())
        );
        let after_cursor = runtime
            .store
            .load_master_event_cursor(&owner_id)
            .expect("load cursor after");
        assert_eq!(after_cursor, before_cursor);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn phase2b_event_cursor_uses_event_id_tiebreak_and_legacy_cursor_skips_duplicates() {
        let runtime_home = temp_runtime_home("phase2b-legacy-cursor-duplicates");
        let owner_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        let mut request = sample_create_request(owner_id.clone());
        request.task_id = Some(TaskId::new("task-phase2b-legacy-cursor"));
        let created = runtime.create_task(request).expect("create task");
        let mut duplicate = runtime
            .task_history(&created.task.task_id)
            .expect("history")
            .last()
            .cloned()
            .expect("last event");
        duplicate.event_id = format!("{}:duplicate", duplicate.event_id);
        runtime
            .store
            .append_event_and_snapshot(&created.task, &duplicate)
            .expect("append duplicate cursor prefix event");

        let full = runtime
            .query_event_inbox(TaskEventInboxQuery {
                after_cursor: None,
                limit: 0,
            })
            .expect("full inbox");
        assert!(
            full.events
                .iter()
                .any(|event| event.cursor.ends_with(&duplicate.event_id)),
            "cursor must include event_id as a uniqueness tiebreak"
        );
        let legacy_cursor = task_event_legacy_cursor_prefix(&duplicate);
        let after_legacy = runtime
            .query_event_inbox(TaskEventInboxQuery {
                after_cursor: Some(legacy_cursor),
                limit: 0,
            })
            .expect("legacy cursor query");
        assert!(
            after_legacy.events.is_empty(),
            "legacy three-part cursor must skip all matching duplicate-prefix events"
        );
        let latest_cursor = full
            .events
            .last()
            .map(|event| event.cursor.clone())
            .expect("latest cursor");
        let after_latest = runtime
            .query_event_inbox(TaskEventInboxQuery {
                after_cursor: Some(latest_cursor),
                limit: 0,
            })
            .expect("latest cursor query");
        assert!(after_latest.events.is_empty());
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn phase2b_master_poll_replay_from_start_advances_stale_master_cursor() {
        let runtime_home = temp_runtime_home("phase2b-replay-from-start");
        let owner_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        let mut request = sample_create_request(owner_id.clone());
        request.task_id = Some(TaskId::new("task-phase2b-replay"));
        runtime.create_task(request).expect("create task");

        let first_poll = runtime
            .run_master_poll(MasterPollRequest {
                after_cursor: None,
                limit: 1,
                include_terminal: false,
                replay_from_start: false,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("first limited poll");
        let stale_cursor = first_poll.persisted_cursor.clone().expect("stale cursor");
        let after_stale_before = runtime
            .query_event_inbox(TaskEventInboxQuery {
                after_cursor: Some(stale_cursor.clone()),
                limit: 0,
            })
            .expect("events after stale cursor");
        assert!(
            !after_stale_before.events.is_empty(),
            "limited poll must leave backlog after stale cursor"
        );

        let replay_poll = runtime
            .run_master_poll(MasterPollRequest {
                after_cursor: None,
                limit: 0,
                include_terminal: false,
                replay_from_start: true,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("replay poll");
        assert_eq!(replay_poll.source_cursor, None);
        assert!(replay_poll.event_inbox.events.len() > 1);
        let latest_cursor = replay_poll
            .persisted_cursor
            .clone()
            .expect("latest persisted cursor");
        assert_ne!(latest_cursor, stale_cursor);
        assert_eq!(
            replay_poll.next_cursor.as_deref(),
            Some(latest_cursor.as_str())
        );

        let after_latest = runtime
            .query_event_inbox(TaskEventInboxQuery {
                after_cursor: Some(latest_cursor.clone()),
                limit: 0,
            })
            .expect("events after latest cursor");
        assert!(
            after_latest.events.is_empty(),
            "replay poll must drain all backlog"
        );

        let err = runtime
            .run_master_poll(MasterPollRequest {
                after_cursor: Some(stale_cursor.clone()),
                limit: 0,
                include_terminal: false,
                replay_from_start: true,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect_err("conflicting replay cursor mode must fail");
        assert_eq!(
            err,
            TaskError::InvalidCursorMode(
                "replay_from_start cannot be combined with after_cursor".to_owned()
            )
        );
        let cursor_after_error = runtime
            .store
            .load_master_event_cursor(&owner_id)
            .expect("load cursor after conflict");
        assert_eq!(cursor_after_error.as_deref(), Some(latest_cursor.as_str()));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn worker_control_query_status_persists_and_recovers() {
        let runtime_home = temp_runtime_home("worker-control-query-status");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-control-query");
        let execution_id = "exec-worker-control-query";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-control-query",
            execution_id,
            50,
        );

        let projection = runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                execution_id,
                WorkerControlOp::QueryStatus,
                Some("control-query-1"),
            ))
            .expect("query status");

        assert_eq!(projection.event.status, "observed");
        assert_eq!(projection.task.status, TaskStatus::Running);
        assert_eq!(projection.agent.agent_id, worker_id);
        assert_eq!(
            projection
                .event
                .payload
                .get("task_status")
                .and_then(Value::as_str),
            Some("running")
        );
        assert!(projection.task_event.is_none());

        let recovered = TaskRuntime::boot(&runtime_home, owner_id).expect("recover");
        let events = recovered
            .query_worker_control_events(&task.task_id, execution_id)
            .expect("control events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].control_id, "control-query-1");
        assert_eq!(events[0].op, WorkerControlOp::QueryStatus);
        assert!(
            runtime_home
                .join("state/task-runtime/master/worker-control/task-worker-control-query/exec-worker-control-query.json")
                .is_file()
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn worker_control_safe_point_events_queue_without_task_mutation() {
        let runtime_home = temp_runtime_home("worker-control-safe-point");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-control-safe");
        let execution_id = "exec-worker-control-safe";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-control-safe",
            execution_id,
            50,
        );
        let before = runtime.query_task(&task.task_id).expect("before");

        let mut ask = sample_worker_control_request(
            &owner_id,
            &worker_id,
            &task.task_id,
            execution_id,
            WorkerControlOp::AskAtSafePoint,
            Some("control-safe-ask"),
        );
        ask.question = Some("What is blocking this task?".to_owned());
        runtime.apply_worker_control(ask).expect("ask");

        let mut constraint = sample_worker_control_request(
            &owner_id,
            &worker_id,
            &task.task_id,
            execution_id,
            WorkerControlOp::AddConstraint,
            Some("control-safe-constraint"),
        );
        constraint.constraint = Some("Do not change public API".to_owned());
        runtime
            .apply_worker_control(constraint)
            .expect("constraint");

        runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                execution_id,
                WorkerControlOp::RequestCheckpoint,
                Some("control-safe-checkpoint"),
            ))
            .expect("checkpoint");
        runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                execution_id,
                WorkerControlOp::RequestSubmissionNow,
                Some("control-safe-submit"),
            ))
            .expect("submission now");

        let after = runtime.query_task(&task.task_id).expect("after");
        assert_eq!(after.status, TaskStatus::Running);
        assert_eq!(after.last_event_seq, before.last_event_seq);
        let events = runtime
            .query_worker_control_events(&task.task_id, execution_id)
            .expect("events");
        assert_eq!(events.len(), 4);
        assert_eq!(
            events
                .iter()
                .map(|event| event.control_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "control-safe-ask",
                "control-safe-constraint",
                "control-safe-checkpoint",
                "control-safe-submit"
            ]
        );
        assert!(events.iter().all(|event| event.status == "queued"));
        assert!(events.iter().any(|event| {
            event.payload.get("question").and_then(Value::as_str)
                == Some("What is blocking this task?")
        }));
        assert!(events.iter().any(|event| {
            event.payload.get("constraint").and_then(Value::as_str)
                == Some("Do not change public API")
        }));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn worker_control_pause_resume_cancel_write_task_consequences() {
        let runtime_home = temp_runtime_home("worker-control-task-consequences");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-control-consequence");
        let execution_id = "exec-worker-control-consequence";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-control-consequence",
            execution_id,
            50,
        );

        let paused = runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                execution_id,
                WorkerControlOp::Pause,
                Some("control-consequence-pause"),
            ))
            .expect("pause");
        assert_eq!(paused.event.status, "applied");
        assert_eq!(paused.task.status, TaskStatus::Paused);
        assert_eq!(
            paused
                .task_event
                .as_ref()
                .map(|event| event.event_type.as_str()),
            Some("TaskPaused")
        );

        let resumed = runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                execution_id,
                WorkerControlOp::Resume,
                Some("control-consequence-resume"),
            ))
            .expect("resume");
        assert_eq!(resumed.task.status, TaskStatus::Running);
        assert_eq!(
            resumed.task.active_execution_id.as_deref(),
            Some(execution_id)
        );
        assert_eq!(
            resumed
                .task_event
                .as_ref()
                .map(|event| event.event_type.as_str()),
            Some("TaskResumed")
        );

        let cancelled = runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                execution_id,
                WorkerControlOp::Cancel,
                Some("control-consequence-cancel"),
            ))
            .expect("cancel");
        assert_eq!(cancelled.task.status, TaskStatus::Cancelled);
        assert_eq!(
            cancelled
                .task_event
                .as_ref()
                .map(|event| event.event_type.as_str()),
            Some("TaskCancelled")
        );

        let events = runtime
            .query_worker_control_events(&task.task_id, execution_id)
            .expect("control events");
        assert_eq!(events.len(), 3);
        assert_eq!(
            events
                .iter()
                .map(|event| event.control_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "control-consequence-pause",
                "control-consequence-resume",
                "control-consequence-cancel"
            ]
        );
        let history = runtime.task_history(&task.task_id).expect("history");
        for required in ["TaskPaused", "TaskResumed", "TaskCancelled"] {
            assert!(
                history.iter().any(|event| event.event_type == required),
                "missing {required}"
            );
        }
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn worker_control_rejects_wrong_execution_without_mutation() {
        let runtime_home = temp_runtime_home("worker-control-wrong-exec");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-control-wrong-exec");
        let execution_id = "exec-worker-control-correct";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-control-wrong-exec",
            execution_id,
            50,
        );
        let before = runtime.query_task(&task.task_id).expect("before");

        let err = runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                "exec-worker-control-wrong",
                WorkerControlOp::QueryStatus,
                Some("control-wrong-exec"),
            ))
            .expect_err("wrong execution should fail");

        assert!(matches!(err, TaskError::InvalidTransition { .. }));
        let after = runtime.query_task(&task.task_id).expect("after");
        assert_eq!(after.status, before.status);
        assert_eq!(after.last_event_seq, before.last_event_seq);
        let events = runtime
            .query_worker_control_events(&task.task_id, execution_id)
            .expect("control events");
        assert!(events.is_empty());
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn worker_control_rejects_terminal_task_without_event() {
        let runtime_home = temp_runtime_home("worker-control-terminal");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-control-terminal");
        let execution_id = "exec-worker-control-terminal";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-control-terminal",
            execution_id,
            50,
        );
        runtime
            .cancel_task(TaskMutationRequest {
                task_id: task.task_id.clone(),
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("cancel task");
        let before = runtime.query_task(&task.task_id).expect("before");

        let err = runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                execution_id,
                WorkerControlOp::QueryStatus,
                Some("control-terminal-query"),
            ))
            .expect_err("terminal control should fail");

        assert!(matches!(
            err,
            TaskError::InvalidTransition {
                from: TaskStatus::Cancelled,
                ..
            }
        ));
        let after = runtime.query_task(&task.task_id).expect("after");
        assert_eq!(after.last_event_seq, before.last_event_seq);
        let events = runtime
            .query_worker_control_events(&task.task_id, execution_id)
            .expect("control events");
        assert!(events.is_empty());
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn worker_control_rejects_missing_question_and_constraint() {
        let runtime_home = temp_runtime_home("worker-control-missing-fields");
        let owner_id = AgentId::new("master");
        let worker_id = AgentId::new("worker-control-missing-fields");
        let execution_id = "exec-worker-control-missing-fields";
        let runtime = TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("boot");
        create_worker_agent(&runtime, &owner_id, &worker_id);
        let task = create_claimed_worker_task(
            &runtime,
            &owner_id,
            &worker_id,
            "task-worker-control-missing-fields",
            execution_id,
            50,
        );

        let ask_err = runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                execution_id,
                WorkerControlOp::AskAtSafePoint,
                Some("control-missing-question"),
            ))
            .expect_err("missing question");
        assert_eq!(ask_err, TaskError::MissingField("question"));

        let constraint_err = runtime
            .apply_worker_control(sample_worker_control_request(
                &owner_id,
                &worker_id,
                &task.task_id,
                execution_id,
                WorkerControlOp::AddConstraint,
                Some("control-missing-constraint"),
            ))
            .expect_err("missing constraint");
        assert_eq!(constraint_err, TaskError::MissingField("constraint"));

        let events = runtime
            .query_worker_control_events(&task.task_id, execution_id)
            .expect("control events");
        assert!(events.is_empty());
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn agent_lifecycle_reducer_projects_model_tool_recovering_and_blocked() {
        let runtime_home = temp_runtime_home("agent-lifecycle-reducer");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let created = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create task")
            .task;

        let thinking = runtime
            .apply_agent_lifecycle_event(AgentLifecycleEvent::ModelThinking {
                agent_id: agent_id.clone(),
                task_id: Some(created.task_id.clone()),
                turn_id: Some(TurnId::new("turn-1")),
                model: "MiniMax-M3".to_owned(),
            })
            .expect("model thinking");
        assert_eq!(thinking.state, AgentLifecycleState::ModelThinking);
        assert_eq!(thinking.stats.model_request_count, 1);
        assert_eq!(thinking.current_task_id, Some(created.task_id.clone()));

        let tool = runtime
            .apply_agent_lifecycle_event(AgentLifecycleEvent::ToolRunning {
                agent_id: agent_id.clone(),
                task_id: Some(created.task_id.clone()),
                turn_id: Some(TurnId::new("turn-1")),
                tool_name: "read_file".to_owned(),
                target: Some("README.md".to_owned()),
            })
            .expect("tool running");
        assert_eq!(tool.state, AgentLifecycleState::ToolRunning);
        assert_eq!(tool.stats.tool_call_count, 1);
        assert_eq!(
            tool.last_activity
                .as_ref()
                .map(|activity| activity.kind.as_str()),
            Some("model_thinking")
        );

        let recovering = runtime
            .apply_agent_lifecycle_event(AgentLifecycleEvent::Recovering {
                agent_id: agent_id.clone(),
                task_id: Some(created.task_id.clone()),
                turn_id: Some(TurnId::new("turn-1")),
                summary: "repairing failed read".to_owned(),
            })
            .expect("recovering");
        assert_eq!(recovering.state, AgentLifecycleState::Recovering);
        assert_eq!(recovering.stats.tool_failure_count, 1);

        let blocked = runtime
            .apply_agent_lifecycle_event(AgentLifecycleEvent::Blocked {
                agent_id: agent_id.clone(),
                task_id: Some(created.task_id),
                turn_id: Some(TurnId::new("turn-1")),
                reason: "missing permission".to_owned(),
            })
            .expect("blocked");
        assert_eq!(blocked.state, AgentLifecycleState::Blocked);
        assert_eq!(blocked.stats.blocked_count, 1);

        let board = runtime.query_agent_board().expect("agent board");
        assert_eq!(board.agents.len(), 1);
        assert_eq!(board.agents[0].state, AgentLifecycleState::Blocked);
        let _ = fs::remove_dir_all(runtime_home);
    }

    fn sample_create_request(agent_id: AgentId) -> TaskCreateRequest {
        TaskCreateRequest {
            task_id: Some(TaskId::new(format!("task-{}", now_unix_seconds()))),
            title: "Implement task persistence".to_owned(),
            content: "Build task persistence skeleton".to_owned(),
            goal: "Task truth survives restart".to_owned(),
            deliverables: vec!["ledger".to_owned(), "snapshot".to_owned()],
            acceptance: vec!["recovery test passes".to_owned()],
            priority: 50,
            target_cwd: Some("/tmp/freehand-task".to_owned()),
            dispatch: TaskDispatchRequest::SelfAgent,
            parent: TaskParentRef {
                session_id: Some(SessionId::new("session-1")),
                turn_id: Some(TurnId::new("turn-1")),
                trace_id: Some(TraceId::new("trace-1")),
            },
            actor: sample_actor(agent_id),
            watermark: sample_watermark(),
        }
    }

    fn sample_actor(agent_id: AgentId) -> TaskActor {
        TaskActor {
            agent_id,
            source: "control.center".to_owned(),
            session_id: Some(SessionId::new("session-1")),
            turn_id: Some(TurnId::new("turn-1")),
            trace_id: Some(TraceId::new("trace-1")),
        }
    }

    fn sample_watermark() -> TaskWatermark {
        TaskWatermark {
            metadata_id: Some("control.center:test".to_owned()),
            hook: Some("ControlHook03AfterModelResponse".to_owned()),
            action_tool_call_id: Some("toolu_task_1".to_owned()),
        }
    }

    fn create_worker_agent(runtime: &TaskRuntime, owner_id: &AgentId, worker_id: &AgentId) {
        runtime
            .create_agent(AgentCreateRequest {
                agent_id: worker_id.clone(),
                capabilities: vec!["code_edit".to_owned(), "test_run".to_owned()],
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("create worker");
    }

    fn create_claimed_worker_task(
        runtime: &TaskRuntime,
        owner_id: &AgentId,
        worker_id: &AgentId,
        task_id: &str,
        execution_id: &str,
        priority: i64,
    ) -> TaskSnapshot {
        let mut request = sample_create_request(owner_id.clone());
        request.task_id = Some(TaskId::new(task_id));
        request.priority = priority;
        request.dispatch = TaskDispatchRequest::None;
        let task = runtime
            .create_task(request)
            .expect("create worker task")
            .task;
        runtime
            .assign_task(TaskAssignRequest {
                task_id: task.task_id.clone(),
                agent_id: worker_id.clone(),
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("assign worker task");
        runtime
            .claim_next_task(TaskClaimRequest {
                agent_id: worker_id.clone(),
                execution_id: execution_id.to_owned(),
                ttl_seconds: 300,
                actor: sample_actor(owner_id.clone()),
                watermark: sample_watermark(),
            })
            .expect("claim worker task")
            .task
            .expect("claimed worker task")
    }

    fn sample_worker_control_request(
        owner_id: &AgentId,
        worker_id: &AgentId,
        task_id: &TaskId,
        execution_id: &str,
        op: WorkerControlOp,
        control_id: Option<&str>,
    ) -> WorkerControlRequest {
        WorkerControlRequest {
            control_id: control_id.map(ToOwned::to_owned),
            task_id: task_id.clone(),
            execution_id: execution_id.to_owned(),
            agent_id: worker_id.clone(),
            op,
            question: None,
            constraint: None,
            note: None,
            actor: sample_actor(owner_id.clone()),
            watermark: sample_watermark(),
        }
    }

    fn temp_runtime_home(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "freehand-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }
}
