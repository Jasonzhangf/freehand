//! Runtime wiring owner for UI command dispatch.

mod master_runner;
mod worker_runner;

pub use master_runner::{
    ProductionMasterRunner, ProductionMasterRunnerError, ProductionMasterTickOutcome,
};
pub use worker_runner::{
    ProductionWorkerRunner, ProductionWorkerRunnerError, ProductionWorkerTickOutcome,
};

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use freehand_blocks::{
    CompletionClaim, CompletionDecision, CompletionSchemaIssue, CompletionSchemaRejection,
    CompletionSubmission, completion_schema_guidance, completion_schema_rejection_feedback,
    parse_completion_submission_block, strip_completion_submission_block,
    validate_completion_submission,
};
use freehand_config::{
    AgentMode, ProviderConfigUpdate, ProviderProtocol as ConfigProviderProtocol, ProviderType,
    SelectedAgentConfig, default_config_path, load_default_config, update_provider_config_in_path,
};
use freehand_contracts::{
    AgentId, ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
    ContextSegmentKind, ContextStability, ErrorClass, ErrorContract, ErrorErr01RuntimeClassified,
    FeatureId, ReasonReq04ToolCall, ReasonReq05ToolResultReentry, RecoveryPolicy, SessionId,
    ToolArgument, ToolPreviewChangeKind, ToolPreviewContract, ToolResultContract, ToolResultStatus,
    TraceId, TurnId,
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
    ProviderCapabilities, ProviderDescriptor, ProviderFamily, ProviderProtocol,
    ProviderSemanticOutput, ProviderToolDefinition, ProviderToolExchange, build_semantic_request,
};
use freehand_reason::{
    PersistedSessionMetadataEntry, ProviderRawLedgerWrite, ProviderRawScenePosition,
    ReasonBroadcastEvent, ReasonPersistence, ReasonPersistenceError,
    ReasonResp04CompletionSchemaRejected, ReasonResp05ModelContinuationWaiting, ReasonTurnEngine,
    SessionHistory, TurnRecord, TurnStartInput,
};
use freehand_task::{
    AgentCreateRequest, AgentLifecycleActivity, AgentLifecycleSnapshot, AgentLifecycleState,
    AgentMutationRequest, AgentSnapshot, AgentStatus, ExecutionFact, ExecutionFactKind,
    MasterPollClassification, MasterPollOutcome, MasterPollRequest, SchedulerTickRequest,
    TaskActor, TaskAppendRequest, TaskAssignRequest, TaskBoardProjection, TaskBoardQuery,
    TaskClaimRequest, TaskCreateRequest, TaskDispatchRequest, TaskError, TaskEventInboxEntry,
    TaskEventInboxProjection, TaskEventInboxQuery, TaskExecutionRecordRequest,
    TaskHeartbeatRequest, TaskId, TaskLedgerEvent, TaskListQuery, TaskMutationRequest,
    TaskParentRef, TaskReviewRejection, TaskReviewSubmission, TaskRuntime, TaskSnapshot,
    TaskStatus, TaskWatermark, WorkerControlEvent, WorkerControlOp, WorkerControlProjection,
    WorkerControlRequest,
};
use freehand_tools::{
    BuiltinToolExecutionScope, BuiltinToolRegistry, ToolRegistryError, with_workspace_root,
};
use freehand_ui_protocol::{
    TurnProjectionInput, UiAgentBoardProjection, UiAgentLifecycleActivityProjection,
    UiAgentLifecycleProjection, UiAgentSnapshotProjection, UiCheckpointSummary, UiClientKind,
    UiCommand, UiCommandDispatchEnvelope, UiCommandDispatchPort, UiCommandDispatchPortError,
    UiCommandDispatchReceipt, UiCompletionSchemaRetryWaiting, UiConfigStatusProjection,
    UiErrorCenterEventListProjection, UiErrorCenterEventProjection, UiExecutionFactCommand,
    UiExecutionFactKind, UiMasterPollClassificationProjection, UiMasterPollProjection,
    UiModelRequestKind, UiModelRequestWaiting, UiProtocolState, UiProviderConfigUpdate,
    UiQueryResult, UiRuntimeQueryPort, UiSchedulerTickCommand, UiSessionMetadataProjection,
    UiTaskAgentCreateCommand, UiTaskAssignCommand, UiTaskBoardProjection, UiTaskClaimCommand,
    UiTaskCreateCommand, UiTaskDispatchCommand, UiTaskEventInboxEntryProjection,
    UiTaskEventInboxProjection, UiTaskHistoryProjection, UiTaskLedgerEventProjection,
    UiTaskListProjection, UiTaskReviewCommand, UiTaskReviewRejectionCommand,
    UiTaskSnapshotProjection, UiTurnProjection, UiWorkerControlCommand,
    UiWorkerControlEventProjection, UiWorkerControlProjection,
    checkpoint_projection_from_runtime_summary, turn_projection_for_client,
    turn_projection_from_events,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use thiserror::Error;

const PROVIDER_EXECUTOR_RETRY_CAP: u32 = 5;
const PROVIDER_EXECUTOR_INITIAL_BACKOFF_MS: u64 = 1_000;
const PROVIDER_EXECUTOR_MAX_BACKOFF_MS: u64 = 16_000;

#[derive(Debug, Clone)]
pub struct LiveReasonTurnRequest {
    pub runtime_home: PathBuf,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub prompt: String,
    pub cwd: Option<PathBuf>,
    pub stream: bool,
    pub cancel_token: Option<LiveReasonCancelToken>,
}

pub type LiveReasonCancelToken = Arc<AtomicBool>;

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
    #[error("anthropic live executor failed: {0}")]
    AnthropicExecutorFailed(String),
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
    #[error("live bridge role `{expected}` requires matching agent mode, got `{actual}`")]
    AgentModeMismatch { expected: String, actual: String },
    #[error("worker live execution requires a target workspace")]
    WorkerWorkspaceRequired,
    #[error("live turn cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiveReasonExecutionRole {
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

    fn tool_definitions(self, registry: &BuiltinToolRegistry) -> Vec<ProviderToolDefinition> {
        match self {
            Self::Master => registry.master_implemented_definitions(),
            Self::Worker => registry.worker_implemented_definitions(),
        }
    }

    fn tool_schema_fingerprint(self, registry: &BuiltinToolRegistry) -> String {
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
        let exponent = retry_index.saturating_sub(1).min(31);
        let multiplier = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX);
        let millis = self
            .initial_backoff_ms
            .saturating_mul(multiplier)
            .min(self.max_backoff_ms);
        Duration::from_millis(millis)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeCheckpointError {
    #[error("checkpoint store bootstrap failed: {0}")]
    StoreBootstrapFailed(String),
    #[error("writable tool `{tool}` is not checkpointable: {message}")]
    UncheckpointableTool { tool: String, message: String },
    #[error("checkpoint snapshot mismatch for `{path}`: {message}")]
    SnapshotMismatch { path: String, message: String },
    #[error("checkpoint persistence failed: {0}")]
    PersistenceFailed(String),
    #[error("checkpoint `{0}` manifest is missing")]
    MissingManifest(String),
    #[error("checkpoint `{checkpoint_id}` blob `{blob}` is missing")]
    MissingBlob { checkpoint_id: String, blob: String },
    #[error("checkpoint rewind failed for `{path}`: {message}")]
    RewindFailed { path: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RuntimeCheckpointManifest {
    checkpoint_id: String,
    agent_id: String,
    session_id: String,
    turn_id: String,
    tool_call_id: String,
    workspace_root: String,
    entries: Vec<RuntimeCheckpointEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RuntimeCheckpointEntry {
    locked_path: String,
    kind: ToolPreviewChangeKind,
    blob_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RuntimeCheckpointLedgerRow {
    event: RuntimeCheckpointLedgerEvent,
    checkpoint_id: String,
    turn_id: String,
    tool_call_id: String,
    changed_paths: Vec<String>,
    detail: Option<String>,
    unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum RuntimeCheckpointLedgerEvent {
    Created,
    Applied,
    Failed,
    Restored,
}

impl RuntimeCheckpointLedgerEvent {
    fn as_status(self) -> &'static str {
        match self {
            RuntimeCheckpointLedgerEvent::Created => "created",
            RuntimeCheckpointLedgerEvent::Applied => "applied",
            RuntimeCheckpointLedgerEvent::Failed => "failed",
            RuntimeCheckpointLedgerEvent::Restored => "restored",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCheckpointSummary {
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

#[derive(Debug, Clone)]
struct RuntimeCheckpointStore {
    workspace_root: PathBuf,
    manifests_dir: PathBuf,
    ledger_path: PathBuf,
    agent_id: AgentId,
    session_id: SessionId,
}

impl RuntimeCheckpointStore {
    fn new(
        runtime_home: &Path,
        agent_id: &AgentId,
        session_id: &SessionId,
    ) -> Result<Self, RuntimeCheckpointError> {
        Self::new_with_workspace_root(
            runtime_home,
            agent_id,
            session_id,
            runtime_home.to_path_buf(),
        )
    }

    fn new_with_workspace_root(
        runtime_home: &Path,
        agent_id: &AgentId,
        session_id: &SessionId,
        workspace_root: PathBuf,
    ) -> Result<Self, RuntimeCheckpointError> {
        let workspace_root = fs::canonicalize(workspace_root)
            .map_err(|err| RuntimeCheckpointError::StoreBootstrapFailed(err.to_string()))?;
        let manifests_dir = runtime_home
            .join("state")
            .join("checkpoints")
            .join(agent_id.as_str())
            .join(session_id.as_str());
        let ledger_dir = runtime_home
            .join("ledgers")
            .join("checkpoints")
            .join(agent_id.as_str());
        fs::create_dir_all(&manifests_dir)
            .map_err(|err| RuntimeCheckpointError::StoreBootstrapFailed(err.to_string()))?;
        fs::create_dir_all(&ledger_dir)
            .map_err(|err| RuntimeCheckpointError::StoreBootstrapFailed(err.to_string()))?;
        Ok(Self {
            workspace_root,
            manifests_dir,
            ledger_path: ledger_dir.join(format!("{}.jsonl", session_id.as_str())),
            agent_id: agent_id.clone(),
            session_id: session_id.clone(),
        })
    }

    fn create_from_preview(
        &self,
        turn: &TurnRecord,
        preview: &ToolPreviewContract,
        tool_name: &str,
    ) -> Result<RuntimeCheckpointManifest, RuntimeCheckpointError> {
        if preview.changes.is_empty() {
            return Err(RuntimeCheckpointError::UncheckpointableTool {
                tool: tool_name.to_owned(),
                message: "preview returned no changes".to_owned(),
            });
        }
        let checkpoint_id =
            checkpoint_id_for(turn.request.turn_id.as_str(), preview.tool_call_id.as_str());
        let checkpoint_dir = self.manifests_dir.join(&checkpoint_id);
        fs::create_dir_all(&checkpoint_dir)
            .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?;

        let mut entries = Vec::with_capacity(preview.changes.len());
        for (index, change) in preview.changes.iter().enumerate() {
            let path = PathBuf::from(&change.locked_path);
            self.ensure_locked_path(&path)?;
            let blob_file = match change.kind {
                ToolPreviewChangeKind::Create => {
                    if change.before_text.is_some() {
                        return Err(RuntimeCheckpointError::UncheckpointableTool {
                            tool: tool_name.to_owned(),
                            message: format!(
                                "preview for `{}` marked create but still carries before_text",
                                path.display()
                            ),
                        });
                    }
                    if path.exists() {
                        return Err(RuntimeCheckpointError::SnapshotMismatch {
                            path: path.display().to_string(),
                            message: "path already exists but preview expected create".to_owned(),
                        });
                    }
                    None
                }
                ToolPreviewChangeKind::Modify | ToolPreviewChangeKind::Delete => {
                    let expected = change.before_text.as_ref().ok_or_else(|| {
                        RuntimeCheckpointError::UncheckpointableTool {
                            tool: tool_name.to_owned(),
                            message: format!(
                                "preview for `{}` is missing before_text",
                                path.display()
                            ),
                        }
                    })?;
                    let current = fs::read_to_string(&path).map_err(|err| {
                        RuntimeCheckpointError::SnapshotMismatch {
                            path: path.display().to_string(),
                            message: err.to_string(),
                        }
                    })?;
                    if current != *expected {
                        return Err(RuntimeCheckpointError::SnapshotMismatch {
                            path: path.display().to_string(),
                            message: "filesystem pre-image no longer matches preview".to_owned(),
                        });
                    }
                    let blob_file = format!("blob-{index}.txt");
                    write_text_atomic(&checkpoint_dir.join(&blob_file), &current)?;
                    Some(blob_file)
                }
            };
            entries.push(RuntimeCheckpointEntry {
                locked_path: path.to_string_lossy().into_owned(),
                kind: change.kind,
                blob_file,
            });
        }

        let manifest = RuntimeCheckpointManifest {
            checkpoint_id: checkpoint_id.clone(),
            agent_id: self.agent_id.as_str().to_owned(),
            session_id: self.session_id.as_str().to_owned(),
            turn_id: turn.request.turn_id.as_str().to_owned(),
            tool_call_id: preview.tool_call_id.as_str().to_owned(),
            workspace_root: self.workspace_root.to_string_lossy().into_owned(),
            entries,
        };
        self.write_manifest(&manifest)?;
        self.append_ledger_row(RuntimeCheckpointLedgerRow {
            event: RuntimeCheckpointLedgerEvent::Created,
            checkpoint_id,
            turn_id: turn.request.turn_id.as_str().to_owned(),
            tool_call_id: preview.tool_call_id.as_str().to_owned(),
            changed_paths: manifest
                .entries
                .iter()
                .map(|entry| entry.locked_path.clone())
                .collect(),
            detail: None,
            unix_seconds: now_unix_seconds(),
        })?;
        Ok(manifest)
    }

    fn mark_applied(
        &self,
        manifest: &RuntimeCheckpointManifest,
    ) -> Result<(), RuntimeCheckpointError> {
        self.append_outcome_row(manifest, RuntimeCheckpointLedgerEvent::Applied, None)
    }

    fn mark_failed(
        &self,
        manifest: &RuntimeCheckpointManifest,
        detail: &str,
    ) -> Result<(), RuntimeCheckpointError> {
        self.append_outcome_row(
            manifest,
            RuntimeCheckpointLedgerEvent::Failed,
            Some(detail.to_owned()),
        )
    }

    fn rewind(
        &self,
        checkpoint_id: &str,
    ) -> Result<RuntimeCheckpointManifest, RuntimeCheckpointError> {
        let manifest = self.load_manifest(checkpoint_id)?;
        if manifest.workspace_root != self.workspace_root.to_string_lossy() {
            return Err(RuntimeCheckpointError::RewindFailed {
                path: manifest.workspace_root,
                message: format!(
                    "current workspace root `{}` does not match manifest workspace root",
                    self.workspace_root.display()
                ),
            });
        }

        for entry in &manifest.entries {
            let path = PathBuf::from(&entry.locked_path);
            self.ensure_locked_path(&path)?;
            match entry.kind {
                ToolPreviewChangeKind::Create => {
                    if path.is_dir() {
                        return Err(RuntimeCheckpointError::RewindFailed {
                            path: path.display().to_string(),
                            message: "expected file path but found directory".to_owned(),
                        });
                    }
                    if path.exists() {
                        fs::remove_file(&path).map_err(|err| {
                            RuntimeCheckpointError::RewindFailed {
                                path: path.display().to_string(),
                                message: err.to_string(),
                            }
                        })?;
                    }
                }
                ToolPreviewChangeKind::Modify | ToolPreviewChangeKind::Delete => {
                    let blob = entry.blob_file.as_ref().ok_or_else(|| {
                        RuntimeCheckpointError::MissingBlob {
                            checkpoint_id: manifest.checkpoint_id.clone(),
                            blob: "(missing blob reference)".to_owned(),
                        }
                    })?;
                    let blob_path = self.manifests_dir.join(&manifest.checkpoint_id).join(blob);
                    let content = fs::read_to_string(&blob_path).map_err(|err| {
                        if blob_path.exists() {
                            RuntimeCheckpointError::RewindFailed {
                                path: path.display().to_string(),
                                message: err.to_string(),
                            }
                        } else {
                            RuntimeCheckpointError::MissingBlob {
                                checkpoint_id: manifest.checkpoint_id.clone(),
                                blob: blob.clone(),
                            }
                        }
                    })?;
                    write_text_atomic(&path, &content)?;
                }
            }
        }

        self.append_outcome_row(&manifest, RuntimeCheckpointLedgerEvent::Restored, None)?;
        Ok(manifest)
    }

    fn load_manifest(
        &self,
        checkpoint_id: &str,
    ) -> Result<RuntimeCheckpointManifest, RuntimeCheckpointError> {
        let path = self.manifest_path(checkpoint_id);
        let raw = fs::read_to_string(&path).map_err(|err| {
            if path.exists() {
                RuntimeCheckpointError::PersistenceFailed(err.to_string())
            } else {
                RuntimeCheckpointError::MissingManifest(checkpoint_id.to_owned())
            }
        })?;
        serde_json::from_str(&raw)
            .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))
    }

    fn list_summaries(&self) -> Result<Vec<RuntimeCheckpointSummary>, RuntimeCheckpointError> {
        let mut manifests: Vec<RuntimeCheckpointManifest> = Vec::new();
        if !self.manifests_dir.exists() {
            return Ok(Vec::new());
        }
        for entry in fs::read_dir(&self.manifests_dir)
            .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?
        {
            let entry =
                entry.map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?;
            if !entry
                .file_type()
                .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?
                .is_dir()
            {
                continue;
            }
            let checkpoint_id = entry.file_name().to_string_lossy().into_owned();
            manifests.push(self.load_manifest(&checkpoint_id)?);
        }

        let ledger_rows = self.read_ledger_rows()?;
        let mut summaries = manifests
            .into_iter()
            .map(|manifest| {
                let latest = ledger_rows
                    .iter()
                    .filter(|row| row.checkpoint_id == manifest.checkpoint_id)
                    .max_by_key(|row| row.unix_seconds);
                RuntimeCheckpointSummary {
                    checkpoint_id: manifest.checkpoint_id,
                    agent_id: AgentId::new(manifest.agent_id),
                    session_id: SessionId::new(manifest.session_id),
                    turn_id: TurnId::new(manifest.turn_id),
                    tool_call_id: manifest.tool_call_id,
                    changed_paths: manifest
                        .entries
                        .iter()
                        .map(|entry| entry.locked_path.clone())
                        .collect(),
                    latest_status: latest
                        .map(|row| row.event.as_status().to_owned())
                        .unwrap_or_else(|| "manifest_only".to_owned()),
                    latest_detail: latest.and_then(|row| row.detail.clone()),
                    updated_unix_seconds: latest.map(|row| row.unix_seconds).unwrap_or(0),
                }
            })
            .collect::<Vec<_>>();
        summaries.sort_by_key(|summary| summary.updated_unix_seconds);
        summaries.reverse();
        Ok(summaries)
    }

    fn read_ledger_rows(&self) -> Result<Vec<RuntimeCheckpointLedgerRow>, RuntimeCheckpointError> {
        if !self.ledger_path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.ledger_path)
            .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?;
        let mut rows = Vec::new();
        for (index, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let row = serde_json::from_str(line).map_err(|err| {
                RuntimeCheckpointError::PersistenceFailed(format!(
                    "checkpoint ledger line {} failed to parse: {err}",
                    index + 1
                ))
            })?;
            rows.push(row);
        }
        Ok(rows)
    }

    fn write_manifest(
        &self,
        manifest: &RuntimeCheckpointManifest,
    ) -> Result<(), RuntimeCheckpointError> {
        let text = serde_json::to_string_pretty(manifest)
            .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?;
        write_text_atomic(&self.manifest_path(&manifest.checkpoint_id), &text)
    }

    fn manifest_path(&self, checkpoint_id: &str) -> PathBuf {
        self.manifests_dir.join(checkpoint_id).join("manifest.json")
    }

    fn append_outcome_row(
        &self,
        manifest: &RuntimeCheckpointManifest,
        event: RuntimeCheckpointLedgerEvent,
        detail: Option<String>,
    ) -> Result<(), RuntimeCheckpointError> {
        self.append_ledger_row(RuntimeCheckpointLedgerRow {
            event,
            checkpoint_id: manifest.checkpoint_id.clone(),
            turn_id: manifest.turn_id.clone(),
            tool_call_id: manifest.tool_call_id.clone(),
            changed_paths: manifest
                .entries
                .iter()
                .map(|entry| entry.locked_path.clone())
                .collect(),
            detail,
            unix_seconds: now_unix_seconds(),
        })
    }

    fn append_ledger_row(
        &self,
        row: RuntimeCheckpointLedgerRow,
    ) -> Result<(), RuntimeCheckpointError> {
        if let Some(parent) = self.ledger_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?;
        }
        let encoded = serde_json::to_string(&row)
            .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.ledger_path)
            .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?;
        writeln!(file, "{encoded}")
            .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))
    }

    fn ensure_locked_path(&self, path: &Path) -> Result<(), RuntimeCheckpointError> {
        if path.starts_with(&self.workspace_root) {
            return Ok(());
        }
        Err(RuntimeCheckpointError::SnapshotMismatch {
            path: path.display().to_string(),
            message: format!(
                "path is outside locked workspace root `{}`",
                self.workspace_root.display()
            ),
        })
    }
}

pub fn rewind_checkpoint(
    runtime_home: impl AsRef<Path>,
    agent_id: &AgentId,
    session_id: &SessionId,
    checkpoint_id: &str,
) -> Result<(), RuntimeCheckpointError> {
    let store = RuntimeCheckpointStore::new(runtime_home.as_ref(), agent_id, session_id)?;
    let _ = store.rewind(checkpoint_id)?;
    Ok(())
}

pub fn list_checkpoints(
    runtime_home: impl AsRef<Path>,
    agent_id: &AgentId,
    session_id: &SessionId,
) -> Result<Vec<RuntimeCheckpointSummary>, RuntimeCheckpointError> {
    RuntimeCheckpointStore::new(runtime_home.as_ref(), agent_id, session_id)?.list_summaries()
}

fn checkpoint_summary_to_ui(summary: RuntimeCheckpointSummary) -> UiCheckpointSummary {
    UiCheckpointSummary {
        checkpoint_id: summary.checkpoint_id,
        agent_id: summary.agent_id,
        session_id: summary.session_id,
        turn_id: summary.turn_id,
        tool_call_id: summary.tool_call_id,
        changed_paths: summary.changed_paths,
        latest_status: summary.latest_status,
        latest_detail: summary.latest_detail,
        updated_unix_seconds: summary.updated_unix_seconds,
    }
}

fn checkpoint_id_for(turn_id: &str, tool_call_id: &str) -> String {
    format!(
        "checkpoint-{}-{}-{}",
        sanitize_identifier(turn_id),
        sanitize_identifier(tool_call_id),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    )
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn now_unix_seconds() -> u64 {
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

fn write_text_atomic(path: &Path, content: &str) -> Result<(), RuntimeCheckpointError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?;
    }
    let temp_path = path.with_extension(format!("tmp-{}", now_unix_seconds()));
    fs::write(&temp_path, content)
        .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))?;
    fs::rename(&temp_path, path)
        .map_err(|err| RuntimeCheckpointError::PersistenceFailed(err.to_string()))
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
    if role == LiveReasonExecutionRole::Worker && request.cwd.is_none() {
        return Err(RuntimeLiveBridgeError::WorkerWorkspaceRequired);
    }
    match (selected.provider.provider_type, selected.provider.protocol) {
        (ProviderType::Anthropic, ConfigProviderProtocol::Messages) => {
            run_live_anthropic_reason_turn(
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

fn run_live_anthropic_reason_turn<FB, FD, FT>(
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
    let agent_id = AgentId::new(selected.name.clone());
    let persistence = ReasonPersistence::new(request.runtime_home.clone(), agent_id.clone());
    let (mut history, restore_status, restored_closed_turns) =
        match persistence.restore(&request.session_id) {
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
                    .map_err(|err| RuntimeLiveBridgeError::RewriteRuntimeFailed(err.to_string()))?,
                LiveReasonRestoreStatus::CreatedNew,
                0,
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
            symbol_path: "run_live_anthropic_reason_turn",
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
                    key: "provider.family".to_owned(),
                    value: json!("anthropic"),
                },
                MetadataEntry {
                    key: "provider.protocol".to_owned(),
                    value: json!("messages"),
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
            function: "run_live_anthropic_reason_turn",
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
                "provider=anthropic/messages".to_owned(),
            ],
        },
    );
    let engine = ReasonTurnEngine::with_debug_hub_and_metadata_center(
        Arc::clone(&debug_hub),
        Arc::clone(&metadata_center),
    );
    let receiver = engine.subscribe(64);
    let mut executor = AnthropicExecutor::new(AnthropicExecutorConfig {
        base_url: selected.provider.base_url.clone(),
        api_key: selected.provider.api_key.clone(),
        anthropic_version: "2023-06-01".to_owned(),
        adapter: AnthropicAdapterConfig {
            max_tokens: DEFAULT_ANTHROPIC_MAX_TOKENS,
        },
    })
    .map_err(map_anthropic_executor_error)?;

    let mut broadcasts = Vec::new();
    let mut schema_rejections = Vec::new();
    let mut consecutive_schema_rejections = 0usize;
    let mut turns = Vec::new();
    let mut round = 0usize;
    let mut tool_executions = 0usize;
    let mut next_prompt = request.prompt.clone();
    let configured_worker = match role {
        LiveReasonExecutionRole::Master => Some(selected.paired_agent_name.as_str()),
        LiveReasonExecutionRole::Worker => None,
    };
    let mut carryover_segments =
        base_live_context_segments(&request.prompt, role, configured_worker);
    let mut tool_exchanges: Vec<ProviderToolExchange> = Vec::new();
    let mut executed_tool_call_ids = Vec::<String>::new();
    let tool_registry = BuiltinToolRegistry::reasonix_aligned();
    let tool_schema_fingerprint = role.tool_schema_fingerprint(&tool_registry);

    loop {
        ensure_live_not_cancelled(&request)?;
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
                    model: selected.provider.default_model.clone(),
                },
            )
            .map_err(|err| RuntimeLiveBridgeError::TurnStartFailed(err.to_string()))?;
        turn.cwd = request
            .cwd
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
        persistence
            .record_turn_started(&history, &turn, schema_rejections.len() as u32)
            .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))?;
        drain_debug_events(&debug_receiver, &mut on_debug);

        let mut semantic_request = build_semantic_request(
            provider_descriptor(selected),
            turn.provider_payload.clone(),
            debug_hub.is_enabled(),
        )
        .map_err(|err| RuntimeLiveBridgeError::ProviderRequestBuildFailed(err.to_string()))?;
        semantic_request.tools = role.tool_definitions(&tool_registry);
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
                symbol_path: "run_live_anthropic_reason_turn",
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
                symbol_path: "run_live_anthropic_reason_turn",
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
                        value: json!("anthropic"),
                    },
                    MetadataEntry {
                        key: "provider.protocol".to_owned(),
                        value: json!("messages"),
                    },
                    MetadataEntry {
                        key: "reason.model".to_owned(),
                        value: json!(selected.provider.default_model.as_str()),
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
                function: "run_live_anthropic_reason_turn",
                status_text: "provider request built",
                detail_lines: vec![
                    format!("round={round}"),
                    format!("stream={}", request.stream),
                    "provider=anthropic/messages".to_owned(),
                    format!("model={}", selected.provider.default_model),
                    format!("tool_definition_count={}", semantic_request.tools.len()),
                    format!(
                        "tool_exchange_count={}",
                        semantic_request.tool_exchanges.len()
                    ),
                ],
            },
        );

        if request.stream {
            let stream_persistence_error = RefCell::new(None::<RuntimeLiveBridgeError>);
            let raw_session_id = turn.request.session_id.clone();
            let raw_turn_id = turn.request.turn_id.clone();
            let raw_trace_id = turn.request.trace_id.clone();
            let stream_result = executor.execute_stream_with_raw(
                &provider_ctx(&turn),
                &semantic_request,
                |raw| {
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
                        return Err(AnthropicExecutorError::Callback(
                            "live bridge failed while persisting raw provider stream".to_owned(),
                        ));
                    }
                    Ok(())
                },
                |batch| {
                    if live_is_cancelled(&request) {
                        *stream_persistence_error.borrow_mut() =
                            Some(RuntimeLiveBridgeError::Cancelled);
                        return Err(AnthropicExecutorError::Callback(
                            "live bridge cancelled while reading stream".to_owned(),
                        ));
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
                        return Err(AnthropicExecutorError::Callback(
                            "live bridge failed while persisting stream output".to_owned(),
                        ));
                    }
                    Ok(())
                },
            );
            if let Some(err) = stream_persistence_error.into_inner() {
                return Err(err);
            }
            if let Err(err) = stream_result {
                let info = classify_anthropic_executor_error(&err);
                let mapped =
                    RuntimeLiveBridgeError::AnthropicExecutorFailed(info.terminal_message());
                record_provider_error_metadata(ProviderErrorMetadataSpec {
                    center: &metadata_center,
                    agent_id: &agent_id,
                    session_id: &request.session_id,
                    turn: &turn,
                    error: &mapped,
                    error_code: &info.code,
                    retry_index: 1,
                    retry_cap: 1,
                })?;
                emit_provider_retry_debug(
                    &debug_hub,
                    &agent_id,
                    &request.session_id,
                    &turn,
                    &info,
                    1,
                    1,
                );
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
                let execute_result = executor.execute_once_with_raw(
                    &provider_ctx(&turn),
                    &semantic_request,
                    |raw| {
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
                            return Err(AnthropicExecutorError::Callback(
                                "live bridge failed while persisting raw provider response"
                                    .to_owned(),
                            ));
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
                        let info = classify_anthropic_executor_error(&err);
                        let mapped = RuntimeLiveBridgeError::AnthropicExecutorFailed(
                            info.terminal_message(),
                        );
                        record_provider_error_metadata(ProviderErrorMetadataSpec {
                            center: &metadata_center,
                            agent_id: &agent_id,
                            session_id: &request.session_id,
                            turn: &turn,
                            error: &mapped,
                            error_code: &info.code,
                            retry_index,
                            retry_cap: retry_plan.cap,
                        })?;
                        emit_provider_retry_debug(
                            &debug_hub,
                            &agent_id,
                            &request.session_id,
                            &turn,
                            &info,
                            retry_index,
                            retry_plan.cap,
                        );
                        if !info.retryable || retry_index >= retry_plan.cap {
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
                        ensure_live_not_cancelled(&request)?;
                        sleep_provider_retry(retry_plan.backoff_duration(retry_index));
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

        let pending_tool_calls = pending_tool_calls_for_execution(&turn, &executed_tool_call_ids);
        if !pending_tool_calls.is_empty() {
            consecutive_schema_rejections = 0;
            let mut reached_task_decision = None;
            for tool_call in pending_tool_calls {
                ensure_live_not_cancelled(&request)?;
                let executed_tool_result = execute_registry_tool_call(
                    &tool_registry,
                    &request.runtime_home,
                    request.cwd.as_deref(),
                    role,
                    configured_worker,
                    &turn,
                    &tool_call,
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
                            symbol_path: "run_live_anthropic_reason_turn",
                            observed: ErrorCenterObservedFailure {
                                source_owner: "tool.registry".to_owned(),
                                source_pipeline_node: "RuntimeLive03ToolExecuted".to_owned(),
                                code: "tool_result_failed".to_owned(),
                                message: tool_result.tool_result.output.clone(),
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
                        symbol_path: "run_live_anthropic_reason_turn",
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
                        symbol_path: "run_live_anthropic_reason_turn",
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
                        function: "run_live_anthropic_reason_turn",
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
                    tool_call,
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
                role,
                configured_worker,
            );
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
                        symbol_path: "run_live_anthropic_reason_turn",
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
                    role,
                    configured_worker,
                );
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
                            symbol_path: "run_live_anthropic_reason_turn",
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
                            symbol_path: "run_live_anthropic_reason_turn",
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
                            function: "run_live_anthropic_reason_turn",
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
                            symbol_path: "run_live_anthropic_reason_turn",
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
                        role,
                        configured_worker,
                    );
                    turns.push(turn);
                    continue;
                }
            }
        }
        let visible_text = public_provider_text;
        match parse_completion_submission_block(&provider_text) {
            Ok(submission) => match validate_completion_submission(&submission)
                .expect("completion submission already validated")
            {
                CompletionDecision::Completed { .. } | CompletionDecision::Blocked { .. } => {
                    ensure_live_not_cancelled(&request)?;
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
                            symbol_path: "run_live_anthropic_reason_turn",
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
                            function: "run_live_anthropic_reason_turn",
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
                        role,
                        configured_worker,
                    );
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
                        symbol_path: "run_live_anthropic_reason_turn",
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
                            symbol_path: "run_live_anthropic_reason_turn",
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
                            function: "run_live_anthropic_reason_turn",
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
                    role,
                    configured_worker,
                );
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
    let paired_pair_token = env::var(&selected.paired_pair_token_env).map_err(|_| {
        RuntimeAgentBootstrapError::MissingPairedTokenEnv {
            paired_agent_name: selected.paired_agent_name.clone(),
            env_var: selected.paired_pair_token_env.clone(),
        }
    })?;
    if paired_pair_token.trim().is_empty() {
        return Err(RuntimeAgentBootstrapError::EmptyPairedTokenEnv {
            paired_agent_name: selected.paired_agent_name.clone(),
            env_var: selected.paired_pair_token_env.clone(),
        });
    }
    if paired_pair_token != selected.pair_token {
        return Err(RuntimeAgentBootstrapError::PairTokenMismatch {
            agent_name: selected.name.clone(),
            paired_agent_name: selected.paired_agent_name.clone(),
        });
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

        Self::new(RuntimeCommandDispatcherConfig {
            session_id: SessionId::new(format!("runtime-session-{}", selected.name)),
            reason_agent_id: AgentId::new(selected.name.clone()),
            master_agent_id: AgentId::new(selected.name.clone()),
            master_node_id: selected.node_id.clone(),
            slave_agent_id: AgentId::new(selected.paired_agent_name.clone()),
            slave_node_id: selected.paired_node_id.clone(),
            pair_token: selected.pair_token.clone(),
            allowed_pair_ip: selected.paired_allowed_pair_ip.map(|ip| ip.to_string()),
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
            UiCommand::QueryConfigStatus => {
                if let Some(status) = state.pending_config_status.clone() {
                    return Ok(Some(UiQueryResult::ConfigStatus(status)));
                }
                let Some(live) = state.config.live.as_ref() else {
                    return Ok(None);
                };
                Ok(Some(UiQueryResult::ConfigStatus(
                    project_config_status_for_ui(&live.selected_agent),
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
    ) -> Option<PreparedLiveSubmit> {
        let live = state.config.live.clone()?;
        let session_id = requested_session_id.unwrap_or_else(|| state.config.session_id.clone());
        let cwd = resolve_session_cwd(state, &session_id, requested_cwd, Some(&live.runtime_home))
            .ok()?;
        state.next_turn_ordinal += 1;
        let turn_id = TurnId::new(format!("runtime-turn-{}", state.next_turn_ordinal));
        let trace_id = TraceId::new(format!("runtime-trace-{}", state.next_turn_ordinal));
        let cancel_token = Arc::new(AtomicBool::new(false));
        state.active_turns.push(ActiveRuntimeTurn {
            turn_id: turn_id.clone(),
            session_id: session_id.clone(),
            cwd: cwd.clone(),
            trace_id: trace_id.clone(),
            user_text: text.clone(),
            cancel_token: Arc::clone(&cancel_token),
        });
        Some(PreparedLiveSubmit {
            live,
            reason_agent_id: state.config.reason_agent_id.clone(),
            master_node_id: state.config.master_node_id.clone(),
            session_id,
            cwd,
            turn_id,
            trace_id,
            prompt: text,
            cancel_token,
        })
    }

    fn publish_prepared_live_submit(&self, prepared: &PreparedLiveSubmit) {
        publish_live_pending_user_projection(
            &self.ui_state,
            &prepared.reason_agent_id,
            &prepared.master_node_id,
            &prepared.session_id,
            &prepared.cwd,
            &prepared.turn_id,
            &prepared.prompt,
        );
    }

    fn dispatch_prepared_live_submit(
        &self,
        envelope: UiCommandDispatchEnvelope,
        prepared: PreparedLiveSubmit,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        self.publish_prepared_live_submit(&prepared);
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
                cwd: Some(prepared.cwd.clone()),
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
        if was_cancelled {
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
                state.turns.extend(outcome.turns.clone());
                self.ui_state
                    .lock()
                    .expect("lock ui state")
                    .apply_turn_projection(projection);
                self.refresh_checkpoint_projection_from_config(&state.config)
                    .map_err(map_checkpoint_dispatch_error)?;
                Ok(outcome)
            }
            Err(err) => {
                let persistence = ReasonPersistence::new(
                    state
                        .config
                        .live
                        .as_ref()
                        .expect("live submit requires live config")
                        .runtime_home
                        .clone(),
                    state.config.reason_agent_id.clone(),
                );
                let restored =
                    persistence
                        .restore(&prepared.session_id)
                        .map_err(|restore_err| {
                            UiCommandDispatchPortError::DispatchFailed(format!(
                                "failed to project live error turn from persistence: {restore_err}"
                            ))
                        })?;
                let mut restored_turns = persistence
                    .restore_turn_snapshots_for_ui(&prepared.session_id)
                    .map_err(|restore_err| {
                        UiCommandDispatchPortError::DispatchFailed(format!(
                            "failed to restore effective session turns for live error projection: {restore_err}"
                        ))
                    })?;
                if let Some(active_turn) = restored.active_turn {
                    restored_turns.push(active_turn.turn);
                }
                state
                    .turns
                    .retain(|turn| turn.request.session_id != prepared.session_id);
                state.turns.extend(restored_turns);
                state
                    .turns
                    .sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
                state.session_cwds = session_cwds_from_turns(&state.turns);
                let current_turn =
                    current_runtime_turn_for_projection(&state.turns, &prepared.turn_id)?;
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
            publish_live_cancelled_projection(
                &self.ui_state,
                &state.config.reason_agent_id,
                &state.config.master_node_id,
                &active,
            );
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

fn project_config_status_for_ui(selected: &SelectedAgentConfig) -> UiConfigStatusProjection {
    UiConfigStatusProjection {
        agent_name: selected.name.clone(),
        agent_mode: selected.mode.as_str().to_owned(),
        node_id: selected.node_id.clone(),
        paired_agent_name: selected.paired_agent_name.clone(),
        paired_agent_mode: selected.paired_agent_mode.as_str().to_owned(),
        paired_node_id: selected.paired_node_id.clone(),
        provider_id: selected.provider.id.clone(),
        provider_type: selected.provider.provider_type.as_str().to_owned(),
        provider_protocol: selected.provider.protocol.as_str().to_owned(),
        provider_base_url_host: config_base_url_host_for_ui(&selected.provider.base_url),
        default_model: selected.provider.default_model.clone(),
        provider_auth_type: selected.provider.auth_type.as_str().to_owned(),
        provider_auth_source: selected.provider.auth_source.as_str().to_owned(),
        restart_required_on_change: selected.restart_required_on_change,
    }
}

fn config_base_url_host_for_ui(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    let without_userinfo = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let host = if without_userinfo.starts_with('[') {
        without_userinfo
            .split(']')
            .next()
            .map(|value| format!("{value}]"))
            .unwrap_or_else(|| without_userinfo.to_owned())
    } else {
        without_userinfo
            .split(':')
            .next()
            .unwrap_or(without_userinfo)
            .to_owned()
    };
    if host.trim().is_empty() {
        "<invalid-host>".to_owned()
    } else {
        host
    }
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
            symbol_path: "run_live_anthropic_reason_turn",
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
            symbol_path: "run_live_anthropic_reason_turn",
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
        } = envelope.command.clone()
        {
            let prepared = {
                let mut state = self.state.lock().expect("lock runtime dispatcher state");
                self.prepare_live_submit_user_input(
                    &mut state,
                    text.clone(),
                    session_id.clone(),
                    cwd.clone(),
                )
            };
            if let Some(prepared) = prepared {
                return self.dispatch_prepared_live_submit(envelope, prepared);
            }
            let mut state = self.state.lock().expect("lock runtime dispatcher state");
            return self.dispatch_submit_user_input(&mut state, envelope, text, session_id, cwd);
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
        let selected = update_provider_config_in_path(
            &config_path,
            ProviderConfigUpdate {
                agent_name: update.agent_name,
                provider_id: update.provider_id,
                provider_type: update.provider_type,
                protocol: update.provider_protocol,
                base_url: update.base_url,
                default_model: update.default_model,
                api_key_env: update.api_key_env,
            },
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
        state.pending_config_status = Some(project_config_status_for_ui(&selected));
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            target_owner_module: envelope.target_owner_module,
            dispatch_status: "provider_config_saved_restart_required".to_owned(),
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
        | NodeRuntimeError::MetadataWriteFailed(_) => {
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
    UiAgentLifecycleProjection {
        agent_id: lifecycle.agent_id,
        role: lifecycle.role,
        alive: lifecycle.alive,
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
    UiTaskSnapshotProjection {
        task_id: task.task_id.as_str().to_owned(),
        status: task_status_label(&task.status).to_owned(),
        title: task.title,
        goal: task.goal,
        priority: task.priority,
        target_cwd: task.target_cwd,
        assignee_agent_id: task.assignee.map(|assignee| assignee.agent_id),
        active_execution_id: task.active_execution_id,
        updated_at: task.updated_at,
        last_progress_at: task.last_progress_at,
        last_event_seq: task.last_event_seq,
    }
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

fn record_live_provider_raw(
    persistence: &ReasonPersistence,
    session_id: &SessionId,
    turn_id: &TurnId,
    trace_id: &TraceId,
    provider_family: ProviderFamily,
    raw: &AnthropicRawCapture,
) -> Result<(), RuntimeLiveBridgeError> {
    let (raw_kind, function, raw_exchange_id, body, headers) = match raw {
        AnthropicRawCapture::ResponseBody { body } => (
            "response_body",
            "AnthropicExecutor::execute_once_with_raw",
            Some("response-body".to_owned()),
            body.clone(),
            BTreeMap::new(),
        ),
        AnthropicRawCapture::HttpErrorBody { status, body } => (
            "http_error_body",
            "AnthropicExecutor::send_rendered_request",
            Some(format!("http-status:{status}")),
            body.clone(),
            BTreeMap::from([("http-status".to_owned(), status.to_string())]),
        ),
        AnthropicRawCapture::StreamEventBody {
            event_index,
            event_body,
        } => (
            "stream_event_body",
            "AnthropicExecutor::execute_stream_with_raw",
            Some(format!("stream-event:{event_index}")),
            event_body.clone(),
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
                crate_name: "freehand-provider-anthropic".to_owned(),
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

fn map_anthropic_executor_error(err: AnthropicExecutorError) -> RuntimeLiveBridgeError {
    let info = classify_anthropic_executor_error(&err);
    RuntimeLiveBridgeError::AnthropicExecutorFailed(info.terminal_message())
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
        },
        AnthropicExecutorError::Http(err) => ProviderExecutorErrorInfo {
            code: "anthropic_http_request_failed".to_owned(),
            message: err.to_string(),
            retryable: err.is_connect() || err.is_timeout() || err.is_request(),
        },
        AnthropicExecutorError::StreamRead(err) => ProviderExecutorErrorInfo {
            code: "anthropic_stream_read_failed".to_owned(),
            message: err.to_string(),
            retryable: true,
        },
        AnthropicExecutorError::Adapter(err) => ProviderExecutorErrorInfo {
            code: "anthropic_adapter_failed".to_owned(),
            message: err.to_string(),
            retryable: false,
        },
        AnthropicExecutorError::InvalidConfig => ProviderExecutorErrorInfo {
            code: "anthropic_invalid_config".to_owned(),
            message: err.to_string(),
            retryable: false,
        },
        AnthropicExecutorError::Callback(message) => ProviderExecutorErrorInfo {
            code: "anthropic_callback_failed".to_owned(),
            message: message.clone(),
            retryable: false,
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

fn sleep_provider_retry(duration: Duration) {
    if duration.is_zero() {
        return;
    }
    thread::sleep(duration);
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
            pipeline_node: "RuntimeLive05ProviderError",
            metadata_suffix: format!("provider_error:{}", spec.retry_index),
            symbol_path: "run_live_anthropic_reason_turn",
            observed: ErrorCenterObservedFailure {
                source_owner: "provider.reason-live-bridge".to_owned(),
                source_pipeline_node: "RuntimeLive05ProviderError".to_owned(),
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
            pipeline_node: "RuntimeLive05ProviderError",
            metadata_suffix: format!("provider_error:{}", spec.retry_index),
            symbol_path: "run_live_anthropic_reason_turn",
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
) {
    emit_live_bridge_debug(
        debug_hub,
        agent_id,
        session_id,
        RuntimeDebugEmitSpec {
            turn_id: &turn.request.turn_id,
            trace_id: &turn.request.trace_id,
            pipeline_node: "RuntimeLive05ProviderError",
            function: "run_live_anthropic_reason_turn",
            status_text: "provider error retry scheduled",
            detail_lines: vec![
                format!("error_code={}", error.code),
                format!("retry_index={retry_index}"),
                format!("retry_cap={retry_cap}"),
                format!("retryable={}", error.retryable),
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
            source: "freehand_runtime".to_owned(),
            reference: Some("completion_schema_guidance".to_owned()),
        },
    }
}

fn control_status_contract_segment() -> ContextSegment {
    ContextSegment {
        segment_id: ContextSegmentId::new("control-status-contract"),
        kind: ContextSegmentKind::CompletionContract,
        stability: ContextStability::Stable,
        cache_policy: ContextCachePolicy::CacheAnchor,
        role: ContextRole::Developer,
        content: concat!(
            "Freehand may read hidden interaction status from exactly one tagged JSON block:\n",
            "<<<freehand_status>>>\n",
            "{\n",
            "  \"schema_version\": 1,\n",
            "  \"status\": {\n",
            "    \"simple_question\": true | false,\n",
            "    \"task_complete\": true | false,\n",
            "    \"evidence\": \"required when task_complete=true\",\n",
            "    \"next_step\": \"required when task_complete=false and more reasoning is needed\",\n",
            "    \"blocked\": true | false,\n",
            "    \"blocked_reason\": \"required when blocked=true\",\n",
            "    \"needs_user_involvement\": true | false,\n",
            "    \"options\": [\"required when needs_user_involvement=true\"]\n",
            "  }\n",
            "}\n",
            "<</freehand_status>>>\n",
            "Status has no side effects. Use built-in tools for task mutations."
        )
        .to_owned(),
        token_budget: 1024,
        provenance: ContextProvenance {
            source: "freehand_control".to_owned(),
            reference: Some("control_status_schema_v1".to_owned()),
        },
    }
}

fn tool_guidance_segment(
    role: LiveReasonExecutionRole,
    configured_worker: Option<&str>,
) -> ContextSegment {
    let content = match role {
        LiveReasonExecutionRole::Master => master_task_orchestration_guidance(
            configured_worker.expect("Master guidance requires configured Worker"),
        ),
        LiveReasonExecutionRole::Worker => worker_execution_guidance().to_owned(),
    };
    ContextSegment {
        segment_id: ContextSegmentId::new("runtime-tool-guidance"),
        kind: ContextSegmentKind::DeveloperPolicy,
        stability: ContextStability::Stable,
        cache_policy: ContextCachePolicy::CacheAnchor,
        role: ContextRole::Developer,
        token_budget: runtime_prompt_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some("runtime_tool_guidance".to_owned()),
        },
    }
}

fn worker_execution_guidance() -> &'static str {
    concat!(
        "Use the available Freehand tool registry to complete the assigned Worker task inside the locked task workspace, then provide the required Freehand completion schema.\n\n",
        "Worker execution policy:\n",
        "- Role: you are a Worker executing one task assigned by the Master through Task Center.\n",
        "- Stay inside the provided workspace and satisfy the task goal, deliverables, and acceptance criteria.\n",
        "- Use repository read/search/write and shell tools when needed; report concrete evidence in the final completion schema.\n",
        "- Path handling: the runner has already canonicalized target_cwd and locked your current workspace to that canonical directory. When the task mentions extra paths, first check whether each path is absolute, whether it contains a leading ~, and whether any path component is a symlink before reading, writing, or reporting that a path is missing.\n",
        "- Symlink handling: if a task path is or passes through a symlink, report both the user-facing path and the canonical resolved path in evidence. Do not treat a symlinked path as missing merely because the textual path differs from the canonical path.\n",
        "- Missing-path handling: if a required source path or output path cannot be resolved from inside the locked workspace/tool policy, return blocked with the exact path, canonicalization error, and the smallest required external action. Do not invent alternate output directories.\n",
        "- Do not create, assign, claim, approve, reject, close, or delegate tasks. Task lifecycle is owned by the framework and Master.\n",
        "- Do not invent task-management tools or attempt recursive subagent delegation.\n",
        "- Tool validation and execution failures are model-visible results. Correct the call and continue when possible; mark the completion blocked only when the assigned work cannot proceed.\n"
    )
}

fn master_task_orchestration_guidance(configured_worker: &str) -> String {
    format!(
        "{}Configured paired Worker id: `{configured_worker}`.\n\
- Current topology: assign production tasks only to this configured Worker id. Historical agents returned by list_agents are persisted history, not eligible production dispatch targets.\n\
- Worker lifecycle boundary: never put task(...), claim_next, heartbeat, record_execution, approve, reject, or close instructions into Worker task content. The Worker does not receive the task tool. The production Worker runner owns claim/heartbeat and converts the Worker completion schema into TaskReviewSubmitted or TaskBlocked truth.\n\n{}",
        concat!(
            "Use the available Freehand tool registry when it helps the task. Choose the smallest sufficient tool for repository inspection or task bookkeeping, then continue and provide the required Freehand completion schema.\n\n",
            "Master task orchestration policy:\n",
            "- Role: you are the master agent. You own the user conversation, task decomposition, worker coordination, review, and final user-facing answer.\n",
            "- Dispatch when: work targets another cwd/repository, needs isolated context, has independent evidence gathering, can run concurrently, is long-running, or should be resumable outside your main context.\n",
            "- Do not dispatch when: the request is conversational, explanatory, or small enough to complete inside your current allowed workspace without isolated execution.\n",
            "- Workspace boundary: do not directly execute work outside your allowed workspace. Create or reuse a worker resource, create a task with target_cwd, assign it, then let the production Worker runner claim and execute it.\n",
            "- Path duty before dispatch: for any user-supplied path, identify whether it is absolute or starts with ~. Treat ~ as the user's home path from the request context, not as the Master's runtime workspace. If the path is outside your allowed workspace, do not probe it repeatedly with Master workspace tools; dispatch a Worker task that resolves the path.\n",
            "- Symlink duty before dispatch: when a user path may include symlinks, instruct the Worker to check the path itself and each parent component for symlinks, resolve the canonical path, and report both the requested path and canonical path. The task goal/acceptance must preserve the original user-facing path and require canonical-path evidence.\n",
            "- target_cwd rule: target_cwd must be the repository/workspace the Worker should operate in, or an explicitly requested existing output workspace. Do not invent /workspace, /tmp, or a sibling output directory when the user supplied a repository path. If a separate output directory is required, make it a deliverable location inside the Worker task only after confirming it exists or asking for creation.\n",
            "- Missing path rule: if a user path cannot be resolved by the Worker, leave the task blocked with exact path evidence and required external action. Do not convert missing-path evidence into broad filesystem searches or silently switch target_cwd.\n",
            "- Multi-agent dispatch: split independent repository/slice work into separate worker tasks, keep each worker focused, then review and synthesize typed worker results in the master answer.\n",
            "- Concurrency control: assign only useful independent subtasks; avoid duplicate dispatch for work already running, recovering, blocked, or review_ready; poll task truth before starting more work.\n",
            "- Flow control: use task(op=\"list_agents\"), task(op=\"list_tasks\"), task(op=\"query\"), and task(op=\"history\") to inspect current framework truth before dispatching duplicates, retrying, approving, rejecting, or closing work.\n",
            "- Task tool workflow: create_agent only when needed; create a task with goal, deliverables, acceptance, target_cwd, and priority; assign it; query task/history while the Worker runner claims, heartbeats, and records execution; approve/reject; close only after accepted review.\n",
            "- Task create dispatch: always set dispatch.mode to none and then assign the configured Worker, or set dispatch.mode to agent with the exact configured Worker id. Never omit dispatch and never use auto or self dispatch, because persisted historical agents are not production targets.\n",
            "- Ownership boundary: as Master, do not call claim_next, heartbeat, or record_execution on behalf of a Worker. Those mutations are owned by the Worker runner. Use them only in explicit framework/debug tests, never as normal production orchestration.\n",
        ),
        concat!(
            "Master task orchestration examples:\n",
            "- Use the owner-scoped task tool; do not invent query_task_board, dispatch_subtask, approve_submission, or reject_submission tool names.\n",
            "- Create worker resources with task(op=\"create_agent\") only when the task needs a worker id that does not exist.\n",
            "- Create and dispatch work with task(op=\"create\") and task(op=\"assign\"). Keep the same task_id and agent_id while the Worker runner creates and preserves the execution_id.\n",
            "- Cross-workspace sample: for a request comparing ~/code/codex with ~/code/Deepseek-reasonix, create one task for the Codex repository analysis and one task for the Reasonix repository analysis, each with target_cwd, deliverables, acceptance, and evidence requirements; assign/claim separate workers when available, then synthesize the comparison only after reviewing the worker results.\n",
            "- Symlinked repo sample: for a request analyzing ~/github/project where ~/github may be a symlink, create a Worker task with the requested repo path as target_cwd and acceptance requiring `pwd -P`, `ls -ld` on the path and parents, and evidence of the canonical resolved path before repository analysis. Do not first search ~/ or /Users from the Master.\n",
            "- Worker success sample: wait for task history to contain Worker-owned review_ready, then task(op=\"approve\"), then task(op=\"close\").\n",
            "- Worker execution error sample: inspect Worker-owned blocked truth and its evidence. Do not close this as success.\n",
            "- Worker retry sample: after task(op=\"reject\"), leave the task and requirements in Task Center for Worker-owned retry/recovery; inspect the next review_ready result before approval.\n",
            "- Tool validation, task transition errors, and worker execution errors are normal model-visible tool results. Use the returned result to decide the next task action instead of treating it as provider failure.\n"
        )
    )
}

fn original_task_segment(prompt: &str) -> ContextSegment {
    let content = format!("Original operator task:\n{prompt}");
    ContextSegment {
        segment_id: ContextSegmentId::new("original-task"),
        kind: ContextSegmentKind::TaskContract,
        stability: ContextStability::SessionStable,
        cache_policy: ContextCachePolicy::Cacheable,
        role: ContextRole::Developer,
        token_budget: runtime_prompt_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some("original_task".to_owned()),
        },
    }
}

fn base_live_context_segments(
    original_prompt: &str,
    role: LiveReasonExecutionRole,
    configured_worker: Option<&str>,
) -> Vec<ContextSegment> {
    vec![
        completion_contract_segment(),
        control_status_contract_segment(),
        tool_guidance_segment(role, configured_worker),
        original_task_segment(original_prompt),
    ]
}

fn runtime_prompt_segment_token_budget(content: &str) -> u32 {
    let estimated = content.chars().count().div_ceil(4);
    u32::try_from(estimated)
        .unwrap_or(u32::MAX)
        .saturating_add(256)
        .max(512)
}

fn next_round_segments(
    original_prompt: &str,
    visible_text: &str,
    rejection_feedback: Option<&str>,
    role: LiveReasonExecutionRole,
    configured_worker: Option<&str>,
) -> Vec<ContextSegment> {
    let mut segments = base_live_context_segments(original_prompt, role, configured_worker);
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
    configured_worker: Option<&str>,
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
                    configured_worker,
                    turn,
                    tool_call,
                );
            }
            if registry.execution_scope(tool_name) == Some(BuiltinToolExecutionScope::Shell) {
                return Ok(master_workspace_denied_result(
                    turn,
                    tool_call,
                    runtime_home,
                    workspace_root,
                    "unsandboxed shell execution is not available to the master",
                ));
            }
            let root = fs::canonicalize(runtime_home).map_err(|err| {
                RuntimeLiveBridgeError::ToolExecutionFailed(format!(
                    "cannot canonicalize master runtime home `{}`: {err}",
                    runtime_home.display()
                ))
            })?;
            if registry.execution_scope(tool_name) == Some(BuiltinToolExecutionScope::Workspace)
                && let Some(requested_root) = workspace_root
            {
                let requested_root = fs::canonicalize(requested_root).map_err(|err| {
                    RuntimeLiveBridgeError::ToolExecutionFailed(format!(
                        "cannot canonicalize requested workspace `{}`: {err}",
                        requested_root.display()
                    ))
                })?;
                if !requested_root.starts_with(&root) {
                    return Ok(master_workspace_denied_result(
                        turn,
                        tool_call,
                        &root,
                        Some(&requested_root),
                        "requested workspace is outside the master runtime home",
                    ));
                }
            }
            root
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
    with_workspace_root(&root, || {
        execute_registry_tool_call_with_workspace(
            registry,
            runtime_home,
            &root,
            role,
            configured_worker,
            turn,
            tool_call,
        )
    })
    .map_err(|err| RuntimeLiveBridgeError::ToolExecutionFailed(err.to_string()))?
}

fn master_workspace_denied_result(
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
    allowed_root: &Path,
    requested_root: Option<&Path>,
    reason: &str,
) -> ExecutedToolResult {
    let requested = requested_root
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "(tool request)".to_owned());
    ExecutedToolResult {
        result: tool_result_reentry(
            turn,
            tool_call,
            ToolResultStatus::Failed,
            format!(
                "Master workspace boundary denied direct access: {reason}. allowed_root={} requested_target={requested}. This is a Master scope/permission boundary, not evidence that the external path is missing. Preserve the requested path and delegate with task(op=\"create_agent\") when no worker exists, task(op=\"create\", target_cwd=\"{requested}\"), and task(op=\"assign\") so a worker performs the external work.",
                allowed_root.display()
            ),
        ),
        task_truth_changed: false,
    }
}

fn registry_error_text(role: LiveReasonExecutionRole, error: &ToolRegistryError) -> String {
    match error {
        ToolRegistryError::WorkspaceBoundaryViolation { root, target, .. } => match role {
            LiveReasonExecutionRole::Master => format!(
                "Master workspace boundary denied direct access: requested target `{target}` is outside `{root}`. This is a Master scope/permission boundary, not evidence that `{target}` is missing. Preserve the requested path and delegate with task(op=\"create_agent\") when no worker exists, task(op=\"create\", target_cwd=\"{target}\"), and task(op=\"assign\") so a worker performs the external work."
            ),
            LiveReasonExecutionRole::Worker => format!(
                "Worker workspace boundary: requested target `{target}` is outside locked task workspace `{root}`."
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
    configured_worker: Option<&str>,
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<ExecutedToolResult, RuntimeLiveBridgeError> {
    let tool_name = tool_call.tool_call.tool_name.as_str();
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
        if let Some(message) = configured_worker_task_boundary_failure(tool_call, configured_worker)
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

fn execute_task_tool(
    runtime_home: &Path,
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<String, String> {
    let args = tool_arguments_object(&tool_call.tool_call.arguments);
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
            Ok(format!(
                "Task created: task_id={} status={:?} events={}",
                outcome.task.task_id.as_str(),
                outcome.task.status,
                outcome.events.len()
            ))
        }
        "query" => {
            let task_id = TaskId::new(required_json_string(&args, "task_id")?);
            let task = task_runtime
                .query_task(&task_id)
                .map_err(|err| err.to_string())?;
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
    configured_worker: Option<&str>,
) -> Option<String> {
    let configured_worker = configured_worker?;
    let args = tool_arguments_object(&tool_call.tool_call.arguments);
    match args.get("op").and_then(Value::as_str) {
        Some("assign") => args
            .get("agent_id")
            .and_then(Value::as_str)
            .filter(|agent_id| *agent_id != configured_worker)
            .map(|_| {
                format!(
                    "Configured topology boundary: task assignment must target paired Worker `{configured_worker}`."
                )
            }),
        Some("create") => match args.get("dispatch") {
            None => Some(format!(
                "Configured topology boundary: task creation must set dispatch.mode to `none` for later assignment, or `agent` with agent_id `{configured_worker}`. Implicit dispatch is not allowed because it can select historical agents."
            )),
            Some(Value::Object(dispatch)) => match dispatch.get("mode").and_then(Value::as_str) {
                Some("none") => None,
                Some("agent") => dispatch
                    .get("agent_id")
                    .and_then(Value::as_str)
                    .filter(|agent_id| *agent_id != configured_worker)
                    .map(|_| {
                        format!(
                            "Configured topology boundary: task creation may dispatch only to paired Worker `{configured_worker}`."
                        )
                    }),
                Some("auto" | "self") => Some(format!(
                    "Configured topology boundary: task creation cannot use auto or self dispatch. Use dispatch.mode `none`, then assign paired Worker `{configured_worker}`, or dispatch directly to that Worker."
                )),
                _ => None,
            },
            Some(_) => None,
        },
        _ => None,
    }
}

fn task_tool_call_mutates_truth(tool_call: &ReasonReq04ToolCall) -> bool {
    let args = tool_arguments_object(&tool_call.tool_call.arguments);
    let Some(Value::String(op)) = args.get("op") else {
        return false;
    };
    matches!(
        op.as_str(),
        "create"
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

fn apply_runtime_reason_broadcast(
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

fn apply_runtime_debug_event(
    ui_state: &Arc<Mutex<UiProtocolState>>,
    reason_agent_id: &AgentId,
    master_node_id: &str,
    event: &DebugEvent,
) {
    let mut ui = ui_state.lock().expect("lock ui state");
    if event.envelope.semantic.pipeline_node.as_deref() == Some("RuntimeLive02ProviderRequestBuilt")
    {
        ui.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: reason_agent_id.clone(),
            source_node_id: master_node_id.to_owned(),
            session_id: event.envelope.semantic.session_id.clone(),
            turn_id: event.envelope.semantic.turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail: event
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.status_text.clone()),
            slave_substream_card: false,
        });
    }
    let _ = ui.apply_debug_event(event);
}

fn publish_live_pending_user_projection(
    ui_state: &Arc<Mutex<UiProtocolState>>,
    reason_agent_id: &AgentId,
    master_node_id: &str,
    session_id: &SessionId,
    cwd: &Path,
    base_turn_id: &TurnId,
    user_text: &str,
) {
    ui_state
        .lock()
        .expect("lock ui state")
        .apply_turn_projection(turn_projection_for_client(
            turn_projection_from_events(TurnProjectionInput {
                source_agent_id: reason_agent_id.clone(),
                source_node_id: master_node_id.to_owned(),
                session_id: session_id.clone(),
                turn_id: derived_turn_id(base_turn_id, 1),
                cwd: Some(cwd.to_string_lossy().into_owned()),
                user_text: Some(user_text.to_owned()),
                semantic_events: Vec::new(),
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                usage_events: Vec::new(),
                terminal_event: None,
                error_events: Vec::new(),
                slave_substream_card: false,
            }),
            UiClientKind::WebUi,
        ));
}

fn publish_live_cancelled_projection(
    ui_state: &Arc<Mutex<UiProtocolState>>,
    reason_agent_id: &AgentId,
    master_node_id: &str,
    active: &ActiveRuntimeTurn,
) {
    ui_state
        .lock()
        .expect("lock ui state")
        .apply_turn_projection(turn_projection_for_client(
            turn_projection_from_events(TurnProjectionInput {
                source_agent_id: reason_agent_id.clone(),
                source_node_id: master_node_id.to_owned(),
                session_id: active.session_id.clone(),
                turn_id: active.turn_id.clone(),
                cwd: Some(active.cwd.to_string_lossy().into_owned()),
                user_text: Some(active.user_text.clone()),
                semantic_events: Vec::new(),
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                usage_events: Vec::new(),
                terminal_event: Some(freehand_contracts::ReasonResp03TerminalEvent {
                    session_id: active.session_id.clone(),
                    turn_id: active.turn_id.clone(),
                    trace_id: active.trace_id.clone(),
                    feature_id: FeatureId::new("runtime.ui-command-dispatch"),
                    agent_id: reason_agent_id.clone(),
                    status: freehand_contracts::TerminalStatus::Cancelled,
                    summary: "cancelled by ui command".to_owned(),
                }),
                error_events: Vec::new(),
                slave_substream_card: false,
            }),
            UiClientKind::WebUi,
        ));
}

fn project_runtime_turn_history(
    reason_agent_id: &AgentId,
    master_node_id: &str,
    turns: &[TurnRecord],
    cwd: Option<String>,
) -> UiTurnProjection {
    let turn = turns
        .last()
        .expect("runtime turn history projection requires at least one turn");
    turn_projection_for_client(
        turn_projection_from_events(TurnProjectionInput {
            source_agent_id: reason_agent_id.clone(),
            source_node_id: master_node_id.to_owned(),
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            cwd: cwd.or_else(|| turn.cwd.clone()),
            user_text: Some(ui_user_text_for_turn(turn)),
            semantic_events: turn.semantic_events.clone(),
            tool_calls: turn.tool_calls.clone(),
            tool_results: turn.tool_results.clone(),
            usage_events: turn.usage_events.clone(),
            terminal_event: turn.terminal_event.clone(),
            error_events: turn.error_events.clone(),
            slave_substream_card: false,
        }),
        UiClientKind::WebUi,
    )
}

fn current_runtime_turn_for_projection(
    turns: &[TurnRecord],
    base_turn_id: &TurnId,
) -> Result<TurnRecord, UiCommandDispatchPortError> {
    let target_ordinal = runtime_turn_position(base_turn_id).0;
    let current_turn = turns
        .iter()
        .filter(|turn| runtime_turn_position(&turn.request.turn_id).0 == target_ordinal)
        .max_by_key(|turn| runtime_turn_position(&turn.request.turn_id))
        .cloned()
        .ok_or_else(|| {
            UiCommandDispatchPortError::DispatchFailed(format!(
                "failed to project live error turn `{}` from persistence",
                base_turn_id.as_str()
            ))
        })?;
    Ok(current_turn)
}

fn project_runtime_turn(
    reason_agent_id: &AgentId,
    master_node_id: &str,
    turn: &TurnRecord,
    cwd: Option<String>,
) -> UiTurnProjection {
    project_runtime_turn_history(
        reason_agent_id,
        master_node_id,
        std::slice::from_ref(turn),
        cwd,
    )
}

fn restore_all_persisted_sessions_into_ui(
    persistence: &ReasonPersistence,
    ui_state: &Arc<Mutex<UiProtocolState>>,
    reason_agent_id: &AgentId,
    master_node_id: &str,
) -> Result<u64, ReasonPersistenceError> {
    let sessions = persistence.list_persisted_sessions()?;
    let mut ui = ui_state.lock().expect("lock ui state");
    let mut max_turn_ordinal = 0_u64;
    for session in sessions {
        let mut turns = persistence.restore_turn_snapshots_for_ui(&session.session_id)?;
        turns.sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
        for turn in turns {
            max_turn_ordinal = max_turn_ordinal.max(runtime_turn_position(&turn.request.turn_id).0);
            ui.apply_turn_projection(project_runtime_turn_history(
                reason_agent_id,
                master_node_id,
                std::slice::from_ref(&turn),
                None,
            ));
        }
    }
    Ok(max_turn_ordinal)
}

fn ui_user_text_for_turn(turn: &TurnRecord) -> String {
    turn.request
        .context_segments
        .iter()
        .find(|segment| {
            segment.provenance.source == "freehand_runtime"
                && segment.provenance.reference.as_deref() == Some("original_task")
        })
        .and_then(|segment| {
            segment
                .content
                .strip_prefix("Original operator task:\n")
                .map(str::to_owned)
        })
        .unwrap_or_else(|| turn.request.user_text.clone())
}

fn rebuild_session_history_from_effective_turns(
    history: &mut SessionHistory,
    session_id: &SessionId,
    turns: &[TurnRecord],
) -> Result<(), RuntimeLiveBridgeError> {
    let rebuilt_segments = effective_turn_context_segments(turns);
    if rebuilt_segments == history.base_context_segments() {
        return Ok(());
    }
    if rebuilt_segments.is_empty() {
        return Ok(());
    }
    history
        .stage_resume_rebuild(
            rebuilt_segments,
            "rebuild session transcript context from effective persisted turns",
            format!("runtime_restore:{}", session_id.as_str()),
        )
        .map_err(|err| RuntimeLiveBridgeError::RewriteRuntimeFailed(err.to_string()))?;
    Ok(())
}

fn effective_turn_context_segments(turns: &[TurnRecord]) -> Vec<ContextSegment> {
    let mut latest_by_logical_turn: BTreeMap<String, &TurnRecord> = BTreeMap::new();
    for turn in turns {
        let (ordinal, round, raw_turn_id) = runtime_turn_position(&turn.request.turn_id);
        let logical_key = if ordinal == 0 {
            raw_turn_id
        } else {
            ordinal.to_string()
        };
        let replace = latest_by_logical_turn
            .get(&logical_key)
            .map(|existing| {
                let (_, existing_round, _) = runtime_turn_position(&existing.request.turn_id);
                round >= existing_round
            })
            .unwrap_or(true);
        if replace {
            latest_by_logical_turn.insert(logical_key, turn);
        }
    }
    let mut latest_turns = latest_by_logical_turn
        .into_values()
        .collect::<Vec<&TurnRecord>>();
    latest_turns.sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
    latest_turns
        .into_iter()
        .filter_map(turn_context_segment)
        .collect()
}

fn turn_context_segment(turn: &TurnRecord) -> Option<ContextSegment> {
    let user_text = ui_user_text_for_turn(turn);
    let assistant_text = history_visible_assistant_text(turn);
    if user_text.trim().is_empty() && assistant_text.trim().is_empty() {
        return None;
    }
    let (ordinal, round, raw_turn_id) = runtime_turn_position(&turn.request.turn_id);
    let content = if assistant_text.trim().is_empty() {
        format!(
            "Historical turn {} (round {}):\nUser: {}",
            ordinal,
            round,
            user_text.trim()
        )
    } else {
        format!(
            "Historical turn {} (round {}):\nUser: {}\nAssistant: {}",
            ordinal,
            round,
            user_text.trim(),
            assistant_text.trim()
        )
    };
    Some(ContextSegment {
        segment_id: ContextSegmentId::new(format!("session-memory-{raw_turn_id}")),
        kind: ContextSegmentKind::SessionMemory,
        stability: ContextStability::SessionStable,
        cache_policy: ContextCachePolicy::Cacheable,
        role: ContextRole::Developer,
        token_budget: history_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some(format!("historical_turn:{raw_turn_id}")),
        },
    })
}

fn history_visible_assistant_text(turn: &TurnRecord) -> String {
    let visible_text = strip_completion_submission_block(&collect_turn_text(turn));
    if !visible_text.trim().is_empty() {
        return visible_text;
    }
    if let Some(terminal) = turn.terminal_event.as_ref()
        && !terminal.summary.trim().is_empty()
    {
        return terminal.summary.clone();
    }
    if let Some(error) = turn.error_events.last() {
        return format!("{}: {}", error.error.code, error.error.message);
    }
    String::new()
}

fn history_segment_token_budget(content: &str) -> u32 {
    let estimated = ((content.chars().count() as u32) / 4).max(32);
    estimated.saturating_add(64)
}

fn runtime_turn_position(turn_id: &TurnId) -> (u64, u64, String) {
    let raw = turn_id.as_str();
    let Some(rest) = raw.strip_prefix("runtime-turn-") else {
        return (0, 0, raw.to_owned());
    };
    let (ordinal_part, round) = match rest.split_once("-r") {
        Some((ordinal, round)) => (ordinal, round.parse::<u64>().ok().unwrap_or(1)),
        None => (rest, 1),
    };
    let ordinal = ordinal_part.parse::<u64>().ok().unwrap_or(0);
    (ordinal, round, raw.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{FeatureId, SemanticEventKind, TerminalStatus};
    use freehand_contracts::{ToolCallContract, ToolCallId};
    use freehand_metadata::MetadataEnvelope;
    use freehand_reason::ProviderRawLedgerRow;
    use freehand_ui_protocol::{UiQueryResult, build_command_dispatch_envelope};
    use serde_json::{Value, json};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn runtime() -> RuntimeCommandDispatcher {
        RuntimeCommandDispatcher::new(RuntimeCommandDispatcherConfig {
            session_id: SessionId::new("runtime-session"),
            reason_agent_id: AgentId::new("reason-agent"),
            master_agent_id: AgentId::new("master-agent"),
            master_node_id: "master-node".to_owned(),
            slave_agent_id: AgentId::new("slave-agent"),
            slave_node_id: "slave-node".to_owned(),
            pair_token: "pair-token".to_owned(),
            allowed_pair_ip: None,
            model: "runtime-model".to_owned(),
            live: None,
        })
        .expect("runtime")
    }

    #[test]
    fn live_bridge_projection_keeps_each_round_as_its_own_card() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                continue_single_response("first round continue"),
                tool_use_single_response(),
                complete_single_response("final round done"),
            ],
        );

        let request = live_request(false);
        fs::create_dir_all(&request.runtime_home).expect("create runtime home");
        fs::write(request.runtime_home.join("Cargo.toml"), "[workspace]\n")
            .expect("write master workspace fixture");

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("live bridge");
        let first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        let third_request = rx.recv().expect("third request");
        handle.join().expect("join");

        assert!(first_request.contains("reply exactly pong"));
        assert!(second_request.contains("first round continue"));
        assert!(second_request.contains("\"tools\""));
        assert!(third_request.contains("\"type\":\"tool_result\""));
        assert_eq!(outcome.rounds, 3);
        assert_eq!(outcome.tool_executions, 1);

        let first_round_projection = project_runtime_turn_history(
            &AgentId::new("agent-live"),
            "agent-live-node",
            std::slice::from_ref(&outcome.turns[1]),
            None,
        );
        let first_round_public =
            freehand_ui_protocol::public_turn_projection(first_round_projection);
        assert_eq!(
            first_round_public.public_conversation[0].body,
            "reply exactly pong"
        );
        assert!(
            first_round_public
                .public_conversation
                .iter()
                .any(
                    |item| item.kind == freehand_ui_protocol::UiConversationItemKind::ToolSummary
                        && item.status == "completed"
                )
        );
        assert!(
            first_round_public
                .public_conversation
                .iter()
                .all(|item| !item.body.contains("final round done"))
        );

        let final_projection = project_runtime_turn_history(
            &AgentId::new("agent-live"),
            "agent-live-node",
            std::slice::from_ref(&outcome.turn),
            None,
        );
        let public = freehand_ui_protocol::public_turn_projection(final_projection);
        assert_eq!(public.public_conversation[0].body, "reply exactly pong");
        assert!(public.public_conversation.iter().all(|item| {
            item.kind != freehand_ui_protocol::UiConversationItemKind::ToolSummary
        }));
        assert!(
            public
                .public_conversation
                .iter()
                .all(|item| !item.body.contains("first round continue"))
        );
        assert!(
            public
                .public_conversation
                .iter()
                .any(|item| item.body.contains("final round done"))
        );
    }

    #[test]
    fn master_lifecycle_closes_in_same_round_as_target_task_mutation() {
        let runtime_home = temp_runtime_home();
        let runtime =
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
        create_lifecycle_test_worker(&runtime);
        let task = create_lifecycle_test_task(&runtime, "lifecycle-target");
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![task_tool_use_response(
                "toolu_lifecycle_assign",
                json!({
                    "op": "assign",
                    "task_id": task.task_id.as_str(),
                    "agent_id": "worker"
                }),
            )],
        );

        let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        selected.paired_agent_name = "worker".to_owned();
        let outcome = run_master_lifecycle_reason_turn(
            &selected,
            lifecycle_live_request(&runtime_home, "lifecycle-target-event"),
            LiveReasonTaskDecisionBoundary {
                task_id: task.task_id.clone(),
                initial_event_seq: task.last_event_seq,
                mode: LiveReasonTaskDecisionMode::TargetMutation,
                max_rounds: 8,
            },
        )
        .expect("lifecycle decision");

        assert_eq!(outcome.rounds, 1);
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );
        assert_eq!(
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
                .expect("reload task runtime")
                .query_task(&task.task_id)
                .expect("assigned task")
                .assignee
                .expect("configured worker assignee")
                .agent_id,
            AgentId::new("worker")
        );
        let history = TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
            .expect("reload task history runtime")
            .task_history(&task.task_id)
            .expect("task history");
        assert_eq!(
            history
                .iter()
                .filter(|event| event.event_type == "TaskAssigned")
                .count(),
            1
        );
        let _ = rx.recv().expect("single provider request");
        assert!(
            rx.try_recv().is_err(),
            "decision must not request another round"
        );
        handle.join().expect("join provider");
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn master_assignment_gate_pairs_failure_then_accepts_configured_worker() {
        let runtime_home = temp_runtime_home();
        let runtime =
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
        create_lifecycle_test_worker(&runtime);
        runtime
            .create_agent(AgentCreateRequest {
                agent_id: AgentId::new("historical-worker"),
                capabilities: vec!["workspace".to_owned()],
                actor: lifecycle_test_actor(),
                watermark: lifecycle_test_watermark("create-historical-worker"),
            })
            .expect("create historical worker");
        let task = create_lifecycle_test_task(&runtime, "lifecycle-assignment-gate");
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                task_tool_use_response(
                    "toolu_assign_historical",
                    json!({
                        "op": "assign",
                        "task_id": task.task_id.as_str(),
                        "agent_id": "historical-worker"
                    }),
                ),
                task_tool_use_response(
                    "toolu_assign_configured",
                    json!({
                        "op": "assign",
                        "task_id": task.task_id.as_str(),
                        "agent_id": "worker"
                    }),
                ),
            ],
        );

        let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        selected.paired_agent_name = "worker".to_owned();
        let outcome = run_master_lifecycle_reason_turn(
            &selected,
            lifecycle_live_request(&runtime_home, "lifecycle-assignment-gate-event"),
            LiveReasonTaskDecisionBoundary {
                task_id: task.task_id.clone(),
                initial_event_seq: task.last_event_seq,
                mode: LiveReasonTaskDecisionMode::TargetMutation,
                max_rounds: 8,
            },
        )
        .expect("corrected lifecycle assignment");

        assert_eq!(outcome.rounds, 2);
        assert_eq!(outcome.tool_executions, 2);
        let requests = collect_provider_requests(&rx, 2);
        assert!(requests[1].contains(
            "Configured topology boundary: task assignment must target paired Worker `worker`."
        ));
        let reloaded =
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("reload runtime");
        let assigned = reloaded.query_task(&task.task_id).expect("assigned task");
        assert_eq!(assigned.status, TaskStatus::Assigned);
        assert_eq!(
            assigned.assignee.expect("configured assignee").agent_id,
            AgentId::new("worker")
        );
        let history = reloaded.task_history(&task.task_id).expect("task history");
        let assigned_events = history
            .iter()
            .filter(|event| event.event_type == "TaskAssigned")
            .collect::<Vec<_>>();
        assert_eq!(assigned_events.len(), 1);
        assert_eq!(
            assigned_events[0]
                .payload
                .get("agent_id")
                .and_then(Value::as_str),
            Some("worker")
        );

        handle.join().expect("join provider");
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn master_create_gate_rejects_implicit_dispatch_without_task_mutation() {
        let runtime_home = temp_runtime_home();
        let runtime =
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
        create_lifecycle_test_worker(&runtime);
        let task_id = "lifecycle-create-gate";
        let create_payload = json!({
            "op": "create",
            "task_id": task_id,
            "title": "Lifecycle create gate",
            "content": "create one task without historical-agent dispatch",
            "goal": "prove configured Worker creation boundary",
            "deliverables": ["task truth"],
            "acceptance": ["only configured Worker is assigned"],
            "priority": 90,
            "target_cwd": std::env::temp_dir()
        });
        let mut corrected_create_payload = create_payload.clone();
        corrected_create_payload
            .as_object_mut()
            .expect("create payload object")
            .insert("dispatch".to_owned(), json!({"mode": "none"}));
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                task_tool_use_response("toolu_create_implicit_dispatch", create_payload),
                task_tool_use_response("toolu_create_explicit_none", corrected_create_payload),
                task_tool_use_response(
                    "toolu_assign_configured_after_create",
                    json!({
                        "op": "assign",
                        "task_id": task_id,
                        "agent_id": "worker"
                    }),
                ),
                complete_single_response("configured Worker task created and assigned"),
            ],
        );
        let mut request = live_request(false);
        request.runtime_home = runtime_home.clone();
        let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        selected.paired_agent_name = "worker".to_owned();

        let outcome =
            run_live_reason_turn(&selected, request).expect("corrected task creation flow");
        let requests = collect_provider_requests(&rx, 4);
        assert!(
            requests[1]
                .contains("task creation must set dispatch.mode to `none` for later assignment")
        );
        assert!(requests[2].contains("Task created"));
        assert!(requests[3].contains("Task assigned"));
        assert_eq!(outcome.rounds, 4);
        assert_eq!(outcome.tool_executions, 3);

        let reloaded =
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("reload runtime");
        let task = reloaded
            .query_task(&TaskId::new(task_id))
            .expect("created task");
        assert_eq!(task.status, TaskStatus::Assigned);
        assert_eq!(
            task.assignee.expect("configured assignee").agent_id,
            AgentId::new("worker")
        );
        let event_types = reloaded
            .task_history(&TaskId::new(task_id))
            .expect("task history")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec!["TaskCreated", "TaskWaitingAgent", "TaskAssigned"]
        );

        handle.join().expect("join provider");
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn master_lifecycle_ignores_unrelated_task_mutation() {
        let runtime_home = temp_runtime_home();
        let runtime =
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
        create_lifecycle_test_worker(&runtime);
        let target = create_lifecycle_test_task(&runtime, "lifecycle-target");
        let unrelated = create_lifecycle_test_task(&runtime, "lifecycle-unrelated");
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                task_tool_use_response(
                    "toolu_lifecycle_assign_unrelated",
                    json!({
                        "op": "assign",
                        "task_id": unrelated.task_id.as_str(),
                        "agent_id": "worker"
                    }),
                ),
                complete_single_response("unrelated mutation observed"),
            ],
        );

        let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        selected.paired_agent_name = "worker".to_owned();
        let outcome = run_master_lifecycle_reason_turn(
            &selected,
            lifecycle_live_request(&runtime_home, "lifecycle-unrelated-event"),
            LiveReasonTaskDecisionBoundary {
                task_id: target.task_id.clone(),
                initial_event_seq: target.last_event_seq,
                mode: LiveReasonTaskDecisionMode::TargetMutation,
                max_rounds: 8,
            },
        )
        .expect("lifecycle decision");

        assert_eq!(outcome.rounds, 2);
        assert_eq!(
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
                .expect("reload target runtime")
                .query_task(&target.task_id)
                .expect("target task")
                .status,
            TaskStatus::WaitingAgent
        );
        assert_eq!(
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
                .expect("reload unrelated runtime")
                .query_task(&unrelated.task_id)
                .expect("unrelated task")
                .status,
            TaskStatus::Assigned
        );
        let _ = collect_provider_requests(&rx, 2);
        handle.join().expect("join provider");
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn master_lifecycle_round_budget_closes_blocked_without_mutation() {
        let runtime_home = temp_runtime_home();
        let runtime =
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
        let target = create_lifecycle_test_task(&runtime, "lifecycle-budget");
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                continue_single_response("keep waiting"),
                continue_single_response("still waiting"),
            ],
        );

        let outcome = run_master_lifecycle_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            lifecycle_live_request(&runtime_home, "lifecycle-budget-event"),
            LiveReasonTaskDecisionBoundary {
                task_id: target.task_id.clone(),
                initial_event_seq: target.last_event_seq,
                mode: LiveReasonTaskDecisionMode::TargetMutation,
                max_rounds: 2,
            },
        )
        .expect("budget closeout");

        assert_eq!(outcome.rounds, 2);
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Blocked)
        );
        assert!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .expect("terminal")
                .summary
                .contains("exceeded the 2-round budget")
        );
        assert_eq!(
            runtime
                .query_task(&target.task_id)
                .expect("unchanged target")
                .status,
            TaskStatus::WaitingAgent
        );
        let _ = collect_provider_requests(&rx, 2);
        handle.join().expect("join provider");
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_bootstrap_restores_all_persisted_sessions_into_ui_state() {
        let runtime_home = temp_runtime_home();
        let (base_url_a, rx_a, handle_a) = spawn_sequence_server(
            "application/json",
            vec![complete_single_response("answer a")],
        );
        run_live_reason_turn(
            &live_selected_agent(base_url_a, freehand_config::ProviderType::Anthropic),
            live_request_for(&runtime_home, "runtime-session-agent-live", 1),
        )
        .expect("persist session a");
        let _ = rx_a.recv().expect("provider request a");
        handle_a.join().expect("join a");

        let (base_url_b, rx_b, handle_b) = spawn_sequence_server(
            "application/json",
            vec![complete_single_response("answer b")],
        );
        run_live_reason_turn(
            &live_selected_agent(base_url_b, freehand_config::ProviderType::Anthropic),
            live_request_for(&runtime_home, "runtime-session-other", 2),
        )
        .expect("persist session b");
        let _ = rx_b.recv().expect("provider request b");
        handle_b.join().expect("join b");

        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");

        let session_list = runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionList)
            .expect("session list query");
        match session_list {
            UiQueryResult::SessionList(list) => {
                let ids = list
                    .sessions
                    .iter()
                    .map(|session| session.session_id.as_str())
                    .collect::<Vec<_>>();
                assert!(ids.contains(&"runtime-session-agent-live"));
                assert!(ids.contains(&"runtime-session-other"));
            }
            other => panic!("unexpected session list query: {other:?}"),
        }

        let transcript = runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: SessionId::new("runtime-session-other"),
            })
            .expect("session turns query");
        match transcript {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 1);
                assert_eq!(
                    transcript.turns[0].user_text.as_deref(),
                    Some("prompt for runtime-session-other")
                );
                assert!(
                    transcript.turns[0]
                        .terminal_text
                        .as_deref()
                        .is_some_and(|text| text.contains("answer b"))
                );
            }
            other => panic!("unexpected session turns query: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_bridge_restores_same_session_history_into_follow_up_provider_request() {
        let runtime_home = temp_runtime_home();
        let session_id = SessionId::new("runtime-session-history");
        let first_request = LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            session_id: session_id.clone(),
            turn_id: TurnId::new("runtime-turn-1"),
            trace_id: TraceId::new("runtime-trace-1"),
            prompt: "first history prompt".to_owned(),
            cwd: None,
            stream: false,
            cancel_token: None,
        };
        let (base_url_first, rx_first, handle_first) = spawn_sequence_server(
            "application/json",
            vec![complete_single_response("first history answer")],
        );
        let first_outcome = run_live_reason_turn(
            &live_selected_agent(base_url_first, freehand_config::ProviderType::Anthropic),
            first_request,
        )
        .expect("first request");
        let raw_first = rx_first.recv().expect("first provider request");
        handle_first.join().expect("join first provider");
        assert!(raw_first.contains("first history prompt"));
        assert_eq!(
            first_outcome.restore_status,
            LiveReasonRestoreStatus::CreatedNew
        );

        let second_request = LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            session_id: session_id.clone(),
            turn_id: TurnId::new("runtime-turn-2"),
            trace_id: TraceId::new("runtime-trace-2"),
            prompt: "second history prompt".to_owned(),
            cwd: None,
            stream: false,
            cancel_token: None,
        };
        let (base_url_second, rx_second, handle_second) = spawn_sequence_server(
            "application/json",
            vec![complete_single_response("second history answer")],
        );
        let second_outcome = run_live_reason_turn(
            &live_selected_agent(base_url_second, freehand_config::ProviderType::Anthropic),
            second_request,
        )
        .expect("second request");
        let raw_second = rx_second.recv().expect("second provider request");
        handle_second.join().expect("join second provider");

        assert_eq!(
            second_outcome.restore_status,
            LiveReasonRestoreStatus::RestoredExisting
        );
        assert_eq!(second_outcome.restored_closed_turns, 1);
        assert!(raw_second.contains("Historical turn 1 (round 1):"));
        assert!(raw_second.contains("User: first history prompt"));
        assert!(raw_second.contains("Assistant: first history answer"));
        assert!(raw_second.contains("second history prompt"));

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn effective_context_uses_last_repaired_round_without_raw_failed_attempt() {
        let session_id = SessionId::new("runtime-session-repair-context");
        let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
        let failed_round = closed_turn_for_context(
            &mut history,
            &session_id,
            "runtime-turn-7",
            "trace-7",
            "repair this task",
            TerminalStatus::Failed,
            "failed attempt details that should stay out of future prompt",
        );
        let repaired_round = closed_turn_for_context(
            &mut history,
            &session_id,
            "runtime-turn-7-r2",
            "trace-7-r2",
            "repair this task",
            TerminalStatus::Success,
            "repaired success summary",
        );
        let unrelated_turn = closed_turn_for_context(
            &mut history,
            &session_id,
            "runtime-turn-8",
            "trace-8",
            "next independent task",
            TerminalStatus::Success,
            "next task summary",
        );

        let segments =
            effective_turn_context_segments(&[failed_round, repaired_round, unrelated_turn]);
        let rendered = freehand_blocks::render_context_segments_as_text(&segments);

        assert_eq!(segments.len(), 2);
        assert!(rendered.contains("Historical turn 7 (round 2):"));
        assert!(rendered.contains("Assistant: repaired success summary"));
        assert!(rendered.contains("Historical turn 8 (round 1):"));
        assert!(
            !rendered.contains("failed attempt details that should stay out of future prompt"),
            "superseded failed repair attempt leaked into future context: {rendered}"
        );
    }

    fn closed_turn_for_context(
        history: &mut SessionHistory,
        session_id: &SessionId,
        turn_id: &str,
        trace_id: &str,
        prompt: &str,
        status: TerminalStatus,
        summary: &str,
    ) -> TurnRecord {
        let turn_id = TurnId::new(turn_id);
        let trace_id = TraceId::new(trace_id);
        let mut turn = ReasonTurnEngine::new()
            .start_turn(
                history,
                TurnStartInput {
                    session_id: session_id.clone(),
                    turn_id: turn_id.clone(),
                    trace_id: trace_id.clone(),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-live"),
                    user_text: prompt.to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model-a".to_owned(),
                },
            )
            .expect("turn");
        turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: session_id.clone(),
            turn_id,
            trace_id,
            feature_id: FeatureId::new("provider.reason-live-bridge"),
            agent_id: AgentId::new("agent-live"),
            status,
            summary: summary.to_owned(),
        });
        turn
    }

    #[test]
    fn runtime_dispatches_session_crud_into_shared_ui_projection() {
        let runtime_home = temp_runtime_home();
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");
        let session_id = SessionId::new("session-crud-runtime");

        let create = build_command_dispatch_envelope(&UiCommand::CreateSession {
            session_id: session_id.clone(),
            title: Some("Initial".to_owned()),
            cwd: Some("/tmp".to_owned()),
        })
        .expect("create envelope");
        let receipt = runtime.dispatch(create).expect("create dispatch");
        assert_eq!(receipt.target_feature_id, "reason.persistence");
        assert_eq!(receipt.dispatch_status, "session_metadata_updated");

        let rename = build_command_dispatch_envelope(&UiCommand::RenameSession {
            session_id: session_id.clone(),
            title: "Renamed".to_owned(),
        })
        .expect("rename envelope");
        runtime.dispatch(rename).expect("rename dispatch");

        match runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionList)
            .expect("session list")
        {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 1);
                assert_eq!(list.sessions[0].session_id, session_id);
                assert_eq!(list.sessions[0].title.as_deref(), Some("Renamed"));
                assert!(!list.sessions[0].archived);
            }
            other => panic!("unexpected session list: {other:?}"),
        }

        let archive = build_command_dispatch_envelope(&UiCommand::ArchiveSession {
            session_id: session_id.clone(),
        })
        .expect("archive envelope");
        runtime.dispatch(archive).expect("archive dispatch");
        match runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QueryArchivedSessionList)
            .expect("archived list")
        {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 1);
                assert_eq!(list.sessions[0].session_id, session_id);
                assert!(list.sessions[0].archived);
            }
            other => panic!("unexpected archived list: {other:?}"),
        }

        let restore = build_command_dispatch_envelope(&UiCommand::RestoreSession {
            session_id: session_id.clone(),
        })
        .expect("restore envelope");
        runtime.dispatch(restore).expect("restore dispatch");
        match runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionList)
            .expect("active list")
        {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 1);
                assert_eq!(list.sessions[0].session_id, session_id);
                assert!(!list.sessions[0].archived);
            }
            other => panic!("unexpected active list: {other:?}"),
        }

        let missing = build_command_dispatch_envelope(&UiCommand::ArchiveSession {
            session_id: SessionId::new("missing-session"),
        })
        .expect("missing envelope");
        let err = runtime.dispatch(missing).expect_err("missing must fail");
        assert_eq!(
            err,
            UiCommandDispatchPortError::TargetNotFound("missing-session".to_owned())
        );

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_query_projects_config_status_without_secrets() {
        let runtime_home = temp_runtime_home();
        let selected = live_selected_agent(
            "https://user:password@example.invalid:8443/v1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        );
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &selected,
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");

        let result = runtime
            .query_runtime(&UiCommand::QueryConfigStatus)
            .expect("config query")
            .expect("runtime-owned result");

        match result {
            UiQueryResult::ConfigStatus(status) => {
                assert_eq!(status.agent_name, "agent-live");
                assert_eq!(status.agent_mode, "master");
                assert_eq!(status.node_id, "agent-live-node");
                assert_eq!(status.paired_agent_name, "agent-live-worker");
                assert_eq!(status.provider_id, "provider-live");
                assert_eq!(status.provider_type, "anthropic");
                assert_eq!(status.provider_protocol, "messages");
                assert_eq!(status.provider_base_url_host, "example.invalid");
                assert_eq!(status.default_model, "MiniMax-M2.7");
                assert_eq!(status.provider_auth_type, "apikey");
                assert_eq!(status.provider_auth_source, "env");
                assert!(status.restart_required_on_change);
                let encoded = serde_json::to_string(&status).expect("status json");
                assert!(!encoded.contains("test-api-key"));
                assert!(!encoded.contains("password"));
                assert!(!encoded.contains("api_key"));
                assert!(!encoded.contains("pair-token"));
            }
            other => panic!("unexpected query result: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_dispatch_updates_provider_config_without_hot_reloading_active_model() {
        let runtime_home = temp_runtime_home();
        fs::create_dir_all(&runtime_home).expect("create runtime home");
        let config_path = runtime_home.join("config.toml");
        fs::write(
            &config_path,
            r#"
[providers.old]
id = "old"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://old.example.test/v1"
default_model = "old-model"

[providers.old.auth]
type = "apikey"
api_key_env = "FREEHAND_RUNTIME_PROVIDER_OLD"

[agents.agent-live]
name = "agent-live"
mode = "master"
node_id = "agent-live-node"
paired_agent = "agent-live-worker"
pair_token = "FREEHAND_RUNTIME_MASTER_TOKEN"
provider = "old"

[agents.agent-live-worker]
name = "agent-live-worker"
mode = "slave"
node_id = "agent-live-worker-node"
paired_agent = "agent-live"
pair_token = "FREEHAND_RUNTIME_WORKER_TOKEN"
provider = "old"
"#,
        )
        .expect("write config");
        // SAFETY: this test owns these unique variable names and removes them before exit.
        unsafe {
            std::env::set_var("FREEHAND_RUNTIME_PROVIDER_OLD", "old-secret");
            std::env::set_var("FREEHAND_RUNTIME_PROVIDER_NEW", "new-secret");
            std::env::set_var("FREEHAND_RUNTIME_MASTER_TOKEN", "pair-token");
            std::env::set_var("FREEHAND_RUNTIME_WORKER_TOKEN", "pair-token");
        }
        let selected = freehand_config::load_config_from_path(&config_path)
            .expect("load config")
            .select_agent("agent-live")
            .expect("select agent");
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &selected,
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");
        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::UpdateProviderConfig {
                    update: UiProviderConfigUpdate {
                        agent_name: "agent-live".to_owned(),
                        provider_id: "new-provider".to_owned(),
                        provider_type: "anthropic".to_owned(),
                        provider_protocol: "messages".to_owned(),
                        base_url: "https://new.example.test/v1".to_owned(),
                        default_model: "new-model".to_owned(),
                        api_key_env: "FREEHAND_RUNTIME_PROVIDER_NEW".to_owned(),
                    },
                })
                .expect("config update envelope"),
            )
            .expect("config update receipt");
        assert_eq!(
            receipt.dispatch_status,
            "provider_config_saved_restart_required"
        );

        match runtime
            .query_runtime(&UiCommand::QueryConfigStatus)
            .expect("config query")
            .expect("runtime-owned config result")
        {
            UiQueryResult::ConfigStatus(status) => {
                assert_eq!(status.provider_id, "new-provider");
                assert_eq!(status.provider_base_url_host, "new.example.test");
                assert_eq!(status.default_model, "new-model");
                assert_eq!(status.provider_auth_source, "env");
                assert!(status.restart_required_on_change);
                let encoded = serde_json::to_string(&status).expect("status json");
                assert!(!encoded.contains("new-secret"));
                assert!(!encoded.contains("old-secret"));
                assert!(!encoded.contains("api_key"));
            }
            other => panic!("unexpected query result: {other:?}"),
        }

        {
            let state = runtime.state.lock().expect("lock runtime state");
            assert_eq!(state.config.model, "old-model");
            assert_eq!(
                state
                    .config
                    .live
                    .as_ref()
                    .unwrap()
                    .selected_agent
                    .provider
                    .id,
                "old"
            );
            assert_eq!(
                state
                    .config
                    .live
                    .as_ref()
                    .unwrap()
                    .selected_agent
                    .provider
                    .default_model,
                "old-model"
            );
        }

        let raw = fs::read_to_string(&config_path).expect("read saved config");
        assert!(raw.contains("[providers.new-provider]"));
        assert!(raw.contains("default_model = \"new-model\""));
        assert!(raw.contains("api_key_env = \"FREEHAND_RUNTIME_PROVIDER_NEW\""));
        assert!(!raw.contains("new-secret"));
        assert!(!raw.contains("old-secret"));

        // SAFETY: undo the test environment mutation before exit.
        unsafe {
            std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_OLD");
            std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_NEW");
            std::env::remove_var("FREEHAND_RUNTIME_MASTER_TOKEN");
            std::env::remove_var("FREEHAND_RUNTIME_WORKER_TOKEN");
        }
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_dispatch_rejects_invalid_provider_config_without_overwrite() {
        let runtime_home = temp_runtime_home();
        fs::create_dir_all(&runtime_home).expect("create runtime home");
        let config_path = runtime_home.join("config.toml");
        fs::write(
            &config_path,
            r#"
[providers.old]
id = "old"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://old.example.test/v1"
default_model = "old-model"

[providers.old.auth]
type = "apikey"
api_key_env = "FREEHAND_RUNTIME_PROVIDER_OLD_INVALID"

[agents.agent-live]
name = "agent-live"
mode = "master"
node_id = "agent-live-node"
paired_agent = "agent-live-worker"
pair_token = "FREEHAND_RUNTIME_MASTER_TOKEN_INVALID"
provider = "old"

[agents.agent-live-worker]
name = "agent-live-worker"
mode = "slave"
node_id = "agent-live-worker-node"
paired_agent = "agent-live"
pair_token = "FREEHAND_RUNTIME_WORKER_TOKEN_INVALID"
provider = "old"
"#,
        )
        .expect("write config");
        // SAFETY: this test owns these unique variable names and removes them before exit.
        unsafe {
            std::env::set_var("FREEHAND_RUNTIME_PROVIDER_OLD_INVALID", "old-secret");
            std::env::set_var("FREEHAND_RUNTIME_MASTER_TOKEN_INVALID", "pair-token");
            std::env::set_var("FREEHAND_RUNTIME_WORKER_TOKEN_INVALID", "pair-token");
        }
        let selected = freehand_config::load_config_from_path(&config_path)
            .expect("load config")
            .select_agent("agent-live")
            .expect("select agent");
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &selected,
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");
        let before = fs::read_to_string(&config_path).expect("read before");
        let err = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::UpdateProviderConfig {
                    update: UiProviderConfigUpdate {
                        agent_name: "agent-live".to_owned(),
                        provider_id: "bad-provider".to_owned(),
                        provider_type: "anthropic".to_owned(),
                        provider_protocol: "messages".to_owned(),
                        base_url: "not-a-url".to_owned(),
                        default_model: "bad-model".to_owned(),
                        api_key_env: "FREEHAND_RUNTIME_PROVIDER_NEW_INVALID".to_owned(),
                    },
                })
                .expect("config update envelope"),
            )
            .expect_err("invalid update must fail");
        let err_text = err.to_string();
        assert!(
            err_text.contains("bad-provider") && err_text.contains("base_url"),
            "unexpected config update error: {err_text}"
        );
        let after = fs::read_to_string(&config_path).expect("read after");
        assert_eq!(after, before);
        assert!(
            runtime
                .query_runtime(&UiCommand::QueryConfigStatus)
                .expect("config query")
                .is_some()
        );

        // SAFETY: undo the test environment mutation before exit.
        unsafe {
            std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_OLD_INVALID");
            std::env::remove_var("FREEHAND_RUNTIME_MASTER_TOKEN_INVALID");
            std::env::remove_var("FREEHAND_RUNTIME_WORKER_TOKEN_INVALID");
        }
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_dispatch_failure_preserves_other_session_transcripts() {
        let runtime_home = temp_runtime_home();
        let preserved_session = SessionId::new("runtime-session-preserved");
        let (base_url_ok, rx_ok, handle_ok) = spawn_sequence_server(
            "application/json",
            vec![complete_single_response("preserved answer")],
        );
        run_live_reason_turn(
            &live_selected_agent(base_url_ok, freehand_config::ProviderType::Anthropic),
            LiveReasonTurnRequest {
                runtime_home: runtime_home.clone(),
                session_id: preserved_session.clone(),
                turn_id: TurnId::new("runtime-turn-1"),
                trace_id: TraceId::new("runtime-trace-1"),
                prompt: "preserved prompt".to_owned(),
                cwd: None,
                stream: false,
                cancel_token: None,
            },
        )
        .expect("persist preserved session");
        let _ = rx_ok.recv().expect("preserved provider request");
        handle_ok.join().expect("join preserved provider");

        let (base_url_fail, rx_fail, handle_fail) = spawn_status_sequence_server(
            (0..PROVIDER_EXECUTOR_RETRY_CAP)
                .map(|index| {
                    (
                        500,
                        "application/json",
                        format!(
                            r#"{{"type":"error","error":{{"type":"api_error","message":"failure {index}"}}}}"#
                        ),
                    )
                })
                .collect(),
        );
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(base_url_fail, freehand_config::ProviderType::Anthropic),
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");
        let failed_session = SessionId::new("runtime-session-failed");

        let err = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "failed prompt".to_owned(),
                    session_id: Some(failed_session.clone()),
                    cwd: None,
                })
                .expect("failed envelope"),
            )
            .expect_err("provider exhaustion must fail");
        for _ in 0..PROVIDER_EXECUTOR_RETRY_CAP {
            let _ = rx_fail.recv().expect("failed provider request");
        }
        handle_fail.join().expect("join failed provider");
        assert!(
            err.to_string().contains("anthropic_http_status_500"),
            "unexpected dispatch error: {err}"
        );

        match runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: preserved_session.clone(),
            })
            .expect("query preserved transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 1);
                assert_eq!(
                    transcript.turns[0].user_text.as_deref(),
                    Some("preserved prompt")
                );
                assert!(
                    transcript.turns[0]
                        .terminal_text
                        .as_deref()
                        .is_some_and(|text| text.contains("preserved answer"))
                );
            }
            other => panic!("unexpected preserved transcript query: {other:?}"),
        }

        match runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: failed_session.clone(),
            })
            .expect("query failed transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert!(
                    transcript
                        .turns
                        .iter()
                        .any(|turn| turn.terminal_status == Some(TerminalStatus::Failed)),
                    "failed session should keep its own failed turn projection: {:?}",
                    transcript.turns
                );
            }
            other => panic!("unexpected failed transcript query: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_dispatches_session_rollback_into_effective_ui_projection() {
        let runtime_home = temp_runtime_home();
        let session_id = SessionId::new("session-rollback-runtime");
        let persistence = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"));
        let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
        for (turn_id, trace_id, prompt, summary) in [
            ("runtime-turn-1", "trace-1", "first prompt", "first done"),
            ("runtime-turn-2", "trace-2", "second prompt", "second done"),
        ] {
            let mut turn = ReasonTurnEngine::new()
                .start_turn(
                    &mut history,
                    TurnStartInput {
                        session_id: session_id.clone(),
                        turn_id: TurnId::new(turn_id),
                        trace_id: TraceId::new(trace_id),
                        feature_id: FeatureId::new("runtime.ui-command-dispatch"),
                        agent_id: AgentId::new("agent-live"),
                        user_text: prompt.to_owned(),
                        planned_context_segments: Vec::new(),
                        tool_schema_fingerprint: None,
                        model: "model-a".to_owned(),
                    },
                )
                .expect("turn");
            turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
                session_id: session_id.clone(),
                turn_id: TurnId::new(turn_id),
                trace_id: TraceId::new(trace_id),
                feature_id: FeatureId::new("runtime.ui-command-dispatch"),
                agent_id: AgentId::new("agent-live"),
                status: TerminalStatus::Success,
                summary: summary.to_owned(),
            });
            persistence
                .record_turn_closed(&history, &turn, 0)
                .expect("persist turn");
        }

        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");
        let rollback = build_command_dispatch_envelope(&UiCommand::RollbackLatestSessionTurn {
            session_id: session_id.clone(),
        })
        .expect("rollback envelope");
        let receipt = runtime.dispatch(rollback).expect("rollback dispatch");
        assert_eq!(receipt.target_feature_id, "reason.persistence");
        assert!(
            receipt
                .dispatch_status
                .contains("session_turn_rolled_back:runtime-turn-2")
        );

        match runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("session turns")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 1);
                assert_eq!(transcript.turns[0].turn_id, TurnId::new("runtime-turn-1"));
                assert_eq!(
                    transcript.turns[0].user_text.as_deref(),
                    Some("first prompt")
                );
            }
            other => panic!("unexpected session turns: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_bootstrap_restores_multiround_turns_as_separate_ui_cards() {
        let runtime_home = temp_runtime_home();
        fs::create_dir_all(&runtime_home).expect("create runtime home");
        fs::write(runtime_home.join("restore.txt"), "restored tool content\n")
            .expect("write restore fixture");
        with_temp_workspace(|_| {
            let (base_url, rx, handle) = spawn_sequence_server(
                "application/json",
                vec![
                    tool_use_named_response(
                        "toolu_restore_read",
                        "read_file",
                        json!({"path":"restore.txt","offset":0,"limit":2}),
                    ),
                    complete_single_response("final after tool"),
                ],
            );
            run_live_reason_turn(
                &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
                live_request_for(&runtime_home, "runtime-session-tool-restore", 9),
            )
            .expect("persist multi-round session");
            let _ = rx.recv().expect("provider request round 1");
            let _ = rx.recv().expect("provider request round 2");
            handle.join().expect("join provider");

            let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
                &live_selected_agent(
                    "http://127.0.0.1:1".to_owned(),
                    freehand_config::ProviderType::Anthropic,
                ),
                runtime_home.clone(),
                false,
            )
            .expect("runtime bootstrap");

            let transcript = runtime
                .ui_state()
                .lock()
                .expect("lock ui")
                .query(&UiCommand::QuerySessionTurns {
                    session_id: SessionId::new("runtime-session-tool-restore"),
                })
                .expect("session turns query");
            match transcript {
                UiQueryResult::SessionTurns(transcript) => {
                    assert_eq!(transcript.turns.len(), 2);
                    let tool_turn = &transcript.turns[0];
                    assert_eq!(tool_turn.turn_id, TurnId::new("runtime-turn-9"));
                    assert_eq!(
                        tool_turn.user_text.as_deref(),
                        Some("prompt for runtime-session-tool-restore")
                    );
                    assert!(
                        tool_turn.tool_activities.iter().any(|tool| {
                            tool.tool_name == "read_file"
                                && tool.status
                                    == freehand_ui_protocol::UiToolActivityStatus::Completed
                        }),
                        "restored first round must retain its own file-read activity: {:?}",
                        tool_turn.tool_activities
                    );
                    assert!(tool_turn.terminal_text.is_none());

                    let final_turn = &transcript.turns[1];
                    assert_eq!(final_turn.turn_id, TurnId::new("runtime-turn-9-r2"));
                    assert_eq!(
                        final_turn.user_text.as_deref(),
                        Some("prompt for runtime-session-tool-restore")
                    );
                    assert!(
                        final_turn.tool_activities.is_empty(),
                        "final round must not aggregate earlier-round tool activity: {:?}",
                        final_turn.tool_activities
                    );
                    assert!(
                        final_turn
                            .terminal_text
                            .as_deref()
                            .is_some_and(|text| text.contains("final after tool"))
                    );
                }
                other => panic!("unexpected session turns query: {other:?}"),
            }
        });

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn provider_request_built_debug_projects_model_waiting_ui_state() {
        let ui_state = Arc::new(Mutex::new(UiProtocolState::default()));
        let session_id = SessionId::new("session-model-request");
        let turn_id = TurnId::new("runtime-turn-77");
        let trace_id = TraceId::new("trace-model-request");
        let semantic = DebugSemanticPosition {
            feature_id: FeatureId::new("provider.reason-live-bridge"),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            trace_id: trace_id.clone(),
            agent_id: Some(AgentId::new("agent-1")),
            pipeline_node: Some("RuntimeLive02ProviderRequestBuilt".to_owned()),
        };
        let scene = DebugScenePosition {
            crate_name: "freehand-runtime".to_owned(),
            file: "src/lib.rs".to_owned(),
            function: "test".to_owned(),
            line: None,
            artifact_path: None,
            raw_exchange_id: None,
        };
        let event = DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic: semantic.clone(),
                scene: scene.clone(),
                input_hash: None,
                output_hash: None,
                artifact_path: None,
                timestamp: "1".to_owned(),
            },
            snapshot: Some(DebugStateSnapshot::new(
                semantic,
                scene,
                "provider request built",
                vec!["model=MiniMax-M2.7".to_owned()],
            )),
        };

        apply_runtime_debug_event(&ui_state, &AgentId::new("agent-1"), "node-1", &event);
        let query = ui_state
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QueryTurn {
                turn_id: turn_id.clone(),
            })
            .expect("query turn");
        match query {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(turn.session_id, session_id);
                assert_eq!(turn.turn_id, turn_id);
                assert_eq!(
                    turn.model_request
                        .as_ref()
                        .and_then(|activity| activity.detail.as_deref()),
                    Some("provider request built")
                );
                assert_eq!(
                    turn.model_request.as_ref().map(|activity| activity.kind),
                    Some(UiModelRequestKind::Thinking)
                );
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }

    #[test]
    fn live_dispatch_projects_schema_polishing_feedback_to_client_before_mismatch_completes() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                invalid_complete_response(),
                complete_single_response("schema polished"),
            ],
        );
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");

        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "trigger schema polishing".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("submit should complete after schema polishing");
        assert!(
            receipt
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        let _first_request = rx.recv().expect("first provider request");
        let second_request = rx.recv().expect("schema polishing provider request");
        handle.join().expect("join provider");

        assert!(second_request.contains("`completion_reason`: is required"));
        assert!(second_request.contains("`evidence`: is required"));
        assert!(second_request.contains("`learned`: is required"));

        let transcript = runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: SessionId::new("runtime-session-agent-live"),
            })
            .expect("query transcript");
        match transcript {
            UiQueryResult::SessionTurns(transcript) => {
                let retry_round = transcript
                    .turns
                    .iter()
                    .find(|turn| turn.turn_id == TurnId::new("runtime-turn-1"))
                    .expect("schema mismatch round");
                let activity = retry_round
                    .model_request
                    .as_ref()
                    .expect("schema polishing must be client-visible");
                assert_eq!(activity.kind, UiModelRequestKind::SchemaRetry);
                let detail = activity.detail.as_deref().expect("schema detail");
                assert!(detail.contains("schema polishing #1"));
                assert!(detail.contains("completion_reason is required"));
                assert!(detail.contains("evidence is required"));
                assert!(detail.contains("learned is required"));

                let final_round = transcript
                    .turns
                    .iter()
                    .find(|turn| turn.turn_id == TurnId::new("runtime-turn-1-r2"))
                    .expect("polishing final round");
                assert_eq!(final_round.terminal_status, Some(TerminalStatus::Success));
                assert!(final_round.model_request.is_none());
            }
            other => panic!("unexpected transcript query: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_dispatch_projects_missing_schema_polishing_feedback_to_client() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                missing_completion_schema_response(),
                complete_single_response("schema polished"),
            ],
        );
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");

        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "trigger missing schema polishing".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("submit should complete after missing schema polishing");
        assert!(
            receipt
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        let _first_request = rx.recv().expect("first provider request");
        let second_request = rx.recv().expect("schema polishing provider request");
        handle.join().expect("join provider");

        assert!(second_request.contains("`freehand_completion`: missing"));
        assert!(second_request.contains("<freehand_completion>"));

        let transcript = runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: SessionId::new("runtime-session-agent-live"),
            })
            .expect("query transcript");
        match transcript {
            UiQueryResult::SessionTurns(transcript) => {
                let retry_round = transcript
                    .turns
                    .iter()
                    .find(|turn| turn.turn_id == TurnId::new("runtime-turn-1"))
                    .expect("schema mismatch round");
                let activity = retry_round
                    .model_request
                    .as_ref()
                    .expect("schema polishing must be client-visible");
                assert_eq!(activity.kind, UiModelRequestKind::SchemaRetry);
                let detail = activity.detail.as_deref().expect("schema detail");
                assert!(detail.contains("schema polishing #1"));
                assert!(detail.contains("freehand_completion missing"));
                assert!(detail.contains("<freehand_completion>"));
            }
            other => panic!("unexpected transcript query: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_submit_uses_requested_session_id_for_new_webui_session() {
        let runtime_home = temp_runtime_home();
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![complete_single_response("new session answer")],
        );
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");

        let requested_session = SessionId::new("webui-session-test");
        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "hello from new session".to_owned(),
                    session_id: Some(requested_session.clone()),
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("submit receipt");
        assert!(
            receipt
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        let _ = rx.recv().expect("provider request");
        handle.join().expect("join provider");

        let transcript = runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: requested_session.clone(),
            })
            .expect("query transcript");
        match transcript {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.session_id, requested_session);
                assert_eq!(transcript.turns.len(), 1);
                assert_eq!(
                    transcript.turns[0].user_text.as_deref(),
                    Some("hello from new session")
                );
                assert_eq!(
                    transcript.turns[0].terminal_status,
                    Some(TerminalStatus::Success)
                );
            }
            other => panic!("unexpected transcript: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_bridge_retries_recoverable_provider_errors_then_succeeds() {
        let runtime_home = temp_runtime_home();
        let (base_url, rx, handle) = spawn_status_sequence_server(vec![
            (
                500,
                "application/json",
                r#"{"type":"error","error":{"type":"api_error","message":"first upstream failure"}}"#
                    .to_owned(),
            ),
            (
                500,
                "application/json",
                r#"{"type":"error","error":{"type":"api_error","message":"second upstream failure"}}"#
                    .to_owned(),
            ),
            (200, "application/json", complete_single_response("retry ok")),
        ]);

        let outcome = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            LiveReasonTurnRequest {
                runtime_home: runtime_home.clone(),
                ..live_request(false)
            },
            |_| {},
            |_| {},
            |_| {},
        )
        .expect("provider retry should recover");

        assert!(
            outcome
                .turn
                .terminal_event
                .expect("terminal")
                .summary
                .contains("retry ok")
        );
        assert_eq!(rx.iter().take(3).count(), 3);
        handle.join().expect("join provider");
        let metadata =
            metadata_ledger_records(&runtime_home, "agent-live", &SessionId::new("session-live"));
        let retry_actions = metadata
            .iter()
            .filter_map(|row| metadata_entry_string(row, "error.recovery_action"))
            .collect::<Vec<_>>();
        assert!(retry_actions.contains(&"retry_same_step".to_owned()));
        assert!(!retry_actions.contains(&"fail_turn".to_owned()));

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_bridge_fails_after_five_provider_retries_with_error_code() {
        let runtime_home = temp_runtime_home();
        let responses = (0..PROVIDER_EXECUTOR_RETRY_CAP)
            .map(|index| {
                (
                    500,
                    "application/json",
                    format!(
                        r#"{{"type":"error","error":{{"type":"api_error","message":"upstream failure {index}"}}}}"#
                    ),
                )
            })
            .collect::<Vec<_>>();
        let (base_url, rx, handle) = spawn_status_sequence_server(responses);

        let err = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            LiveReasonTurnRequest {
                runtime_home: runtime_home.clone(),
                ..live_request(false)
            },
            |_| {},
            |_| {},
            |_| {},
        )
        .expect_err("provider retry exhaustion should fail");

        assert!(err.to_string().contains("anthropic_http_status_500"));
        assert_eq!(
            rx.iter().take(PROVIDER_EXECUTOR_RETRY_CAP as usize).count(),
            5
        );
        handle.join().expect("join provider");
        let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
            .restore(&SessionId::new("session-live"))
            .expect("restore failed turn");
        assert!(restored.active_turn.is_none());
        let closed = restored.closed_turns.last().expect("closed turn");
        let error = closed.error_events.last().expect("error event");
        assert_eq!(error.error.code, "anthropic_http_status_500");
        assert!(
            closed
                .terminal_event
                .as_ref()
                .expect("terminal")
                .summary
                .contains("anthropic_http_status_500")
        );
        let metadata =
            metadata_ledger_records(&runtime_home, "agent-live", &SessionId::new("session-live"));
        let retry_indexes = metadata
            .iter()
            .filter_map(|row| metadata_entry_u64(row, "error.retry_index"))
            .collect::<Vec<_>>();
        assert!(retry_indexes.contains(&1));
        assert!(retry_indexes.contains(&5));
        let recovery_actions = metadata
            .iter()
            .filter_map(|row| metadata_entry_string(row, "error.recovery_action"))
            .collect::<Vec<_>>();
        assert!(recovery_actions.contains(&"retry_same_step".to_owned()));
        assert!(recovery_actions.contains(&"fail_turn".to_owned()));

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn submit_cwd_is_projected_and_inherited_by_session() {
        let root = temp_runtime_home();
        fs::create_dir_all(&root).expect("create cwd");
        let runtime = runtime();
        let session_id = SessionId::new("webui-session-cwd-runtime");
        let cwd = fs::canonicalize(&root)
            .expect("canonical cwd")
            .to_string_lossy()
            .into_owned();

        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "first cwd turn".to_owned(),
                    session_id: Some(session_id.clone()),
                    cwd: Some(cwd.clone()),
                })
                .expect("first envelope"),
            )
            .expect("first receipt");
        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "second cwd turn".to_owned(),
                    session_id: Some(session_id.clone()),
                    cwd: None,
                })
                .expect("second envelope"),
            )
            .expect("second receipt");

        match runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("query transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.cwd.as_deref(), Some(cwd.as_str()));
                assert_eq!(transcript.turns.len(), 2);
                assert!(
                    transcript
                        .turns
                        .iter()
                        .all(|turn| turn.cwd.as_deref() == Some(cwd.as_str()))
                );
            }
            other => panic!("unexpected transcript: {other:?}"),
        }

        fs::remove_dir_all(root).expect("cleanup cwd");
    }

    #[test]
    fn live_master_tool_execution_rejects_external_session_cwd() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let workspace = temp_runtime_home();
        fs::create_dir_all(&workspace).expect("create workspace");
        fs::write(workspace.join("session-cwd.txt"), "session cwd content\n")
            .expect("write workspace file");
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_named_response(
                    "toolu_session_cwd",
                    "read_file",
                    json!({"path":"session-cwd.txt","offset":0,"limit":5}),
                ),
                complete_single_response("read session cwd"),
            ],
        );
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");
        let session_id = SessionId::new("webui-session-tool-cwd");
        let cwd = fs::canonicalize(&workspace)
            .expect("canonical workspace")
            .to_string_lossy()
            .into_owned();

        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "read the session cwd file".to_owned(),
                    session_id: Some(session_id.clone()),
                    cwd: Some(cwd.clone()),
                })
                .expect("envelope"),
            )
            .expect("submit receipt");
        let _first_request = rx.recv().expect("first provider request");
        let reentry_request = rx.recv().expect("tool reentry provider request");
        handle.join().expect("join provider");

        assert!(
            receipt
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        assert!(reentry_request.contains("Master workspace boundary"));
        assert!(reentry_request.contains("scope/permission boundary"));
        assert!(reentry_request.contains("task(op=\\\"create\\\""));
        assert!(!reentry_request.contains("session cwd content"));
        match runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("query transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.cwd.as_deref(), Some(cwd.as_str()));
                assert_eq!(transcript.turns[0].cwd.as_deref(), Some(cwd.as_str()));
            }
            other => panic!("unexpected transcript: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
        fs::remove_dir_all(workspace).expect("cleanup workspace");
    }

    fn selected_master_agent() -> SelectedAgentConfig {
        SelectedAgentConfig {
            name: "master".to_owned(),
            mode: AgentMode::Master,
            node_id: "master-node".to_owned(),
            paired_agent_name: "worker".to_owned(),
            paired_agent_mode: AgentMode::Slave,
            paired_node_id: "worker-node".to_owned(),
            paired_allowed_pair_ip: Some("127.0.0.1".parse().expect("ip")),
            paired_pair_token_env: "FREEHAND_PAIR_TOKEN_WORKER".to_owned(),
            allowed_pair_ip: None,
            pair_token_env: "FREEHAND_PAIR_TOKEN_MASTER".to_owned(),
            pair_token: "pair-token".to_owned(),
            provider: freehand_config::SelectedProviderConfig {
                id: "provider-master".to_owned(),
                provider_type: freehand_config::ProviderType::Anthropic,
                protocol: freehand_config::ProviderProtocol::Messages,
                base_url: "https://example.invalid".to_owned(),
                default_model: "model-master".to_owned(),
                auth_type: freehand_config::ProviderAuthType::ApiKey,
                auth_source: freehand_config::ProviderAuthSourceKind::Inline,
                api_key: "secret".to_owned(),
            },
            restart_required_on_change: true,
        }
    }

    fn live_selected_agent(
        base_url: String,
        provider_type: freehand_config::ProviderType,
    ) -> SelectedAgentConfig {
        let protocol = match provider_type {
            freehand_config::ProviderType::Anthropic => ConfigProviderProtocol::Messages,
            freehand_config::ProviderType::OpenAi => ConfigProviderProtocol::ChatCompletions,
        };
        SelectedAgentConfig {
            name: "agent-live".to_owned(),
            mode: AgentMode::Master,
            node_id: "agent-live-node".to_owned(),
            paired_agent_name: "agent-live-worker".to_owned(),
            paired_agent_mode: AgentMode::Slave,
            paired_node_id: "agent-live-worker-node".to_owned(),
            paired_allowed_pair_ip: None,
            paired_pair_token_env: "FREEHAND_WORKER_TOKEN".to_owned(),
            allowed_pair_ip: None,
            pair_token_env: "FREEHAND_MASTER_TOKEN".to_owned(),
            pair_token: "pair-token".to_owned(),
            provider: freehand_config::SelectedProviderConfig {
                id: "provider-live".to_owned(),
                provider_type,
                protocol,
                base_url,
                default_model: "MiniMax-M2.7".to_owned(),
                auth_type: freehand_config::ProviderAuthType::ApiKey,
                auth_source: freehand_config::ProviderAuthSourceKind::Env,
                api_key: "test-api-key".to_owned(),
            },
            restart_required_on_change: true,
        }
    }

    fn temp_runtime_home() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("freehand-runtime-live-{stamp}-{counter}"))
    }

    fn live_request(stream: bool) -> LiveReasonTurnRequest {
        LiveReasonTurnRequest {
            runtime_home: temp_runtime_home(),
            session_id: SessionId::new("session-live"),
            turn_id: TurnId::new("turn-live"),
            trace_id: TraceId::new("trace-live"),
            prompt: "reply exactly pong".to_owned(),
            cwd: None,
            stream,
            cancel_token: None,
        }
    }

    fn live_request_for(
        runtime_home: &Path,
        session_id: &str,
        ordinal: u64,
    ) -> LiveReasonTurnRequest {
        LiveReasonTurnRequest {
            runtime_home: runtime_home.to_path_buf(),
            session_id: SessionId::new(session_id),
            turn_id: TurnId::new(format!("runtime-turn-{ordinal}")),
            trace_id: TraceId::new(format!("runtime-trace-{ordinal}")),
            prompt: format!("prompt for {session_id}"),
            cwd: None,
            stream: false,
            cancel_token: None,
        }
    }

    fn lifecycle_live_request(runtime_home: &Path, event_id: &str) -> LiveReasonTurnRequest {
        LiveReasonTurnRequest {
            runtime_home: runtime_home.to_path_buf(),
            session_id: SessionId::new(format!("master-lifecycle-{event_id}")),
            turn_id: TurnId::new(format!("master-lifecycle-{event_id}-decision")),
            trace_id: TraceId::new(format!("master-lifecycle-trace-{event_id}")),
            prompt: format!("make one Task Center decision for {event_id}"),
            cwd: Some(runtime_home.to_path_buf()),
            stream: false,
            cancel_token: None,
        }
    }

    fn create_lifecycle_test_worker(runtime: &TaskRuntime) {
        runtime
            .create_agent(AgentCreateRequest {
                agent_id: AgentId::new("worker"),
                capabilities: vec!["workspace".to_owned()],
                actor: lifecycle_test_actor(),
                watermark: lifecycle_test_watermark("create-worker"),
            })
            .expect("create worker");
    }

    fn create_lifecycle_test_task(runtime: &TaskRuntime, task_id: &str) -> TaskSnapshot {
        runtime
            .create_task(TaskCreateRequest {
                task_id: Some(TaskId::new(task_id)),
                title: format!("{task_id} title"),
                content: "lifecycle decision fixture".to_owned(),
                goal: "persist one target task decision".to_owned(),
                deliverables: vec!["decision evidence".to_owned()],
                acceptance: vec!["target task changes".to_owned()],
                priority: 90,
                target_cwd: Some(std::env::temp_dir().display().to_string()),
                dispatch: TaskDispatchRequest::None,
                parent: TaskParentRef {
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                actor: lifecycle_test_actor(),
                watermark: lifecycle_test_watermark("create-task"),
            })
            .expect("create lifecycle task")
            .task
    }

    fn lifecycle_test_actor() -> TaskActor {
        TaskActor {
            agent_id: AgentId::new("agent-live"),
            source: "runtime.master-worker-loop.test".to_owned(),
            session_id: None,
            turn_id: None,
            trace_id: None,
        }
    }

    fn lifecycle_test_watermark(hook: &str) -> TaskWatermark {
        TaskWatermark {
            metadata_id: None,
            hook: Some(hook.to_owned()),
            action_tool_call_id: None,
        }
    }

    fn with_temp_workspace<F>(test: F)
    where
        F: FnOnce(&Path),
    {
        with_locked_cwd(|| {
            let original = std::env::current_dir().expect("current dir");
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "freehand-runtime-tools-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root).expect("create temp workspace");
            std::env::set_current_dir(&root).expect("set cwd");
            let restore = RestoreCwd { original };
            test(&root);
            drop(restore);
            fs::remove_dir_all(&root).expect("cleanup temp workspace");
        });
    }

    fn with_locked_cwd<F, R>(test: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        test()
    }

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn fnv1a_hex_for_test(input: &str) -> String {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in input.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{hash:016x}")
    }

    struct RestoreCwd {
        original: PathBuf,
    }

    impl Drop for RestoreCwd {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    fn checkpoint_ledger_rows(
        runtime_home: &Path,
        agent_id: &str,
        session_id: &SessionId,
    ) -> Vec<RuntimeCheckpointLedgerRow> {
        let path = runtime_home
            .join("ledgers")
            .join("checkpoints")
            .join(agent_id)
            .join(format!("{}.jsonl", session_id.as_str()));
        let raw = fs::read_to_string(path).expect("read checkpoint ledger");
        raw.lines()
            .map(|line| serde_json::from_str(line).expect("decode ledger row"))
            .collect()
    }

    fn metadata_ledger_records(
        runtime_home: &Path,
        agent_id: &str,
        session_id: &SessionId,
    ) -> Vec<MetadataEnvelope> {
        let path = runtime_home
            .join("ledgers")
            .join("metadata")
            .join(agent_id)
            .join(format!("{}.jsonl", session_id.as_str()));
        let raw = fs::read_to_string(path).expect("read metadata ledger");
        raw.lines()
            .map(|line| serde_json::from_str(line).expect("decode metadata ledger row"))
            .collect()
    }

    fn provider_raw_ledger_rows(
        runtime_home: &Path,
        provider_family: &str,
        agent_id: &str,
        session_id: &SessionId,
        turn_id: &str,
    ) -> Vec<ProviderRawLedgerRow> {
        let path = runtime_home
            .join("ledgers")
            .join("providers")
            .join(provider_family)
            .join(agent_id)
            .join(session_id.as_str())
            .join(format!("{turn_id}.jsonl"));
        let raw = fs::read_to_string(path).expect("read provider raw ledger");
        raw.lines()
            .map(|line| serde_json::from_str(line).expect("decode provider raw ledger row"))
            .collect()
    }

    fn runtime_debug_events<'a>(
        events: &'a [DebugEvent],
        pipeline_node: &str,
    ) -> Vec<&'a DebugEvent> {
        events
            .iter()
            .filter(|event| {
                event
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.scene.crate_name == "freehand-runtime")
                    && event.envelope.semantic.feature_id.as_str() == "provider.reason-live-bridge"
                    && event.envelope.semantic.pipeline_node.as_deref() == Some(pipeline_node)
            })
            .collect()
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
        spawn_status_sequence_server(
            response_bodies
                .into_iter()
                .map(|body| (200, content_type, body))
                .collect(),
        )
    }

    fn spawn_status_sequence_server(
        responses: Vec<(u16, &'static str, String)>,
    ) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            for (status, content_type, response_body) in responses {
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
                    "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
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
        let tagged = tagged_completion_json(&format!(
            r#"{{"claim":"complete","completion_reason":"done","evidence":"provider returned {visible_text}","summary":"{visible_text}","learned":"keep tagged completion strict"}}"#
        ));
        format!(
            r#"{{"content":[{{"type":"text","text":"{visible}\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":82}},"stop_reason":"end_turn"}}"#,
            visible = visible_text,
            tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
        )
    }

    fn status_stop_single_response(visible_text: &str) -> String {
        let status = r#"<<<freehand_status>>>
{"schema_version":1,"status":{"simple_question":true}}
<</freehand_status>>>"#;
        json!({
            "content": [{
                "type": "text",
                "text": format!("{visible_text}\n{status}")
            }],
            "usage": {"input_tokens": 14, "output_tokens": 40},
            "stop_reason": "end_turn"
        })
        .to_string()
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

    fn continue_with_visible_response(visible_text: &str, next_step: &str) -> String {
        let tagged = tagged_completion_json(&format!(
            r#"{{"claim":"continue","next_step":"{next_step}"}}"#
        ));
        json!({
            "content": [{
                "type": "text",
                "text": format!("{visible_text}\n{tagged}")
            }],
            "usage": {"input_tokens": 14, "output_tokens": 40},
            "stop_reason": "end_turn"
        })
        .to_string()
    }

    fn invalid_complete_response() -> String {
        let tagged = tagged_completion_json(r#"{"claim":"complete","summary":"pong"}"#);
        format!(
            r#"{{"content":[{{"type":"text","text":"draft\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":40}},"stop_reason":"end_turn"}}"#,
            tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
        )
    }

    fn missing_completion_schema_response() -> String {
        json!({
            "content": [{
                "type": "text",
                "text": "draft without the required Freehand completion block"
            }],
            "usage": {"input_tokens": 14, "output_tokens": 40},
            "stop_reason": "end_turn"
        })
        .to_string()
    }

    fn max_tokens_text_response() -> String {
        json!({
            "content": [{
                "type": "text",
                "text": "partial response without a completion schema"
            }],
            "usage": {"input_tokens": 14, "output_tokens": 512},
            "stop_reason": "max_tokens"
        })
        .to_string()
    }

    fn task_tool_call(arguments: Vec<(&str, Value)>) -> ReasonReq04ToolCall {
        ReasonReq04ToolCall {
            session_id: SessionId::new("session-task"),
            turn_id: TurnId::new("turn-task"),
            trace_id: TraceId::new("trace-task"),
            feature_id: FeatureId::new("provider.reason-live-bridge"),
            agent_id: AgentId::new("agent-task"),
            tool_call: ToolCallContract {
                tool_call_id: ToolCallId::new("toolu_task_1"),
                tool_name: "task".to_owned(),
                arguments: arguments
                    .into_iter()
                    .map(|(name, value)| ToolArgument {
                        name: name.to_owned(),
                        value,
                    })
                    .collect(),
                arguments_complete: true,
            },
        }
    }

    fn tool_use_named_response(tool_call_id: &str, tool_name: &str, input: Value) -> String {
        json!({
            "content": [{
                "type": "tool_use",
                "id": tool_call_id,
                "name": tool_name,
                "input": input
            }],
            "usage": {"input_tokens": 20, "output_tokens": 16},
            "stop_reason": "tool_use"
        })
        .to_string()
    }

    fn task_tool_use_response(tool_call_id: &str, input: Value) -> String {
        tool_use_named_response(tool_call_id, "task", input)
    }

    fn master_autonomy_prompt(sentinel: &str) -> String {
        format!(
            "{}\n{sentinel}",
            (0..80)
                .map(|index| format!(
                    "step-{index}: master must create a worker task, dispatch it, inspect worker result, handle success, execution error, and incomplete review retry without losing this instruction."
                ))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    fn collect_provider_requests(rx: &mpsc::Receiver<String>, expected: usize) -> Vec<String> {
        (0..expected)
            .map(|index| {
                rx.recv()
                    .unwrap_or_else(|err| panic!("provider request {index}: {err}"))
            })
            .collect()
    }

    fn assert_master_task_request_contract(
        raw_request: &str,
        sentinel: &str,
        configured_worker: &str,
    ) {
        assert!(raw_request.contains(sentinel));
        assert!(raw_request.contains("Master task orchestration policy"));
        assert!(raw_request.contains("you are the master agent"));
        assert!(raw_request.contains("Dispatch when"));
        assert!(raw_request.contains("Multi-agent dispatch"));
        assert!(raw_request.contains("Concurrency control"));
        assert!(raw_request.contains("Flow control"));
        assert!(raw_request.contains("Task tool workflow"));
        assert!(raw_request.contains(&format!(
            "Configured paired Worker id: `{configured_worker}`"
        )));
        assert!(raw_request.contains("Historical agents returned by list_agents"));
        assert!(raw_request.contains("never put task(...)"));
        assert!(raw_request.contains("The Worker does not receive the task tool"));
        assert!(raw_request.contains(
            "converts the Worker completion schema into TaskReviewSubmitted or TaskBlocked"
        ));
        assert!(
            raw_request.contains("do not directly execute work outside your allowed workspace")
        );
        assert!(raw_request.contains("assign only useful independent subtasks"));
        assert!(raw_request.contains("task(op=\\\"list_agents\\\")"));
        assert!(raw_request.contains("Master task orchestration examples"));
        assert!(raw_request.contains("Cross-workspace sample"));
        assert!(raw_request.contains("~/code/codex"));
        assert!(raw_request.contains("~/code/Deepseek-reasonix"));
        assert!(raw_request.contains("target_cwd"));
        assert!(raw_request.contains("Worker success sample"));
        assert!(raw_request.contains("Worker execution error sample"));
        assert!(raw_request.contains("Worker retry sample"));
        assert!(raw_request.contains("\"name\":\"task\""));
        assert!(raw_request.contains("\"record_execution\""));
        assert!(raw_request.contains("\"retry_count\""));
        assert!(raw_request.contains("create_agent"));
        assert!(raw_request.contains("review_ready"));
    }

    fn task_truth(runtime_home: &Path, task_id: &str) -> (TaskSnapshot, Vec<String>) {
        let task_runtime =
            TaskRuntime::boot(runtime_home, AgentId::new("agent-live")).expect("task runtime");
        let task = task_runtime
            .query_task(&TaskId::new(task_id))
            .expect("query task truth");
        let event_types = task_runtime
            .task_history(&TaskId::new(task_id))
            .expect("task history")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();
        (task, event_types)
    }

    fn event_index(events: &[String], event_type: &str) -> usize {
        events
            .iter()
            .position(|event| event == event_type)
            .unwrap_or_else(|| panic!("missing event {event_type}: {events:?}"))
    }

    fn tool_use_single_response() -> String {
        tool_use_named_response(
            "toolu_read_1",
            "read_file",
            json!({"path":"Cargo.toml","offset":0,"limit":2}),
        )
    }

    fn incomplete_tool_use_stream_response() -> String {
        concat!(
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_incomplete_1\",\"name\":\"read_file\",\"input\":{}}}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"input_tokens\":20,\"output_tokens\":8}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        )
        .to_owned()
    }

    fn tool_use_missing_read_response() -> String {
        tool_use_named_response(
            "toolu_missing_read_1",
            "read_file",
            json!({"path":"definitely-missing-freehand-file.txt","offset":0,"limit":2}),
        )
    }

    fn tool_use_unknown_response() -> String {
        tool_use_named_response(
            "toolu_unknown_1",
            "totally_unknown_tool",
            json!({"path":"Cargo.toml"}),
        )
    }

    fn tool_use_write_file_response(path: &str, content: &str) -> String {
        tool_use_named_response(
            "toolu_write_1",
            "write_file",
            json!({
                "path": path,
                "content": content
            }),
        )
    }

    fn tool_use_edit_file_response(path: &str, old_string: &str, new_string: &str) -> String {
        tool_use_named_response(
            "toolu_edit_1",
            "edit_file",
            json!({
                "path": path,
                "old_string": old_string,
                "new_string": new_string
            }),
        )
    }

    fn tool_use_bash_response(command: &str) -> String {
        tool_use_named_response(
            "toolu_bash_1",
            "bash",
            json!({
                "command": command
            }),
        )
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
    fn live_bridge_runs_single_shot_anthropic_provider_into_turn_truth() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) =
            spawn_mock_server(200, "application/json", complete_single_response("pong"));
        let request = live_request(false);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();
        let mut debug_events = Vec::<DebugEvent>::new();

        let outcome = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
            |_| {},
            |event| debug_events.push(event.clone()),
            |_| {},
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

        let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "provider.reason-live-bridge"
                && record.write_node.pipeline_node == "RuntimeLive01RestoreResolved"
        }));
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "provider.reason-live-bridge"
                && record.write_node.pipeline_node == "RuntimeLive02ProviderRequestBuilt"
        }));
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "provider.reason-live-bridge"
                && record.write_node.pipeline_node == "RuntimeLive04TurnClosed"
        }));
        assert!(metadata.iter().all(
            |record| serde_json::to_string(record).expect("encode metadata")
                != outcome.turn.request.user_text
        ));
        assert!(metadata.iter().all(|record| {
            let encoded = serde_json::to_string(record).expect("encode metadata");
            !encoded.contains("reply exactly pong")
        }));
        let provider_raw = provider_raw_ledger_rows(
            &runtime_home,
            "anthropic",
            "agent-live",
            &session_id,
            "turn-live",
        );
        assert_eq!(provider_raw.len(), 1);
        assert_eq!(provider_raw[0].raw_kind, "response_body");
        assert!(
            provider_raw[0]
                .body
                .contains("\"stop_reason\":\"end_turn\"")
        );
        assert_eq!(
            runtime_debug_events(&debug_events, "RuntimeLive01RestoreResolved").len(),
            1
        );
        assert_eq!(
            runtime_debug_events(&debug_events, "RuntimeLive02ProviderRequestBuilt").len(),
            1
        );
        assert_eq!(
            runtime_debug_events(&debug_events, "RuntimeLive04TurnClosed").len(),
            1
        );
        let expected_tool_count = BuiltinToolRegistry::reasonix_aligned()
            .master_implemented_definitions()
            .len();
        assert!(
            runtime_debug_events(&debug_events, "RuntimeLive02ProviderRequestBuilt")
                .into_iter()
                .flat_map(|event| {
                    event
                        .snapshot
                        .as_ref()
                        .expect("runtime snapshot")
                        .detail_lines
                        .iter()
                })
                .any(|line| line == &format!("tool_definition_count={expected_tool_count}"))
        );
        assert!(debug_events.iter().all(|event| {
            let encoded = serde_json::to_string(event).expect("encode debug event");
            !encoded.contains("reply exactly pong")
        }));
    }

    #[test]
    fn worker_live_bridge_exposes_shell_excludes_task_and_locks_task_workspace() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_mock_server(
            200,
            "application/json",
            complete_single_response("worker complete"),
        );
        let runtime_home = temp_runtime_home();
        let workspace = temp_runtime_home();
        fs::create_dir_all(&workspace).expect("create worker workspace");
        let canonical_workspace = fs::canonicalize(&workspace).expect("canonical workspace");
        let mut request = live_request(false);
        request.runtime_home = runtime_home.clone();
        request.cwd = Some(canonical_workspace.clone());
        let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        selected.name = "worker-live".to_owned();
        selected.mode = AgentMode::Slave;
        selected.paired_agent_name = "master-live".to_owned();
        selected.paired_agent_mode = AgentMode::Master;

        let outcome = run_worker_live_reason_turn(&selected, request).expect("worker live bridge");
        let raw_request = rx.recv().expect("provider request");
        handle.join().expect("join provider");

        assert!(raw_request.contains("\"name\":\"bash\""));
        assert!(raw_request.contains("\"name\":\"read_file\""));
        assert!(!raw_request.contains("\"name\":\"task\""));
        assert!(raw_request.contains("Worker execution policy"));
        assert_eq!(
            outcome.turn.cwd.as_deref(),
            Some(canonical_workspace.to_string_lossy().as_ref())
        );
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
        fs::remove_dir_all(workspace).expect("cleanup workspace");
    }

    #[test]
    fn worker_live_bridge_rejects_master_mode_and_missing_workspace() {
        let selected = selected_master_agent();
        let mut request = live_request(false);
        request.cwd = Some(temp_runtime_home());
        assert!(matches!(
            run_worker_live_reason_turn(&selected, request),
            Err(RuntimeLiveBridgeError::AgentModeMismatch { .. })
        ));

        let mut worker = selected;
        worker.mode = AgentMode::Slave;
        worker.paired_agent_mode = AgentMode::Master;
        assert_eq!(
            run_worker_live_reason_turn(&worker, live_request(false)),
            Err(RuntimeLiveBridgeError::WorkerWorkspaceRequired)
        );
    }

    #[test]
    fn live_bridge_accepts_simple_status_stop_hook_without_completion_schema() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) =
            spawn_mock_server(200, "application/json", status_stop_single_response("pong"));
        let request = live_request(false);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
            |_| {},
            |_| {},
            |_| {},
        )
        .expect("status stop hook");
        let _raw_request = rx.recv().expect("request");
        handle.join().expect("join");

        assert_eq!(outcome.rounds, 1);
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );
        assert!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .is_some_and(|event| event.summary.contains("Summary: pong"))
        );

        let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "control.center"
                && record.write_node.pipeline_node == "ControlHook03AfterModelResponse"
                && record.entries.iter().any(|entry| {
                    entry.key == "control.decision" && entry.value == json!("allow_natural_stop")
                })
        }));
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "control.center"
                && record.write_node.pipeline_node == "ControlHook04BeforeClientReturn"
                && record.entries.iter().any(|entry| {
                    entry.key == "control.public_projection_stripped" && entry.value == json!(true)
                })
        }));
        assert!(metadata.iter().all(|record| {
            let encoded = serde_json::to_string(record).expect("metadata json");
            !encoded.contains("<<<freehand_status>>>") && !encoded.contains("pong")
        }));
    }

    #[test]
    fn live_bridge_polishes_invalid_control_status_without_provider_failure() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let invalid_status = json!({
            "content": [{
                "type": "text",
                "text": "working\n<<<freehand_status>>>\n{\"schema_version\":1,\"status\":{\"simple_question\":true,\"next_step\":42}}\n<</freehand_status>>>"
            }],
            "usage": {"input_tokens": 14, "output_tokens": 40},
            "stop_reason": "end_turn"
        })
        .to_string();
        let corrected_status = json!({
            "content": [{
                "type": "text",
                "text": "pong\n<<<freehand_status>>>\n{\"schema_version\":1,\"status\":{\"simple_question\":true,\"next_step\":null,\"blocked_reason\":null}}\n<</freehand_status>>>"
            }],
            "usage": {"input_tokens": 14, "output_tokens": 40},
            "stop_reason": "end_turn"
        })
        .to_string();
        let (base_url, rx, handle) =
            spawn_sequence_server("application/json", vec![invalid_status, corrected_status]);
        let request = live_request(false);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();
        let broadcasts = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&broadcasts);

        let outcome = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
            move |event| {
                captured.lock().expect("broadcast lock").push(event.clone());
            },
            |_| {},
            |_| {},
        )
        .expect("control status should polish and continue");
        let first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        handle.join().expect("join");

        assert!(!first_request.contains("status schema was rejected"));
        assert!(second_request.contains("status schema was rejected"));
        assert!(second_request.contains("next_step"));
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
        assert!(
            broadcasts
                .lock()
                .expect("broadcast lock")
                .iter()
                .any(|event| matches!(
                    event,
                    ReasonBroadcastEvent::CompletionSchemaRejected(rejection)
                        if rejection.feedback.contains("next_step")
                ))
        );
        let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "control.center"
                && record.write_node.pipeline_node == "ControlHook03AfterModelResponse"
                && record.entries.iter().any(|entry| {
                    entry.key == "control.status_validation" && entry.value == json!("rejected")
                })
        }));
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "error.center"
                && record.entries.iter().any(|entry| {
                    entry.key == "error.code"
                        && entry.value == json!("control_status_schema_rejected")
                })
                && record.entries.iter().any(|entry| {
                    entry.key == "error.recovery_action" && entry.value == json!("repair_schema")
                })
        }));
    }

    #[test]
    fn live_bridge_blocks_after_three_consecutive_invalid_control_statuses() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let invalid_status = || {
            json!({
                "content": [{
                    "type": "text",
                    "text": "working\n<<<freehand_status>>>\n{\"schema_version\":1,\"status\":{\"simple_question\":true,\"next_step\":42}}\n<</freehand_status>>>"
                }],
                "usage": {"input_tokens": 14, "output_tokens": 40},
                "stop_reason": "end_turn"
            })
            .to_string()
        };
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![invalid_status(), invalid_status(), invalid_status()],
        );

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(false),
        )
        .expect("schema mismatch exhaustion is blocked truth");
        for _ in 0..3 {
            rx.recv().expect("provider request");
        }
        handle.join().expect("join");

        assert_eq!(outcome.rounds, 3);
        assert_eq!(outcome.schema_rejections.len(), 3);
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Blocked)
        );
        assert!(outcome.turn.terminal_event.as_ref().is_some_and(|event| {
            event.summary.contains("3 polishing attempts") && event.summary.contains("next_step")
        }));
        assert_eq!(
            outcome
                .broadcasts
                .iter()
                .filter(|event| matches!(event, ReasonBroadcastEvent::CompletionSchemaRejected(_)))
                .count(),
            2
        );
    }

    #[test]
    fn task_tool_create_persists_and_queries_task() {
        let runtime_home = temp_runtime_home();
        let engine = ReasonTurnEngine::new();
        let mut history =
            SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-task"),
                    turn_id: TurnId::new("turn-task"),
                    trace_id: TraceId::new("trace-task"),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-task"),
                    user_text: "create a task".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model".to_owned(),
                },
            )
            .expect("turn");
        let create_call = task_tool_call(vec![
            ("op", json!("create")),
            ("task_id", json!("task-runtime-test")),
            ("title", json!("Task persistence")),
            ("content", json!("Persist and recover task")),
            ("goal", json!("Task query survives runtime reboot")),
            ("deliverables", json!(["ledger", "snapshot"])),
            ("acceptance", json!(["query returns assigned task"])),
            ("dispatch", json!({"mode":"self"})),
        ]);

        let create_output =
            execute_task_tool(&runtime_home, &turn, &create_call).expect("create task");

        assert!(create_output.contains("task_id=task-runtime-test"));
        assert!(create_output.contains("status=Assigned"));

        let query_call = task_tool_call(vec![
            ("op", json!("query")),
            ("task_id", json!("task-runtime-test")),
        ]);
        let query_output =
            execute_task_tool(&runtime_home, &turn, &query_call).expect("query task");

        assert!(query_output.contains("\"task_id\":\"task-runtime-test\""));
        assert!(query_output.contains("\"status\":\"assigned\""));

        let agents_call = task_tool_call(vec![("op", json!("list_agents"))]);
        let agents_output =
            execute_task_tool(&runtime_home, &turn, &agents_call).expect("list agents");

        assert!(agents_output.contains("\"agent_id\":\"agent-task\""));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_tool_review_lifecycle_rejects_early_close_and_closes_after_approval() {
        let runtime_home = temp_runtime_home();
        let engine = ReasonTurnEngine::new();
        let mut history =
            SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-task"),
                    turn_id: TurnId::new("turn-task"),
                    trace_id: TraceId::new("trace-task"),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-task"),
                    user_text: "create a task".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model".to_owned(),
                },
            )
            .expect("turn");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("create")),
                ("task_id", json!("task-runtime-review")),
                ("title", json!("Review lifecycle")),
                ("content", json!("Exercise review lifecycle")),
                ("goal", json!("Close only after approval")),
                ("deliverables", json!(["code"])),
                ("acceptance", json!(["approval required"])),
                ("dispatch", json!({"mode":"self"})),
            ]),
        )
        .expect("create task");

        let early_close = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("close")),
                ("task_id", json!("task-runtime-review")),
            ]),
        )
        .expect_err("early close must fail");
        assert!(early_close.contains("invalid task transition"));

        for call in [
            task_tool_call(vec![
                ("op", json!("resume")),
                ("task_id", json!("task-runtime-review")),
            ]),
            task_tool_call(vec![
                ("op", json!("submit_review")),
                ("task_id", json!("task-runtime-review")),
                ("summary", json!("ready")),
                ("deliverables", json!(["code"])),
                ("evidence", json!(["tests passed"])),
            ]),
            task_tool_call(vec![
                ("op", json!("approve")),
                ("task_id", json!("task-runtime-review")),
            ]),
        ] {
            execute_task_tool(&runtime_home, &turn, &call).expect("lifecycle op");
        }
        let close = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("close")),
                ("task_id", json!("task-runtime-review")),
            ]),
        )
        .expect("close after approval");

        assert!(close.contains("status=Closed"));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_tool_resume_and_heartbeat_persist_running_lease() {
        let runtime_home = temp_runtime_home();
        let engine = ReasonTurnEngine::new();
        let mut history =
            SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-task"),
                    turn_id: TurnId::new("turn-task"),
                    trace_id: TraceId::new("trace-task"),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-task"),
                    user_text: "run a task".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model".to_owned(),
                },
            )
            .expect("turn");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("create")),
                ("task_id", json!("task-runtime-heartbeat")),
                ("title", json!("Heartbeat lifecycle")),
                ("content", json!("Exercise task heartbeat")),
                ("goal", json!("Running task keeps a lease")),
                ("deliverables", json!(["lease"])),
                ("acceptance", json!(["heartbeat accepted"])),
                ("dispatch", json!({"mode":"self"})),
            ]),
        )
        .expect("create task");

        let resume = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("resume")),
                ("task_id", json!("task-runtime-heartbeat")),
            ]),
        )
        .expect("resume");
        assert!(resume.contains("status=Running"));

        let heartbeat = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("heartbeat")),
                ("task_id", json!("task-runtime-heartbeat")),
                ("ttl_seconds", json!(600)),
            ]),
        )
        .expect("heartbeat");
        assert!(heartbeat.contains("event=TaskHeartbeat"));

        let query = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("query")),
                ("task_id", json!("task-runtime-heartbeat")),
            ]),
        )
        .expect("query");
        assert!(query.contains("\"status\":\"running\""));
        assert!(
            runtime_home
                .join("state/task-runtime/agent-task/leases.json")
                .is_file()
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_tool_agent_assign_cancel_close_lifecycle() {
        let runtime_home = temp_runtime_home();
        let engine = ReasonTurnEngine::new();
        let mut history =
            SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-task"),
                    turn_id: TurnId::new("turn-task"),
                    trace_id: TraceId::new("trace-task"),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-task"),
                    user_text: "assign a task".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model".to_owned(),
                },
            )
            .expect("turn");
        let create_agent = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("create_agent")),
                ("agent_id", json!("worker-runtime")),
                ("capabilities", json!(["code_edit"])),
            ]),
        )
        .expect("create agent");
        assert!(create_agent.contains("status=Available"));

        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("create")),
                ("task_id", json!("task-runtime-assign")),
                ("title", json!("Assign lifecycle")),
                ("content", json!("Exercise assign and cancel")),
                ("goal", json!("Assigned task can be cancelled")),
                ("deliverables", json!(["task"])),
                ("acceptance", json!(["agent released"])),
                ("dispatch", json!({"mode":"none"})),
            ]),
        )
        .expect("create waiting task");

        let assigned = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("assign")),
                ("task_id", json!("task-runtime-assign")),
                ("agent_id", json!("worker-runtime")),
            ]),
        )
        .expect("assign");
        assert!(assigned.contains("status=Assigned"));

        let busy_close = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("close_agent")),
                ("agent_id", json!("worker-runtime")),
            ]),
        )
        .expect_err("busy worker cannot close");
        assert!(busy_close.contains("invalid agent transition"));

        let cancelled = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("cancel")),
                ("task_id", json!("task-runtime-assign")),
            ]),
        )
        .expect("cancel");
        assert!(cancelled.contains("status=Cancelled"));

        let closed = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("close_agent")),
                ("agent_id", json!("worker-runtime")),
            ]),
        )
        .expect("close idle worker");
        assert!(closed.contains("status=Closed"));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_tool_claim_next_runs_highest_priority_task() {
        let runtime_home = temp_runtime_home();
        let engine = ReasonTurnEngine::new();
        let mut history =
            SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-task"),
                    turn_id: TurnId::new("turn-task"),
                    trace_id: TraceId::new("trace-task"),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-task"),
                    user_text: "claim highest priority task".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model".to_owned(),
                },
            )
            .expect("turn");
        for (task_id, priority) in [("task-low", 10), ("task-high", 90)] {
            execute_task_tool(
                &runtime_home,
                &turn,
                &task_tool_call(vec![
                    ("op", json!("create")),
                    ("task_id", json!(task_id)),
                    ("title", json!(format!("Claim {task_id}"))),
                    ("content", json!("Exercise priority claim")),
                    ("goal", json!("Claim highest priority task")),
                    ("deliverables", json!(["task"])),
                    ("acceptance", json!(["highest priority claimed"])),
                    ("priority", json!(priority)),
                    ("dispatch", json!({"mode":"none"})),
                ]),
            )
            .expect("create task");
            execute_task_tool(
                &runtime_home,
                &turn,
                &task_tool_call(vec![
                    ("op", json!("assign")),
                    ("task_id", json!(task_id)),
                    ("agent_id", json!("agent-task")),
                ]),
            )
            .expect("assign task");
        }

        let claimed = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("claim_next")),
                ("agent_id", json!("agent-task")),
                ("execution_id", json!("exec-task-high")),
                ("ttl_seconds", json!(600)),
            ]),
        )
        .expect("claim next");
        assert!(claimed.contains("task_id=task-high"));
        assert!(claimed.contains("status=Running"));
        assert!(claimed.contains("execution_id=exec-task-high"));

        let low = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![("op", json!("query")), ("task_id", json!("task-low"))]),
        )
        .expect("query low");
        assert!(low.contains("\"status\":\"assigned\""));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_tool_record_execution_requires_running_task() {
        let runtime_home = temp_runtime_home();
        let engine = ReasonTurnEngine::new();
        let mut history =
            SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-task"),
                    turn_id: TurnId::new("turn-task"),
                    trace_id: TraceId::new("trace-task"),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-task"),
                    user_text: "record worker progress".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model".to_owned(),
                },
            )
            .expect("turn");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("create")),
                ("task_id", json!("task-runtime-execution")),
                ("title", json!("Execution progress")),
                ("content", json!("Record execution progress")),
                ("goal", json!("Progress enters task ledger")),
                ("deliverables", json!(["event"])),
                ("acceptance", json!(["running only"])),
                ("dispatch", json!({"mode":"self"})),
            ]),
        )
        .expect("create task");
        let rejected = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("record_execution")),
                ("task_id", json!("task-runtime-execution")),
                ("phase", json!("debug")),
                ("summary", json!("should fail before running")),
                ("evidence", json!(["assigned only"])),
            ]),
        )
        .expect_err("assigned task cannot record execution");
        assert!(rejected.contains("invalid task transition"));

        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("resume")),
                ("task_id", json!("task-runtime-execution")),
            ]),
        )
        .expect("resume task");
        let recorded = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("record_execution")),
                ("task_id", json!("task-runtime-execution")),
                ("phase", json!("debug")),
                ("summary", json!("read function map")),
                (
                    "evidence",
                    json!(["docs/function-maps/task.orchestration.md"]),
                ),
            ]),
        )
        .expect("record execution");
        assert!(recorded.contains("status=Running"));
        assert!(recorded.contains("event=TaskExecutionRecorded"));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_tool_structured_execution_status_requires_execution_identity() {
        let runtime_home = temp_runtime_home();
        let engine = ReasonTurnEngine::new();
        let mut history =
            SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-task"),
                    turn_id: TurnId::new("turn-task"),
                    trace_id: TraceId::new("trace-task"),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-task"),
                    user_text: "record structured worker state".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model".to_owned(),
                },
            )
            .expect("turn");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("create")),
                ("task_id", json!("task-runtime-structured-status")),
                ("title", json!("Structured execution status")),
                ("content", json!("Reject missing execution identity")),
                ("goal", json!("No implicit execution id fallback")),
                ("deliverables", json!(["explicit error"])),
                ("acceptance", json!(["task remains running"])),
                ("dispatch", json!({"mode":"self"})),
            ]),
        )
        .expect("create task");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("resume")),
                ("task_id", json!("task-runtime-structured-status")),
            ]),
        )
        .expect("resume task");

        let err = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("record_execution")),
                ("task_id", json!("task-runtime-structured-status")),
                ("agent_id", json!("agent-task")),
                ("status", json!("blocked")),
                ("phase", json!("execution_error")),
                (
                    "summary",
                    json!("worker failed but execution id is missing"),
                ),
                ("evidence", json!(["missing execution id"])),
            ]),
        )
        .expect_err("structured execution status requires execution id");
        assert!(err.contains("`execution_id` is required"));

        let query = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("query")),
                ("task_id", json!("task-runtime-structured-status")),
            ]),
        )
        .expect("query");
        assert!(query.contains("\"status\":\"running\""));
        let history_output = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("history")),
                ("task_id", json!("task-runtime-structured-status")),
            ]),
        )
        .expect("history");
        assert!(!history_output.contains("TaskBlocked"));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_tool_history_returns_ordered_execution_timeline() {
        let runtime_home = temp_runtime_home();
        let engine = ReasonTurnEngine::new();
        let mut history =
            SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-task"),
                    turn_id: TurnId::new("turn-task"),
                    trace_id: TraceId::new("trace-task"),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-task"),
                    user_text: "query task timeline".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model".to_owned(),
                },
            )
            .expect("turn");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("create")),
                ("task_id", json!("task-runtime-history")),
                ("title", json!("History")),
                ("content", json!("Query task history")),
                ("goal", json!("Timeline is queryable")),
                ("deliverables", json!(["history"])),
                ("acceptance", json!(["ordered events"])),
                ("dispatch", json!({"mode":"self"})),
            ]),
        )
        .expect("create task");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("resume")),
                ("task_id", json!("task-runtime-history")),
            ]),
        )
        .expect("resume task");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("record_execution")),
                ("task_id", json!("task-runtime-history")),
                ("phase", json!("debug")),
                ("summary", json!("inspect timeline")),
                ("evidence", json!(["ledger query"])),
            ]),
        )
        .expect("record execution");

        let timeline = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("history")),
                ("task_id", json!("task-runtime-history")),
            ]),
        )
        .expect("history");

        assert!(timeline.contains("\"event_type\":\"TaskCreated\""));
        assert!(timeline.contains("\"event_type\":\"TaskExecutionRecorded\""));
        assert!(timeline.contains("\"seq\":1"));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn task_tool_list_tasks_filters_queue_projection() {
        let runtime_home = temp_runtime_home();
        let engine = ReasonTurnEngine::new();
        let mut history =
            SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
        let turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: SessionId::new("session-task"),
                    turn_id: TurnId::new("turn-task"),
                    trace_id: TraceId::new("trace-task"),
                    feature_id: FeatureId::new("provider.reason-live-bridge"),
                    agent_id: AgentId::new("agent-task"),
                    user_text: "list assigned tasks".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model".to_owned(),
                },
            )
            .expect("turn");
        for (task_id, priority) in [("task-list-low", 10), ("task-list-high", 90)] {
            execute_task_tool(
                &runtime_home,
                &turn,
                &task_tool_call(vec![
                    ("op", json!("create")),
                    ("task_id", json!(task_id)),
                    ("title", json!(format!("List {task_id}"))),
                    ("content", json!("List task queue")),
                    ("goal", json!("Filter by assigned state")),
                    ("deliverables", json!(["list"])),
                    ("acceptance", json!(["filtered"])),
                    ("priority", json!(priority)),
                    ("dispatch", json!({"mode":"none"})),
                ]),
            )
            .expect("create task");
            execute_task_tool(
                &runtime_home,
                &turn,
                &task_tool_call(vec![
                    ("op", json!("assign")),
                    ("task_id", json!(task_id)),
                    ("agent_id", json!("agent-task")),
                ]),
            )
            .expect("assign task");
        }

        let tasks = execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("list_tasks")),
                ("status", json!("assigned")),
                ("agent_id", json!("agent-task")),
            ]),
        )
        .expect("list tasks");

        let high_pos = tasks.find("\"task_id\":\"task-list-high\"").expect("high");
        let low_pos = tasks.find("\"task_id\":\"task-list-low\"").expect("low");
        assert!(high_pos < low_pos);
        assert!(tasks.contains("\"status\":\"assigned\""));
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn live_bridge_stamps_tool_schema_fingerprint_into_planner_diagnostics() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, _rx, handle) =
            spawn_mock_server(200, "application/json", complete_single_response("pong"));

        let request = live_request(false);

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("live bridge");
        handle.join().expect("join");

        let registry = BuiltinToolRegistry::reasonix_aligned();
        let expected = fnv1a_hex_for_test(&registry.master_implemented_schema_fingerprint());
        let empty = fnv1a_hex_for_test("");

        assert_eq!(
            outcome.turn.planned_context.diagnostics.tool_schema_hash,
            expected
        );
        assert_ne!(
            outcome.turn.planned_context.diagnostics.tool_schema_hash,
            empty
        );
    }

    #[test]
    fn live_bridge_admits_long_operator_task_without_semantic_truncation() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_mock_server(
            200,
            "application/json",
            complete_single_response("accepted"),
        );
        let mut request = live_request(false);
        request.prompt = format!(
            "{}\nSENTINEL_MASTER_AUTONOMY_LONG_PROMPT_END",
            (0..80)
                .map(|index| format!(
                    "step-{index}: master must create a worker task, dispatch it, inspect worker status, handle rejection, retry, approve, and close without losing this instruction."
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("long operator task must reach provider request");
        let raw_request = rx.recv().expect("request");
        handle.join().expect("join");

        assert!(raw_request.contains("step-79"));
        assert!(raw_request.contains("SENTINEL_MASTER_AUTONOMY_LONG_PROMPT_END"));
        assert_master_task_request_contract(
            &raw_request,
            "SENTINEL_MASTER_AUTONOMY_LONG_PROMPT_END",
            "agent-live-worker",
        );
        let original_task = outcome
            .turn
            .planned_context
            .ordered_segments
            .iter()
            .find(|segment| segment.segment_id.as_str() == "original-task")
            .expect("original task segment");
        assert_eq!(original_task.kind, ContextSegmentKind::TaskContract);
        let original_task_cost = outcome
            .turn
            .planned_context
            .diagnostics
            .segment_token_costs
            .iter()
            .find(|cost| cost.segment_id.as_str() == "original-task")
            .expect("original task token cost");
        assert!(original_task.token_budget >= original_task_cost.estimated_tokens);
        assert!(original_task.token_budget > 128);
    }

    #[test]
    fn live_bridge_admits_long_previous_visible_output_without_fixed_cap() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let visible_text = format!(
            "{}\nSENTINEL_PREVIOUS_VISIBLE_OUTPUT_LONG_END",
            (0..180)
                .map(|index| format!(
                    "round-one-visible-{index}: keep this model-visible repair context for the next round without a short fixed cap."
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                continue_with_visible_response(&visible_text, "finish after carrying prior output"),
                complete_single_response("final after long visible output"),
            ],
        );
        let request = live_request(false);

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("long prior visible output must reach next provider request");
        let _first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        handle.join().expect("join");

        assert_eq!(outcome.rounds, 2);
        assert!(second_request.contains("SENTINEL_PREVIOUS_VISIBLE_OUTPUT_LONG_END"));
        assert!(second_request.contains("<<<freehand_status>>>"));
        assert!(second_request.contains("Master task orchestration examples"));
        let previous_output = outcome
            .turn
            .planned_context
            .ordered_segments
            .iter()
            .find(|segment| segment.segment_id.as_str() == "previous-visible-output")
            .expect("previous visible output segment");
        let previous_output_cost = outcome
            .turn
            .planned_context
            .diagnostics
            .segment_token_costs
            .iter()
            .find(|cost| cost.segment_id.as_str() == "previous-visible-output")
            .expect("previous visible output token cost");
        assert!(previous_output.token_budget >= previous_output_cost.estimated_tokens);
        assert!(previous_output.token_budget > 512);
    }

    #[test]
    fn live_bridge_master_autonomy_success_dispatches_worker_and_closes_task() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let sentinel = "SENTINEL_MASTER_AUTONOMY_SUCCESS_END";
        let task_id = "task-master-autonomy-success";
        let worker_id = "worker-master-autonomy-success";
        let execution_id = "exec-master-autonomy-success";
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                task_tool_use_response(
                    "toolu_success_agent",
                    json!({
                        "op":"create_agent",
                        "agent_id":worker_id,
                        "capabilities":["code_edit","test_run"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_success_create",
                    json!({
                        "op":"create",
                        "task_id":task_id,
                        "title":"Autonomy success task",
                        "content":"Worker should complete the delegated task successfully.",
                        "goal":"Prove master can dispatch and close a successful worker task.",
                        "deliverables":["success report"],
                        "acceptance":["task closes after approval"],
                        "dispatch":{"mode":"none"},
                        "priority":90
                    }),
                ),
                task_tool_use_response(
                    "toolu_success_assign",
                    json!({
                        "op":"assign",
                        "task_id":task_id,
                        "agent_id":worker_id
                    }),
                ),
                task_tool_use_response(
                    "toolu_success_claim",
                    json!({
                        "op":"claim_next",
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "ttl_seconds":600
                    }),
                ),
                task_tool_use_response(
                    "toolu_success_running",
                    json!({
                        "op":"record_execution",
                        "status":"running",
                        "task_id":task_id,
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "phase":"implementation",
                        "summary":"worker implemented the requested change",
                        "evidence":["changed files inspected"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_success_review_ready",
                    json!({
                        "op":"record_execution",
                        "status":"review_ready",
                        "task_id":task_id,
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "phase":"review",
                        "summary":"worker completed all acceptance checks",
                        "deliverables":["success report"],
                        "evidence":["unit test passed","owner truth updated"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_success_approve",
                    json!({
                        "op":"approve",
                        "task_id":task_id
                    }),
                ),
                task_tool_use_response(
                    "toolu_success_close",
                    json!({
                        "op":"close",
                        "task_id":task_id
                    }),
                ),
                complete_single_response("master closed successful worker task"),
            ],
        );
        let mut request = live_request(false);
        request.runtime_home = runtime_home.clone();
        request.prompt = master_autonomy_prompt(sentinel);

        let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        selected.paired_agent_name = worker_id.to_owned();
        let outcome =
            run_live_reason_turn(&selected, request).expect("master autonomy success path");
        let requests = collect_provider_requests(&rx, 9);
        handle.join().expect("join provider");

        assert_master_task_request_contract(&requests[0], sentinel, worker_id);
        assert!(requests[1].contains("Agent created"));
        assert!(requests.iter().any(|request| {
            request.contains("\"tool_use_id\":\"toolu_success_review_ready\"")
                && request.contains("Task review submitted")
        }));
        assert!(requests.iter().any(|request| {
            request.contains("\"tool_use_id\":\"toolu_success_close\"")
                && request.contains("Task closed")
        }));
        assert_eq!(outcome.tool_executions, 8);
        assert_eq!(outcome.rounds, 9);
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );

        let (task, event_types) = task_truth(&runtime_home, task_id);
        assert_eq!(task.status, TaskStatus::Closed);
        for required in [
            "TaskCreated",
            "TaskAssigned",
            "TaskResumed",
            "TaskExecutionRecorded",
            "TaskReviewSubmitted",
            "TaskReviewApproved",
            "TaskClosed",
        ] {
            assert!(
                event_types.iter().any(|event| event == required),
                "missing {required}: {event_types:?}"
            );
        }
        assert!(
            !event_types
                .iter()
                .any(|event| event == "TaskReviewRejected")
        );
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_bridge_master_autonomy_execution_error_blocks_without_success_close() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let sentinel = "SENTINEL_MASTER_AUTONOMY_EXECUTION_ERROR_END";
        let task_id = "task-master-autonomy-error";
        let worker_id = "worker-master-autonomy-error";
        let execution_id = "exec-master-autonomy-error";
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                task_tool_use_response(
                    "toolu_error_agent",
                    json!({
                        "op":"create_agent",
                        "agent_id":worker_id,
                        "capabilities":["code_edit"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_error_create",
                    json!({
                        "op":"create",
                        "task_id":task_id,
                        "title":"Autonomy execution error task",
                        "content":"Worker should report an execution error.",
                        "goal":"Prove master keeps errored worker task blocked instead of closing it.",
                        "deliverables":["error report"],
                        "acceptance":["blocked state is visible"],
                        "dispatch":{"mode":"none"},
                        "priority":80
                    }),
                ),
                task_tool_use_response(
                    "toolu_error_assign",
                    json!({
                        "op":"assign",
                        "task_id":task_id,
                        "agent_id":worker_id
                    }),
                ),
                task_tool_use_response(
                    "toolu_error_claim",
                    json!({
                        "op":"claim_next",
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "ttl_seconds":600
                    }),
                ),
                task_tool_use_response(
                    "toolu_error_running",
                    json!({
                        "op":"record_execution",
                        "status":"running",
                        "task_id":task_id,
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "phase":"implementation",
                        "summary":"worker started execution",
                        "evidence":["worker heartbeat observed"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_error_blocked",
                    json!({
                        "op":"record_execution",
                        "status":"blocked",
                        "task_id":task_id,
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "phase":"execution_error",
                        "summary":"worker hit provider_error_500 and cannot continue without master decision",
                        "evidence":["provider_error_500","no deliverable produced"]
                    }),
                ),
                complete_single_response("master left errored worker task blocked"),
            ],
        );
        let mut request = live_request(false);
        request.runtime_home = runtime_home.clone();
        request.prompt = master_autonomy_prompt(sentinel);

        let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        selected.paired_agent_name = worker_id.to_owned();
        let outcome =
            run_live_reason_turn(&selected, request).expect("master autonomy execution error path");
        let requests = collect_provider_requests(&rx, 7);
        handle.join().expect("join provider");

        assert_master_task_request_contract(&requests[0], sentinel, worker_id);
        assert!(requests.iter().any(|request| {
            request.contains("\"tool_use_id\":\"toolu_error_blocked\"")
                && request.contains("TaskBlocked")
                && request.contains("status=Blocked")
        }));
        assert_eq!(outcome.tool_executions, 6);
        assert_eq!(outcome.rounds, 7);

        let (task, event_types) = task_truth(&runtime_home, task_id);
        assert_eq!(task.status, TaskStatus::Blocked);
        assert!(event_types.iter().any(|event| event == "TaskBlocked"));
        assert!(
            !event_types
                .iter()
                .any(|event| event == "TaskReviewSubmitted")
        );
        assert!(
            !event_types
                .iter()
                .any(|event| event == "TaskReviewApproved")
        );
        assert!(!event_types.iter().any(|event| event == "TaskClosed"));
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_bridge_master_autonomy_rejected_review_retries_and_closes() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let sentinel = "SENTINEL_MASTER_AUTONOMY_REJECT_RETRY_END";
        let task_id = "task-master-autonomy-retry";
        let worker_id = "worker-master-autonomy-retry";
        let execution_id = "exec-master-autonomy-retry";
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                task_tool_use_response(
                    "toolu_retry_agent",
                    json!({
                        "op":"create_agent",
                        "agent_id":worker_id,
                        "capabilities":["code_edit","test_run"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_retry_create",
                    json!({
                        "op":"create",
                        "task_id":task_id,
                        "title":"Autonomy retry task",
                        "content":"Worker first submits incomplete work, then fixes it.",
                        "goal":"Prove master rejects incomplete worker submission and closes only after retry.",
                        "deliverables":["complete report"],
                        "acceptance":["review rejection precedes retry close"],
                        "dispatch":{"mode":"none"},
                        "priority":85
                    }),
                ),
                task_tool_use_response(
                    "toolu_retry_assign",
                    json!({
                        "op":"assign",
                        "task_id":task_id,
                        "agent_id":worker_id
                    }),
                ),
                task_tool_use_response(
                    "toolu_retry_claim",
                    json!({
                        "op":"claim_next",
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "ttl_seconds":600
                    }),
                ),
                task_tool_use_response(
                    "toolu_retry_incomplete_review",
                    json!({
                        "op":"record_execution",
                        "status":"review_ready",
                        "task_id":task_id,
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "phase":"review",
                        "summary":"worker submitted partial implementation without regression proof",
                        "deliverables":["partial report"],
                        "evidence":["no regression evidence"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_retry_reject",
                    json!({
                        "op":"reject",
                        "task_id":task_id,
                        "reject_reason":"missing regression proof",
                        "next_requirements":["run regression evidence","resubmit complete deliverable"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_retry_recovering",
                    json!({
                        "op":"record_execution",
                        "status":"recovering",
                        "task_id":task_id,
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "phase":"retry",
                        "summary":"worker is fixing rejected submission",
                        "evidence":["rejection reason acknowledged"],
                        "retry_count":1
                    }),
                ),
                task_tool_use_response(
                    "toolu_retry_complete_review",
                    json!({
                        "op":"record_execution",
                        "status":"review_ready",
                        "task_id":task_id,
                        "agent_id":worker_id,
                        "execution_id":execution_id,
                        "phase":"review",
                        "summary":"worker resubmitted complete implementation with regression proof",
                        "deliverables":["complete report"],
                        "evidence":["regression passed","missing proof supplied"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_retry_approve",
                    json!({
                        "op":"approve",
                        "task_id":task_id
                    }),
                ),
                task_tool_use_response(
                    "toolu_retry_close",
                    json!({
                        "op":"close",
                        "task_id":task_id
                    }),
                ),
                complete_single_response("master closed retried worker task"),
            ],
        );
        let mut request = live_request(false);
        request.runtime_home = runtime_home.clone();
        request.prompt = master_autonomy_prompt(sentinel);

        let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        selected.paired_agent_name = worker_id.to_owned();
        let outcome = run_live_reason_turn(&selected, request)
            .expect("master autonomy rejected-review retry path");
        let requests = collect_provider_requests(&rx, 11);
        handle.join().expect("join provider");

        assert_master_task_request_contract(&requests[0], sentinel, worker_id);
        assert!(requests.iter().any(|request| {
            request.contains("\"tool_use_id\":\"toolu_retry_reject\"")
                && request.contains("Task rejected")
        }));
        assert!(requests.iter().any(|request| {
            request.contains("\"tool_use_id\":\"toolu_retry_recovering\"")
                && request.contains("TaskExecutionRecovering")
        }));
        assert!(requests.iter().any(|request| {
            request.contains("\"tool_use_id\":\"toolu_retry_close\"")
                && request.contains("Task closed")
        }));
        assert_eq!(outcome.tool_executions, 10);
        assert_eq!(outcome.rounds, 11);

        let (task, event_types) = task_truth(&runtime_home, task_id);
        assert_eq!(task.status, TaskStatus::Closed);
        let first_review = event_index(&event_types, "TaskReviewSubmitted");
        let rejected = event_index(&event_types, "TaskReviewRejected");
        let recovering = event_index(&event_types, "TaskExecutionRecovering");
        let second_review = event_types
            .iter()
            .enumerate()
            .skip(rejected.saturating_add(1))
            .find(|(_, event)| event.as_str() == "TaskReviewSubmitted")
            .map(|(index, _)| index)
            .expect("second review submission after rejection");
        let approved = event_index(&event_types, "TaskReviewApproved");
        let closed = event_index(&event_types, "TaskClosed");
        assert!(first_review < rejected);
        assert!(rejected < recovering);
        assert!(recovering < second_review);
        assert!(second_review < approved);
        assert!(approved < closed);
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_bridge_runs_streaming_anthropic_provider_into_broadcasts() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) =
            spawn_mock_server(200, "text/event-stream", complete_stream_response("pong"));
        let request = live_request(true);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
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
        let provider_raw = provider_raw_ledger_rows(
            &runtime_home,
            "anthropic",
            "agent-live",
            &session_id,
            "turn-live",
        );
        assert!(!provider_raw.is_empty());
        assert!(
            provider_raw
                .iter()
                .all(|row| row.raw_kind == "stream_event_body")
        );
        assert!(
            provider_raw
                .iter()
                .any(|row| row.body.contains("\"type\":\"message_stop\""))
        );
        assert!(outcome.broadcasts.iter().any(
            |event| matches!(event, ReasonBroadcastEvent::Semantic(event) if event.kind == SemanticEventKind::Reasoning)
        ));
    }

    #[test]
    fn live_bridge_applies_stream_outputs_before_provider_finishes() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        let outcome = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
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
            |_| {},
            |_| {},
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
    fn live_bridge_cancel_token_stops_before_tool_execution() {
        let cancel_token = Arc::new(AtomicBool::new(true));
        let mut request = live_request(false);
        request.cancel_token = Some(cancel_token);

        let err = run_live_reason_turn(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            request,
        )
        .expect_err("cancelled live bridge");

        assert_eq!(err, RuntimeLiveBridgeError::Cancelled);
    }

    #[test]
    fn live_bridge_cancel_token_stops_after_provider_output_before_tool_execution() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let cancel_token = Arc::new(AtomicBool::new(false));
        let mut request = live_request(false);
        request.cancel_token = Some(Arc::clone(&cancel_token));
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();
        let (base_url, _rx, handle) =
            spawn_mock_server(200, "application/json", tool_use_single_response());

        let err = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
            |event| {
                if matches!(event, ReasonBroadcastEvent::Tool(_)) {
                    cancel_token.store(true, Ordering::SeqCst);
                }
            },
            |_| {},
            |_| {},
        )
        .expect_err("cancelled before tool execution");
        handle.join().expect("join");

        assert_eq!(err, RuntimeLiveBridgeError::Cancelled);

        let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
            .restore(&session_id)
            .expect("restore live session");
        assert!(
            restored
                .closed_turns
                .iter()
                .all(|turn| turn.terminal_event.is_none()),
            "tool-call cancellation should not materialize terminal truth"
        );
        let latest = restored
            .active_turn
            .as_ref()
            .expect("active turn should remain");
        assert!(latest.turn.tool_results.is_empty());
        assert!(latest.turn.terminal_event.is_none());
    }

    #[test]
    fn live_bridge_cancel_token_stops_before_terminal_persistence() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let cancel_token = Arc::new(AtomicBool::new(false));
        let mut request = live_request(false);
        request.cancel_token = Some(Arc::clone(&cancel_token));
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();
        let (base_url, _rx, handle) =
            spawn_mock_server(200, "application/json", complete_single_response("pong"));

        let err = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
            |event| {
                if matches!(event, ReasonBroadcastEvent::Terminal(_)) {
                    cancel_token.store(true, Ordering::SeqCst);
                }
            },
            |_| {},
            |_| {},
        )
        .expect_err("cancelled before terminal persistence");
        handle.join().expect("join");

        assert_eq!(err, RuntimeLiveBridgeError::Cancelled);

        let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
            .restore(&session_id)
            .expect("restore live session");
        assert!(
            restored.closed_turns.is_empty(),
            "terminal cancellation should not materialize closed-turn truth"
        );
        let latest = restored
            .active_turn
            .as_ref()
            .expect("active turn should remain");
        assert!(
            latest.turn.terminal_event.is_none(),
            "terminal cancellation should not persist terminal truth into the active snapshot"
        );
    }

    #[test]
    fn live_bridge_records_error_center_metadata_for_schema_repair() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                invalid_complete_response(),
                complete_single_response("pong"),
            ],
        );

        let request = live_request(false);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("live bridge");
        let first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        handle.join().expect("join");

        assert!(first_request.contains("reply exactly pong"));
        assert!(second_request.contains("Fix these schema entries"));
        assert!(second_request.contains("`completion_reason`: is required"));
        assert!(second_request.contains("`evidence`: is required"));
        assert!(second_request.contains("`learned`: is required"));
        assert!(second_request.contains("Use plain string values for required text fields"));
        assert_eq!(outcome.rounds, 2);
        assert_eq!(outcome.schema_rejections.len(), 1);
        assert!(outcome.broadcasts.iter().any(|event| {
            matches!(
                event,
                ReasonBroadcastEvent::CompletionSchemaRejected(rejection)
                    if rejection.feedback.contains("`evidence`: is required")
                        && rejection.feedback.contains("`completion_reason`: is required")
            )
        }));
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );
        let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "error.center"
                && record.write_node.pipeline_node == "ReasonResp04CompletionSchemaRejected"
                && record.entries.iter().any(|entry| {
                    entry.key == "error.recovery_action" && entry.value == json!("repair_schema")
                })
                && record
                    .entries
                    .iter()
                    .any(|entry| entry.key == "error.domain" && entry.value == json!("schema"))
        }));
        assert!(!metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "error.center"
                && record
                    .entries
                    .iter()
                    .any(|entry| entry.key == "error.domain" && entry.value == json!("provider"))
        }));
        assert!(!metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "error.center"
                && record.entries.iter().any(|entry| {
                    entry.key == "error.recovery_action" && entry.value == json!("fail_turn")
                })
        }));
    }

    #[test]
    fn live_bridge_retries_missing_completion_schema_then_completes() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                missing_completion_schema_response(),
                complete_single_response("pong"),
            ],
        );

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(false),
        )
        .expect("live bridge");
        let _first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        handle.join().expect("join");

        assert!(second_request.contains("Fix these schema entries"));
        assert!(second_request.contains("`freehand_completion`: missing"));
        assert!(second_request.contains("<freehand_completion>"));
        assert_eq!(outcome.rounds, 2);
        assert_eq!(outcome.schema_rejections.len(), 1);
        assert!(outcome.broadcasts.iter().any(|event| {
            matches!(
                event,
                ReasonBroadcastEvent::CompletionSchemaRejected(rejection)
                    if rejection.feedback.contains("`freehand_completion`: missing")
            )
        }));
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
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                continue_single_response("open the file and confirm pong"),
                complete_single_response("pong"),
            ],
        );

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
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
    fn live_bridge_executes_real_registry_tool_reenters_result_and_persists_terminal_turn() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        let mut debug_events = Vec::<DebugEvent>::new();

        let outcome = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
            |_| {},
            |event| debug_events.push(event.clone()),
            |_| {},
        )
        .expect("live bridge");
        let first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        handle.join().expect("join");

        assert!(first_request.contains("\"tools\""));
        assert!(first_request.contains("\"name\":\"read_file\""));
        assert!(!first_request.contains("\"tool_choice\""));
        assert!(second_request.contains("\"type\":\"tool_result\""));
        assert!(second_request.contains("toolu_read_1"));
        assert!(second_request.contains("Cargo.toml"));
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

        let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "provider.reason-live-bridge"
                && record.write_node.pipeline_node == "RuntimeLive03ToolExecuted"
                && record
                    .entries
                    .iter()
                    .any(|entry| entry.key == "tool.name" && entry.value == json!("read_file"))
        }));
        let tool_debug = runtime_debug_events(&debug_events, "RuntimeLive03ToolExecuted");
        assert_eq!(tool_debug.len(), 1);
        let tool_snapshot = tool_debug[0].snapshot.as_ref().expect("tool snapshot");
        assert!(
            tool_snapshot
                .detail_lines
                .iter()
                .any(|line| line == "tool_name=read_file")
        );
        assert!(
            tool_snapshot
                .detail_lines
                .iter()
                .any(|line| line == "tool_call_id=toolu_read_1")
        );
    }

    #[test]
    fn live_bridge_returns_incomplete_tool_use_as_failed_tool_result_without_schema_retry() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_sequence_server(
            "text/event-stream",
            vec![
                incomplete_tool_use_stream_response(),
                complete_stream_response("tool recovered"),
            ],
        );

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(true),
        )
        .expect("live bridge");
        let first_request = rx.recv().expect("first request");
        let second_request = rx.recv().expect("second request");
        handle.join().expect("join");

        assert!(first_request.contains("\"stream\":true"));
        assert!(
            second_request.contains("\"type\":\"tool_result\""),
            "incomplete tool_use must be paired back to the model"
        );
        assert!(second_request.contains("toolu_incomplete_1"));
        assert!(second_request.contains("is_error"));
        assert_eq!(outcome.rounds, 2);
        assert_eq!(outcome.schema_rejections.len(), 0);
        assert_eq!(outcome.tool_executions, 1);
        assert!(outcome.turns.iter().any(|turn| {
            turn.tool_results.iter().any(|result| {
                result.tool_result.status == ToolResultStatus::Failed
                    && result
                        .tool_result
                        .output
                        .contains("cannot execute incomplete tool arguments")
            })
        }));
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
    fn live_bridge_returns_tool_execution_failure_to_model_for_next_round() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_missing_read_response(),
                complete_single_response("recovered after tool failure"),
            ],
        );
        let request = live_request(false);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("tool execution failure should be model-visible result");
        let _first_request = rx.recv().expect("first provider request");
        let second_request = rx.recv().expect("second provider request");
        handle.join().expect("join");

        assert!(second_request.contains("\"type\":\"tool_result\""));
        assert!(second_request.contains("\"tool_use_id\":\"toolu_missing_read_1\""));
        assert!(second_request.contains("\"is_error\":true"));
        assert!(second_request.contains("Tool execution failed:"));
        assert_eq!(outcome.rounds, 2);
        assert_eq!(outcome.tool_executions, 1);
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
            .expect("restore live session");
        let latest = restored
            .closed_turns
            .last()
            .expect("turn should be materialized after model continuation");
        assert!(restored.active_turn.is_none());
        assert!(outcome.turns.iter().any(|turn| {
            turn.tool_calls
                .iter()
                .any(|call| call.tool_call.tool_name == "read_file")
                && turn.tool_results.iter().any(|result| {
                    result.tool_result.tool_call_id.as_str() == "toolu_missing_read_1"
                        && result.tool_result.status == ToolResultStatus::Failed
                })
        }));
        assert!(outcome.broadcasts.iter().any(|event| {
            matches!(
                event,
                ReasonBroadcastEvent::ModelContinuationWaiting(waiting)
                    if waiting.detail.contains("1 failed / 1 total")
            )
        }));
        assert_eq!(
            latest
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );
        assert!(latest.error_events.is_empty());
    }

    #[test]
    fn live_bridge_blocks_external_master_work_then_accepts_worker_dispatch() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let external_workspace = temp_runtime_home().join("external-repo");
        fs::create_dir_all(&runtime_home).expect("create runtime home");
        fs::create_dir_all(&external_workspace).expect("create external workspace");
        fs::write(external_workspace.join("secret.txt"), "must-not-be-read")
            .expect("write external fixture");
        let task_id = "task-cross-workspace-boundary";
        let agent_id = "worker-cross-workspace-boundary";
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_named_response(
                    "toolu_external_read",
                    "read_file",
                    json!({"path": external_workspace.join("secret.txt")}),
                ),
                task_tool_use_response(
                    "toolu_create_worker",
                    json!({
                        "op": "create_agent",
                        "agent_id": agent_id,
                        "capabilities": ["repository"]
                    }),
                ),
                task_tool_use_response(
                    "toolu_create_task",
                    json!({
                        "op": "create",
                        "task_id": task_id,
                        "title": "Inspect external repository",
                        "content": "Inspect the target repository without master-side access",
                        "goal": "Delegate external workspace work",
                        "deliverables": ["worker report"],
                        "acceptance": ["worker owns external access"],
                        "priority": 90,
                        "target_cwd": external_workspace,
                        "dispatch": {"mode": "none"}
                    }),
                ),
                task_tool_use_response(
                    "toolu_assign_task",
                    json!({
                        "op": "assign",
                        "task_id": task_id,
                        "agent_id": agent_id
                    }),
                ),
                complete_single_response("delegated external workspace"),
            ],
        );
        let mut request = live_request(false);
        request.runtime_home = runtime_home.clone();
        request.cwd = Some(external_workspace.clone());

        let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        selected.paired_agent_name = agent_id.to_owned();
        let outcome = run_live_reason_turn(&selected, request)
            .expect("boundary failure must return to model and permit task dispatch");
        let requests = (0..5)
            .map(|_| rx.recv().expect("provider request"))
            .collect::<Vec<_>>();
        handle.join().expect("join");

        assert!(!requests[0].contains("\"name\":\"bash\""));
        assert!(requests[0].contains("\"name\":\"task\""));
        assert!(requests[1].contains("Master workspace boundary"));
        assert!(requests[1].contains("not evidence that"));
        assert!(requests[1].contains("task(op=\\\"create\\\""));
        assert!(!requests[1].contains("must-not-be-read"));
        assert!(requests[2].contains("Agent created"));
        assert!(requests[3].contains("Task created"));
        assert!(requests[4].contains("Task assigned"));
        assert_eq!(outcome.rounds, 5);
        assert_eq!(outcome.tool_executions, 4);
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );

        let task_runtime =
            TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
        let task = task_runtime
            .query_task(&TaskId::new(task_id))
            .expect("delegated task");
        assert_eq!(
            task.assignee.as_ref().map(|assignee| &assignee.agent_id),
            Some(&AgentId::new(agent_id))
        );
        assert_eq!(task.target_cwd.as_deref(), external_workspace.to_str());
        let _ = fs::remove_dir_all(runtime_home);
        let _ = fs::remove_dir_all(
            external_workspace
                .parent()
                .expect("external workspace parent"),
        );
    }

    #[test]
    fn live_bridge_returns_unknown_tool_as_failed_tool_result_without_terminalizing() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_unknown_response(),
                complete_single_response("recovered after unknown tool"),
            ],
        );
        let request = live_request(false);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("unknown tool should be returned to model as failed tool result");
        let _first_request = rx.recv().expect("first provider request");
        let second_request = rx.recv().expect("second provider request");
        handle.join().expect("join");

        assert!(second_request.contains("\"type\":\"tool_result\""));
        assert!(second_request.contains("\"tool_use_id\":\"toolu_unknown_1\""));
        assert!(second_request.contains("\"is_error\":true"));
        assert!(second_request.contains("unknown tool `totally_unknown_tool`"));
        assert_eq!(outcome.rounds, 2);
        assert_eq!(outcome.tool_executions, 1);
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );
        let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
        assert!(metadata.iter().any(|record| {
            record.owner.feature_id.as_str() == "error.center"
                && record.write_node.pipeline_node == "RuntimeLive03ToolExecuted"
                && record
                    .entries
                    .iter()
                    .any(|entry| entry.key == "error.domain" && entry.value == json!("tool"))
                && record.entries.iter().any(|entry| {
                    entry.key == "error.recovery_action" && entry.value == json!("repair_schema")
                })
        }));
        assert!(metadata.iter().all(|record| {
            let encoded = serde_json::to_string(record).expect("metadata json");
            !encoded.contains("unknown tool `totally_unknown_tool`")
        }));
    }

    #[test]
    fn live_dispatch_projects_failed_tool_result_without_command_failure() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_unknown_response(),
                complete_single_response("dispatch recovered after first failure"),
                tool_use_unknown_response(),
                complete_single_response("dispatch recovered after second failure"),
            ],
        );
        let runtime_home = temp_runtime_home();
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            runtime_home,
            false,
        )
        .expect("runtime");

        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "trigger tool failure".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("submit should continue after tool execution failure");
        let _ = rx.recv().expect("first provider request");
        let first_reentry = rx.recv().expect("first reentry provider request");
        assert!(
            receipt
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        assert!(first_reentry.contains("\"is_error\":true"));
        assert!(first_reentry.contains("unknown tool `totally_unknown_tool`"));
        let second_receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "trigger tool failure again".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("second submit should also continue after tool execution failure");
        let _ = rx.recv().expect("second provider request");
        let second_reentry = rx.recv().expect("second reentry provider request");
        handle.join().expect("join");
        assert!(
            second_receipt
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        assert!(second_reentry.contains("\"is_error\":true"));

        let latest = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match latest {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(turn.turn_id, TurnId::new("runtime-turn-2-r2"));
                assert!(
                    turn.tool_activities.is_empty(),
                    "final round must not aggregate failed tool activity from the previous round"
                );
                assert_eq!(turn.terminal_status, Some(TerminalStatus::Success));
                assert!(
                    turn.terminal_text.as_deref().is_some_and(
                        |text| text.contains("dispatch recovered after second failure")
                    )
                );
                assert!(turn.errors.is_empty());
            }
            other => panic!("unexpected failed latest turn: {other:?}"),
        }
        let transcript = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QuerySessionTurns {
                session_id: SessionId::new("runtime-session-agent-live"),
            })
            .expect("query transcript");
        match transcript {
            UiQueryResult::SessionTurns(transcript) => {
                let failed_tool_round = transcript
                    .turns
                    .iter()
                    .find(|turn| turn.turn_id == TurnId::new("runtime-turn-2"))
                    .expect("second request first round");
                assert_eq!(failed_tool_round.tool_activities.len(), 1);
                assert_eq!(
                    failed_tool_round.tool_activities[0].status.as_str(),
                    "failed"
                );
                assert!(failed_tool_round.terminal_status.is_none());
            }
            other => panic!("unexpected transcript query: {other:?}"),
        }
    }

    #[test]
    fn live_bridge_fails_explicitly_when_runtime_metadata_ledger_is_not_writable() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let request = live_request(false);
        let metadata_path = metadata_ledger_path(
            &request.runtime_home,
            &AgentId::new("agent-live"),
            &request.session_id,
        );
        fs::create_dir_all(&metadata_path).expect("poison metadata path as directory");

        let err = run_live_reason_turn(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            request,
        )
        .expect_err("must fail when metadata ledger is unwritable");

        assert!(matches!(err, RuntimeLiveBridgeError::MetadataFailed(_)));
    }

    #[test]
    fn live_bridge_fails_explicitly_when_provider_raw_ledger_is_not_writable() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, _rx, handle) =
            spawn_mock_server(200, "application/json", complete_single_response("pong"));
        let request = live_request(false);
        let raw_path = request
            .runtime_home
            .join("ledgers")
            .join("providers")
            .join("anthropic")
            .join("agent-live")
            .join(request.session_id.as_str())
            .join("turn-live.jsonl");
        fs::create_dir_all(&raw_path).expect("poison provider raw ledger path as directory");

        let err = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect_err("must fail when provider raw ledger is unwritable");
        handle.join().expect("join");

        assert!(matches!(
            err,
            RuntimeLiveBridgeError::ReasonPersistenceFailed(_)
        ));
    }

    #[test]
    fn live_bridge_blocks_after_three_invalid_schema_retries_without_failed_status() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, _rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                invalid_complete_response(),
                invalid_complete_response(),
                invalid_complete_response(),
            ],
        );

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
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
            Some(TerminalStatus::Blocked)
        );
    }

    #[test]
    fn live_bridge_interrupts_non_candidate_max_tokens_without_failed_status() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, _rx, handle) =
            spawn_mock_server(200, "application/json", max_tokens_text_response());

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(false),
        )
        .expect("live bridge should materialize interrupted turn");
        handle.join().expect("join");

        assert_eq!(outcome.rounds, 1);
        assert_eq!(outcome.schema_rejections.len(), 0);
        let terminal = outcome
            .turn
            .terminal_event
            .as_ref()
            .expect("terminal event");
        assert_eq!(terminal.status, TerminalStatus::Interrupted);
        assert!(
            terminal
                .summary
                .contains("Provider ended before completion schema was available: max_tokens")
        );
    }

    #[test]
    fn live_bridge_rejects_unsupported_provider_selection() {
        let err = run_live_reason_turn(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::OpenAi,
            ),
            live_request(false),
        )
        .expect_err("must fail");

        assert!(matches!(
            err,
            RuntimeLiveBridgeError::UnsupportedLiveProvider { provider, protocol }
                if provider == "openai" && protocol == "chat_completions"
        ));
    }

    #[test]
    fn live_bridge_writes_provider_error_metadata_on_executor_failure() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Return HTTP 500 so the executor returns HttpStatus, which maps to
        // RuntimeLiveBridgeError::AnthropicExecutorFailed and triggers
        // RuntimeLive05ProviderError metadata + debug emission.
        let (base_url, _rx, handle) = spawn_status_sequence_server(
            (0..PROVIDER_EXECUTOR_RETRY_CAP)
                .map(|_| {
                    (
                        500,
                        "application/json",
                        r#"{"error":{"type":"internal_error","message":"server exploded"}}"#
                            .to_string(),
                    )
                })
                .collect(),
        );
        let request = live_request(false);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();
        let metadata_path =
            metadata_ledger_path(&runtime_home, &AgentId::new("agent-live"), &session_id);

        let err = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect_err("must fail on HTTP 500");

        assert!(matches!(
            err,
            RuntimeLiveBridgeError::AnthropicExecutorFailed(ref msg)
                if msg.contains("500")
        ));

        // Verify provider error metadata was written to the durable ledger.
        let raw = fs::read_to_string(&metadata_path).expect("read metadata ledger");
        let records: Vec<MetadataEnvelope> = raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("decode metadata"))
            .collect();

        assert!(records.iter().any(|record| {
            record.kind == MetadataKind::Provider
                && record.write_node.pipeline_node == "RuntimeLive05ProviderError"
                && record
                    .entries
                    .iter()
                    .any(|e| e.key == "error.kind" && e.value == json!("executor_failure"))
        }));
        assert!(records.iter().any(|record| {
            record.owner.feature_id.as_str() == "error.center"
                && record.write_node.pipeline_node == "RuntimeLive05ProviderError"
                && record
                    .entries
                    .iter()
                    .any(|entry| entry.key == "error.domain" && entry.value == json!("provider"))
                && record.entries.iter().any(|entry| {
                    entry.key == "error.recovery_action" && entry.value == json!("fail_turn")
                })
                && record.entries.iter().any(|entry| {
                    entry.key == "error.retry_index"
                        && entry.value == json!(PROVIDER_EXECUTOR_RETRY_CAP)
                })
        }));
        assert!(
            records
                .iter()
                .filter(|record| record.owner.feature_id.as_str() == "error.center")
                .all(|record| {
                    let encoded = serde_json::to_string(record).expect("metadata json");
                    !encoded.contains("server exploded")
                })
        );

        let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
            .restore(&session_id)
            .expect("restore failed provider turn");
        assert!(restored.active_turn.is_none());
        let failed_turn = restored
            .closed_turns
            .last()
            .expect("provider failure must close the turn");
        assert_eq!(
            failed_turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(freehand_contracts::TerminalStatus::Failed)
        );
        assert!(failed_turn.error_events.iter().any(|event| {
            event.error.code == "anthropic_http_status_500"
                && event
                    .error
                    .message
                    .contains("anthropic live executor failed")
        }));

        let _ = handle.join();
        let _ = fs::remove_dir_all(&runtime_home);
    }

    #[test]
    fn live_bridge_creates_checkpoint_for_write_file_and_rewinds_created_file() {
        with_temp_workspace(|root| {
            fs::create_dir_all(root.join("scratch")).expect("create parent directory");
            let (base_url, rx, handle) = spawn_sequence_server(
                "application/json",
                vec![
                    tool_use_write_file_response("scratch/note.txt", "pong\n"),
                    complete_single_response("write done"),
                ],
            );
            let mut request = live_request(false);
            request.runtime_home = root.to_path_buf();
            let runtime_home = request.runtime_home.clone();
            let session_id = request.session_id.clone();

            let outcome = run_live_reason_turn(
                &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
                request,
            )
            .expect("live bridge");
            let _ = rx.recv().expect("first provider request");
            let _ = rx.recv().expect("second provider request");
            handle.join().expect("join");

            assert_eq!(outcome.tool_executions, 1);
            let file_path = root.join("scratch/note.txt");
            assert_eq!(
                fs::read_to_string(&file_path).expect("written file"),
                "pong\n"
            );

            let rows = checkpoint_ledger_rows(&runtime_home, "agent-live", &session_id);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].event, RuntimeCheckpointLedgerEvent::Created);
            assert_eq!(rows[1].event, RuntimeCheckpointLedgerEvent::Applied);
            let checkpoint_id = rows[0].checkpoint_id.clone();

            let store = RuntimeCheckpointStore::new(
                &runtime_home,
                &AgentId::new("agent-live"),
                &session_id,
            )
            .expect("checkpoint store");
            let manifest = store.load_manifest(&checkpoint_id).expect("manifest");
            assert_eq!(manifest.entries.len(), 1);
            assert_eq!(manifest.entries[0].kind, ToolPreviewChangeKind::Create);
            assert_eq!(manifest.entries[0].blob_file, None);

            rewind_checkpoint(
                &runtime_home,
                &AgentId::new("agent-live"),
                &session_id,
                &checkpoint_id,
            )
            .expect("rewind");
            assert!(!file_path.exists());

            let rows = checkpoint_ledger_rows(&runtime_home, "agent-live", &session_id);
            assert_eq!(rows.len(), 3);
            assert_eq!(rows[2].event, RuntimeCheckpointLedgerEvent::Restored);
        });
    }

    #[test]
    fn live_bridge_rewinds_modify_checkpoint_back_to_original_text() {
        with_temp_workspace(|root| {
            let file_path = root.join("edit-target.txt");
            fs::write(&file_path, "before\n").expect("seed file");

            let (base_url, rx, handle) = spawn_sequence_server(
                "application/json",
                vec![
                    tool_use_edit_file_response("edit-target.txt", "before", "after"),
                    complete_single_response("edit done"),
                ],
            );
            let mut request = live_request(false);
            request.runtime_home = root.to_path_buf();
            let runtime_home = request.runtime_home.clone();
            let session_id = request.session_id.clone();

            let outcome = run_live_reason_turn(
                &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
                request,
            )
            .expect("live bridge");
            let _ = rx.recv().expect("first provider request");
            let _ = rx.recv().expect("second provider request");
            handle.join().expect("join");

            assert_eq!(outcome.tool_executions, 1);
            assert_eq!(
                fs::read_to_string(&file_path).expect("edited file"),
                "after\n"
            );

            let rows = checkpoint_ledger_rows(&runtime_home, "agent-live", &session_id);
            assert_eq!(rows[0].event, RuntimeCheckpointLedgerEvent::Created);
            assert_eq!(rows[1].event, RuntimeCheckpointLedgerEvent::Applied);
            let checkpoint_id = rows[0].checkpoint_id.clone();

            let store = RuntimeCheckpointStore::new(
                &runtime_home,
                &AgentId::new("agent-live"),
                &session_id,
            )
            .expect("checkpoint store");
            let manifest = store.load_manifest(&checkpoint_id).expect("manifest");
            assert_eq!(manifest.entries[0].kind, ToolPreviewChangeKind::Modify);
            assert_eq!(manifest.entries[0].blob_file.as_deref(), Some("blob-0.txt"));

            rewind_checkpoint(
                &runtime_home,
                &AgentId::new("agent-live"),
                &session_id,
                &checkpoint_id,
            )
            .expect("rewind");
            assert_eq!(
                fs::read_to_string(&file_path).expect("rewound file"),
                "before\n"
            );
        });
    }

    #[test]
    fn rewind_checkpoint_rejects_missing_manifest_explicitly() {
        let runtime_home = temp_runtime_home();
        fs::create_dir_all(&runtime_home).expect("create runtime home");
        let err = rewind_checkpoint(
            &runtime_home,
            &AgentId::new("agent-live"),
            &SessionId::new("session-live"),
            "checkpoint-missing",
        )
        .expect_err("missing manifest must fail");

        assert_eq!(
            err,
            RuntimeCheckpointError::MissingManifest("checkpoint-missing".to_owned())
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn rewind_checkpoint_rejects_missing_blob_file_explicitly() {
        with_temp_workspace(|root| {
            let file_path = root.join("edit-target.txt");
            fs::write(&file_path, "before\n").expect("seed file");

            let (base_url, rx, handle) = spawn_sequence_server(
                "application/json",
                vec![
                    tool_use_edit_file_response("edit-target.txt", "before", "after"),
                    complete_single_response("edit done"),
                ],
            );
            let mut request = live_request(false);
            request.runtime_home = root.to_path_buf();
            let runtime_home = request.runtime_home.clone();
            let session_id = request.session_id.clone();

            let outcome = run_live_reason_turn(
                &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
                request,
            )
            .expect("live bridge");
            let _ = rx.recv().expect("first provider request");
            let _ = rx.recv().expect("second provider request");
            handle.join().expect("join");

            assert_eq!(outcome.tool_executions, 1);
            let rows = checkpoint_ledger_rows(&runtime_home, "agent-live", &session_id);
            let checkpoint_id = rows[0].checkpoint_id.clone();

            let store = RuntimeCheckpointStore::new(
                &runtime_home,
                &AgentId::new("agent-live"),
                &session_id,
            )
            .expect("checkpoint store");
            let manifest = store.load_manifest(&checkpoint_id).expect("manifest");
            let blob = manifest.entries[0]
                .blob_file
                .clone()
                .expect("modify checkpoint blob");
            fs::remove_file(
                runtime_home
                    .join("state")
                    .join("checkpoints")
                    .join("agent-live")
                    .join(session_id.as_str())
                    .join(&checkpoint_id)
                    .join(&blob),
            )
            .expect("remove blob");

            let err = rewind_checkpoint(
                &runtime_home,
                &AgentId::new("agent-live"),
                &session_id,
                &checkpoint_id,
            )
            .expect_err("missing blob must fail");
            assert_eq!(
                err,
                RuntimeCheckpointError::MissingBlob {
                    checkpoint_id: checkpoint_id.clone(),
                    blob: blob.clone(),
                }
            );
            assert_eq!(
                fs::read_to_string(&file_path).expect("post-failure file still modified"),
                "after\n"
            );
        });
    }

    #[test]
    fn list_checkpoints_rejects_corrupt_ledger_line_explicitly() {
        let runtime_home = temp_runtime_home();
        let session_id = SessionId::new("session-live");
        let ledger_dir = runtime_home
            .join("ledgers")
            .join("checkpoints")
            .join("agent-live");
        fs::create_dir_all(&ledger_dir).expect("create ledger dir");
        fs::write(
            ledger_dir.join(format!("{}.jsonl", session_id.as_str())),
            "{not-json}\n",
        )
        .expect("write corrupt ledger");

        let err = list_checkpoints(&runtime_home, &AgentId::new("agent-live"), &session_id)
            .expect_err("corrupt ledger must fail");
        match err {
            RuntimeCheckpointError::PersistenceFailed(message) => {
                assert!(message.contains("checkpoint ledger line 1 failed to parse"));
            }
            other => panic!("unexpected corrupt-ledger error: {other:?}"),
        }
    }

    #[test]
    fn live_bridge_executes_bash_without_checkpoint_preview() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_bash_response("printf 'pong'"),
                complete_single_response("bash done"),
            ],
        );
        let request = live_request(false);
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("live bridge");
        let _ = rx.recv().expect("first provider request");
        let second_request = rx.recv().expect("second provider request");
        handle.join().expect("join");

        assert!(second_request.contains("\"type\":\"tool_result\""));
        assert!(second_request.contains("pong"));
        assert_eq!(outcome.tool_executions, 1);
        assert_eq!(outcome.rounds, 2);
        let checkpoint_path = runtime_home
            .join("ledgers")
            .join("checkpoints")
            .join("agent-live")
            .join(format!("{}.jsonl", session_id.as_str()));
        assert!(!checkpoint_path.exists());
    }

    #[test]
    fn bootstrap_with_live_restore_recovers_ui_projection_and_next_turn_ordinal() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let session_id = SessionId::new("runtime-session-agent-live");
        let (first_url, first_rx, first_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("first done"),
            ],
        );
        let selected = live_selected_agent(first_url, freehand_config::ProviderType::Anthropic);
        let first_outcome = run_live_reason_turn(
            &selected,
            LiveReasonTurnRequest {
                runtime_home: runtime_home.clone(),
                session_id: session_id.clone(),
                turn_id: TurnId::new("runtime-turn-1"),
                trace_id: TraceId::new("runtime-trace-1"),
                prompt: "first request".to_owned(),
                cwd: None,
                stream: false,
                cancel_token: None,
            },
        )
        .expect("first live turn");
        let _ = first_rx.recv().expect("first provider request");
        let _ = first_rx.recv().expect("second provider request");
        first_handle.join().expect("join first provider");
        assert_eq!(
            first_outcome.turn.request.turn_id,
            TurnId::new("runtime-turn-1-r2")
        );

        let (second_url, second_rx, second_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("second done"),
            ],
        );
        let mut restored_selected = selected.clone();
        restored_selected.provider.base_url = second_url;
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &restored_selected,
            runtime_home.clone(),
            false,
        )
        .expect("restored runtime");

        let latest = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match latest {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(turn.turn_id, TurnId::new("runtime-turn-1-r2"));
                assert!(
                    turn.terminal_text
                        .as_deref()
                        .is_some_and(|text| text.contains("Summary: first done"))
                );
            }
            other => panic!("unexpected restored latest turn: {other:?}"),
        }

        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "second request".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("second receipt");
        assert_eq!(
            receipt.dispatch_status,
            "reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 restored_closed_turns=1"
        );
        let _ = second_rx.recv().expect("restart provider request");
        let _ = second_rx.recv().expect("restart tool-result request");
        second_handle.join().expect("join second provider");

        let latest = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match latest {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(turn.turn_id, TurnId::new("runtime-turn-2-r2"));
                assert!(
                    turn.terminal_text
                        .as_deref()
                        .is_some_and(|text| text.contains("Summary: second done"))
                );
            }
            other => panic!("unexpected latest turn after restart submit: {other:?}"),
        }
    }

    #[test]
    fn live_restore_resumes_turn_ordinal_from_selected_non_default_session() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let selected_session_id = SessionId::new("webui-session-selected-ordinal");
        let (first_url, first_rx, first_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("selected first done"),
            ],
        );
        let selected = live_selected_agent(first_url, freehand_config::ProviderType::Anthropic);
        let first_outcome = run_live_reason_turn(
            &selected,
            LiveReasonTurnRequest {
                runtime_home: runtime_home.clone(),
                session_id: selected_session_id.clone(),
                turn_id: TurnId::new("runtime-turn-1"),
                trace_id: TraceId::new("runtime-trace-1"),
                prompt: "selected first request".to_owned(),
                cwd: None,
                stream: false,
                cancel_token: None,
            },
        )
        .expect("first selected live turn");
        let _ = first_rx.recv().expect("first selected provider request");
        let _ = first_rx.recv().expect("first selected tool-result request");
        first_handle.join().expect("join first selected provider");
        assert_eq!(
            first_outcome.turn.request.turn_id,
            TurnId::new("runtime-turn-1-r2")
        );

        let (second_url, second_rx, second_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("selected second done"),
            ],
        );
        let mut restored_selected = selected.clone();
        restored_selected.provider.base_url = second_url;
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &restored_selected,
            runtime_home.clone(),
            false,
        )
        .expect("restored runtime");

        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "selected second request".to_owned(),
                    session_id: Some(selected_session_id.clone()),
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("second selected receipt");
        assert_eq!(
            receipt.dispatch_status,
            "reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 restored_closed_turns=1"
        );
        let _ = second_rx.recv().expect("second selected provider request");
        let _ = second_rx
            .recv()
            .expect("second selected tool-result request");
        second_handle.join().expect("join second selected provider");

        let latest = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QuerySessionTurns {
                session_id: selected_session_id,
            })
            .expect("query selected session");
        match latest {
            UiQueryResult::SessionTurns(transcript) => {
                let turn_ids = transcript
                    .turns
                    .iter()
                    .map(|turn| turn.turn_id.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(
                    turn_ids,
                    vec![
                        "runtime-turn-1",
                        "runtime-turn-1-r2",
                        "runtime-turn-2",
                        "runtime-turn-2-r2"
                    ]
                );
            }
            other => panic!("unexpected selected session turns after restart submit: {other:?}"),
        }
    }

    #[test]
    fn submit_input_dispatches_to_reason_and_updates_ui_state() {
        let runtime = runtime();
        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "hello runtime".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("receipt");
        assert_eq!(receipt.target_feature_id, "reason.turn");
        assert_eq!(receipt.dispatch_status, "reason_turn_started");

        let ui_state = runtime.ui_state();
        let latest = ui_state
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match latest {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(turn.source.source_node_id, "master-node");
                assert_eq!(turn.turn_id, TurnId::new("runtime-turn-1"));
                assert_eq!(turn.user_text.as_deref(), Some("hello runtime"));
                let public = freehand_ui_protocol::public_turn_projection(turn);
                assert_eq!(public.public_conversation[0].body, "hello runtime");
            }
            other => panic!("unexpected latest turn query: {other:?}"),
        }
    }

    #[test]
    fn cancel_turn_dispatches_to_reason_owner() {
        let runtime = runtime();
        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "cancel me".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("submit");

        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::CancelTurn {
                    turn_id: TurnId::new("runtime-turn-1"),
                })
                .expect("envelope"),
            )
            .expect("cancel receipt");
        assert_eq!(receipt.dispatch_status, "reason_turn_cancelled");

        let ui_state = runtime.ui_state();
        let latest = ui_state
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match latest {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(
                    turn.terminal_text.as_deref(),
                    Some("cancelled by ui command")
                );
            }
            other => panic!("unexpected latest turn query: {other:?}"),
        }
    }

    #[test]
    fn cancel_latest_active_turn_dispatches_to_latest_reason_turn() {
        let runtime = runtime();
        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "cancel latest".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("submit envelope"),
            )
            .expect("submit");

        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::CancelLatestActiveTurn {})
                    .expect("cancel latest envelope"),
            )
            .expect("cancel latest receipt");
        assert_eq!(receipt.ingress.command_kind, "cancel_latest_active_turn");
        assert_eq!(receipt.dispatch_status, "reason_turn_cancelled");

        let latest = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match latest {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(turn.terminal_status, Some(TerminalStatus::Cancelled));
            }
            other => panic!("unexpected latest turn query: {other:?}"),
        }
    }

    #[test]
    fn cancel_turn_missing_target_returns_target_not_found() {
        let runtime = runtime();
        let err = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::CancelTurn {
                    turn_id: TurnId::new("runtime-turn-missing"),
                })
                .expect("cancel envelope"),
            )
            .expect_err("missing turn must fail");
        assert_eq!(
            err,
            UiCommandDispatchPortError::TargetNotFound("runtime-turn-missing".to_owned())
        );
    }

    #[test]
    fn cancel_latest_active_turn_without_any_turn_returns_target_not_found() {
        let runtime = runtime();
        let err = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::CancelLatestActiveTurn {})
                    .expect("cancel latest envelope"),
            )
            .expect_err("empty runtime must fail");
        assert_eq!(
            err,
            UiCommandDispatchPortError::TargetNotFound("latest-active-turn".to_owned())
        );
    }

    #[test]
    fn active_live_cancel_returns_before_provider_finishes_and_blocks_success_projection() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let first_chunk = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"working\"}}\n\n"
        )
        .to_owned();
        let remaining_chunks = complete_stream_response("late success");
        let (base_url, _rx, released_rx, continue_tx, handle) =
            spawn_incremental_stream_server(first_chunk, remaining_chunks);
        let runtime = Arc::new(
            RuntimeCommandDispatcher::from_selected_agent_with_live(
                &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
                temp_runtime_home(),
                true,
            )
            .expect("runtime"),
        );
        let submit_runtime = Arc::clone(&runtime);
        let submit_handle = thread::spawn(move || {
            submit_runtime.dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "start long stream".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("submit envelope"),
            )
        });

        loop {
            let latest = runtime
                .ui_state()
                .lock()
                .expect("lock ui state")
                .query(&UiCommand::QueryLatestActiveTurn)
                .expect("query");
            if matches!(latest, UiQueryResult::Turn(Some(_))) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        let cancel_receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::CancelTurn {
                    turn_id: TurnId::new("runtime-turn-1"),
                })
                .expect("cancel envelope"),
            )
            .expect("cancel receipt");
        assert_eq!(
            cancel_receipt.dispatch_status,
            "reason_live_turn_cancel_requested"
        );

        let latest = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match latest {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(turn.terminal_status, Some(TerminalStatus::Cancelled));
                let public = freehand_ui_protocol::public_turn_projection(turn);
                assert_eq!(
                    public
                        .public_conversation
                        .last()
                        .map(|item| item.status.as_str()),
                    Some("cancelled")
                );
            }
            other => panic!("unexpected cancelled latest turn: {other:?}"),
        }

        continue_tx.send(()).expect("release provider");
        let released = released_rx.recv().expect("release status");
        assert!(released);
        let submit_err = submit_handle
            .join()
            .expect("submit thread")
            .expect_err("submit should observe cancellation");
        assert_eq!(
            submit_err,
            UiCommandDispatchPortError::DispatchFailed("live turn cancelled".to_owned())
        );
        handle.join().expect("join provider");

        let latest = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match latest {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(turn.terminal_status, Some(TerminalStatus::Cancelled));
                assert!(
                    turn.terminal_text
                        .as_deref()
                        .is_some_and(|text| text.contains("cancelled"))
                );
            }
            other => panic!("unexpected final cancelled latest turn: {other:?}"),
        }
    }

    #[test]
    fn direct_message_dispatches_to_node_owner() {
        let runtime = runtime();
        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SendDirectMessageToSlave {
                    node_id: "slave-node".to_owned(),
                    text: "ping".to_owned(),
                })
                .expect("envelope"),
            )
            .expect("receipt");
        assert_eq!(receipt.target_feature_id, "node.master-slave");
        assert_eq!(receipt.dispatch_status, "node_direct_message_dispatched");
    }

    #[test]
    fn direct_message_wrong_slave_target_returns_target_not_found() {
        let runtime = runtime();
        let err = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SendDirectMessageToSlave {
                    node_id: "wrong-slave".to_owned(),
                    text: "ping".to_owned(),
                })
                .expect("envelope"),
            )
            .expect_err("wrong node must fail");
        assert_eq!(
            err,
            UiCommandDispatchPortError::TargetNotFound("wrong-slave".to_owned())
        );
    }

    #[test]
    fn rewind_checkpoint_dispatch_rejects_non_live_runtime() {
        let runtime = runtime();
        let err = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::RewindCheckpoint {
                    checkpoint_id: "checkpoint-1".to_owned(),
                })
                .expect("envelope"),
            )
            .expect_err("rewind should fail");
        assert_eq!(
            err,
            UiCommandDispatchPortError::Unsupported(
                "rewind dispatch requires a live runtime home".to_owned()
            )
        );
    }

    #[test]
    fn rewind_checkpoint_dispatch_restores_workspace_file_state() {
        with_temp_workspace(|root| {
            fs::create_dir_all(root.join("scratch")).expect("create parent directory");
            let (base_url, rx, handle) = spawn_sequence_server(
                "application/json",
                vec![
                    tool_use_write_file_response("scratch/rewind.txt", "rewind me\n"),
                    complete_single_response("write done"),
                ],
            );
            let runtime_home = root.to_path_buf();
            let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
                &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
                runtime_home.clone(),
                false,
            )
            .expect("runtime");

            let receipt = runtime
                .dispatch(
                    build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                        text: "create checkpoint".to_owned(),
                        session_id: None,
                        cwd: None,
                    })
                    .expect("envelope"),
                )
                .expect("submit receipt");
            assert!(
                receipt
                    .dispatch_status
                    .contains("reason_live_turn_completed")
            );
            let _ = rx.recv().expect("first provider request");
            let _ = rx.recv().expect("second provider request");
            handle.join().expect("join");

            let file_path = root.join("scratch/rewind.txt");
            assert_eq!(
                fs::read_to_string(&file_path).expect("written file"),
                "rewind me\n"
            );
            let rows = checkpoint_ledger_rows(
                &runtime_home,
                "agent-live",
                &SessionId::new("runtime-session-agent-live"),
            );
            let checkpoint_id = rows.first().expect("created row").checkpoint_id.clone();
            let checkpoint_query = runtime
                .ui_state()
                .lock()
                .expect("lock ui state")
                .query(&UiCommand::QueryCheckpoints)
                .expect("checkpoint query");
            match checkpoint_query {
                UiQueryResult::Checkpoints(snapshot) => {
                    assert_eq!(snapshot.checkpoints.len(), 1);
                    assert_eq!(snapshot.checkpoints[0].checkpoint_id, checkpoint_id);
                    assert_eq!(snapshot.checkpoints[0].latest_status, "applied");
                }
                other => panic!("unexpected checkpoint query: {other:?}"),
            }

            let rewind = runtime
                .dispatch(
                    build_command_dispatch_envelope(&UiCommand::RewindCheckpoint {
                        checkpoint_id: checkpoint_id.clone(),
                    })
                    .expect("envelope"),
                )
                .expect("rewind receipt");
            assert_eq!(
                rewind.dispatch_status,
                format!("runtime_checkpoint_rewound checkpoint_id={checkpoint_id}")
            );
            assert!(!file_path.exists());
            let checkpoint_query = runtime
                .ui_state()
                .lock()
                .expect("lock ui state")
                .query(&UiCommand::QueryCheckpoints)
                .expect("checkpoint query");
            match checkpoint_query {
                UiQueryResult::Checkpoints(snapshot) => {
                    assert_eq!(snapshot.checkpoints[0].latest_status, "restored");
                }
                other => panic!("unexpected checkpoint query after rewind: {other:?}"),
            }
        });
    }

    #[test]
    fn rewind_checkpoint_dispatch_maps_missing_manifest_to_target_not_found() {
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            temp_runtime_home(),
            false,
        )
        .expect("runtime");

        let err = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::RewindCheckpoint {
                    checkpoint_id: "checkpoint-missing".to_owned(),
                })
                .expect("envelope"),
            )
            .expect_err("missing checkpoint must fail");
        assert_eq!(
            err,
            UiCommandDispatchPortError::TargetNotFound("checkpoint-missing".to_owned())
        );
    }

    #[test]
    fn runtime_query_reads_task_truth_from_task_runtime() {
        let runtime_home = temp_runtime_home();
        let task_runtime = TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
            .expect("task runtime boot");
        task_runtime
            .create_task(TaskCreateRequest {
                task_id: Some(TaskId::new("runtime-query-task-1")),
                title: "Runtime query task".to_owned(),
                content: "Task query bridge content".to_owned(),
                goal: "Expose persisted task truth".to_owned(),
                deliverables: vec!["task list".to_owned()],
                acceptance: vec!["task history".to_owned()],
                priority: 90,
                target_cwd: Some("/tmp".to_owned()),
                dispatch: TaskDispatchRequest::None,
                parent: TaskParentRef {
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                actor: TaskActor {
                    agent_id: AgentId::new("agent-live"),
                    source: "runtime_query_test".to_owned(),
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                watermark: TaskWatermark {
                    metadata_id: None,
                    hook: Some("runtime_query_test".to_owned()),
                    action_tool_call_id: None,
                },
            })
            .expect("create task");
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");

        let list = runtime
            .query_runtime(&UiCommand::QueryTaskList {
                status: Some("waiting_agent".to_owned()),
                agent_id: None,
            })
            .expect("task list query")
            .expect("runtime-backed task list");
        match list {
            UiQueryResult::TaskList(list) => {
                assert_eq!(list.source_agent_id.as_str(), "agent-live");
                assert_eq!(list.tasks.len(), 1);
                assert_eq!(list.tasks[0].task_id, "runtime-query-task-1");
                assert_eq!(list.tasks[0].status, "waiting_agent");
                assert_eq!(list.tasks[0].priority, 90);
            }
            other => panic!("unexpected task list result: {other:?}"),
        }

        let history = runtime
            .query_runtime(&UiCommand::QueryTaskHistory {
                task_id: "runtime-query-task-1".to_owned(),
            })
            .expect("task history query")
            .expect("runtime-backed task history");
        match history {
            UiQueryResult::TaskHistory(history) => {
                assert_eq!(history.task_id, "runtime-query-task-1");
                assert_eq!(history.events.len(), 2);
                assert_eq!(history.events[0].event_type, "TaskCreated");
                assert_eq!(history.events[1].event_type, "TaskWaitingAgent");
            }
            other => panic!("unexpected task history result: {other:?}"),
        }

        let err = runtime
            .query_runtime(&UiCommand::QueryTaskHistory {
                task_id: "missing-runtime-task".to_owned(),
            })
            .expect_err("missing task history must fail");
        assert_eq!(
            err,
            UiCommandDispatchPortError::TargetNotFound("missing-runtime-task".to_owned())
        );

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_query_reads_phase1_task_and_agent_boards() {
        let runtime_home = temp_runtime_home();
        let task_runtime = TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
            .expect("task runtime boot");
        task_runtime
            .create_task(TaskCreateRequest {
                task_id: Some(TaskId::new("runtime-phase1-board-task")),
                title: "Runtime phase1 board task".to_owned(),
                content: "TaskBoard and AgentBoard query bridge content".to_owned(),
                goal: "Expose phase1 board truth".to_owned(),
                deliverables: vec!["task board".to_owned()],
                acceptance: vec!["agent board".to_owned()],
                priority: 91,
                target_cwd: Some("/tmp".to_owned()),
                dispatch: TaskDispatchRequest::SelfAgent,
                parent: TaskParentRef {
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                actor: TaskActor {
                    agent_id: AgentId::new("agent-live"),
                    source: "runtime_phase1_board_test".to_owned(),
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                watermark: TaskWatermark {
                    metadata_id: None,
                    hook: Some("runtime_phase1_board_test".to_owned()),
                    action_tool_call_id: None,
                },
            })
            .expect("create task");
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");

        let task_board = runtime
            .query_runtime(&UiCommand::QueryTaskBoard {
                status: None,
                agent_id: None,
                include_terminal: false,
            })
            .expect("task board query")
            .expect("runtime-backed task board");
        match task_board {
            UiQueryResult::TaskBoard(board) => {
                assert_eq!(board.source_agent_id.as_str(), "agent-live");
                assert_eq!(board.tasks.len(), 1);
                assert_eq!(board.tasks[0].task_id, "runtime-phase1-board-task");
                assert_eq!(board.tasks[0].status, "assigned");
                assert_eq!(board.agents.len(), 1);
                assert_eq!(board.agents[0].agent_id.as_str(), "agent-live");
                assert_eq!(
                    board.agents[0].current_task_id.as_deref(),
                    Some("runtime-phase1-board-task")
                );
            }
            other => panic!("unexpected task board result: {other:?}"),
        }

        let agent_board = runtime
            .query_runtime(&UiCommand::QueryAgentBoard)
            .expect("agent board query")
            .expect("runtime-backed agent board");
        match agent_board {
            UiQueryResult::AgentBoard(board) => {
                assert_eq!(board.source_agent_id.as_str(), "agent-live");
                assert_eq!(board.agents.len(), 1);
                assert_eq!(board.agents[0].agent_id.as_str(), "agent-live");
                assert_eq!(board.agents[0].state, "assigned");
                assert_eq!(
                    board.agents[0].current_task_id.as_deref(),
                    Some("runtime-phase1-board-task")
                );
            }
            other => panic!("unexpected agent board result: {other:?}"),
        }

        let lifecycle = runtime
            .query_runtime(&UiCommand::QueryAgentLifecycle {
                agent_id: AgentId::new("agent-live"),
            })
            .expect("agent lifecycle query")
            .expect("runtime-backed agent lifecycle");
        match lifecycle {
            UiQueryResult::AgentLifecycle(lifecycle) => {
                assert_eq!(lifecycle.agent_id.as_str(), "agent-live");
                assert_eq!(lifecycle.state, "assigned");
                assert_eq!(
                    lifecycle.current_task_id.as_deref(),
                    Some("runtime-phase1-board-task")
                );
            }
            other => panic!("unexpected lifecycle result: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_dispatch_execution_fact_and_scheduler_tick_update_task_truth() {
        let runtime_home = temp_runtime_home();
        let owner_id = AgentId::new("agent-live");
        let worker_id = AgentId::new("runtime-phase1-worker");
        let task_runtime =
            TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("task runtime boot");
        task_runtime
            .create_agent(AgentCreateRequest {
                agent_id: worker_id.clone(),
                capabilities: vec!["phase1".to_owned()],
                actor: TaskActor {
                    agent_id: owner_id.clone(),
                    source: "runtime_phase1_fact_test".to_owned(),
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                watermark: TaskWatermark {
                    metadata_id: None,
                    hook: Some("runtime_phase1_fact_test".to_owned()),
                    action_tool_call_id: None,
                },
            })
            .expect("create worker agent");
        for (task_id, title) in [
            ("runtime-phase1-review-task", "Runtime phase1 review task"),
            ("runtime-phase1-blocked-task", "Runtime phase1 blocked task"),
        ] {
            task_runtime
                .create_task(TaskCreateRequest {
                    task_id: Some(TaskId::new(task_id)),
                    title: title.to_owned(),
                    content: format!("{title} content"),
                    goal: "prove phase1 execution fact dispatch".to_owned(),
                    deliverables: vec!["execution fact".to_owned()],
                    acceptance: vec!["TaskBoard projection updates".to_owned()],
                    priority: 80,
                    target_cwd: None,
                    dispatch: TaskDispatchRequest::None,
                    parent: TaskParentRef {
                        session_id: Some(SessionId::new("runtime-phase1-fact-session")),
                        turn_id: Some(TurnId::new("runtime-phase1-fact-turn")),
                        trace_id: None,
                    },
                    actor: TaskActor {
                        agent_id: owner_id.clone(),
                        source: "runtime_phase1_fact_test".to_owned(),
                        session_id: None,
                        turn_id: None,
                        trace_id: None,
                    },
                    watermark: TaskWatermark {
                        metadata_id: None,
                        hook: Some("runtime_phase1_fact_test".to_owned()),
                        action_tool_call_id: None,
                    },
                })
                .expect("create waiting task");
            task_runtime
                .assign_task(TaskAssignRequest {
                    task_id: TaskId::new(task_id),
                    agent_id: worker_id.clone(),
                    actor: TaskActor {
                        agent_id: owner_id.clone(),
                        source: "runtime_phase1_fact_test".to_owned(),
                        session_id: None,
                        turn_id: None,
                        trace_id: None,
                    },
                    watermark: TaskWatermark {
                        metadata_id: None,
                        hook: Some("runtime_phase1_fact_test".to_owned()),
                        action_tool_call_id: None,
                    },
                })
                .expect("assign waiting task");
        }
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");
        let turn_id = TurnId::new("runtime-phase1-fact-turn");
        let agent_id = worker_id;

        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::ApplyExecutionFact {
                    fact: UiExecutionFactCommand {
                        execution_id: "runtime-phase1-exec-blocked".to_owned(),
                        task_id: "runtime-phase1-blocked-task".to_owned(),
                        agent_id: agent_id.clone(),
                        turn_id: Some(turn_id.clone()),
                        kind: UiExecutionFactKind::Running {
                            phase: "implementation".to_owned(),
                            summary: "worker started".to_owned(),
                            evidence: vec!["running evidence".to_owned()],
                        },
                    },
                })
                .expect("running fact envelope"),
            )
            .expect("running fact dispatch");
        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::ApplyExecutionFact {
                    fact: UiExecutionFactCommand {
                        execution_id: "runtime-phase1-exec-blocked".to_owned(),
                        task_id: "runtime-phase1-blocked-task".to_owned(),
                        agent_id: agent_id.clone(),
                        turn_id: Some(turn_id.clone()),
                        kind: UiExecutionFactKind::Recovering {
                            summary: "worker retrying".to_owned(),
                            evidence: vec!["recovering evidence".to_owned()],
                            retry_count: 1,
                        },
                    },
                })
                .expect("recovering fact envelope"),
            )
            .expect("recovering fact dispatch");
        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::ApplyExecutionFact {
                    fact: UiExecutionFactCommand {
                        execution_id: "runtime-phase1-exec-review".to_owned(),
                        task_id: "runtime-phase1-review-task".to_owned(),
                        agent_id: agent_id.clone(),
                        turn_id: Some(turn_id.clone()),
                        kind: UiExecutionFactKind::ReviewReady {
                            summary: "ready for review".to_owned(),
                            deliverables: vec!["review deliverable".to_owned()],
                            evidence: vec!["review evidence".to_owned()],
                        },
                    },
                })
                .expect("review fact envelope"),
            )
            .expect("review fact dispatch");

        thread::sleep(Duration::from_secs(2));
        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::RunSchedulerTick {
                    tick: UiSchedulerTickCommand {
                        stale_after_seconds: 1,
                        soft_timeout_seconds: 1,
                        hard_timeout_seconds: 30,
                    },
                })
                .expect("scheduler tick envelope"),
            )
            .expect("scheduler tick dispatch");
        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::ApplyExecutionFact {
                    fact: UiExecutionFactCommand {
                        execution_id: "runtime-phase1-exec-blocked".to_owned(),
                        task_id: "runtime-phase1-blocked-task".to_owned(),
                        agent_id,
                        turn_id: Some(turn_id),
                        kind: UiExecutionFactKind::Blocked {
                            reason: "waiting on dependency".to_owned(),
                            evidence: vec!["blocked evidence".to_owned()],
                        },
                    },
                })
                .expect("blocked fact envelope"),
            )
            .expect("blocked fact dispatch");

        let board = runtime
            .query_runtime(&UiCommand::QueryTaskBoard {
                status: None,
                agent_id: None,
                include_terminal: false,
            })
            .expect("final board query")
            .expect("final board result");
        match board {
            UiQueryResult::TaskBoard(board) => {
                assert!(
                    board
                        .blocked
                        .iter()
                        .any(|task| task.task_id == "runtime-phase1-blocked-task"),
                    "blocked view must include execution-blocked task: {:?}",
                    board.blocked
                );
                assert!(
                    board
                        .review_ready
                        .iter()
                        .any(|task| task.task_id == "runtime-phase1-review-task"),
                    "review view must include review-ready task: {:?}",
                    board.review_ready
                );
                assert!(
                    board
                        .stale
                        .iter()
                        .any(|task| task.task_id == "runtime-phase1-blocked-task"),
                    "stale view must include scheduler-observed task: {:?}",
                    board.stale
                );
            }
            other => panic!("unexpected final board result: {other:?}"),
        }

        let history = runtime
            .query_runtime(&UiCommand::QueryTaskHistory {
                task_id: "runtime-phase1-blocked-task".to_owned(),
            })
            .expect("history query")
            .expect("history result");
        match history {
            UiQueryResult::TaskHistory(history) => {
                let event_types = history
                    .events
                    .iter()
                    .map(|event| event.event_type.as_str())
                    .collect::<Vec<_>>();
                assert!(event_types.contains(&"TaskExecutionRecorded"));
                assert!(event_types.contains(&"TaskExecutionRecovering"));
                assert!(event_types.contains(&"TaskSchedulerTick"));
                assert!(event_types.contains(&"TaskBlocked"));
            }
            other => panic!("unexpected history result: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_dispatches_phase2a_master_worker_loop_into_task_truth() {
        let runtime_home = temp_runtime_home();
        let owner_id = AgentId::new("agent-live");
        let worker_id = AgentId::new("runtime-phase2a-worker");
        let task_id = "runtime-phase2a-task".to_owned();
        let execution_id = "runtime-phase2a-exec".to_owned();
        let turn_id = TurnId::new("runtime-phase2a-turn");
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");

        for command in [
            UiCommand::CreateTaskAgent {
                agent: UiTaskAgentCreateCommand {
                    agent_id: worker_id.clone(),
                    capabilities: vec!["code_edit".to_owned()],
                },
            },
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some(task_id.clone()),
                    title: "Runtime phase2a task".to_owned(),
                    content: "Runtime phase2a content".to_owned(),
                    goal: "prove runtime master worker loop".to_owned(),
                    deliverables: vec!["worker loop".to_owned()],
                    acceptance: vec!["approved before close".to_owned()],
                    priority: 90,
                    target_cwd: None,
                    session_id: Some(SessionId::new("runtime-phase2a-session")),
                    turn_id: Some(turn_id.clone()),
                    dispatch: Some(UiTaskDispatchCommand::None),
                },
            },
            UiCommand::AssignTask {
                assignment: UiTaskAssignCommand {
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                },
            },
            UiCommand::ClaimNextTask {
                claim: UiTaskClaimCommand {
                    agent_id: worker_id.clone(),
                    execution_id: execution_id.clone(),
                    ttl_seconds: Some(300),
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Running {
                        phase: "progress".to_owned(),
                        summary: "worker progress".to_owned(),
                        evidence: vec!["progress evidence".to_owned()],
                    },
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Blocked {
                        reason: "needs input".to_owned(),
                        evidence: vec!["blocked evidence".to_owned()],
                    },
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Recovering {
                        summary: "recovering".to_owned(),
                        evidence: vec!["recovery evidence".to_owned()],
                        retry_count: 1,
                    },
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::ReviewReady {
                        summary: "first review".to_owned(),
                        deliverables: vec!["draft".to_owned()],
                        evidence: vec!["draft evidence".to_owned()],
                    },
                },
            },
            UiCommand::RejectTaskReview {
                rejection: UiTaskReviewRejectionCommand {
                    task_id: task_id.clone(),
                    reject_reason: "needs retry".to_owned(),
                    next_requirements: vec!["retry".to_owned()],
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Running {
                        phase: "retry".to_owned(),
                        summary: "retry progress".to_owned(),
                        evidence: vec!["retry evidence".to_owned()],
                    },
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id),
                    kind: UiExecutionFactKind::ReviewReady {
                        summary: "second review".to_owned(),
                        deliverables: vec!["accepted".to_owned()],
                        evidence: vec!["accepted evidence".to_owned()],
                    },
                },
            },
            UiCommand::ApproveTaskReview {
                task_id: task_id.clone(),
            },
            UiCommand::CloseTask {
                task_id: task_id.clone(),
            },
        ] {
            runtime
                .dispatch(build_command_dispatch_envelope(&command).expect("phase2a envelope"))
                .expect("phase2a dispatch");
        }

        let board = runtime
            .query_runtime(&UiCommand::QueryTaskBoard {
                status: None,
                agent_id: None,
                include_terminal: true,
            })
            .expect("phase2a board query")
            .expect("phase2a board result");
        match board {
            UiQueryResult::TaskBoard(board) => {
                let task = board
                    .tasks
                    .iter()
                    .find(|task| task.task_id == task_id)
                    .expect("closed task");
                assert_eq!(task.status, "closed");
                assert_eq!(task.assignee_agent_id.as_ref(), Some(&worker_id));
                assert_eq!(
                    task.active_execution_id.as_deref(),
                    Some(execution_id.as_str())
                );
            }
            other => panic!("unexpected phase2a board result: {other:?}"),
        }

        let lifecycle = runtime
            .query_runtime(&UiCommand::QueryAgentLifecycle {
                agent_id: worker_id.clone(),
            })
            .expect("phase2a lifecycle query")
            .expect("phase2a lifecycle result");
        match lifecycle {
            UiQueryResult::AgentLifecycle(lifecycle) => {
                assert_eq!(lifecycle.agent_id, worker_id);
                assert_eq!(lifecycle.state, "closed");
                assert_eq!(
                    lifecycle.current_execution_id.as_deref(),
                    Some(execution_id.as_str())
                );
            }
            other => panic!("unexpected phase2a lifecycle result: {other:?}"),
        }

        let history = runtime
            .query_runtime(&UiCommand::QueryTaskHistory {
                task_id: task_id.clone(),
            })
            .expect("phase2a history query")
            .expect("phase2a history result");
        match history {
            UiQueryResult::TaskHistory(history) => {
                let event_types = history
                    .events
                    .iter()
                    .map(|event| event.event_type.as_str())
                    .collect::<Vec<_>>();
                for required in [
                    "TaskCreated",
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
                        event_types.contains(&required),
                        "missing {required}: {event_types:?}"
                    );
                }
            }
            other => panic!("unexpected phase2a history result: {other:?}"),
        }

        assert_eq!(owner_id.as_str(), "agent-live");
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_dispatches_phase2b_master_poll_and_event_inbox() {
        let runtime_home = temp_runtime_home();
        let worker_id = AgentId::new("runtime-phase2b-worker");
        let task_id = "runtime-phase2b-task".to_owned();
        let execution_id = "runtime-phase2b-exec".to_owned();
        let turn_id = TurnId::new("runtime-phase2b-turn");
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");

        for index in 0..55 {
            let backlog_task_id = format!("runtime-phase2b-backlog-{index:03}");
            runtime
                .dispatch(
                    build_command_dispatch_envelope(&UiCommand::CreateTask {
                        task: UiTaskCreateCommand {
                            task_id: Some(backlog_task_id),
                            title: format!("Runtime phase2b backlog {index:03}"),
                            content: "Runtime phase2b backlog content".to_owned(),
                            goal: "prove default master poll drains backlog".to_owned(),
                            deliverables: vec!["backlog event".to_owned()],
                            acceptance: vec!["backlog remains visible to EventInbox".to_owned()],
                            priority: 1,
                            target_cwd: None,
                            session_id: Some(SessionId::new(format!(
                                "runtime-phase2b-backlog-session-{index:03}"
                            ))),
                            turn_id: None,
                            dispatch: Some(UiTaskDispatchCommand::None),
                        },
                    })
                    .expect("phase2b backlog envelope"),
                )
                .expect("phase2b backlog dispatch");
        }

        for command in [
            UiCommand::CreateTaskAgent {
                agent: UiTaskAgentCreateCommand {
                    agent_id: worker_id.clone(),
                    capabilities: vec!["code_edit".to_owned()],
                },
            },
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some(task_id.clone()),
                    title: "Runtime phase2b task".to_owned(),
                    content: "Runtime phase2b content".to_owned(),
                    goal: "prove runtime master poll loop".to_owned(),
                    deliverables: vec!["event inbox".to_owned()],
                    acceptance: vec!["master poll reads state without mutating".to_owned()],
                    priority: 95,
                    target_cwd: None,
                    session_id: Some(SessionId::new("runtime-phase2b-session")),
                    turn_id: Some(turn_id.clone()),
                    dispatch: Some(UiTaskDispatchCommand::None),
                },
            },
            UiCommand::AssignTask {
                assignment: UiTaskAssignCommand {
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                },
            },
            UiCommand::ClaimNextTask {
                claim: UiTaskClaimCommand {
                    agent_id: worker_id.clone(),
                    execution_id: execution_id.clone(),
                    ttl_seconds: Some(300),
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Running {
                        phase: "phase2b_running".to_owned(),
                        summary: "worker running".to_owned(),
                        evidence: vec!["running evidence".to_owned()],
                    },
                },
            },
        ] {
            runtime
                .dispatch(build_command_dispatch_envelope(&command).expect("phase2b envelope"))
                .expect("phase2b dispatch");
        }

        thread::sleep(Duration::from_secs(2));
        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::RunSchedulerTick {
                    tick: UiSchedulerTickCommand {
                        stale_after_seconds: 1,
                        soft_timeout_seconds: 10,
                        hard_timeout_seconds: 30,
                    },
                })
                .expect("scheduler tick envelope"),
            )
            .expect("scheduler tick dispatch");
        for command in [
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Blocked {
                        reason: "needs master unblock".to_owned(),
                        evidence: vec!["blocked evidence".to_owned()],
                    },
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Recovering {
                        summary: "worker recovered".to_owned(),
                        evidence: vec!["recovery evidence".to_owned()],
                        retry_count: 1,
                    },
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id),
                    kind: UiExecutionFactKind::ReviewReady {
                        summary: "ready for master review".to_owned(),
                        deliverables: vec!["phase2b deliverable".to_owned()],
                        evidence: vec!["review evidence".to_owned()],
                    },
                },
            },
        ] {
            runtime
                .dispatch(build_command_dispatch_envelope(&command).expect("phase2b envelope"))
                .expect("phase2b dispatch");
        }

        let before_poll = runtime
            .query_runtime(&UiCommand::QueryTaskBoard {
                status: None,
                agent_id: None,
                include_terminal: true,
            })
            .expect("before poll board query")
            .expect("before poll board");
        let before_status = match before_poll {
            UiQueryResult::TaskBoard(board) => board
                .tasks
                .iter()
                .find(|task| task.task_id == task_id)
                .expect("task before poll")
                .status
                .clone(),
            other => panic!("unexpected before poll result: {other:?}"),
        };

        let inbox = runtime
            .query_runtime(&UiCommand::QueryEventInbox {
                after_cursor: None,
                limit: None,
            })
            .expect("event inbox query")
            .expect("event inbox result");
        let inbox_cursor = match inbox {
            UiQueryResult::EventInbox(inbox) => {
                assert!(
                    inbox.events.len() > 100,
                    "backlog regression must exceed old default page size"
                );
                let kinds = inbox
                    .events
                    .iter()
                    .map(|event| event.kind.as_str())
                    .collect::<Vec<_>>();
                assert!(
                    kinds.contains(&"execution_blocked"),
                    "missing blocked event: {kinds:?}"
                );
                assert!(
                    kinds.contains(&"review_ready"),
                    "missing review event: {kinds:?}"
                );
                assert!(
                    kinds.contains(&"scheduler_tick"),
                    "missing scheduler event: {kinds:?}"
                );
                inbox.next_cursor.expect("event inbox cursor")
            }
            other => panic!("unexpected event inbox result: {other:?}"),
        };

        let poll = runtime
            .query_runtime(&UiCommand::RunMasterPoll {
                after_cursor: None,
                limit: None,
                include_terminal: true,
                replay_from_start: true,
            })
            .expect("master poll query")
            .expect("master poll result");
        let persisted_cursor = match poll {
            UiQueryResult::MasterPoll(poll) => {
                assert!(poll.task_board.include_terminal);
                assert_eq!(poll.next_cursor.as_deref(), Some(inbox_cursor.as_str()));
                assert_eq!(
                    poll.persisted_cursor.as_deref(),
                    Some(inbox_cursor.as_str())
                );
                let kinds = poll
                    .classifications
                    .iter()
                    .map(|classification| classification.kind.as_str())
                    .collect::<Vec<_>>();
                assert!(kinds.contains(&"blocked"), "missing blocked: {kinds:?}");
                assert!(
                    kinds.contains(&"review_ready"),
                    "missing review_ready: {kinds:?}"
                );
                assert!(kinds.contains(&"stale"), "missing stale: {kinds:?}");
                poll.persisted_cursor.expect("persisted cursor")
            }
            other => panic!("unexpected master poll result: {other:?}"),
        };

        let after_poll = runtime
            .query_runtime(&UiCommand::QueryTaskBoard {
                status: None,
                agent_id: None,
                include_terminal: true,
            })
            .expect("after poll board query")
            .expect("after poll board");
        match after_poll {
            UiQueryResult::TaskBoard(board) => {
                let task = board
                    .tasks
                    .iter()
                    .find(|task| task.task_id == task_id)
                    .expect("task after poll");
                assert_eq!(task.status, before_status);
                assert_eq!(task.status, "review_submitted");
                assert_eq!(
                    task.active_execution_id.as_deref(),
                    Some(execution_id.as_str())
                );
            }
            other => panic!("unexpected after poll result: {other:?}"),
        }

        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::RunMasterPoll {
                    after_cursor: None,
                    limit: None,
                    include_terminal: true,
                    replay_from_start: true,
                })
                .expect("master poll envelope"),
            )
            .expect("master poll receipt");
        assert!(receipt.dispatch_status.starts_with("master_poll_recorded:"));

        let recovered = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("recovered runtime");
        let recovered_poll = recovered
            .query_runtime(&UiCommand::RunMasterPoll {
                after_cursor: None,
                limit: None,
                include_terminal: true,
                replay_from_start: false,
            })
            .expect("recovered master poll query")
            .expect("recovered master poll");
        match recovered_poll {
            UiQueryResult::MasterPoll(poll) => {
                assert_eq!(
                    poll.source_cursor.as_deref(),
                    Some(persisted_cursor.as_str())
                );
                assert_eq!(
                    poll.persisted_cursor.as_deref(),
                    Some(persisted_cursor.as_str())
                );
                assert!(poll.event_inbox.events.is_empty());
            }
            other => panic!("unexpected recovered master poll result: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_dispatches_worker_control_to_task_owner() {
        let runtime_home = temp_runtime_home();
        let worker_id = AgentId::new("runtime-phase2c-worker");
        let task_id = "runtime-phase2c-task".to_owned();
        let execution_id = "runtime-phase2c-exec".to_owned();
        let turn_id = TurnId::new("runtime-phase2c-turn");
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");

        for command in [
            UiCommand::CreateTaskAgent {
                agent: UiTaskAgentCreateCommand {
                    agent_id: worker_id.clone(),
                    capabilities: vec!["code_edit".to_owned(), "test_run".to_owned()],
                },
            },
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some(task_id.clone()),
                    title: "Runtime phase2c task".to_owned(),
                    content: "Runtime phase2c content".to_owned(),
                    goal: "prove worker control bridge".to_owned(),
                    deliverables: vec!["worker control".to_owned()],
                    acceptance: vec!["control events persist".to_owned()],
                    priority: 97,
                    target_cwd: None,
                    session_id: Some(SessionId::new("runtime-phase2c-session")),
                    turn_id: Some(turn_id.clone()),
                    dispatch: Some(UiTaskDispatchCommand::None),
                },
            },
            UiCommand::AssignTask {
                assignment: UiTaskAssignCommand {
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                },
            },
            UiCommand::ClaimNextTask {
                claim: UiTaskClaimCommand {
                    agent_id: worker_id.clone(),
                    execution_id: execution_id.clone(),
                    ttl_seconds: Some(300),
                },
            },
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: Some(turn_id),
                    kind: UiExecutionFactKind::Running {
                        phase: "phase2c_running".to_owned(),
                        summary: "worker running before safe-point control".to_owned(),
                        evidence: vec!["running evidence".to_owned()],
                    },
                },
            },
        ] {
            runtime
                .dispatch(
                    build_command_dispatch_envelope(&command).expect("phase2c setup envelope"),
                )
                .expect("phase2c setup dispatch");
        }

        let controls = [
            (
                "cli-phase2c-query",
                "wctl-phase2c-query",
                "query_status",
                None,
                None,
            ),
            (
                "cli-phase2c-ask",
                "wctl-phase2c-ask",
                "ask_at_safe_point",
                Some("what is blocking the execution?".to_owned()),
                None,
            ),
            (
                "cli-phase2c-constraint",
                "wctl-phase2c-constraint",
                "add_constraint",
                None,
                Some("do not leave the task without a checkpoint".to_owned()),
            ),
            (
                "cli-phase2c-pause",
                "wctl-phase2c-pause",
                "pause",
                None,
                None,
            ),
            (
                "cli-phase2c-resume",
                "wctl-phase2c-resume",
                "resume",
                None,
                None,
            ),
            (
                "cli-phase2c-cancel",
                "wctl-phase2c-cancel",
                "cancel",
                None,
                None,
            ),
        ];
        for (_request_id, control_id, op, question, constraint) in controls {
            let receipt = runtime
                .dispatch(
                    build_command_dispatch_envelope(&UiCommand::WorkerControl {
                        control: UiWorkerControlCommand {
                            control_id: Some(control_id.to_owned()),
                            task_id: task_id.clone(),
                            execution_id: execution_id.clone(),
                            agent_id: worker_id.clone(),
                            op: op.to_owned(),
                            question,
                            constraint,
                            note: Some("runtime phase2c proof".to_owned()),
                        },
                    })
                    .expect("phase2c worker control envelope"),
                )
                .expect("phase2c worker control dispatch");
            assert!(
                receipt
                    .dispatch_status
                    .starts_with(&format!("worker_control_applied:{op}:{control_id}:")),
                "unexpected receipt {}",
                receipt.dispatch_status
            );
        }

        let control_query = runtime
            .query_runtime(&UiCommand::QueryWorkerControl {
                task_id: task_id.clone(),
                execution_id: execution_id.clone(),
            })
            .expect("worker control query")
            .expect("worker control result");
        match control_query {
            UiQueryResult::WorkerControl(projection) => {
                assert_eq!(projection.source_agent_id, AgentId::new("agent-live"));
                assert_eq!(projection.events.len(), 6);
                let ids = projection
                    .events
                    .iter()
                    .map(|event| event.control_id.as_str())
                    .collect::<Vec<_>>();
                for required in [
                    "wctl-phase2c-query",
                    "wctl-phase2c-ask",
                    "wctl-phase2c-constraint",
                    "wctl-phase2c-pause",
                    "wctl-phase2c-resume",
                    "wctl-phase2c-cancel",
                ] {
                    assert!(ids.contains(&required), "missing {required}: {ids:?}");
                }
                assert_eq!(
                    projection.event.as_ref().map(|event| event.op.as_str()),
                    Some("cancel")
                );
            }
            other => panic!("unexpected worker control result: {other:?}"),
        }

        let board = runtime
            .query_runtime(&UiCommand::QueryTaskBoard {
                status: None,
                agent_id: None,
                include_terminal: true,
            })
            .expect("phase2c board query")
            .expect("phase2c board result");
        match board {
            UiQueryResult::TaskBoard(board) => {
                let task = board
                    .tasks
                    .iter()
                    .find(|task| task.task_id == task_id)
                    .expect("phase2c task");
                assert_eq!(task.status, "cancelled");
                assert_eq!(
                    task.active_execution_id.as_deref(),
                    Some(execution_id.as_str())
                );
            }
            other => panic!("unexpected phase2c board result: {other:?}"),
        }

        let history = runtime
            .query_runtime(&UiCommand::QueryTaskHistory {
                task_id: task_id.clone(),
            })
            .expect("phase2c history query")
            .expect("phase2c history result");
        match history {
            UiQueryResult::TaskHistory(history) => {
                let event_types = history
                    .events
                    .iter()
                    .map(|event| event.event_type.as_str())
                    .collect::<Vec<_>>();
                for required in ["TaskPaused", "TaskResumed", "TaskCancelled"] {
                    assert!(
                        event_types.contains(&required),
                        "missing {required}: {event_types:?}"
                    );
                }
            }
            other => panic!("unexpected phase2c history result: {other:?}"),
        }

        let recovered = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("recovered runtime");
        let recovered_control = recovered
            .query_runtime(&UiCommand::QueryWorkerControl {
                task_id: task_id.clone(),
                execution_id: execution_id.clone(),
            })
            .expect("recovered worker control query")
            .expect("recovered worker control result");
        match recovered_control {
            UiQueryResult::WorkerControl(projection) => {
                assert_eq!(projection.events.len(), 6);
                assert!(projection.events.iter().any(|event| {
                    event.control_id == "wctl-phase2c-cancel" && event.op == "cancel"
                }));
            }
            other => panic!("unexpected recovered phase2c result: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_worker_control_invalid_target_returns_explicit_failure() {
        let runtime_home = temp_runtime_home();
        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");

        let err = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::WorkerControl {
                    control: UiWorkerControlCommand {
                        control_id: Some("wctl-phase2c-missing".to_owned()),
                        task_id: "missing-phase2c-task".to_owned(),
                        execution_id: "missing-phase2c-exec".to_owned(),
                        agent_id: AgentId::new("missing-phase2c-worker"),
                        op: "query_status".to_owned(),
                        question: None,
                        constraint: None,
                        note: None,
                    },
                })
                .expect("worker control envelope"),
            )
            .expect_err("missing target must fail");
        assert_eq!(
            err,
            UiCommandDispatchPortError::TargetNotFound("missing-phase2c-task".to_owned())
        );

        let query_err = runtime
            .query_runtime(&UiCommand::QueryWorkerControl {
                task_id: "missing-phase2c-task".to_owned(),
                execution_id: "missing-phase2c-exec".to_owned(),
            })
            .expect_err("missing worker-control query must fail");
        assert_eq!(
            query_err,
            UiCommandDispatchPortError::TargetNotFound("missing-phase2c-task".to_owned())
        );

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_query_reads_error_center_metadata_without_raw_text() {
        let runtime_home = temp_runtime_home();
        let session_id = SessionId::new("runtime-session-agent-live");
        let trace_id = TraceId::new("runtime-trace-error-query");
        let turn_id = TurnId::new("runtime-turn-error-query");
        let ledger_path =
            metadata_ledger_path(&runtime_home, &AgentId::new("agent-live"), &session_id);
        let mut center = MetadataCenter::with_ledger_path(&ledger_path).expect("metadata center");
        center
            .write(
                MetadataEnvelope::new(
                    MetadataId::new("error.center:runtime-trace-error-query:schema"),
                    MetadataKind::RuntimeState,
                    MetadataWriteOwner {
                        feature_id: FeatureId::new("error.center"),
                        crate_name: "freehand-control".to_owned(),
                        module_path: "freehand_control".to_owned(),
                        symbol_path: "classify_error_center_failure".to_owned(),
                    },
                    MetadataWriteNode {
                        pipeline_node: "ReasonResp04CompletionSchemaRejected".to_owned(),
                        runtime_node_id: None,
                    },
                    MetadataSubject {
                        agent_id: Some(AgentId::new("agent-live")),
                        session_id: Some(session_id.clone()),
                        turn_id: Some(turn_id.clone()),
                        trace_id: trace_id.clone(),
                    },
                    vec![
                        MetadataEntry {
                            key: "error.domain".to_owned(),
                            value: json!("schema"),
                        },
                        MetadataEntry {
                            key: "error.class".to_owned(),
                            value: json!("validation"),
                        },
                        MetadataEntry {
                            key: "error.code".to_owned(),
                            value: json!("completion_schema_rejected"),
                        },
                        MetadataEntry {
                            key: "error.source_owner".to_owned(),
                            value: json!("provider.reason-live-bridge"),
                        },
                        MetadataEntry {
                            key: "error.source_pipeline_node".to_owned(),
                            value: json!("ReasonResp04CompletionSchemaRejected"),
                        },
                        MetadataEntry {
                            key: "error.recovery_action".to_owned(),
                            value: json!("repair_schema"),
                        },
                        MetadataEntry {
                            key: "error.retry_index".to_owned(),
                            value: json!(1),
                        },
                        MetadataEntry {
                            key: "error.retry_cap".to_owned(),
                            value: json!(2),
                        },
                        MetadataEntry {
                            key: "error.public_visibility".to_owned(),
                            value: json!("internal"),
                        },
                        MetadataEntry {
                            key: "error.owner_target".to_owned(),
                            value: json!("reason.turn"),
                        },
                        MetadataEntry {
                            key: "error.repair_fields".to_owned(),
                            value: json!(["summary"]),
                        },
                        MetadataEntry {
                            key: "error.raw_hash".to_owned(),
                            value: json!("hash-only"),
                        },
                    ],
                )
                .expect("error center envelope"),
            )
            .expect("write error center metadata");
        center
            .write(
                MetadataEnvelope::new(
                    MetadataId::new("control.center:runtime-trace-error-query:ignored"),
                    MetadataKind::RuntimeState,
                    MetadataWriteOwner {
                        feature_id: FeatureId::new("control.center"),
                        crate_name: "freehand-control".to_owned(),
                        module_path: "freehand_control".to_owned(),
                        symbol_path: "control_status_rhythm_decision".to_owned(),
                    },
                    MetadataWriteNode {
                        pipeline_node: "ControlHook03AfterModelResponse".to_owned(),
                        runtime_node_id: None,
                    },
                    MetadataSubject {
                        agent_id: Some(AgentId::new("agent-live")),
                        session_id: Some(session_id.clone()),
                        turn_id: Some(turn_id.clone()),
                        trace_id: trace_id.clone(),
                    },
                    vec![MetadataEntry {
                        key: "control.hook".to_owned(),
                        value: json!("ControlHook03AfterModelResponse"),
                    }],
                )
                .expect("control envelope"),
            )
            .expect("write control metadata");

        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");

        let result = runtime
            .query_runtime(&UiCommand::QueryErrorCenterEvents {
                session_id: session_id.clone(),
                trace_id: Some(trace_id.as_str().to_owned()),
                turn_id: Some(turn_id.clone()),
                domain: Some("schema".to_owned()),
            })
            .expect("error center query")
            .expect("runtime-backed error center result");
        match result {
            UiQueryResult::ErrorCenterEvents(list) => {
                assert_eq!(list.session_id, session_id);
                assert_eq!(list.events.len(), 1);
                let event = &list.events[0];
                assert_eq!(event.domain, "schema");
                assert_eq!(event.class, "validation");
                assert_eq!(event.recovery_action, "repair_schema");
                assert_eq!(event.raw_hash, "hash-only");
                assert_eq!(event.repair_fields, vec!["summary".to_owned()]);
                assert!(
                    !serde_json::to_string(event)
                        .expect("json")
                        .contains("raw provider body")
                );
            }
            other => panic!("unexpected error center result: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn runtime_task_tool_mutation_publishes_task_list_projection() {
        let runtime_home = temp_runtime_home();
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_named_response(
                    "toolu_task_create_1",
                    "task",
                    json!({
                        "op":"create",
                        "task_id":"runtime-push-task-1",
                        "title":"Runtime push task",
                        "content":"Task list push content",
                        "goal":"Publish task projection",
                        "deliverables":["task projection"],
                        "acceptance":["subscriber sees task"],
                        "dispatch":{"mode":"none"},
                        "priority":77
                    }),
                ),
                complete_single_response("task push done"),
            ],
        );
        let request = LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            session_id: SessionId::new("runtime-task-push-session"),
            turn_id: TurnId::new("runtime-turn-task-push-1"),
            trace_id: TraceId::new("runtime-trace-task-push-1"),
            prompt: "create a task".to_owned(),
            cwd: None,
            stream: false,
            cancel_token: None,
        };
        let mut task_projections = Vec::<UiTaskListProjection>::new();

        let outcome = run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
            |_| {},
            |_| {},
            |projection| task_projections.push(projection.clone()),
        )
        .expect("live bridge");
        let _ = rx.recv().expect("first provider request");
        let _ = rx.recv().expect("second provider request");
        handle.join().expect("join provider");

        assert_eq!(outcome.tool_executions, 1);
        assert_eq!(task_projections.len(), 1);
        assert_eq!(task_projections[0].tasks.len(), 1);
        assert_eq!(task_projections[0].tasks[0].task_id, "runtime-push-task-1");
        assert_eq!(task_projections[0].tasks[0].status, "waiting_agent");
        assert_eq!(task_projections[0].tasks[0].priority, 77);

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn bootstrap_with_corrupt_checkpoint_ledger_fails_explicitly() {
        let runtime_home = temp_runtime_home();
        let session_id = SessionId::new("runtime-session-agent-live");
        let ledger_dir = runtime_home
            .join("ledgers")
            .join("checkpoints")
            .join("agent-live");
        fs::create_dir_all(&ledger_dir).expect("create ledger dir");
        fs::write(
            ledger_dir.join(format!("{}.jsonl", session_id.as_str())),
            "{not-json}\n",
        )
        .expect("write corrupt ledger");

        let err = match RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home,
            false,
        ) {
            Ok(_) => panic!("bootstrap must fail"),
            Err(err) => err,
        };
        match err {
            RuntimeCommandDispatcherError::CheckpointProjectionBootstrap(message) => {
                assert!(message.contains("checkpoint ledger line 1 failed to parse"));
            }
            other => panic!("unexpected bootstrap error: {other:?}"),
        }
    }

    #[test]
    fn resume_turn_is_explicitly_unsupported() {
        let runtime = runtime();
        let err = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::ResumeTurn {
                    turn_id: TurnId::new("runtime-turn-1"),
                })
                .expect("envelope"),
            )
            .expect_err("resume should fail");
        assert_eq!(
            err,
            UiCommandDispatchPortError::Unsupported(
                "resume dispatch for `runtime-turn-1` is not implemented".to_owned()
            )
        );
    }

    #[test]
    fn bootstrap_from_selected_master_agent_uses_selected_runtime_truth() {
        let runtime = RuntimeCommandDispatcher::from_selected_agent(&selected_master_agent())
            .expect("runtime");

        let ui_state = runtime.ui_state();
        let node_status = ui_state
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryNodeStatus {
                node_id: "worker-node".to_owned(),
            })
            .expect("query");
        match node_status {
            UiQueryResult::NodeStatus(Some(snapshot)) => {
                assert_eq!(snapshot.node_id, "worker-node");
                assert_eq!(snapshot.pairing_state, "paired");
            }
            other => panic!("unexpected node status query: {other:?}"),
        }
    }

    #[test]
    fn bootstrap_from_selected_live_agent_wires_node_metadata_into_shared_ledger() {
        let runtime_home = temp_runtime_home();
        let _runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");

        let metadata_path = metadata_ledger_path(
            &runtime_home,
            &AgentId::new("agent-live"),
            &SessionId::new("runtime-session-agent-live"),
        );
        let raw = fs::read_to_string(&metadata_path).expect("read metadata ledger");
        let records: Vec<MetadataEnvelope> = raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("decode metadata"))
            .collect();

        assert!(records.iter().any(|record| {
            record.owner.feature_id == FeatureId::new("node.master-slave")
                && record.write_node.pipeline_node == "NodeReq01BootstrapListening"
        }));
        assert!(records.iter().any(|record| {
            record.owner.feature_id == FeatureId::new("node.master-slave")
                && record.write_node.pipeline_node == "NodeReq02PairingAccepted"
        }));
        assert!(!raw.contains("pair-token"));

        let _ = fs::remove_dir_all(&runtime_home);
    }

    #[test]
    fn bootstrap_rejects_unwritable_node_metadata_ledger_explicitly() {
        let runtime_home = temp_runtime_home();
        let metadata_path = metadata_ledger_path(
            &runtime_home,
            &AgentId::new("agent-live"),
            &SessionId::new("runtime-session-agent-live"),
        );
        fs::create_dir_all(&metadata_path).expect("poison metadata path as directory");

        let err = match RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        ) {
            Ok(_) => panic!("bootstrap must fail"),
            Err(err) => err,
        };

        match err {
            RuntimeCommandDispatcherError::NodeRuntimeInit(message) => {
                assert!(message.contains("metadata write failed"));
                assert!(message.contains("metadata ledger io failed"));
            }
            other => panic!("unexpected bootstrap error: {other:?}"),
        }

        let _ = fs::remove_dir_all(&runtime_home);
    }

    #[test]
    fn bootstrap_rejects_slave_mode_agent_for_ui_host() {
        let mut selected = selected_master_agent();
        selected.mode = AgentMode::Slave;
        let err = match RuntimeCommandDispatcher::from_selected_agent(&selected) {
            Ok(_) => panic!("slave-mode agent must be rejected"),
            Err(err) => err,
        };
        assert_eq!(
            err,
            RuntimeCommandDispatcherError::HostRequiresMasterMode {
                agent_name: "master".to_owned(),
                mode: "slave".to_owned(),
            }
        );
    }
}
