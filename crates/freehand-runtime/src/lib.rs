//! Runtime wiring owner for UI command dispatch.

mod checkpoint_store;
mod live_context;
mod master_runner;
mod path_diagnostics;
mod timer_store;
mod turn_projection;
mod worker_runner;

pub use checkpoint_store::{
    RuntimeCheckpointError, RuntimeCheckpointSummary, list_checkpoints, rewind_checkpoint,
};
#[cfg(test)]
pub(crate) use checkpoint_store::{RuntimeCheckpointLedgerEvent, RuntimeCheckpointLedgerRow};
pub(crate) use checkpoint_store::{RuntimeCheckpointStore, checkpoint_summary_to_ui};
pub use master_runner::{
    ProductionMasterRunner, ProductionMasterRunnerError, ProductionMasterTickOutcome,
};
pub(crate) use path_diagnostics::{expand_leading_tilde_path, path_resolution_diagnostic_text};
pub(crate) use timer_store::{
    DueTimerSchedule, TimerScheduleMode, TimerScheduleRequest, TimerStore,
    claim_due_timer_schedule, complete_due_timer_schedule, fail_due_timer_schedule,
};
pub(crate) use timer_store::{TimerRepeatRule, TimerSchedule, parse_cron_expression};
#[cfg(test)]
pub(crate) use timer_store::{local_datetime, next_daily_due, next_weekly_due};
pub(crate) use turn_projection::{
    current_runtime_turn_for_projection, persist_prepared_live_submit_active_turn,
    project_runtime_turn, project_runtime_turn_history, publish_live_cancelled_projection,
    rebuild_session_history_from_effective_turns, restore_all_persisted_sessions_into_ui,
    restore_or_materialize_cancelled_live_submit, restore_or_materialize_failed_live_submit,
    runtime_turn_position, ui_user_text_for_turn,
};
#[cfg(test)]
pub(crate) use turn_projection::{
    effective_turn_context_segments, publish_live_pending_user_projection,
};
pub use worker_runner::{
    ProductionWorkerRunner, ProductionWorkerRunnerError, ProductionWorkerTickOutcome,
};

#[cfg(test)]
use live_context::original_task_segment;
use live_context::{
    LiveContextSegmentBuildEvent, base_live_context_segments,
    base_live_context_segments_with_observer, configured_worker_label,
    runtime_prompt_segment_token_budget, task_space_snapshot_segment,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use chrono::{Datelike, Timelike};
use freehand_blocks::{
    CompletionClaim, CompletionDecision, CompletionSchemaIssue, CompletionSchemaRejection,
    CompletionSubmission, completion_schema_rejection_feedback, parse_completion_submission_block,
    strip_completion_submission_block, validate_completion_submission,
};
#[cfg(test)]
use freehand_config::SelectedPeerAgentConfig;
use freehand_config::{
    AgentMode, AgentModelGroupSelectionConfigUpdate, AgentProviderSelectionConfigUpdate,
    AgentResourceConfigUpdate, LoadedConfig, MAX_AGENT_RESOURCE_COUNT, ModelGroupConfigUpdate,
    ModelRouteConfig, ModelWeightedRouteConfig, ProviderConfigUpdate,
    ProviderProtocol as ConfigProviderProtocol, ProviderType, ProviderWebSearchMode,
    SelectedAgentConfig, SelectedProviderConfig, default_config_path, load_config_from_path,
    load_default_config, provider_base_url_host_for_projection,
    safe_provider_base_url_for_projection, switch_agent_model_group_in_path,
    switch_agent_provider_in_path, update_agent_resource_config_in_path,
    update_provider_config_in_path, upsert_model_group_config_in_path,
    upsert_provider_config_in_path,
};
use freehand_contracts::{
    AgentId, ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
    ContextSegmentKind, ContextStability, ErrorClass, ErrorContract, ErrorErr01RuntimeClassified,
    FeatureId, InputAttachmentKind, InputAttachmentMetadata, ReasonReq03ProviderPayload,
    ReasonReq04ToolCall, ReasonReq05ToolResultReentry, RecoveryPolicy, SessionId, ToolArgument,
    ToolResultContract, ToolResultStatus, TraceId, TurnId,
};
use freehand_control::{
    ControlRhythmDecision, ControlStatusRejection, ControlStatusSubmission, ErrorCenterDecision,
    ErrorCenterObservedFailure, classify_error_center_failure, control_status_rejection_feedback,
    control_status_rhythm_decision, parse_control_status_block, strip_control_status_block,
};
use freehand_debug::{
    DebugEvent, DebugHub, DebugScenePosition, DebugSemanticPosition, DebugStateSnapshot,
    DebugTraceEnvelope,
};
use freehand_metadata::{
    MetadataCenter, MetadataEntry, MetadataEnvelope, MetadataError, MetadataId, MetadataKind,
    MetadataSubject, MetadataWriteNode, MetadataWriteOwner,
};
use freehand_node::{
    LocalNodeRuntime, MasterNodeConfig, NodeRuntimeError, PairingRequest, PairingTransport,
    SlaveNodeConfig,
};
use freehand_provider_anthropic::{
    AnthropicAdapterConfig, AnthropicExecutor, AnthropicExecutorConfig, AnthropicExecutorError,
    AnthropicRawCapture, DEFAULT_ANTHROPIC_MAX_TOKENS,
};
use freehand_provider_core::{
    ProviderCapabilities, ProviderDescriptor, ProviderEventContext, ProviderFamily,
    ProviderHostedToolDefinition, ProviderInputAttachment, ProviderInputAttachmentKind,
    ProviderProtocol, ProviderSemanticOutput, ProviderSemanticRequest, ProviderToolChoice,
    ProviderToolDefinition, ProviderToolExchange, ProviderWebSearchCapability,
    ProviderWebSearchMode as SemanticWebSearchMode, build_semantic_request,
};
use freehand_provider_openai::{
    OpenAiExecutor, OpenAiExecutorConfig, OpenAiExecutorError, OpenAiRawCapture,
};
use freehand_reason::{
    PersistedSessionIndexEntry, PersistedSessionMetadataEntry, ProviderRawLedgerWrite,
    ProviderRawScenePosition, ReasonBroadcastEvent, ReasonPersistence, ReasonPersistenceError,
    ReasonResp04CompletionSchemaRejected, ReasonResp05ModelContinuationWaiting, ReasonTurnEngine,
    SessionHistory, SessionRollbackMarker, TurnRecord, TurnStartInput,
};
use freehand_task::{
    AgentCreateRequest, AgentLifecycleActivity, AgentLifecycleSnapshot, AgentLifecycleState,
    AgentMutationRequest, AgentSnapshot, AgentStatus, ExecutionFact, ExecutionFactKind,
    MasterPollClassification, MasterPollOutcome, MasterPollRequest, SchedulerTickRequest,
    TaskActor, TaskAppendRequest, TaskAssignRequest, TaskBoardProjection, TaskBoardQuery,
    TaskClaimRequest, TaskCreateRequest, TaskDispatchRequest, TaskError, TaskEventInboxEntry,
    TaskEventInboxProjection, TaskEventInboxQuery, TaskExecutionProfile,
    TaskExecutionRecordRequest, TaskHeartbeatRequest, TaskId, TaskLedgerEvent, TaskListQuery,
    TaskMutationRequest, TaskParentRef, TaskReviewRejection, TaskReviewSubmission, TaskRuntime,
    TaskSnapshot, TaskStatus, TaskWatermark, WorkerControlEvent, WorkerControlOp,
    WorkerControlProjection, WorkerControlRequest,
};
use freehand_tools::{
    BuiltinToolExecutionScope, BuiltinToolRegistry, ToolRegistryError, with_workspace_root,
};
use freehand_ui_protocol::{
    TurnProjectionInput, UiAgentBoardProjection, UiAgentLifecycleActivityProjection,
    UiAgentLifecycleProjection, UiAgentModelGroupSelectionUpdate, UiAgentProcessProjection,
    UiAgentProviderSelectionUpdate, UiAgentResourceConfigUpdate, UiAgentSnapshotProjection,
    UiAttachmentMetadataProjection, UiClientKind, UiCommand, UiCommandDispatchEnvelope,
    UiCommandDispatchPort, UiCommandDispatchPortError, UiCommandDispatchReceipt,
    UiCompletionSchemaRetryWaiting, UiConfigPeerProjection, UiConfigStatusProjection,
    UiDiagnosticLogFileProjection, UiDiagnosticsProjection, UiErrorCenterEventListProjection,
    UiErrorCenterEventProjection, UiExecutionFactCommand, UiExecutionFactKind,
    UiInputAttachmentKind, UiMasterPollClassificationProjection, UiMasterPollProjection,
    UiModelGroupConfigProjection, UiModelGroupConfigUpdate, UiModelRequestKind,
    UiModelRequestWaiting, UiModelRouteProjection, UiModelRouteUpdate, UiModelTransportActivity,
    UiModelTransportKind, UiModelWeightedRouteProjection, UiModelWeightedRouteUpdate,
    UiProtocolState, UiProviderConfigSummaryProjection, UiProviderConfigUpdate, UiQueryResult,
    UiRuntimeQueryPort, UiSchedulerTickCommand, UiSessionMetadataProjection,
    UiSessionSearchChildProjection, UiSessionSearchProjection, UiSessionSearchResultProjection,
    UiSubmitMetadata, UiTaskAgentCreateCommand, UiTaskAssignCommand, UiTaskBoardProjection,
    UiTaskClaimCommand, UiTaskCreateCommand, UiTaskDispatchCommand,
    UiTaskEventInboxEntryProjection, UiTaskEventInboxProjection, UiTaskHistoryProjection,
    UiTaskLedgerEventProjection, UiTaskListProjection, UiTaskReviewCommand,
    UiTaskReviewRejectionCommand, UiTaskSnapshotProjection, UiTimerEventProjection,
    UiTimerListProjection, UiTimerProjection, UiTimerRepeatCommand, UiTimerScheduleCommand,
    UiToolRegistryProjection, UiToolRegistryToolProjection, UiTurnProjection,
    UiTurnTimingProjection, UiWorkerControlCommand, UiWorkerControlEventProjection,
    UiWorkerControlProjection, checkpoint_projection_from_runtime_summary,
    turn_projection_for_client, turn_projection_from_events,
};
use serde_json::{Map, Value, json};
use thiserror::Error;

const PROVIDER_EXECUTOR_RETRY_CAP: u32 = 10;
const PROVIDER_EXECUTOR_INITIAL_BACKOFF_MS: u64 = 1_000;
const PROVIDER_EXECUTOR_MAX_BACKOFF_MS: u64 = 20_000;
const DEFAULT_PROVIDER_WEB_SEARCH_TEST_QUERY: &str = "Use web_search to find the current UTC date and one current news headline from openai.com today. Do not answer from memory.";

#[derive(Debug, Clone)]
pub struct LiveReasonTurnRequest {
    pub runtime_home: PathBuf,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub prompt: String,
    pub attachments: Vec<ProviderInputAttachment>,
    pub attachment_metadata: Vec<InputAttachmentMetadata>,
    pub cwd: Option<PathBuf>,
    pub execution_profile: LiveReasonExecutionProfile,
    pub stream: bool,
    pub cancel_token: Option<LiveReasonCancelToken>,
}

pub type LiveReasonCancelToken = Arc<AtomicBool>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveReasonExecutionProfile {
    Workspace,
    CleanSearch,
}

impl LiveReasonExecutionProfile {
    fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::CleanSearch => "clean_search",
        }
    }

    fn requires_worker_workspace(self) -> bool {
        matches!(self, Self::Workspace)
    }
}

impl From<TaskExecutionProfile> for LiveReasonExecutionProfile {
    fn from(value: TaskExecutionProfile) -> Self {
        match value {
            TaskExecutionProfile::Workspace => Self::Workspace,
            TaskExecutionProfile::CleanSearch => Self::CleanSearch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LiveReasonTaskDecisionMode {
    TargetMutation,
    TargetStatuses(Vec<TaskStatus>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveReasonTaskDecisionBoundary {
    pub task_id: TaskId,
    pub initial_event_seq: u64,
    pub mode: LiveReasonTaskDecisionMode,
    pub max_rounds: usize,
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeLiveBridgeError {
    #[error("live bridge provider `{provider}` with protocol `{protocol}` is not supported")]
    UnsupportedLiveProvider { provider: String, protocol: String },
    #[error("turn start failed: {0}")]
    TurnStartFailed(String),
    #[error("rewrite runtime failed: {0}")]
    RewriteRuntimeFailed(String),
    #[error("provider semantic request build failed: {0}")]
    ProviderRequestBuildFailed(String),
    #[error("provider output apply failed: {0}")]
    ProviderOutputApplyFailed(String),
    #[error("provider live executor failed: {0}")]
    ProviderExecutorFailed(String),
    #[error("reason persistence failed: {0}")]
    ReasonPersistenceFailed(String),
    #[error("metadata failed: {0}")]
    MetadataFailed(String),
    #[error("writable tool checkpoint failed: {0}")]
    ToolCheckpointFailed(String),
    #[error("live tool execution failed: {0}")]
    ToolExecutionFailed(String),
    #[error("task projection failed: {0}")]
    TaskProjectionFailed(String),
    #[error("master active-work state failed: {0}")]
    MasterWorkStateFailed(String),
    #[error("instruction capability admission failed: {0}")]
    InstructionCapabilityFailed(String),
    #[error("live bridge role `{expected}` requires matching agent mode, got `{actual}`")]
    AgentModeMismatch { expected: String, actual: String },
    #[error("worker live execution requires a target workspace")]
    WorkerWorkspaceRequired,
    #[error("live turn cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiveReasonExecutionRole {
    Master,
    Worker,
}

impl LiveReasonExecutionRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Master => "master",
            Self::Worker => "slave",
        }
    }

    fn required_agent_mode(self) -> AgentMode {
        match self {
            Self::Master => AgentMode::Master,
            Self::Worker => AgentMode::Slave,
        }
    }

    fn tool_definitions(
        self,
        registry: &BuiltinToolRegistry,
        execution_profile: LiveReasonExecutionProfile,
    ) -> Vec<ProviderToolDefinition> {
        if execution_profile == LiveReasonExecutionProfile::CleanSearch {
            return Vec::new();
        }
        match self {
            Self::Master => registry.master_implemented_definitions(),
            Self::Worker => registry.worker_implemented_definitions(),
        }
    }

    fn hosted_tool_definitions(
        self,
        descriptor: &ProviderDescriptor,
        execution_profile: LiveReasonExecutionProfile,
    ) -> Vec<ProviderHostedToolDefinition> {
        let allow_hosted_search = match (self, execution_profile) {
            (Self::Master, LiveReasonExecutionProfile::Workspace) => descriptor
                .capabilities
                .web_search
                .can_mix_with_function_tools(),
            (Self::Worker, LiveReasonExecutionProfile::CleanSearch) => {
                descriptor.capabilities.web_search.is_hosted()
            }
            _ => false,
        };
        if allow_hosted_search {
            vec![ProviderHostedToolDefinition::WebSearch {
                mode: SemanticWebSearchMode::Live,
                external_web_access: true,
            }]
        } else {
            Vec::new()
        }
    }

    fn tool_schema_fingerprint(
        self,
        registry: &BuiltinToolRegistry,
        execution_profile: LiveReasonExecutionProfile,
    ) -> String {
        if execution_profile == LiveReasonExecutionProfile::CleanSearch {
            return "clean-search:no-function-tools".to_owned();
        }
        match self {
            Self::Master => registry.master_implemented_schema_fingerprint(),
            Self::Worker => registry.worker_implemented_schema_fingerprint(),
        }
    }
}

#[derive(Debug, Clone)]
struct ExecutedToolResult {
    result: ReasonReq05ToolResultReentry,
    task_truth_changed: bool,
}

enum FrameworkLiveTurnFinalization {
    Complete(String),
    Blocked(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderExecutorErrorInfo {
    code: String,
    message: String,
    retryable: bool,
    failover_eligible: bool,
}

impl ProviderExecutorErrorInfo {
    fn terminal_message(&self) -> String {
        format!("{}: {}", self.code, self.message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProviderExecutorRetryPlan {
    cap: u32,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
}

struct ProviderErrorMetadataSpec<'a> {
    center: &'a Arc<Mutex<MetadataCenter>>,
    agent_id: &'a AgentId,
    session_id: &'a SessionId,
    turn: &'a TurnRecord,
    error: &'a RuntimeLiveBridgeError,
    error_code: &'a str,
    retry_index: u32,
    retry_cap: u32,
    pipeline_node: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderRouteKind {
    Primary,
    Fallback,
}

impl ProviderRouteKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Fallback => "fallback",
        }
    }
}

struct ProviderRoute<'a> {
    kind: ProviderRouteKind,
    provider: &'a SelectedProviderConfig,
}

impl ProviderExecutorRetryPlan {
    fn production() -> Self {
        Self {
            cap: PROVIDER_EXECUTOR_RETRY_CAP,
            initial_backoff_ms: PROVIDER_EXECUTOR_INITIAL_BACKOFF_MS,
            max_backoff_ms: PROVIDER_EXECUTOR_MAX_BACKOFF_MS,
        }
    }

    fn backoff_duration(self, retry_index: u32) -> Duration {
        if self.initial_backoff_ms == self.max_backoff_ms {
            return Duration::from_millis(self.initial_backoff_ms);
        }
        let exponent = retry_index.saturating_sub(1).min(31);
        let multiplier = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX);
        let exponential = self.initial_backoff_ms.saturating_mul(multiplier);
        let stagger = u64::from(retry_index)
            .saturating_sub(1)
            .saturating_mul(self.initial_backoff_ms / 2);
        let millis = exponential.saturating_add(stagger).min(self.max_backoff_ms);
        Duration::from_millis(millis)
    }
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn compact_status_fragment(value: &str, max_chars: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut output = String::new();
    let mut truncated = false;
    for (index, ch) in normalized.chars().enumerate() {
        if index >= max_chars {
            truncated = true;
            break;
        }
        output.push(if ch == '|' { '/' } else { ch });
    }
    if truncated {
        output.push_str("...");
    }
    if output.is_empty() {
        "empty".to_owned()
    } else {
        output
    }
}

pub(crate) fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}

fn fnv1a_hex(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn run_live_reason_turn(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
) -> Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError> {
    run_live_reason_turn_with_policy(
        selected,
        request,
        LiveReasonExecutionRole::Master,
        None,
        |_| {},
        |_| {},
        |_| {},
    )
}

pub(crate) fn run_master_lifecycle_reason_turn(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
    decision_boundary: LiveReasonTaskDecisionBoundary,
) -> Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError> {
    run_live_reason_turn_with_policy(
        selected,
        request,
        LiveReasonExecutionRole::Master,
        Some(decision_boundary),
        |_| {},
        |_| {},
        |_| {},
    )
}

pub(crate) fn run_master_lifecycle_reason_turn_with_hooks<FB, FD, FT>(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
    decision_boundary: LiveReasonTaskDecisionBoundary,
    on_broadcast: FB,
    on_debug: FD,
    on_task_list_projection: FT,
) -> Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
    FD: FnMut(&DebugEvent),
    FT: FnMut(&UiTaskListProjection),
{
    run_live_reason_turn_with_policy(
        selected,
        request,
        LiveReasonExecutionRole::Master,
        Some(decision_boundary),
        on_broadcast,
        on_debug,
        on_task_list_projection,
    )
}

pub fn run_worker_live_reason_turn(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
) -> Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError> {
    run_live_reason_turn_with_policy(
        selected,
        request,
        LiveReasonExecutionRole::Worker,
        None,
        |_| {},
        |_| {},
        |_| {},
    )
}

pub fn run_live_reason_turn_with_hooks<FB, FD, FT>(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
    on_broadcast: FB,
    on_debug: FD,
    on_task_list_projection: FT,
) -> Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
    FD: FnMut(&DebugEvent),
    FT: FnMut(&UiTaskListProjection),
{
    run_live_reason_turn_with_policy(
        selected,
        request,
        LiveReasonExecutionRole::Master,
        None,
        on_broadcast,
        on_debug,
        on_task_list_projection,
    )
}

fn record_master_live_safe_point(
    role: LiveReasonExecutionRole,
    request: &LiveReasonTurnRequest,
    agent_id: &AgentId,
    safe_point: master_runner::MasterWorkSafePoint,
) -> Result<Option<master_runner::MasterAttentionResolution>, RuntimeLiveBridgeError> {
    if role != LiveReasonExecutionRole::Master {
        return Ok(None);
    }
    let checkpoint = master_runner::record_master_active_work_safe_point_if_current(
        &request.runtime_home,
        agent_id,
        &request.session_id,
        &request.turn_id,
        safe_point,
    )
    .map_err(RuntimeLiveBridgeError::MasterWorkStateFailed)?;
    if checkpoint.is_none() {
        return Ok(None);
    }
    await_master_attention_resolution_if_needed(
        request,
        &request.runtime_home,
        agent_id,
        &request.session_id,
        &request.turn_id,
        &request.trace_id,
    )
}

fn await_master_attention_resolution_if_needed(
    request: &LiveReasonTurnRequest,
    runtime_home: &Path,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn_id: &TurnId,
    trace_id: &TraceId,
) -> Result<Option<master_runner::MasterAttentionResolution>, RuntimeLiveBridgeError> {
    loop {
        ensure_live_not_cancelled(request)?;
        if let Some(resolution) = master_runner::take_master_attention_resolution_if_current(
            runtime_home,
            agent_id,
            session_id,
            turn_id,
            trace_id,
        )
        .map_err(RuntimeLiveBridgeError::MasterWorkStateFailed)?
        {
            return Ok(Some(resolution));
        }
        let Some(checkpoint) = master_runner::inspect_master_active_work_if_current(
            runtime_home,
            agent_id,
            session_id,
            turn_id,
            trace_id,
        )
        .map_err(RuntimeLiveBridgeError::MasterWorkStateFailed)?
        else {
            return Err(RuntimeLiveBridgeError::MasterWorkStateFailed(
                "active Master work disappeared while waiting for attention resolution".to_owned(),
            ));
        };
        match checkpoint.state {
            master_runner::MasterActiveWorkState::SuspendedByAttention
            | master_runner::MasterActiveWorkState::Restoring => {
                thread::sleep(Duration::from_millis(25));
            }
            master_runner::MasterActiveWorkState::Running
            | master_runner::MasterActiveWorkState::SuspendRequested => return Ok(None),
        }
    }
}

fn enter_master_terminal_persistence(
    role: LiveReasonExecutionRole,
    request: &LiveReasonTurnRequest,
    agent_id: &AgentId,
) -> Result<Option<master_runner::MasterAttentionResolution>, RuntimeLiveBridgeError> {
    if let Some(resolution) = record_master_live_safe_point(
        role,
        request,
        agent_id,
        master_runner::MasterWorkSafePoint::BeforeTerminalPersistence,
    )? {
        return Ok(Some(resolution));
    }
    record_master_live_safe_point(
        role,
        request,
        agent_id,
        master_runner::MasterWorkSafePoint::TerminalPersistenceInFlight,
    )
}

fn run_live_reason_turn_with_policy<FB, FD, FT>(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
    role: LiveReasonExecutionRole,
    task_decision_boundary: Option<LiveReasonTaskDecisionBoundary>,
    on_broadcast: FB,
    on_debug: FD,
    on_task_list_projection: FT,
) -> Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
    FD: FnMut(&DebugEvent),
    FT: FnMut(&UiTaskListProjection),
{
    if selected.mode != role.required_agent_mode() {
        return Err(RuntimeLiveBridgeError::AgentModeMismatch {
            expected: role.as_str().to_owned(),
            actual: selected.mode.as_str().to_owned(),
        });
    }
    if role == LiveReasonExecutionRole::Worker
        && request.execution_profile.requires_worker_workspace()
        && request.cwd.is_none()
    {
        return Err(RuntimeLiveBridgeError::WorkerWorkspaceRequired);
    }
    match (selected.provider.provider_type, selected.provider.protocol) {
        (ProviderType::Anthropic, ConfigProviderProtocol::Messages)
        | (ProviderType::OpenAi, ConfigProviderProtocol::Responses)
        | (ProviderType::OpenAi, ConfigProviderProtocol::ChatCompletions) => {
            run_live_provider_reason_turn(
                selected,
                request,
                role,
                task_decision_boundary,
                on_broadcast,
                on_debug,
                on_task_list_projection,
            )
        }
        _ => Err(RuntimeLiveBridgeError::UnsupportedLiveProvider {
            provider: selected.provider.provider_type.as_str().to_owned(),
            protocol: selected.provider.protocol.as_str().to_owned(),
        }),
    }
}

fn run_live_provider_reason_turn<FB, FD, FT>(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
    role: LiveReasonExecutionRole,
    task_decision_boundary: Option<LiveReasonTaskDecisionBoundary>,
    mut on_broadcast: FB,
    mut on_debug: FD,
    mut on_task_list_projection: FT,
) -> Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
    FD: FnMut(&DebugEvent),
    FT: FnMut(&UiTaskListProjection),
{
    let mut active_route = ProviderRoute {
        kind: ProviderRouteKind::Primary,
        provider: &selected.provider,
    };
    let mut provider_label = live_provider_label(active_route.provider);
    let mut active_provider_descriptor = provider_descriptor(active_route.provider)?;
    let web_search_route_guidance =
        provider_web_search_route_guidance(selected, &active_provider_descriptor);
    let mut executor = build_live_provider_driver(active_route.provider)?;
    let mut fallback_activated = false;
    let agent_id = AgentId::new(selected.name.clone());
    let persistence = ReasonPersistence::new(request.runtime_home.clone(), agent_id.clone());
    let (mut history, restore_status, restored_closed_turns, restored_active_turn) =
        match persistence.restore(&request.session_id) {
            Ok(restored) => {
                let count = restored.closed_turns.len();
                (
                    restored.history,
                    LiveReasonRestoreStatus::RestoredExisting,
                    count,
                    restored.active_turn,
                )
            }
            Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => (
                SessionHistory::new(request.session_id.clone(), Vec::new())
                    .map_err(|err| RuntimeLiveBridgeError::RewriteRuntimeFailed(err.to_string()))?,
                LiveReasonRestoreStatus::CreatedNew,
                0,
                None,
            ),
            Err(err) => {
                return Err(RuntimeLiveBridgeError::ReasonPersistenceFailed(
                    err.to_string(),
                ));
            }
        };
    if restore_status == LiveReasonRestoreStatus::RestoredExisting {
        let mut effective_turns = persistence
            .restore_turn_snapshots_for_ui(&request.session_id)
            .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))?;
        if restored_active_turn
            .as_ref()
            .is_some_and(|snapshot| snapshot.turn.request.turn_id == request.turn_id)
        {
            effective_turns.retain(|turn| turn.request.turn_id != request.turn_id);
        }
        effective_turns.sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
        rebuild_session_history_from_effective_turns(
            &mut history,
            &request.session_id,
            &effective_turns,
        )?;
    }
    let debug_hub = Arc::new(DebugHub::new(true));
    let debug_receiver = debug_hub.subscribe(64);
    let first_round_turn_id = derived_turn_id(&request.turn_id, 1);
    let first_round_trace_id = derived_trace_id(&request.trace_id, 1);
    let metadata_center = Arc::new(Mutex::new(
        MetadataCenter::with_ledger_path(metadata_ledger_path(
            &request.runtime_home,
            &agent_id,
            &request.session_id,
        ))
        .map_err(|err: MetadataError| RuntimeLiveBridgeError::MetadataFailed(err.to_string()))?,
    ));
    write_live_bridge_metadata(
        &metadata_center,
        &agent_id,
        &request.session_id,
        RuntimeMetadataWriteSpec {
            turn_id: None,
            trace_id: &request.trace_id,
            kind: MetadataKind::RuntimeState,
            pipeline_node: "RuntimeLive01RestoreResolved",
            metadata_suffix: "restore_resolved".to_owned(),
            symbol_path: "run_live_provider_reason_turn",
            entries: vec![
                MetadataEntry {
                    key: "runtime.restore_status".to_owned(),
                    value: json!(match restore_status {
                        LiveReasonRestoreStatus::CreatedNew => "created_new",
                        LiveReasonRestoreStatus::RestoredExisting => "restored_existing",
                    }),
                },
                MetadataEntry {
                    key: "runtime.restored_closed_turns".to_owned(),
                    value: json!(restored_closed_turns),
                },
                MetadataEntry {
                    key: "runtime.stream".to_owned(),
                    value: json!(request.stream),
                },
                MetadataEntry {
                    key: "runtime.execution_profile".to_owned(),
                    value: json!(request.execution_profile.as_str()),
                },
                MetadataEntry {
                    key: "provider.family".to_owned(),
                    value: json!(provider_label.family),
                },
                MetadataEntry {
                    key: "provider.protocol".to_owned(),
                    value: json!(provider_label.protocol),
                },
                MetadataEntry {
                    key: "provider.route".to_owned(),
                    value: json!(active_route.kind.as_str()),
                },
                MetadataEntry {
                    key: "provider.id".to_owned(),
                    value: json!(active_route.provider.id.as_str()),
                },
            ],
        },
    )?;
    emit_live_bridge_debug(
        &debug_hub,
        &agent_id,
        &request.session_id,
        RuntimeDebugEmitSpec {
            turn_id: &first_round_turn_id,
            trace_id: &first_round_trace_id,
            pipeline_node: "RuntimeLive01RestoreResolved",
            function: "run_live_provider_reason_turn",
            status_text: "runtime restore resolved",
            detail_lines: vec![
                format!(
                    "restore_status={}",
                    match restore_status {
                        LiveReasonRestoreStatus::CreatedNew => "created_new",
                        LiveReasonRestoreStatus::RestoredExisting => "restored_existing",
                    }
                ),
                format!("restored_closed_turns={restored_closed_turns}"),
                format!("stream={}", request.stream),
                format!("provider={}", provider_label.display),
                format!("provider_route={}", active_route.kind.as_str()),
                format!("provider_id={}", active_route.provider.id),
            ],
        },
    );
    let engine = ReasonTurnEngine::with_debug_hub_and_metadata_center(
        Arc::clone(&debug_hub),
        Arc::clone(&metadata_center),
    );
    let receiver = engine.subscribe(64);

    let mut broadcasts = Vec::new();
    let mut schema_rejections = Vec::new();
    let mut consecutive_schema_rejections = 0usize;
    let mut turns = Vec::new();
    let mut round = 0usize;
    let mut tool_executions = 0usize;
    let mut next_prompt = request.prompt.clone();
    let configured_workers = match role {
        LiveReasonExecutionRole::Master => selected.worker_peer_names(),
        LiveReasonExecutionRole::Worker => Vec::new(),
    };
    let configured_worker_set = match role {
        LiveReasonExecutionRole::Master => Some(configured_workers.as_slice()),
        LiveReasonExecutionRole::Worker => None,
    };
    write_live_bridge_metadata(
        &metadata_center,
        &agent_id,
        &request.session_id,
        RuntimeMetadataWriteSpec {
            turn_id: Some(&first_round_turn_id),
            trace_id: &first_round_trace_id,
            kind: MetadataKind::RuntimeState,
            pipeline_node: "RuntimeLive01ContextPlanningStarted",
            metadata_suffix: "context_planning_started".to_owned(),
            symbol_path: "run_live_provider_reason_turn",
            entries: vec![
                MetadataEntry {
                    key: "runtime.role".to_owned(),
                    value: json!(role.as_str()),
                },
                MetadataEntry {
                    key: "runtime.stream".to_owned(),
                    value: json!(request.stream),
                },
                MetadataEntry {
                    key: "runtime.execution_profile".to_owned(),
                    value: json!(request.execution_profile.as_str()),
                },
                MetadataEntry {
                    key: "context.cwd_bound".to_owned(),
                    value: json!(request.cwd.is_some()),
                },
                MetadataEntry {
                    key: "context.configured_worker_count".to_owned(),
                    value: json!(
                        configured_worker_set
                            .map(|workers| workers.len())
                            .unwrap_or(0)
                    ),
                },
            ],
        },
    )?;
    emit_live_bridge_debug(
        &debug_hub,
        &agent_id,
        &request.session_id,
        RuntimeDebugEmitSpec {
            turn_id: &first_round_turn_id,
            trace_id: &first_round_trace_id,
            pipeline_node: "RuntimeLive01ContextPlanningStarted",
            function: "run_live_provider_reason_turn",
            status_text: "preparing request context",
            detail_lines: vec![
                format!("role={}", role.as_str()),
                format!("stream={}", request.stream),
                format!("cwd_bound={}", request.cwd.is_some()),
                format!(
                    "configured_worker_count={}",
                    configured_worker_set
                        .map(|workers| workers.len())
                        .unwrap_or(0)
                ),
            ],
        },
    );
    drain_debug_events(&debug_receiver, &mut on_debug);
    let context_record_scope = LiveContextSegmentRecordScope {
        metadata_center: &metadata_center,
        debug_hub: &debug_hub,
        debug_receiver: &debug_receiver,
        agent_id: &agent_id,
        session_id: &request.session_id,
        turn_id: &first_round_turn_id,
        trace_id: &first_round_trace_id,
    };
    let mut carryover_segments = base_live_context_segments_with_observer(
        &request.prompt,
        role,
        request.execution_profile,
        configured_worker_set,
        Some(web_search_route_guidance.as_str()),
        &request.runtime_home,
        request.cwd.as_deref(),
        &agent_id,
        |event| {
            record_live_context_segment_build_event(&context_record_scope, &mut on_debug, event)
        },
    )?;
    let context_segment_count = carryover_segments.len();
    let context_estimated_tokens = carryover_segments
        .iter()
        .map(|segment| u64::from(segment.token_budget))
        .sum::<u64>();
    write_live_bridge_metadata(
        &metadata_center,
        &agent_id,
        &request.session_id,
        RuntimeMetadataWriteSpec {
            turn_id: Some(&first_round_turn_id),
            trace_id: &first_round_trace_id,
            kind: MetadataKind::RuntimeState,
            pipeline_node: "RuntimeLive01ContextPlanningCompleted",
            metadata_suffix: "context_planning_completed".to_owned(),
            symbol_path: "run_live_provider_reason_turn",
            entries: vec![
                MetadataEntry {
                    key: "context.segment_count".to_owned(),
                    value: json!(context_segment_count),
                },
                MetadataEntry {
                    key: "context.estimated_token_budget".to_owned(),
                    value: json!(context_estimated_tokens),
                },
            ],
        },
    )?;
    emit_live_bridge_debug(
        &debug_hub,
        &agent_id,
        &request.session_id,
        RuntimeDebugEmitSpec {
            turn_id: &first_round_turn_id,
            trace_id: &first_round_trace_id,
            pipeline_node: "RuntimeLive01ContextPlanningCompleted",
            function: "run_live_provider_reason_turn",
            status_text: "request context ready",
            detail_lines: vec![
                format!("context_segment_count={context_segment_count}"),
                format!("context_estimated_token_budget={context_estimated_tokens}"),
            ],
        },
    );
    drain_debug_events(&debug_receiver, &mut on_debug);
    let mut tool_exchanges: Vec<ProviderToolExchange> = Vec::new();
    let mut executed_tool_call_ids = Vec::<String>::new();
    let tool_registry = BuiltinToolRegistry::reasonix_aligned();
    let tool_schema_fingerprint =
        role.tool_schema_fingerprint(&tool_registry, request.execution_profile);

    'reason_loop: loop {
        ensure_live_not_cancelled(&request)?;
        if let Some(resolution) = record_master_live_safe_point(
            role,
            &request,
            &agent_id,
            master_runner::MasterWorkSafePoint::BeforeProviderRequest,
        )? {
            admit_master_attention_resolution_for_next_round(
                &mut carryover_segments,
                &resolution,
                LiveRoundContext {
                    role,
                    execution_profile: request.execution_profile,
                    configured_worker_set,
                    web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                    runtime_home: &request.runtime_home,
                    cwd: request.cwd.as_deref(),
                    agent_id: &agent_id,
                },
            )?;
            next_prompt = master_attention_continuation_prompt();
        }
        if let Some(resolution) = record_master_live_safe_point(
            role,
            &request,
            &agent_id,
            master_runner::MasterWorkSafePoint::ProviderInFlight,
        )? {
            admit_master_attention_resolution_for_next_round(
                &mut carryover_segments,
                &resolution,
                LiveRoundContext {
                    role,
                    execution_profile: request.execution_profile,
                    configured_worker_set,
                    web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                    runtime_home: &request.runtime_home,
                    cwd: request.cwd.as_deref(),
                    agent_id: &agent_id,
                },
            )?;
            next_prompt = master_attention_continuation_prompt();
        }
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
                    tool_schema_fingerprint: Some(tool_schema_fingerprint.clone()),
                    model: active_route.provider.default_model.clone(),
                },
            )
            .map_err(|err| RuntimeLiveBridgeError::TurnStartFailed(err.to_string()))?;
        turn.cwd = request
            .cwd
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
        if round == 1 {
            turn.attachments = request.attachment_metadata.clone();
        }
        persistence
            .record_turn_started(&history, &turn, schema_rejections.len() as u32)
            .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))?;
        drain_debug_events(&debug_receiver, &mut on_debug);

        let mut semantic_request = build_semantic_request(
            active_provider_descriptor.clone(),
            turn.provider_payload.clone(),
            debug_hub.is_enabled(),
        )
        .map_err(|err| RuntimeLiveBridgeError::ProviderRequestBuildFailed(err.to_string()))?;
        semantic_request.input_attachments = if round == 1 {
            request.attachments.clone()
        } else {
            Vec::new()
        };
        semantic_request.tools = role.tool_definitions(&tool_registry, request.execution_profile);
        semantic_request.hosted_tools =
            role.hosted_tool_definitions(&active_provider_descriptor, request.execution_profile);
        semantic_request.tool_choice = None;
        semantic_request.tool_exchanges = tool_exchanges.clone();
        write_control_hook_metadata(
            &metadata_center,
            &agent_id,
            &request.session_id,
            RuntimeControlHookWriteSpec {
                turn_id: Some(&turn.request.turn_id),
                trace_id: &turn.request.trace_id,
                pipeline_node: "ControlHook02BeforeModelRequest",
                metadata_suffix: "before_model_request".to_owned(),
                symbol_path: "run_live_provider_reason_turn",
                entries: vec![
                    MetadataEntry {
                        key: "control.hook".to_owned(),
                        value: json!("ControlHook02BeforeModelRequest"),
                    },
                    MetadataEntry {
                        key: "control.status_schema_version".to_owned(),
                        value: json!(1),
                    },
                    MetadataEntry {
                        key: "control.status_guidance_included".to_owned(),
                        value: json!(true),
                    },
                ],
            },
        )?;
        write_live_bridge_metadata(
            &metadata_center,
            &agent_id,
            &request.session_id,
            RuntimeMetadataWriteSpec {
                turn_id: Some(&turn.request.turn_id),
                trace_id: &turn.request.trace_id,
                kind: MetadataKind::Provider,
                pipeline_node: "RuntimeLive02ProviderRequestBuilt",
                metadata_suffix: "provider_request_built".to_owned(),
                symbol_path: "run_live_provider_reason_turn",
                entries: vec![
                    MetadataEntry {
                        key: "bridge.round_ordinal".to_owned(),
                        value: json!(round),
                    },
                    MetadataEntry {
                        key: "runtime.stream".to_owned(),
                        value: json!(request.stream),
                    },
                    MetadataEntry {
                        key: "provider.family".to_owned(),
                        value: json!(provider_label.family),
                    },
                    MetadataEntry {
                        key: "provider.protocol".to_owned(),
                        value: json!(provider_label.protocol),
                    },
                    MetadataEntry {
                        key: "provider.route".to_owned(),
                        value: json!(active_route.kind.as_str()),
                    },
                    MetadataEntry {
                        key: "provider.id".to_owned(),
                        value: json!(active_route.provider.id.as_str()),
                    },
                    MetadataEntry {
                        key: "reason.model".to_owned(),
                        value: json!(active_route.provider.default_model.as_str()),
                    },
                    MetadataEntry {
                        key: "tool.definition_count".to_owned(),
                        value: json!(semantic_request.tools.len()),
                    },
                    MetadataEntry {
                        key: "tool.exchange_count".to_owned(),
                        value: json!(semantic_request.tool_exchanges.len()),
                    },
                ],
            },
        )?;
        emit_live_bridge_debug(
            &debug_hub,
            &agent_id,
            &request.session_id,
            RuntimeDebugEmitSpec {
                turn_id: &turn.request.turn_id,
                trace_id: &turn.request.trace_id,
                pipeline_node: "RuntimeLive02ProviderRequestBuilt",
                function: "run_live_provider_reason_turn",
                status_text: "provider request built",
                detail_lines: vec![
                    format!("round={round}"),
                    format!("stream={}", request.stream),
                    format!("provider={}", provider_label.display),
                    format!("provider_route={}", active_route.kind.as_str()),
                    format!("provider_id={}", active_route.provider.id),
                    format!("model={}", active_route.provider.default_model),
                    format!("execution_profile={}", request.execution_profile.as_str()),
                    format!("tool_definition_count={}", semantic_request.tools.len()),
                    format!(
                        "hosted_tool_definition_count={}",
                        semantic_request.hosted_tools.len()
                    ),
                    format!(
                        "tool_exchange_count={}",
                        semantic_request.tool_exchanges.len()
                    ),
                ],
            },
        );
        drain_debug_events(&debug_receiver, &mut on_debug);

        if request.stream {
            let stream_persistence_error = RefCell::new(None::<RuntimeLiveBridgeError>);
            let raw_session_id = turn.request.session_id.clone();
            let raw_turn_id = turn.request.turn_id.clone();
            let raw_trace_id = turn.request.trace_id.clone();
            let _ = record_master_live_safe_point(
                role,
                &request,
                &agent_id,
                master_runner::MasterWorkSafePoint::ProviderInFlight,
            )?;
            let stream_result = executor.execute_stream_with_raw(
                &provider_ctx(&turn),
                &semantic_request,
                &mut |raw| {
                    if semantic_request.raw_retention
                        == freehand_provider_core::RawRetentionPolicy::DoNotRetain
                    {
                        return Ok(());
                    }
                    if let Err(err) = record_live_provider_raw(
                        &persistence,
                        &raw_session_id,
                        &raw_turn_id,
                        &raw_trace_id,
                        semantic_request.descriptor.family,
                        raw,
                    ) {
                        *stream_persistence_error.borrow_mut() = Some(err);
                        return Err(
                            "live bridge failed while persisting raw provider stream".to_owned()
                        );
                    }
                    Ok(())
                },
                &mut |batch| {
                    if live_is_cancelled(&request) {
                        *stream_persistence_error.borrow_mut() =
                            Some(RuntimeLiveBridgeError::Cancelled);
                        return Err("live bridge cancelled while reading stream".to_owned());
                    }
                    let mut apply_ctx = LiveApplyContext {
                        engine: &engine,
                        persistence: &persistence,
                        history: &history,
                        receiver: &receiver,
                        debug_receiver: &debug_receiver,
                        broadcasts: &mut broadcasts,
                        on_broadcast: &mut on_broadcast,
                        on_debug: &mut on_debug,
                    };
                    if let Err(err) = apply_provider_outputs_persist_and_capture_broadcasts(
                        &mut apply_ctx,
                        &mut turn,
                        batch,
                        schema_rejections.len() as u32,
                    ) {
                        *stream_persistence_error.borrow_mut() = Some(err);
                        return Err("live bridge failed while persisting stream output".to_owned());
                    }
                    Ok(())
                },
            );
            if let Some(err) = stream_persistence_error.into_inner() {
                return Err(err);
            }
            if let Err(err) = stream_result {
                let info = err.info().clone();
                let mapped =
                    RuntimeLiveBridgeError::ProviderExecutorFailed(info.terminal_message());
                record_provider_error_metadata(ProviderErrorMetadataSpec {
                    center: &metadata_center,
                    agent_id: &agent_id,
                    session_id: &request.session_id,
                    turn: &turn,
                    error: &mapped,
                    error_code: &info.code,
                    retry_index: 1,
                    retry_cap: 1,
                    pipeline_node: "RuntimeLive05ProviderError",
                })?;
                let mut failure_ctx = ProviderExecutorFailureContext {
                    engine: &engine,
                    persistence: &persistence,
                    history: &history,
                    receiver: &receiver,
                    broadcasts: &mut broadcasts,
                    on_broadcast: &mut on_broadcast,
                    debug_receiver: &debug_receiver,
                    on_debug: &mut on_debug,
                    schema_rejection_count: schema_rejections.len() as u32,
                    error_code: info.code,
                };
                materialize_provider_executor_failure(&mut failure_ctx, &mut turn, &mapped)?;
                turns.push(turn);
                return Err(mapped);
            }
        } else {
            let retry_plan = provider_executor_retry_plan();
            let mut retry_index = 0_u32;
            let outputs = loop {
                retry_index = retry_index.saturating_add(1);
                let single_raw_error = RefCell::new(None::<RuntimeLiveBridgeError>);
                let _ = record_master_live_safe_point(
                    role,
                    &request,
                    &agent_id,
                    master_runner::MasterWorkSafePoint::ProviderInFlight,
                )?;
                let execute_result = executor.execute_once_with_raw(
                    &provider_ctx(&turn),
                    &semantic_request,
                    &mut |raw| {
                        if semantic_request.raw_retention
                            == freehand_provider_core::RawRetentionPolicy::DoNotRetain
                        {
                            return Ok(());
                        }
                        if let Err(err) = record_live_provider_raw(
                            &persistence,
                            &turn.request.session_id,
                            &turn.request.turn_id,
                            &turn.request.trace_id,
                            semantic_request.descriptor.family,
                            raw,
                        ) {
                            *single_raw_error.borrow_mut() = Some(err);
                            return Err(
                                "live bridge failed while persisting raw provider response"
                                    .to_owned(),
                            );
                        }
                        Ok(())
                    },
                );
                if let Some(err) = single_raw_error.into_inner() {
                    return Err(err);
                }
                match execute_result {
                    Ok(outputs) => break outputs,
                    Err(err) => {
                        let info = err.info().clone();
                        let mapped =
                            RuntimeLiveBridgeError::ProviderExecutorFailed(info.terminal_message());
                        let primary_exhausted = !info.retryable || retry_index >= retry_plan.cap;
                        let should_failover = active_route.kind == ProviderRouteKind::Primary
                            && info.failover_eligible
                            && primary_exhausted
                            && !fallback_activated
                            && selected.fallback_provider.is_some();
                        let error_pipeline_node = if should_failover {
                            "RuntimeLive05ProviderFailover"
                        } else {
                            "RuntimeLive05ProviderError"
                        };
                        record_provider_error_metadata(ProviderErrorMetadataSpec {
                            center: &metadata_center,
                            agent_id: &agent_id,
                            session_id: &request.session_id,
                            turn: &turn,
                            error: &mapped,
                            error_code: &info.code,
                            retry_index,
                            retry_cap: retry_plan.cap,
                            pipeline_node: error_pipeline_node,
                        })?;
                        if should_failover
                            && let Some(fallback_provider) = selected.fallback_provider.as_ref()
                        {
                            let from_provider_id = active_route.provider.id.clone();
                            active_route = ProviderRoute {
                                kind: ProviderRouteKind::Fallback,
                                provider: fallback_provider,
                            };
                            fallback_activated = true;
                            provider_label = live_provider_label(active_route.provider);
                            active_provider_descriptor =
                                provider_descriptor(active_route.provider)?;
                            executor = build_live_provider_driver(active_route.provider)?;
                            turn.provider_payload.model =
                                active_route.provider.default_model.clone();
                            let prior_tools = semantic_request.tools.clone();
                            let prior_hosted_tools = semantic_request.hosted_tools.clone();
                            let prior_tool_choice = semantic_request.tool_choice.clone();
                            let prior_tool_exchanges = semantic_request.tool_exchanges.clone();
                            let prior_input_attachments =
                                semantic_request.input_attachments.clone();
                            semantic_request = build_semantic_request(
                                active_provider_descriptor.clone(),
                                turn.provider_payload.clone(),
                                debug_hub.is_enabled(),
                            )
                            .map_err(|err| {
                                RuntimeLiveBridgeError::ProviderRequestBuildFailed(err.to_string())
                            })?;
                            semantic_request.tools = prior_tools;
                            semantic_request.hosted_tools = prior_hosted_tools;
                            semantic_request.tool_choice = prior_tool_choice;
                            semantic_request.tool_exchanges = prior_tool_exchanges;
                            semantic_request.input_attachments = prior_input_attachments;
                            write_live_bridge_metadata(
                                &metadata_center,
                                &agent_id,
                                &request.session_id,
                                RuntimeMetadataWriteSpec {
                                    turn_id: Some(&turn.request.turn_id),
                                    trace_id: &turn.request.trace_id,
                                    kind: MetadataKind::Routing,
                                    pipeline_node: "RuntimeLive05ProviderFailover",
                                    metadata_suffix: format!("provider_failover:{retry_index}"),
                                    symbol_path: "run_live_provider_reason_turn",
                                    entries: vec![
                                        MetadataEntry {
                                            key: "provider.route".to_owned(),
                                            value: json!(active_route.kind.as_str()),
                                        },
                                        MetadataEntry {
                                            key: "provider.failover_from".to_owned(),
                                            value: json!(from_provider_id),
                                        },
                                        MetadataEntry {
                                            key: "provider.failover_to".to_owned(),
                                            value: json!(active_route.provider.id.as_str()),
                                        },
                                        MetadataEntry {
                                            key: "provider.failover_error_code".to_owned(),
                                            value: json!(info.code.as_str()),
                                        },
                                        MetadataEntry {
                                            key: "provider.family".to_owned(),
                                            value: json!(provider_label.family),
                                        },
                                        MetadataEntry {
                                            key: "provider.protocol".to_owned(),
                                            value: json!(provider_label.protocol),
                                        },
                                        MetadataEntry {
                                            key: "reason.model".to_owned(),
                                            value: json!(
                                                active_route.provider.default_model.as_str()
                                            ),
                                        },
                                    ],
                                },
                            )?;
                            emit_live_bridge_debug(
                                &debug_hub,
                                &agent_id,
                                &request.session_id,
                                RuntimeDebugEmitSpec {
                                    turn_id: &turn.request.turn_id,
                                    trace_id: &turn.request.trace_id,
                                    pipeline_node: "RuntimeLive05ProviderFailover",
                                    function: "run_live_provider_reason_turn",
                                    status_text: "provider route switched to fallback",
                                    detail_lines: vec![
                                        format!("from_provider={from_provider_id}"),
                                        format!("to_provider={}", active_route.provider.id),
                                        format!("error_code={}", info.code),
                                        format!("model={}", active_route.provider.default_model),
                                    ],
                                },
                            );
                            drain_debug_events(&debug_receiver, &mut on_debug);
                            retry_index = 0;
                            continue;
                        }
                        if primary_exhausted {
                            let mut failure_ctx = ProviderExecutorFailureContext {
                                engine: &engine,
                                persistence: &persistence,
                                history: &history,
                                receiver: &receiver,
                                broadcasts: &mut broadcasts,
                                on_broadcast: &mut on_broadcast,
                                debug_receiver: &debug_receiver,
                                on_debug: &mut on_debug,
                                schema_rejection_count: schema_rejections.len() as u32,
                                error_code: info.code.clone(),
                            };
                            materialize_provider_executor_failure(
                                &mut failure_ctx,
                                &mut turn,
                                &mapped,
                            )?;
                            turns.push(turn);
                            return Err(mapped);
                        }
                        let retry_backoff = retry_plan.backoff_duration(retry_index);
                        emit_provider_retry_debug(
                            &debug_hub,
                            &agent_id,
                            &request.session_id,
                            &turn,
                            &info,
                            retry_index,
                            retry_plan.cap,
                            retry_backoff,
                        );
                        drain_debug_events(&debug_receiver, &mut on_debug);
                        ensure_live_not_cancelled(&request)?;
                        sleep_provider_retry(&request, retry_backoff)?;
                    }
                }
            };
            ensure_live_not_cancelled(&request)?;
            let mut apply_ctx = LiveApplyContext {
                engine: &engine,
                persistence: &persistence,
                history: &history,
                receiver: &receiver,
                debug_receiver: &debug_receiver,
                broadcasts: &mut broadcasts,
                on_broadcast: &mut on_broadcast,
                on_debug: &mut on_debug,
            };
            apply_provider_outputs_persist_and_capture_broadcasts(
                &mut apply_ctx,
                &mut turn,
                &outputs,
                schema_rejections.len() as u32,
            )?;
        }
        ensure_live_not_cancelled(&request)?;
        drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);

        let mut attention_resolution_after_provider = record_master_live_safe_point(
            role,
            &request,
            &agent_id,
            master_runner::MasterWorkSafePoint::BeforeToolExecution,
        )?;
        let pending_tool_calls = pending_tool_calls_for_execution(&turn, &executed_tool_call_ids);
        if !pending_tool_calls.is_empty()
            && let Some(resolution) = attention_resolution_after_provider.take()
        {
            let mut apply_ctx = LiveApplyContext {
                engine: &engine,
                persistence: &persistence,
                history: &history,
                receiver: &receiver,
                debug_receiver: &debug_receiver,
                broadcasts: &mut broadcasts,
                on_broadcast: &mut on_broadcast,
                on_debug: &mut on_debug,
            };
            prepare_master_attention_tool_invalidation(
                &mut apply_ctx,
                &mut turn,
                &pending_tool_calls,
                &resolution,
                schema_rejections.len() as u32,
                &mut tool_exchanges,
                &mut executed_tool_call_ids,
                &mut tool_executions,
                &mut next_prompt,
                &mut carryover_segments,
                &request.prompt,
                LiveRoundContext {
                    role,
                    execution_profile: request.execution_profile,
                    configured_worker_set,
                    web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                    runtime_home: &request.runtime_home,
                    cwd: request.cwd.as_deref(),
                    agent_id: &agent_id,
                },
            )?;
            turns.push(turn);
            continue 'reason_loop;
        }
        if !pending_tool_calls.is_empty() {
            consecutive_schema_rejections = 0;
            let mut reached_task_decision = None;
            for (tool_index, tool_call) in pending_tool_calls.iter().enumerate() {
                ensure_live_not_cancelled(&request)?;
                if let Some(resolution) = record_master_live_safe_point(
                    role,
                    &request,
                    &agent_id,
                    master_runner::MasterWorkSafePoint::BeforeToolExecution,
                )? {
                    let remaining_tool_calls = pending_tool_calls[tool_index..].to_vec();
                    let mut apply_ctx = LiveApplyContext {
                        engine: &engine,
                        persistence: &persistence,
                        history: &history,
                        receiver: &receiver,
                        debug_receiver: &debug_receiver,
                        broadcasts: &mut broadcasts,
                        on_broadcast: &mut on_broadcast,
                        on_debug: &mut on_debug,
                    };
                    prepare_master_attention_tool_invalidation(
                        &mut apply_ctx,
                        &mut turn,
                        &remaining_tool_calls,
                        &resolution,
                        schema_rejections.len() as u32,
                        &mut tool_exchanges,
                        &mut executed_tool_call_ids,
                        &mut tool_executions,
                        &mut next_prompt,
                        &mut carryover_segments,
                        &request.prompt,
                        LiveRoundContext {
                            role,
                            execution_profile: request.execution_profile,
                            configured_worker_set,
                            web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                            runtime_home: &request.runtime_home,
                            cwd: request.cwd.as_deref(),
                            agent_id: &agent_id,
                        },
                    )?;
                    turns.push(turn);
                    continue 'reason_loop;
                }
                if let Some(resolution) = record_master_live_safe_point(
                    role,
                    &request,
                    &agent_id,
                    master_runner::MasterWorkSafePoint::ToolEffectInFlight,
                )? {
                    let remaining_tool_calls = pending_tool_calls[tool_index..].to_vec();
                    let mut apply_ctx = LiveApplyContext {
                        engine: &engine,
                        persistence: &persistence,
                        history: &history,
                        receiver: &receiver,
                        debug_receiver: &debug_receiver,
                        broadcasts: &mut broadcasts,
                        on_broadcast: &mut on_broadcast,
                        on_debug: &mut on_debug,
                    };
                    prepare_master_attention_tool_invalidation(
                        &mut apply_ctx,
                        &mut turn,
                        &remaining_tool_calls,
                        &resolution,
                        schema_rejections.len() as u32,
                        &mut tool_exchanges,
                        &mut executed_tool_call_ids,
                        &mut tool_executions,
                        &mut next_prompt,
                        &mut carryover_segments,
                        &request.prompt,
                        LiveRoundContext {
                            role,
                            execution_profile: request.execution_profile,
                            configured_worker_set,
                            web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                            runtime_home: &request.runtime_home,
                            cwd: request.cwd.as_deref(),
                            agent_id: &agent_id,
                        },
                    )?;
                    turns.push(turn);
                    continue 'reason_loop;
                }
                let executed_tool_result = execute_registry_tool_call(
                    &tool_registry,
                    &request.runtime_home,
                    request.cwd.as_deref(),
                    role,
                    configured_worker_set,
                    &turn,
                    tool_call,
                )?;
                let tool_result = executed_tool_result.result.clone();
                if tool_result.tool_result.status == ToolResultStatus::Failed {
                    write_error_center_metadata(
                        &metadata_center,
                        &agent_id,
                        &request.session_id,
                        RuntimeErrorCenterWriteSpec {
                            turn_id: Some(&turn.request.turn_id),
                            trace_id: &turn.request.trace_id,
                            pipeline_node: "RuntimeLive03ToolExecuted",
                            metadata_suffix: format!(
                                "tool_result_failed:{}",
                                tool_call.tool_call.tool_call_id.as_str()
                            ),
                            symbol_path: "run_live_provider_reason_turn",
                            observed: ErrorCenterObservedFailure {
                                source_owner: "tool.registry".to_owned(),
                                source_pipeline_node: "RuntimeLive03ToolExecuted".to_owned(),
                                code: "tool_result_failed".to_owned(),
                                message: "tool_result_failed: model-visible recovery text is stored only in reason tool-result truth".to_owned(),
                                retry_index: 0,
                                retry_cap: 0,
                            },
                        },
                    )?;
                }
                write_control_hook_metadata(
                    &metadata_center,
                    &agent_id,
                    &request.session_id,
                    RuntimeControlHookWriteSpec {
                        turn_id: Some(&turn.request.turn_id),
                        trace_id: &turn.request.trace_id,
                        pipeline_node: "ControlHook01AfterLocalToolResult",
                        metadata_suffix: format!(
                            "after_local_tool_result:{}",
                            tool_call.tool_call.tool_call_id.as_str()
                        ),
                        symbol_path: "run_live_provider_reason_turn",
                        entries: vec![
                            MetadataEntry {
                                key: "control.hook".to_owned(),
                                value: json!("ControlHook01AfterLocalToolResult"),
                            },
                            MetadataEntry {
                                key: "tool.name".to_owned(),
                                value: json!(tool_call.tool_call.tool_name.as_str()),
                            },
                            MetadataEntry {
                                key: "tool.call_id".to_owned(),
                                value: json!(tool_call.tool_call.tool_call_id.as_str()),
                            },
                            MetadataEntry {
                                key: "tool.result_status".to_owned(),
                                value: json!(tool_result.tool_result.status),
                            },
                        ],
                    },
                )?;
                write_live_bridge_metadata(
                    &metadata_center,
                    &agent_id,
                    &request.session_id,
                    RuntimeMetadataWriteSpec {
                        turn_id: Some(&turn.request.turn_id),
                        trace_id: &turn.request.trace_id,
                        kind: MetadataKind::Routing,
                        pipeline_node: "RuntimeLive03ToolExecuted",
                        metadata_suffix: format!(
                            "tool_executed:{}",
                            tool_call.tool_call.tool_call_id.as_str()
                        ),
                        symbol_path: "run_live_provider_reason_turn",
                        entries: vec![
                            MetadataEntry {
                                key: "bridge.round_ordinal".to_owned(),
                                value: json!(round),
                            },
                            MetadataEntry {
                                key: "tool.name".to_owned(),
                                value: json!(tool_call.tool_call.tool_name.as_str()),
                            },
                            MetadataEntry {
                                key: "tool.call_id".to_owned(),
                                value: json!(tool_call.tool_call.tool_call_id.as_str()),
                            },
                            MetadataEntry {
                                key: "tool.result_status".to_owned(),
                                value: json!(tool_result.tool_result.status),
                            },
                        ],
                    },
                )?;
                emit_live_bridge_debug(
                    &debug_hub,
                    &agent_id,
                    &request.session_id,
                    RuntimeDebugEmitSpec {
                        turn_id: &turn.request.turn_id,
                        trace_id: &turn.request.trace_id,
                        pipeline_node: "RuntimeLive03ToolExecuted",
                        function: "run_live_provider_reason_turn",
                        status_text: "registry tool executed",
                        detail_lines: vec![
                            format!("round={round}"),
                            format!("tool_name={}", tool_call.tool_call.tool_name.as_str()),
                            format!("tool_call_id={}", tool_call.tool_call.tool_call_id.as_str()),
                            format!("tool_result_status={:?}", tool_result.tool_result.status),
                        ],
                    },
                );
                ensure_live_not_cancelled(&request)?;
                let output = ProviderSemanticOutput::ToolResultReentry(tool_result.clone());
                engine
                    .apply_provider_output(&mut turn, output.clone())
                    .map_err(|err| {
                        RuntimeLiveBridgeError::ProviderOutputApplyFailed(err.to_string())
                    })?;
                persistence
                    .record_provider_output_applied(
                        &history,
                        &turn,
                        &output,
                        schema_rejections.len() as u32,
                    )
                    .map_err(|err| {
                        RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string())
                    })?;
                drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
                drain_debug_events(&debug_receiver, &mut on_debug);
                if executed_tool_result.task_truth_changed
                    && tool_result.tool_result.status == ToolResultStatus::Success
                {
                    let projection = task_list_projection_from_runtime(
                        &request.runtime_home,
                        &agent_id,
                        None,
                        None,
                    )
                    .map_err(|err| RuntimeLiveBridgeError::TaskProjectionFailed(err.to_string()))?;
                    on_task_list_projection(&projection);
                    if let Some(boundary) = task_decision_boundary.as_ref()
                        && reached_task_decision.is_none()
                    {
                        reached_task_decision = task_decision_boundary_summary(
                            &request.runtime_home,
                            &agent_id,
                            boundary,
                        )?;
                    }
                }
                executed_tool_call_ids.push(tool_call.tool_call.tool_call_id.as_str().to_owned());
                tool_exchanges.push(ProviderToolExchange {
                    tool_call: tool_call.clone(),
                    tool_result,
                });
                tool_executions = tool_executions.saturating_add(1);
            }
            if let Some(summary) = reached_task_decision {
                finalize_framework_live_turn(
                    &engine,
                    &persistence,
                    &history,
                    &receiver,
                    &debug_receiver,
                    &mut broadcasts,
                    &mut on_broadcast,
                    &mut on_debug,
                    &mut turn,
                    FrameworkLiveTurnFinalization::Complete(summary),
                    schema_rejections.len() as u32,
                )?;
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
            if let Some(reason) =
                task_decision_round_budget_reason(task_decision_boundary.as_ref(), round)
            {
                finalize_framework_live_turn(
                    &engine,
                    &persistence,
                    &history,
                    &receiver,
                    &debug_receiver,
                    &mut broadcasts,
                    &mut on_broadcast,
                    &mut on_debug,
                    &mut turn,
                    FrameworkLiveTurnFinalization::Blocked(reason),
                    schema_rejections.len() as u32,
                )?;
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
            let failed_tool_results = tool_exchanges
                .iter()
                .filter(|exchange| {
                    exchange.tool_result.tool_result.status == ToolResultStatus::Failed
                })
                .count();
            let detail = if failed_tool_results == 0 {
                format!(
                    "tool result returned: {} ok · waiting model",
                    tool_exchanges.len()
                )
            } else {
                format!(
                    "tool result returned: {} failed / {} total · waiting model",
                    failed_tool_results,
                    tool_exchanges.len()
                )
            };
            let wait_event = ReasonBroadcastEvent::ModelContinuationWaiting(
                ReasonResp05ModelContinuationWaiting {
                    session_id: turn.request.session_id.clone(),
                    turn_id: turn.request.turn_id.clone(),
                    trace_id: turn.request.trace_id.clone(),
                    feature_id: turn.request.feature_id.clone(),
                    agent_id: turn.request.agent_id.clone(),
                    detail,
                },
            );
            on_broadcast(&wait_event);
            broadcasts.push(wait_event);
            next_prompt = "The tool result has been returned. Use it to continue the task, then provide the required Freehand completion schema when done.".to_owned();
            carryover_segments = next_round_segments(
                &request.prompt,
                &collect_turn_text(&turn),
                None,
                LiveRoundContext {
                    role,
                    execution_profile: request.execution_profile,
                    configured_worker_set,
                    web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                    runtime_home: &request.runtime_home,
                    cwd: request.cwd.as_deref(),
                    agent_id: &agent_id,
                },
            )?;
            turns.push(turn);
            continue;
        }

        ensure_live_not_cancelled(&request)?;
        if !turn_has_completion_candidate_finish_reason(&turn) {
            let reason = latest_finish_reason(&turn)
                .unwrap_or("missing_finish_reason")
                .to_owned();
            engine.interrupt_turn(
                &mut turn,
                format!("Provider ended before completion schema was available: {reason}"),
            );
            drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
            drain_debug_events(&debug_receiver, &mut on_debug);
            ensure_live_not_cancelled(&request)?;
            persistence
                .record_turn_closed(&history, &turn, schema_rejections.len() as u32)
                .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))?;
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

        let provider_text = collect_turn_text(&turn);
        let public_provider_text =
            strip_control_status_block(&strip_completion_submission_block(&provider_text));
        if let Some(resolution) = attention_resolution_after_provider.take() {
            prepare_master_attention_reasoning_continuation(
                &resolution,
                &mut next_prompt,
                &mut carryover_segments,
                &request.prompt,
                &public_provider_text,
                LiveRoundContext {
                    role,
                    execution_profile: request.execution_profile,
                    configured_worker_set,
                    web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                    runtime_home: &request.runtime_home,
                    cwd: request.cwd.as_deref(),
                    agent_id: &agent_id,
                },
            )?;
            turns.push(turn);
            continue 'reason_loop;
        }
        let status_decision = match run_control_status_stop_hook(
            &metadata_center,
            &agent_id,
            &request.session_id,
            &turn,
            &provider_text,
        )? {
            ControlStatusHookOutcome::Absent => None,
            ControlStatusHookOutcome::Accepted(decision) => Some(decision),
            ControlStatusHookOutcome::Rejected {
                rejection,
                feedback,
            } => {
                ensure_live_not_cancelled(&request)?;
                let response_rejection = completion_rejection_from_control_status(&rejection);
                schema_rejections.push(response_rejection.clone());
                consecutive_schema_rejections = consecutive_schema_rejections.saturating_add(1);
                write_error_center_metadata(
                    &metadata_center,
                    &agent_id,
                    &request.session_id,
                    RuntimeErrorCenterWriteSpec {
                        turn_id: Some(&turn.request.turn_id),
                        trace_id: &turn.request.trace_id,
                        pipeline_node: "ControlHook03AfterModelResponse",
                        metadata_suffix: format!(
                            "control_status_schema_rejected:{}",
                            consecutive_schema_rejections
                        ),
                        symbol_path: "run_live_provider_reason_turn",
                        observed: ErrorCenterObservedFailure {
                            source_owner: "control.center".to_owned(),
                            source_pipeline_node: "ControlHook03AfterModelResponse".to_owned(),
                            code: "control_status_schema_rejected".to_owned(),
                            message: feedback.clone(),
                            retry_index: consecutive_schema_rejections as u32,
                            retry_cap: 3,
                        },
                    },
                )?;
                persistence
                    .record_completion_rejected(
                        &history,
                        &turn,
                        &response_rejection,
                        consecutive_schema_rejections as u32,
                    )
                    .map_err(|err| {
                        RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string())
                    })?;
                if consecutive_schema_rejections >= 3 {
                    engine.block_turn(
                        &mut turn,
                        format!(
                            "Response schema still invalid after 3 polishing attempts.\n{}",
                            feedback
                        ),
                    );
                    drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
                    drain_debug_events(&debug_receiver, &mut on_debug);
                    ensure_live_not_cancelled(&request)?;
                    persistence
                        .record_turn_closed(&history, &turn, schema_rejections.len() as u32)
                        .map_err(|err| {
                            RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string())
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
                let retry_event = ReasonBroadcastEvent::CompletionSchemaRejected(
                    ReasonResp04CompletionSchemaRejected {
                        session_id: turn.request.session_id.clone(),
                        turn_id: turn.request.turn_id.clone(),
                        trace_id: turn.request.trace_id.clone(),
                        feature_id: turn.request.feature_id.clone(),
                        agent_id: turn.request.agent_id.clone(),
                        retry_index: consecutive_schema_rejections as u32,
                        rejection: response_rejection,
                        feedback: feedback.clone(),
                    },
                );
                on_broadcast(&retry_event);
                broadcasts.push(retry_event);
                next_prompt = feedback.clone();
                carryover_segments = next_round_segments(
                    &request.prompt,
                    &public_provider_text,
                    Some(feedback.as_str()),
                    LiveRoundContext {
                        role,
                        execution_profile: request.execution_profile,
                        configured_worker_set,
                        web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                        runtime_home: &request.runtime_home,
                        cwd: request.cwd.as_deref(),
                        agent_id: &agent_id,
                    },
                )?;
                turns.push(turn);
                continue;
            }
        };
        if let Some(decision) = status_decision {
            match decision {
                ControlRhythmDecision::AllowNaturalStop
                | ControlRhythmDecision::AllowTaskCompletion
                | ControlRhythmDecision::StopForUserOptions(_) => {
                    ensure_live_not_cancelled(&request)?;
                    let terminal_summary = control_status_terminal_summary(
                        &decision,
                        &provider_text,
                        &public_provider_text,
                    );
                    let submission = CompletionSubmission {
                        claim: CompletionClaim::Complete,
                        completion_reason: Some(format!(
                            "control status accepted {}",
                            control_decision_label(&decision)
                        )),
                        evidence: Some(terminal_summary.clone()),
                        summary: Some(terminal_summary),
                        learned: Some("control status stopHook accepted".to_owned()),
                        next_step: None,
                        blocked_reason: None,
                    };
                    if let Some(resolution) =
                        enter_master_terminal_persistence(role, &request, &agent_id)?
                    {
                        prepare_master_attention_reasoning_continuation(
                            &resolution,
                            &mut next_prompt,
                            &mut carryover_segments,
                            &request.prompt,
                            &public_provider_text,
                            LiveRoundContext {
                                role,
                                execution_profile: request.execution_profile,
                                configured_worker_set,
                                web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                                runtime_home: &request.runtime_home,
                                cwd: request.cwd.as_deref(),
                                agent_id: &agent_id,
                            },
                        )?;
                        turns.push(turn);
                        continue 'reason_loop;
                    }
                    let _ = engine
                        .submit_completion(&mut turn, &submission)
                        .map_err(|err| RuntimeLiveBridgeError::TurnStartFailed(err.to_string()))?;
                    drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
                    drain_debug_events(&debug_receiver, &mut on_debug);
                    ensure_live_not_cancelled(&request)?;
                    write_control_hook_metadata(
                        &metadata_center,
                        &agent_id,
                        &request.session_id,
                        RuntimeControlHookWriteSpec {
                            turn_id: Some(&turn.request.turn_id),
                            trace_id: &turn.request.trace_id,
                            pipeline_node: "ControlHook04BeforeClientReturn",
                            metadata_suffix: "before_client_return:status_stop".to_owned(),
                            symbol_path: "run_live_provider_reason_turn",
                            entries: vec![
                                MetadataEntry {
                                    key: "control.hook".to_owned(),
                                    value: json!("ControlHook04BeforeClientReturn"),
                                },
                                MetadataEntry {
                                    key: "control.decision".to_owned(),
                                    value: json!(control_decision_label(&decision)),
                                },
                                MetadataEntry {
                                    key: "control.public_projection_stripped".to_owned(),
                                    value: json!(true),
                                },
                            ],
                        },
                    )?;
                    write_live_bridge_metadata(
                        &metadata_center,
                        &agent_id,
                        &request.session_id,
                        RuntimeMetadataWriteSpec {
                            turn_id: Some(&turn.request.turn_id),
                            trace_id: &turn.request.trace_id,
                            kind: MetadataKind::RuntimeState,
                            pipeline_node: "RuntimeLive04TurnClosed",
                            metadata_suffix: "turn_closed".to_owned(),
                            symbol_path: "run_live_provider_reason_turn",
                            entries: vec![
                                MetadataEntry {
                                    key: "bridge.rounds".to_owned(),
                                    value: json!(round),
                                },
                                MetadataEntry {
                                    key: "bridge.schema_rejections".to_owned(),
                                    value: json!(schema_rejections.len()),
                                },
                                MetadataEntry {
                                    key: "bridge.tool_executions".to_owned(),
                                    value: json!(tool_executions),
                                },
                                MetadataEntry {
                                    key: "terminal.status".to_owned(),
                                    value: json!("Success"),
                                },
                            ],
                        },
                    )?;
                    emit_live_bridge_debug(
                        &debug_hub,
                        &agent_id,
                        &request.session_id,
                        RuntimeDebugEmitSpec {
                            turn_id: &turn.request.turn_id,
                            trace_id: &turn.request.trace_id,
                            pipeline_node: "ControlHook04BeforeClientReturn",
                            function: "run_live_provider_reason_turn",
                            status_text: "control status accepted stop",
                            detail_lines: vec![
                                format!("decision={}", control_decision_label(&decision)),
                                "public_projection_stripped=true".to_owned(),
                            ],
                        },
                    );
                    drain_debug_events(&debug_receiver, &mut on_debug);
                    persistence
                        .record_turn_closed(&history, &turn, schema_rejections.len() as u32)
                        .map_err(|err| {
                            RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string())
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
                ControlRhythmDecision::StopBlocked(blocked_reason) => {
                    if let Some(resolution) =
                        enter_master_terminal_persistence(role, &request, &agent_id)?
                    {
                        prepare_master_attention_reasoning_continuation(
                            &resolution,
                            &mut next_prompt,
                            &mut carryover_segments,
                            &request.prompt,
                            &public_provider_text,
                            LiveRoundContext {
                                role,
                                execution_profile: request.execution_profile,
                                configured_worker_set,
                                web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                                runtime_home: &request.runtime_home,
                                cwd: request.cwd.as_deref(),
                                agent_id: &agent_id,
                            },
                        )?;
                        turns.push(turn);
                        continue 'reason_loop;
                    }
                    engine.block_turn(&mut turn, blocked_reason);
                    drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
                    drain_debug_events(&debug_receiver, &mut on_debug);
                    ensure_live_not_cancelled(&request)?;
                    write_control_hook_metadata(
                        &metadata_center,
                        &agent_id,
                        &request.session_id,
                        RuntimeControlHookWriteSpec {
                            turn_id: Some(&turn.request.turn_id),
                            trace_id: &turn.request.trace_id,
                            pipeline_node: "ControlHook04BeforeClientReturn",
                            metadata_suffix: "before_client_return:status_blocked".to_owned(),
                            symbol_path: "run_live_provider_reason_turn",
                            entries: vec![
                                MetadataEntry {
                                    key: "control.hook".to_owned(),
                                    value: json!("ControlHook04BeforeClientReturn"),
                                },
                                MetadataEntry {
                                    key: "control.decision".to_owned(),
                                    value: json!("stop_blocked"),
                                },
                                MetadataEntry {
                                    key: "control.public_projection_stripped".to_owned(),
                                    value: json!(true),
                                },
                            ],
                        },
                    )?;
                    persistence
                        .record_turn_closed(&history, &turn, schema_rejections.len() as u32)
                        .map_err(|err| {
                            RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string())
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
                ControlRhythmDecision::ContinueWithNextStep(next_step) => {
                    if let Some(reason) =
                        task_decision_round_budget_reason(task_decision_boundary.as_ref(), round)
                    {
                        finalize_framework_live_turn(
                            &engine,
                            &persistence,
                            &history,
                            &receiver,
                            &debug_receiver,
                            &mut broadcasts,
                            &mut on_broadcast,
                            &mut on_debug,
                            &mut turn,
                            FrameworkLiveTurnFinalization::Blocked(reason),
                            schema_rejections.len() as u32,
                        )?;
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
                    consecutive_schema_rejections = 0;
                    next_prompt = next_step;
                    carryover_segments = next_round_segments(
                        &request.prompt,
                        &public_provider_text,
                        None,
                        LiveRoundContext {
                            role,
                            execution_profile: request.execution_profile,
                            configured_worker_set,
                            web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                            runtime_home: &request.runtime_home,
                            cwd: request.cwd.as_deref(),
                            agent_id: &agent_id,
                        },
                    )?;
                    turns.push(turn);
                    continue;
                }
            }
        }
        let visible_text = public_provider_text;
        let completion_submission = match parse_completion_submission_block(&provider_text) {
            Ok(submission) => {
                match master_session_completion_rejection(
                    &request.runtime_home,
                    &agent_id,
                    &request.session_id,
                    role,
                    task_decision_boundary.as_ref(),
                    &submission,
                )? {
                    Some(rejection) => Err(rejection),
                    None => Ok(submission),
                }
            }
            Err(rejection) => Err(rejection),
        };
        match completion_submission {
            Ok(submission) => match validate_completion_submission(&submission)
                .expect("completion submission already validated")
            {
                CompletionDecision::Completed { .. }
                | CompletionDecision::Waiting { .. }
                | CompletionDecision::Blocked { .. } => {
                    ensure_live_not_cancelled(&request)?;
                    if let Some(resolution) =
                        enter_master_terminal_persistence(role, &request, &agent_id)?
                    {
                        prepare_master_attention_reasoning_continuation(
                            &resolution,
                            &mut next_prompt,
                            &mut carryover_segments,
                            &request.prompt,
                            &visible_text,
                            LiveRoundContext {
                                role,
                                execution_profile: request.execution_profile,
                                configured_worker_set,
                                web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                                runtime_home: &request.runtime_home,
                                cwd: request.cwd.as_deref(),
                                agent_id: &agent_id,
                            },
                        )?;
                        turns.push(turn);
                        continue 'reason_loop;
                    }
                    let _ = engine
                        .submit_completion(&mut turn, &submission)
                        .map_err(|err| RuntimeLiveBridgeError::TurnStartFailed(err.to_string()))?;
                    drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
                    drain_debug_events(&debug_receiver, &mut on_debug);
                    ensure_live_not_cancelled(&request)?;
                    write_live_bridge_metadata(
                        &metadata_center,
                        &agent_id,
                        &request.session_id,
                        RuntimeMetadataWriteSpec {
                            turn_id: Some(&turn.request.turn_id),
                            trace_id: &turn.request.trace_id,
                            kind: MetadataKind::RuntimeState,
                            pipeline_node: "RuntimeLive04TurnClosed",
                            metadata_suffix: "turn_closed".to_owned(),
                            symbol_path: "run_live_provider_reason_turn",
                            entries: vec![
                                MetadataEntry {
                                    key: "bridge.rounds".to_owned(),
                                    value: json!(round),
                                },
                                MetadataEntry {
                                    key: "bridge.schema_rejections".to_owned(),
                                    value: json!(schema_rejections.len()),
                                },
                                MetadataEntry {
                                    key: "bridge.tool_executions".to_owned(),
                                    value: json!(tool_executions),
                                },
                                MetadataEntry {
                                    key: "terminal.status".to_owned(),
                                    value: json!(format!(
                                        "{:?}",
                                        turn.terminal_event
                                            .as_ref()
                                            .expect("terminal event after completion")
                                            .status
                                    )),
                                },
                            ],
                        },
                    )?;
                    emit_live_bridge_debug(
                        &debug_hub,
                        &agent_id,
                        &request.session_id,
                        RuntimeDebugEmitSpec {
                            turn_id: &turn.request.turn_id,
                            trace_id: &turn.request.trace_id,
                            pipeline_node: "RuntimeLive04TurnClosed",
                            function: "run_live_provider_reason_turn",
                            status_text: "turn closed",
                            detail_lines: terminal_debug_details(
                                round,
                                schema_rejections.len(),
                                tool_executions,
                                turn.terminal_event
                                    .as_ref()
                                    .expect("terminal event after completion")
                                    .status
                                    .clone(),
                            ),
                        },
                    );
                    drain_debug_events(&debug_receiver, &mut on_debug);
                    persistence
                        .record_turn_closed(&history, &turn, schema_rejections.len() as u32)
                        .map_err(|err| {
                            RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string())
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
                    if let Some(reason) =
                        task_decision_round_budget_reason(task_decision_boundary.as_ref(), round)
                    {
                        finalize_framework_live_turn(
                            &engine,
                            &persistence,
                            &history,
                            &receiver,
                            &debug_receiver,
                            &mut broadcasts,
                            &mut on_broadcast,
                            &mut on_debug,
                            &mut turn,
                            FrameworkLiveTurnFinalization::Blocked(reason),
                            schema_rejections.len() as u32,
                        )?;
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
                    consecutive_schema_rejections = 0;
                    next_prompt = next_step;
                    carryover_segments = next_round_segments(
                        &request.prompt,
                        &visible_text,
                        None,
                        LiveRoundContext {
                            role,
                            execution_profile: request.execution_profile,
                            configured_worker_set,
                            web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                            runtime_home: &request.runtime_home,
                            cwd: request.cwd.as_deref(),
                            agent_id: &agent_id,
                        },
                    )?;
                    turns.push(turn);
                }
            },
            Err(rejection) => {
                ensure_live_not_cancelled(&request)?;
                let feedback = completion_schema_rejection_feedback(&rejection);
                schema_rejections.push(rejection.clone());
                consecutive_schema_rejections = consecutive_schema_rejections.saturating_add(1);
                write_error_center_metadata(
                    &metadata_center,
                    &agent_id,
                    &request.session_id,
                    RuntimeErrorCenterWriteSpec {
                        turn_id: Some(&turn.request.turn_id),
                        trace_id: &turn.request.trace_id,
                        pipeline_node: "ReasonResp04CompletionSchemaRejected",
                        metadata_suffix: format!(
                            "schema_rejected:{}",
                            consecutive_schema_rejections
                        ),
                        symbol_path: "run_live_provider_reason_turn",
                        observed: ErrorCenterObservedFailure {
                            source_owner: "reason.turn".to_owned(),
                            source_pipeline_node: "ReasonResp04CompletionSchemaRejected".to_owned(),
                            code: "completion_schema_rejected".to_owned(),
                            message: feedback.clone(),
                            retry_index: consecutive_schema_rejections as u32,
                            retry_cap: 3,
                        },
                    },
                )?;
                persistence
                    .record_completion_rejected(
                        &history,
                        &turn,
                        &rejection,
                        consecutive_schema_rejections as u32,
                    )
                    .map_err(|err| {
                        RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string())
                    })?;
                if consecutive_schema_rejections >= 3 {
                    engine.block_turn(
                        &mut turn,
                        format!(
                            "Completion schema still invalid after 3 repair attempts.\n{}",
                            feedback
                        ),
                    );
                    drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
                    drain_debug_events(&debug_receiver, &mut on_debug);
                    ensure_live_not_cancelled(&request)?;
                    write_live_bridge_metadata(
                        &metadata_center,
                        &agent_id,
                        &request.session_id,
                        RuntimeMetadataWriteSpec {
                            turn_id: Some(&turn.request.turn_id),
                            trace_id: &turn.request.trace_id,
                            kind: MetadataKind::RuntimeState,
                            pipeline_node: "RuntimeLive04TurnClosed",
                            metadata_suffix: "turn_closed".to_owned(),
                            symbol_path: "run_live_provider_reason_turn",
                            entries: vec![
                                MetadataEntry {
                                    key: "bridge.rounds".to_owned(),
                                    value: json!(round),
                                },
                                MetadataEntry {
                                    key: "bridge.schema_rejections".to_owned(),
                                    value: json!(schema_rejections.len()),
                                },
                                MetadataEntry {
                                    key: "bridge.tool_executions".to_owned(),
                                    value: json!(tool_executions),
                                },
                                MetadataEntry {
                                    key: "terminal.status".to_owned(),
                                    value: json!(format!(
                                        "{:?}",
                                        turn.terminal_event
                                            .as_ref()
                                            .expect("terminal event after failure")
                                            .status
                                    )),
                                },
                            ],
                        },
                    )?;
                    emit_live_bridge_debug(
                        &debug_hub,
                        &agent_id,
                        &request.session_id,
                        RuntimeDebugEmitSpec {
                            turn_id: &turn.request.turn_id,
                            trace_id: &turn.request.trace_id,
                            pipeline_node: "RuntimeLive04TurnClosed",
                            function: "run_live_provider_reason_turn",
                            status_text: "turn closed",
                            detail_lines: terminal_debug_details(
                                round,
                                schema_rejections.len(),
                                tool_executions,
                                turn.terminal_event
                                    .as_ref()
                                    .expect("terminal event after failure")
                                    .status
                                    .clone(),
                            ),
                        },
                    );
                    drain_debug_events(&debug_receiver, &mut on_debug);
                    persistence
                        .record_turn_closed(&history, &turn, schema_rejections.len() as u32)
                        .map_err(|err| {
                            RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string())
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
                let retry_event = ReasonBroadcastEvent::CompletionSchemaRejected(
                    ReasonResp04CompletionSchemaRejected {
                        session_id: turn.request.session_id.clone(),
                        turn_id: turn.request.turn_id.clone(),
                        trace_id: turn.request.trace_id.clone(),
                        feature_id: turn.request.feature_id.clone(),
                        agent_id: turn.request.agent_id.clone(),
                        retry_index: consecutive_schema_rejections as u32,
                        rejection: rejection.clone(),
                        feedback: feedback.clone(),
                    },
                );
                on_broadcast(&retry_event);
                broadcasts.push(retry_event);
                next_prompt = feedback.clone();
                carryover_segments = next_round_segments(
                    &request.prompt,
                    &visible_text,
                    Some(feedback.as_str()),
                    LiveRoundContext {
                        role,
                        execution_profile: request.execution_profile,
                        configured_worker_set,
                        web_search_route_guidance: Some(web_search_route_guidance.as_str()),
                        runtime_home: &request.runtime_home,
                        cwd: request.cwd.as_deref(),
                        agent_id: &agent_id,
                    },
                )?;
                turns.push(turn);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCommandDispatcherConfig {
    pub session_id: SessionId,
    pub reason_agent_id: AgentId,
    pub master_agent_id: AgentId,
    pub master_node_id: String,
    pub slave_agent_id: AgentId,
    pub slave_node_id: String,
    pub pair_token: String,
    pub allowed_pair_ip: Option<String>,
    pub model: String,
    pub live: Option<RuntimeLiveDispatcherConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLiveDispatcherConfig {
    pub selected_agent: SelectedAgentConfig,
    pub runtime_home: PathBuf,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAgentBootstrap {
    pub selected_agent: SelectedAgentConfig,
    pub runtime_home: PathBuf,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeAgentBootstrapError {
    #[error("agent name must not be empty")]
    EmptyAgentName,
    #[error("config load failed: {0}")]
    ConfigLoad(String),
    #[error("agent selection failed: {0}")]
    AgentSelection(String),
    #[error("paired agent `{paired_agent_name}` environment variable `{env_var}` is not set")]
    MissingPairedTokenEnv {
        paired_agent_name: String,
        env_var: String,
    },
    #[error("paired agent `{paired_agent_name}` environment variable `{env_var}` is empty")]
    EmptyPairedTokenEnv {
        paired_agent_name: String,
        env_var: String,
    },
    #[error(
        "agent `{agent_name}` pair token does not match paired agent `{paired_agent_name}` pair token"
    )]
    PairTokenMismatch {
        agent_name: String,
        paired_agent_name: String,
    },
}

pub fn load_default_runtime_agent(
    agent_name: &str,
) -> Result<RuntimeAgentBootstrap, RuntimeAgentBootstrapError> {
    if agent_name.trim().is_empty() {
        return Err(RuntimeAgentBootstrapError::EmptyAgentName);
    }
    let config = load_default_config()
        .map_err(|err| RuntimeAgentBootstrapError::ConfigLoad(err.to_string()))?;
    let selected = config
        .select_agent(agent_name)
        .map_err(|err| RuntimeAgentBootstrapError::AgentSelection(err.to_string()))?;
    for peer in &selected.paired_agents {
        let paired_pair_token = env::var(&peer.pair_token_env).map_err(|_| {
            RuntimeAgentBootstrapError::MissingPairedTokenEnv {
                paired_agent_name: peer.name.clone(),
                env_var: peer.pair_token_env.clone(),
            }
        })?;
        if paired_pair_token.trim().is_empty() {
            return Err(RuntimeAgentBootstrapError::EmptyPairedTokenEnv {
                paired_agent_name: peer.name.clone(),
                env_var: peer.pair_token_env.clone(),
            });
        }
        if paired_pair_token != selected.pair_token {
            return Err(RuntimeAgentBootstrapError::PairTokenMismatch {
                agent_name: selected.name.clone(),
                paired_agent_name: peer.name.clone(),
            });
        }
    }
    let runtime_home = default_config_path()
        .map_err(|err| RuntimeAgentBootstrapError::ConfigLoad(err.to_string()))?
        .parent()
        .ok_or_else(|| {
            RuntimeAgentBootstrapError::ConfigLoad(
                "default config path has no runtime home parent".to_owned(),
            )
        })?
        .to_path_buf();
    Ok(RuntimeAgentBootstrap {
        selected_agent: selected,
        runtime_home,
    })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeCommandDispatcherError {
    #[error("master node id must not be empty")]
    EmptyMasterNodeId,
    #[error("slave node id must not be empty")]
    EmptySlaveNodeId,
    #[error("pair token must not be empty")]
    EmptyPairToken,
    #[error("agent name must not be empty")]
    EmptyAgentName,
    #[error("model must not be empty")]
    EmptyModel,
    #[error("runtime host requires a master agent, but `{agent_name}` is configured as `{mode}`")]
    HostRequiresMasterMode { agent_name: String, mode: String },
    #[error("runtime host master `{agent_name}` requires at least one configured worker peer")]
    HostRequiresWorkerPeer { agent_name: String },
    #[error("config load failed: {0}")]
    ConfigLoad(String),
    #[error("agent selection failed: {0}")]
    AgentSelection(String),
    #[error("paired agent `{paired_agent_name}` environment variable `{env_var}` is not set")]
    MissingPairedTokenEnv {
        paired_agent_name: String,
        env_var: String,
    },
    #[error("paired agent `{paired_agent_name}` environment variable `{env_var}` is empty")]
    EmptyPairedTokenEnv {
        paired_agent_name: String,
        env_var: String,
    },
    #[error(
        "agent `{agent_name}` pair token does not match paired agent `{paired_agent_name}` pair token"
    )]
    PairTokenMismatch {
        agent_name: String,
        paired_agent_name: String,
    },
    #[error("session history init failed: {0}")]
    SessionHistoryInit(String),
    #[error("node runtime init failed: {0}")]
    NodeRuntimeInit(String),
    #[error("node runtime pairing failed: {0}")]
    NodeRuntimePairing(String),
    #[error("reason persistence bootstrap restore failed: {0}")]
    ReasonPersistenceBootstrap(String),
    #[error("checkpoint projection bootstrap failed: {0}")]
    CheckpointProjectionBootstrap(String),
}

fn runtime_dispatcher_bootstrap_error(
    error: RuntimeAgentBootstrapError,
) -> RuntimeCommandDispatcherError {
    match error {
        RuntimeAgentBootstrapError::EmptyAgentName => RuntimeCommandDispatcherError::EmptyAgentName,
        RuntimeAgentBootstrapError::ConfigLoad(message) => {
            RuntimeCommandDispatcherError::ConfigLoad(message)
        }
        RuntimeAgentBootstrapError::AgentSelection(message) => {
            RuntimeCommandDispatcherError::AgentSelection(message)
        }
        RuntimeAgentBootstrapError::MissingPairedTokenEnv {
            paired_agent_name,
            env_var,
        } => RuntimeCommandDispatcherError::MissingPairedTokenEnv {
            paired_agent_name,
            env_var,
        },
        RuntimeAgentBootstrapError::EmptyPairedTokenEnv {
            paired_agent_name,
            env_var,
        } => RuntimeCommandDispatcherError::EmptyPairedTokenEnv {
            paired_agent_name,
            env_var,
        },
        RuntimeAgentBootstrapError::PairTokenMismatch {
            agent_name,
            paired_agent_name,
        } => RuntimeCommandDispatcherError::PairTokenMismatch {
            agent_name,
            paired_agent_name,
        },
    }
}

struct RuntimeCommandDispatcherState {
    config: RuntimeCommandDispatcherConfig,
    reason_engine: ReasonTurnEngine,
    session_history: SessionHistory,
    turns: Vec<TurnRecord>,
    session_cwds: BTreeMap<SessionId, PathBuf>,
    active_turns: Vec<ActiveRuntimeTurn>,
    node_runtime: LocalNodeRuntime,
    next_turn_ordinal: u64,
    pending_config_status: Option<UiConfigStatusProjection>,
}

#[derive(Clone)]
struct ActiveRuntimeTurn {
    turn_id: TurnId,
    session_id: SessionId,
    cwd: PathBuf,
    trace_id: TraceId,
    user_text: String,
    cancel_token: LiveReasonCancelToken,
}

struct PreparedLiveSubmit {
    live: RuntimeLiveDispatcherConfig,
    reason_agent_id: AgentId,
    master_node_id: String,
    session_id: SessionId,
    cwd: PathBuf,
    turn_id: TurnId,
    trace_id: TraceId,
    prompt: String,
    attachments: Vec<ProviderInputAttachment>,
    attachment_metadata: Vec<InputAttachmentMetadata>,
    cancel_token: LiveReasonCancelToken,
}

pub struct RuntimeCommandDispatcher {
    ui_state: Arc<Mutex<UiProtocolState>>,
    state: Mutex<RuntimeCommandDispatcherState>,
}

impl RuntimeCommandDispatcher {
    pub fn from_default_config(agent_name: &str) -> Result<Self, RuntimeCommandDispatcherError> {
        let bootstrap =
            load_default_runtime_agent(agent_name).map_err(runtime_dispatcher_bootstrap_error)?;
        Self::from_selected_agent_with_live(
            &bootstrap.selected_agent,
            bootstrap.runtime_home,
            false,
        )
    }

    pub fn from_selected_agent(
        selected: &SelectedAgentConfig,
    ) -> Result<Self, RuntimeCommandDispatcherError> {
        Self::from_selected_agent_inner(selected, None)
    }

    pub fn from_selected_agent_with_live(
        selected: &SelectedAgentConfig,
        runtime_home: PathBuf,
        stream: bool,
    ) -> Result<Self, RuntimeCommandDispatcherError> {
        Self::from_selected_agent_inner(
            selected,
            Some(RuntimeLiveDispatcherConfig {
                selected_agent: selected.clone(),
                runtime_home,
                stream,
            }),
        )
    }

    fn from_selected_agent_inner(
        selected: &SelectedAgentConfig,
        live: Option<RuntimeLiveDispatcherConfig>,
    ) -> Result<Self, RuntimeCommandDispatcherError> {
        if selected.name.trim().is_empty() {
            return Err(RuntimeCommandDispatcherError::EmptyAgentName);
        }
        if selected.mode != AgentMode::Master {
            return Err(RuntimeCommandDispatcherError::HostRequiresMasterMode {
                agent_name: selected.name.clone(),
                mode: selected.mode.as_str().to_owned(),
            });
        }
        let first_worker_peer = selected.worker_peers().next().ok_or_else(|| {
            RuntimeCommandDispatcherError::HostRequiresWorkerPeer {
                agent_name: selected.name.clone(),
            }
        })?;

        Self::new(RuntimeCommandDispatcherConfig {
            session_id: SessionId::new(format!("runtime-session-{}", selected.name)),
            reason_agent_id: AgentId::new(selected.name.clone()),
            master_agent_id: AgentId::new(selected.name.clone()),
            master_node_id: selected.node_id.clone(),
            slave_agent_id: AgentId::new(first_worker_peer.name.clone()),
            slave_node_id: first_worker_peer.node_id.clone(),
            pair_token: selected.pair_token.clone(),
            allowed_pair_ip: first_worker_peer.allowed_pair_ip.map(|ip| ip.to_string()),
            model: selected.provider.default_model.clone(),
            live,
        })
    }

    pub fn new(
        config: RuntimeCommandDispatcherConfig,
    ) -> Result<Self, RuntimeCommandDispatcherError> {
        if config.master_node_id.trim().is_empty() {
            return Err(RuntimeCommandDispatcherError::EmptyMasterNodeId);
        }
        if config.slave_node_id.trim().is_empty() {
            return Err(RuntimeCommandDispatcherError::EmptySlaveNodeId);
        }
        if config.pair_token.trim().is_empty() {
            return Err(RuntimeCommandDispatcherError::EmptyPairToken);
        }
        if config.model.trim().is_empty() {
            return Err(RuntimeCommandDispatcherError::EmptyModel);
        }

        let mut session_history = SessionHistory::new(config.session_id.clone(), Vec::new())
            .map_err(|err| RuntimeCommandDispatcherError::SessionHistoryInit(err.to_string()))?;
        let mut turns = Vec::new();
        let mut next_turn_ordinal = 0_u64;

        let node_master = MasterNodeConfig {
            node_id: config.master_node_id.clone(),
            agent_id: config.master_agent_id.clone(),
            paired_slave_node_id: config.slave_node_id.clone(),
        };
        let node_slave = SlaveNodeConfig {
            node_id: config.slave_node_id.clone(),
            agent_id: config.slave_agent_id.clone(),
            paired_master_node_id: config.master_node_id.clone(),
            pair_token: config.pair_token.clone(),
            allowed_pair_ip: config.allowed_pair_ip.clone(),
        };
        let mut node_runtime = if let Some(live) = &config.live {
            let metadata_center = Arc::new(Mutex::new(
                MetadataCenter::with_ledger_path(metadata_ledger_path(
                    &live.runtime_home,
                    &config.reason_agent_id,
                    &config.session_id,
                ))
                .map_err(|err| RuntimeCommandDispatcherError::NodeRuntimeInit(err.to_string()))?,
            ));
            LocalNodeRuntime::with_metadata_center(node_master, node_slave, metadata_center)
        } else {
            LocalNodeRuntime::new(node_master, node_slave)
        }
        .map_err(|err| RuntimeCommandDispatcherError::NodeRuntimeInit(err.to_string()))?;

        node_runtime
            .pair_slave(PairingRequest {
                source_node_id: config.master_node_id.clone(),
                source_ip: config.allowed_pair_ip.clone(),
                presented_token: config.pair_token.clone(),
                transport: PairingTransport::WebSocket,
            })
            .map_err(|err| RuntimeCommandDispatcherError::NodeRuntimePairing(err.to_string()))?;

        let ui_state = Arc::new(Mutex::new(UiProtocolState::default()));
        if let Some(node_status) = node_runtime.query_node_status() {
            ui_state
                .lock()
                .expect("lock ui state")
                .set_node_status(node_status);
        }
        if let Some(live) = &config.live {
            let persistence =
                ReasonPersistence::new(live.runtime_home.clone(), config.reason_agent_id.clone());
            next_turn_ordinal = restore_all_persisted_sessions_into_ui(
                &persistence,
                &ui_state,
                &config.reason_agent_id,
                &config.master_node_id,
            )
            .map_err(|err| {
                RuntimeCommandDispatcherError::ReasonPersistenceBootstrap(err.to_string())
            })?;
            let metadata_entries = persistence.load_session_metadata().map_err(|err| {
                RuntimeCommandDispatcherError::ReasonPersistenceBootstrap(err.to_string())
            })?;
            ui_state
                .lock()
                .expect("lock ui state")
                .set_session_metadata_entries(
                    metadata_entries.into_iter().map(session_metadata_to_ui),
                );
            match persistence.restore(&config.session_id) {
                Ok(restored) => {
                    session_history = restored.history;
                    turns = restored.closed_turns;
                    if let Some(active) = restored.active_turn {
                        turns.push(active.turn);
                    }
                    turns.sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
                    next_turn_ordinal = next_turn_ordinal.max(
                        turns
                            .iter()
                            .map(|turn| runtime_turn_position(&turn.request.turn_id))
                            .map(|(ordinal, _round, _raw)| ordinal)
                            .max()
                            .unwrap_or(0),
                    );
                }
                Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => {}
                Err(err) => {
                    return Err(RuntimeCommandDispatcherError::ReasonPersistenceBootstrap(
                        err.to_string(),
                    ));
                }
            }
        }

        let mut session_cwds = session_cwds_from_turns(&turns);
        if let Some(live) = &config.live {
            let persistence =
                ReasonPersistence::new(live.runtime_home.clone(), config.reason_agent_id.clone());
            for metadata in persistence.load_session_metadata().map_err(|err| {
                RuntimeCommandDispatcherError::ReasonPersistenceBootstrap(err.to_string())
            })? {
                if let Some(cwd) = metadata.cwd
                    && let Ok(path) = fs::canonicalize(cwd)
                {
                    session_cwds.insert(metadata.session_id, path);
                }
            }
        }
        let dispatcher = Self {
            ui_state,
            state: Mutex::new(RuntimeCommandDispatcherState {
                config,
                reason_engine: ReasonTurnEngine::new(),
                session_history,
                turns,
                session_cwds,
                active_turns: Vec::new(),
                node_runtime,
                next_turn_ordinal,
                pending_config_status: None,
            }),
        };
        dispatcher
            .recover_stale_master_active_work_on_bootstrap()
            .map_err(|err| {
                RuntimeCommandDispatcherError::ReasonPersistenceBootstrap(err.to_string())
            })?;
        dispatcher.refresh_checkpoint_projection().map_err(|err| {
            RuntimeCommandDispatcherError::CheckpointProjectionBootstrap(err.to_string())
        })?;
        Ok(dispatcher)
    }

    pub fn ui_state(&self) -> Arc<Mutex<UiProtocolState>> {
        Arc::clone(&self.ui_state)
    }

    pub fn query_runtime(
        &self,
        command: &UiCommand,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError> {
        let state = self.state.lock().expect("lock runtime dispatcher state");
        match command {
            UiCommand::QuerySessionSearch { query, limit } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                Ok(Some(UiQueryResult::SessionSearch(
                    query_session_search_for_ui(&state.config, live, query, *limit)?,
                )))
            }
            UiCommand::QuerySessionTurns { session_id } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let Some((source_agent_id, mut turns)) = restore_session_turns_for_ui_query(
                    &state.config,
                    &live.runtime_home,
                    session_id,
                )?
                else {
                    if session_id.as_str().starts_with("worker-task-") {
                        return Err(UiCommandDispatchPortError::TargetNotFound(format!(
                            "Worker session `{}` has no persisted transcript",
                            session_id.as_str()
                        )));
                    }
                    return Ok(None);
                };
                turns.sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
                let source_node_id = node_id_for_query_agent(&state.config, &source_agent_id)?;
                let projections = turns
                    .iter()
                    .map(|turn| {
                        let cwd = state
                            .session_cwds
                            .get(session_id)
                            .map(|path| path.to_string_lossy().into_owned())
                            .or_else(|| turn.cwd.clone());
                        project_runtime_turn(&source_agent_id, &source_node_id, turn, cwd)
                    })
                    .collect::<Vec<_>>();
                let mut ui = self.ui_state.lock().expect("lock ui state");
                apply_error_center_live_activity_to_session_projections(
                    &live.runtime_home,
                    &source_agent_id,
                    &source_node_id,
                    session_id,
                    &mut ui,
                    projections,
                )?;
                ui.query(command)
                    .map(Some)
                    .map_err(|error| UiCommandDispatchPortError::DispatchFailed(error.to_string()))
            }
            UiCommand::QueryConfigStatus => {
                if let Some(status) = state.pending_config_status.clone() {
                    return Ok(Some(UiQueryResult::ConfigStatus(status)));
                }
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                Ok(Some(UiQueryResult::ConfigStatus(
                    project_live_config_status_for_ui(live)?,
                )))
            }
            UiCommand::QueryTaskList { status, agent_id } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let task_runtime =
                    TaskRuntime::boot(&live.runtime_home, state.config.reason_agent_id.clone())
                        .map_err(map_task_query_error)?;
                let status_filter = status
                    .as_deref()
                    .map(parse_task_status)
                    .transpose()
                    .map_err(UiCommandDispatchPortError::DispatchFailed)?;
                let tasks = task_runtime
                    .list_tasks(TaskListQuery {
                        status: status_filter,
                        assignee: agent_id.clone(),
                    })
                    .map_err(map_task_query_error)?;
                Ok(Some(UiQueryResult::TaskList(project_task_list_for_ui(
                    state.config.reason_agent_id.clone(),
                    status.clone(),
                    agent_id.clone(),
                    tasks,
                ))))
            }
            UiCommand::QueryTaskBoard {
                status,
                agent_id,
                include_terminal,
            } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let task_runtime =
                    TaskRuntime::boot(&live.runtime_home, state.config.reason_agent_id.clone())
                        .map_err(map_task_query_error)?;
                let status_filter = status
                    .as_deref()
                    .map(parse_task_status)
                    .transpose()
                    .map_err(UiCommandDispatchPortError::DispatchFailed)?;
                let board = task_runtime
                    .query_task_board(TaskBoardQuery {
                        status: status_filter,
                        assignee: agent_id.clone(),
                        include_terminal: *include_terminal,
                    })
                    .map_err(map_task_query_error)?;
                Ok(Some(UiQueryResult::TaskBoard(project_task_board_for_ui(
                    state.config.reason_agent_id.clone(),
                    status.clone(),
                    agent_id.clone(),
                    *include_terminal,
                    board,
                ))))
            }
            UiCommand::QueryAgentBoard => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let task_runtime =
                    TaskRuntime::boot(&live.runtime_home, state.config.reason_agent_id.clone())
                        .map_err(map_task_query_error)?;
                let board = task_runtime
                    .query_agent_board()
                    .map_err(map_task_query_error)?;
                Ok(Some(UiQueryResult::AgentBoard(project_agent_board_for_ui(
                    state.config.reason_agent_id.clone(),
                    board.agents,
                ))))
            }
            UiCommand::QueryAgentLifecycle { agent_id } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let task_runtime =
                    TaskRuntime::boot(&live.runtime_home, state.config.reason_agent_id.clone())
                        .map_err(map_task_query_error)?;
                let lifecycle = task_runtime
                    .query_agent_lifecycle(agent_id)
                    .map_err(map_task_query_error)?;
                Ok(Some(UiQueryResult::AgentLifecycle(
                    project_agent_lifecycle_for_ui(lifecycle),
                )))
            }
            UiCommand::QueryTaskHistory { task_id } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let task_runtime =
                    TaskRuntime::boot(&live.runtime_home, state.config.reason_agent_id.clone())
                        .map_err(map_task_query_error)?;
                let events = task_runtime
                    .task_history(&TaskId::new(task_id.clone()))
                    .map_err(map_task_query_error)?;
                Ok(Some(UiQueryResult::TaskHistory(
                    project_task_history_for_ui(
                        state.config.reason_agent_id.clone(),
                        task_id.clone(),
                        events,
                    ),
                )))
            }
            UiCommand::QueryWorkerControl {
                task_id,
                execution_id,
            } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let task_runtime =
                    TaskRuntime::boot(&live.runtime_home, state.config.reason_agent_id.clone())
                        .map_err(map_task_query_error)?;
                let events = task_runtime
                    .query_worker_control_events(&TaskId::new(task_id.clone()), execution_id)
                    .map_err(map_task_query_error)?;
                Ok(Some(UiQueryResult::WorkerControl(Box::new(
                    project_worker_control_events_for_ui(
                        state.config.reason_agent_id.clone(),
                        events,
                    ),
                ))))
            }
            UiCommand::QueryTimerList { include_terminal } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let store = TimerStore::new(&live.runtime_home, &state.config.reason_agent_id);
                let schedules = store
                    .load_schedules()
                    .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
                let events = store
                    .load_events()
                    .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
                Ok(Some(UiQueryResult::TimerList(project_timer_list_for_ui(
                    state.config.reason_agent_id.clone(),
                    *include_terminal,
                    schedules,
                    events,
                ))))
            }
            UiCommand::QueryToolRegistry => Ok(Some(UiQueryResult::ToolRegistry(
                project_tool_registry_for_ui(state.config.reason_agent_id.clone()),
            ))),
            UiCommand::QueryDiagnostics => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                Ok(Some(UiQueryResult::Diagnostics(
                    project_diagnostics_for_ui(
                        state.config.reason_agent_id.clone(),
                        &live.runtime_home,
                    )?,
                )))
            }
            UiCommand::QueryEventInbox {
                after_cursor,
                limit,
            } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let task_runtime =
                    TaskRuntime::boot(&live.runtime_home, state.config.reason_agent_id.clone())
                        .map_err(map_task_query_error)?;
                let inbox = task_runtime
                    .query_event_inbox(TaskEventInboxQuery {
                        after_cursor: after_cursor.clone(),
                        limit: limit.unwrap_or(0),
                    })
                    .map_err(map_task_query_error)?;
                Ok(Some(UiQueryResult::EventInbox(project_event_inbox_for_ui(
                    state.config.reason_agent_id.clone(),
                    inbox,
                ))))
            }
            UiCommand::RunMasterPoll {
                after_cursor,
                limit,
                include_terminal,
                replay_from_start,
            } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let task_runtime =
                    TaskRuntime::boot(&live.runtime_home, state.config.reason_agent_id.clone())
                        .map_err(map_task_query_error)?;
                let outcome = task_runtime
                    .run_master_poll(MasterPollRequest {
                        after_cursor: after_cursor.clone(),
                        limit: limit.unwrap_or(0),
                        include_terminal: *include_terminal,
                        replay_from_start: *replay_from_start,
                        actor: ui_task_actor(&state.config.reason_agent_id, None, None),
                        watermark: ui_task_watermark("run_master_poll"),
                    })
                    .map_err(map_task_query_error)?;
                Ok(Some(UiQueryResult::MasterPoll(project_master_poll_for_ui(
                    state.config.reason_agent_id.clone(),
                    outcome,
                ))))
            }
            UiCommand::QueryErrorCenterEvents {
                session_id,
                trace_id,
                turn_id,
                domain,
            } => {
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                let projection = query_error_center_events_for_ui(
                    &live.runtime_home,
                    &state.config.reason_agent_id,
                    session_id,
                    trace_id.clone(),
                    turn_id.clone(),
                    domain.clone(),
                )
                .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
                Ok(Some(UiQueryResult::ErrorCenterEvents(projection)))
            }
            _ => Ok(None),
        }
    }

    fn dispatch_submit_user_input(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        text: String,
        requested_session_id: Option<SessionId>,
        requested_cwd: Option<String>,
        metadata: Option<UiSubmitMetadata>,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let session_id = requested_session_id.unwrap_or_else(|| state.config.session_id.clone());
        let cwd = resolve_session_cwd(state, &session_id, requested_cwd, None)?;
        state.next_turn_ordinal += 1;
        let turn_id = TurnId::new(format!("runtime-turn-{}", state.next_turn_ordinal));
        let trace_id = TraceId::new(format!("runtime-trace-{}", state.next_turn_ordinal));
        let mut session_history = if &session_id == state.session_history.session_id() {
            state.session_history.clone()
        } else {
            SessionHistory::new(session_id.clone(), Vec::new())
                .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?
        };

        let (_provider_attachments, attachment_metadata) = submit_attachment_inputs(metadata);
        let mut turn = state
            .reason_engine
            .start_turn(
                &mut session_history,
                TurnStartInput {
                    session_id: session_id.clone(),
                    turn_id,
                    trace_id,
                    feature_id: FeatureId::new("reason.turn"),
                    agent_id: state.config.reason_agent_id.clone(),
                    user_text: text,
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: state.config.model.clone(),
                },
            )
            .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        turn.cwd = Some(cwd.to_string_lossy().into_owned());
        turn.attachments = attachment_metadata;
        if session_id == state.config.session_id {
            state.session_history = session_history;
        }

        let projection = project_runtime_turn_history(
            &state.config.reason_agent_id,
            &state.config.master_node_id,
            std::slice::from_ref(&turn),
            Some(cwd.to_string_lossy().into_owned()),
        );
        state.turns.push(turn);
        self.ui_state
            .lock()
            .expect("lock ui state")
            .apply_turn_projection(projection);

        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "reason_turn_started".to_owned(),
        })
    }

    fn prepare_live_submit_user_input(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        text: String,
        requested_session_id: Option<SessionId>,
        requested_cwd: Option<String>,
        metadata: Option<UiSubmitMetadata>,
    ) -> Result<Option<PreparedLiveSubmit>, UiCommandDispatchPortError> {
        let Some(live) = state.config.live.clone() else {
            return Ok(None);
        };
        self.recover_stale_master_active_work_before_live_submit(state, &live)?;
        let session_id = requested_session_id.unwrap_or_else(|| state.config.session_id.clone());
        let cwd = resolve_session_cwd(state, &session_id, requested_cwd, Some(&live.runtime_home))?;
        let next_turn_ordinal = state.next_turn_ordinal.saturating_add(1);
        let turn_id = TurnId::new(format!("runtime-turn-{next_turn_ordinal}"));
        let trace_id = TraceId::new(format!("runtime-trace-{next_turn_ordinal}"));
        let cancel_token = Arc::new(AtomicBool::new(false));
        let (attachments, attachment_metadata) = submit_attachment_inputs(metadata);
        master_runner::register_master_active_work(
            &live.runtime_home,
            &state.config.reason_agent_id,
            &session_id,
            &turn_id,
            &trace_id,
        )
        .map_err(UiCommandDispatchPortError::DispatchFailed)?;
        state.next_turn_ordinal = next_turn_ordinal;
        state.active_turns.push(ActiveRuntimeTurn {
            turn_id: turn_id.clone(),
            session_id: session_id.clone(),
            cwd: cwd.clone(),
            trace_id: trace_id.clone(),
            user_text: text.clone(),
            cancel_token: Arc::clone(&cancel_token),
        });
        let prepared = PreparedLiveSubmit {
            live,
            reason_agent_id: state.config.reason_agent_id.clone(),
            master_node_id: state.config.master_node_id.clone(),
            session_id,
            cwd,
            turn_id,
            trace_id,
            prompt: text,
            attachments,
            attachment_metadata,
            cancel_token,
        };
        let current_turn = match persist_prepared_live_submit_active_turn(state, &prepared) {
            Ok(current_turn) => current_turn,
            Err(err) => {
                remove_active_turn(&mut state.active_turns, &prepared.turn_id);
                if state.next_turn_ordinal == next_turn_ordinal {
                    state.next_turn_ordinal = next_turn_ordinal.saturating_sub(1);
                }
                let _ = master_runner::clear_master_active_work_if_current(
                    &prepared.live.runtime_home,
                    &prepared.reason_agent_id,
                    &prepared.turn_id,
                );
                return Err(err);
            }
        };
        let projection = project_runtime_turn_history(
            &state.config.reason_agent_id,
            &state.config.master_node_id,
            std::slice::from_ref(&current_turn),
            Some(prepared.cwd.to_string_lossy().into_owned()),
        );
        self.ui_state
            .lock()
            .expect("lock ui state")
            .apply_turn_projection(projection);
        Ok(Some(prepared))
    }

    fn recover_stale_master_active_work_on_bootstrap(
        &self,
    ) -> Result<(), UiCommandDispatchPortError> {
        let mut state = self.state.lock().expect("lock runtime dispatcher state");
        let Some(live) = state.config.live.clone() else {
            return Ok(());
        };
        self.recover_stale_master_active_work_before_live_submit(&mut state, &live)
    }

    fn dispatch_prepared_live_submit(
        &self,
        envelope: UiCommandDispatchEnvelope,
        prepared: PreparedLiveSubmit,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let ui_state = Arc::clone(&self.ui_state);
        let reason_agent_id = prepared.reason_agent_id.clone();
        let master_node_id = prepared.master_node_id.clone();
        let cancel_token = Arc::clone(&prepared.cancel_token);
        let outcome = run_live_reason_turn_with_hooks(
            &prepared.live.selected_agent,
            LiveReasonTurnRequest {
                runtime_home: prepared.live.runtime_home.clone(),
                session_id: prepared.session_id.clone(),
                turn_id: prepared.turn_id.clone(),
                trace_id: prepared.trace_id.clone(),
                prompt: prepared.prompt.clone(),
                attachments: prepared.attachments.clone(),
                attachment_metadata: prepared.attachment_metadata.clone(),
                cwd: Some(prepared.cwd.clone()),
                execution_profile: LiveReasonExecutionProfile::Workspace,
                stream: prepared.live.stream,
                cancel_token: Some(Arc::clone(&cancel_token)),
            },
            |event| {
                if !cancel_token.load(Ordering::SeqCst) {
                    apply_runtime_reason_broadcast(
                        &ui_state,
                        &reason_agent_id,
                        &master_node_id,
                        event,
                    );
                }
            },
            |event| {
                if !cancel_token.load(Ordering::SeqCst) {
                    apply_runtime_debug_event(&ui_state, &reason_agent_id, &master_node_id, event);
                }
            },
            |projection| {
                if !cancel_token.load(Ordering::SeqCst) {
                    ui_state
                        .lock()
                        .expect("lock ui state")
                        .publish_task_list_projection(projection.clone());
                }
            },
        );
        let outcome = self.finish_live_submit(&prepared, outcome)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!(
                "reason_live_turn_completed rounds={} schema_rejections={} tool_executions={} restored_closed_turns={}",
                outcome.rounds,
                outcome.schema_rejections.len(),
                outcome.tool_executions,
                outcome.restored_closed_turns
            ),
        })
    }

    fn finish_live_submit(
        &self,
        prepared: &PreparedLiveSubmit,
        outcome: Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError>,
    ) -> Result<LiveReasonTurnOutcome, UiCommandDispatchPortError> {
        let mut state = self.state.lock().expect("lock runtime dispatcher state");
        let active = remove_active_turn(&mut state.active_turns, &prepared.turn_id);
        let was_cancelled = active
            .as_ref()
            .is_some_and(|turn| turn.cancel_token.load(Ordering::SeqCst))
            || prepared.cancel_token.load(Ordering::SeqCst);
        if let Err(error) = master_runner::clear_master_active_work_if_current(
            &prepared.live.runtime_home,
            &prepared.reason_agent_id,
            &prepared.turn_id,
        ) {
            let current_turn =
                restore_or_materialize_failed_live_submit(&mut state, prepared, &error)?;
            let projection = project_runtime_turn_history(
                &state.config.reason_agent_id,
                &state.config.master_node_id,
                std::slice::from_ref(&current_turn),
                Some(prepared.cwd.to_string_lossy().into_owned()),
            );
            self.ui_state
                .lock()
                .expect("lock ui state")
                .apply_turn_projection(projection);
            return Err(UiCommandDispatchPortError::DispatchFailed(error));
        }
        if was_cancelled {
            let current_turn = restore_or_materialize_cancelled_live_submit(
                &mut state,
                prepared,
                RuntimeLiveBridgeError::Cancelled.to_string().as_str(),
            )?;
            let projection = project_runtime_turn_history(
                &state.config.reason_agent_id,
                &state.config.master_node_id,
                std::slice::from_ref(&current_turn),
                Some(prepared.cwd.to_string_lossy().into_owned()),
            );
            self.ui_state
                .lock()
                .expect("lock ui state")
                .apply_turn_projection(projection);
            return Err(UiCommandDispatchPortError::DispatchFailed(
                RuntimeLiveBridgeError::Cancelled.to_string(),
            ));
        }
        match outcome {
            Ok(outcome) => {
                let projection = project_runtime_turn_history(
                    &state.config.reason_agent_id,
                    &state.config.master_node_id,
                    std::slice::from_ref(&outcome.turn),
                    Some(prepared.cwd.to_string_lossy().into_owned()),
                );
                state.turns.retain(|existing| {
                    !outcome
                        .turns
                        .iter()
                        .any(|turn| turn.request.turn_id == existing.request.turn_id)
                });
                state.turns.extend(outcome.turns.clone());
                state
                    .turns
                    .sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
                state.session_cwds = session_cwds_from_turns(&state.turns);
                state
                    .session_cwds
                    .insert(prepared.session_id.clone(), prepared.cwd.clone());
                self.ui_state
                    .lock()
                    .expect("lock ui state")
                    .apply_turn_projection(projection);
                self.refresh_checkpoint_projection_from_config(&state.config)
                    .map_err(map_checkpoint_dispatch_error)?;
                Ok(outcome)
            }
            Err(err) => {
                let current_turn = restore_or_materialize_failed_live_submit(
                    &mut state,
                    prepared,
                    &err.to_string(),
                )?;
                let projection = project_runtime_turn_history(
                    &state.config.reason_agent_id,
                    &state.config.master_node_id,
                    std::slice::from_ref(&current_turn),
                    Some(prepared.cwd.to_string_lossy().into_owned()),
                );
                self.ui_state
                    .lock()
                    .expect("lock ui state")
                    .apply_turn_projection(projection);
                let _ = self.refresh_checkpoint_projection_from_config(&state.config);
                Err(UiCommandDispatchPortError::DispatchFailed(err.to_string()))
            }
        }
    }

    fn recover_stale_master_active_work_before_live_submit(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        live: &RuntimeLiveDispatcherConfig,
    ) -> Result<(), UiCommandDispatchPortError> {
        let task_runtime =
            TaskRuntime::boot(&live.runtime_home, state.config.reason_agent_id.clone())
                .map_err(map_task_query_error)?;
        let Some(checkpoint) = master_runner::recoverable_stale_master_active_work(
            &live.runtime_home,
            &state.config.reason_agent_id,
            &task_runtime,
        )
        .map_err(UiCommandDispatchPortError::DispatchFailed)?
        else {
            return Ok(());
        };
        if state
            .active_turns
            .iter()
            .any(|active| active_runtime_turn_matches_master_work(active, &checkpoint))
        {
            return Ok(());
        }

        let persistence = ReasonPersistence::new(
            live.runtime_home.clone(),
            state.config.reason_agent_id.clone(),
        );
        let restored = match persistence.restore(&checkpoint.session_id) {
            Ok(restored) => restored,
            Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => {
                master_runner::clear_master_active_work_if_current(
                    &live.runtime_home,
                    &state.config.reason_agent_id,
                    &checkpoint.logical_turn_id,
                )
                .map_err(UiCommandDispatchPortError::DispatchFailed)?;
                return Ok(());
            }
            Err(error) => {
                return Err(UiCommandDispatchPortError::DispatchFailed(format!(
                    "failed to restore stale Master active work `{}`: {error}",
                    checkpoint.work_id
                )));
            }
        };
        let Some(active_snapshot) = restored
            .active_turn
            .as_ref()
            .filter(|snapshot| turn_record_matches_master_work(&snapshot.turn, &checkpoint))
            .cloned()
        else {
            master_runner::clear_master_active_work_if_current(
                &live.runtime_home,
                &state.config.reason_agent_id,
                &checkpoint.logical_turn_id,
            )
            .map_err(UiCommandDispatchPortError::DispatchFailed)?;
            return Ok(());
        };

        let mut turn = active_snapshot.turn;
        if turn.terminal_event.is_none() {
            state.reason_engine.interrupt_turn(
                &mut turn,
                format!(
                    "Master active work `{}` was interrupted during daemon recovery after attention `{}`; no live foreground runner remained to consume the attention resolution.",
                    checkpoint.work_id,
                    checkpoint
                        .suspend_requested_by
                        .as_ref()
                        .map(|reference| reference.event_id.as_str())
                        .unwrap_or("unknown")
                ),
            );
        }
        persistence
            .record_turn_closed(&restored.history, &turn, active_snapshot.schema_rejections)
            .map_err(|error| {
                UiCommandDispatchPortError::DispatchFailed(format!(
                    "failed to persist stale Master active work recovery for `{}`: {error}",
                    checkpoint.work_id
                ))
            })?;
        master_runner::clear_master_active_work_if_current(
            &live.runtime_home,
            &state.config.reason_agent_id,
            &checkpoint.logical_turn_id,
        )
        .map_err(UiCommandDispatchPortError::DispatchFailed)?;

        let restored_turns = persistence
            .restore_turn_snapshots_for_ui(&checkpoint.session_id)
            .map_err(|error| {
                UiCommandDispatchPortError::DispatchFailed(format!(
                    "failed to project stale Master active work recovery for `{}`: {error}",
                    checkpoint.work_id
                ))
            })?;
        state
            .turns
            .retain(|existing| existing.request.session_id != checkpoint.session_id);
        state.turns.extend(restored_turns);
        state
            .turns
            .sort_by_key(|existing| runtime_turn_position(&existing.request.turn_id));
        state.next_turn_ordinal = state
            .next_turn_ordinal
            .max(runtime_turn_position(&turn.request.turn_id).0);
        state.session_cwds = session_cwds_from_turns(&state.turns);
        let current_turn =
            current_runtime_turn_for_projection(&state.turns, &turn.request.turn_id)?;
        let cwd = state
            .session_cwds
            .get(&checkpoint.session_id)
            .map(|path| path.to_string_lossy().into_owned())
            .or_else(|| current_turn.cwd.clone());
        let projection = project_runtime_turn_history(
            &state.config.reason_agent_id,
            &state.config.master_node_id,
            std::slice::from_ref(&current_turn),
            cwd,
        );
        self.ui_state
            .lock()
            .expect("lock ui state")
            .apply_turn_projection(projection);
        Ok(())
    }

    fn dispatch_cancel_turn(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        turn_id: TurnId,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        if let Some(active) = state
            .active_turns
            .iter()
            .find(|active| active.turn_id == turn_id)
            .cloned()
        {
            active.cancel_token.store(true, Ordering::SeqCst);
            if let Some(live) = state.config.live.clone() {
                let prepared = PreparedLiveSubmit {
                    live,
                    reason_agent_id: state.config.reason_agent_id.clone(),
                    master_node_id: state.config.master_node_id.clone(),
                    session_id: active.session_id.clone(),
                    cwd: active.cwd.clone(),
                    turn_id: active.turn_id.clone(),
                    trace_id: active.trace_id.clone(),
                    prompt: active.user_text.clone(),
                    attachments: Vec::new(),
                    attachment_metadata: Vec::new(),
                    cancel_token: Arc::clone(&active.cancel_token),
                };
                let current_turn = restore_or_materialize_cancelled_live_submit(
                    state,
                    &prepared,
                    "cancelled by ui command",
                )?;
                let clear_result = master_runner::clear_master_active_work_if_current(
                    &prepared.live.runtime_home,
                    &prepared.reason_agent_id,
                    &prepared.turn_id,
                );
                remove_active_turn(&mut state.active_turns, &prepared.turn_id);
                let projection = project_runtime_turn_history(
                    &state.config.reason_agent_id,
                    &state.config.master_node_id,
                    std::slice::from_ref(&current_turn),
                    Some(prepared.cwd.to_string_lossy().into_owned()),
                );
                self.ui_state
                    .lock()
                    .expect("lock ui state")
                    .apply_turn_projection(projection);
                if let Err(error) = clear_result {
                    return Err(UiCommandDispatchPortError::DispatchFailed(error));
                }
            } else {
                publish_live_cancelled_projection(
                    &self.ui_state,
                    &state.config.reason_agent_id,
                    &state.config.master_node_id,
                    &active,
                );
            }
            return Ok(UiCommandDispatchReceipt {
                ingress: envelope.ingress,
                target_feature_id: envelope.target_feature_id,
                target_owner_module: envelope.target_owner_module,
                dispatch_status: "reason_live_turn_cancel_requested".to_owned(),
            });
        }

        let turn = state
            .turns
            .iter_mut()
            .find(|turn| turn.request.turn_id == turn_id)
            .ok_or_else(|| {
                UiCommandDispatchPortError::TargetNotFound(turn_id.as_str().to_owned())
            })?;

        state
            .reason_engine
            .cancel_turn(turn, "cancelled by ui command");
        let cwd = state
            .session_cwds
            .get(&turn.request.session_id)
            .map(|path| path.to_string_lossy().into_owned());
        let projection = project_runtime_turn(
            &state.config.reason_agent_id,
            &state.config.master_node_id,
            turn,
            cwd,
        );
        self.ui_state
            .lock()
            .expect("lock ui state")
            .apply_turn_projection(projection);

        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "reason_turn_cancelled".to_owned(),
        })
    }

    fn dispatch_cancel_latest_active_turn(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        if let Some(active) = state.active_turns.last().cloned() {
            return self.dispatch_cancel_turn(state, envelope, active.turn_id);
        }
        let turn_id = state
            .turns
            .last()
            .map(|turn| turn.request.turn_id.clone())
            .ok_or_else(|| {
                UiCommandDispatchPortError::TargetNotFound("latest-active-turn".to_owned())
            })?;
        self.dispatch_cancel_turn(state, envelope, turn_id)
    }

    fn dispatch_resume_turn(
        &self,
        envelope: UiCommandDispatchEnvelope,
        turn_id: TurnId,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let _ = envelope;
        Err(UiCommandDispatchPortError::Unsupported(format!(
            "resume dispatch for `{}` is not implemented",
            turn_id.as_str()
        )))
    }

    fn dispatch_session_management(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let live = state.config.live.as_ref().ok_or_else(|| {
            UiCommandDispatchPortError::Unsupported(
                "session management requires a live runtime home".to_owned(),
            )
        })?;
        let persistence = ReasonPersistence::new(
            live.runtime_home.clone(),
            state.config.reason_agent_id.clone(),
        );
        let metadata = match envelope.command.clone() {
            UiCommand::CreateSession {
                session_id,
                title,
                cwd,
            } => persistence.create_session_metadata(session_id, title, cwd),
            UiCommand::RenameSession { session_id, title } => {
                persistence.rename_session(&session_id, title)
            }
            UiCommand::ArchiveSession { session_id } => persistence.archive_session(&session_id),
            UiCommand::RestoreSession { session_id } => persistence.restore_session(&session_id),
            UiCommand::DeleteSession { session_id } => persistence.delete_session(&session_id),
            UiCommand::RollbackLatestSessionTurn { session_id } => {
                let marker = persistence
                    .rollback_latest_session_turn(&session_id)
                    .map_err(map_session_metadata_dispatch_error)?;
                cancel_tasks_for_session_rollback(
                    &live.runtime_home,
                    &state.config.reason_agent_id,
                    &marker,
                )?;
                let effective_turns = persistence
                    .restore_turn_snapshots_for_ui(&session_id)
                    .map_err(map_session_metadata_dispatch_error)?;
                state
                    .turns
                    .retain(|turn| turn.request.session_id != session_id);
                state.turns.extend(effective_turns.clone());
                state
                    .turns
                    .sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
                state.session_cwds = session_cwds_from_turns(&state.turns);
                let projections = effective_turns
                    .iter()
                    .map(|turn| {
                        let cwd = state
                            .session_cwds
                            .get(&turn.request.session_id)
                            .map(|path| path.to_string_lossy().into_owned())
                            .or_else(|| turn.cwd.clone());
                        project_runtime_turn(
                            &state.config.reason_agent_id,
                            &state.config.master_node_id,
                            turn,
                            cwd,
                        )
                    })
                    .collect::<Vec<_>>();
                self.ui_state
                    .lock()
                    .expect("lock ui state")
                    .replace_session_turn_projections(&session_id, projections);
                return Ok(UiCommandDispatchReceipt {
                    ingress: envelope.ingress,
                    target_feature_id: envelope.target_feature_id,
                    target_owner_module: envelope.target_owner_module,
                    dispatch_status: format!(
                        "session_turn_rolled_back:{}",
                        marker.target_turn_id.as_str()
                    ),
                });
            }
            _ => {
                return Err(UiCommandDispatchPortError::Unsupported(
                    "command is not a session management target".to_owned(),
                ));
            }
        }
        .map_err(map_session_metadata_dispatch_error)?;
        if let Some(cwd) = metadata.cwd.as_ref()
            && let Ok(path) = fs::canonicalize(cwd)
        {
            state.session_cwds.insert(metadata.session_id.clone(), path);
        }
        self.ui_state
            .lock()
            .expect("lock ui state")
            .set_session_metadata(session_metadata_to_ui(metadata));
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "session_metadata_updated".to_owned(),
        })
    }

    fn dispatch_direct_message(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        node_id: String,
        text: String,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        if node_id != state.config.slave_node_id {
            return Err(UiCommandDispatchPortError::TargetNotFound(node_id));
        }
        state
            .node_runtime
            .send_direct_message(&state.config.master_node_id, &text)
            .map_err(map_node_dispatch_error)?;

        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "node_direct_message_dispatched".to_owned(),
        })
    }

    fn dispatch_rewind_checkpoint(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        checkpoint_id: String,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let live = state.config.live.as_ref().ok_or_else(|| {
            UiCommandDispatchPortError::Unsupported(
                "rewind dispatch requires a live runtime home".to_owned(),
            )
        })?;
        rewind_checkpoint(
            &live.runtime_home,
            &state.config.reason_agent_id,
            &state.config.session_id,
            &checkpoint_id,
        )
        .map_err(map_checkpoint_dispatch_error)?;
        self.refresh_checkpoint_projection_from_config(&state.config)
            .map_err(map_checkpoint_dispatch_error)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!("runtime_checkpoint_rewound checkpoint_id={checkpoint_id}"),
        })
    }

    fn refresh_checkpoint_projection(&self) -> Result<(), RuntimeCheckpointError> {
        let state = self.state.lock().expect("lock runtime dispatcher state");
        self.refresh_checkpoint_projection_from_config(&state.config)
    }

    fn refresh_checkpoint_projection_from_config(
        &self,
        config: &RuntimeCommandDispatcherConfig,
    ) -> Result<(), RuntimeCheckpointError> {
        let Some(live) = &config.live else {
            return Ok(());
        };
        let summaries = list_checkpoints(
            &live.runtime_home,
            &config.reason_agent_id,
            &config.session_id,
        )?;
        let snapshot = checkpoint_projection_from_runtime_summary(
            config.reason_agent_id.clone(),
            config.master_node_id.clone(),
            summaries
                .into_iter()
                .map(checkpoint_summary_to_ui)
                .collect(),
        );
        self.ui_state
            .lock()
            .expect("lock ui state")
            .set_checkpoint_snapshot(snapshot);
        Ok(())
    }
}

fn submit_attachment_inputs(
    metadata: Option<UiSubmitMetadata>,
) -> (Vec<ProviderInputAttachment>, Vec<InputAttachmentMetadata>) {
    let Some(metadata) = metadata else {
        return (Vec::new(), Vec::new());
    };
    let mut provider_inputs = Vec::new();
    let mut session_metadata = Vec::new();
    for attachment in metadata.attachments {
        let kind = match attachment.kind {
            UiInputAttachmentKind::Image => (
                ProviderInputAttachmentKind::Image,
                InputAttachmentKind::Image,
            ),
        };
        let size_bytes = attachment.size_bytes;
        session_metadata.push(InputAttachmentMetadata {
            attachment_id: attachment.attachment_id.clone(),
            kind: kind.1,
            media_type: attachment.media_type.clone(),
            name: attachment.name.clone(),
            size_bytes,
        });
        if let Some(data_base64) = attachment.data_base64 {
            provider_inputs.push(ProviderInputAttachment {
                attachment_id: attachment.attachment_id,
                kind: kind.0,
                media_type: attachment.media_type,
                name: attachment.name,
                size_bytes,
                data_base64,
            });
        }
    }
    (provider_inputs, session_metadata)
}

fn resolve_session_cwd(
    state: &mut RuntimeCommandDispatcherState,
    session_id: &SessionId,
    requested_cwd: Option<String>,
    default_root: Option<&Path>,
) -> Result<PathBuf, UiCommandDispatchPortError> {
    let cwd = if let Some(cwd) = requested_cwd {
        canonicalize_session_cwd(&cwd)?
    } else if let Some(existing) = state.session_cwds.get(session_id) {
        existing.clone()
    } else {
        canonicalize_default_runtime_cwd(default_root)?
    };
    state.session_cwds.insert(session_id.clone(), cwd.clone());
    Ok(cwd)
}

fn cancel_tasks_for_session_rollback(
    runtime_home: &Path,
    agent_id: &AgentId,
    marker: &SessionRollbackMarker,
) -> Result<(), UiCommandDispatchPortError> {
    let runtime =
        TaskRuntime::boot(runtime_home, agent_id.clone()).map_err(map_task_query_error)?;
    let board = runtime
        .query_task_board(TaskBoardQuery {
            status: None,
            assignee: None,
            include_terminal: true,
        })
        .map_err(map_task_query_error)?;
    for task in board.tasks.into_iter().filter(|task| {
        task.parent.session_id.as_ref() == Some(&marker.session_id)
            && task.parent.turn_id.as_ref().is_some_and(|turn_id| {
                rollback_turns_share_logical_group(
                    turn_id,
                    &marker.target_turn_id,
                    &marker.target_logical_turn_key,
                )
            })
            && !matches!(
                task.status,
                TaskStatus::Closed | TaskStatus::Cancelled | TaskStatus::Failed
            )
    }) {
        runtime
            .cancel_task(TaskMutationRequest {
                task_id: task.task_id.clone(),
                actor: ui_task_actor(
                    agent_id,
                    Some(marker.session_id.clone()),
                    Some(marker.target_turn_id.clone()),
                ),
                watermark: ui_task_watermark("rollback_cancel_child_task"),
            })
            .map_err(map_task_query_error)?;
    }
    Ok(())
}

fn rollback_turns_share_logical_group(
    task_turn_id: &TurnId,
    target_turn_id: &TurnId,
    target_logical_turn_key: &str,
) -> bool {
    if task_turn_id.as_str() == target_turn_id.as_str() {
        return true;
    }
    let (task_ordinal, _, task_raw) = runtime_turn_position(task_turn_id);
    if task_ordinal == 0 {
        task_raw == target_logical_turn_key
    } else {
        format!("runtime-turn-{task_ordinal}") == target_logical_turn_key
    }
}

fn session_cwds_from_turns(turns: &[TurnRecord]) -> BTreeMap<SessionId, PathBuf> {
    let mut cwds = BTreeMap::new();
    for turn in turns {
        if let Some(cwd) = &turn.cwd
            && let Ok(path) = fs::canonicalize(cwd)
        {
            cwds.insert(turn.request.session_id.clone(), path);
        }
    }
    cwds
}

fn project_live_config_status_for_ui(
    live: &RuntimeLiveDispatcherConfig,
) -> Result<UiConfigStatusProjection, UiCommandDispatchPortError> {
    let config_path = live.runtime_home.join("config.toml");
    project_config_status_from_path_for_ui(&config_path, &live.selected_agent.name)
}

fn project_config_status_from_path_for_ui(
    config_path: &Path,
    agent_name: &str,
) -> Result<UiConfigStatusProjection, UiCommandDispatchPortError> {
    let loaded = load_config_from_path(config_path)
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    let selected = loaded
        .select_agent(agent_name)
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    Ok(project_config_status_for_ui(&selected, Some(&loaded)))
}

fn project_config_status_for_ui(
    selected: &SelectedAgentConfig,
    loaded_config: Option<&LoadedConfig>,
) -> UiConfigStatusProjection {
    let worker_peers = selected.worker_peers().collect::<Vec<_>>();
    let shared_provider_id = worker_peers.first().and_then(|first| {
        worker_peers
            .iter()
            .all(|peer| peer.provider_id == first.provider_id)
            .then(|| first.provider_id.clone())
    });
    let provider_registry = loaded_config
        .map(|loaded| {
            loaded
                .safe_provider_registry()
                .into_iter()
                .map(|provider| {
                    let (effective, reason) = provider_web_search_effective_status(
                        &provider.id,
                        provider.provider_type,
                        provider.protocol,
                        provider.web_search,
                    );
                    UiProviderConfigSummaryProjection {
                        provider_id: provider.id,
                        enabled: provider.enabled,
                        provider_type: provider.provider_type.as_str().to_owned(),
                        provider_protocol: provider.protocol.as_str().to_owned(),
                        provider_base_url: provider.base_url,
                        provider_base_url_host: provider.base_url_host,
                        default_model: provider.default_model,
                        provider_web_search: provider.web_search.as_str().to_owned(),
                        provider_web_search_effective: effective,
                        provider_web_search_reason: reason,
                        provider_auth_type: provider.auth_type.as_str().to_owned(),
                        provider_auth_source: provider.auth_source.as_str().to_owned(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let model_group_registry = loaded_config
        .map(|loaded| {
            loaded
                .safe_model_group_registry()
                .into_iter()
                .map(|group| UiModelGroupConfigProjection {
                    group_id: group.id,
                    enabled: group.enabled,
                    label: group.label,
                    primary: ui_model_route_from_config(group.primary),
                    sub: group.sub.map(ui_model_route_from_config),
                    search: group.search.map(ui_model_route_from_config),
                    title: group.title.map(ui_model_route_from_config),
                    fallback: group.fallback.map(ui_model_route_from_config),
                    load_balance: group
                        .load_balance
                        .into_iter()
                        .map(ui_model_weighted_route_from_config)
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();
    let (selected_web_search_effective, selected_web_search_reason) =
        provider_web_search_effective_status(
            &selected.provider.id,
            selected.provider.provider_type,
            selected.provider.protocol,
            selected.provider.web_search,
        );
    UiConfigStatusProjection {
        agent_name: selected.name.clone(),
        agent_mode: selected.mode.as_str().to_owned(),
        node_id: selected.node_id.clone(),
        paired_agents: selected
            .paired_agents
            .iter()
            .map(|peer| UiConfigPeerProjection {
                agent_name: peer.name.clone(),
                agent_mode: peer.mode.as_str().to_owned(),
                node_id: peer.node_id.clone(),
                provider_id: peer.provider_id.clone(),
                fallback_provider_id: peer.fallback_provider_id.clone(),
                model_group_id: peer.model_group_id.clone(),
            })
            .collect(),
        provider_registry,
        model_group_registry,
        agent_resource_count: worker_peers.len(),
        agent_resource_limit: MAX_AGENT_RESOURCE_COUNT,
        agent_resource_provider_mode: if worker_peers.is_empty() {
            "not_applicable"
        } else if shared_provider_id.is_some() {
            "shared"
        } else {
            "per_agent"
        }
        .to_owned(),
        agent_resource_provider_id: shared_provider_id,
        provider_id: selected.provider.id.clone(),
        fallback_provider_id: selected
            .fallback_provider
            .as_ref()
            .map(|provider| provider.id.clone()),
        model_group_id: selected.model_group_id.clone(),
        provider_type: selected.provider.provider_type.as_str().to_owned(),
        provider_protocol: selected.provider.protocol.as_str().to_owned(),
        provider_base_url: safe_provider_base_url_for_projection(&selected.provider.base_url),
        provider_base_url_host: provider_base_url_host_for_projection(&selected.provider.base_url),
        default_model: selected.provider.default_model.clone(),
        provider_web_search: selected.provider.web_search.as_str().to_owned(),
        provider_web_search_effective: selected_web_search_effective,
        provider_web_search_reason: selected_web_search_reason,
        provider_web_search_route_summary: provider_web_search_route_summary(selected),
        provider_auth_type: selected.provider.auth_type.as_str().to_owned(),
        provider_auth_source: selected.provider.auth_source.as_str().to_owned(),
        restart_required_on_change: selected.restart_required_on_change,
    }
}

fn ui_model_route_from_config(route: ModelRouteConfig) -> UiModelRouteProjection {
    UiModelRouteProjection {
        provider_id: route.provider_id,
        model: route.model,
    }
}

fn ui_model_weighted_route_from_config(
    route: ModelWeightedRouteConfig,
) -> UiModelWeightedRouteProjection {
    UiModelWeightedRouteProjection {
        provider_id: route.provider_id,
        model: route.model,
        weight: route.weight,
    }
}

fn model_route_config_from_ui(route: UiModelRouteUpdate) -> ModelRouteConfig {
    ModelRouteConfig {
        provider_id: route.provider_id,
        model: route.model,
    }
}

fn model_weighted_route_config_from_ui(
    route: UiModelWeightedRouteUpdate,
) -> ModelWeightedRouteConfig {
    ModelWeightedRouteConfig {
        provider_id: route.provider_id,
        model: route.model,
        weight: route.weight,
    }
}

fn provider_web_search_effective_status(
    provider_id: &str,
    provider_type: ProviderType,
    protocol: ConfigProviderProtocol,
    mode: ProviderWebSearchMode,
) -> (String, String) {
    if mode == ProviderWebSearchMode::Disabled {
        return (
            "disabled".to_owned(),
            format!("provider `{provider_id}` has web_search=disabled"),
        );
    }
    match provider_web_search_capability_from_parts(provider_type, protocol, mode) {
        ProviderWebSearchCapability::Hosted { .. } => (
            "hosted_declared".to_owned(),
            format!(
                "provider `{provider_id}` declares provider-hosted web_search through `{}/{}` and can be live-tested from Settings",
                provider_type.as_str(),
                protocol.as_str()
            ),
        ),
        ProviderWebSearchCapability::Unsupported => (
            "protocol_unsupported".to_owned(),
            format!(
                "provider `{provider_id}` has web_search={} but `{}/{}` does not expose provider-hosted web_search",
                mode.as_str(),
                provider_type.as_str(),
                protocol.as_str()
            ),
        ),
    }
}

fn provider_web_search_route_summary(selected: &SelectedAgentConfig) -> String {
    let descriptor = match provider_descriptor(&selected.provider) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            return format!("selected provider descriptor failed: {error}");
        }
    };
    provider_web_search_route_guidance(selected, &descriptor)
}

fn execute_provider_web_search_test(
    selected: &SelectedAgentConfig,
    provider: SelectedProviderConfig,
    query: Option<&str>,
) -> Result<String, UiCommandDispatchPortError> {
    let descriptor = provider_descriptor(&provider)
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    if !descriptor.capabilities.web_search.is_hosted() {
        return Err(UiCommandDispatchPortError::Unsupported(format!(
            "provider `{}` protocol `{}` has web_search={} and does not declare provider-hosted web_search",
            provider.id,
            provider.protocol.as_str(),
            provider.web_search.as_str()
        )));
    }
    let test_query = query
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_PROVIDER_WEB_SEARCH_TEST_QUERY);
    let stamp = now_unix_seconds();
    let session_id = SessionId::new(format!("provider-web-search-test-{stamp}"));
    let turn_id = TurnId::new("provider-web-search-test-turn");
    let trace_id = TraceId::new(format!("provider-web-search-test-trace-{stamp}"));
    let payload = ReasonReq03ProviderPayload {
        session_id: session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id: trace_id.clone(),
        feature_id: FeatureId::new("provider.reason-live-bridge"),
        agent_id: AgentId::new(selected.name.clone()),
        model: provider.default_model.clone(),
        input_segments: vec![
            ContextSegment {
                segment_id: ContextSegmentId::new("provider-web-search-test-instructions"),
                kind: ContextSegmentKind::DeveloperPolicy,
                stability: ContextStability::TurnVolatile,
                cache_policy: ContextCachePolicy::NoCache,
                role: ContextRole::System,
                content:
                    "This is a provider capability test. Use provider-hosted web_search now; do not answer from memory."
                        .to_owned(),
                token_budget: 96,
                provenance: ContextProvenance {
                    source: "provider_web_search_test".to_owned(),
                    reference: None,
                },
            },
            ContextSegment {
                segment_id: ContextSegmentId::new("provider-web-search-test-query"),
                kind: ContextSegmentKind::UserTurnInput,
                stability: ContextStability::TurnVolatile,
                cache_policy: ContextCachePolicy::NoCache,
                role: ContextRole::User,
                content: format!(
                    "Run a web search for this exact query and answer in one short sentence: {test_query}"
                ),
                token_budget: 128,
                provenance: ContextProvenance {
                    source: "provider_web_search_test".to_owned(),
                    reference: Some(provider.id.clone()),
                },
            },
        ],
    };
    let mut semantic_request = build_semantic_request(descriptor.clone(), payload, false)
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    semantic_request.hosted_tools = vec![ProviderHostedToolDefinition::WebSearch {
        mode: SemanticWebSearchMode::Live,
        external_web_access: true,
    }];
    semantic_request.tools = Vec::new();
    semantic_request.tool_choice = Some(match descriptor.protocol {
        ProviderProtocol::AnthropicMessages => ProviderToolChoice::Required {
            name: "web_search".to_owned(),
        },
        _ => ProviderToolChoice::Auto,
    });

    let mut driver = build_live_provider_driver(&provider)
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    let ctx = ProviderEventContext {
        agent_id: AgentId::new(selected.name.clone()),
        session_id,
        turn_id,
        trace_id,
        feature_id: FeatureId::new("provider.reason-live-bridge"),
    };
    let outputs = driver
        .execute_once_with_raw(&ctx, &semantic_request, &mut |_| Ok(()))
        .map_err(|err| {
            UiCommandDispatchPortError::DispatchFailed(format!(
                "provider web_search test failed for `{}`: {}",
                provider.id,
                public_error_center_message(&err.info().terminal_message())
            ))
        })?;
    if !provider_outputs_have_hosted_web_search(&outputs) {
        return Err(UiCommandDispatchPortError::DispatchFailed(format!(
            "provider web_search test did not observe provider-hosted web_search for `{}`; semantic_outputs={} observed_outputs={}",
            provider.id,
            outputs.len(),
            provider_semantic_outputs_summary(&outputs)
        )));
    }
    Ok(format!(
        "provider_web_search_test_passed:provider={}:protocol={}:model={}:hosted_tool=web_search:hosted_observed=true:semantic_outputs={}",
        sanitize_identifier(&provider.id),
        provider.protocol.as_str(),
        sanitize_identifier(&provider.default_model),
        outputs.len()
    ))
}

fn provider_outputs_have_hosted_web_search(outputs: &[ProviderSemanticOutput]) -> bool {
    outputs.iter().any(|output| {
        matches!(
            output,
            ProviderSemanticOutput::SemanticEvent(event)
                if event.content.contains("provider-hosted web_search")
        )
    })
}

fn provider_semantic_outputs_summary(outputs: &[ProviderSemanticOutput]) -> String {
    if outputs.is_empty() {
        return "none".to_owned();
    }
    let mut parts = outputs
        .iter()
        .take(6)
        .map(|output| match output {
            ProviderSemanticOutput::SemanticEvent(event) => format!(
                "semantic:{:?}:{}",
                event.kind,
                compact_status_fragment(&event.content, 160)
            ),
            ProviderSemanticOutput::ToolCall(tool_call) => format!(
                "tool_call:{}",
                compact_status_fragment(&tool_call.tool_call.tool_name, 80)
            ),
            ProviderSemanticOutput::ToolResultReentry(tool_result) => format!(
                "tool_result:{:?}:{}",
                tool_result.tool_result.status,
                compact_status_fragment(&tool_result.tool_result.output, 120)
            ),
            ProviderSemanticOutput::Usage(usage) => format!(
                "usage:finish={}:total={}",
                usage.usage.finish_reason.as_deref().unwrap_or("unknown"),
                usage.usage.resolved_total_tokens()
            ),
            ProviderSemanticOutput::Terminal(terminal) => format!(
                "terminal:{:?}:{}",
                terminal.status,
                compact_status_fragment(&terminal.summary, 120)
            ),
            ProviderSemanticOutput::Error(error) => format!(
                "error:{}:{}",
                compact_status_fragment(&error.error.code, 80),
                compact_status_fragment(&error.error.message, 120)
            ),
        })
        .collect::<Vec<_>>();
    if outputs.len() > 6 {
        parts.push(format!("+{} more", outputs.len() - 6));
    }
    parts.join(";")
}

enum ControlStatusHookOutcome {
    Absent,
    Accepted(ControlRhythmDecision),
    Rejected {
        rejection: ControlStatusRejection,
        feedback: String,
    },
}

fn run_control_status_stop_hook(
    center: &Arc<Mutex<MetadataCenter>>,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn: &TurnRecord,
    provider_text: &str,
) -> Result<ControlStatusHookOutcome, RuntimeLiveBridgeError> {
    if !provider_text.contains("<<<freehand_status>>>") {
        return Ok(ControlStatusHookOutcome::Absent);
    }
    let raw_hash = stable_debug_hash(provider_text);
    match parse_control_status_block(provider_text) {
        Ok(submission) => match control_status_rhythm_decision(&submission) {
            Ok(decision) => {
                record_control_status_metadata(
                    center,
                    agent_id,
                    session_id,
                    turn,
                    &submission,
                    &decision,
                    raw_hash,
                )?;
                Ok(ControlStatusHookOutcome::Accepted(decision))
            }
            Err(rejection) => {
                record_control_status_rejection_metadata(
                    center, agent_id, session_id, turn, &rejection, raw_hash,
                )?;
                Ok(ControlStatusHookOutcome::Rejected {
                    feedback: control_status_rejection_feedback(&rejection),
                    rejection,
                })
            }
        },
        Err(rejection) => {
            record_control_status_rejection_metadata(
                center, agent_id, session_id, turn, &rejection, raw_hash,
            )?;
            Ok(ControlStatusHookOutcome::Rejected {
                feedback: control_status_rejection_feedback(&rejection),
                rejection,
            })
        }
    }
}

fn completion_rejection_from_control_status(
    rejection: &ControlStatusRejection,
) -> CompletionSchemaRejection {
    CompletionSchemaRejection {
        issues: rejection
            .issues
            .iter()
            .map(|issue| CompletionSchemaIssue {
                field: format!("status.{}", issue.field),
                message: issue.message.clone(),
            })
            .collect(),
    }
}

fn record_control_status_metadata(
    center: &Arc<Mutex<MetadataCenter>>,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn: &TurnRecord,
    submission: &ControlStatusSubmission,
    decision: &ControlRhythmDecision,
    raw_hash: u64,
) -> Result<(), RuntimeLiveBridgeError> {
    write_control_hook_metadata(
        center,
        agent_id,
        session_id,
        RuntimeControlHookWriteSpec {
            turn_id: Some(&turn.request.turn_id),
            trace_id: &turn.request.trace_id,
            pipeline_node: "ControlHook03AfterModelResponse",
            metadata_suffix: "after_model_response:status_accepted".to_owned(),
            symbol_path: "run_live_provider_reason_turn",
            entries: vec![
                MetadataEntry {
                    key: "control.hook".to_owned(),
                    value: json!("ControlHook03AfterModelResponse"),
                },
                MetadataEntry {
                    key: "control.status_schema_version".to_owned(),
                    value: json!(submission.schema_version),
                },
                MetadataEntry {
                    key: "control.status_validation".to_owned(),
                    value: json!("accepted"),
                },
                MetadataEntry {
                    key: "control.decision".to_owned(),
                    value: json!(control_decision_label(decision)),
                },
                MetadataEntry {
                    key: "control.raw_hash".to_owned(),
                    value: json!(raw_hash),
                },
                MetadataEntry {
                    key: "control.block_hash".to_owned(),
                    value: json!(stable_debug_hash(
                        &serde_json::to_string(submission).unwrap_or_default()
                    )),
                },
            ],
        },
    )
}

fn record_control_status_rejection_metadata(
    center: &Arc<Mutex<MetadataCenter>>,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn: &TurnRecord,
    rejection: &ControlStatusRejection,
    raw_hash: u64,
) -> Result<(), RuntimeLiveBridgeError> {
    write_control_hook_metadata(
        center,
        agent_id,
        session_id,
        RuntimeControlHookWriteSpec {
            turn_id: Some(&turn.request.turn_id),
            trace_id: &turn.request.trace_id,
            pipeline_node: "ControlHook03AfterModelResponse",
            metadata_suffix: "after_model_response:status_rejected".to_owned(),
            symbol_path: "run_live_provider_reason_turn",
            entries: vec![
                MetadataEntry {
                    key: "control.hook".to_owned(),
                    value: json!("ControlHook03AfterModelResponse"),
                },
                MetadataEntry {
                    key: "control.status_validation".to_owned(),
                    value: json!("rejected"),
                },
                MetadataEntry {
                    key: "control.raw_hash".to_owned(),
                    value: json!(raw_hash),
                },
                MetadataEntry {
                    key: "control.issue_count".to_owned(),
                    value: json!(rejection.issues.len()),
                },
                MetadataEntry {
                    key: "control.issue_summary".to_owned(),
                    value: json!(control_status_rejection_summary(rejection)),
                },
            ],
        },
    )
}

fn control_status_terminal_summary(
    decision: &ControlRhythmDecision,
    provider_text: &str,
    public_provider_text: &str,
) -> String {
    match decision {
        ControlRhythmDecision::AllowNaturalStop => {
            let summary = public_provider_text.trim();
            if summary.is_empty() {
                "Summary: simple request completed by accepted control status".to_owned()
            } else {
                format!("Summary: {summary}")
            }
        }
        ControlRhythmDecision::AllowTaskCompletion => parse_control_status_block(provider_text)
            .ok()
            .and_then(|submission| submission.status.summary.or(submission.status.evidence))
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("Summary: {}", value.trim()))
            .unwrap_or_else(|| "Summary: task completed by accepted control status".to_owned()),
        ControlRhythmDecision::StopForUserOptions(options) => {
            format!("Summary: user options available: {}", options.join(", "))
        }
        ControlRhythmDecision::ContinueWithNextStep(next_step) => {
            format!("Summary: continuing with next step: {next_step}")
        }
        ControlRhythmDecision::StopBlocked(blocked_reason) => {
            format!("Blocked reason: {blocked_reason}")
        }
    }
}

fn control_decision_label(decision: &ControlRhythmDecision) -> &'static str {
    match decision {
        ControlRhythmDecision::AllowNaturalStop => "allow_natural_stop",
        ControlRhythmDecision::AllowTaskCompletion => "allow_task_completion",
        ControlRhythmDecision::ContinueWithNextStep(_) => "continue_with_next_step",
        ControlRhythmDecision::StopBlocked(_) => "stop_blocked",
        ControlRhythmDecision::StopForUserOptions(_) => "stop_for_user_options",
    }
}

fn control_status_rejection_summary(rejection: &ControlStatusRejection) -> String {
    rejection
        .issues
        .iter()
        .map(|issue| format!("{}: {}", issue.field, issue.message))
        .collect::<Vec<_>>()
        .join("; ")
}

fn stable_debug_hash(input: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}

fn canonicalize_session_cwd(cwd: &str) -> Result<PathBuf, UiCommandDispatchPortError> {
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return Err(UiCommandDispatchPortError::DispatchFailed(
            "session cwd must be non-empty".to_owned(),
        ));
    }
    fs::canonicalize(trimmed).map_err(|err| {
        UiCommandDispatchPortError::TargetNotFound(format!(
            "session cwd `{trimmed}` is not accessible: {err}"
        ))
    })
}

fn canonicalize_default_runtime_cwd(
    default_root: Option<&Path>,
) -> Result<PathBuf, UiCommandDispatchPortError> {
    let root = if let Some(default_root) = default_root {
        default_root.to_path_buf()
    } else {
        env::var_os("FREEHAND_WORKSPACE_ROOT")
            .or_else(|| env::var_os("FREEHAND_DAEMON_WORKDIR"))
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(|| {
                env::current_dir().map_err(|err| {
                    UiCommandDispatchPortError::DispatchFailed(format!(
                        "cannot read runtime current working directory: {err}"
                    ))
                })
            })?
    };
    fs::canonicalize(&root).map_err(|err| {
        UiCommandDispatchPortError::DispatchFailed(format!(
            "cannot canonicalize runtime workspace `{}`: {err}",
            root.display()
        ))
    })
}

impl UiCommandDispatchPort for RuntimeCommandDispatcher {
    fn dispatch(
        &self,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        if let UiCommand::SubmitUserInput {
            text,
            session_id,
            cwd,
            metadata,
        } = envelope.command.clone()
        {
            let prepared = {
                let mut state = self.state.lock().expect("lock runtime dispatcher state");
                self.prepare_live_submit_user_input(
                    &mut state,
                    text.clone(),
                    session_id.clone(),
                    cwd.clone(),
                    metadata.clone(),
                )
            }?;
            if let Some(prepared) = prepared {
                return self.dispatch_prepared_live_submit(envelope, prepared);
            }
            let mut state = self.state.lock().expect("lock runtime dispatcher state");
            return self
                .dispatch_submit_user_input(&mut state, envelope, text, session_id, cwd, metadata);
        }

        if let UiCommand::TestProviderWebSearch { .. } = envelope.command.clone() {
            let (runtime_home, agent_name) = {
                let state = self.state.lock().expect("lock runtime dispatcher state");
                let live = state.config.live.as_ref().ok_or_else(|| {
                    UiCommandDispatchPortError::Unsupported(
                        "provider web_search test requires a live runtime home".to_owned(),
                    )
                })?;
                (live.runtime_home.clone(), live.selected_agent.name.clone())
            };
            return self.dispatch_test_provider_web_search(envelope, runtime_home, agent_name);
        }

        let mut state = self.state.lock().expect("lock runtime dispatcher state");
        match envelope.command.clone() {
            UiCommand::CreateSession { .. }
            | UiCommand::RenameSession { .. }
            | UiCommand::ArchiveSession { .. }
            | UiCommand::RestoreSession { .. }
            | UiCommand::DeleteSession { .. }
            | UiCommand::RollbackLatestSessionTurn { .. } => {
                self.dispatch_session_management(&mut state, envelope)
            }
            UiCommand::CancelTurn { turn_id } => {
                self.dispatch_cancel_turn(&mut state, envelope, turn_id)
            }
            UiCommand::CancelLatestActiveTurn {} => {
                self.dispatch_cancel_latest_active_turn(&mut state, envelope)
            }
            UiCommand::UpdateProviderConfig { update } => {
                self.dispatch_update_provider_config(&mut state, envelope, update)
            }
            UiCommand::UpsertProviderConfig { update } => {
                self.dispatch_upsert_provider_config(&mut state, envelope, update)
            }
            UiCommand::UpsertModelGroupConfig { group } => {
                self.dispatch_upsert_model_group_config(&mut state, envelope, group)
            }
            UiCommand::UpdateAgentModelGroupSelection { selection } => {
                self.dispatch_update_agent_model_group_selection(&mut state, envelope, selection)
            }
            UiCommand::UpdateAgentProviderSelection { selection } => {
                self.dispatch_update_agent_provider_selection(&mut state, envelope, selection)
            }
            UiCommand::UpdateAgentResourceConfig { update } => {
                self.dispatch_update_agent_resource_config(&mut state, envelope, update)
            }
            UiCommand::CreateTask { task } => self.dispatch_create_task(&mut state, envelope, task),
            UiCommand::CreateTaskAgent { agent } => {
                self.dispatch_create_task_agent(&mut state, envelope, agent)
            }
            UiCommand::AssignTask { assignment } => {
                self.dispatch_assign_task(&mut state, envelope, assignment)
            }
            UiCommand::ClaimNextTask { claim } => {
                self.dispatch_claim_next_task(&mut state, envelope, claim)
            }
            UiCommand::SubmitTaskReview { review } => {
                self.dispatch_submit_task_review(&mut state, envelope, review)
            }
            UiCommand::RejectTaskReview { rejection } => {
                self.dispatch_reject_task_review(&mut state, envelope, rejection)
            }
            UiCommand::ApproveTaskReview { task_id } => {
                self.dispatch_approve_task_review(&mut state, envelope, task_id)
            }
            UiCommand::CloseTask { task_id } => {
                self.dispatch_close_task(&mut state, envelope, task_id)
            }
            UiCommand::ApplyExecutionFact { fact } => {
                self.dispatch_apply_execution_fact(&mut state, envelope, fact)
            }
            UiCommand::RunSchedulerTick { tick } => {
                self.dispatch_run_scheduler_tick(&mut state, envelope, tick)
            }
            UiCommand::RunMasterPoll { .. } => self.dispatch_run_master_poll(&mut state, envelope),
            UiCommand::WorkerControl { control } => {
                self.dispatch_worker_control(&mut state, envelope, control)
            }
            UiCommand::ScheduleTimer { timer } => {
                self.dispatch_schedule_timer(&mut state, envelope, timer)
            }
            UiCommand::CancelTimer { timer_id } => {
                self.dispatch_cancel_timer(&mut state, envelope, timer_id)
            }
            UiCommand::ResumeTurn { turn_id } => self.dispatch_resume_turn(envelope, turn_id),
            UiCommand::SendDirectMessageToSlave { node_id, text } => {
                self.dispatch_direct_message(&mut state, envelope, node_id, text)
            }
            UiCommand::RewindCheckpoint { checkpoint_id } => {
                self.dispatch_rewind_checkpoint(&mut state, envelope, checkpoint_id)
            }
            _ => Err(UiCommandDispatchPortError::Unsupported(
                "command is not a runtime dispatch target".to_owned(),
            )),
        }
    }
}

impl RuntimeCommandDispatcher {
    fn dispatch_create_task(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        task: UiTaskCreateCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, agent_id) = task_runtime_target(state)?;
        let runtime =
            TaskRuntime::boot(&runtime_home, agent_id.clone()).map_err(map_task_query_error)?;
        let outcome = runtime
            .create_task(TaskCreateRequest {
                task_id: task.task_id.map(TaskId::new),
                title: task.title,
                content: task.content,
                goal: task.goal,
                deliverables: task.deliverables,
                acceptance: task.acceptance,
                priority: task.priority,
                target_cwd: task.target_cwd,
                execution_profile: parse_task_execution_profile_value(&task.execution_profile)
                    .map_err(|err| {
                        UiCommandDispatchPortError::DispatchFailed(format!(
                            "task execution profile is invalid: {err}"
                        ))
                    })?,
                dispatch: task_dispatch_from_ui(task.dispatch),
                parent: TaskParentRef {
                    session_id: task.session_id,
                    turn_id: task.turn_id,
                    trace_id: None,
                },
                actor: ui_task_actor(&agent_id, None, None),
                watermark: ui_task_watermark("create_task"),
            })
            .map_err(map_task_query_error)?;
        self.publish_task_list_from_runtime(&runtime_home, &agent_id)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!("task_created:{}", outcome.task.task_id.as_str()),
        })
    }

    fn dispatch_create_task_agent(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        agent: UiTaskAgentCreateCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, source_agent_id) = task_runtime_target(state)?;
        let runtime = TaskRuntime::boot(&runtime_home, source_agent_id.clone())
            .map_err(map_task_query_error)?;
        let outcome = runtime
            .create_agent(AgentCreateRequest {
                agent_id: agent.agent_id.clone(),
                capabilities: agent.capabilities,
                actor: ui_task_actor(&source_agent_id, None, None),
                watermark: ui_task_watermark("create_task_agent"),
            })
            .map_err(map_task_query_error)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!("task_agent_created:{}", outcome.agent.agent_id.as_str()),
        })
    }

    fn dispatch_assign_task(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        assignment: UiTaskAssignCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, source_agent_id) = task_runtime_target(state)?;
        let runtime = TaskRuntime::boot(&runtime_home, source_agent_id.clone())
            .map_err(map_task_query_error)?;
        let outcome = runtime
            .assign_task(TaskAssignRequest {
                task_id: TaskId::new(assignment.task_id),
                agent_id: assignment.agent_id,
                actor: ui_task_actor(&source_agent_id, None, None),
                watermark: ui_task_watermark("assign_task"),
            })
            .map_err(map_task_query_error)?;
        self.publish_task_list_from_runtime(&runtime_home, &source_agent_id)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!("task_assigned:{}", outcome.task.task_id.as_str()),
        })
    }

    fn dispatch_claim_next_task(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        claim: UiTaskClaimCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, source_agent_id) = task_runtime_target(state)?;
        let runtime = TaskRuntime::boot(&runtime_home, source_agent_id.clone())
            .map_err(map_task_query_error)?;
        let outcome = runtime
            .claim_next_task(TaskClaimRequest {
                agent_id: claim.agent_id.clone(),
                execution_id: claim.execution_id.clone(),
                ttl_seconds: claim.ttl_seconds.unwrap_or(300),
                actor: ui_task_actor(&source_agent_id, None, None),
                watermark: ui_task_watermark("claim_next_task"),
            })
            .map_err(map_task_query_error)?;
        self.publish_task_list_from_runtime(&runtime_home, &source_agent_id)?;
        let status = outcome
            .task
            .as_ref()
            .map(|task| {
                format!(
                    "task_claimed:{}:{}",
                    task.task_id.as_str(),
                    claim.execution_id
                )
            })
            .unwrap_or_else(|| format!("task_claimed:none:{}", claim.agent_id.as_str()));
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: status,
        })
    }

    fn dispatch_submit_task_review(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        review: UiTaskReviewCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, agent_id) = task_runtime_target(state)?;
        let runtime =
            TaskRuntime::boot(&runtime_home, agent_id.clone()).map_err(map_task_query_error)?;
        let outcome = runtime
            .submit_review(TaskReviewSubmission {
                task_id: TaskId::new(review.task_id),
                summary: review.summary,
                deliverables: review.deliverables,
                evidence: review.evidence,
                actor: ui_task_actor(&agent_id, None, None),
                watermark: ui_task_watermark("submit_task_review"),
            })
            .map_err(map_task_query_error)?;
        self.publish_task_list_from_runtime(&runtime_home, &agent_id)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!("task_review_submitted:{}", outcome.task.task_id.as_str()),
        })
    }

    fn dispatch_reject_task_review(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        rejection: UiTaskReviewRejectionCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, agent_id) = task_runtime_target(state)?;
        let runtime =
            TaskRuntime::boot(&runtime_home, agent_id.clone()).map_err(map_task_query_error)?;
        let outcome = runtime
            .reject_review(TaskReviewRejection {
                task_id: TaskId::new(rejection.task_id),
                reject_reason: rejection.reject_reason,
                next_requirements: rejection.next_requirements,
                actor: ui_task_actor(&agent_id, None, None),
                watermark: ui_task_watermark("reject_task_review"),
            })
            .map_err(map_task_query_error)?;
        self.publish_task_list_from_runtime(&runtime_home, &agent_id)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!("task_review_rejected:{}", outcome.task.task_id.as_str()),
        })
    }

    fn dispatch_approve_task_review(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        task_id: String,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, agent_id) = task_runtime_target(state)?;
        let runtime =
            TaskRuntime::boot(&runtime_home, agent_id.clone()).map_err(map_task_query_error)?;
        let outcome = runtime
            .approve_review(TaskMutationRequest {
                task_id: TaskId::new(task_id),
                actor: ui_task_actor(&agent_id, None, None),
                watermark: ui_task_watermark("approve_task_review"),
            })
            .map_err(map_task_query_error)?;
        self.publish_task_list_from_runtime(&runtime_home, &agent_id)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!("task_review_approved:{}", outcome.task.task_id.as_str()),
        })
    }

    fn dispatch_close_task(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        task_id: String,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, agent_id) = task_runtime_target(state)?;
        let runtime =
            TaskRuntime::boot(&runtime_home, agent_id.clone()).map_err(map_task_query_error)?;
        let outcome = runtime
            .close_task(TaskMutationRequest {
                task_id: TaskId::new(task_id),
                actor: ui_task_actor(&agent_id, None, None),
                watermark: ui_task_watermark("close_task"),
            })
            .map_err(map_task_query_error)?;
        self.publish_task_list_from_runtime(&runtime_home, &agent_id)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!("task_closed:{}", outcome.task.task_id.as_str()),
        })
    }

    fn dispatch_apply_execution_fact(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        fact: UiExecutionFactCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, source_agent_id) = task_runtime_target(state)?;
        let runtime = TaskRuntime::boot(&runtime_home, source_agent_id.clone())
            .map_err(map_task_query_error)?;
        let task_id = fact.task_id.clone();
        runtime
            .apply_execution_fact(ui_execution_fact_to_task_fact(fact))
            .map_err(map_task_query_error)?;
        self.publish_task_list_from_runtime(&runtime_home, &source_agent_id)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!("execution_fact_applied:{task_id}"),
        })
    }

    fn dispatch_run_scheduler_tick(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        tick: UiSchedulerTickCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, agent_id) = task_runtime_target(state)?;
        let runtime =
            TaskRuntime::boot(&runtime_home, agent_id.clone()).map_err(map_task_query_error)?;
        let outcome = runtime
            .run_scheduler_tick(SchedulerTickRequest {
                now: now_unix_seconds(),
                stale_after_seconds: tick.stale_after_seconds,
                soft_timeout_seconds: tick.soft_timeout_seconds,
                hard_timeout_seconds: tick.hard_timeout_seconds,
                actor: ui_task_actor(&agent_id, None, None),
                watermark: ui_task_watermark("run_scheduler_tick"),
            })
            .map_err(map_task_query_error)?;
        self.publish_task_list_from_runtime(&runtime_home, &agent_id)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!(
                "scheduler_tick_recorded:facts={} events={}",
                outcome.facts.len(),
                outcome.events.len()
            ),
        })
    }

    fn dispatch_run_master_poll(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let UiCommand::RunMasterPoll {
            after_cursor,
            limit,
            include_terminal,
            replay_from_start,
        } = envelope.command.clone()
        else {
            return Err(UiCommandDispatchPortError::Unsupported(
                "command is not a master poll target".to_owned(),
            ));
        };
        let (runtime_home, agent_id) = task_runtime_target(state)?;
        let runtime =
            TaskRuntime::boot(&runtime_home, agent_id.clone()).map_err(map_task_query_error)?;
        let outcome = runtime
            .run_master_poll(MasterPollRequest {
                after_cursor,
                limit: limit.unwrap_or(0),
                include_terminal,
                replay_from_start,
                actor: ui_task_actor(&agent_id, None, None),
                watermark: ui_task_watermark("run_master_poll"),
            })
            .map_err(map_task_query_error)?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!(
                "master_poll_recorded:events={} classifications={} cursor={}",
                outcome.event_inbox.events.len(),
                outcome.classifications.len(),
                outcome
                    .persisted_cursor
                    .as_deref()
                    .or(outcome.next_cursor.as_deref())
                    .unwrap_or("none")
            ),
        })
    }

    fn dispatch_worker_control(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        control: UiWorkerControlCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let (runtime_home, source_agent_id) = task_runtime_target(state)?;
        let runtime = TaskRuntime::boot(&runtime_home, source_agent_id.clone())
            .map_err(map_task_query_error)?;
        let projection = runtime
            .apply_worker_control(ui_worker_control_to_task_request(
                &source_agent_id,
                control,
            )?)
            .map_err(map_task_query_error)?;
        let task_truth_changed = projection.task_event.is_some();
        let ui_projection = project_worker_control_for_ui(source_agent_id.clone(), projection);
        if task_truth_changed {
            self.publish_task_list_from_runtime(&runtime_home, &source_agent_id)?;
        }
        let event = ui_projection
            .event
            .expect("worker control dispatch projection always carries latest event");
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!(
                "worker_control_applied:{}:{}:{}",
                event.op, event.control_id, event.status
            ),
        })
    }

    fn dispatch_schedule_timer(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        timer: UiTimerScheduleCommand,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let Some(live) = state.config.live.as_ref() else {
            return Err(UiCommandDispatchPortError::Unsupported(
                "timer scheduling requires a live runtime home".to_owned(),
            ));
        };
        let store = TimerStore::new(&live.runtime_home, &state.config.reason_agent_id);
        let schedule = store
            .schedule_from_request(ui_timer_schedule_to_runtime_request(timer)?)
            .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))
            .and_then(|schedule| {
                store
                    .upsert_schedule(schedule)
                    .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))
            })?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!(
                "timer_scheduled:timer_id={} next_due_at={} status={}",
                schedule.timer_id, schedule.next_due_at, schedule.status
            ),
        })
    }

    fn dispatch_cancel_timer(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        timer_id: String,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let Some(live) = state.config.live.as_ref() else {
            return Err(UiCommandDispatchPortError::Unsupported(
                "timer cancellation requires a live runtime home".to_owned(),
            ));
        };
        let store = TimerStore::new(&live.runtime_home, &state.config.reason_agent_id);
        let schedule = store.cancel(timer_id.trim()).map_err(|err| match err {
            timer_store::TimerStoreError::NotFound(timer_id) => {
                UiCommandDispatchPortError::TargetNotFound(timer_id)
            }
            other => UiCommandDispatchPortError::DispatchFailed(other.to_string()),
        })?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!(
                "timer_cancelled:timer_id={} status={}",
                schedule.timer_id, schedule.status
            ),
        })
    }

    fn publish_task_list_from_runtime(
        &self,
        runtime_home: &Path,
        agent_id: &AgentId,
    ) -> Result<(), UiCommandDispatchPortError> {
        let projection = task_list_projection_from_runtime(runtime_home, agent_id, None, None)
            .map_err(map_task_query_error)?;
        self.ui_state
            .lock()
            .expect("lock ui state")
            .publish_task_list_projection(projection);
        Ok(())
    }

    fn dispatch_update_provider_config(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        update: UiProviderConfigUpdate,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let live = state.config.live.as_ref().ok_or_else(|| {
            UiCommandDispatchPortError::Unsupported(
                "provider config update requires a live runtime home".to_owned(),
            )
        })?;
        let config_path = live.runtime_home.join("config.toml");
        update_provider_config_in_path(
            &config_path,
            ProviderConfigUpdate {
                agent_name: update.agent_name.clone(),
                provider_id: update.provider_id,
                provider_type: update.provider_type,
                protocol: update.provider_protocol,
                base_url: update.base_url,
                default_model: update.default_model,
                web_search: update.web_search,
                api_key_env: update.api_key_env,
            },
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        state.pending_config_status = Some(project_config_status_from_path_for_ui(
            &config_path,
            &update.agent_name,
        )?);
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "provider_config_saved_restart_required".to_owned(),
        })
    }

    fn dispatch_upsert_provider_config(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        update: UiProviderConfigUpdate,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let live = state.config.live.as_ref().ok_or_else(|| {
            UiCommandDispatchPortError::Unsupported(
                "provider config upsert requires a live runtime home".to_owned(),
            )
        })?;
        let config_path = live.runtime_home.join("config.toml");
        upsert_provider_config_in_path(
            &config_path,
            ProviderConfigUpdate {
                agent_name: update.agent_name.clone(),
                provider_id: update.provider_id,
                provider_type: update.provider_type,
                protocol: update.provider_protocol,
                base_url: update.base_url,
                default_model: update.default_model,
                web_search: update.web_search,
                api_key_env: update.api_key_env,
            },
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        state.pending_config_status = Some(project_config_status_from_path_for_ui(
            &config_path,
            &update.agent_name,
        )?);
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "provider_config_upserted_restart_required".to_owned(),
        })
    }

    fn dispatch_upsert_model_group_config(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        group: UiModelGroupConfigUpdate,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let live = state.config.live.as_ref().ok_or_else(|| {
            UiCommandDispatchPortError::Unsupported(
                "model group upsert requires a live runtime home".to_owned(),
            )
        })?;
        let agent_name = group.agent_name.clone();
        let config_path = live.runtime_home.join("config.toml");
        upsert_model_group_config_in_path(
            &config_path,
            ModelGroupConfigUpdate {
                agent_name: agent_name.clone(),
                group_id: group.group_id,
                enabled: group.enabled,
                label: group.label,
                primary: model_route_config_from_ui(group.primary),
                sub: group.sub.map(model_route_config_from_ui),
                search: group.search.map(model_route_config_from_ui),
                title: group.title.map(model_route_config_from_ui),
                fallback: group.fallback.map(model_route_config_from_ui),
                load_balance: group
                    .load_balance
                    .into_iter()
                    .map(model_weighted_route_config_from_ui)
                    .collect(),
            },
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        state.pending_config_status = Some(project_config_status_from_path_for_ui(
            &config_path,
            &agent_name,
        )?);
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "model_group_config_upserted_restart_required".to_owned(),
        })
    }

    fn dispatch_update_agent_model_group_selection(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        selection: UiAgentModelGroupSelectionUpdate,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let live = state.config.live.as_ref().ok_or_else(|| {
            UiCommandDispatchPortError::Unsupported(
                "model group selection update requires a live runtime home".to_owned(),
            )
        })?;
        let config_path = live.runtime_home.join("config.toml");
        switch_agent_model_group_in_path(
            &config_path,
            AgentModelGroupSelectionConfigUpdate {
                agent_name: selection.agent_name.clone(),
                model_group_id: selection.model_group_id,
            },
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        state.pending_config_status = Some(project_config_status_from_path_for_ui(
            &config_path,
            &selection.agent_name,
        )?);
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "model_group_selection_saved_restart_required".to_owned(),
        })
    }

    fn dispatch_test_provider_web_search(
        &self,
        envelope: UiCommandDispatchEnvelope,
        runtime_home: PathBuf,
        agent_name: String,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let UiCommand::TestProviderWebSearch { provider_id, query } = envelope.command.clone()
        else {
            return Err(UiCommandDispatchPortError::Unsupported(
                "command is not a provider web_search test".to_owned(),
            ));
        };
        let config_path = runtime_home.join("config.toml");
        let loaded = load_config_from_path(&config_path)
            .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        let selected = loaded
            .select_agent(&agent_name)
            .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        let provider = loaded
            .select_provider_for_test(&provider_id)
            .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        let receipt_status =
            execute_provider_web_search_test(&selected, provider, query.as_deref())?;
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: receipt_status,
        })
    }

    fn dispatch_update_agent_provider_selection(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        selection: UiAgentProviderSelectionUpdate,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let live = state.config.live.as_ref().ok_or_else(|| {
            UiCommandDispatchPortError::Unsupported(
                "provider selection update requires a live runtime home".to_owned(),
            )
        })?;
        let config_path = live.runtime_home.join("config.toml");
        switch_agent_provider_in_path(
            &config_path,
            AgentProviderSelectionConfigUpdate {
                agent_name: selection.agent_name.clone(),
                provider_id: selection.provider_id,
                fallback_provider_id: selection.fallback_provider_id,
            },
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        state.pending_config_status = Some(project_config_status_from_path_for_ui(
            &config_path,
            &selection.agent_name,
        )?);
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "agent_provider_selection_saved_restart_required".to_owned(),
        })
    }

    fn dispatch_update_agent_resource_config(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        update: UiAgentResourceConfigUpdate,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let live = state.config.live.as_ref().ok_or_else(|| {
            UiCommandDispatchPortError::Unsupported(
                "Agent resource config update requires a live runtime home".to_owned(),
            )
        })?;
        let config_path = live.runtime_home.join("config.toml");
        let resource_count = update.resource_count;
        update_agent_resource_config_in_path(
            &config_path,
            AgentResourceConfigUpdate {
                agent_name: update.agent_name.clone(),
                resource_count,
            },
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        state.pending_config_status = Some(project_config_status_from_path_for_ui(
            &config_path,
            &update.agent_name,
        )?);
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: format!(
                "agent_resource_config_saved_restart_required:count={resource_count}"
            ),
        })
    }
}

impl UiRuntimeQueryPort for RuntimeCommandDispatcher {
    fn query_runtime(
        &self,
        command: &UiCommand,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError> {
        RuntimeCommandDispatcher::query_runtime(self, command)
    }
}

fn map_node_dispatch_error(err: NodeRuntimeError) -> UiCommandDispatchPortError {
    match err {
        NodeRuntimeError::SlaveNotPaired
        | NodeRuntimeError::UnsupportedTransport
        | NodeRuntimeError::MetadataWriteFailed(_)
        | NodeRuntimeError::RemoteDaemonDirectory(_) => {
            UiCommandDispatchPortError::DispatchFailed(err.to_string())
        }
        NodeRuntimeError::UnauthorizedPairSourceNode
        | NodeRuntimeError::UnauthorizedPairSourceIp
        | NodeRuntimeError::PairTokenMismatch
        | NodeRuntimeError::EmptyDirectMessage
        | NodeRuntimeError::EmptyTaskStatus
        | NodeRuntimeError::EmptyMasterNodeId
        | NodeRuntimeError::EmptySlaveNodeId
        | NodeRuntimeError::EmptyPairedMasterNodeId
        | NodeRuntimeError::EmptyPairedSlaveNodeId
        | NodeRuntimeError::EmptyPairToken => {
            UiCommandDispatchPortError::TargetNotFound(err.to_string())
        }
    }
}

fn map_session_metadata_dispatch_error(err: ReasonPersistenceError) -> UiCommandDispatchPortError {
    match err {
        ReasonPersistenceError::SessionMetadataTargetNotFound(session_id) => {
            UiCommandDispatchPortError::TargetNotFound(session_id)
        }
        ReasonPersistenceError::SessionRollbackTargetNotFound(session_id) => {
            UiCommandDispatchPortError::TargetNotFound(session_id)
        }
        ReasonPersistenceError::InvalidSessionMetadata(message) => {
            UiCommandDispatchPortError::DispatchFailed(message)
        }
        other => UiCommandDispatchPortError::DispatchFailed(other.to_string()),
    }
}

fn restore_session_turns_for_ui_query(
    config: &RuntimeCommandDispatcherConfig,
    runtime_home: &Path,
    session_id: &SessionId,
) -> Result<Option<(AgentId, Vec<TurnRecord>)>, UiCommandDispatchPortError> {
    for agent_id in queryable_reason_agent_ids(config) {
        let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), agent_id.clone());
        match persistence.restore_turn_snapshots_for_ui(session_id) {
            Ok(turns) => return Ok(Some((agent_id, turns))),
            Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => continue,
            Err(error) => {
                return Err(UiCommandDispatchPortError::DispatchFailed(format!(
                    "failed to restore session turns from reason persistence: {error}"
                )));
            }
        }
    }
    Ok(None)
}

fn queryable_reason_agent_ids(config: &RuntimeCommandDispatcherConfig) -> Vec<AgentId> {
    let mut agent_ids = Vec::<AgentId>::new();
    push_unique_agent_id(&mut agent_ids, config.reason_agent_id.clone());
    push_unique_agent_id(&mut agent_ids, config.slave_agent_id.clone());
    if let Some(live) = config.live.as_ref() {
        for peer in live.selected_agent.worker_peers() {
            push_unique_agent_id(&mut agent_ids, AgentId::new(peer.name.clone()));
        }
    }
    agent_ids
}

fn push_unique_agent_id(agent_ids: &mut Vec<AgentId>, agent_id: AgentId) {
    if !agent_ids.iter().any(|known| known == &agent_id) {
        agent_ids.push(agent_id);
    }
}

fn query_session_search_for_ui(
    config: &RuntimeCommandDispatcherConfig,
    live: &RuntimeLiveDispatcherConfig,
    query: &str,
    limit: Option<usize>,
) -> Result<UiSessionSearchProjection, UiCommandDispatchPortError> {
    let query = query.trim();
    let normalized_query = query.to_lowercase();
    let result_limit = limit.unwrap_or(20).clamp(1, 50);
    let master_persistence =
        ReasonPersistence::new(live.runtime_home.clone(), config.reason_agent_id.clone());
    let mut master_index = master_persistence
        .list_persisted_sessions()
        .map_err(map_session_search_persistence_error)?;
    let parent_metadata_by_session = master_persistence
        .load_session_metadata()
        .map_err(map_session_search_persistence_error)?
        .into_iter()
        .filter(|metadata| !metadata.archived && !internal_runtime_session_id(&metadata.session_id))
        .map(|metadata| (metadata.session_id.clone(), metadata))
        .collect::<BTreeMap<_, _>>();
    for metadata in parent_metadata_by_session.values() {
        if !master_index
            .iter()
            .any(|entry| entry.session_id == metadata.session_id)
        {
            master_index.push(PersistedSessionIndexEntry {
                agent_id: metadata.agent_id.clone(),
                session_id: metadata.session_id.clone(),
                latest_turn_id: None,
                active_turn_id: None,
                latest_terminal_summary: None,
            });
        }
    }
    let parent_index_by_session = master_index
        .iter()
        .map(|entry| (entry.session_id.clone(), entry.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut results = BTreeMap::<SessionId, UiSessionSearchResultProjection>::new();

    for entry in &master_index {
        let Some(metadata) = parent_metadata_by_session.get(&entry.session_id) else {
            continue;
        };
        if let Some((matched_fields, snippet)) =
            session_search_match(entry, Some(metadata), &normalized_query)
        {
            results.insert(
                entry.session_id.clone(),
                parent_session_search_result(entry, metadata, matched_fields, snippet),
            );
        }
    }

    let worker_parent_by_session =
        worker_parent_session_map(&live.runtime_home, &config.reason_agent_id)?;
    for agent_id in queryable_reason_agent_ids(config)
        .into_iter()
        .filter(|agent_id| agent_id != &config.reason_agent_id)
    {
        let persistence = ReasonPersistence::new(live.runtime_home.clone(), agent_id);
        let index = persistence
            .list_persisted_sessions()
            .map_err(map_session_search_persistence_error)?;
        for child in index {
            let Some((parent_session_id, task_id, task_title)) =
                worker_parent_by_session.get(&child.session_id)
            else {
                continue;
            };
            let Some(parent_metadata) = parent_metadata_by_session.get(parent_session_id) else {
                continue;
            };
            let Some((matched_fields, snippet)) =
                session_search_match(&child, None, &normalized_query)
            else {
                continue;
            };
            let parent_entry = parent_index_by_session
                .get(parent_session_id)
                .cloned()
                .unwrap_or_else(|| PersistedSessionIndexEntry {
                    agent_id: config.reason_agent_id.clone(),
                    session_id: parent_session_id.clone(),
                    latest_turn_id: None,
                    active_turn_id: None,
                    latest_terminal_summary: None,
                });
            let result = results.entry(parent_session_id.clone()).or_insert_with(|| {
                parent_session_search_result(
                    &parent_entry,
                    parent_metadata,
                    vec!["worker_child".to_owned()],
                    format!(
                        "Worker child match in {}",
                        task_title.as_deref().unwrap_or(child.session_id.as_str())
                    ),
                )
            });
            result.child_matches.push(UiSessionSearchChildProjection {
                session_id: child.session_id.clone(),
                task_id: Some(task_id.clone()),
                title: task_title.clone(),
                latest_turn_id: child.latest_turn_id.clone(),
                latest_status: session_index_status(&child),
                snippet,
                matched_fields,
            });
        }
    }

    let mut results = results.into_values().collect::<Vec<_>>();
    results.sort_by(|left, right| right.session_id.cmp(&left.session_id));
    results.truncate(result_limit);
    Ok(UiSessionSearchProjection {
        query: query.to_owned(),
        results,
    })
}

fn map_session_search_persistence_error(err: ReasonPersistenceError) -> UiCommandDispatchPortError {
    UiCommandDispatchPortError::DispatchFailed(format!(
        "failed to query persisted session index: {err}"
    ))
}

fn worker_parent_session_map(
    runtime_home: &Path,
    source_agent_id: &AgentId,
) -> Result<BTreeMap<SessionId, (SessionId, String, Option<String>)>, UiCommandDispatchPortError> {
    let task_runtime =
        TaskRuntime::boot(runtime_home, source_agent_id.clone()).map_err(map_task_query_error)?;
    let board = task_runtime
        .query_task_board(TaskBoardQuery {
            status: None,
            assignee: None,
            include_terminal: true,
        })
        .map_err(map_task_query_error)?;
    let mut map = BTreeMap::new();
    for task in board.tasks {
        let Some(parent_session_id) = task.parent.session_id.clone() else {
            continue;
        };
        map.insert(
            worker_session_id_for_task(&task.task_id),
            (
                parent_session_id,
                task.task_id.as_str().to_owned(),
                Some(task.title),
            ),
        );
    }
    Ok(map)
}

fn internal_runtime_session_id(session_id: &SessionId) -> bool {
    let id = session_id.as_str();
    id.starts_with("worker-task-")
        || id.starts_with("master-lifecycle-")
        || id.starts_with("master-timer-")
}

fn session_search_match(
    entry: &PersistedSessionIndexEntry,
    metadata: Option<&PersistedSessionMetadataEntry>,
    normalized_query: &str,
) -> Option<(Vec<String>, String)> {
    let mut matched_fields = Vec::new();
    let mut snippet = None;
    for (field, value) in session_search_fields(entry, metadata) {
        if value.to_lowercase().contains(normalized_query) {
            matched_fields.push(field);
            if snippet.is_none() {
                snippet = Some(search_snippet(&value, normalized_query));
            }
        }
    }
    if matched_fields.is_empty() {
        None
    } else {
        Some((matched_fields, snippet.unwrap_or_default()))
    }
}

fn session_search_fields(
    entry: &PersistedSessionIndexEntry,
    metadata: Option<&PersistedSessionMetadataEntry>,
) -> Vec<(String, String)> {
    let mut fields = vec![
        (
            "session_id".to_owned(),
            entry.session_id.as_str().to_owned(),
        ),
        ("agent_id".to_owned(), entry.agent_id.as_str().to_owned()),
    ];
    if let Some(turn_id) = &entry.latest_turn_id {
        fields.push(("latest_turn_id".to_owned(), turn_id.as_str().to_owned()));
    }
    if let Some(summary) = &entry.latest_terminal_summary {
        fields.push(("latest_summary".to_owned(), summary.clone()));
    }
    if let Some(metadata) = metadata {
        if let Some(title) = &metadata.title {
            fields.push(("title".to_owned(), title.clone()));
        }
        if let Some(cwd) = &metadata.cwd {
            fields.push(("cwd".to_owned(), cwd.clone()));
        }
    }
    fields
}

fn search_snippet(value: &str, normalized_query: &str) -> String {
    let _ = normalized_query;
    value
        .chars()
        .take(180)
        .collect::<String>()
        .replace('\n', " ")
}

fn parent_session_search_result(
    entry: &PersistedSessionIndexEntry,
    metadata: &PersistedSessionMetadataEntry,
    matched_fields: Vec<String>,
    snippet: String,
) -> UiSessionSearchResultProjection {
    UiSessionSearchResultProjection {
        session_id: entry.session_id.clone(),
        title: metadata.title.clone(),
        cwd: metadata.cwd.clone(),
        latest_turn_id: entry.latest_turn_id.clone(),
        latest_status: session_index_status(entry),
        snippet,
        matched_fields,
        child_matches: Vec::new(),
    }
}

fn session_index_status(entry: &PersistedSessionIndexEntry) -> String {
    if entry.active_turn_id.is_some() {
        "running".to_owned()
    } else if entry.latest_turn_id.is_some() {
        "completed".to_owned()
    } else {
        "session".to_owned()
    }
}

fn node_id_for_query_agent(
    config: &RuntimeCommandDispatcherConfig,
    agent_id: &AgentId,
) -> Result<String, UiCommandDispatchPortError> {
    if agent_id == &config.reason_agent_id {
        return Ok(config.master_node_id.clone());
    }
    if agent_id == &config.slave_agent_id {
        return Ok(config.slave_node_id.clone());
    }
    if let Some(live) = config.live.as_ref()
        && let Some(peer) = live
            .selected_agent
            .worker_peers()
            .find(|peer| peer.name == agent_id.as_str())
    {
        return Ok(peer.node_id.clone());
    }
    Err(UiCommandDispatchPortError::TargetNotFound(format!(
        "query agent `{}` has no configured node",
        agent_id.as_str()
    )))
}

fn session_metadata_to_ui(entry: PersistedSessionMetadataEntry) -> UiSessionMetadataProjection {
    UiSessionMetadataProjection {
        session_id: entry.session_id,
        title: entry.title,
        archived: entry.archived,
        cwd: entry.cwd,
    }
}

fn map_checkpoint_dispatch_error(err: RuntimeCheckpointError) -> UiCommandDispatchPortError {
    match err {
        RuntimeCheckpointError::MissingManifest(checkpoint_id) => {
            UiCommandDispatchPortError::TargetNotFound(checkpoint_id)
        }
        other => UiCommandDispatchPortError::DispatchFailed(other.to_string()),
    }
}

fn task_runtime_target(
    state: &RuntimeCommandDispatcherState,
) -> Result<(PathBuf, AgentId), UiCommandDispatchPortError> {
    let live = state.config.live.as_ref().ok_or_else(|| {
        UiCommandDispatchPortError::Unsupported(
            "task mutation requires a live runtime home".to_owned(),
        )
    })?;
    Ok((
        live.runtime_home.clone(),
        state.config.reason_agent_id.clone(),
    ))
}

fn ui_task_actor(
    agent_id: &AgentId,
    session_id: Option<SessionId>,
    turn_id: Option<TurnId>,
) -> TaskActor {
    TaskActor {
        agent_id: agent_id.clone(),
        source: "ui.protocol".to_owned(),
        session_id,
        turn_id,
        trace_id: Some(TraceId::new(format!(
            "ui-task-command-{}",
            now_unix_seconds()
        ))),
    }
}

fn ui_task_watermark(action: &str) -> TaskWatermark {
    TaskWatermark {
        metadata_id: None,
        hook: Some(format!("ui.protocol.{action}")),
        action_tool_call_id: None,
    }
}

fn map_task_query_error(err: TaskError) -> UiCommandDispatchPortError {
    match err {
        TaskError::TaskNotFound(task_id)
        | TaskError::AgentNotFound(task_id)
        | TaskError::CursorNotFound(task_id) => UiCommandDispatchPortError::TargetNotFound(task_id),
        other => UiCommandDispatchPortError::DispatchFailed(other.to_string()),
    }
}

fn project_task_list_for_ui(
    source_agent_id: AgentId,
    status_filter: Option<String>,
    agent_filter: Option<AgentId>,
    tasks: Vec<TaskSnapshot>,
) -> UiTaskListProjection {
    UiTaskListProjection {
        source_agent_id,
        status_filter,
        agent_filter,
        tasks: tasks
            .into_iter()
            .map(project_task_snapshot_for_ui)
            .collect(),
    }
}

fn task_list_projection_from_runtime(
    runtime_home: &Path,
    source_agent_id: &AgentId,
    status_filter: Option<String>,
    agent_filter: Option<AgentId>,
) -> Result<UiTaskListProjection, TaskError> {
    let task_runtime = TaskRuntime::boot(runtime_home, source_agent_id.clone())?;
    let parsed_status = status_filter
        .as_deref()
        .map(parse_task_status)
        .transpose()
        .map_err(TaskError::Persistence)?;
    let tasks = task_runtime.list_tasks(TaskListQuery {
        status: parsed_status,
        assignee: agent_filter.clone(),
    })?;
    Ok(project_task_list_for_ui(
        source_agent_id.clone(),
        status_filter,
        agent_filter,
        tasks,
    ))
}

fn project_task_board_for_ui(
    source_agent_id: AgentId,
    status_filter: Option<String>,
    agent_filter: Option<AgentId>,
    include_terminal: bool,
    board: TaskBoardProjection,
) -> UiTaskBoardProjection {
    UiTaskBoardProjection {
        source_agent_id,
        status_filter,
        agent_filter,
        include_terminal,
        tasks: board
            .tasks
            .into_iter()
            .map(project_task_snapshot_for_ui)
            .collect(),
        agents: board
            .agents
            .into_iter()
            .map(project_agent_snapshot_for_ui)
            .collect(),
        blocked: board
            .blocked
            .into_iter()
            .map(project_task_snapshot_for_ui)
            .collect(),
        review_ready: board
            .review_ready
            .into_iter()
            .map(project_task_snapshot_for_ui)
            .collect(),
        stale: board
            .stale
            .into_iter()
            .map(project_task_snapshot_for_ui)
            .collect(),
    }
}

fn project_agent_snapshot_for_ui(agent: AgentSnapshot) -> UiAgentSnapshotProjection {
    UiAgentSnapshotProjection {
        agent_id: agent.agent_id,
        status: agent_status_label(&agent.status).to_owned(),
        current_task_id: agent
            .current_task_id
            .map(|task_id| task_id.as_str().to_owned()),
        current_cwd: agent.current_cwd,
        running_tasks: agent.running_tasks,
        queued_tasks: agent.queued_tasks,
        last_seen_at: agent.last_seen_at,
    }
}

fn project_agent_board_for_ui(
    source_agent_id: AgentId,
    agents: Vec<AgentLifecycleSnapshot>,
) -> UiAgentBoardProjection {
    UiAgentBoardProjection {
        source_agent_id,
        agents: agents
            .into_iter()
            .map(project_agent_lifecycle_for_ui)
            .collect(),
    }
}

fn project_agent_lifecycle_for_ui(lifecycle: AgentLifecycleSnapshot) -> UiAgentLifecycleProjection {
    let process = lifecycle_process_projection(&lifecycle);
    UiAgentLifecycleProjection {
        agent_id: lifecycle.agent_id,
        role: lifecycle.role,
        alive: lifecycle.alive,
        process,
        state: agent_lifecycle_state_label(&lifecycle.state).to_owned(),
        current_task_id: lifecycle
            .current_task_id
            .map(|task_id| task_id.as_str().to_owned()),
        current_execution_id: lifecycle.current_execution_id,
        current_turn_id: lifecycle.current_turn_id,
        current_activity: lifecycle
            .current_activity
            .map(project_agent_lifecycle_activity_for_ui),
        last_activity: lifecycle
            .last_activity
            .map(project_agent_lifecycle_activity_for_ui),
        model_request_count: lifecycle.stats.model_request_count,
        model_retry_count: lifecycle.stats.model_retry_count,
        tool_call_count: lifecycle.stats.tool_call_count,
        tool_failure_count: lifecycle.stats.tool_failure_count,
        schema_polish_count: lifecycle.stats.schema_polish_count,
        provider_error_count: lifecycle.stats.provider_error_count,
        blocked_count: lifecycle.stats.blocked_count,
        current_model: lifecycle.stats.current_model,
        last_seen_at: lifecycle.last_seen_at,
        elapsed_ms: lifecycle.elapsed_ms,
    }
}

fn lifecycle_process_projection(
    lifecycle: &AgentLifecycleSnapshot,
) -> Option<Box<UiAgentProcessProjection>> {
    if lifecycle.process_id.is_none()
        && lifecycle.process_instance_id.is_none()
        && lifecycle.process_started_at.is_none()
        && lifecycle.process_heartbeat_at.is_none()
        && lifecycle.restart_count == 0
    {
        return None;
    }
    Some(Box::new(UiAgentProcessProjection {
        process_id: lifecycle.process_id,
        process_instance_id: lifecycle.process_instance_id.clone(),
        started_at: lifecycle.process_started_at,
        heartbeat_at: lifecycle.process_heartbeat_at,
        restart_count: lifecycle.restart_count,
        next_check_at: lifecycle.next_check_at,
    }))
}

fn project_agent_lifecycle_activity_for_ui(
    activity: AgentLifecycleActivity,
) -> UiAgentLifecycleActivityProjection {
    UiAgentLifecycleActivityProjection {
        kind: activity.kind,
        semantic_summary: activity.semantic_summary,
        target: activity.target,
        elapsed_ms: activity.elapsed_ms,
        tool_name: activity.tool_name,
        model: activity.model,
        retry_count: activity.retry_count,
        visibility: activity.visibility,
    }
}

fn ui_execution_fact_to_task_fact(fact: UiExecutionFactCommand) -> ExecutionFact {
    ExecutionFact {
        execution_id: fact.execution_id,
        task_id: TaskId::new(fact.task_id),
        agent_id: fact.agent_id,
        turn_id: fact.turn_id,
        occurred_at: now_unix_seconds(),
        kind: match fact.kind {
            UiExecutionFactKind::Running {
                phase,
                summary,
                evidence,
            } => ExecutionFactKind::Running {
                phase,
                summary,
                evidence,
            },
            UiExecutionFactKind::Recovering {
                summary,
                evidence,
                retry_count,
            } => ExecutionFactKind::Recovering {
                summary,
                evidence,
                retry_count,
            },
            UiExecutionFactKind::Blocked { reason, evidence } => {
                ExecutionFactKind::Blocked { reason, evidence }
            }
            UiExecutionFactKind::Interrupted { reason, evidence } => {
                ExecutionFactKind::Interrupted { reason, evidence }
            }
            UiExecutionFactKind::ReviewReady {
                summary,
                deliverables,
                evidence,
            } => ExecutionFactKind::ReviewReady {
                summary,
                deliverables,
                evidence,
            },
        },
        watermark: ui_task_watermark("apply_execution_fact"),
    }
}

fn project_task_snapshot_for_ui(task: TaskSnapshot) -> UiTaskSnapshotProjection {
    let worker_session_id = worker_session_id_for_task(&task.task_id);
    UiTaskSnapshotProjection {
        task_id: task.task_id.as_str().to_owned(),
        status: task_status_label(&task.status).to_owned(),
        title: task.title,
        goal: task.goal,
        priority: task.priority,
        target_cwd: task.target_cwd,
        execution_profile: task.execution_profile.as_str().to_owned(),
        parent_session_id: task.parent.session_id,
        attached_session_ids: task.attached_session_ids,
        worker_session_id: Some(worker_session_id),
        assignee_agent_id: task.assignee.map(|assignee| assignee.agent_id),
        active_execution_id: task.active_execution_id,
        created_at: task.created_at,
        updated_at: task.updated_at,
        last_progress_at: task.last_progress_at,
        last_event_seq: task.last_event_seq,
    }
}

fn worker_session_id_for_task(task_id: &TaskId) -> SessionId {
    SessionId::new(format!(
        "worker-task-{}",
        sanitize_session_component(task_id.as_str())
    ))
}

fn sanitize_session_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn project_task_history_for_ui(
    source_agent_id: AgentId,
    task_id: String,
    events: Vec<TaskLedgerEvent>,
) -> UiTaskHistoryProjection {
    UiTaskHistoryProjection {
        source_agent_id,
        task_id,
        events: events
            .into_iter()
            .map(project_task_ledger_event_for_ui)
            .collect(),
    }
}

fn project_task_ledger_event_for_ui(event: TaskLedgerEvent) -> UiTaskLedgerEventProjection {
    UiTaskLedgerEventProjection {
        seq: event.seq,
        event_id: event.event_id,
        event_type: event.event_type,
        from_status: event
            .from_status
            .as_ref()
            .map(task_status_label)
            .map(str::to_owned),
        to_status: task_status_label(&event.to_status).to_owned(),
        timestamp: event.timestamp,
        actor_agent_id: event.actor.agent_id,
        payload: event.payload,
    }
}

fn project_worker_control_for_ui(
    source_agent_id: AgentId,
    projection: WorkerControlProjection,
) -> UiWorkerControlProjection {
    let event = project_worker_control_event_for_ui(projection.event);
    UiWorkerControlProjection {
        source_agent_id,
        generated_at: projection.generated_at,
        event: Some(event.clone()),
        events: vec![event],
        task: Some(project_task_snapshot_for_ui(projection.task)),
        agent: Some(project_agent_snapshot_for_ui(projection.agent)),
        lifecycle: projection.lifecycle.map(project_agent_lifecycle_for_ui),
        task_event: projection.task_event.map(project_task_ledger_event_for_ui),
    }
}

fn project_worker_control_events_for_ui(
    source_agent_id: AgentId,
    events: Vec<WorkerControlEvent>,
) -> UiWorkerControlProjection {
    let events = events
        .into_iter()
        .map(project_worker_control_event_for_ui)
        .collect::<Vec<_>>();
    UiWorkerControlProjection {
        source_agent_id,
        generated_at: now_unix_seconds(),
        event: events.last().cloned(),
        events,
        task: None,
        agent: None,
        lifecycle: None,
        task_event: None,
    }
}

fn project_worker_control_event_for_ui(
    event: WorkerControlEvent,
) -> UiWorkerControlEventProjection {
    UiWorkerControlEventProjection {
        control_id: event.control_id,
        op: worker_control_op_label(&event.op).to_owned(),
        status: event.status,
        task_id: event.task_id.as_str().to_owned(),
        execution_id: event.execution_id,
        agent_id: event.agent_id,
        created_at: event.created_at,
        summary: event.summary,
        payload: event.payload,
    }
}

fn project_event_inbox_for_ui(
    source_agent_id: AgentId,
    inbox: TaskEventInboxProjection,
) -> UiTaskEventInboxProjection {
    UiTaskEventInboxProjection {
        source_agent_id,
        generated_at: inbox.generated_at,
        source_cursor: inbox.source_cursor,
        next_cursor: inbox.next_cursor,
        events: inbox
            .events
            .into_iter()
            .map(project_event_inbox_entry_for_ui)
            .collect(),
    }
}

fn project_event_inbox_entry_for_ui(entry: TaskEventInboxEntry) -> UiTaskEventInboxEntryProjection {
    UiTaskEventInboxEntryProjection {
        cursor: entry.cursor,
        event_id: entry.event_id,
        kind: entry.kind,
        task_id: entry.task_id.as_str().to_owned(),
        execution_id: entry.execution_id,
        agent_id: entry.agent_id,
        created_at: entry.created_at,
        payload: entry.payload,
    }
}

fn project_master_poll_for_ui(
    source_agent_id: AgentId,
    outcome: MasterPollOutcome,
) -> UiMasterPollProjection {
    let include_terminal = outcome.include_terminal;
    UiMasterPollProjection {
        source_agent_id: source_agent_id.clone(),
        generated_at: outcome.generated_at,
        source_cursor: outcome.source_cursor,
        next_cursor: outcome.next_cursor,
        persisted_cursor: outcome.persisted_cursor,
        event_inbox: project_event_inbox_for_ui(source_agent_id.clone(), outcome.event_inbox),
        task_board: project_task_board_for_ui(
            source_agent_id.clone(),
            None,
            None,
            include_terminal,
            outcome.task_board,
        ),
        agent_board: project_agent_board_for_ui(source_agent_id, outcome.agent_board.agents),
        classifications: outcome
            .classifications
            .into_iter()
            .map(project_master_poll_classification_for_ui)
            .collect(),
    }
}

fn project_master_poll_classification_for_ui(
    classification: MasterPollClassification,
) -> UiMasterPollClassificationProjection {
    UiMasterPollClassificationProjection {
        kind: classification.kind,
        summary: classification.summary,
        task_id: classification
            .task_id
            .map(|task_id| task_id.as_str().to_owned()),
        execution_id: classification.execution_id,
        agent_id: classification.agent_id,
        recommended_actions: classification.recommended_actions,
    }
}

fn project_timer_list_for_ui(
    source_agent_id: AgentId,
    include_terminal: bool,
    schedules: Vec<TimerSchedule>,
    events: Vec<timer_store::TimerLedgerEvent>,
) -> UiTimerListProjection {
    let mut timers = schedules
        .into_iter()
        .filter(|schedule| include_terminal || timer_schedule_is_nonterminal(schedule))
        .map(project_timer_schedule_for_ui)
        .collect::<Vec<_>>();
    timers.sort_by_key(|timer| (timer.next_due_at, timer.timer_id.clone()));
    let mut events = events
        .into_iter()
        .filter(|event| include_terminal || timer_event_is_nonterminal(event))
        .map(project_timer_event_for_ui)
        .collect::<Vec<_>>();
    events.sort_by_key(|event| (event.occurred_at, event.event_id.clone()));
    UiTimerListProjection {
        source_agent_id,
        generated_at: now_unix_seconds(),
        include_terminal,
        timers,
        events,
    }
}

fn project_tool_registry_for_ui(source_agent_id: AgentId) -> UiToolRegistryProjection {
    let projection = BuiltinToolRegistry::reasonix_aligned().registry_projection();
    UiToolRegistryProjection {
        source_agent_id,
        generated_at: now_unix_seconds(),
        registry_version: projection.registry_version,
        guidance: projection.guidance,
        tools: projection
            .tools
            .into_iter()
            .map(|tool| UiToolRegistryToolProjection {
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
                read_only: tool.read_only,
                implemented: tool.implemented,
                execution_scope: tool.execution_scope,
                exposed_to_master: tool.exposed_to_master,
                exposed_to_worker: tool.exposed_to_worker,
                examples: tool.examples,
                guidance: tool.guidance,
            })
            .collect(),
    }
}

fn project_diagnostics_for_ui(
    source_agent_id: AgentId,
    runtime_home: &Path,
) -> Result<UiDiagnosticsProjection, UiCommandDispatchPortError> {
    let logs_dir = runtime_home.join("logs");
    let mut files = Vec::new();
    if logs_dir.exists() {
        for entry in fs::read_dir(&logs_dir).map_err(|err| {
            UiCommandDispatchPortError::DispatchFailed(format!(
                "failed to read diagnostics logs directory: {err}"
            ))
        })? {
            let entry = entry.map_err(|err| {
                UiCommandDispatchPortError::DispatchFailed(format!(
                    "failed to read diagnostics log entry: {err}"
                ))
            })?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !name.ends_with(".log") {
                continue;
            }
            let metadata = entry.metadata().map_err(|err| {
                UiCommandDispatchPortError::DispatchFailed(format!(
                    "failed to read diagnostics log metadata for {name}: {err}"
                ))
            })?;
            if !metadata.is_file() {
                continue;
            }
            files.push(project_diagnostic_log_file(
                name.to_owned(),
                path,
                metadata,
            )?);
        }
    }
    files.sort_by(|left, right| {
        right
            .modified_at
            .cmp(&left.modified_at)
            .then_with(|| left.name.cmp(&right.name))
    });
    files.truncate(20);
    Ok(UiDiagnosticsProjection {
        source_agent_id,
        generated_at: now_unix_seconds(),
        runtime_home: "~/.freehand".to_owned(),
        logs_dir: "logs".to_owned(),
        files,
    })
}

fn project_diagnostic_log_file(
    name: String,
    path: PathBuf,
    metadata: fs::Metadata,
) -> Result<UiDiagnosticLogFileProjection, UiCommandDispatchPortError> {
    let tail_lines = diagnostic_log_tail_lines(&path)?;
    Ok(UiDiagnosticLogFileProjection {
        relative_path: format!("logs/{name}"),
        name,
        size_bytes: metadata.len(),
        modified_at: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs()),
        tail_lines,
    })
}

fn diagnostic_log_tail_lines(path: &Path) -> Result<Vec<String>, UiCommandDispatchPortError> {
    const MAX_TAIL_BYTES: usize = 64 * 1024;
    let mut file = fs::File::open(path).map_err(|err| {
        UiCommandDispatchPortError::DispatchFailed(format!(
            "failed to read diagnostics log tail for {}: {err}",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown.log")
        ))
    })?;
    let file_len = file
        .metadata()
        .map_err(|err| {
            UiCommandDispatchPortError::DispatchFailed(format!(
                "failed to read diagnostics log metadata for {}: {err}",
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unknown.log")
            ))
        })?
        .len();
    let tail_len = file_len.min(MAX_TAIL_BYTES as u64);
    file.seek(SeekFrom::Start(file_len.saturating_sub(tail_len)))
        .map_err(|err| {
            UiCommandDispatchPortError::DispatchFailed(format!(
                "failed to seek diagnostics log tail for {}: {err}",
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("unknown.log")
            ))
        })?;
    let mut bytes = Vec::with_capacity(tail_len as usize);
    file.read_to_end(&mut bytes).map_err(|err| {
        UiCommandDispatchPortError::DispatchFailed(format!(
            "failed to read diagnostics log tail for {}: {err}",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown.log")
        ))
    })?;
    let text = String::from_utf8_lossy(&bytes);
    let text = if file_len > tail_len {
        text.split_once('\n').map(|(_, rest)| rest).unwrap_or("")
    } else {
        text.as_ref()
    };
    let mut lines = text
        .lines()
        .rev()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(redact_diagnostic_log_line(trimmed))
            }
        })
        .take(5)
        .collect::<Vec<_>>();
    lines.reverse();
    Ok(lines)
}

fn redact_diagnostic_log_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("authorization")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("x-api-key")
        || lower.contains("bearer ")
        || lower.contains("pair_token")
        || lower.contains("secret")
        || lower.contains("provider request")
        || lower.contains("provider payload")
        || lower.contains("/users/")
        || lower.contains("/volumes/")
    {
        return "[redacted diagnostic line: sensitive marker]".to_owned();
    }
    line.chars().take(240).collect()
}

fn timer_schedule_is_nonterminal(schedule: &TimerSchedule) -> bool {
    matches!(schedule.status.as_str(), "active" | "running")
}

fn timer_event_is_nonterminal(event: &timer_store::TimerLedgerEvent) -> bool {
    matches!(
        event.event_type.as_str(),
        "TimerScheduled" | "TimerFired" | "TimerFailed"
    )
}

fn project_timer_schedule_for_ui(schedule: TimerSchedule) -> UiTimerProjection {
    let (repeat_kind, repeat_summary) = timer_repeat_projection(schedule.repeat.as_ref());
    UiTimerProjection {
        timer_id: schedule.timer_id,
        agent_id: schedule.agent_id,
        status: schedule.status,
        reason: schedule.reason,
        prompt: schedule.prompt,
        next_due_at: schedule.next_due_at,
        created_at: schedule.created_at,
        updated_at: schedule.updated_at,
        fired_count: schedule.fired_count,
        max_runs: schedule.max_runs,
        repeat_kind,
        repeat_summary,
        source_session_id: schedule.source_session_id,
        source_turn_id: schedule.source_turn_id,
    }
}

fn timer_repeat_projection(repeat: Option<&TimerRepeatRule>) -> (String, String) {
    match repeat {
        Some(TimerRepeatRule::Interval {
            interval_seconds, ..
        }) => ("interval".to_owned(), format!("every {interval_seconds}s")),
        Some(TimerRepeatRule::Daily {
            time_of_day_seconds_local,
            skip_weekends,
            ..
        }) => {
            let extra = if *skip_weekends {
                ", skip weekends"
            } else {
                ""
            };
            (
                "daily".to_owned(),
                format!("daily at local +{}s{}", time_of_day_seconds_local, extra),
            )
        }
        Some(TimerRepeatRule::Weekly {
            time_of_day_seconds_local,
            weekdays,
            ..
        }) => (
            "weekly".to_owned(),
            format!(
                "weekly {:?} at local +{}s",
                weekdays, time_of_day_seconds_local
            ),
        ),
        Some(TimerRepeatRule::Cron { expression, .. }) => {
            ("cron".to_owned(), format!("cron `{expression}`"))
        }
        None => (String::new(), String::new()),
    }
}

fn project_timer_event_for_ui(event: timer_store::TimerLedgerEvent) -> UiTimerEventProjection {
    let summary = match event.event_type.as_str() {
        "TimerScheduled" => format!(
            "scheduled next_due_at={} max_runs={}",
            event
                .payload
                .get("next_due_at")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            event
                .payload
                .get("max_runs")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
        "TimerFired" => format!(
            "fired at {}",
            event
                .payload
                .get("fired_at")
                .and_then(Value::as_u64)
                .unwrap_or(event.occurred_at)
        ),
        "TimerCompleted" => format!(
            "completed status={} fired_count={}",
            event
                .payload
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("completed"),
            event
                .payload
                .get("fired_count")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
        "TimerCancelled" => "cancelled".to_owned(),
        "TimerFailed" => format!(
            "failed: {}",
            event
                .payload
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown timer error")
        ),
        other => other.to_owned(),
    };
    UiTimerEventProjection {
        event_id: event.event_id,
        timer_id: event.timer_id,
        event_type: event.event_type,
        occurred_at: event.occurred_at,
        summary,
    }
}

fn query_error_center_events_for_ui(
    runtime_home: &Path,
    source_agent_id: &AgentId,
    session_id: &SessionId,
    trace_filter: Option<String>,
    turn_filter: Option<TurnId>,
    domain_filter: Option<String>,
) -> Result<UiErrorCenterEventListProjection, MetadataError> {
    let center = MetadataCenter::with_ledger_path(metadata_ledger_path(
        runtime_home,
        source_agent_id,
        session_id,
    ))?;
    let events = center
        .records()
        .iter()
        .filter(|record| record.owner.feature_id.as_str() == "error.center")
        .filter(|record| {
            trace_filter
                .as_deref()
                .is_none_or(|trace_id| record.subject.trace_id.as_str() == trace_id)
        })
        .filter(|record| {
            turn_filter
                .as_ref()
                .is_none_or(|turn_id| record.subject.turn_id.as_ref() == Some(turn_id))
        })
        .filter_map(project_error_center_event_for_ui)
        .filter(|event| {
            domain_filter
                .as_deref()
                .is_none_or(|domain| event.domain == domain)
        })
        .collect();
    Ok(UiErrorCenterEventListProjection {
        source_agent_id: source_agent_id.clone(),
        session_id: session_id.clone(),
        trace_filter,
        turn_filter,
        domain_filter,
        events,
    })
}

fn apply_error_center_live_activity_to_session_projections(
    runtime_home: &Path,
    source_agent_id: &AgentId,
    source_node_id: &str,
    session_id: &SessionId,
    ui: &mut UiProtocolState,
    projections: Vec<UiTurnProjection>,
) -> Result<(), UiCommandDispatchPortError> {
    let recovered_waiting = error_center_live_activity_waitings(
        runtime_home,
        source_agent_id,
        source_node_id,
        session_id,
        &projections,
    )
    .map_err(|error| UiCommandDispatchPortError::DispatchFailed(error.to_string()))?;
    ui.replace_session_turn_projections(session_id, projections);
    if recovered_waiting.is_empty() {
        return Ok(());
    }
    let eligible_turn_ids = match ui
        .query(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .map_err(|error| UiCommandDispatchPortError::DispatchFailed(error.to_string()))?
    {
        UiQueryResult::SessionTurns(transcript) => transcript
            .turns
            .into_iter()
            .filter(|turn| {
                turn.model_request.is_none()
                    && turn.terminal_status.is_none()
                    && turn.terminal_text.is_none()
            })
            .map(|turn| turn.turn_id)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    for waiting in recovered_waiting {
        if eligible_turn_ids.contains(&waiting.turn_id) {
            ui.apply_model_request_waiting_kind(waiting);
        }
    }
    Ok(())
}

fn error_center_live_activity_waitings(
    runtime_home: &Path,
    source_agent_id: &AgentId,
    source_node_id: &str,
    session_id: &SessionId,
    projections: &[UiTurnProjection],
) -> Result<Vec<UiModelRequestWaiting>, MetadataError> {
    let error_events = query_error_center_events_for_ui(
        runtime_home,
        source_agent_id,
        session_id,
        None,
        None,
        None,
    )?;
    let mut waitings = Vec::new();
    let Some(projection) = projections.last() else {
        return Ok(waitings);
    };
    if projection.model_request.is_some()
        || projection.terminal_status.is_some()
        || projection.terminal_text.is_some()
    {
        return Ok(waitings);
    }
    let Some(waiting_activity) = error_events
        .events
        .iter()
        .filter(|event| event.turn_id.as_ref() == Some(&projection.turn_id))
        .filter_map(error_center_model_waiting_activity)
        .next_back()
    else {
        return Ok(waitings);
    };
    waitings.push(UiModelRequestWaiting {
        source_agent_id: source_agent_id.clone(),
        source_node_id: source_node_id.to_owned(),
        session_id: session_id.clone(),
        turn_id: projection.turn_id.clone(),
        kind: waiting_activity.kind,
        detail: waiting_activity.detail,
        transport: waiting_activity.transport,
        slave_substream_card: projection.slave_substream_card,
    });
    Ok(waitings)
}

struct ErrorCenterModelWaitingActivity {
    kind: UiModelRequestKind,
    detail: Option<String>,
    transport: Option<UiModelTransportActivity>,
}

fn error_center_model_waiting_activity(
    event: &UiErrorCenterEventProjection,
) -> Option<ErrorCenterModelWaitingActivity> {
    match (event.domain.as_str(), event.recovery_action.as_str()) {
        ("provider", "retry_same_step") => Some(ErrorCenterModelWaitingActivity {
            kind: UiModelRequestKind::Thinking,
            detail: Some("Waiting for model response.".to_owned()),
            transport: Some(UiModelTransportActivity {
                kind: UiModelTransportKind::ProviderRetry,
                detail: Some(error_center_provider_retry_detail(event)),
            }),
        }),
        ("provider", "failover_provider") => Some(ErrorCenterModelWaitingActivity {
            kind: UiModelRequestKind::Thinking,
            detail: Some("Waiting for model response.".to_owned()),
            transport: Some(UiModelTransportActivity {
                kind: UiModelTransportKind::ProviderFailover,
                detail: Some(format!(
                    "provider failover after {} at {}/{}",
                    event.code, event.retry_index, event.retry_cap
                )),
            }),
        }),
        ("schema", "repair_schema") => Some(ErrorCenterModelWaitingActivity {
            kind: UiModelRequestKind::SchemaRetry,
            detail: Some(schema_error_center_waiting_detail(event)),
            transport: None,
        }),
        _ => None,
    }
}

fn error_center_provider_retry_detail(event: &UiErrorCenterEventProjection) -> String {
    let base = format!(
        "provider retry {}/{}: {}",
        event.retry_index, event.retry_cap, event.code
    );
    match event
        .public_message
        .as_deref()
        .filter(|message| !message.is_empty())
    {
        Some(message) => format!("{base}; error: {message}; raw_hash={}", event.raw_hash),
        None => format!("{base}; raw_hash={}", event.raw_hash),
    }
}

fn schema_error_center_waiting_detail(event: &UiErrorCenterEventProjection) -> String {
    let fields = if event.repair_fields.is_empty() {
        event.code.clone()
    } else {
        event.repair_fields.join(", ")
    };
    format!("schema polishing #{}: {}", event.retry_index, fields)
}

fn project_error_center_event_for_ui(
    record: &MetadataEnvelope,
) -> Option<UiErrorCenterEventProjection> {
    Some(UiErrorCenterEventProjection {
        metadata_id: record.metadata_id.as_str().to_owned(),
        source_agent_id: record.subject.agent_id.clone(),
        session_id: record.subject.session_id.clone(),
        turn_id: record.subject.turn_id.clone(),
        trace_id: record.subject.trace_id.as_str().to_owned(),
        writer_feature_id: record.owner.feature_id.as_str().to_owned(),
        writer_crate: record.owner.crate_name.clone(),
        writer_symbol: record.owner.symbol_path.clone(),
        pipeline_node: record.write_node.pipeline_node.clone(),
        domain: metadata_entry_string(record, "error.domain")?,
        class: metadata_entry_string(record, "error.class")?,
        code: metadata_entry_string(record, "error.code")?,
        source_owner: metadata_entry_string(record, "error.source_owner")?,
        source_pipeline_node: metadata_entry_string(record, "error.source_pipeline_node")?,
        recovery_action: metadata_entry_string(record, "error.recovery_action")?,
        retry_index: metadata_entry_u64(record, "error.retry_index")?,
        retry_cap: metadata_entry_u64(record, "error.retry_cap")?,
        public_visibility: metadata_entry_string(record, "error.public_visibility")?,
        owner_target: metadata_entry_string(record, "error.owner_target")?,
        repair_fields: metadata_entry_string_array(record, "error.repair_fields")?,
        raw_hash: metadata_entry_string(record, "error.raw_hash")?,
        public_message: metadata_entry_string(record, "error.public_message"),
    })
}

fn metadata_entry_string(record: &MetadataEnvelope, key: &str) -> Option<String> {
    record
        .entries
        .iter()
        .find(|entry| entry.key == key)?
        .value
        .as_str()
        .map(str::to_owned)
}

fn metadata_entry_u64(record: &MetadataEnvelope, key: &str) -> Option<u64> {
    record
        .entries
        .iter()
        .find(|entry| entry.key == key)?
        .value
        .as_u64()
}

fn metadata_entry_string_array(record: &MetadataEnvelope, key: &str) -> Option<Vec<String>> {
    let value = &record.entries.iter().find(|entry| entry.key == key)?.value;
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .or_else(|| value.as_str().map(|item| vec![item.to_owned()]))
}

fn task_status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Created => "created",
        TaskStatus::WaitingAgent => "waiting_agent",
        TaskStatus::Assigned => "assigned",
        TaskStatus::Running => "running",
        TaskStatus::Interrupted => "interrupted",
        TaskStatus::Paused => "paused",
        TaskStatus::Blocked => "blocked",
        TaskStatus::ReviewSubmitted => "review_submitted",
        TaskStatus::Approved => "approved",
        TaskStatus::Rejected => "rejected",
        TaskStatus::Failed => "failed",
        TaskStatus::Cancelled => "cancelled",
        TaskStatus::Closed => "closed",
    }
}

fn agent_status_label(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Available => "available",
        AgentStatus::Busy => "busy",
        AgentStatus::Paused => "paused",
        AgentStatus::Offline => "offline",
        AgentStatus::Closing => "closing",
        AgentStatus::Closed => "closed",
        AgentStatus::Failed => "failed",
    }
}

fn task_dispatch_from_ui(dispatch: Option<UiTaskDispatchCommand>) -> TaskDispatchRequest {
    match dispatch {
        None | Some(UiTaskDispatchCommand::SelfAgent) => TaskDispatchRequest::SelfAgent,
        Some(UiTaskDispatchCommand::None) => TaskDispatchRequest::None,
        Some(UiTaskDispatchCommand::Agent { agent_id }) => TaskDispatchRequest::Agent { agent_id },
    }
}

fn ui_timer_schedule_to_runtime_request(
    timer: UiTimerScheduleCommand,
) -> Result<TimerScheduleRequest, UiCommandDispatchPortError> {
    let mode = match timer.mode.trim() {
        "relative" => TimerScheduleMode::Relative {
            delay_seconds: timer.delay_seconds.unwrap_or(0),
        },
        "absolute" => TimerScheduleMode::Absolute {
            run_at_unix_seconds: timer.run_at_unix_seconds.unwrap_or(0),
        },
        "recurring" => TimerScheduleMode::Recurring {
            repeat: ui_timer_repeat_to_runtime(timer.repeat.ok_or_else(|| {
                UiCommandDispatchPortError::DispatchFailed(
                    "timer recurring schedule requires repeat".to_owned(),
                )
            })?)?,
        },
        other => {
            return Err(UiCommandDispatchPortError::DispatchFailed(format!(
                "unsupported timer mode `{other}`"
            )));
        }
    };
    Ok(TimerScheduleRequest {
        timer_id: timer.timer_id,
        mode,
        reason: timer.reason,
        prompt: timer.prompt,
        max_runs: timer.max_runs,
        source_session_id: timer.source_session_id,
        source_turn_id: None,
        source_trace_id: None,
    })
}

fn ui_timer_repeat_to_runtime(
    repeat: UiTimerRepeatCommand,
) -> Result<TimerRepeatRule, UiCommandDispatchPortError> {
    match repeat {
        UiTimerRepeatCommand::Interval {
            interval_seconds,
            max_runs,
        } => Ok(TimerRepeatRule::Interval {
            interval_seconds,
            max_runs,
        }),
        UiTimerRepeatCommand::Daily {
            time_of_day_seconds_local,
            skip_weekends,
            max_runs,
        } => Ok(TimerRepeatRule::Daily {
            time_of_day_seconds_local,
            skip_weekends,
            max_runs,
        }),
        UiTimerRepeatCommand::Weekly {
            time_of_day_seconds_local,
            weekdays,
            max_runs,
        } => Ok(TimerRepeatRule::Weekly {
            time_of_day_seconds_local,
            weekdays,
            max_runs,
        }),
        UiTimerRepeatCommand::Cron {
            expression,
            max_runs,
        } => {
            parse_cron_expression(&expression)
                .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
            Ok(TimerRepeatRule::Cron {
                expression,
                max_runs,
            })
        }
    }
}

fn ui_worker_control_to_task_request(
    source_agent_id: &AgentId,
    control: UiWorkerControlCommand,
) -> Result<WorkerControlRequest, UiCommandDispatchPortError> {
    Ok(WorkerControlRequest {
        control_id: control.control_id,
        task_id: TaskId::new(control.task_id),
        execution_id: control.execution_id,
        agent_id: control.agent_id,
        op: worker_control_op_from_ui(&control.op)?,
        question: control.question,
        constraint: control.constraint,
        note: control.note,
        actor: ui_task_actor(source_agent_id, None, None),
        watermark: ui_task_watermark("worker_control"),
    })
}

fn worker_control_op_from_ui(op: &str) -> Result<WorkerControlOp, UiCommandDispatchPortError> {
    match op {
        "query_status" => Ok(WorkerControlOp::QueryStatus),
        "ask_at_safe_point" => Ok(WorkerControlOp::AskAtSafePoint),
        "add_constraint" => Ok(WorkerControlOp::AddConstraint),
        "request_checkpoint" => Ok(WorkerControlOp::RequestCheckpoint),
        "request_submission_now" => Ok(WorkerControlOp::RequestSubmissionNow),
        "pause" => Ok(WorkerControlOp::Pause),
        "resume" => Ok(WorkerControlOp::Resume),
        "cancel" => Ok(WorkerControlOp::Cancel),
        other => Err(UiCommandDispatchPortError::DispatchFailed(format!(
            "unknown worker control op `{other}`"
        ))),
    }
}

fn worker_control_op_label(op: &WorkerControlOp) -> &'static str {
    match op {
        WorkerControlOp::QueryStatus => "query_status",
        WorkerControlOp::AskAtSafePoint => "ask_at_safe_point",
        WorkerControlOp::AddConstraint => "add_constraint",
        WorkerControlOp::RequestCheckpoint => "request_checkpoint",
        WorkerControlOp::RequestSubmissionNow => "request_submission_now",
        WorkerControlOp::Pause => "pause",
        WorkerControlOp::Resume => "resume",
        WorkerControlOp::Cancel => "cancel",
    }
}

fn agent_lifecycle_state_label(state: &AgentLifecycleState) -> &'static str {
    match state {
        AgentLifecycleState::Idle => "idle",
        AgentLifecycleState::Assigned => "assigned",
        AgentLifecycleState::Running => "running",
        AgentLifecycleState::Progressing => "progressing",
        AgentLifecycleState::ModelThinking => "model_thinking",
        AgentLifecycleState::ToolRunning => "tool_running",
        AgentLifecycleState::Recovering => "recovering",
        AgentLifecycleState::Blocked => "blocked",
        AgentLifecycleState::WaitingReview => "waiting_review",
        AgentLifecycleState::Retrying => "retrying",
        AgentLifecycleState::Approved => "approved",
        AgentLifecycleState::Closed => "closed",
        AgentLifecycleState::Failed => "failed",
        AgentLifecycleState::Offline => "offline",
    }
}

fn remove_active_turn(
    active_turns: &mut Vec<ActiveRuntimeTurn>,
    turn_id: &TurnId,
) -> Option<ActiveRuntimeTurn> {
    let index = active_turns
        .iter()
        .position(|active| &active.turn_id == turn_id)?;
    Some(active_turns.remove(index))
}

fn active_runtime_turn_matches_master_work(
    active: &ActiveRuntimeTurn,
    checkpoint: &master_runner::MasterActiveWorkCheckpoint,
) -> bool {
    active.session_id == checkpoint.session_id
        && runtime_turn_position(&active.turn_id).0
            == runtime_turn_position(&checkpoint.logical_turn_id).0
        && active.trace_id == checkpoint.trace_id
}

fn turn_record_matches_master_work(
    turn: &TurnRecord,
    checkpoint: &master_runner::MasterActiveWorkCheckpoint,
) -> bool {
    turn.request.session_id == checkpoint.session_id
        && runtime_turn_position(&turn.request.turn_id).0
            == runtime_turn_position(&checkpoint.logical_turn_id).0
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

struct RuntimeMetadataWriteSpec<'a> {
    turn_id: Option<&'a TurnId>,
    trace_id: &'a TraceId,
    kind: MetadataKind,
    pipeline_node: &'a str,
    metadata_suffix: String,
    symbol_path: &'a str,
    entries: Vec<MetadataEntry>,
}

struct RuntimeControlHookWriteSpec<'a> {
    turn_id: Option<&'a TurnId>,
    trace_id: &'a TraceId,
    pipeline_node: &'a str,
    metadata_suffix: String,
    symbol_path: &'a str,
    entries: Vec<MetadataEntry>,
}

struct RuntimeErrorCenterWriteSpec<'a> {
    turn_id: Option<&'a TurnId>,
    trace_id: &'a TraceId,
    pipeline_node: &'a str,
    metadata_suffix: String,
    symbol_path: &'a str,
    observed: ErrorCenterObservedFailure,
}

struct RuntimeDebugEmitSpec<'a> {
    turn_id: &'a TurnId,
    trace_id: &'a TraceId,
    pipeline_node: &'a str,
    function: &'a str,
    status_text: &'a str,
    detail_lines: Vec<String>,
}

fn metadata_ledger_path(
    runtime_home: &Path,
    agent_id: &AgentId,
    session_id: &SessionId,
) -> PathBuf {
    runtime_home
        .join("ledgers")
        .join("metadata")
        .join(agent_id.as_str())
        .join(format!("{}.jsonl", session_id.as_str()))
}

fn write_control_hook_metadata(
    center: &Arc<Mutex<MetadataCenter>>,
    agent_id: &AgentId,
    session_id: &SessionId,
    spec: RuntimeControlHookWriteSpec<'_>,
) -> Result<(), RuntimeLiveBridgeError> {
    let envelope = MetadataEnvelope::new(
        MetadataId::new(format!(
            "control.center:{}:{}",
            spec.trace_id.as_str(),
            spec.metadata_suffix
        )),
        MetadataKind::RuntimeState,
        MetadataWriteOwner {
            feature_id: FeatureId::new("control.center"),
            crate_name: "freehand-control".to_owned(),
            module_path: "freehand_control".to_owned(),
            symbol_path: spec.symbol_path.to_owned(),
        },
        MetadataWriteNode {
            pipeline_node: spec.pipeline_node.to_owned(),
            runtime_node_id: None,
        },
        MetadataSubject {
            agent_id: Some(agent_id.clone()),
            session_id: Some(session_id.clone()),
            turn_id: spec.turn_id.cloned(),
            trace_id: spec.trace_id.clone(),
        },
        spec.entries,
    )
    .map_err(|err: MetadataError| RuntimeLiveBridgeError::MetadataFailed(err.to_string()))?;
    center
        .lock()
        .map_err(|err: std::sync::PoisonError<_>| {
            RuntimeLiveBridgeError::MetadataFailed(err.to_string())
        })?
        .write(envelope)
        .map_err(|err: MetadataError| RuntimeLiveBridgeError::MetadataFailed(err.to_string()))
}

fn write_error_center_metadata(
    center: &Arc<Mutex<MetadataCenter>>,
    agent_id: &AgentId,
    session_id: &SessionId,
    spec: RuntimeErrorCenterWriteSpec<'_>,
) -> Result<ErrorCenterDecision, RuntimeLiveBridgeError> {
    let decision = classify_error_center_failure(&spec.observed);
    let envelope = MetadataEnvelope::new(
        MetadataId::new(format!(
            "error.center:{}:{}",
            spec.trace_id.as_str(),
            spec.metadata_suffix
        )),
        MetadataKind::RuntimeState,
        MetadataWriteOwner {
            feature_id: FeatureId::new("error.center"),
            crate_name: "freehand-control".to_owned(),
            module_path: "freehand_control".to_owned(),
            symbol_path: spec.symbol_path.to_owned(),
        },
        MetadataWriteNode {
            pipeline_node: spec.pipeline_node.to_owned(),
            runtime_node_id: None,
        },
        MetadataSubject {
            agent_id: Some(agent_id.clone()),
            session_id: Some(session_id.clone()),
            turn_id: spec.turn_id.cloned(),
            trace_id: spec.trace_id.clone(),
        },
        vec![
            MetadataEntry {
                key: "error.domain".to_owned(),
                value: json!(decision.domain.as_str()),
            },
            MetadataEntry {
                key: "error.class".to_owned(),
                value: json!(decision.class.as_str()),
            },
            MetadataEntry {
                key: "error.code".to_owned(),
                value: json!(spec.observed.code),
            },
            MetadataEntry {
                key: "error.source_owner".to_owned(),
                value: json!(spec.observed.source_owner),
            },
            MetadataEntry {
                key: "error.source_pipeline_node".to_owned(),
                value: json!(spec.observed.source_pipeline_node),
            },
            MetadataEntry {
                key: "error.recovery_action".to_owned(),
                value: json!(decision.recovery_action.as_str()),
            },
            MetadataEntry {
                key: "error.retry_index".to_owned(),
                value: json!(decision.retry_index),
            },
            MetadataEntry {
                key: "error.retry_cap".to_owned(),
                value: json!(decision.retry_cap),
            },
            MetadataEntry {
                key: "error.public_visibility".to_owned(),
                value: json!(decision.public_visibility.as_str()),
            },
            MetadataEntry {
                key: "error.owner_target".to_owned(),
                value: json!(decision.owner_target),
            },
            MetadataEntry {
                key: "error.repair_fields".to_owned(),
                value: json!(decision.repair_fields),
            },
            MetadataEntry {
                key: "error.raw_hash".to_owned(),
                value: json!(fnv1a_hex(&spec.observed.message)),
            },
            MetadataEntry {
                key: "error.public_message".to_owned(),
                value: json!(public_error_center_message(&spec.observed.message)),
            },
        ],
    )
    .map_err(|err: MetadataError| RuntimeLiveBridgeError::MetadataFailed(err.to_string()))?;
    center
        .lock()
        .map_err(|err: std::sync::PoisonError<_>| {
            RuntimeLiveBridgeError::MetadataFailed(err.to_string())
        })?
        .write(envelope)
        .map_err(|err: MetadataError| RuntimeLiveBridgeError::MetadataFailed(err.to_string()))?;
    Ok(decision)
}

fn write_live_bridge_metadata(
    center: &Arc<Mutex<MetadataCenter>>,
    agent_id: &AgentId,
    session_id: &SessionId,
    spec: RuntimeMetadataWriteSpec<'_>,
) -> Result<(), RuntimeLiveBridgeError> {
    let envelope = MetadataEnvelope::new(
        MetadataId::new(format!(
            "{}:{}:{}",
            spec.trace_id.as_str(),
            spec.pipeline_node,
            spec.metadata_suffix
        )),
        spec.kind,
        MetadataWriteOwner {
            feature_id: FeatureId::new("provider.reason-live-bridge"),
            crate_name: "freehand-runtime".to_owned(),
            module_path: "freehand_runtime".to_owned(),
            symbol_path: spec.symbol_path.to_owned(),
        },
        MetadataWriteNode {
            pipeline_node: spec.pipeline_node.to_owned(),
            runtime_node_id: None,
        },
        MetadataSubject {
            agent_id: Some(agent_id.clone()),
            session_id: Some(session_id.clone()),
            turn_id: spec.turn_id.cloned(),
            trace_id: spec.trace_id.clone(),
        },
        spec.entries,
    )
    .map_err(|err: MetadataError| RuntimeLiveBridgeError::MetadataFailed(err.to_string()))?;
    center
        .lock()
        .map_err(|err: std::sync::PoisonError<_>| {
            RuntimeLiveBridgeError::MetadataFailed(err.to_string())
        })?
        .write(envelope)
        .map_err(|err: MetadataError| RuntimeLiveBridgeError::MetadataFailed(err.to_string()))
}

fn emit_live_bridge_debug(
    debug_hub: &DebugHub,
    agent_id: &AgentId,
    session_id: &SessionId,
    spec: RuntimeDebugEmitSpec<'_>,
) {
    let snapshot = DebugStateSnapshot::new(
        DebugSemanticPosition {
            feature_id: FeatureId::new("provider.reason-live-bridge"),
            session_id: session_id.clone(),
            turn_id: spec.turn_id.clone(),
            trace_id: spec.trace_id.clone(),
            agent_id: Some(agent_id.clone()),
            pipeline_node: Some(spec.pipeline_node.to_owned()),
        },
        DebugScenePosition {
            crate_name: "freehand-runtime".to_owned(),
            file: "src/lib.rs".to_owned(),
            function: spec.function.to_owned(),
            line: None,
            artifact_path: None,
            raw_exchange_id: None,
        },
        spec.status_text,
        spec.detail_lines,
    );
    let event = DebugEvent {
        envelope: DebugTraceEnvelope {
            semantic: snapshot.semantic.clone(),
            scene: snapshot.scene.clone(),
            input_hash: None,
            output_hash: None,
            artifact_path: snapshot.scene.artifact_path.clone(),
            timestamp: now_unix_seconds().to_string(),
        },
        snapshot: Some(snapshot),
    };
    let _ = debug_hub.emit(event);
}

struct LiveContextSegmentRecordScope<'a> {
    metadata_center: &'a Arc<Mutex<MetadataCenter>>,
    debug_hub: &'a DebugHub,
    debug_receiver: &'a Receiver<DebugEvent>,
    agent_id: &'a AgentId,
    session_id: &'a SessionId,
    turn_id: &'a TurnId,
    trace_id: &'a TraceId,
}

fn record_live_context_segment_build_event<F>(
    scope: &LiveContextSegmentRecordScope<'_>,
    on_debug: &mut F,
    event: LiveContextSegmentBuildEvent,
) -> Result<(), RuntimeLiveBridgeError>
where
    F: FnMut(&DebugEvent) + ?Sized,
{
    let pipeline_node = match event.status {
        live_context::LiveContextSegmentBuildStatus::Started => {
            "RuntimeLive01ContextSegmentStarted"
        }
        live_context::LiveContextSegmentBuildStatus::Completed => {
            "RuntimeLive01ContextSegmentCompleted"
        }
        live_context::LiveContextSegmentBuildStatus::Failed => "RuntimeLive01ContextSegmentFailed",
    };
    let mut entries = vec![
        MetadataEntry {
            key: "context.segment_id".to_owned(),
            value: json!(event.segment_id),
        },
        MetadataEntry {
            key: "context.segment_status".to_owned(),
            value: json!(event.status.as_str()),
        },
    ];
    if let Some(included) = event.included {
        entries.push(MetadataEntry {
            key: "context.segment_included".to_owned(),
            value: json!(included),
        });
    }
    if let Some(elapsed_ms) = event.elapsed_ms {
        entries.push(MetadataEntry {
            key: "context.segment_elapsed_ms".to_owned(),
            value: json!(u64::try_from(elapsed_ms).unwrap_or(u64::MAX)),
        });
    }
    write_live_bridge_metadata(
        scope.metadata_center,
        scope.agent_id,
        scope.session_id,
        RuntimeMetadataWriteSpec {
            turn_id: Some(scope.turn_id),
            trace_id: scope.trace_id,
            kind: MetadataKind::RuntimeState,
            pipeline_node,
            metadata_suffix: format!(
                "context_segment:{}:{}",
                event.segment_id,
                event.status.as_str()
            ),
            symbol_path: "live_context::base_live_context_segments",
            entries,
        },
    )?;

    let status_text = match (event.status, event.included) {
        (live_context::LiveContextSegmentBuildStatus::Started, _) => {
            "request context segment started"
        }
        (live_context::LiveContextSegmentBuildStatus::Completed, Some(false)) => {
            "request context segment skipped"
        }
        (live_context::LiveContextSegmentBuildStatus::Completed, _) => {
            "request context segment ready"
        }
        (live_context::LiveContextSegmentBuildStatus::Failed, _) => {
            "request context segment failed"
        }
    };
    let mut detail_lines = vec![
        format!("context_segment_id={}", event.segment_id),
        format!("context_segment_status={}", event.status.as_str()),
    ];
    if let Some(included) = event.included {
        detail_lines.push(format!("context_segment_included={included}"));
    }
    if let Some(elapsed_ms) = event.elapsed_ms {
        detail_lines.push(format!("context_segment_elapsed_ms={elapsed_ms}"));
    }
    emit_live_bridge_debug(
        scope.debug_hub,
        scope.agent_id,
        scope.session_id,
        RuntimeDebugEmitSpec {
            turn_id: scope.turn_id,
            trace_id: scope.trace_id,
            pipeline_node,
            function: "live_context::base_live_context_segments",
            status_text,
            detail_lines,
        },
    );
    drain_debug_events(scope.debug_receiver, on_debug);
    Ok(())
}

fn record_live_provider_raw(
    persistence: &ReasonPersistence,
    session_id: &SessionId,
    turn_id: &TurnId,
    trace_id: &TraceId,
    provider_family: ProviderFamily,
    raw: LiveProviderRawCapture<'_>,
) -> Result<(), RuntimeLiveBridgeError> {
    let (raw_kind, crate_name, function, raw_exchange_id, body, headers) = match raw {
        LiveProviderRawCapture::Response {
            body,
            crate_name,
            function,
        } => (
            "response_body",
            crate_name,
            function,
            Some("response-body".to_owned()),
            body.to_owned(),
            BTreeMap::new(),
        ),
        LiveProviderRawCapture::HttpError {
            status,
            body,
            crate_name,
            function,
        } => (
            "http_error_body",
            crate_name,
            function,
            Some(format!("http-status:{status}")),
            body.to_owned(),
            BTreeMap::from([("http-status".to_owned(), status.to_string())]),
        ),
        LiveProviderRawCapture::StreamEvent {
            event_index,
            event_body,
            crate_name,
            function,
        } => (
            "stream_event_body",
            crate_name,
            function,
            Some(format!("stream-event:{event_index}")),
            event_body.to_owned(),
            BTreeMap::from([("stream-event-index".to_owned(), event_index.to_string())]),
        ),
    };
    persistence
        .record_provider_raw_event(ProviderRawLedgerWrite {
            provider_family,
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            trace_id: trace_id.clone(),
            raw_kind: raw_kind.to_owned(),
            scene: ProviderRawScenePosition {
                crate_name: crate_name.to_owned(),
                file: "src/lib.rs".to_owned(),
                function: function.to_owned(),
                line: None,
                raw_exchange_id,
            },
            body,
            headers,
        })
        .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))
}

fn terminal_debug_details(
    round: usize,
    schema_rejections: usize,
    tool_executions: usize,
    status: freehand_contracts::TerminalStatus,
) -> Vec<String> {
    vec![
        format!("rounds={round}"),
        format!("schema_rejections={schema_rejections}"),
        format!("tool_executions={tool_executions}"),
        format!("terminal_status={status:?}"),
    ]
}

fn classify_anthropic_executor_error(err: &AnthropicExecutorError) -> ProviderExecutorErrorInfo {
    match err {
        AnthropicExecutorError::HttpStatus { status, body } => ProviderExecutorErrorInfo {
            code: format!("anthropic_http_status_{status}"),
            message: body.clone(),
            retryable: *status == 408
                || *status == 409
                || *status == 425
                || *status == 429
                || *status >= 500,
            failover_eligible: true,
        },
        AnthropicExecutorError::Http(err) => ProviderExecutorErrorInfo {
            code: "anthropic_http_request_failed".to_owned(),
            message: err.to_string(),
            retryable: err.is_connect() || err.is_timeout() || err.is_request(),
            failover_eligible: true,
        },
        AnthropicExecutorError::StreamRead(err) => ProviderExecutorErrorInfo {
            code: "anthropic_stream_read_failed".to_owned(),
            message: err.to_string(),
            retryable: true,
            failover_eligible: true,
        },
        AnthropicExecutorError::Adapter(err) => ProviderExecutorErrorInfo {
            code: "anthropic_adapter_failed".to_owned(),
            message: err.to_string(),
            retryable: false,
            failover_eligible: false,
        },
        AnthropicExecutorError::InvalidConfig => ProviderExecutorErrorInfo {
            code: "anthropic_invalid_config".to_owned(),
            message: err.to_string(),
            retryable: false,
            failover_eligible: false,
        },
        AnthropicExecutorError::Callback(message) => ProviderExecutorErrorInfo {
            code: "anthropic_callback_failed".to_owned(),
            message: message.clone(),
            retryable: false,
            failover_eligible: false,
        },
    }
}

fn classify_openai_executor_error(err: &OpenAiExecutorError) -> ProviderExecutorErrorInfo {
    match err {
        OpenAiExecutorError::HttpStatus { status, body } => ProviderExecutorErrorInfo {
            code: format!("openai_http_status_{status}"),
            message: body.clone(),
            retryable: *status == 408
                || *status == 409
                || *status == 425
                || *status == 429
                || *status >= 500,
            failover_eligible: true,
        },
        OpenAiExecutorError::Http(err) => ProviderExecutorErrorInfo {
            code: "openai_http_request_failed".to_owned(),
            message: err.to_string(),
            retryable: err.is_connect() || err.is_timeout() || err.is_request(),
            failover_eligible: true,
        },
        OpenAiExecutorError::StreamRead(err) => ProviderExecutorErrorInfo {
            code: "openai_stream_read_failed".to_owned(),
            message: err.to_string(),
            retryable: true,
            failover_eligible: true,
        },
        OpenAiExecutorError::Adapter(err) => ProviderExecutorErrorInfo {
            code: "openai_adapter_failed".to_owned(),
            message: err.to_string(),
            retryable: false,
            failover_eligible: false,
        },
        OpenAiExecutorError::InvalidConfig => ProviderExecutorErrorInfo {
            code: "openai_invalid_config".to_owned(),
            message: err.to_string(),
            retryable: false,
            failover_eligible: false,
        },
        OpenAiExecutorError::Callback(message) => ProviderExecutorErrorInfo {
            code: "openai_callback_failed".to_owned(),
            message: message.clone(),
            retryable: false,
            failover_eligible: false,
        },
    }
}

fn provider_executor_retry_plan() -> ProviderExecutorRetryPlan {
    let mut plan = ProviderExecutorRetryPlan::production();
    #[cfg(test)]
    {
        plan.initial_backoff_ms = 0;
        plan.max_backoff_ms = 0;
    }
    if let Ok(value) = env::var("FREEHAND_PROVIDER_RETRY_BACKOFF_MS")
        && let Ok(millis) = value.parse::<u64>()
    {
        plan.initial_backoff_ms = millis;
        plan.max_backoff_ms = millis;
    }
    plan
}

fn provider_retry_detail(
    error: &ProviderExecutorErrorInfo,
    retry_index: u32,
    retry_cap: u32,
    backoff: Option<Duration>,
) -> String {
    let wait = backoff
        .map(|duration| {
            format!(
                "; wait {} before internal resend",
                format_duration_for_ui(duration)
            )
        })
        .unwrap_or_default();
    format!(
        "provider retry {}/{}: {}{}; error: {}; raw_hash={}",
        retry_index,
        retry_cap,
        error.code,
        wait,
        public_error_center_message(&error.message),
        fnv1a_hex(&error.message)
    )
}

fn format_duration_for_ui(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1000 {
        return format!("{millis}ms");
    }
    let seconds = millis / 1000;
    let remainder_ms = millis % 1000;
    if remainder_ms == 0 {
        format!("{seconds}s")
    } else {
        format!("{}.{:03}s", seconds, remainder_ms)
    }
}

fn public_error_center_message(message: &str) -> String {
    const MAX_PUBLIC_ERROR_MESSAGE_CHARS: usize = 240;
    let compact = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let compact = provider_error_json_public_message(&compact).unwrap_or(compact);
    let mut public = compact
        .chars()
        .take(MAX_PUBLIC_ERROR_MESSAGE_CHARS)
        .collect::<String>();
    if compact.chars().count() > MAX_PUBLIC_ERROR_MESSAGE_CHARS {
        public.push_str("...");
    }
    public
}

fn provider_error_json_public_message(message: &str) -> Option<String> {
    let json_start = message.find('{')?;
    let value: serde_json::Value = serde_json::from_str(&message[json_start..]).ok()?;
    let error = value.get("error").unwrap_or(&value);
    let mut parts = Vec::new();
    if let Some(kind) = error
        .get("type")
        .or_else(|| error.get("code"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(kind.trim().to_owned());
    }
    if let Some(message) = error
        .get("message")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(message.trim().to_owned());
    }
    (!parts.is_empty()).then(|| parts.join(": "))
}

fn sleep_provider_retry(
    request: &LiveReasonTurnRequest,
    duration: Duration,
) -> Result<(), RuntimeLiveBridgeError> {
    if duration.is_zero() {
        return Ok(());
    }
    let mut remaining = duration;
    let quantum = Duration::from_millis(50);
    while !remaining.is_zero() {
        ensure_live_not_cancelled(request)?;
        let step = remaining.min(quantum);
        thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
    ensure_live_not_cancelled(request)
}

fn record_provider_error_metadata(
    spec: ProviderErrorMetadataSpec<'_>,
) -> Result<(), RuntimeLiveBridgeError> {
    write_error_center_metadata(
        spec.center,
        spec.agent_id,
        spec.session_id,
        RuntimeErrorCenterWriteSpec {
            turn_id: Some(&spec.turn.request.turn_id),
            trace_id: &spec.turn.request.trace_id,
            pipeline_node: spec.pipeline_node,
            metadata_suffix: format!("provider_error:{}", spec.retry_index),
            symbol_path: "run_live_provider_reason_turn",
            observed: ErrorCenterObservedFailure {
                source_owner: "provider.reason-live-bridge".to_owned(),
                source_pipeline_node: spec.pipeline_node.to_owned(),
                code: spec.error_code.to_owned(),
                message: spec.error.to_string(),
                retry_index: spec.retry_index,
                retry_cap: spec.retry_cap,
            },
        },
    )?;
    write_live_bridge_metadata(
        spec.center,
        spec.agent_id,
        spec.session_id,
        RuntimeMetadataWriteSpec {
            turn_id: Some(&spec.turn.request.turn_id),
            trace_id: &spec.turn.request.trace_id,
            kind: MetadataKind::Provider,
            pipeline_node: spec.pipeline_node,
            metadata_suffix: format!("provider_error:{}", spec.retry_index),
            symbol_path: "run_live_provider_reason_turn",
            entries: vec![
                MetadataEntry {
                    key: "error.kind".to_owned(),
                    value: json!("executor_failure"),
                },
                MetadataEntry {
                    key: "error.code".to_owned(),
                    value: json!(spec.error_code),
                },
                MetadataEntry {
                    key: "error.summary".to_owned(),
                    value: json!(spec.error.to_string()),
                },
                MetadataEntry {
                    key: "error.retry_index".to_owned(),
                    value: json!(spec.retry_index),
                },
                MetadataEntry {
                    key: "error.retry_cap".to_owned(),
                    value: json!(spec.retry_cap),
                },
            ],
        },
    )
}

struct ProviderExecutorFailureContext<'a> {
    engine: &'a ReasonTurnEngine,
    persistence: &'a ReasonPersistence,
    history: &'a SessionHistory,
    receiver: &'a Receiver<ReasonBroadcastEvent>,
    broadcasts: &'a mut Vec<ReasonBroadcastEvent>,
    on_broadcast: &'a mut dyn FnMut(&ReasonBroadcastEvent),
    debug_receiver: &'a Receiver<DebugEvent>,
    on_debug: &'a mut dyn FnMut(&DebugEvent),
    schema_rejection_count: u32,
    error_code: String,
}

fn materialize_provider_executor_failure(
    ctx: &mut ProviderExecutorFailureContext<'_>,
    turn: &mut TurnRecord,
    error: &RuntimeLiveBridgeError,
) -> Result<(), RuntimeLiveBridgeError> {
    let message = error.to_string();
    let output = ProviderSemanticOutput::Error(ErrorErr01RuntimeClassified {
        session_id: Some(turn.request.session_id.clone()),
        turn_id: Some(turn.request.turn_id.clone()),
        trace_id: turn.request.trace_id.clone(),
        feature_id: turn.request.feature_id.clone(),
        agent_id: Some(turn.request.agent_id.clone()),
        error: ErrorContract {
            code: ctx.error_code.clone(),
            class: ErrorClass::Upstream,
            recovery: RecoveryPolicy::Recoverable,
            message: message.clone(),
        },
    });
    ctx.engine
        .apply_provider_output(turn, output.clone())
        .map_err(|err| RuntimeLiveBridgeError::ProviderOutputApplyFailed(err.to_string()))?;
    ctx.persistence
        .record_provider_output_applied(ctx.history, turn, &output, ctx.schema_rejection_count)
        .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))?;
    drain_broadcasts(ctx.receiver, ctx.broadcasts, ctx.on_broadcast);
    drain_debug_events(ctx.debug_receiver, ctx.on_debug);

    ctx.engine.fail_turn(turn, message);
    drain_broadcasts(ctx.receiver, ctx.broadcasts, ctx.on_broadcast);
    drain_debug_events(ctx.debug_receiver, ctx.on_debug);
    ctx.persistence
        .record_turn_closed(ctx.history, turn, ctx.schema_rejection_count)
        .map(|_| ())
        .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))
}

fn emit_provider_retry_debug(
    debug_hub: &DebugHub,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn: &TurnRecord,
    error: &ProviderExecutorErrorInfo,
    retry_index: u32,
    retry_cap: u32,
    backoff: Duration,
) {
    let detail = provider_retry_detail(error, retry_index, retry_cap, Some(backoff));
    emit_live_bridge_debug(
        debug_hub,
        agent_id,
        session_id,
        RuntimeDebugEmitSpec {
            turn_id: &turn.request.turn_id,
            trace_id: &turn.request.trace_id,
            pipeline_node: "RuntimeLive05ProviderError",
            function: "run_live_provider_reason_turn",
            status_text: &detail,
            detail_lines: vec![
                format!("error_code={}", error.code),
                format!(
                    "error_message={}",
                    public_error_center_message(&error.message)
                ),
                format!("error_hash={}", fnv1a_hex(&error.message)),
                format!("retry_index={retry_index}"),
                format!("retry_cap={retry_cap}"),
                format!("retryable={}", error.retryable),
                format!("backoff_ms={}", backoff.as_millis()),
                "retry_scope=internal_provider_resend".to_owned(),
            ],
        },
    );
}

fn live_is_cancelled(request: &LiveReasonTurnRequest) -> bool {
    request
        .cancel_token
        .as_ref()
        .is_some_and(|token| token.load(Ordering::SeqCst))
}

fn ensure_live_not_cancelled(
    request: &LiveReasonTurnRequest,
) -> Result<(), RuntimeLiveBridgeError> {
    if live_is_cancelled(request) {
        return Err(RuntimeLiveBridgeError::Cancelled);
    }
    Ok(())
}

fn provider_descriptor(
    provider: &SelectedProviderConfig,
) -> Result<ProviderDescriptor, RuntimeLiveBridgeError> {
    let (family, protocol) = match (provider.provider_type, provider.protocol) {
        (ProviderType::Anthropic, ConfigProviderProtocol::Messages) => (
            ProviderFamily::Anthropic,
            ProviderProtocol::AnthropicMessages,
        ),
        (ProviderType::OpenAi, ConfigProviderProtocol::Responses) => (
            ProviderFamily::OpenAiCompatible,
            ProviderProtocol::OpenAiResponses,
        ),
        (ProviderType::OpenAi, ConfigProviderProtocol::ChatCompletions) => (
            ProviderFamily::OpenAiCompatible,
            ProviderProtocol::OpenAiChatCompletions,
        ),
        _ => {
            return Err(RuntimeLiveBridgeError::UnsupportedLiveProvider {
                provider: provider.provider_type.as_str().to_owned(),
                protocol: provider.protocol.as_str().to_owned(),
            });
        }
    };
    Ok(ProviderDescriptor {
        provider_name: provider.id.clone(),
        family,
        protocol,
        model: provider.default_model.clone(),
        capabilities: ProviderCapabilities {
            web_search: provider_web_search_capability(provider),
            multimodal: provider_multimodal_capability(provider),
            vision: provider_vision_capability(provider),
            reasoning: true,
        },
    })
}

fn provider_web_search_capability(
    provider: &SelectedProviderConfig,
) -> ProviderWebSearchCapability {
    provider_web_search_capability_from_parts(
        provider.provider_type,
        provider.protocol,
        provider.web_search,
    )
}

fn provider_web_search_capability_from_parts(
    provider_type: ProviderType,
    protocol: ConfigProviderProtocol,
    mode: ProviderWebSearchMode,
) -> ProviderWebSearchCapability {
    if mode == ProviderWebSearchMode::Disabled {
        return ProviderWebSearchCapability::Unsupported;
    }
    match (provider_type, protocol) {
        (ProviderType::OpenAi, ConfigProviderProtocol::Responses)
        | (ProviderType::Anthropic, ConfigProviderProtocol::Messages) => {
            ProviderWebSearchCapability::hosted_live_with_functions()
        }
        _ => ProviderWebSearchCapability::Unsupported,
    }
}

fn provider_web_search_route_guidance(
    selected: &SelectedAgentConfig,
    selected_descriptor: &ProviderDescriptor,
) -> String {
    let selected_mode = selected.provider.web_search.as_str();
    let (selected_effective, selected_reason) = provider_web_search_effective_status(
        &selected.provider.id,
        selected.provider.provider_type,
        selected.provider.protocol,
        selected.provider.web_search,
    );
    let worker_routes = provider_web_search_worker_routes(selected);
    let route_line = if selected_descriptor
        .capabilities
        .web_search
        .can_mix_with_function_tools()
    {
        format!(
            "current Master provider `{}` will declare provider-hosted web_search in this request",
            selected.provider.id
        )
    } else if worker_routes.is_empty() {
        "no configured Worker clean_search route currently has verified hosted web_search"
            .to_owned()
    } else {
        format!(
            "configured Worker clean_search route available via {}; create/assign a task with execution_profile=\"clean_search\" for broad/current search, then review Worker evidence",
            worker_routes.join(", ")
        )
    };
    format!(
        "Web Search Route Status (runtime truth): selected provider `{}` ({}/{}/{}) configured web_search={} effective={} reason=\"{}\". A local Freehand function tool named `web_search` is never exposed. Fallback provider is not automatic capability fallback for this Master turn. Route: {}.",
        selected.provider.id,
        selected.provider.provider_type.as_str(),
        selected.provider.protocol.as_str(),
        selected.provider.default_model,
        selected_mode,
        selected_effective,
        selected_reason,
        route_line
    )
}

fn provider_web_search_worker_routes(selected: &SelectedAgentConfig) -> Vec<String> {
    selected
        .worker_peers()
        .filter_map(|peer| {
            provider_for_peer_id(selected, &peer.provider_id).map(|provider| (peer, provider))
        })
        .filter(|(_, provider)| provider_web_search_capability(provider).is_hosted())
        .map(|(peer, provider)| {
            format!(
                "`{}` using `{}` ({}/{})",
                peer.name,
                provider.id,
                provider.protocol.as_str(),
                provider.default_model
            )
        })
        .collect()
}

fn provider_for_peer_id<'a>(
    selected: &'a SelectedAgentConfig,
    provider_id: &str,
) -> Option<&'a SelectedProviderConfig> {
    if selected.provider.id == provider_id {
        return Some(&selected.provider);
    }
    selected
        .fallback_provider
        .as_ref()
        .filter(|provider| provider.id == provider_id)
}

fn provider_vision_capability(provider: &SelectedProviderConfig) -> bool {
    let model = provider.default_model.trim().to_ascii_lowercase();
    match provider.provider_type {
        ProviderType::OpenAi => {
            model.starts_with("gpt-5")
                || model.starts_with("gpt-4.1")
                || model.starts_with("gpt-4o")
                || model.contains("vision")
        }
        ProviderType::Anthropic => {
            model.contains("claude")
                || model.starts_with("minimax-m")
                || model.starts_with("minimax-")
        }
    }
}

fn provider_multimodal_capability(provider: &SelectedProviderConfig) -> bool {
    provider_vision_capability(provider)
}

#[derive(Debug, Clone, Copy)]
struct LiveProviderLabel {
    family: &'static str,
    protocol: &'static str,
    display: &'static str,
}

fn live_provider_label(provider: &SelectedProviderConfig) -> LiveProviderLabel {
    match (provider.provider_type, provider.protocol) {
        (ProviderType::Anthropic, ConfigProviderProtocol::Messages) => LiveProviderLabel {
            family: "anthropic",
            protocol: "messages",
            display: "anthropic/messages",
        },
        (ProviderType::OpenAi, ConfigProviderProtocol::Responses) => LiveProviderLabel {
            family: "openai",
            protocol: "responses",
            display: "openai/responses",
        },
        (ProviderType::OpenAi, ConfigProviderProtocol::ChatCompletions) => LiveProviderLabel {
            family: "openai",
            protocol: "chat_completions",
            display: "openai/chat_completions",
        },
        _ => LiveProviderLabel {
            family: "unsupported",
            protocol: "unsupported",
            display: "unsupported/unsupported",
        },
    }
}

enum LiveProviderRawCapture<'a> {
    Response {
        crate_name: &'static str,
        function: &'static str,
        body: &'a str,
    },
    HttpError {
        crate_name: &'static str,
        function: &'static str,
        status: u16,
        body: &'a str,
    },
    StreamEvent {
        crate_name: &'static str,
        function: &'static str,
        event_index: usize,
        event_body: &'a str,
    },
}

#[derive(Debug)]
struct LiveProviderDriverError {
    info: ProviderExecutorErrorInfo,
}

impl LiveProviderDriverError {
    fn info(&self) -> &ProviderExecutorErrorInfo {
        &self.info
    }
}

trait LiveProviderDriver {
    fn execute_once_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(LiveProviderRawCapture<'_>) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, LiveProviderDriverError>;

    fn execute_stream_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(LiveProviderRawCapture<'_>) -> Result<(), String>,
        on_outputs: &mut dyn FnMut(&[ProviderSemanticOutput]) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, LiveProviderDriverError>;
}

struct AnthropicLiveProviderDriver {
    executor: AnthropicExecutor,
}

struct OpenAiLiveProviderDriver {
    executor: OpenAiExecutor,
}

impl LiveProviderDriver for AnthropicLiveProviderDriver {
    fn execute_once_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(LiveProviderRawCapture<'_>) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, LiveProviderDriverError> {
        self.executor
            .execute_once_with_raw(ctx, request, |raw| {
                on_raw(live_raw_from_anthropic(raw)).map_err(AnthropicExecutorError::Callback)
            })
            .map_err(|err| LiveProviderDriverError {
                info: classify_anthropic_executor_error(&err),
            })
    }

    fn execute_stream_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(LiveProviderRawCapture<'_>) -> Result<(), String>,
        on_outputs: &mut dyn FnMut(&[ProviderSemanticOutput]) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, LiveProviderDriverError> {
        self.executor
            .execute_stream_with_raw(
                ctx,
                request,
                |raw| {
                    on_raw(live_raw_from_anthropic(raw)).map_err(AnthropicExecutorError::Callback)
                },
                |batch| on_outputs(batch).map_err(AnthropicExecutorError::Callback),
            )
            .map_err(|err| LiveProviderDriverError {
                info: classify_anthropic_executor_error(&err),
            })
    }
}

impl LiveProviderDriver for OpenAiLiveProviderDriver {
    fn execute_once_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(LiveProviderRawCapture<'_>) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, LiveProviderDriverError> {
        self.executor
            .execute_once_with_raw(ctx, request, |raw| {
                on_raw(live_raw_from_openai(raw)).map_err(OpenAiExecutorError::Callback)
            })
            .map_err(|err| LiveProviderDriverError {
                info: classify_openai_executor_error(&err),
            })
    }

    fn execute_stream_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(LiveProviderRawCapture<'_>) -> Result<(), String>,
        on_outputs: &mut dyn FnMut(&[ProviderSemanticOutput]) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, LiveProviderDriverError> {
        self.executor
            .execute_stream_with_raw(
                ctx,
                request,
                |raw| on_raw(live_raw_from_openai(raw)).map_err(OpenAiExecutorError::Callback),
                |batch| on_outputs(batch).map_err(OpenAiExecutorError::Callback),
            )
            .map_err(|err| LiveProviderDriverError {
                info: classify_openai_executor_error(&err),
            })
    }
}

fn build_live_provider_driver(
    provider: &SelectedProviderConfig,
) -> Result<Box<dyn LiveProviderDriver>, RuntimeLiveBridgeError> {
    match (provider.provider_type, provider.protocol) {
        (ProviderType::Anthropic, ConfigProviderProtocol::Messages) => {
            let executor = AnthropicExecutor::new(AnthropicExecutorConfig {
                base_url: provider.base_url.clone(),
                api_key: provider.api_key.clone(),
                anthropic_version: "2023-06-01".to_owned(),
                adapter: AnthropicAdapterConfig {
                    max_tokens: DEFAULT_ANTHROPIC_MAX_TOKENS,
                },
            })
            .map_err(|err| {
                RuntimeLiveBridgeError::ProviderExecutorFailed(
                    classify_anthropic_executor_error(&err).terminal_message(),
                )
            })?;
            Ok(Box::new(AnthropicLiveProviderDriver { executor }))
        }
        (ProviderType::OpenAi, ConfigProviderProtocol::Responses)
        | (ProviderType::OpenAi, ConfigProviderProtocol::ChatCompletions) => {
            let executor = OpenAiExecutor::new(OpenAiExecutorConfig {
                base_url: provider.base_url.clone(),
                api_key: provider.api_key.clone(),
            })
            .map_err(|err| {
                RuntimeLiveBridgeError::ProviderExecutorFailed(
                    classify_openai_executor_error(&err).terminal_message(),
                )
            })?;
            Ok(Box::new(OpenAiLiveProviderDriver { executor }))
        }
        _ => Err(RuntimeLiveBridgeError::UnsupportedLiveProvider {
            provider: provider.provider_type.as_str().to_owned(),
            protocol: provider.protocol.as_str().to_owned(),
        }),
    }
}

fn live_raw_from_anthropic(raw: &AnthropicRawCapture) -> LiveProviderRawCapture<'_> {
    match raw {
        AnthropicRawCapture::ResponseBody { body } => LiveProviderRawCapture::Response {
            crate_name: "freehand-provider-anthropic",
            function: "AnthropicExecutor::execute_once_with_raw",
            body,
        },
        AnthropicRawCapture::HttpErrorBody { status, body } => LiveProviderRawCapture::HttpError {
            crate_name: "freehand-provider-anthropic",
            function: "AnthropicExecutor::send_rendered_request",
            status: *status,
            body,
        },
        AnthropicRawCapture::StreamEventBody {
            event_index,
            event_body,
        } => LiveProviderRawCapture::StreamEvent {
            crate_name: "freehand-provider-anthropic",
            function: "AnthropicExecutor::execute_stream_with_raw",
            event_index: *event_index,
            event_body,
        },
    }
}

fn live_raw_from_openai(raw: &OpenAiRawCapture) -> LiveProviderRawCapture<'_> {
    match raw {
        OpenAiRawCapture::ResponseBody { body } => LiveProviderRawCapture::Response {
            crate_name: "freehand-provider-openai",
            function: "OpenAiExecutor::execute_once_with_raw",
            body,
        },
        OpenAiRawCapture::HttpErrorBody { status, body } => LiveProviderRawCapture::HttpError {
            crate_name: "freehand-provider-openai",
            function: "OpenAiExecutor::send_rendered_request",
            status: *status,
            body,
        },
        OpenAiRawCapture::StreamEventBody {
            event_index,
            event_body,
        } => LiveProviderRawCapture::StreamEvent {
            crate_name: "freehand-provider-openai",
            function: "OpenAiExecutor::execute_stream_with_raw",
            event_index: *event_index,
            event_body,
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

#[derive(Clone, Copy)]
struct LiveRoundContext<'a> {
    role: LiveReasonExecutionRole,
    execution_profile: LiveReasonExecutionProfile,
    configured_worker_set: Option<&'a [String]>,
    web_search_route_guidance: Option<&'a str>,
    runtime_home: &'a Path,
    cwd: Option<&'a Path>,
    agent_id: &'a AgentId,
}

fn next_round_segments(
    original_prompt: &str,
    visible_text: &str,
    rejection_feedback: Option<&str>,
    context: LiveRoundContext<'_>,
) -> Result<Vec<ContextSegment>, RuntimeLiveBridgeError> {
    let mut segments = base_live_context_segments(
        original_prompt,
        context.role,
        context.execution_profile,
        context.configured_worker_set,
        context.web_search_route_guidance,
        context.runtime_home,
        context.cwd,
        context.agent_id,
    )?;
    if !visible_text.trim().is_empty() {
        let content = format!("Previous round visible output:\n{visible_text}");
        segments.push(ContextSegment {
            segment_id: ContextSegmentId::new("previous-visible-output"),
            kind: ContextSegmentKind::SubagentConclusion,
            stability: ContextStability::TurnVolatile,
            cache_policy: ContextCachePolicy::NoCache,
            role: ContextRole::Developer,
            token_budget: runtime_prompt_segment_token_budget(&content),
            content,
            provenance: ContextProvenance {
                source: "freehand_runtime".to_owned(),
                reference: Some("previous_visible_output".to_owned()),
            },
        });
    }
    if let Some(feedback) = rejection_feedback {
        let content = format!("Completion schema rejection feedback:\n{feedback}");
        segments.push(ContextSegment {
            segment_id: ContextSegmentId::new("completion-schema-feedback"),
            kind: ContextSegmentKind::SubagentConclusion,
            stability: ContextStability::TurnVolatile,
            cache_policy: ContextCachePolicy::NoCache,
            role: ContextRole::Developer,
            token_budget: runtime_prompt_segment_token_budget(&content),
            content,
            provenance: ContextProvenance {
                source: "freehand_runtime".to_owned(),
                reference: Some("completion_schema_feedback".to_owned()),
            },
        });
    }
    Ok(segments)
}

fn attention_resolution_segment(
    resolution: &master_runner::MasterAttentionResolution,
) -> Result<ContextSegment, RuntimeLiveBridgeError> {
    let resolution_json = serde_json::to_string_pretty(resolution)
        .map_err(|err| RuntimeLiveBridgeError::MasterWorkStateFailed(err.to_string()))?;
    let content = format!(
        "Master attention resolution for the original foreground work. Re-evaluate the original objective against the refreshed TaskSpaceSnapshot. Do not reuse any provider tool call or terminal candidate created before this resolution.\n<freehand_attention_resolution>\n{resolution_json}\n</freehand_attention_resolution>"
    );
    Ok(ContextSegment {
        segment_id: ContextSegmentId::new(format!(
            "attention-resolution:{}",
            sanitize_identifier(&resolution.attention_event_id)
        )),
        kind: ContextSegmentKind::AttentionResolution,
        stability: ContextStability::TurnVolatile,
        cache_policy: ContextCachePolicy::NoCache,
        role: ContextRole::Developer,
        token_budget: runtime_prompt_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "master_work.attention_resolution".to_owned(),
            reference: Some(resolution.attention_event_id.clone()),
        },
    })
}

fn admit_master_attention_resolution_for_next_round(
    segments: &mut Vec<ContextSegment>,
    resolution: &master_runner::MasterAttentionResolution,
    context: LiveRoundContext<'_>,
) -> Result<(), RuntimeLiveBridgeError> {
    segments.retain(|segment| {
        segment.kind != ContextSegmentKind::TaskSpaceSnapshot
            && segment.kind != ContextSegmentKind::AttentionResolution
    });
    if let Some(snapshot) = task_space_snapshot_segment(
        context.runtime_home,
        context.agent_id,
        context.role,
        context.configured_worker_set,
    )? {
        segments.push(snapshot);
    }
    segments.push(attention_resolution_segment(resolution)?);
    Ok(())
}

fn master_attention_continuation_prompt() -> String {
    "Master attention changed authoritative task truth. Continue the original foreground objective using the typed AttentionResolution and refreshed TaskSpaceSnapshot. Re-evaluate before choosing any tool or terminal completion."
        .to_owned()
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

fn pending_tool_calls_for_execution(
    turn: &TurnRecord,
    executed_tool_call_ids: &[String],
) -> Vec<ReasonReq04ToolCall> {
    let mut ordered_ids = Vec::<String>::new();
    let mut latest_by_id = BTreeMap::<String, ReasonReq04ToolCall>::new();
    turn.tool_calls
        .iter()
        .filter(|call| {
            !executed_tool_call_ids
                .iter()
                .any(|id| id == call.tool_call.tool_call_id.as_str())
        })
        .for_each(|call| {
            let id = call.tool_call.tool_call_id.as_str().to_owned();
            if !latest_by_id.contains_key(&id) {
                ordered_ids.push(id.clone());
            }
            let replace = latest_by_id
                .get(&id)
                .map(|existing| {
                    !existing.tool_call.arguments_complete || call.tool_call.arguments_complete
                })
                .unwrap_or(true);
            if replace {
                latest_by_id.insert(id, call.clone());
            }
        });
    ordered_ids
        .into_iter()
        .filter_map(|id| latest_by_id.remove(&id))
        .collect()
}

fn latest_finish_reason(turn: &TurnRecord) -> Option<&str> {
    turn.usage_events
        .iter()
        .rev()
        .find_map(|event| event.usage.finish_reason.as_deref())
}

fn turn_has_completion_candidate_finish_reason(turn: &TurnRecord) -> bool {
    latest_finish_reason(turn).is_some_and(|reason| {
        matches!(
            reason,
            "stop" | "end_turn" | "completed" | "complete" | "success"
        )
    })
}

fn execute_registry_tool_call(
    registry: &BuiltinToolRegistry,
    runtime_home: &Path,
    workspace_root: Option<&Path>,
    role: LiveReasonExecutionRole,
    configured_worker_set: Option<&[String]>,
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<ExecutedToolResult, RuntimeLiveBridgeError> {
    if !tool_call.tool_call.arguments_complete {
        return Ok(ExecutedToolResult {
            result: tool_result_reentry(
                turn,
                tool_call,
                ToolResultStatus::Failed,
                "Tool execution failed: cannot execute incomplete tool arguments".to_owned(),
            ),
            task_truth_changed: false,
        });
    }
    let tool_name = tool_call.tool_call.tool_name.as_str();
    let root = match role {
        LiveReasonExecutionRole::Master => {
            if tool_name == "task" {
                return execute_registry_tool_call_with_workspace(
                    registry,
                    runtime_home,
                    runtime_home,
                    role,
                    configured_worker_set,
                    turn,
                    tool_call,
                );
            }
            match registry.execution_scope(tool_name) {
                Some(BuiltinToolExecutionScope::Workspace) => workspace_root
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| runtime_home.to_path_buf()),
                Some(BuiltinToolExecutionScope::Framework) if tool_name == "timer" => {
                    runtime_home.to_path_buf()
                }
                Some(BuiltinToolExecutionScope::Network) => runtime_home.to_path_buf(),
                _ => {
                    return Ok(master_capability_boundary_result(
                        turn,
                        tool_call,
                        configured_worker_set,
                    ));
                }
            }
        }
        LiveReasonExecutionRole::Worker => {
            if tool_name == "task" {
                return Ok(ExecutedToolResult {
                    result: tool_result_reentry(
                        turn,
                        tool_call,
                        ToolResultStatus::Failed,
                        "Worker capability boundary: recursive task management is not available. Complete only the assigned task inside the locked workspace.".to_owned(),
                    ),
                    task_truth_changed: false,
                });
            }
            if tool_name == "timer" {
                return Ok(ExecutedToolResult {
                    result: tool_result_reentry(
                        turn,
                        tool_call,
                        ToolResultStatus::Failed,
                        "Worker capability boundary: internal timer scheduling is only available to the Master."
                            .to_owned(),
                    ),
                    task_truth_changed: false,
                });
            }
            if registry.execution_scope(tool_name) == Some(BuiltinToolExecutionScope::Shell) {
                return Ok(ExecutedToolResult {
                    result: tool_result_reentry(
                        turn,
                        tool_call,
                        ToolResultStatus::Failed,
                        format!(
                            "Worker capability boundary: shell execution is not available because write intent cannot be reliably bounded to the worker task cwd. Available Worker tools are exactly: {}. Do not call shell, bash, readlink, pwd, cat, find, python, or any unlisted tool. Use governed workspace tools only inside the locked task cwd.",
                            worker_tool_surface_label()
                        ),
                    ),
                    task_truth_changed: false,
                });
            }
            let requested_root =
                workspace_root.ok_or(RuntimeLiveBridgeError::WorkerWorkspaceRequired)?;
            fs::canonicalize(requested_root).map_err(|err| {
                RuntimeLiveBridgeError::ToolExecutionFailed(format!(
                    "cannot canonicalize worker workspace `{}`: {err}",
                    requested_root.display()
                ))
            })?
        }
    };
    let root = fs::canonicalize(&root).map_err(|err| {
        RuntimeLiveBridgeError::ToolExecutionFailed(format!(
            "cannot canonicalize workspace `{}`: {err}",
            root.display()
        ))
    })?;
    with_workspace_root(&root, || {
        execute_registry_tool_call_with_workspace(
            registry,
            runtime_home,
            &root,
            role,
            configured_worker_set,
            turn,
            tool_call,
        )
    })
    .map_err(|err| RuntimeLiveBridgeError::ToolExecutionFailed(err.to_string()))?
}

fn master_capability_boundary_result(
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
    configured_worker_set: Option<&[String]>,
) -> ExecutedToolResult {
    let worker = configured_worker_label(configured_worker_set);
    ExecutedToolResult {
        result: tool_result_reentry(
            turn,
            tool_call,
            ToolResultStatus::Failed,
            format!(
                "Master capability boundary: `{}` is not available to the Master live tool surface. The Master may use local workspace tools (`ls`, `read_file`, `grep`, `glob`, `write_file`, `edit_file`, `multi_edit`, `delete_range`), network tool `web_fetch`, plus `task` and `timer`; shell, browser, broad web_search, todo_write, and complete_step are not available. For a different cwd, isolated long-running work, or parallel work, create a Worker task with task({{\"op\":\"create\", \"target_cwd\":\"<existing repository cwd>\", \"dispatch\":{{\"mode\":\"none\"}}}}), then task({{\"op\":\"assign\", \"agent_id\":\"{worker}\"}}). If a configured Worker has the needed capability, dispatch instead of blocking. No file content was read or written by this rejected Master call.",
                tool_call.tool_call.tool_name
            ),
        ),
        task_truth_changed: false,
    }
}

fn worker_tool_surface_label() -> String {
    BuiltinToolRegistry::reasonix_aligned()
        .worker_implemented_definitions()
        .into_iter()
        .map(|definition| format!("`{}`", definition.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn registry_error_text(role: LiveReasonExecutionRole, error: &ToolRegistryError) -> String {
    match error {
        ToolRegistryError::UnknownTool(name) => match role {
            LiveReasonExecutionRole::Master => format!(
                "Tool execution failed: unknown tool `{name}` for Master. The Master live tool surface is local workspace tools (`ls`, `read_file`, `grep`, `glob`, `write_file`, `edit_file`, `multi_edit`, `delete_range`), `web_fetch` for known HTTP/HTTPS URLs, plus `task` and `timer`; shell, browser, broad web_search, todo_write, and complete_step are not available. Use `web_fetch` for concrete URLs, local workspace tools for the current selected cwd, or delegate different-cwd/isolated/capability-matched work through exact task JSON: task({{\"op\":\"create\",\"target_cwd\":\"/absolute/existing/workspace\",\"dispatch\":{{\"mode\":\"none\"}}}}) plus task({{\"op\":\"assign\",\"task_id\":\"...\",\"agent_id\":\"<configured Worker>\"}})."
            ),
            LiveReasonExecutionRole::Worker => format!(
                "Tool execution failed: unknown tool `{name}` for Worker. Available Worker tools are exactly: {}. Do not call shell, bash, readlink, pwd, cat, find, python, or any unlisted tool. Use `ls`, `read_file`, `grep`, and `glob` for repository inspection; use owner-rendered `path_diagnostic` for symlink or missing-path evidence.",
                worker_tool_surface_label()
            ),
        },
        ToolRegistryError::UnimplementedTool(name) => match role {
            LiveReasonExecutionRole::Master => format!(
                "Tool execution failed: `{name}` is not implemented for the Master. The Master live tool surface is local workspace tools plus `web_fetch`, `task`, and `timer`; dispatch to a configured Worker when its advertised capability surface can complete the slice."
            ),
            LiveReasonExecutionRole::Worker => format!(
                "Tool execution failed: `{name}` is registered but not implemented for Worker use. Available Worker tools are exactly: {}. Do not switch to shell/readlink; continue with the implemented workspace tools or return blocked with evidence.",
                worker_tool_surface_label()
            ),
        },
        ToolRegistryError::WorkspaceBoundaryViolation {
            tool,
            field,
            root,
            target,
        } => match role {
            LiveReasonExecutionRole::Master => format!(
                "Workspace boundary denied: `{tool}.{field}` targeted `{target}` outside the current agent cwd `{root}`. This is a Master scope/permission boundary, not evidence that `{target}` is missing. Confirm the correct existing cwd for that work, then delegate with exact task JSON: task({{\"op\":\"create_agent\",\"agent_id\":\"<new-worker-id>\",\"capabilities\":[\"repository\"]}}) only when no configured Worker exists, task({{\"op\":\"create\",\"target_cwd\":\"<existing target workspace cwd>\",\"dispatch\":{{\"mode\":\"none\"}}}}), and task({{\"op\":\"assign\",\"task_id\":\"...\",\"agent_id\":\"<configured Worker>\"}})."
            ),
            LiveReasonExecutionRole::Worker
                if matches!(tool.as_str(), "read_file" | "grep" | "glob" | "ls") =>
            {
                format!(
                    "Workspace boundary denied: `{tool}.{field}` targeted `{target}` outside the worker task cwd `{root}`. Worker path tools are locked to the task cwd after absolute-normalization and symlink/canonical resolution. Use relative paths inside the task cwd; absolute or leading-~ paths are valid only when they canonicalize under that cwd. Do not probe `/Users`, home directories, parent directories, `/tmp`, sibling repos, or external roots. For symlink or missing-path evidence, use the owner-rendered path_diagnostic from path tools, not readlink or shell. If the required path is outside this task cwd, return blocked with the required target workspace cwd so the Master can delegate correctly."
                )
            }
            LiveReasonExecutionRole::Worker => format!(
                "Write boundary denied: `{tool}.{field}` targeted `{target}` outside the worker task cwd `{root}`. Worker mutation tools are locked to the task cwd after absolute-normalization and symlink/canonical resolution. Use relative paths inside the task cwd; if the required write target is outside this task cwd, return blocked with the required target workspace cwd so the Master can delegate correctly."
            ),
        },
        _ => format!("Tool execution failed: {error}"),
    }
}

fn execute_registry_tool_call_with_workspace(
    registry: &BuiltinToolRegistry,
    runtime_home: &Path,
    workspace_root: &Path,
    role: LiveReasonExecutionRole,
    configured_worker_set: Option<&[String]>,
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<ExecutedToolResult, RuntimeLiveBridgeError> {
    let tool_name = tool_call.tool_call.tool_name.as_str();
    if tool_name == "timer" {
        if role == LiveReasonExecutionRole::Worker {
            return Ok(ExecutedToolResult {
                result: tool_result_reentry(
                    turn,
                    tool_call,
                    ToolResultStatus::Failed,
                    "Worker capability boundary: internal timer scheduling is only available to the Master."
                        .to_owned(),
                ),
                task_truth_changed: false,
            });
        }
        let (status, output) = match execute_timer_tool(runtime_home, turn, tool_call) {
            Ok(output) => (ToolResultStatus::Success, output),
            Err(err) => (
                ToolResultStatus::Failed,
                format!("Timer tool execution failed: {err}"),
            ),
        };
        return Ok(ExecutedToolResult {
            result: tool_result_reentry(turn, tool_call, status, output),
            task_truth_changed: false,
        });
    }
    if tool_name == "task" {
        if role == LiveReasonExecutionRole::Worker {
            return Ok(ExecutedToolResult {
                result: tool_result_reentry(
                    turn,
                    tool_call,
                    ToolResultStatus::Failed,
                    "Worker capability boundary: recursive task management is not available."
                        .to_owned(),
                ),
                task_truth_changed: false,
            });
        }
        if let Some(message) =
            configured_worker_task_boundary_failure(tool_call, configured_worker_set)
        {
            return Ok(ExecutedToolResult {
                result: tool_result_reentry(turn, tool_call, ToolResultStatus::Failed, message),
                task_truth_changed: false,
            });
        }
        let (status, output, task_truth_changed) =
            match execute_task_tool(runtime_home, turn, tool_call) {
                Ok(output) => (
                    ToolResultStatus::Success,
                    output,
                    task_tool_call_mutates_truth(tool_call),
                ),
                Err(err) => (
                    ToolResultStatus::Failed,
                    format!("Task tool execution failed: {err}"),
                    false,
                ),
            };
        return Ok(ExecutedToolResult {
            result: tool_result_reentry(turn, tool_call, status, output),
            task_truth_changed,
        });
    }
    if is_checkpointable_file_mutation_tool(tool_name) {
        let store = RuntimeCheckpointStore::new_with_workspace_root(
            runtime_home,
            &turn.request.agent_id,
            &turn.request.session_id,
            workspace_root.to_path_buf(),
        )
        .map_err(|err| RuntimeLiveBridgeError::ToolCheckpointFailed(err.to_string()))?;
        let preview = match registry.preview(tool_call) {
            Ok(preview) => preview,
            Err(err @ ToolRegistryError::WorkspaceBoundaryViolation { .. }) => {
                return Ok(ExecutedToolResult {
                    result: tool_result_reentry(
                        turn,
                        tool_call,
                        ToolResultStatus::Failed,
                        registry_error_text(role, &err),
                    ),
                    task_truth_changed: false,
                });
            }
            Err(err) => {
                return Err(RuntimeLiveBridgeError::ToolCheckpointFailed(
                    RuntimeCheckpointError::UncheckpointableTool {
                        tool: tool_name.to_owned(),
                        message: err.to_string(),
                    }
                    .to_string(),
                ));
            }
        };
        let manifest = store
            .create_from_preview(turn, &preview, tool_name)
            .map_err(|err| RuntimeLiveBridgeError::ToolCheckpointFailed(err.to_string()))?;
        let (status, output) = match registry.execute(tool_call) {
            Ok(output) => (ToolResultStatus::Success, output.text),
            Err(err) => {
                let _ = store.mark_failed(&manifest, &err.to_string());
                (ToolResultStatus::Failed, registry_error_text(role, &err))
            }
        };
        if status == ToolResultStatus::Success {
            store
                .mark_applied(&manifest)
                .map_err(|err| RuntimeLiveBridgeError::ToolCheckpointFailed(err.to_string()))?;
        }
        return Ok(ExecutedToolResult {
            result: tool_result_reentry(turn, tool_call, status, output),
            task_truth_changed: false,
        });
    }
    let (status, output) = match registry.execute(tool_call) {
        Ok(output) => (ToolResultStatus::Success, output.text),
        Err(err) => (ToolResultStatus::Failed, registry_error_text(role, &err)),
    };
    Ok(ExecutedToolResult {
        result: tool_result_reentry(turn, tool_call, status, output),
        task_truth_changed: false,
    })
}

fn execute_timer_tool(
    runtime_home: &Path,
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<String, String> {
    let args = tool_arguments_object(&tool_call.tool_call.arguments);
    let op = required_json_string(&args, "op")?;
    let store = TimerStore::new(runtime_home, &turn.request.agent_id);
    match op {
        "schedule" => {
            let schedule = store
                .schedule_from_args(&args, turn)
                .map_err(|err| err.to_string())
                .and_then(|schedule| {
                    store
                        .upsert_schedule(schedule)
                        .map_err(|err| err.to_string())
                })?;
            Ok(format!(
                "Timer scheduled: timer_id={} next_due_at={} max_runs={} fired_count={} status={}",
                schedule.timer_id,
                schedule.next_due_at,
                schedule.max_runs,
                schedule.fired_count,
                schedule.status
            ))
        }
        "cancel" => {
            let timer_id = required_json_string(&args, "timer_id")?;
            let schedule = store.cancel(timer_id).map_err(|err| err.to_string())?;
            Ok(format!(
                "Timer cancelled: timer_id={} status={}",
                schedule.timer_id, schedule.status
            ))
        }
        "list" => {
            let schedules = store.active_schedules().map_err(|err| err.to_string())?;
            serde_json::to_string(&schedules)
                .map_err(|err| format!("timer list serialization failed: {err}"))
        }
        other => Err(format!("unsupported timer op `{other}`")),
    }
}

fn execute_task_tool(
    runtime_home: &Path,
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<String, String> {
    let args = tool_arguments_object(&tool_call.tool_call.arguments);
    if !args.contains_key("op") {
        return Err(concat!(
            "`op` is required as a top-level task field. ",
            "Valid production examples: ",
            "task({\"op\":\"create\",\"title\":\"...\",\"content\":\"...\",\"goal\":\"...\",\"target_cwd\":\"/absolute/existing/repo\",\"execution_profile\":\"workspace\",\"dispatch\":{\"mode\":\"none\"}}), ",
            "or for broad hosted search task({\"op\":\"create\",\"title\":\"...\",\"content\":\"...\",\"goal\":\"...\",\"execution_profile\":\"clean_search\",\"dispatch\":{\"mode\":\"none\"}}), ",
            "then task({\"op\":\"assign\",\"task_id\":\"...\",\"agent_id\":\"<configured Worker>\"}). ",
            "Use task({\"op\":\"query\",\"task_id\":\"...\"}) or task({\"op\":\"history\",\"task_id\":\"...\"}) only for specific existing-task truth."
        )
        .to_owned());
    }
    let op = required_json_string(&args, "op")?;
    let task_runtime = TaskRuntime::boot(runtime_home, turn.request.agent_id.clone())
        .map_err(|err| err.to_string())?;
    match op {
        "create" => {
            let request = TaskCreateRequest {
                task_id: optional_json_string(&args, "task_id").map(TaskId::new),
                title: required_json_string(&args, "title")?.to_owned(),
                content: required_json_string(&args, "content")?.to_owned(),
                goal: required_json_string(&args, "goal")?.to_owned(),
                deliverables: required_json_string_array(&args, "deliverables")?,
                acceptance: required_json_string_array(&args, "acceptance")?,
                priority: optional_json_i64(&args, "priority").unwrap_or(50),
                target_cwd: optional_json_string(&args, "target_cwd").map(ToOwned::to_owned),
                execution_profile: parse_task_execution_profile(&args)?,
                dispatch: parse_task_dispatch(&args)?,
                parent: TaskParentRef {
                    session_id: Some(turn.request.session_id.clone()),
                    turn_id: Some(turn.request.turn_id.clone()),
                    trace_id: Some(turn.request.trace_id.clone()),
                },
                actor: task_actor(turn),
                watermark: task_watermark(tool_call),
            };
            let outcome = task_runtime
                .create_task(request)
                .map_err(|err| err.to_string())?;
            let mut output = format!(
                "Task created: task_id={} status={:?} events={}",
                outcome.task.task_id.as_str(),
                outcome.task.status,
                outcome.events.len()
            );
            if let Some(target_cwd) = outcome.task.target_cwd.as_deref() {
                output.push('\n');
                output.push_str(&path_resolution_diagnostic_text("target_cwd", target_cwd));
                output.push_str(
                    "\nUse this diagnostic before asking the user to clarify a path: symlink ancestors are valid aliases, nearest_existing_canonical is the resolved owner truth for existing parents, and missing_suffix names the unresolved leaf that must exist before Worker execution.",
                );
            }
            Ok(output)
        }
        "query" => {
            let task_id = TaskId::new(required_json_string(&args, "task_id")?);
            let task =
                query_task_and_attach_visible_session(&task_runtime, turn, &task_id, tool_call)?;
            Ok(serde_json::to_string(&task).unwrap_or_else(|_| {
                format!(
                    "Task query: task_id={} status={:?}",
                    task.task_id.as_str(),
                    task.status
                )
            }))
        }
        "list_tasks" => {
            let tasks = task_runtime
                .list_tasks(TaskListQuery {
                    status: optional_json_string(&args, "status")
                        .map(parse_task_status)
                        .transpose()?,
                    assignee: optional_json_string(&args, "agent_id").map(AgentId::new),
                })
                .map_err(|err| err.to_string())?;
            Ok(serde_json::to_string(&tasks)
                .unwrap_or_else(|_| format!("Task list: count={}", tasks.len())))
        }
        "history" => {
            let task_id = TaskId::new(required_json_string(&args, "task_id")?);
            query_task_and_attach_visible_session(&task_runtime, turn, &task_id, tool_call)?;
            let events = task_runtime
                .task_history(&task_id)
                .map_err(|err| err.to_string())?;
            Ok(serde_json::to_string(&events)
                .unwrap_or_else(|_| format!("Task history: events={}", events.len())))
        }
        "append" => {
            let outcome = task_runtime
                .append_task(TaskAppendRequest {
                    task_id: TaskId::new(required_json_string(&args, "task_id")?),
                    note: required_json_string(&args, "note")?.to_owned(),
                    actor: task_actor(turn),
                    watermark: task_watermark(tool_call),
                })
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task appended",
                &outcome.task,
                &outcome.event,
            ))
        }
        "pause" => {
            let outcome = task_runtime
                .pause_task(task_mutation_request(&args, turn, tool_call)?)
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task paused",
                &outcome.task,
                &outcome.event,
            ))
        }
        "resume" => {
            let outcome = task_runtime
                .resume_task(task_mutation_request(&args, turn, tool_call)?)
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task resumed",
                &outcome.task,
                &outcome.event,
            ))
        }
        "heartbeat" => {
            let outcome = task_runtime
                .heartbeat_task(TaskHeartbeatRequest {
                    task_id: TaskId::new(required_json_string(&args, "task_id")?),
                    ttl_seconds: optional_json_i64(&args, "ttl_seconds")
                        .unwrap_or(300)
                        .try_into()
                        .map_err(|_| "`ttl_seconds` must be positive".to_owned())?,
                    actor: task_actor(turn),
                    watermark: task_watermark(tool_call),
                })
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task heartbeat",
                &outcome.task,
                &outcome.event,
            ))
        }
        "assign" => {
            let outcome = task_runtime
                .assign_task(TaskAssignRequest {
                    task_id: TaskId::new(required_json_string(&args, "task_id")?),
                    agent_id: AgentId::new(required_json_string(&args, "agent_id")?),
                    actor: task_actor(turn),
                    watermark: task_watermark(tool_call),
                })
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task assigned",
                &outcome.task,
                &outcome.event,
            ))
        }
        "claim_next" => {
            let outcome = task_runtime
                .claim_next_task(TaskClaimRequest {
                    agent_id: AgentId::new(required_json_string(&args, "agent_id")?),
                    execution_id: required_json_string(&args, "execution_id")?.to_owned(),
                    ttl_seconds: optional_json_i64(&args, "ttl_seconds")
                        .unwrap_or(300)
                        .try_into()
                        .map_err(|_| "`ttl_seconds` must be positive".to_owned())?,
                    actor: task_actor(turn),
                    watermark: task_watermark(tool_call),
                })
                .map_err(|err| err.to_string())?;
            if let Some(task) = outcome.task {
                Ok(format!(
                    "Task claimed: task_id={} status={:?} execution_id={}",
                    task.task_id.as_str(),
                    task.status,
                    outcome.execution_id.unwrap_or_default()
                ))
            } else {
                Ok("Task claimed: none".to_owned())
            }
        }
        "record_execution" => {
            let task_id = TaskId::new(required_json_string(&args, "task_id")?);
            let phase = required_json_string(&args, "phase")?.to_owned();
            let summary = required_json_string(&args, "summary")?.to_owned();
            let evidence = required_json_string_array(&args, "evidence")?;
            let outcome = if let Some(status) = optional_json_string(&args, "status") {
                let agent_id = AgentId::new(required_json_string(&args, "agent_id")?);
                let execution_id = required_json_string(&args, "execution_id")?.to_owned();
                let kind = match status {
                    "running" => ExecutionFactKind::Running {
                        phase,
                        summary,
                        evidence,
                    },
                    "recovering" => ExecutionFactKind::Recovering {
                        summary,
                        evidence,
                        retry_count: required_json_u32(&args, "retry_count")?,
                    },
                    "blocked" => ExecutionFactKind::Blocked {
                        reason: summary,
                        evidence,
                    },
                    "interrupted" => ExecutionFactKind::Interrupted {
                        reason: summary,
                        evidence,
                    },
                    "review_ready" => ExecutionFactKind::ReviewReady {
                        summary,
                        deliverables: required_json_string_array(&args, "deliverables")?,
                        evidence,
                    },
                    other => return Err(format!("unsupported execution status `{other}`")),
                };
                task_runtime
                    .apply_execution_fact(ExecutionFact {
                        execution_id,
                        task_id,
                        agent_id,
                        turn_id: Some(turn.request.turn_id.clone()),
                        occurred_at: now_unix_seconds(),
                        kind,
                        watermark: task_watermark(tool_call),
                    })
                    .map_err(|err| err.to_string())?
            } else {
                task_runtime
                    .record_execution(TaskExecutionRecordRequest {
                        task_id,
                        phase,
                        summary,
                        evidence,
                        actor: task_actor(turn),
                        watermark: task_watermark(tool_call),
                    })
                    .map_err(|err| err.to_string())?
            };
            Ok(task_mutation_result(
                task_event_result_label(&outcome.event),
                &outcome.task,
                &outcome.event,
            ))
        }
        "cancel" => {
            let outcome = task_runtime
                .cancel_task(task_mutation_request(&args, turn, tool_call)?)
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task cancelled",
                &outcome.task,
                &outcome.event,
            ))
        }
        "submit_review" => {
            let outcome = task_runtime
                .submit_review(TaskReviewSubmission {
                    task_id: TaskId::new(required_json_string(&args, "task_id")?),
                    summary: required_json_string(&args, "summary")?.to_owned(),
                    deliverables: required_json_string_array(&args, "deliverables")?,
                    evidence: required_json_string_array(&args, "evidence")?,
                    actor: task_actor(turn),
                    watermark: task_watermark(tool_call),
                })
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task review submitted",
                &outcome.task,
                &outcome.event,
            ))
        }
        "approve" => {
            let outcome = task_runtime
                .approve_review(task_mutation_request(&args, turn, tool_call)?)
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task approved",
                &outcome.task,
                &outcome.event,
            ))
        }
        "reject" => {
            let outcome = task_runtime
                .reject_review(TaskReviewRejection {
                    task_id: TaskId::new(required_json_string(&args, "task_id")?),
                    reject_reason: required_json_string(&args, "reject_reason")?.to_owned(),
                    next_requirements: required_json_string_array(&args, "next_requirements")?,
                    actor: task_actor(turn),
                    watermark: task_watermark(tool_call),
                })
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task rejected",
                &outcome.task,
                &outcome.event,
            ))
        }
        "close" => {
            let outcome = task_runtime
                .close_task(task_mutation_request(&args, turn, tool_call)?)
                .map_err(|err| err.to_string())?;
            Ok(task_mutation_result(
                "Task closed",
                &outcome.task,
                &outcome.event,
            ))
        }
        "list_agents" => {
            let agents = task_runtime.list_agents().map_err(|err| err.to_string())?;
            Ok(serde_json::to_string(&agents)
                .unwrap_or_else(|_| format!("Agent list: count={}", agents.len())))
        }
        "query_agent" => {
            let agent_id = AgentId::new(required_json_string(&args, "agent_id")?);
            let agent = task_runtime
                .query_agent(&agent_id)
                .map_err(|err| err.to_string())?;
            Ok(serde_json::to_string(&agent)
                .unwrap_or_else(|_| format!("Agent query: agent_id={}", agent.agent_id.as_str())))
        }
        "create_agent" => {
            let outcome = task_runtime
                .create_agent(AgentCreateRequest {
                    agent_id: AgentId::new(required_json_string(&args, "agent_id")?),
                    capabilities: required_json_string_array(&args, "capabilities")?,
                    actor: task_actor(turn),
                    watermark: task_watermark(tool_call),
                })
                .map_err(|err| err.to_string())?;
            Ok(format!(
                "Agent created: agent_id={} status={:?}",
                outcome.agent.agent_id.as_str(),
                outcome.agent.status
            ))
        }
        "close_agent" => {
            let outcome = task_runtime
                .close_agent(AgentMutationRequest {
                    agent_id: AgentId::new(required_json_string(&args, "agent_id")?),
                    actor: task_actor(turn),
                    watermark: task_watermark(tool_call),
                })
                .map_err(|err| err.to_string())?;
            Ok(format!(
                "Agent closed: agent_id={} status={:?}",
                outcome.agent.agent_id.as_str(),
                outcome.agent.status
            ))
        }
        other => Err(format!("unsupported task op `{other}`")),
    }
}

fn configured_worker_task_boundary_failure(
    tool_call: &ReasonReq04ToolCall,
    configured_worker_set: Option<&[String]>,
) -> Option<String> {
    let configured_worker_set = configured_worker_set?;
    let configured_worker_list = configured_worker_label(Some(configured_worker_set));
    let args = tool_arguments_object(&tool_call.tool_call.arguments);
    match args.get("op").and_then(Value::as_str) {
        Some("assign") => args
            .get("agent_id")
            .and_then(Value::as_str)
            .filter(|agent_id| {
                !configured_worker_set
                    .iter()
                    .any(|configured| configured == *agent_id)
            })
            .map(|_| {
                format!(
                    "Configured topology boundary: task assignment must target one configured Worker: `{configured_worker_list}`."
                )
            }),
        Some("create") => match args.get("dispatch") {
            None => Some(format!(
                "Configured topology boundary: task creation must set dispatch.mode to `none` for later assignment, or `agent` with one configured Worker id `{configured_worker_list}`. Implicit dispatch is not allowed because it can select historical agents."
            )),
            Some(Value::Object(dispatch)) => match dispatch.get("mode").and_then(Value::as_str) {
                Some("none") => None,
                Some("agent") => dispatch
                    .get("agent_id")
                    .and_then(Value::as_str)
                    .filter(|agent_id| {
                        !configured_worker_set
                            .iter()
                            .any(|configured| configured == *agent_id)
                    })
                    .map(|_| {
                        format!(
                            "Configured topology boundary: task creation may dispatch only to one configured Worker: `{configured_worker_list}`."
                        )
                    }),
                Some("auto" | "self") => Some(format!(
                    "Configured topology boundary: task creation cannot use auto or self dispatch. Use dispatch.mode `none`, then assign one configured Worker `{configured_worker_list}`, or dispatch directly to that Worker."
                )),
                _ => None,
            },
            Some(_) => None,
        },
        _ => None,
    }
}

fn query_task_and_attach_visible_session(
    task_runtime: &TaskRuntime,
    turn: &TurnRecord,
    task_id: &TaskId,
    tool_call: &ReasonReq04ToolCall,
) -> Result<TaskSnapshot, String> {
    let session_id = &turn.request.session_id;
    if session_id.as_str().starts_with("master-lifecycle-")
        || session_id.as_str().starts_with("master-timer-")
        || session_id.as_str().starts_with("worker-task-")
    {
        return task_runtime
            .query_task(task_id)
            .map_err(|err| err.to_string());
    }
    task_runtime
        .attach_task_to_session(
            task_id,
            session_id,
            task_actor(turn),
            task_watermark(tool_call),
        )
        .map_err(|err| err.to_string())
}

fn task_tool_call_mutates_truth(tool_call: &ReasonReq04ToolCall) -> bool {
    let args = tool_arguments_object(&tool_call.tool_call.arguments);
    let Some(Value::String(op)) = args.get("op") else {
        return false;
    };
    matches!(
        op.as_str(),
        "query"
            | "history"
            | "create"
            | "append"
            | "pause"
            | "resume"
            | "heartbeat"
            | "assign"
            | "claim_next"
            | "record_execution"
            | "cancel"
            | "submit_review"
            | "approve"
            | "reject"
            | "close"
            | "create_agent"
            | "close_agent"
    )
}

fn task_mutation_request(
    args: &Map<String, Value>,
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<TaskMutationRequest, String> {
    Ok(TaskMutationRequest {
        task_id: TaskId::new(required_json_string(args, "task_id")?),
        actor: task_actor(turn),
        watermark: task_watermark(tool_call),
    })
}

fn parse_task_status(value: &str) -> Result<TaskStatus, String> {
    match value {
        "created" => Ok(TaskStatus::Created),
        "waiting_agent" => Ok(TaskStatus::WaitingAgent),
        "assigned" => Ok(TaskStatus::Assigned),
        "running" => Ok(TaskStatus::Running),
        "interrupted" => Ok(TaskStatus::Interrupted),
        "paused" => Ok(TaskStatus::Paused),
        "blocked" => Ok(TaskStatus::Blocked),
        "review_submitted" => Ok(TaskStatus::ReviewSubmitted),
        "approved" => Ok(TaskStatus::Approved),
        "rejected" => Ok(TaskStatus::Rejected),
        "failed" => Ok(TaskStatus::Failed),
        "cancelled" => Ok(TaskStatus::Cancelled),
        "closed" => Ok(TaskStatus::Closed),
        other => Err(format!("unsupported task status `{other}`")),
    }
}

fn parse_task_execution_profile(args: &Map<String, Value>) -> Result<TaskExecutionProfile, String> {
    let Some(value) = optional_json_string(args, "execution_profile") else {
        return Ok(TaskExecutionProfile::Workspace);
    };
    parse_task_execution_profile_value(value)
}

fn parse_task_execution_profile_value(value: &str) -> Result<TaskExecutionProfile, String> {
    match value.trim() {
        "" | "workspace" => Ok(TaskExecutionProfile::Workspace),
        "clean_search" => Ok(TaskExecutionProfile::CleanSearch),
        other => Err(format!(
            "unsupported execution_profile `{other}`; expected `workspace` or `clean_search`"
        )),
    }
}

fn task_actor(turn: &TurnRecord) -> TaskActor {
    TaskActor {
        agent_id: turn.request.agent_id.clone(),
        source: "control.center".to_owned(),
        session_id: Some(turn.request.session_id.clone()),
        turn_id: Some(turn.request.turn_id.clone()),
        trace_id: Some(turn.request.trace_id.clone()),
    }
}

fn task_watermark(tool_call: &ReasonReq04ToolCall) -> TaskWatermark {
    TaskWatermark {
        metadata_id: None,
        hook: Some("ControlHook03AfterModelResponse".to_owned()),
        action_tool_call_id: Some(tool_call.tool_call.tool_call_id.as_str().to_owned()),
    }
}

fn task_mutation_result(
    label: &str,
    task: &freehand_task::TaskSnapshot,
    event: &freehand_task::TaskLedgerEvent,
) -> String {
    format!(
        "{label}: task_id={} status={:?} event={} seq={}",
        task.task_id.as_str(),
        task.status,
        event.event_type,
        event.seq
    )
}

fn task_event_result_label(event: &freehand_task::TaskLedgerEvent) -> &'static str {
    match event.event_type.as_str() {
        "TaskBlocked" => "Task blocked",
        "TaskExecutionRecovering" => "Task execution recovering",
        "TaskExecutionRecorded" => "Task execution recorded",
        "TaskReviewSubmitted" => "Task review submitted",
        _ => "Task event recorded",
    }
}

fn tool_arguments_object(arguments: &[ToolArgument]) -> Map<String, Value> {
    arguments
        .iter()
        .map(|argument| (argument.name.clone(), argument.value.clone()))
        .collect()
}

fn parse_task_dispatch(args: &Map<String, Value>) -> Result<TaskDispatchRequest, String> {
    let Some(dispatch) = args.get("dispatch") else {
        return Ok(TaskDispatchRequest::Auto {
            allow_create_agent: false,
        });
    };
    let object = dispatch
        .as_object()
        .ok_or_else(|| "`dispatch` must be an object".to_owned())?;
    match required_json_string(object, "mode")? {
        "none" => Ok(TaskDispatchRequest::None),
        "self" => Ok(TaskDispatchRequest::SelfAgent),
        "agent" => Ok(TaskDispatchRequest::Agent {
            agent_id: AgentId::new(required_json_string(object, "agent_id")?),
        }),
        "auto" => Ok(TaskDispatchRequest::Auto {
            allow_create_agent: object
                .get("allow_create_agent")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }),
        other => Err(format!("unsupported dispatch mode `{other}`")),
    }
}

fn required_json_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("`{field}` is required"))
}

fn optional_json_string<'a>(object: &'a Map<String, Value>, field: &str) -> Option<&'a str> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn optional_json_i64(object: &Map<String, Value>, field: &str) -> Option<i64> {
    object.get(field).and_then(Value::as_i64)
}

fn required_json_u32(object: &Map<String, Value>, field: &str) -> Result<u32, String> {
    let value = object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("`{field}` is required and must be a non-negative integer"))?;
    u32::try_from(value).map_err(|_| format!("`{field}` is too large"))
}

fn required_json_string_array(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Vec<String>, String> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("`{field}` is required and must be an array"))?;
    let mut result = Vec::new();
    for value in values {
        let item = value
            .as_str()
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .ok_or_else(|| format!("`{field}` must contain non-empty strings"))?;
        result.push(item.to_owned());
    }
    if result.is_empty() {
        return Err(format!("`{field}` must not be empty"));
    }
    Ok(result)
}

fn tool_result_reentry(
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
    status: ToolResultStatus,
    output: String,
) -> ReasonReq05ToolResultReentry {
    ReasonReq05ToolResultReentry {
        session_id: turn.request.session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: turn.request.feature_id.clone(),
        agent_id: turn.request.agent_id.clone(),
        tool_result: ToolResultContract {
            tool_call_id: tool_call.tool_call.tool_call_id.clone(),
            status,
            output,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn pair_master_attention_invalidated_tool_calls<FB>(
    ctx: &mut LiveApplyContext<'_, FB>,
    turn: &mut TurnRecord,
    tool_calls: &[ReasonReq04ToolCall],
    resolution: &master_runner::MasterAttentionResolution,
    schema_rejection_count: u32,
    tool_exchanges: &mut Vec<ProviderToolExchange>,
    executed_tool_call_ids: &mut Vec<String>,
) -> Result<usize, RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
{
    for tool_call in tool_calls {
        let tool_result = tool_result_reentry(
            turn,
            tool_call,
            ToolResultStatus::Failed,
            format!(
                "invalidated_before_execution_by_master_attention: attention_event_id={} decision_kind={}",
                resolution.attention_event_id, resolution.decision_kind
            ),
        );
        let output = ProviderSemanticOutput::ToolResultReentry(tool_result.clone());
        ctx.engine
            .apply_provider_output(turn, output.clone())
            .map_err(|err| RuntimeLiveBridgeError::ProviderOutputApplyFailed(err.to_string()))?;
        ctx.persistence
            .record_provider_output_applied(ctx.history, turn, &output, schema_rejection_count)
            .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))?;
        executed_tool_call_ids.push(tool_call.tool_call.tool_call_id.as_str().to_owned());
        tool_exchanges.push(ProviderToolExchange {
            tool_call: tool_call.clone(),
            tool_result,
        });
    }
    drain_broadcasts(ctx.receiver, ctx.broadcasts, ctx.on_broadcast);
    drain_debug_events(ctx.debug_receiver, ctx.on_debug);
    Ok(tool_calls.len())
}

#[allow(clippy::too_many_arguments)]
fn prepare_master_attention_tool_invalidation<FB>(
    ctx: &mut LiveApplyContext<'_, FB>,
    turn: &mut TurnRecord,
    tool_calls: &[ReasonReq04ToolCall],
    resolution: &master_runner::MasterAttentionResolution,
    schema_rejection_count: u32,
    tool_exchanges: &mut Vec<ProviderToolExchange>,
    executed_tool_call_ids: &mut Vec<String>,
    tool_executions: &mut usize,
    next_prompt: &mut String,
    carryover_segments: &mut Vec<ContextSegment>,
    original_prompt: &str,
    round_context: LiveRoundContext<'_>,
) -> Result<(), RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
{
    let invalidated_count = pair_master_attention_invalidated_tool_calls(
        ctx,
        turn,
        tool_calls,
        resolution,
        schema_rejection_count,
        tool_exchanges,
        executed_tool_call_ids,
    )?;
    *tool_executions = (*tool_executions).saturating_add(invalidated_count);
    *next_prompt = master_attention_continuation_prompt();
    *carryover_segments = next_round_segments(
        original_prompt,
        &collect_turn_text(turn),
        None,
        round_context,
    )?;
    admit_master_attention_resolution_for_next_round(carryover_segments, resolution, round_context)
}

fn prepare_master_attention_reasoning_continuation(
    resolution: &master_runner::MasterAttentionResolution,
    next_prompt: &mut String,
    carryover_segments: &mut Vec<ContextSegment>,
    original_prompt: &str,
    visible_text: &str,
    round_context: LiveRoundContext<'_>,
) -> Result<(), RuntimeLiveBridgeError> {
    *next_prompt = master_attention_continuation_prompt();
    *carryover_segments = next_round_segments(original_prompt, visible_text, None, round_context)?;
    admit_master_attention_resolution_for_next_round(carryover_segments, resolution, round_context)
}

fn is_checkpointable_file_mutation_tool(tool_name: &str) -> bool {
    matches!(tool_name, "write_file" | "edit_file" | "multi_edit")
}

fn task_decision_boundary_summary(
    runtime_home: &Path,
    agent_id: &AgentId,
    boundary: &LiveReasonTaskDecisionBoundary,
) -> Result<Option<String>, RuntimeLiveBridgeError> {
    let runtime = TaskRuntime::boot(runtime_home, agent_id.clone())
        .map_err(|err| RuntimeLiveBridgeError::TaskProjectionFailed(err.to_string()))?;
    let task = runtime
        .query_task(&boundary.task_id)
        .map_err(|err| RuntimeLiveBridgeError::TaskProjectionFailed(err.to_string()))?;
    if task.last_event_seq <= boundary.initial_event_seq {
        return Ok(None);
    }
    let reached = match &boundary.mode {
        LiveReasonTaskDecisionMode::TargetMutation => true,
        LiveReasonTaskDecisionMode::TargetStatuses(statuses) => {
            statuses.iter().any(|status| status == &task.status)
        }
    };
    if reached {
        Ok(Some(format!(
            "Task Center decision persisted for `{}` at status `{:?}` with event sequence {}.",
            task.task_id.as_str(),
            task.status,
            task.last_event_seq
        )))
    } else {
        Ok(None)
    }
}

fn task_decision_round_budget_reason(
    boundary: Option<&LiveReasonTaskDecisionBoundary>,
    round: usize,
) -> Option<String> {
    let boundary = boundary?;
    (round >= boundary.max_rounds).then(|| {
        format!(
            "Master lifecycle decision for task `{}` exceeded the {}-round budget without reaching its Task Center decision boundary.",
            boundary.task_id.as_str(),
            boundary.max_rounds
        )
    })
}

fn master_session_completion_rejection(
    runtime_home: &Path,
    agent_id: &AgentId,
    session_id: &SessionId,
    role: LiveReasonExecutionRole,
    task_decision_boundary: Option<&LiveReasonTaskDecisionBoundary>,
    submission: &CompletionSubmission,
) -> Result<Option<CompletionSchemaRejection>, RuntimeLiveBridgeError> {
    if role != LiveReasonExecutionRole::Master || task_decision_boundary.is_some() {
        return Ok(None);
    }

    let lifecycle = master_session_lifecycle_owner_truth(runtime_home, agent_id, session_id)?;
    match submission.claim {
        CompletionClaim::Complete => {
            if lifecycle.open_child_tasks.is_empty() {
                return Ok(None);
            }

            Ok(Some(CompletionSchemaRejection {
                issues: vec![CompletionSchemaIssue {
                    field: "claim".to_owned(),
                    message: format!(
                        "cannot be `complete` while child Worker tasks for this Master session are still open: {}. Inspect Task Center truth, wait with `claim=\"waiting\"`, or continue only if you can approve/close all required child work in this turn.",
                        lifecycle.open_child_tasks.join(", ")
                    ),
                }],
            }))
        }
        CompletionClaim::Waiting => {
            if lifecycle.has_open_owner_truth() {
                return Ok(None);
            }

            Ok(Some(CompletionSchemaRejection {
                issues: vec![CompletionSchemaIssue {
                    field: "claim".to_owned(),
                    message: "claim=`waiting` requires open Task Center or timer owner truth for this Master session so the lifecycle can resume without another user message. Current child tasks are terminal and no active/running source timer exists. If the next action requires a user choice, use `claim=\"blocked\"` with a precise `blocked_reason`; if the user objective is actually complete, use `claim=\"complete\"` with evidence."
                        .to_owned(),
                }],
            }))
        }
        CompletionClaim::Continue | CompletionClaim::Blocked => Ok(None),
    }
}

struct MasterSessionLifecycleOwnerTruth {
    open_child_tasks: Vec<String>,
    open_timers: Vec<String>,
}

impl MasterSessionLifecycleOwnerTruth {
    fn has_open_owner_truth(&self) -> bool {
        !self.open_child_tasks.is_empty() || !self.open_timers.is_empty()
    }
}

fn master_session_lifecycle_owner_truth(
    runtime_home: &Path,
    agent_id: &AgentId,
    session_id: &SessionId,
) -> Result<MasterSessionLifecycleOwnerTruth, RuntimeLiveBridgeError> {
    let runtime = TaskRuntime::boot(runtime_home, agent_id.clone())
        .map_err(|err| RuntimeLiveBridgeError::TaskProjectionFailed(err.to_string()))?;
    let board = runtime
        .query_task_board(TaskBoardQuery {
            status: None,
            assignee: None,
            include_terminal: true,
        })
        .map_err(|err| RuntimeLiveBridgeError::TaskProjectionFailed(err.to_string()))?;
    let open_children = board
        .tasks
        .iter()
        .filter(|task| task.parent.session_id.as_ref() == Some(session_id))
        .filter(|task| task_status_blocks_parent_completion(&task.status))
        .map(|task| format!("{}:{:?}", task.task_id.as_str(), task.status))
        .collect::<Vec<_>>();
    let timer_store = TimerStore::new(runtime_home, agent_id);
    let open_timers = timer_store
        .load_schedules()
        .map_err(|err| RuntimeLiveBridgeError::TaskProjectionFailed(err.to_string()))?
        .into_iter()
        .filter(|schedule| schedule.source_session_id.as_ref() == Some(session_id))
        .filter(|schedule| matches!(schedule.status.as_str(), "active" | "running"))
        .map(|schedule| format!("{}:{}", schedule.timer_id, schedule.status))
        .collect::<Vec<_>>();
    Ok(MasterSessionLifecycleOwnerTruth {
        open_child_tasks: open_children,
        open_timers,
    })
}

fn task_status_blocks_parent_completion(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Created
            | TaskStatus::WaitingAgent
            | TaskStatus::Assigned
            | TaskStatus::Running
            | TaskStatus::Interrupted
            | TaskStatus::Paused
            | TaskStatus::Blocked
            | TaskStatus::ReviewSubmitted
            | TaskStatus::Approved
            | TaskStatus::Rejected
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_framework_live_turn<FB, FD>(
    engine: &ReasonTurnEngine,
    persistence: &ReasonPersistence,
    history: &SessionHistory,
    receiver: &Receiver<ReasonBroadcastEvent>,
    debug_receiver: &Receiver<DebugEvent>,
    broadcasts: &mut Vec<ReasonBroadcastEvent>,
    on_broadcast: &mut FB,
    on_debug: &mut FD,
    turn: &mut TurnRecord,
    finalization: FrameworkLiveTurnFinalization,
    schema_rejection_count: u32,
) -> Result<(), RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
    FD: FnMut(&DebugEvent),
{
    match finalization {
        FrameworkLiveTurnFinalization::Complete(summary) => {
            let submission = CompletionSubmission {
                claim: CompletionClaim::Complete,
                completion_reason: Some("framework lifecycle decision boundary reached".to_owned()),
                evidence: Some(summary.clone()),
                summary: Some(summary),
                learned: Some("return control to durable EventInbox polling".to_owned()),
                next_step: None,
                blocked_reason: None,
            };
            let _ = engine
                .submit_completion(turn, &submission)
                .map_err(|err| RuntimeLiveBridgeError::TurnStartFailed(err.to_string()))?;
        }
        FrameworkLiveTurnFinalization::Blocked(reason) => {
            engine.block_turn(turn, reason);
        }
    }
    drain_broadcasts(receiver, broadcasts, on_broadcast);
    drain_debug_events(debug_receiver, on_debug);
    persistence
        .record_turn_closed(history, turn, schema_rejection_count)
        .map(|_| ())
        .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))
}

struct LiveApplyContext<'a, FB>
where
    FB: FnMut(&ReasonBroadcastEvent),
{
    engine: &'a ReasonTurnEngine,
    persistence: &'a ReasonPersistence,
    history: &'a SessionHistory,
    receiver: &'a Receiver<ReasonBroadcastEvent>,
    debug_receiver: &'a Receiver<DebugEvent>,
    broadcasts: &'a mut Vec<ReasonBroadcastEvent>,
    on_broadcast: &'a mut FB,
    on_debug: &'a mut dyn FnMut(&DebugEvent),
}

fn apply_provider_outputs_persist_and_capture_broadcasts<FB>(
    ctx: &mut LiveApplyContext<'_, FB>,
    turn: &mut TurnRecord,
    outputs: &[ProviderSemanticOutput],
    schema_rejections: u32,
) -> Result<(), RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
{
    for output in outputs {
        ctx.engine
            .apply_provider_output(turn, output.clone())
            .map_err(|err| RuntimeLiveBridgeError::ProviderOutputApplyFailed(err.to_string()))?;
        ctx.persistence
            .record_provider_output_applied(ctx.history, turn, output, schema_rejections)
            .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))?;
    }
    drain_broadcasts(ctx.receiver, ctx.broadcasts, ctx.on_broadcast);
    drain_debug_events(ctx.debug_receiver, ctx.on_debug);
    Ok(())
}

fn drain_broadcasts<F>(
    receiver: &Receiver<ReasonBroadcastEvent>,
    broadcasts: &mut Vec<ReasonBroadcastEvent>,
    on_broadcast: &mut F,
) where
    F: FnMut(&ReasonBroadcastEvent) + ?Sized,
{
    while let Ok(event) = receiver.try_recv() {
        on_broadcast(&event);
        broadcasts.push(event);
    }
}

fn drain_debug_events<F>(receiver: &Receiver<DebugEvent>, on_debug: &mut F)
where
    F: FnMut(&DebugEvent) + ?Sized,
{
    while let Ok(event) = receiver.try_recv() {
        on_debug(&event);
    }
}

pub(crate) fn apply_runtime_reason_broadcast(
    ui_state: &Arc<Mutex<UiProtocolState>>,
    reason_agent_id: &AgentId,
    master_node_id: &str,
    event: &ReasonBroadcastEvent,
) {
    let mut ui = ui_state.lock().expect("lock ui state");
    match event {
        ReasonBroadcastEvent::Semantic(event) => {
            ui.apply_semantic_event(
                reason_agent_id.clone(),
                master_node_id.to_owned(),
                event,
                false,
            );
        }
        ReasonBroadcastEvent::Tool(event) => {
            ui.apply_tool_call(
                reason_agent_id.clone(),
                master_node_id.to_owned(),
                event,
                false,
            );
        }
        ReasonBroadcastEvent::ToolResult(event) => {
            ui.apply_tool_result(
                reason_agent_id.clone(),
                master_node_id.to_owned(),
                event,
                false,
            );
        }
        ReasonBroadcastEvent::Usage(event) => {
            ui.apply_usage_event(
                reason_agent_id.clone(),
                master_node_id.to_owned(),
                event,
                false,
            );
        }
        ReasonBroadcastEvent::CompletionSchemaRejected(event) => {
            let issue_summary = event
                .rejection
                .issues
                .iter()
                .map(|issue| format!("{} {}", issue.field, issue.message))
                .collect::<Vec<_>>()
                .join("; ");
            ui.apply_completion_schema_retry_waiting(UiCompletionSchemaRetryWaiting {
                source_agent_id: reason_agent_id.clone(),
                source_node_id: master_node_id.to_owned(),
                session_id: event.session_id.clone(),
                turn_id: event.turn_id.clone(),
                retry_index: event.retry_index,
                issue_summary,
                slave_substream_card: false,
            });
        }
        ReasonBroadcastEvent::ModelContinuationWaiting(event) => {
            ui.apply_model_request_waiting_kind(UiModelRequestWaiting {
                source_agent_id: reason_agent_id.clone(),
                source_node_id: master_node_id.to_owned(),
                session_id: event.session_id.clone(),
                turn_id: event.turn_id.clone(),
                kind: UiModelRequestKind::ToolResultContinuation,
                detail: Some(event.detail.clone()),
                transport: None,
                slave_substream_card: false,
            });
        }
        ReasonBroadcastEvent::Terminal(event) => {
            ui.apply_terminal_event(
                reason_agent_id.clone(),
                master_node_id.to_owned(),
                event,
                false,
            );
        }
        ReasonBroadcastEvent::Error(event) => {
            ui.apply_error_event(
                reason_agent_id.clone(),
                master_node_id.to_owned(),
                event,
                false,
            );
        }
    }
}

pub(crate) fn apply_runtime_debug_event(
    ui_state: &Arc<Mutex<UiProtocolState>>,
    reason_agent_id: &AgentId,
    master_node_id: &str,
    event: &DebugEvent,
) {
    let mut ui = ui_state.lock().expect("lock ui state");
    let model_request_waiting = match event.envelope.semantic.pipeline_node.as_deref() {
        Some("RuntimeLive01ContextPlanningStarted")
        | Some("RuntimeLive01ContextSegmentStarted")
        | Some("RuntimeLive01ContextSegmentCompleted")
        | Some("RuntimeLive01ContextSegmentFailed")
        | Some("RuntimeLive01ContextPlanningCompleted")
        | Some("RuntimeLive02ProviderRequestBuilt") => Some(ErrorCenterModelWaitingActivity {
            kind: UiModelRequestKind::Thinking,
            detail: event
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.status_text.clone()),
            transport: None,
        }),
        Some("RuntimeLive05ProviderError") => Some(ErrorCenterModelWaitingActivity {
            kind: UiModelRequestKind::Thinking,
            detail: Some("Waiting for model response.".to_owned()),
            transport: event
                .snapshot
                .as_ref()
                .map(|snapshot| UiModelTransportActivity {
                    kind: UiModelTransportKind::ProviderRetry,
                    detail: Some(snapshot.status_text.clone()),
                }),
        }),
        Some("RuntimeLive05ProviderFailover") => Some(ErrorCenterModelWaitingActivity {
            kind: UiModelRequestKind::Thinking,
            detail: Some("Waiting for model response.".to_owned()),
            transport: event
                .snapshot
                .as_ref()
                .map(|snapshot| UiModelTransportActivity {
                    kind: UiModelTransportKind::ProviderFailover,
                    detail: Some(snapshot.status_text.clone()),
                }),
        }),
        _ => None,
    };
    if let Some(waiting) = model_request_waiting {
        ui.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: reason_agent_id.clone(),
            source_node_id: master_node_id.to_owned(),
            session_id: event.envelope.semantic.session_id.clone(),
            turn_id: event.envelope.semantic.turn_id.clone(),
            kind: waiting.kind,
            detail: waiting.detail,
            transport: waiting.transport,
            slave_substream_card: false,
        });
    }
    let _ = ui.apply_debug_event(event);
}

#[cfg(test)]
mod tests;
