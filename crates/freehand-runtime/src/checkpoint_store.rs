use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_contracts::{AgentId, SessionId, ToolPreviewChangeKind, ToolPreviewContract, TurnId};
use freehand_reason::TurnRecord;
use freehand_ui_protocol::UiCheckpointSummary;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{now_unix_seconds, sanitize_identifier};

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
pub(crate) struct RuntimeCheckpointManifest {
    pub(crate) checkpoint_id: String,
    pub(crate) agent_id: String,
    pub(crate) session_id: String,
    pub(crate) turn_id: String,
    pub(crate) tool_call_id: String,
    pub(crate) workspace_root: String,
    pub(crate) entries: Vec<RuntimeCheckpointEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RuntimeCheckpointEntry {
    pub(crate) locked_path: String,
    pub(crate) kind: ToolPreviewChangeKind,
    pub(crate) blob_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RuntimeCheckpointLedgerRow {
    pub(crate) event: RuntimeCheckpointLedgerEvent,
    pub(crate) checkpoint_id: String,
    pub(crate) turn_id: String,
    pub(crate) tool_call_id: String,
    pub(crate) changed_paths: Vec<String>,
    pub(crate) detail: Option<String>,
    pub(crate) unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RuntimeCheckpointLedgerEvent {
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
pub(crate) struct RuntimeCheckpointStore {
    workspace_root: PathBuf,
    manifests_dir: PathBuf,
    ledger_path: PathBuf,
    agent_id: AgentId,
    session_id: SessionId,
}

impl RuntimeCheckpointStore {
    pub(crate) fn new(
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

    pub(crate) fn new_with_workspace_root(
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

    pub(crate) fn create_from_preview(
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

    pub(crate) fn mark_applied(
        &self,
        manifest: &RuntimeCheckpointManifest,
    ) -> Result<(), RuntimeCheckpointError> {
        self.append_outcome_row(manifest, RuntimeCheckpointLedgerEvent::Applied, None)
    }

    pub(crate) fn mark_failed(
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

    pub(crate) fn load_manifest(
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

pub(crate) fn checkpoint_summary_to_ui(summary: RuntimeCheckpointSummary) -> UiCheckpointSummary {
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
