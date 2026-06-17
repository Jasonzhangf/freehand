//! Runtime wiring owner for UI command dispatch.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_blocks::{
    CompletionDecision, CompletionSchemaRejection, completion_schema_guidance,
    completion_schema_rejection_feedback, parse_completion_submission_block,
    strip_completion_submission_block, validate_completion_submission,
};
use freehand_config::{
    AgentMode, ProviderProtocol as ConfigProviderProtocol, ProviderType, SelectedAgentConfig,
    default_config_path, load_default_config,
};
use freehand_contracts::{
    AgentId, ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
    ContextSegmentKind, ContextStability, FeatureId, ReasonReq04ToolCall,
    ReasonReq05ToolResultReentry, SessionId, ToolPreviewChangeKind, ToolPreviewContract,
    ToolResultContract, TraceId, TurnId,
};
use freehand_debug::{DebugEvent, DebugHub};
use freehand_node::{
    LocalNodeRuntime, MasterNodeConfig, NodeRuntimeError, PairingRequest, PairingTransport,
    SlaveNodeConfig,
};
use freehand_provider_anthropic::{
    AnthropicAdapterConfig, AnthropicExecutor, AnthropicExecutorConfig, AnthropicExecutorError,
};
use freehand_provider_core::{
    ProviderCapabilities, ProviderDescriptor, ProviderFamily, ProviderProtocol,
    ProviderSemanticOutput, ProviderToolExchange, build_semantic_request,
};
use freehand_reason::{
    ReasonBroadcastEvent, ReasonPersistence, ReasonPersistenceError, ReasonTurnEngine,
    SessionHistory, TurnRecord, TurnStartInput,
};
use freehand_tools::BuiltinToolRegistry;
use freehand_ui_protocol::{
    TurnProjectionInput, UiCheckpointSummary, UiClientKind, UiCommand, UiCommandDispatchEnvelope,
    UiCommandDispatchPort, UiCommandDispatchPortError, UiCommandDispatchReceipt, UiProtocolState,
    UiTurnProjection, checkpoint_projection_from_runtime_summary, turn_projection_for_client,
    turn_projection_from_events,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveReasonTurnRequest {
    pub runtime_home: PathBuf,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub prompt: String,
    pub stream: bool,
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
    #[error("anthropic live executor failed: {0}")]
    AnthropicExecutorFailed(String),
    #[error("reason persistence failed: {0}")]
    ReasonPersistenceFailed(String),
    #[error("writable tool checkpoint failed: {0}")]
    ToolCheckpointFailed(String),
    #[error("live tool execution failed: {0}")]
    ToolExecutionFailed(String),
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
        let workspace_root = env::current_dir()
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
    let engine = ReasonTurnEngine::with_debug_hub(Arc::clone(&debug_hub));
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
    let mut turns = Vec::new();
    let mut round = 0usize;
    let mut tool_executions = 0usize;
    let mut next_prompt = request.prompt.clone();
    let mut carryover_segments = vec![
        completion_contract_segment(),
        tool_guidance_segment(),
        original_task_segment(&request.prompt),
    ];
    let mut tool_exchanges: Vec<ProviderToolExchange> = Vec::new();
    let mut executed_tool_call_ids = Vec::<String>::new();
    let tool_registry = BuiltinToolRegistry::reasonix_aligned();

    loop {
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
                    model: selected.provider.default_model.clone(),
                },
            )
            .map_err(|err| RuntimeLiveBridgeError::TurnStartFailed(err.to_string()))?;
        persistence
            .record_turn_started(&history, &turn, schema_rejections.len() as u32)
            .map_err(|err| RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string()))?;
        drain_debug_events(&debug_receiver, &mut on_debug);

        let mut semantic_request = build_semantic_request(
            provider_descriptor(selected),
            turn.provider_payload.clone(),
            false,
        )
        .map_err(|err| RuntimeLiveBridgeError::ProviderRequestBuildFailed(err.to_string()))?;
        semantic_request.tools = tool_registry.implemented_definitions();
        semantic_request.tool_choice = None;
        semantic_request.tool_exchanges = tool_exchanges.clone();

        if request.stream {
            let mut stream_persistence_error = None::<RuntimeLiveBridgeError>;
            executor
                .execute_stream_with(&provider_ctx(&turn), &semantic_request, |batch| {
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
                        stream_persistence_error = Some(err);
                        return Err(AnthropicExecutorError::Callback(
                            "live bridge failed while persisting stream output".to_owned(),
                        ));
                    }
                    Ok(())
                })
                .map_err(map_anthropic_executor_error)?;
            if let Some(err) = stream_persistence_error {
                return Err(err);
            }
        } else {
            let outputs = executor
                .execute_once(&provider_ctx(&turn), &semantic_request)
                .map_err(map_anthropic_executor_error)?;
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
        drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);

        let completed_tool_calls = pending_completed_tool_calls(&turn, &executed_tool_call_ids);
        if !completed_tool_calls.is_empty() {
            for tool_call in completed_tool_calls {
                let tool_result = execute_registry_tool_call(
                    &tool_registry,
                    &request.runtime_home,
                    &turn,
                    &tool_call,
                )?;
                let output = ProviderSemanticOutput::ToolResultReentry(tool_result.clone());
                engine.apply_provider_output(&mut turn, output.clone());
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
                drain_debug_events(&debug_receiver, &mut on_debug);
                executed_tool_call_ids.push(tool_call.tool_call.tool_call_id.as_str().to_owned());
                tool_exchanges.push(ProviderToolExchange {
                    tool_call,
                    tool_result,
                });
                tool_executions = tool_executions.saturating_add(1);
            }
            next_prompt = "The tool result has been returned. Use it to continue the task, then provide the required Freehand completion schema when done.".to_owned();
            carryover_segments =
                next_round_segments(&request.prompt, &collect_turn_text(&turn), None);
            turns.push(turn);
            continue;
        }

        let provider_text = collect_turn_text(&turn);
        let visible_text = strip_completion_submission_block(&provider_text);
        match parse_completion_submission_block(&provider_text) {
            Ok(submission) => match validate_completion_submission(&submission)
                .expect("completion submission already validated")
            {
                CompletionDecision::Completed { .. } | CompletionDecision::Blocked { .. } => {
                    let _ = engine
                        .submit_completion(&mut turn, &submission)
                        .map_err(|err| RuntimeLiveBridgeError::TurnStartFailed(err.to_string()))?;
                    drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
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
                    next_prompt = next_step;
                    carryover_segments = next_round_segments(&request.prompt, &visible_text, None);
                    turns.push(turn);
                }
            },
            Err(rejection) => {
                let feedback = completion_schema_rejection_feedback(&rejection);
                schema_rejections.push(rejection.clone());
                persistence
                    .record_completion_rejected(
                        &history,
                        &turn,
                        &rejection,
                        schema_rejections.len() as u32,
                    )
                    .map_err(|err| {
                        RuntimeLiveBridgeError::ReasonPersistenceFailed(err.to_string())
                    })?;
                if schema_rejections.len() >= 3 {
                    engine.fail_turn(
                        &mut turn,
                        format!(
                            "Failed after 3 invalid completion schema retries.\n{}",
                            feedback
                        ),
                    );
                    drain_broadcasts(&receiver, &mut broadcasts, &mut on_broadcast);
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
    node_runtime: LocalNodeRuntime,
    next_turn_ordinal: u64,
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

        let mut node_runtime = LocalNodeRuntime::new(
            MasterNodeConfig {
                node_id: config.master_node_id.clone(),
                agent_id: config.master_agent_id.clone(),
                paired_slave_node_id: config.slave_node_id.clone(),
            },
            SlaveNodeConfig {
                node_id: config.slave_node_id.clone(),
                agent_id: config.slave_agent_id.clone(),
                paired_master_node_id: config.master_node_id.clone(),
                pair_token: config.pair_token.clone(),
                allowed_pair_ip: config.allowed_pair_ip.clone(),
            },
        )
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
            match persistence.restore(&config.session_id) {
                Ok(restored) => {
                    session_history = restored.history;
                    turns = restored.closed_turns;
                    if let Some(active) = restored.active_turn {
                        turns.push(active.turn);
                    }
                    turns.sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
                    next_turn_ordinal = turns
                        .iter()
                        .map(|turn| runtime_turn_position(&turn.request.turn_id))
                        .map(|(ordinal, _round, _raw)| ordinal)
                        .max()
                        .unwrap_or(0);
                    let mut ui = ui_state.lock().expect("lock ui state");
                    for turn in &turns {
                        ui.apply_turn_projection(project_runtime_turn(
                            &config.reason_agent_id,
                            &config.master_node_id,
                            turn,
                        ));
                    }
                }
                Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => {}
                Err(err) => {
                    return Err(RuntimeCommandDispatcherError::ReasonPersistenceBootstrap(
                        err.to_string(),
                    ));
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
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        state.next_turn_ordinal += 1;
        let turn_id = TurnId::new(format!("runtime-turn-{}", state.next_turn_ordinal));
        let trace_id = TraceId::new(format!("runtime-trace-{}", state.next_turn_ordinal));
        if let Some(live) = &state.config.live {
            let ui_state = Arc::clone(&self.ui_state);
            let reason_agent_id = state.config.reason_agent_id.clone();
            let master_node_id = state.config.master_node_id.clone();
            let outcome = run_live_reason_turn_with_hooks(
                &live.selected_agent,
                LiveReasonTurnRequest {
                    runtime_home: live.runtime_home.clone(),
                    session_id: state.config.session_id.clone(),
                    turn_id,
                    trace_id,
                    prompt: text,
                    stream: live.stream,
                },
                |event| {
                    apply_runtime_reason_broadcast(
                        &ui_state,
                        &reason_agent_id,
                        &master_node_id,
                        event,
                    );
                },
                |event| {
                    apply_runtime_debug_event(&ui_state, event);
                },
            )
            .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
            let projection = project_runtime_turn(
                &state.config.reason_agent_id,
                &state.config.master_node_id,
                &outcome.turn,
            );
            state.turns.extend(outcome.turns);
            self.ui_state
                .lock()
                .expect("lock ui state")
                .apply_turn_projection(projection);
            self.refresh_checkpoint_projection_from_config(&state.config)
                .map_err(map_checkpoint_dispatch_error)?;

            return Ok(UiCommandDispatchReceipt {
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
            });
        }

        let turn = state
            .reason_engine
            .start_turn(
                &mut state.session_history,
                TurnStartInput {
                    session_id: state.config.session_id.clone(),
                    turn_id,
                    trace_id,
                    feature_id: FeatureId::new("reason.turn"),
                    agent_id: state.config.reason_agent_id.clone(),
                    user_text: text,
                    planned_context_segments: Vec::new(),
                    model: state.config.model.clone(),
                },
            )
            .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;

        let projection = project_runtime_turn(
            &state.config.reason_agent_id,
            &state.config.master_node_id,
            &turn,
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

    fn dispatch_cancel_turn(
        &self,
        state: &mut RuntimeCommandDispatcherState,
        envelope: UiCommandDispatchEnvelope,
        turn_id: TurnId,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let turn = state
            .turns
            .iter_mut()
            .find(|turn| turn.request.turn_id == turn_id)
            .ok_or_else(|| {
                UiCommandDispatchPortError::TargetNotFound(turn_id.as_str().to_owned())
            })?;

        state
            .reason_engine
            .fail_turn(turn, "cancelled by ui command");
        let projection = project_runtime_turn(
            &state.config.reason_agent_id,
            &state.config.master_node_id,
            turn,
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

impl UiCommandDispatchPort for RuntimeCommandDispatcher {
    fn dispatch(
        &self,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        let mut state = self.state.lock().expect("lock runtime dispatcher state");
        match envelope.command.clone() {
            UiCommand::SubmitUserInput { text } => {
                self.dispatch_submit_user_input(&mut state, envelope, text)
            }
            UiCommand::CancelTurn { turn_id } => {
                self.dispatch_cancel_turn(&mut state, envelope, turn_id)
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
        NodeRuntimeError::SlaveNotPaired | NodeRuntimeError::UnsupportedTransport => {
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

fn map_checkpoint_dispatch_error(err: RuntimeCheckpointError) -> UiCommandDispatchPortError {
    match err {
        RuntimeCheckpointError::MissingManifest(checkpoint_id) => {
            UiCommandDispatchPortError::TargetNotFound(checkpoint_id)
        }
        other => UiCommandDispatchPortError::DispatchFailed(other.to_string()),
    }
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

fn map_anthropic_executor_error(err: AnthropicExecutorError) -> RuntimeLiveBridgeError {
    RuntimeLiveBridgeError::AnthropicExecutorFailed(err.to_string())
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

fn pending_completed_tool_calls(
    turn: &TurnRecord,
    executed_tool_call_ids: &[String],
) -> Vec<ReasonReq04ToolCall> {
    turn.tool_calls
        .iter()
        .filter(|call| {
            call.tool_call.arguments_complete
                && !executed_tool_call_ids
                    .iter()
                    .any(|id| id == call.tool_call.tool_call_id.as_str())
        })
        .cloned()
        .collect()
}

fn execute_registry_tool_call(
    registry: &BuiltinToolRegistry,
    runtime_home: &Path,
    turn: &TurnRecord,
    tool_call: &ReasonReq04ToolCall,
) -> Result<ReasonReq05ToolResultReentry, RuntimeLiveBridgeError> {
    if !tool_call.tool_call.arguments_complete {
        return Err(RuntimeLiveBridgeError::ToolExecutionFailed(
            "cannot execute incomplete tool arguments".to_owned(),
        ));
    }
    let tool_name = tool_call.tool_call.tool_name.as_str();
    if registry.read_only(tool_name) == Some(false) {
        let store = RuntimeCheckpointStore::new(
            runtime_home,
            &turn.request.agent_id,
            &turn.request.session_id,
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
        let output = match registry.execute(tool_call) {
            Ok(output) => output,
            Err(err) => {
                let _ = store.mark_failed(&manifest, &err.to_string());
                return Err(RuntimeLiveBridgeError::ToolExecutionFailed(err.to_string()));
            }
        };
        store
            .mark_applied(&manifest)
            .map_err(|err| RuntimeLiveBridgeError::ToolCheckpointFailed(err.to_string()))?;
        return Ok(ReasonReq05ToolResultReentry {
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            trace_id: turn.request.trace_id.clone(),
            feature_id: turn.request.feature_id.clone(),
            agent_id: turn.request.agent_id.clone(),
            tool_result: ToolResultContract {
                tool_call_id: tool_call.tool_call.tool_call_id.clone(),
                output: output.text,
            },
        });
    }
    let output = registry
        .execute(tool_call)
        .map_err(|err| RuntimeLiveBridgeError::ToolExecutionFailed(err.to_string()))?;
    Ok(ReasonReq05ToolResultReentry {
        session_id: turn.request.session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: turn.request.feature_id.clone(),
        agent_id: turn.request.agent_id.clone(),
        tool_result: ToolResultContract {
            tool_call_id: tool_call.tool_call.tool_call_id.clone(),
            output: output.text,
        },
    })
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
        ctx.engine.apply_provider_output(turn, output.clone());
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
    F: FnMut(&ReasonBroadcastEvent),
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
        ReasonBroadcastEvent::Usage(event) => {
            ui.apply_usage_event(
                reason_agent_id.clone(),
                master_node_id.to_owned(),
                event,
                false,
            );
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

fn apply_runtime_debug_event(ui_state: &Arc<Mutex<UiProtocolState>>, event: &DebugEvent) {
    let _ = ui_state
        .lock()
        .expect("lock ui state")
        .apply_debug_event(event);
}

fn project_runtime_turn(
    reason_agent_id: &AgentId,
    master_node_id: &str,
    turn: &TurnRecord,
) -> UiTurnProjection {
    turn_projection_for_client(
        turn_projection_from_events(TurnProjectionInput {
            source_agent_id: reason_agent_id.clone(),
            source_node_id: master_node_id.to_owned(),
            session_id: turn.request.session_id.clone(),
            turn_id: turn.request.turn_id.clone(),
            semantic_events: turn.semantic_events.clone(),
            tool_calls: turn.tool_calls.clone(),
            usage_events: turn.usage_events.clone(),
            terminal_event: turn.terminal_event.clone(),
            error_events: turn.error_events.clone(),
            slave_substream_card: false,
        }),
        UiClientKind::WebUi,
    )
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
    use freehand_contracts::{SemanticEventKind, TerminalStatus};
    use freehand_ui_protocol::{UiQueryResult, build_command_dispatch_envelope};
    use serde_json::json;
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
            stream,
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

    fn tool_use_single_response() -> String {
        r#"{"content":[{"type":"tool_use","id":"toolu_read_1","name":"read_file","input":{"path":"Cargo.toml","offset":0,"limit":2}}],"usage":{"input_tokens":20,"output_tokens":16},"stop_reason":"tool_use"}"#.to_owned()
    }

    fn tool_use_write_file_response(path: &str, content: &str) -> String {
        json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_write_1",
                "name": "write_file",
                "input": {
                    "path": path,
                    "content": content
                }
            }],
            "usage": {"input_tokens": 20, "output_tokens": 16},
            "stop_reason": "tool_use"
        })
        .to_string()
    }

    fn tool_use_edit_file_response(path: &str, old_string: &str, new_string: &str) -> String {
        json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_edit_1",
                "name": "edit_file",
                "input": {
                    "path": path,
                    "old_string": old_string,
                    "new_string": new_string
                }
            }],
            "usage": {"input_tokens": 20, "output_tokens": 16},
            "stop_reason": "tool_use"
        })
        .to_string()
    }

    fn tool_use_bash_response(command: &str) -> String {
        json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_bash_1",
                "name": "bash",
                "input": {
                    "command": command
                }
            }],
            "usage": {"input_tokens": 20, "output_tokens": 16},
            "stop_reason": "tool_use"
        })
        .to_string()
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

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(false),
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
    }

    #[test]
    fn live_bridge_runs_streaming_anthropic_provider_into_broadcasts() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, rx, handle) =
            spawn_mock_server(200, "text/event-stream", complete_stream_response("pong"));

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(true),
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

        let outcome = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
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
    }

    #[test]
    fn live_bridge_fails_after_three_invalid_schema_retries() {
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
            Some(TerminalStatus::Failed)
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
    fn live_bridge_rejects_writable_tool_without_preview_support() {
        let _cwd_lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (base_url, _rx, handle) = spawn_sequence_server(
            "application/json",
            vec![tool_use_bash_response("printf 'pong'")],
        );

        let err = run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request(false),
        )
        .expect_err("must fail");
        handle.join().expect("join");

        match err {
            RuntimeLiveBridgeError::ToolCheckpointFailed(message) => {
                assert!(message.contains("bash"));
                assert!(message.contains("preview"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
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
                stream: false,
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
    fn submit_input_dispatches_to_reason_and_updates_ui_state() {
        let runtime = runtime();
        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                    text: "hello runtime".to_owned(),
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
