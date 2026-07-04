//! Runtime wiring owner for UI command dispatch.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_blocks::{
    CompletionClaim, CompletionDecision, CompletionSchemaRejection, CompletionSubmission,
    completion_schema_guidance, completion_schema_rejection_feedback,
    parse_completion_submission_block, strip_completion_submission_block,
    validate_completion_submission,
};
use freehand_config::{
    AgentMode, ProviderProtocol as ConfigProviderProtocol, ProviderType, SelectedAgentConfig,
    default_config_path, load_default_config,
};
use freehand_contracts::{
    AgentId, ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
    ContextSegmentKind, ContextStability, ErrorClass, ErrorContract, ErrorErr01RuntimeClassified,
    FeatureId, ReasonReq04ToolCall, ReasonReq05ToolResultReentry, RecoveryPolicy, SessionId,
    ToolArgument, ToolPreviewChangeKind, ToolPreviewContract, ToolResultContract, ToolResultStatus,
    TraceId, TurnId,
};
use freehand_control::{
    ControlRhythmDecision, ControlStatusRejection, ControlStatusSubmission,
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
    AnthropicRawCapture,
};
use freehand_provider_core::{
    ProviderCapabilities, ProviderDescriptor, ProviderFamily, ProviderProtocol,
    ProviderSemanticOutput, ProviderToolExchange, build_semantic_request,
};
use freehand_reason::{
    PersistedSessionMetadataEntry, ProviderRawLedgerWrite, ProviderRawScenePosition,
    ReasonBroadcastEvent, ReasonPersistence, ReasonPersistenceError,
    ReasonResp04CompletionSchemaRejected, ReasonResp05ModelContinuationWaiting, ReasonTurnEngine,
    SessionHistory, TurnRecord, TurnStartInput,
};
use freehand_task::{
    TaskActor, TaskAppendRequest, TaskCreateRequest, TaskDispatchRequest, TaskId,
    TaskMutationRequest, TaskParentRef, TaskReviewRejection, TaskReviewSubmission, TaskRuntime,
    TaskWatermark,
};
use freehand_tools::{BuiltinToolRegistry, with_workspace_root};
use freehand_ui_protocol::{
    TurnProjectionInput, UiCheckpointSummary, UiClientKind, UiCommand, UiCommandDispatchEnvelope,
    UiCommandDispatchPort, UiCommandDispatchPortError, UiCommandDispatchReceipt,
    UiCompletionSchemaRetryWaiting, UiModelRequestKind, UiModelRequestWaiting, UiProtocolState,
    UiSessionMetadataProjection, UiTurnProjection, checkpoint_projection_from_runtime_summary,
    turn_projection_for_client, turn_projection_from_events,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use thiserror::Error;

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
    #[error("live turn cancelled")]
    Cancelled,
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
            checkpoint_workspace_root()?,
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

fn checkpoint_workspace_root() -> Result<PathBuf, RuntimeCheckpointError> {
    checkpoint_workspace_root_from_env(
        env::var_os("FREEHAND_WORKSPACE_ROOT").or_else(|| env::var_os("FREEHAND_DAEMON_WORKDIR")),
    )
}

fn checkpoint_workspace_root_from_env(
    configured_root: Option<std::ffi::OsString>,
) -> Result<PathBuf, RuntimeCheckpointError> {
    let root = if let Some(path) = configured_root {
        PathBuf::from(path)
    } else {
        env::current_dir()
            .map_err(|err| RuntimeCheckpointError::StoreBootstrapFailed(err.to_string()))?
    };
    fs::canonicalize(root)
        .map_err(|err| RuntimeCheckpointError::StoreBootstrapFailed(err.to_string()))
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
    run_live_reason_turn_with_hooks(selected, request, |_| {}, |_| {})
}

pub fn run_live_reason_turn_with_hooks<FB, FD>(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
    on_broadcast: FB,
    on_debug: FD,
) -> Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
    FD: FnMut(&DebugEvent),
{
    match (selected.provider.provider_type, selected.provider.protocol) {
        (ProviderType::Anthropic, ConfigProviderProtocol::Messages) => {
            run_live_anthropic_reason_turn(selected, request, on_broadcast, on_debug)
        }
        _ => Err(RuntimeLiveBridgeError::UnsupportedLiveProvider {
            provider: selected.provider.provider_type.as_str().to_owned(),
            protocol: selected.provider.protocol.as_str().to_owned(),
        }),
    }
}

fn run_live_anthropic_reason_turn<FB, FD>(
    selected: &SelectedAgentConfig,
    request: LiveReasonTurnRequest,
    mut on_broadcast: FB,
    mut on_debug: FD,
) -> Result<LiveReasonTurnOutcome, RuntimeLiveBridgeError>
where
    FB: FnMut(&ReasonBroadcastEvent),
    FD: FnMut(&DebugEvent),
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
        adapter: AnthropicAdapterConfig { max_tokens: 512 },
    })
    .map_err(map_anthropic_executor_error)?;

    let mut broadcasts = Vec::new();
    let mut schema_rejections = Vec::new();
    let mut consecutive_schema_rejections = 0usize;
    let mut turns = Vec::new();
    let mut round = 0usize;
    let mut tool_executions = 0usize;
    let mut next_prompt = request.prompt.clone();
    let mut carryover_segments = vec![
        completion_contract_segment(),
        control_status_contract_segment(),
        tool_guidance_segment(),
        original_task_segment(&request.prompt),
    ];
    let mut tool_exchanges: Vec<ProviderToolExchange> = Vec::new();
    let mut executed_tool_call_ids = Vec::<String>::new();
    let tool_registry = BuiltinToolRegistry::reasonix_aligned();
    let tool_schema_fingerprint = tool_registry.implemented_schema_fingerprint();

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
        semantic_request.tools = tool_registry.implemented_definitions();
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
                let mapped = map_anthropic_executor_error(err);
                record_provider_error_metadata(
                    &metadata_center,
                    &agent_id,
                    &request.session_id,
                    &turn,
                    &mapped,
                )?;
                emit_provider_error_debug(
                    &debug_hub,
                    &agent_id,
                    &request.session_id,
                    &turn,
                    &mapped,
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
                };
                materialize_provider_executor_failure(&mut failure_ctx, &mut turn, &mapped)?;
                turns.push(turn);
                return Err(mapped);
            }
        } else {
            let single_raw_error = RefCell::new(None::<RuntimeLiveBridgeError>);
            let execute_result =
                executor.execute_once_with_raw(&provider_ctx(&turn), &semantic_request, |raw| {
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
                            "live bridge failed while persisting raw provider response".to_owned(),
                        ));
                    }
                    Ok(())
                });
            if let Some(err) = single_raw_error.into_inner() {
                return Err(err);
            }
            let outputs = match execute_result {
                Ok(o) => o,
                Err(err) => {
                    let mapped = map_anthropic_executor_error(err);
                    record_provider_error_metadata(
                        &metadata_center,
                        &agent_id,
                        &request.session_id,
                        &turn,
                        &mapped,
                    )?;
                    emit_provider_error_debug(
                        &debug_hub,
                        &agent_id,
                        &request.session_id,
                        &turn,
                        &mapped,
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
                    };
                    materialize_provider_executor_failure(&mut failure_ctx, &mut turn, &mapped)?;
                    turns.push(turn);
                    return Err(mapped);
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
            for tool_call in pending_tool_calls {
                ensure_live_not_cancelled(&request)?;
                let tool_result = execute_registry_tool_call(
                    &tool_registry,
                    &request.runtime_home,
                    request.cwd.as_deref(),
                    &turn,
                    &tool_call,
                )?;
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
                executed_tool_call_ids.push(tool_call.tool_call.tool_call_id.as_str().to_owned());
                tool_exchanges.push(ProviderToolExchange {
                    tool_call,
                    tool_result,
                });
                tool_executions = tool_executions.saturating_add(1);
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
            carryover_segments =
                next_round_segments(&request.prompt, &collect_turn_text(&turn), None);
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
        let status_decision = run_control_status_stop_hook(
            &metadata_center,
            &agent_id,
            &request.session_id,
            &turn,
            &provider_text,
        )?;
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
                    consecutive_schema_rejections = 0;
                    next_prompt = next_step;
                    carryover_segments =
                        next_round_segments(&request.prompt, &public_provider_text, None);
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
                    consecutive_schema_rejections = 0;
                    next_prompt = next_step;
                    carryover_segments = next_round_segments(&request.prompt, &visible_text, None);
                    turns.push(turn);
                }
            },
            Err(rejection) => {
                ensure_live_not_cancelled(&request)?;
                let feedback = completion_schema_rejection_feedback(&rejection);
                schema_rejections.push(rejection.clone());
                consecutive_schema_rejections = consecutive_schema_rejections.saturating_add(1);
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
                carryover_segments =
                    next_round_segments(&request.prompt, &visible_text, Some(feedback.as_str()));
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

struct RuntimeCommandDispatcherState {
    config: RuntimeCommandDispatcherConfig,
    reason_engine: ReasonTurnEngine,
    session_history: SessionHistory,
    turns: Vec<TurnRecord>,
    session_cwds: BTreeMap<SessionId, PathBuf>,
    active_turns: Vec<ActiveRuntimeTurn>,
    node_runtime: LocalNodeRuntime,
    next_turn_ordinal: u64,
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
        if agent_name.trim().is_empty() {
            return Err(RuntimeCommandDispatcherError::EmptyAgentName);
        }
        let config = load_default_config()
            .map_err(|err| RuntimeCommandDispatcherError::ConfigLoad(err.to_string()))?;
        let selected = config
            .select_agent(agent_name)
            .map_err(|err| RuntimeCommandDispatcherError::AgentSelection(err.to_string()))?;
        let paired_pair_token = env::var(&selected.paired_pair_token_env).map_err(|_| {
            RuntimeCommandDispatcherError::MissingPairedTokenEnv {
                paired_agent_name: selected.paired_agent_name.clone(),
                env_var: selected.paired_pair_token_env.clone(),
            }
        })?;
        if paired_pair_token.trim().is_empty() {
            return Err(RuntimeCommandDispatcherError::EmptyPairedTokenEnv {
                paired_agent_name: selected.paired_agent_name.clone(),
                env_var: selected.paired_pair_token_env.clone(),
            });
        }
        if paired_pair_token != selected.pair_token {
            return Err(RuntimeCommandDispatcherError::PairTokenMismatch {
                agent_name: selected.name.clone(),
                paired_agent_name: selected.paired_agent_name.clone(),
            });
        }
        let runtime_home = default_config_path()
            .map_err(|err| RuntimeCommandDispatcherError::ConfigLoad(err.to_string()))?
            .parent()
            .ok_or_else(|| {
                RuntimeCommandDispatcherError::ConfigLoad(
                    "default config path has no runtime home parent".to_owned(),
                )
            })?
            .to_path_buf();
        Self::from_selected_agent_with_live(&selected, runtime_home, false)
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

    fn dispatch_submit_user_input(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        text: String,
        requested_session_id: Option<SessionId>,
        requested_cwd: Option<String>,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let session_id = requested_session_id.unwrap_or_else(|| state.config.session_id.clone());
        let cwd = resolve_session_cwd(state, &session_id, requested_cwd)?;
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
        let cwd = resolve_session_cwd(state, &session_id, requested_cwd).ok()?;
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
                state.turns = restored.closed_turns;
                if let Some(active_turn) = restored.active_turn {
                    state.turns.push(active_turn.turn);
                }
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
) -> Result<PathBuf, UiCommandDispatchPortError> {
    let cwd = if let Some(cwd) = requested_cwd {
        canonicalize_session_cwd(&cwd)?
    } else if let Some(existing) = state.session_cwds.get(session_id) {
        existing.clone()
    } else {
        canonicalize_default_runtime_cwd()?
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

fn run_control_status_stop_hook(
    center: &Arc<Mutex<MetadataCenter>>,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn: &TurnRecord,
    provider_text: &str,
) -> Result<Option<ControlRhythmDecision>, RuntimeLiveBridgeError> {
    if !provider_text.contains("<<<freehand_status>>>") {
        return Ok(None);
    }
    let raw_hash = stable_debug_hash(provider_text);
    match parse_control_status_block(provider_text) {
        Ok(submission) => {
            let decision = control_status_rhythm_decision(&submission).map_err(|rejection| {
                RuntimeLiveBridgeError::ProviderRequestBuildFailed(
                    control_status_rejection_summary(&rejection),
                )
            })?;
            record_control_status_metadata(
                center,
                agent_id,
                session_id,
                turn,
                &submission,
                &decision,
                raw_hash,
            )?;
            Ok(Some(decision))
        }
        Err(rejection) => {
            record_control_status_rejection_metadata(
                center, agent_id, session_id, turn, &rejection, raw_hash,
            )?;
            Err(RuntimeLiveBridgeError::ProviderRequestBuildFailed(
                control_status_rejection_summary(&rejection),
            ))
        }
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

fn canonicalize_default_runtime_cwd() -> Result<PathBuf, UiCommandDispatchPortError> {
    let root = env::var_os("FREEHAND_WORKSPACE_ROOT")
        .or_else(|| env::var_os("FREEHAND_DAEMON_WORKDIR"))
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(|| {
            env::current_dir().map_err(|err| {
                UiCommandDispatchPortError::DispatchFailed(format!(
                    "cannot read runtime current working directory: {err}"
                ))
            })
        })?;
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
            | UiCommand::DeleteSession { .. } => {
                self.dispatch_session_management(&mut state, envelope)
            }
            UiCommand::CancelTurn { turn_id } => {
                self.dispatch_cancel_turn(&mut state, envelope, turn_id)
            }
            UiCommand::CancelLatestActiveTurn {} => {
                self.dispatch_cancel_latest_active_turn(&mut state, envelope)
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
    RuntimeLiveBridgeError::AnthropicExecutorFailed(err.to_string())
}

fn record_provider_error_metadata(
    center: &Arc<Mutex<MetadataCenter>>,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn: &TurnRecord,
    error: &RuntimeLiveBridgeError,
) -> Result<(), RuntimeLiveBridgeError> {
    write_live_bridge_metadata(
        center,
        agent_id,
        session_id,
        RuntimeMetadataWriteSpec {
            turn_id: Some(&turn.request.turn_id),
            trace_id: &turn.request.trace_id,
            kind: MetadataKind::Provider,
            pipeline_node: "RuntimeLive05ProviderError",
            metadata_suffix: "provider_error".to_owned(),
            symbol_path: "run_live_anthropic_reason_turn",
            entries: vec![
                MetadataEntry {
                    key: "error.kind".to_owned(),
                    value: json!("executor_failure"),
                },
                MetadataEntry {
                    key: "error.summary".to_owned(),
                    value: json!(error.to_string()),
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
            code: "provider_executor_failure".to_owned(),
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

fn emit_provider_error_debug(
    debug_hub: &DebugHub,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn: &TurnRecord,
    error: &RuntimeLiveBridgeError,
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
            status_text: "provider error occurred",
            detail_lines: vec![format!("error={}", error)],
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
            "    \"simple_request\": true | false,\n",
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

fn tool_guidance_segment() -> ContextSegment {
    ContextSegment {
        segment_id: ContextSegmentId::new("runtime-tool-guidance"),
        kind: ContextSegmentKind::DeveloperPolicy,
        stability: ContextStability::Stable,
        cache_policy: ContextCachePolicy::CacheAnchor,
        role: ContextRole::Developer,
        content: "Use the available Freehand tool registry when it helps the task. Choose the smallest sufficient tool for repository inspection or task bookkeeping, then continue and provide the required Freehand completion schema.".to_owned(),
        token_budget: 160,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some("runtime_tool_guidance".to_owned()),
        },
    }
}

fn original_task_segment(prompt: &str) -> ContextSegment {
    ContextSegment {
        segment_id: ContextSegmentId::new("original-task"),
        kind: ContextSegmentKind::SessionMemory,
        stability: ContextStability::SessionStable,
        cache_policy: ContextCachePolicy::Cacheable,
        role: ContextRole::Developer,
        content: format!("Original operator task:\n{prompt}"),
        token_budget: 128,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some("original_task".to_owned()),
        },
    }
}

fn next_round_segments(
    original_prompt: &str,
    visible_text: &str,
    rejection_feedback: Option<&str>,
) -> Vec<ContextSegment> {
    let mut segments = vec![
        completion_contract_segment(),
        original_task_segment(original_prompt),
    ];
    if !visible_text.trim().is_empty() {
        segments.push(ContextSegment {
            segment_id: ContextSegmentId::new("previous-visible-output"),
            kind: ContextSegmentKind::SubagentConclusion,
            stability: ContextStability::TurnVolatile,
            cache_policy: ContextCachePolicy::NoCache,
            role: ContextRole::Developer,
            content: format!("Previous round visible output:\n{visible_text}"),
            token_budget: 512,
            provenance: ContextProvenance {
                source: "freehand_runtime".to_owned(),
                reference: Some("previous_visible_output".to_owned()),
            },
        });
    }
    if let Some(feedback) = rejection_feedback {
        segments.push(ContextSegment {
            segment_id: ContextSegmentId::new("completion-schema-feedback"),
            kind: ContextSegmentKind::SubagentConclusion,
            stability: ContextStability::TurnVolatile,
            cache_policy: ContextCachePolicy::NoCache,
            role: ContextRole::Developer,
            content: format!("Completion schema rejection feedback:\n{feedback}"),
            token_budget: 1024,
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
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<ReasonReq05ToolResultReentry, RuntimeLiveBridgeError> {
    if !tool_call.tool_call.arguments_complete {
        return Ok(tool_result_reentry(
            turn,
            tool_call,
            ToolResultStatus::Failed,
            "Tool execution failed: cannot execute incomplete tool arguments".to_owned(),
        ));
    }
    if let Some(root) = workspace_root {
        return with_workspace_root(root, || {
            execute_registry_tool_call_with_workspace(registry, runtime_home, root, turn, tool_call)
        })
        .map_err(|err| RuntimeLiveBridgeError::ToolExecutionFailed(err.to_string()))?;
    }
    let root = checkpoint_workspace_root()
        .map_err(|err| RuntimeLiveBridgeError::ToolCheckpointFailed(err.to_string()))?;
    execute_registry_tool_call_with_workspace(registry, runtime_home, &root, turn, tool_call)
}

fn execute_registry_tool_call_with_workspace(
    registry: &BuiltinToolRegistry,
    runtime_home: &Path,
    workspace_root: &Path,
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<ReasonReq05ToolResultReentry, RuntimeLiveBridgeError> {
    let tool_name = tool_call.tool_call.tool_name.as_str();
    if tool_name == "task" {
        let (status, output) = match execute_task_tool(runtime_home, turn, tool_call) {
            Ok(output) => (ToolResultStatus::Success, output),
            Err(err) => (
                ToolResultStatus::Failed,
                format!("Task tool execution failed: {err}"),
            ),
        };
        return Ok(tool_result_reentry(turn, tool_call, status, output));
    }
    if is_checkpointable_file_mutation_tool(tool_name) {
        let store = RuntimeCheckpointStore::new_with_workspace_root(
            runtime_home,
            &turn.request.agent_id,
            &turn.request.session_id,
            workspace_root.to_path_buf(),
        )
        .map_err(|err| RuntimeLiveBridgeError::ToolCheckpointFailed(err.to_string()))?;
        let preview = registry.preview(tool_call).map_err(|err| {
            RuntimeLiveBridgeError::ToolCheckpointFailed(
                RuntimeCheckpointError::UncheckpointableTool {
                    tool: tool_name.to_owned(),
                    message: err.to_string(),
                }
                .to_string(),
            )
        })?;
        let manifest = store
            .create_from_preview(turn, &preview, tool_name)
            .map_err(|err| RuntimeLiveBridgeError::ToolCheckpointFailed(err.to_string()))?;
        let (status, output) = match registry.execute(tool_call) {
            Ok(output) => (ToolResultStatus::Success, output.text),
            Err(err) => {
                let _ = store.mark_failed(&manifest, &err.to_string());
                (
                    ToolResultStatus::Failed,
                    format!("Tool execution failed: {err}"),
                )
            }
        };
        if status == ToolResultStatus::Success {
            store
                .mark_applied(&manifest)
                .map_err(|err| RuntimeLiveBridgeError::ToolCheckpointFailed(err.to_string()))?;
        }
        return Ok(tool_result_reentry(turn, tool_call, status, output));
    }
    let (status, output) = match registry.execute(tool_call) {
        Ok(output) => (ToolResultStatus::Success, output.text),
        Err(err) => (
            ToolResultStatus::Failed,
            format!("Tool execution failed: {err}"),
        ),
    };
    Ok(tool_result_reentry(turn, tool_call, status, output))
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
        other => Err(format!("unsupported task op `{other}`")),
    }
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

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(false),
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
    fn live_bootstrap_restores_multiround_turns_as_separate_ui_cards() {
        let runtime_home = temp_runtime_home();
        with_temp_workspace(|_| {
            let (base_url, rx, handle) = spawn_sequence_server(
                "application/json",
                vec![
                    tool_use_bash_response("printf restored-tool"),
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
                            tool.tool_name == "bash"
                                && tool.status
                                    == freehand_ui_protocol::UiToolActivityStatus::Completed
                        }),
                        "restored first round must retain its own bash activity: {:?}",
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
    fn live_dispatch_projects_schema_retry_feedback_to_client_before_repair_completes() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                invalid_complete_response(),
                complete_single_response("schema repaired"),
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
                    text: "trigger schema repair".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("submit should complete after schema repair");
        assert!(
            receipt
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        let _first_request = rx.recv().expect("first provider request");
        let second_request = rx.recv().expect("schema repair provider request");
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
                    .expect("schema retry round");
                let activity = retry_round
                    .model_request
                    .as_ref()
                    .expect("schema retry must be client-visible");
                assert_eq!(activity.kind, UiModelRequestKind::SchemaRetry);
                let detail = activity.detail.as_deref().expect("schema detail");
                assert!(detail.contains("schema retry #1"));
                assert!(detail.contains("completion_reason is required"));
                assert!(detail.contains("evidence is required"));
                assert!(detail.contains("learned is required"));

                let final_round = transcript
                    .turns
                    .iter()
                    .find(|turn| turn.turn_id == TurnId::new("runtime-turn-1-r2"))
                    .expect("repair final round");
                assert_eq!(final_round.terminal_status, Some(TerminalStatus::Success));
                assert!(final_round.model_request.is_none());
            }
            other => panic!("unexpected transcript query: {other:?}"),
        }

        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    }

    #[test]
    fn live_dispatch_projects_missing_schema_retry_feedback_to_client() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_home = temp_runtime_home();
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                missing_completion_schema_response(),
                complete_single_response("schema repaired"),
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
                    text: "trigger missing schema repair".to_owned(),
                    session_id: None,
                    cwd: None,
                })
                .expect("envelope"),
            )
            .expect("submit should complete after missing schema repair");
        assert!(
            receipt
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        let _first_request = rx.recv().expect("first provider request");
        let second_request = rx.recv().expect("schema repair provider request");
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
                    .expect("schema retry round");
                let activity = retry_round
                    .model_request
                    .as_ref()
                    .expect("schema retry must be client-visible");
                assert_eq!(activity.kind, UiModelRequestKind::SchemaRetry);
                let detail = activity.detail.as_deref().expect("schema detail");
                assert!(detail.contains("schema retry #1"));
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
    fn live_tool_execution_uses_requested_session_cwd() {
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
        assert!(reentry_request.contains("session cwd content"));
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
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            for response_body in response_bodies {
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
                    "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
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
{"schema_version":1,"status":{"simple_request":true}}
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
            .implemented_definitions()
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
    fn live_bridge_stamps_tool_schema_fingerprint_into_planner_diagnostics() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, _rx, handle) =
            spawn_mock_server(200, "application/json", complete_single_response("pong"));

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(false),
        )
        .expect("live bridge");
        handle.join().expect("join");

        let registry = BuiltinToolRegistry::reasonix_aligned();
        let expected = fnv1a_hex_for_test(&registry.implemented_schema_fingerprint());
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
    fn live_bridge_retries_invalid_schema_then_completes() {
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

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(false),
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
        let (base_url, _rx, handle) = spawn_mock_server(
            500,
            "application/json",
            r#"{"error":{"type":"internal_error","message":"server exploded"}}"#.to_string(),
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
            event.error.code == "provider_executor_failure"
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
            let request = live_request(false);
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
            let request = live_request(false);
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
        let err = rewind_checkpoint(
            temp_runtime_home(),
            &AgentId::new("agent-live"),
            &SessionId::new("session-live"),
            "checkpoint-missing",
        )
        .expect_err("missing manifest must fail");

        assert_eq!(
            err,
            RuntimeCheckpointError::MissingManifest("checkpoint-missing".to_owned())
        );
    }

    #[test]
    fn checkpoint_store_uses_daemon_workdir_env_before_current_dir() {
        let workspace_root = temp_runtime_home().join("daemon-workdir");
        fs::create_dir_all(&workspace_root).expect("create workspace root");
        let root = checkpoint_workspace_root_from_env(Some(workspace_root.clone().into()))
            .expect("workspace root");
        assert_eq!(
            root,
            fs::canonicalize(workspace_root).expect("canonical workspace root")
        );
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
            let request = live_request(false);
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
            let runtime_home = temp_runtime_home();
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
