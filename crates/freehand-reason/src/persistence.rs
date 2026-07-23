use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_blocks::CompletionSchemaRejection;
use freehand_contracts::{AgentId, ContextSegment, SessionId, TraceId, TurnId};
use freehand_provider_core::{ProviderFamily, ProviderSemanticOutput};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ReasonTurnEngine, SessionHistory, TurnProjection, TurnRecord};

const PERSISTENCE_SCHEMA_VERSION: u32 = 1;
const PROVIDER_RAW_LEDGER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonPersistenceCursor {
    pub schema_version: u32,
    pub last_applied_reason_seq: u64,
    pub latest_turn_id: Option<TurnId>,
    pub active_turn_id: Option<TurnId>,
}

impl Default for ReasonPersistenceCursor {
    fn default() -> Self {
        Self {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            last_applied_reason_seq: 0,
            latest_turn_id: None,
            active_turn_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveTurnSnapshot {
    pub turn: TurnRecord,
    pub schema_rejections: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasonLedgerPayload {
    TurnStarted {
        snapshot: ActiveTurnSnapshot,
    },
    ProviderOutputApplied {
        output: ProviderSemanticOutput,
        snapshot: ActiveTurnSnapshot,
    },
    CompletionRejected {
        rejection: CompletionSchemaRejection,
        snapshot: ActiveTurnSnapshot,
    },
    TurnClosed {
        turn: TurnRecord,
        schema_rejections: u32,
    },
    SessionRollback {
        marker: SessionRollbackMarker,
    },
    RewriteStateUpdated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonLedgerRow {
    pub schema_version: u32,
    pub seq: u64,
    #[serde(default)]
    pub created_at: u64,
    pub session_id: SessionId,
    pub turn_id: Option<TurnId>,
    pub cursor_after: ReasonPersistenceCursor,
    pub session_history: SessionHistory,
    pub payload: ReasonLedgerPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedSessionView {
    pub agent_id: AgentId,
    pub session_id: SessionId,
    pub latest_turn_id: Option<TurnId>,
    pub active_turn_id: Option<TurnId>,
    pub projections: Vec<TurnProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedSessionIndexEntry {
    pub agent_id: AgentId,
    pub session_id: SessionId,
    pub latest_turn_id: Option<TurnId>,
    pub active_turn_id: Option<TurnId>,
    pub latest_terminal_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedSessionMetadataEntry {
    pub agent_id: AgentId,
    pub session_id: SessionId,
    pub title: Option<String>,
    pub archived: bool,
    pub cwd: Option<String>,
    pub updated_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRollbackMarker {
    pub rollback_id: String,
    pub session_id: SessionId,
    pub target_turn_id: TurnId,
    pub target_logical_turn_key: String,
    pub previous_effective_head: Option<TurnId>,
    pub restored_user_text: String,
    pub writer_owner: String,
    pub reason: String,
    pub updated_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRawLedgerWrite {
    pub provider_family: ProviderFamily,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub raw_kind: String,
    pub scene: ProviderRawScenePosition,
    pub body: String,
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRawScenePosition {
    pub crate_name: String,
    pub file: String,
    pub function: String,
    pub line: Option<u32>,
    pub raw_exchange_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRawLedgerRow {
    pub schema_version: u32,
    pub provider_family: ProviderFamily,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub raw_kind: String,
    pub scene: ProviderRawScenePosition,
    pub body: String,
    pub headers: BTreeMap<String, String>,
    pub captured_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredReasonSession {
    pub history: SessionHistory,
    pub cursor: ReasonPersistenceCursor,
    pub active_turn: Option<ActiveTurnSnapshot>,
    pub closed_turns: Vec<TurnRecord>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReasonPersistenceError {
    #[error("reason persistence file io failed: {0}")]
    FileIoFailed(String),
    #[error("reason persistence json render failed: {0}")]
    JsonRenderFailed(String),
    #[error("reason persistence json parse failed: {0}")]
    JsonParseFailed(String),
    #[error("persisted cursor is inconsistent: {0}")]
    InvalidCursorCoherence(String),
    #[error("reason ledger row is inconsistent: {0}")]
    InvalidLedgerCoherence(String),
    #[error("reason ledger sequence is invalid: expected {expected}, got {actual}")]
    LedgerSequenceGap { expected: u64, actual: u64 },
    #[error("no authoritative snapshot or reason ledger exists for session `{0}`")]
    MissingRecoveryTruth(String),
    #[error("session metadata mutation target was not found: {0}")]
    SessionMetadataTargetNotFound(String),
    #[error("session metadata is invalid: {0}")]
    InvalidSessionMetadata(String),
    #[error("session rollback target was not found: {0}")]
    SessionRollbackTargetNotFound(String),
    #[error("session rollback cannot run while an active turn exists: {0}")]
    SessionRollbackActiveTurn(String),
}

pub struct ReasonPersistence {
    runtime_home: PathBuf,
    agent_id: AgentId,
}

impl ReasonPersistence {
    pub fn new(runtime_home: impl Into<PathBuf>, agent_id: AgentId) -> Self {
        Self {
            runtime_home: runtime_home.into(),
            agent_id,
        }
    }

    pub fn runtime_home(&self) -> &Path {
        &self.runtime_home
    }

    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    pub fn record_turn_started(
        &self,
        history: &SessionHistory,
        turn: &TurnRecord,
        schema_rejections: u32,
    ) -> Result<ReasonPersistenceCursor, ReasonPersistenceError> {
        let snapshot = ActiveTurnSnapshot {
            turn: turn.clone(),
            schema_rejections,
        };
        self.persist_row(
            history,
            Some(turn.request.turn_id.clone()),
            ReasonLedgerPayload::TurnStarted {
                snapshot: snapshot.clone(),
            },
            Some(snapshot),
            None,
        )
    }

    pub fn record_provider_output_applied(
        &self,
        history: &SessionHistory,
        turn: &TurnRecord,
        output: &ProviderSemanticOutput,
        schema_rejections: u32,
    ) -> Result<ReasonPersistenceCursor, ReasonPersistenceError> {
        let snapshot = ActiveTurnSnapshot {
            turn: turn.clone(),
            schema_rejections,
        };
        self.persist_row(
            history,
            Some(turn.request.turn_id.clone()),
            ReasonLedgerPayload::ProviderOutputApplied {
                output: output.clone(),
                snapshot: snapshot.clone(),
            },
            Some(snapshot),
            None,
        )
    }

    pub fn record_completion_rejected(
        &self,
        history: &SessionHistory,
        turn: &TurnRecord,
        rejection: &CompletionSchemaRejection,
        schema_rejections: u32,
    ) -> Result<ReasonPersistenceCursor, ReasonPersistenceError> {
        let snapshot = ActiveTurnSnapshot {
            turn: turn.clone(),
            schema_rejections,
        };
        self.persist_row(
            history,
            Some(turn.request.turn_id.clone()),
            ReasonLedgerPayload::CompletionRejected {
                rejection: rejection.clone(),
                snapshot: snapshot.clone(),
            },
            Some(snapshot),
            None,
        )
    }

    pub fn record_turn_closed(
        &self,
        history: &SessionHistory,
        turn: &TurnRecord,
        schema_rejections: u32,
    ) -> Result<ReasonPersistenceCursor, ReasonPersistenceError> {
        self.persist_row(
            history,
            Some(turn.request.turn_id.clone()),
            ReasonLedgerPayload::TurnClosed {
                turn: turn.clone(),
                schema_rejections,
            },
            None,
            Some(turn.clone()),
        )
    }

    pub fn record_rewrite_state_updated(
        &self,
        history: &SessionHistory,
        latest_turn_id: Option<TurnId>,
        active_turn: Option<ActiveTurnSnapshot>,
    ) -> Result<ReasonPersistenceCursor, ReasonPersistenceError> {
        self.persist_row(
            history,
            latest_turn_id,
            ReasonLedgerPayload::RewriteStateUpdated,
            active_turn,
            None,
        )
    }

    pub fn record_provider_raw_event(
        &self,
        write: ProviderRawLedgerWrite,
    ) -> Result<(), ReasonPersistenceError> {
        let row = ProviderRawLedgerRow {
            schema_version: PROVIDER_RAW_LEDGER_SCHEMA_VERSION,
            provider_family: write.provider_family,
            session_id: write.session_id,
            turn_id: write.turn_id,
            trace_id: write.trace_id,
            raw_kind: write.raw_kind,
            scene: write.scene,
            body: write.body,
            headers: write.headers,
            captured_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?
                .as_secs(),
        };
        self.append_provider_raw_row(&row)
    }

    pub fn restore(
        &self,
        session_id: &SessionId,
    ) -> Result<RestoredReasonSession, ReasonPersistenceError> {
        let ledger_rows = self.load_reason_ledger(session_id)?;
        match self.load_authoritative_state(session_id) {
            Ok(Some(mut restored)) => {
                let last_applied_seq = restored.cursor.last_applied_reason_seq;
                for row in ledger_rows
                    .iter()
                    .filter(|row| row.cursor_after.last_applied_reason_seq > last_applied_seq)
                {
                    apply_ledger_row(&mut restored, row)?;
                }
                self.persist_restored_state(session_id, &restored)?;
                Ok(restored)
            }
            Ok(None) => {
                if ledger_rows.is_empty() {
                    Err(ReasonPersistenceError::MissingRecoveryTruth(
                        session_id.as_str().to_owned(),
                    ))
                } else {
                    let restored = rebuild_from_ledger_rows(&ledger_rows)?;
                    self.persist_restored_state(session_id, &restored)?;
                    Ok(restored)
                }
            }
            Err(snapshot_err) => {
                if ledger_rows.is_empty() {
                    Err(snapshot_err)
                } else {
                    let restored = rebuild_from_ledger_rows(&ledger_rows)?;
                    self.persist_restored_state(session_id, &restored)?;
                    Ok(restored)
                }
            }
        }
    }

    pub fn restore_turn_start_snapshots(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<TurnRecord>, ReasonPersistenceError> {
        let rollback_markers = self.load_session_rollback_markers(session_id)?;
        let is_rolled_back = |turn: &TurnRecord| {
            let key = logical_turn_key(&turn.request.turn_id);
            rollback_markers
                .iter()
                .any(|marker| marker.target_logical_turn_key == key)
        };
        let ledger_rows = self.load_reason_ledger(session_id)?;
        if !ledger_rows.is_empty() {
            return Ok(ledger_rows
                .into_iter()
                .filter_map(|row| match row.payload {
                    ReasonLedgerPayload::TurnStarted { snapshot } => Some(snapshot.turn),
                    _ => None,
                })
                .filter(|turn| !is_rolled_back(turn))
                .collect());
        }

        let restored = self.restore(session_id)?;
        let mut turns = restored.closed_turns;
        if let Some(active) = restored.active_turn {
            turns.push(active.turn);
        }
        turns.retain(|turn| !is_rolled_back(turn));
        turns.sort_by(|left, right| left.request.turn_id.cmp(&right.request.turn_id));
        Ok(turns)
    }

    pub fn list_persisted_sessions(
        &self,
    ) -> Result<Vec<PersistedSessionIndexEntry>, ReasonPersistenceError> {
        self.load_session_index()
    }

    pub fn load_session_metadata(
        &self,
    ) -> Result<Vec<PersistedSessionMetadataEntry>, ReasonPersistenceError> {
        self.load_session_metadata_entries()
    }

    pub fn create_session_metadata(
        &self,
        session_id: SessionId,
        title: Option<String>,
        cwd: Option<String>,
    ) -> Result<PersistedSessionMetadataEntry, ReasonPersistenceError> {
        validate_session_id(&session_id)?;
        let mut entries = self.load_session_metadata_entries()?;
        let entry = PersistedSessionMetadataEntry {
            agent_id: self.agent_id.clone(),
            session_id,
            title: normalize_optional_title(title)?,
            archived: false,
            cwd: normalize_optional_string(cwd),
            updated_unix_seconds: unix_seconds_now(),
        };
        upsert_session_metadata_entry(&mut entries, entry.clone());
        self.write_session_metadata_entries(&entries)?;
        Ok(entry)
    }

    pub fn rename_session(
        &self,
        session_id: &SessionId,
        title: String,
    ) -> Result<PersistedSessionMetadataEntry, ReasonPersistenceError> {
        validate_session_target_exists(self, session_id)?;
        let title = normalize_title(title)?;
        self.mutate_session_metadata(session_id, |entry| {
            entry.title = Some(title);
            entry.updated_unix_seconds = unix_seconds_now();
        })
    }

    pub fn archive_session(
        &self,
        session_id: &SessionId,
    ) -> Result<PersistedSessionMetadataEntry, ReasonPersistenceError> {
        validate_session_target_exists(self, session_id)?;
        self.mutate_session_metadata(session_id, |entry| {
            entry.archived = true;
            entry.updated_unix_seconds = unix_seconds_now();
        })
    }

    pub fn restore_session(
        &self,
        session_id: &SessionId,
    ) -> Result<PersistedSessionMetadataEntry, ReasonPersistenceError> {
        validate_session_target_exists(self, session_id)?;
        self.mutate_session_metadata(session_id, |entry| {
            entry.archived = false;
            entry.updated_unix_seconds = unix_seconds_now();
        })
    }

    pub fn delete_session(
        &self,
        session_id: &SessionId,
    ) -> Result<PersistedSessionMetadataEntry, ReasonPersistenceError> {
        self.archive_session(session_id)
    }

    pub fn rollback_latest_session_turn(
        &self,
        session_id: &SessionId,
    ) -> Result<SessionRollbackMarker, ReasonPersistenceError> {
        validate_session_id(session_id)?;
        let lock_path = self.reason_persistence_lock_path(session_id);
        ensure_parent_dir(&lock_path)?;
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        lock_file
            .lock_exclusive()
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        let result = self.rollback_latest_session_turn_locked(session_id);
        let unlock_result = lock_file
            .unlock()
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()));
        match (result, unlock_result) {
            (Ok(marker), Ok(())) => Ok(marker),
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
        }
    }

    pub fn restore_turn_snapshots_for_ui(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<TurnRecord>, ReasonPersistenceError> {
        if let Some(restored) = self.load_authoritative_state(session_id)? {
            let authoritative_turns = ui_turn_snapshots_from_restored(restored);
            if ui_turn_snapshots_have_complete_rounds(&authoritative_turns) {
                return Ok(authoritative_turns);
            }

            let ledger_turns =
                ui_turn_snapshots_from_ledger_rows(self.load_reason_ledger(session_id)?);
            if ledger_turns.is_empty() {
                return Err(ReasonPersistenceError::InvalidCursorCoherence(
                    "authoritative UI snapshots are missing earlier round truth and reason ledger is empty"
                        .to_owned(),
                ));
            }
            return Ok(ledger_turns);
        }

        let mut turns = ui_turn_snapshots_from_ledger_rows(self.load_reason_ledger(session_id)?);
        if turns.is_empty() {
            let restored = self.restore(session_id)?;
            turns = ui_turn_snapshots_from_restored(restored);
        }
        Ok(turns)
    }

    pub fn restore_authoritative_turn_snapshots_for_ui(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<TurnRecord>, ReasonPersistenceError> {
        self.load_authoritative_state(session_id)?
            .map(ui_turn_snapshots_from_restored)
            .ok_or_else(|| {
                ReasonPersistenceError::MissingRecoveryTruth(session_id.as_str().to_owned())
            })
    }

    fn rollback_latest_session_turn_locked(
        &self,
        session_id: &SessionId,
    ) -> Result<SessionRollbackMarker, ReasonPersistenceError> {
        validate_session_target_exists(self, session_id)?;
        let restored = self.restore(session_id).map_err(|err| match err {
            ReasonPersistenceError::MissingRecoveryTruth(_) => {
                ReasonPersistenceError::SessionRollbackTargetNotFound(
                    session_id.as_str().to_owned(),
                )
            }
            other => other,
        })?;
        if restored.active_turn.is_some() {
            return Err(ReasonPersistenceError::SessionRollbackActiveTurn(
                session_id.as_str().to_owned(),
            ));
        }
        let mut closed_turns = restored.closed_turns.clone();
        closed_turns.sort_by(|left, right| {
            (
                logical_turn_key(&left.request.turn_id),
                left.request.turn_id.as_str().to_owned(),
            )
                .cmp(&(
                    logical_turn_key(&right.request.turn_id),
                    right.request.turn_id.as_str().to_owned(),
                ))
        });
        let target = closed_turns.last().cloned().ok_or_else(|| {
            ReasonPersistenceError::SessionRollbackTargetNotFound(session_id.as_str().to_owned())
        })?;
        let target_logical_turn_key = logical_turn_key(&target.request.turn_id);
        let restored_user_text = closed_turns
            .iter()
            .find(|turn| logical_turn_key(&turn.request.turn_id) == target_logical_turn_key)
            .map(|turn| turn.request.user_text.clone())
            .unwrap_or_else(|| target.request.user_text.clone());
        let previous_effective_head = closed_turns
            .iter()
            .rev()
            .find(|turn| logical_turn_key(&turn.request.turn_id) != target_logical_turn_key)
            .map(|turn| turn.request.turn_id.clone());
        let marker = SessionRollbackMarker {
            rollback_id: format!(
                "rollback-{}-{}",
                target.request.turn_id.as_str(),
                unix_seconds_now()
            ),
            session_id: session_id.clone(),
            target_turn_id: target.request.turn_id.clone(),
            target_logical_turn_key,
            previous_effective_head: previous_effective_head.clone(),
            restored_user_text,
            writer_owner: "reason.persistence".to_owned(),
            reason: "rollback latest session turn".to_owned(),
            updated_unix_seconds: unix_seconds_now(),
        };
        self.persist_row_locked(
            &restored.history,
            previous_effective_head,
            ReasonLedgerPayload::SessionRollback {
                marker: marker.clone(),
            },
            None,
            None,
        )?;
        Ok(marker)
    }

    fn persist_row(
        &self,
        history: &SessionHistory,
        latest_turn_id: Option<TurnId>,
        payload: ReasonLedgerPayload,
        active_turn: Option<ActiveTurnSnapshot>,
        closed_turn: Option<TurnRecord>,
    ) -> Result<ReasonPersistenceCursor, ReasonPersistenceError> {
        let lock_path = self.reason_persistence_lock_path(history.session_id());
        ensure_parent_dir(&lock_path)?;
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        lock_file
            .lock_exclusive()
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        let result =
            self.persist_row_locked(history, latest_turn_id, payload, active_turn, closed_turn);
        let unlock_result = lock_file
            .unlock()
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()));
        match (result, unlock_result) {
            (Ok(cursor), Ok(())) => Ok(cursor),
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
        }
    }

    fn persist_row_locked(
        &self,
        history: &SessionHistory,
        latest_turn_id: Option<TurnId>,
        payload: ReasonLedgerPayload,
        active_turn: Option<ActiveTurnSnapshot>,
        closed_turn: Option<TurnRecord>,
    ) -> Result<ReasonPersistenceCursor, ReasonPersistenceError> {
        let current = self.load_authoritative_state(history.session_id())?;
        let current_cursor = current
            .as_ref()
            .map(|state| state.cursor.clone())
            .unwrap_or_default();
        let next_seq = current_cursor.last_applied_reason_seq.saturating_add(1);
        let rollback_marker = match &payload {
            ReasonLedgerPayload::SessionRollback { marker } => Some(marker.clone()),
            _ => None,
        };
        let cursor_after = ReasonPersistenceCursor {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            last_applied_reason_seq: next_seq,
            latest_turn_id: if rollback_marker.is_some() {
                latest_turn_id.clone()
            } else {
                latest_turn_id.clone().or(current_cursor.latest_turn_id)
            },
            active_turn_id: active_turn
                .as_ref()
                .map(|snapshot| snapshot.turn.request.turn_id.clone()),
        };
        let row = ReasonLedgerRow {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            seq: next_seq,
            created_at: unix_seconds_now(),
            session_id: history.session_id().clone(),
            turn_id: latest_turn_id,
            cursor_after: cursor_after.clone(),
            session_history: history.clone(),
            payload,
        };
        self.append_row_only(history.session_id(), &row)?;
        if let Some(marker) = &rollback_marker {
            self.append_session_rollback_marker(history.session_id(), marker)?;
        }

        let mut closed_turns = self.load_closed_turns(history.session_id())?;
        if let Some(turn) = closed_turn {
            upsert_closed_turn(&mut closed_turns, turn);
        }
        if let Some(marker) = rollback_marker {
            closed_turns.retain(|turn| {
                logical_turn_key(&turn.request.turn_id) != marker.target_logical_turn_key
            });
        }

        let mut restored_history = history.clone();
        filter_history_context_to_effective_turns(
            &mut restored_history,
            active_turn.as_ref(),
            &closed_turns,
        )?;

        let restored = RestoredReasonSession {
            history: restored_history,
            cursor: cursor_after.clone(),
            active_turn,
            closed_turns,
        };
        self.persist_restored_state(history.session_id(), &restored)?;
        Ok(cursor_after)
    }

    fn append_row_only(
        &self,
        session_id: &SessionId,
        row: &ReasonLedgerRow,
    ) -> Result<(), ReasonPersistenceError> {
        let ledger_path = self.reason_ledger_path(session_id);
        ensure_parent_dir(&ledger_path)?;
        let payload = serde_json::to_string(row)
            .map_err(|err| ReasonPersistenceError::JsonRenderFailed(err.to_string()))?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&ledger_path)
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        file.lock_exclusive()
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        use std::io::Write;
        let result = writeln!(file, "{payload}")
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()));
        let unlock_result = file
            .unlock()
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()));
        result?;
        unlock_result
    }

    fn persist_restored_state(
        &self,
        session_id: &SessionId,
        restored: &RestoredReasonSession,
    ) -> Result<(), ReasonPersistenceError> {
        let session_dir = self.session_dir(session_id);
        fs::create_dir_all(self.turns_dir(session_id))
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        write_json_atomic(&self.session_history_path(session_id), &restored.history)?;
        write_json_atomic(&self.cursor_path(session_id), &restored.cursor)?;
        match &restored.active_turn {
            Some(snapshot) => write_json_atomic(&self.active_turn_path(session_id), snapshot)?,
            None => remove_if_exists(&self.active_turn_path(session_id))?,
        }
        for turn in &restored.closed_turns {
            write_json_atomic(
                &self.closed_turn_path(session_id, &turn.request.turn_id),
                turn,
            )?;
        }
        self.write_sidecars(session_id, restored)?;
        if !session_dir.exists() {
            fs::create_dir_all(session_dir)
                .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        }
        Ok(())
    }

    fn write_sidecars(
        &self,
        session_id: &SessionId,
        restored: &RestoredReasonSession,
    ) -> Result<(), ReasonPersistenceError> {
        let engine = ReasonTurnEngine::new();
        let mut turns = restored.closed_turns.clone();
        if let Some(active) = &restored.active_turn {
            turns.push(active.turn.clone());
        }
        turns.sort_by(|left, right| {
            left.request
                .turn_id
                .as_str()
                .cmp(right.request.turn_id.as_str())
        });
        let projections = engine.project_session(&turns);
        let view = PersistedSessionView {
            agent_id: self.agent_id.clone(),
            session_id: session_id.clone(),
            latest_turn_id: restored.cursor.latest_turn_id.clone(),
            active_turn_id: restored.cursor.active_turn_id.clone(),
            projections,
        };
        write_json_atomic(&self.ui_sidecar_path(session_id), &view)?;

        let mut index = self.load_session_index()?;
        let entry = PersistedSessionIndexEntry {
            agent_id: self.agent_id.clone(),
            session_id: session_id.clone(),
            latest_turn_id: restored.cursor.latest_turn_id.clone(),
            active_turn_id: restored.cursor.active_turn_id.clone(),
            latest_terminal_summary: restored
                .closed_turns
                .last()
                .and_then(|turn| turn.terminal_event.as_ref())
                .map(|event| event.summary.clone()),
        };
        index.retain(|existing| existing.session_id != *session_id);
        index.push(entry);
        index.sort_by(|left, right| left.session_id.as_str().cmp(right.session_id.as_str()));
        write_json_atomic(&self.session_index_path(), &index)
    }

    fn load_authoritative_state(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<RestoredReasonSession>, ReasonPersistenceError> {
        let history_exists = self.session_history_path(session_id).is_file();
        let cursor_exists = self.cursor_path(session_id).is_file();
        let active_exists = self.active_turn_path(session_id).is_file();
        let turns_exist = self.turns_dir(session_id).is_dir();
        if !history_exists && !cursor_exists && !active_exists && !turns_exist {
            return Ok(None);
        }
        if !history_exists || !cursor_exists {
            return Err(ReasonPersistenceError::InvalidCursorCoherence(
                "authoritative snapshots require both session-history and cursor files".to_owned(),
            ));
        }
        let mut history = SessionHistory::load_from_path(self.session_history_path(session_id))
            .map_err(|err| ReasonPersistenceError::JsonParseFailed(err.to_string()))?;
        let cursor: ReasonPersistenceCursor = read_json_file(&self.cursor_path(session_id))?;
        let active_turn = if active_exists {
            let path = self.active_turn_path(session_id);
            let mut snapshot: ActiveTurnSnapshot = read_json_file(&path)?;
            ensure_turn_created_at(&mut snapshot.turn, file_modified_unix_seconds(&path)?);
            Some(snapshot)
        } else {
            None
        };
        let mut closed_turns = self.load_closed_turns(session_id)?;
        apply_rollback_markers_to_closed_turns(
            &mut closed_turns,
            &self.load_session_rollback_markers(session_id)?,
        );
        filter_history_context_to_effective_turns(
            &mut history,
            active_turn.as_ref(),
            &closed_turns,
        )?;
        validate_cursor(&cursor, active_turn.as_ref(), &closed_turns)?;
        Ok(Some(RestoredReasonSession {
            history,
            cursor,
            active_turn,
            closed_turns,
        }))
    }

    fn load_reason_ledger(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<ReasonLedgerRow>, ReasonPersistenceError> {
        if !self.reason_ledger_path(session_id).is_file() {
            return Ok(Vec::new());
        }
        let payload = fs::read_to_string(self.reason_ledger_path(session_id))
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        let mut rows = Vec::new();
        for line in payload.lines().filter(|line| !line.trim().is_empty()) {
            let row: ReasonLedgerRow = serde_json::from_str(line)
                .map_err(|err| ReasonPersistenceError::JsonParseFailed(err.to_string()))?;
            rows.push(row);
        }
        normalize_ledger_rows(session_id, rows)
    }

    fn load_closed_turns(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<TurnRecord>, ReasonPersistenceError> {
        let turns_dir = self.turns_dir(session_id);
        if !turns_dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(turns_dir)
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        entries.sort_by_key(|entry| entry.file_name());
        let mut turns = Vec::new();
        for entry in entries {
            if entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
            {
                let path = entry.path();
                if !is_closed_turn_snapshot_path(&path) {
                    continue;
                }
                let mut turn = read_json_file::<TurnRecord>(&path)?;
                ensure_turn_created_at(&mut turn, file_modified_unix_seconds(&path)?);
                turns.push(turn);
            }
        }
        Ok(turns)
    }

    fn load_session_rollback_markers(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRollbackMarker>, ReasonPersistenceError> {
        let path = self.rollback_markers_path(session_id);
        if !path.is_file() {
            return Ok(Vec::new());
        }
        read_json_file(&path)
    }

    fn append_session_rollback_marker(
        &self,
        session_id: &SessionId,
        marker: &SessionRollbackMarker,
    ) -> Result<(), ReasonPersistenceError> {
        let mut markers = self.load_session_rollback_markers(session_id)?;
        if !markers
            .iter()
            .any(|existing| existing.rollback_id == marker.rollback_id)
        {
            markers.push(marker.clone());
        }
        write_json_atomic(&self.rollback_markers_path(session_id), &markers)
    }

    fn load_session_index(
        &self,
    ) -> Result<Vec<PersistedSessionIndexEntry>, ReasonPersistenceError> {
        if !self.session_index_path().is_file() {
            return Ok(Vec::new());
        }
        read_json_file(&self.session_index_path())
    }

    fn load_session_metadata_entries(
        &self,
    ) -> Result<Vec<PersistedSessionMetadataEntry>, ReasonPersistenceError> {
        if !self.session_metadata_path().is_file() {
            return Ok(Vec::new());
        }
        read_json_file(&self.session_metadata_path())
    }

    fn write_session_metadata_entries(
        &self,
        entries: &[PersistedSessionMetadataEntry],
    ) -> Result<(), ReasonPersistenceError> {
        write_json_atomic(&self.session_metadata_path(), &entries)
    }

    fn mutate_session_metadata(
        &self,
        session_id: &SessionId,
        mutate: impl FnOnce(&mut PersistedSessionMetadataEntry),
    ) -> Result<PersistedSessionMetadataEntry, ReasonPersistenceError> {
        let mut entries = self.load_session_metadata_entries()?;
        let index = entries
            .iter()
            .position(|entry| entry.session_id == *session_id)
            .unwrap_or_else(|| {
                entries.push(PersistedSessionMetadataEntry {
                    agent_id: self.agent_id.clone(),
                    session_id: session_id.clone(),
                    title: None,
                    archived: false,
                    cwd: None,
                    updated_unix_seconds: unix_seconds_now(),
                });
                entries.len() - 1
            });
        mutate(&mut entries[index]);
        let updated = entries[index].clone();
        self.write_session_metadata_entries(&entries)?;
        Ok(updated)
    }

    fn append_provider_raw_row(
        &self,
        row: &ProviderRawLedgerRow,
    ) -> Result<(), ReasonPersistenceError> {
        let path =
            self.provider_raw_ledger_path(row.provider_family, &row.session_id, &row.turn_id);
        ensure_parent_dir(&path)?;
        let payload = serde_json::to_string(row)
            .map_err(|err| ReasonPersistenceError::JsonRenderFailed(err.to_string()))?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
        use std::io::Write;
        writeln!(file, "{payload}")
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))
    }

    fn session_dir(&self, session_id: &SessionId) -> PathBuf {
        self.runtime_home
            .join("state")
            .join("turns")
            .join(self.agent_id.as_str())
            .join(session_id.as_str())
    }

    fn session_history_path(&self, session_id: &SessionId) -> PathBuf {
        self.session_dir(session_id).join("session-history.json")
    }

    fn cursor_path(&self, session_id: &SessionId) -> PathBuf {
        self.session_dir(session_id).join("session-cursor.json")
    }

    fn active_turn_path(&self, session_id: &SessionId) -> PathBuf {
        self.session_dir(session_id).join("active-turn.json")
    }

    fn turns_dir(&self, session_id: &SessionId) -> PathBuf {
        self.session_dir(session_id).join("turns")
    }

    fn closed_turn_path(&self, session_id: &SessionId, turn_id: &TurnId) -> PathBuf {
        self.turns_dir(session_id)
            .join(format!("{}.json", turn_id.as_str()))
    }

    fn rollback_markers_path(&self, session_id: &SessionId) -> PathBuf {
        self.session_dir(session_id).join("rollback-markers.json")
    }

    fn reason_ledger_path(&self, session_id: &SessionId) -> PathBuf {
        self.runtime_home
            .join("ledgers")
            .join("reason")
            .join(self.agent_id.as_str())
            .join(format!("{}.jsonl", session_id.as_str()))
    }

    fn reason_persistence_lock_path(&self, session_id: &SessionId) -> PathBuf {
        self.runtime_home
            .join("locks")
            .join("reason")
            .join(self.agent_id.as_str())
            .join(format!("{}.lock", session_id.as_str()))
    }

    fn provider_raw_ledger_path(
        &self,
        provider_family: ProviderFamily,
        session_id: &SessionId,
        turn_id: &TurnId,
    ) -> PathBuf {
        let family = match provider_family {
            ProviderFamily::OpenAiCompatible => "openai-compatible",
            ProviderFamily::Anthropic => "anthropic",
        };
        self.runtime_home
            .join("ledgers")
            .join("providers")
            .join(family)
            .join(self.agent_id.as_str())
            .join(session_id.as_str())
            .join(format!("{}.jsonl", turn_id.as_str()))
    }

    fn ui_sidecar_path(&self, session_id: &SessionId) -> PathBuf {
        self.runtime_home
            .join("state")
            .join("ui")
            .join(self.agent_id.as_str())
            .join(format!("{}.json", session_id.as_str()))
    }

    fn session_index_path(&self) -> PathBuf {
        self.runtime_home
            .join("cache")
            .join("session-index")
            .join(format!("{}.json", self.agent_id.as_str()))
    }

    fn session_metadata_path(&self) -> PathBuf {
        self.runtime_home
            .join("state")
            .join("ui")
            .join(self.agent_id.as_str())
            .join("session-metadata.json")
    }
}

fn validate_cursor(
    cursor: &ReasonPersistenceCursor,
    active_turn: Option<&ActiveTurnSnapshot>,
    closed_turns: &[TurnRecord],
) -> Result<(), ReasonPersistenceError> {
    if cursor.schema_version != PERSISTENCE_SCHEMA_VERSION {
        return Err(ReasonPersistenceError::InvalidCursorCoherence(
            "unsupported cursor schema version".to_owned(),
        ));
    }
    match (&cursor.active_turn_id, active_turn) {
        (Some(turn_id), Some(snapshot)) if snapshot.turn.request.turn_id == *turn_id => {}
        (None, None) => {}
        (Some(_), None) => {
            return Err(ReasonPersistenceError::InvalidCursorCoherence(
                "cursor references active turn but active-turn snapshot is missing".to_owned(),
            ));
        }
        (None, Some(_)) => {
            return Err(ReasonPersistenceError::InvalidCursorCoherence(
                "active-turn snapshot exists but cursor does not reference it".to_owned(),
            ));
        }
        (Some(_), Some(_)) => {
            return Err(ReasonPersistenceError::InvalidCursorCoherence(
                "active-turn snapshot does not match cursor active_turn_id".to_owned(),
            ));
        }
    }
    if let Some(latest_turn_id) = &cursor.latest_turn_id {
        let active_matches = active_turn
            .as_ref()
            .is_some_and(|snapshot| snapshot.turn.request.turn_id == *latest_turn_id);
        let closed_matches = closed_turns
            .iter()
            .any(|turn| turn.request.turn_id == *latest_turn_id);
        if !active_matches && !closed_matches {
            return Err(ReasonPersistenceError::InvalidCursorCoherence(
                "cursor latest_turn_id does not exist in persisted turn truth".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_session_id(session_id: &SessionId) -> Result<(), ReasonPersistenceError> {
    if session_id.as_str().trim().is_empty() {
        return Err(ReasonPersistenceError::InvalidSessionMetadata(
            "session id must be non-empty".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_title(title: String) -> Result<String, ReasonPersistenceError> {
    let title = title.trim().to_owned();
    if title.is_empty() {
        return Err(ReasonPersistenceError::InvalidSessionMetadata(
            "session title must be non-empty".to_owned(),
        ));
    }
    Ok(title)
}

fn normalize_optional_title(
    title: Option<String>,
) -> Result<Option<String>, ReasonPersistenceError> {
    title.map(normalize_title).transpose()
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn ensure_turn_created_at(turn: &mut TurnRecord, fallback: u64) {
    if turn.created_at == 0 {
        turn.created_at = fallback;
    }
}

fn file_modified_unix_seconds(path: &Path) -> Result<u64, ReasonPersistenceError> {
    let modified = fs::metadata(path)
        .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?
        .modified()
        .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
    Ok(modified
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0))
}

fn unix_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn validate_session_target_exists(
    persistence: &ReasonPersistence,
    session_id: &SessionId,
) -> Result<(), ReasonPersistenceError> {
    validate_session_id(session_id)?;
    let in_metadata = persistence
        .load_session_metadata_entries()?
        .iter()
        .any(|entry| entry.session_id == *session_id);
    let in_index = persistence
        .load_session_index()?
        .iter()
        .any(|entry| entry.session_id == *session_id);
    if in_metadata || in_index {
        Ok(())
    } else {
        Err(ReasonPersistenceError::SessionMetadataTargetNotFound(
            session_id.as_str().to_owned(),
        ))
    }
}

fn upsert_session_metadata_entry(
    entries: &mut Vec<PersistedSessionMetadataEntry>,
    entry: PersistedSessionMetadataEntry,
) {
    entries.retain(|existing| existing.session_id != entry.session_id);
    entries.push(entry);
    entries.sort_by(|left, right| left.session_id.as_str().cmp(right.session_id.as_str()));
}

fn normalize_ledger_rows(
    session_id: &SessionId,
    rows: Vec<ReasonLedgerRow>,
) -> Result<Vec<ReasonLedgerRow>, ReasonPersistenceError> {
    let mut expected_seq = 1_u64;
    let mut normalized = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().cloned().enumerate() {
        if row.schema_version != PERSISTENCE_SCHEMA_VERSION {
            return Err(ReasonPersistenceError::InvalidLedgerCoherence(
                "unsupported ledger schema version".to_owned(),
            ));
        }
        if row.session_id != *session_id {
            return Err(ReasonPersistenceError::InvalidLedgerCoherence(
                "ledger row session id does not match requested session".to_owned(),
            ));
        }
        if row.seq != expected_seq {
            if row.seq.saturating_add(1) == expected_seq
                && remaining_rows_contain_seq(&rows, index + 1, expected_seq, session_id)?
            {
                continue;
            }
            return Err(ReasonPersistenceError::LedgerSequenceGap {
                expected: expected_seq,
                actual: row.seq,
            });
        }
        if row.cursor_after.last_applied_reason_seq != row.seq {
            return Err(ReasonPersistenceError::InvalidLedgerCoherence(
                "ledger row cursor does not match row sequence".to_owned(),
            ));
        }
        normalized.push(row);
        expected_seq = expected_seq.saturating_add(1);
    }
    Ok(normalized)
}

fn remaining_rows_contain_seq(
    rows: &[ReasonLedgerRow],
    start_index: usize,
    expected_seq: u64,
    session_id: &SessionId,
) -> Result<bool, ReasonPersistenceError> {
    for row in rows.iter().skip(start_index) {
        if row.schema_version != PERSISTENCE_SCHEMA_VERSION {
            return Err(ReasonPersistenceError::InvalidLedgerCoherence(
                "unsupported ledger schema version".to_owned(),
            ));
        }
        if row.session_id != *session_id {
            return Err(ReasonPersistenceError::InvalidLedgerCoherence(
                "ledger row session id does not match requested session".to_owned(),
            ));
        }
        if row.seq == expected_seq {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rebuild_from_ledger_rows(
    rows: &[ReasonLedgerRow],
) -> Result<RestoredReasonSession, ReasonPersistenceError> {
    let Some(first) = rows.first() else {
        return Err(ReasonPersistenceError::MissingRecoveryTruth(
            "ledger-only rebuild requires at least one row".to_owned(),
        ));
    };
    let mut restored = RestoredReasonSession {
        history: first.session_history.clone(),
        cursor: first.cursor_after.clone(),
        active_turn: None,
        closed_turns: Vec::new(),
    };
    for row in rows {
        apply_ledger_row(&mut restored, row)?;
    }
    Ok(restored)
}

fn apply_ledger_row(
    restored: &mut RestoredReasonSession,
    row: &ReasonLedgerRow,
) -> Result<(), ReasonPersistenceError> {
    restored.history = row.session_history.clone();
    restored.cursor = row.cursor_after.clone();
    match &row.payload {
        ReasonLedgerPayload::TurnStarted { snapshot }
        | ReasonLedgerPayload::ProviderOutputApplied { snapshot, .. }
        | ReasonLedgerPayload::CompletionRejected { snapshot, .. } => {
            let mut snapshot = snapshot.clone();
            ensure_turn_created_at(&mut snapshot.turn, row.created_at);
            restored.active_turn = Some(snapshot);
        }
        ReasonLedgerPayload::TurnClosed { turn, .. } => {
            let mut turn = turn.clone();
            ensure_turn_created_at(&mut turn, row.created_at);
            upsert_closed_turn(&mut restored.closed_turns, turn);
            restored.active_turn = None;
        }
        ReasonLedgerPayload::SessionRollback { marker } => {
            restored.closed_turns.retain(|turn| {
                logical_turn_key(&turn.request.turn_id) != marker.target_logical_turn_key
            });
            restored.active_turn = None;
        }
        ReasonLedgerPayload::RewriteStateUpdated => {}
    }
    filter_history_context_to_effective_turns(
        &mut restored.history,
        restored.active_turn.as_ref(),
        &restored.closed_turns,
    )?;
    validate_cursor(
        &restored.cursor,
        restored.active_turn.as_ref(),
        &restored.closed_turns,
    )
}

fn apply_rollback_markers_to_closed_turns(
    turns: &mut Vec<TurnRecord>,
    markers: &[SessionRollbackMarker],
) {
    for marker in markers {
        turns.retain(|turn| {
            logical_turn_key(&turn.request.turn_id) != marker.target_logical_turn_key
        });
    }
}

fn logical_turn_key(turn_id: &TurnId) -> String {
    let raw = turn_id.as_str();
    if let Some((base, _round)) = raw.split_once("-r") {
        base.to_owned()
    } else {
        raw.to_owned()
    }
}

fn filter_history_context_to_effective_turns(
    history: &mut SessionHistory,
    active_turn: Option<&ActiveTurnSnapshot>,
    closed_turns: &[TurnRecord],
) -> Result<(), ReasonPersistenceError> {
    let mut effective_logical_keys = BTreeSet::new();
    for turn in closed_turns {
        effective_logical_keys.insert(logical_turn_key(&turn.request.turn_id));
    }
    if let Some(active) = active_turn {
        effective_logical_keys.insert(logical_turn_key(&active.turn.request.turn_id));
    }

    history
        .retain_base_context_segments(|segment| {
            historical_turn_reference_logical_key(segment)
                .map(|key| effective_logical_keys.contains(&key))
                .unwrap_or(true)
        })
        .map_err(|err| ReasonPersistenceError::InvalidCursorCoherence(err.to_string()))
}

fn historical_turn_reference_logical_key(segment: &ContextSegment) -> Option<String> {
    segment
        .provenance
        .reference
        .as_ref()
        .and_then(|reference| reference.strip_prefix("historical_turn:"))
        .map(|turn_id| logical_turn_key(&TurnId::new(turn_id)))
}

fn logical_turn_round(turn_id: &TurnId) -> u64 {
    let raw = turn_id.as_str();
    raw.rsplit_once("-r")
        .and_then(|(_base, round)| round.parse::<u64>().ok())
        .unwrap_or(0)
}

fn ui_turn_snapshots_from_restored(restored: RestoredReasonSession) -> Vec<TurnRecord> {
    let mut turns_by_turn_id = BTreeMap::<String, TurnRecord>::new();
    for turn in restored.closed_turns {
        upsert_ui_turn_snapshot_by_turn_id(&mut turns_by_turn_id, turn);
    }
    if let Some(active) = restored.active_turn {
        upsert_ui_turn_snapshot_by_turn_id(&mut turns_by_turn_id, active.turn);
    }
    sorted_ui_turn_snapshots(turns_by_turn_id)
}

fn ui_turn_snapshots_from_ledger_rows(rows: Vec<ReasonLedgerRow>) -> Vec<TurnRecord> {
    let mut turns_by_turn_id = BTreeMap::<String, TurnRecord>::new();
    for row in rows {
        match row.payload {
            ReasonLedgerPayload::TurnStarted { snapshot }
            | ReasonLedgerPayload::ProviderOutputApplied { snapshot, .. }
            | ReasonLedgerPayload::CompletionRejected { snapshot, .. } => {
                let mut turn = snapshot.turn;
                ensure_turn_created_at(&mut turn, row.created_at);
                upsert_ui_turn_snapshot_by_turn_id(&mut turns_by_turn_id, turn);
            }
            ReasonLedgerPayload::TurnClosed { turn, .. } => {
                let mut turn = turn;
                ensure_turn_created_at(&mut turn, row.created_at);
                upsert_ui_turn_snapshot_by_turn_id(&mut turns_by_turn_id, turn);
            }
            ReasonLedgerPayload::SessionRollback { marker } => {
                turns_by_turn_id.retain(|_turn_id, turn| {
                    logical_turn_key(&turn.request.turn_id) != marker.target_logical_turn_key
                });
            }
            ReasonLedgerPayload::RewriteStateUpdated => {}
        }
    }
    sorted_ui_turn_snapshots(turns_by_turn_id)
}

fn ui_turn_snapshots_have_complete_rounds(turns: &[TurnRecord]) -> bool {
    let mut count_by_logical_key = BTreeMap::<String, u64>::new();
    let mut expected_by_logical_key = BTreeMap::<String, u64>::new();
    for turn in turns {
        let logical_key = logical_turn_key(&turn.request.turn_id);
        *count_by_logical_key.entry(logical_key.clone()).or_insert(0) += 1;
        let expected = logical_turn_round(&turn.request.turn_id).max(1);
        expected_by_logical_key
            .entry(logical_key)
            .and_modify(|known| *known = (*known).max(expected))
            .or_insert(expected);
    }
    expected_by_logical_key
        .into_iter()
        .all(|(logical_key, expected)| {
            count_by_logical_key.get(&logical_key).copied().unwrap_or(0) >= expected
        })
}

fn upsert_ui_turn_snapshot_by_turn_id(
    turns_by_turn_id: &mut BTreeMap<String, TurnRecord>,
    candidate: TurnRecord,
) {
    turns_by_turn_id.insert(candidate.request.turn_id.as_str().to_owned(), candidate);
}

fn sorted_ui_turn_snapshots(turns_by_turn_id: BTreeMap<String, TurnRecord>) -> Vec<TurnRecord> {
    let mut turns = turns_by_turn_id.into_values().collect::<Vec<_>>();
    turns.sort_by(|left, right| {
        let left_key = logical_turn_key(&left.request.turn_id);
        let right_key = logical_turn_key(&right.request.turn_id);
        left_key
            .cmp(&right_key)
            .then_with(|| {
                logical_turn_round(&left.request.turn_id)
                    .cmp(&logical_turn_round(&right.request.turn_id))
            })
            .then_with(|| {
                left.request
                    .turn_id
                    .as_str()
                    .cmp(right.request.turn_id.as_str())
            })
    });
    turns
}

fn upsert_closed_turn(turns: &mut Vec<TurnRecord>, candidate: TurnRecord) {
    if let Some(existing) = turns
        .iter_mut()
        .find(|turn| turn.request.turn_id == candidate.request.turn_id)
    {
        *existing = candidate;
    } else {
        turns.push(candidate);
        turns.sort_by(|left, right| {
            left.request
                .turn_id
                .as_str()
                .cmp(right.request.turn_id.as_str())
        });
    }
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), ReasonPersistenceError> {
    ensure_parent_dir(path)?;
    let payload = serde_json::to_string_pretty(value)
        .map_err(|err| ReasonPersistenceError::JsonRenderFailed(err.to_string()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?
        .as_nanos();
    let temp_path = path.with_extension(format!("tmp-{stamp}"));
    fs::write(&temp_path, payload)
        .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
    fs::rename(&temp_path, path)
        .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ReasonPersistenceError> {
    let payload = fs::read_to_string(path)
        .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
    serde_json::from_str(&payload).map_err(|err| {
        ReasonPersistenceError::JsonParseFailed(format!("{}: {err}", path.display()))
    })
}

fn is_closed_turn_snapshot_path(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("json")
}

fn ensure_parent_dir(path: &Path) -> Result<(), ReasonPersistenceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| ReasonPersistenceError::FileIoFailed(err.to_string()))?;
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), ReasonPersistenceError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(ReasonPersistenceError::FileIoFailed(err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentId, FeatureId, ReasonTurnEngine, SessionId, TraceId, TurnId, TurnStartInput};
    use freehand_contracts::{
        ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
        ContextSegmentKind, ContextStability, ReasonResp01SemanticEvent, SemanticEventKind,
        TerminalStatus,
    };
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_runtime_home() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("freehand-reason-persistence-{stamp}-{counter}"))
    }

    fn stable_segment(id: &str, kind: ContextSegmentKind, content: &str) -> ContextSegment {
        let (stability, cache_policy, role) = match kind {
            ContextSegmentKind::SystemAnchor => (
                ContextStability::Stable,
                ContextCachePolicy::CacheAnchor,
                ContextRole::System,
            ),
            ContextSegmentKind::DeveloperPolicy | ContextSegmentKind::CompletionContract => (
                ContextStability::Stable,
                ContextCachePolicy::CacheAnchor,
                ContextRole::Developer,
            ),
            ContextSegmentKind::SessionMemory
            | ContextSegmentKind::SessionSummary
            | ContextSegmentKind::InstructionCapability
            | ContextSegmentKind::TaskContract => (
                ContextStability::SessionStable,
                ContextCachePolicy::Cacheable,
                ContextRole::Developer,
            ),
            ContextSegmentKind::TaskSpaceSnapshot => (
                ContextStability::TurnVolatile,
                ContextCachePolicy::NoCache,
                ContextRole::Developer,
            ),
            _ => panic!("unsupported stable segment kind"),
        };
        ContextSegment {
            segment_id: ContextSegmentId::new(id),
            kind,
            stability,
            cache_policy,
            role,
            content: content.to_owned(),
            token_budget: 64,
            provenance: ContextProvenance {
                source: "reason_persistence_test".to_owned(),
                reference: None,
            },
        }
    }

    fn historical_turn_segment(id: &str, turn_id: &str, content: &str) -> ContextSegment {
        let mut segment = stable_segment(id, ContextSegmentKind::SessionMemory, content);
        segment.provenance.reference = Some(format!("historical_turn:{turn_id}"));
        segment
    }

    fn session_history() -> SessionHistory {
        SessionHistory::new(
            SessionId::new("session-1"),
            vec![stable_segment(
                "memory-1",
                ContextSegmentKind::SessionMemory,
                "remember persistence state",
            )],
        )
        .expect("history")
    }

    fn started_turn(history: &mut SessionHistory) -> TurnRecord {
        started_turn_with_id(history, "turn-1", "trace-1")
    }

    fn started_turn_with_id(
        history: &mut SessionHistory,
        turn_id: &str,
        trace_id: &str,
    ) -> TurnRecord {
        ReasonTurnEngine::new()
            .start_turn(
                history,
                TurnStartInput {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new(turn_id),
                    trace_id: TraceId::new(trace_id),
                    feature_id: FeatureId::new("reason.persistence"),
                    agent_id: AgentId::new("agent-1"),
                    user_text: "persist this".to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model-a".to_owned(),
                },
            )
            .expect("turn")
    }

    #[test]
    fn concurrent_same_session_writes_allocate_monotonic_sequences() {
        let runtime_home = temp_runtime_home();
        let writer_count = 8;
        let barrier = Arc::new(Barrier::new(writer_count));
        let mut handles = Vec::new();

        for index in 0..writer_count {
            let runtime_home = runtime_home.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
                let mut history = session_history();
                let turn = started_turn_with_id(
                    &mut history,
                    &format!("turn-{index}"),
                    &format!("trace-{index}"),
                );
                barrier.wait();
                coordinator
                    .record_turn_started(&history, &turn, 0)
                    .expect("concurrent persist")
            }));
        }

        let mut cursors = Vec::new();
        for handle in handles {
            cursors.push(handle.join().expect("writer thread"));
        }
        cursors.sort_by_key(|cursor| cursor.last_applied_reason_seq);
        assert_eq!(
            cursors
                .iter()
                .map(|cursor| cursor.last_applied_reason_seq)
                .collect::<Vec<_>>(),
            (1..=writer_count as u64).collect::<Vec<_>>()
        );

        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let rows = coordinator
            .load_reason_ledger(&SessionId::new("session-1"))
            .expect("ledger rows");
        assert_eq!(
            rows.iter().map(|row| row.seq).collect::<Vec<_>>(),
            (1..=writer_count as u64).collect::<Vec<_>>()
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn persistence_save_reload_smoke() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let turn = started_turn(&mut history);

        coordinator
            .record_turn_started(&history, &turn, 0)
            .expect("persist");

        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert_eq!(restored.history, history);
        let active = restored.active_turn.expect("active");
        assert_eq!(active.turn.request.turn_id, TurnId::new("turn-1"));
        assert!(active.turn.created_at > 0);

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn terminal_turn_materialization_smoke() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let mut turn = started_turn(&mut history);
        turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "done".to_owned(),
        });

        coordinator
            .record_turn_closed(&history, &turn, 0)
            .expect("close persist");

        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert!(restored.active_turn.is_none());
        assert_eq!(restored.closed_turns.len(), 1);
        assert!(restored.closed_turns[0].created_at > 0);
        assert!(
            coordinator
                .closed_turn_path(history.session_id(), &TurnId::new("turn-1"))
                .is_file()
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn restore_ignores_leftover_atomic_tmp_turn_files() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let mut turn = started_turn(&mut history);
        turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "done".to_owned(),
        });

        coordinator
            .record_turn_closed(&history, &turn, 0)
            .expect("close persist");
        let temp_path = coordinator
            .turns_dir(history.session_id())
            .join("turn-2.tmp-1784723820102497000");
        fs::write(&temp_path, "").expect("write leftover atomic temp file");

        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert_eq!(restored.closed_turns.len(), 1);
        assert_eq!(
            restored.closed_turns[0].request.turn_id,
            TurnId::new("turn-1")
        );
        let ui_turns = coordinator
            .restore_authoritative_turn_snapshots_for_ui(history.session_id())
            .expect("restore authoritative ui turns");
        assert_eq!(ui_turns.len(), 1);
        assert_eq!(ui_turns[0].request.turn_id, TurnId::new("turn-1"));

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn recovery_from_snapshot_plus_ledger_tail() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let mut turn = started_turn(&mut history);
        coordinator
            .record_turn_started(&history, &turn, 0)
            .expect("start persist");

        let output = ProviderSemanticOutput::SemanticEvent(ReasonResp01SemanticEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            kind: SemanticEventKind::Text,
            content: "hello".to_owned(),
        });
        ReasonTurnEngine::new()
            .apply_provider_output(&mut turn, output.clone())
            .expect("apply provider output");

        let stale_cursor: ReasonPersistenceCursor =
            read_json_file(&coordinator.cursor_path(history.session_id())).expect("cursor");
        let next_seq = stale_cursor.last_applied_reason_seq.saturating_add(1);
        let row = ReasonLedgerRow {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            seq: next_seq,
            created_at: 10,
            session_id: history.session_id().clone(),
            turn_id: Some(turn.request.turn_id.clone()),
            cursor_after: ReasonPersistenceCursor {
                schema_version: PERSISTENCE_SCHEMA_VERSION,
                last_applied_reason_seq: next_seq,
                latest_turn_id: Some(turn.request.turn_id.clone()),
                active_turn_id: Some(turn.request.turn_id.clone()),
            },
            session_history: history.clone(),
            payload: ReasonLedgerPayload::ProviderOutputApplied {
                output,
                snapshot: ActiveTurnSnapshot {
                    turn: turn.clone(),
                    schema_rejections: 0,
                },
            },
        };
        coordinator
            .append_row_only(history.session_id(), &row)
            .expect("append tail");

        let restored = coordinator.restore(history.session_id()).expect("restore");
        let active = restored.active_turn.expect("active");
        assert_eq!(active.turn.semantic_events.len(), 1);
        assert_eq!(restored.cursor.last_applied_reason_seq, 2);

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn restore_rejects_invalid_persisted_snapshot_json_explicitly() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let session_id = SessionId::new("session-1");
        ensure_parent_dir(&coordinator.session_history_path(&session_id)).expect("parent");
        fs::write(
            coordinator.session_history_path(&session_id),
            "{not-valid-json}\n",
        )
        .expect("write invalid history");
        write_json_atomic(
            &coordinator.cursor_path(&session_id),
            &ReasonPersistenceCursor::default(),
        )
        .expect("write cursor");

        let err = coordinator
            .restore(&session_id)
            .expect_err("invalid persisted snapshot json must fail recovery");
        assert!(matches!(err, ReasonPersistenceError::JsonParseFailed(_)));

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn restore_accepts_legacy_closed_turn_without_tool_result_status() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let mut turn = started_turn(&mut history);
        turn.tool_results
            .push(freehand_contracts::ReasonReq05ToolResultReentry {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("reason.persistence"),
                agent_id: AgentId::new("agent-1"),
                tool_result: freehand_contracts::ToolResultContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    status: freehand_contracts::ToolResultStatus::Success,
                    output: "legacy output".to_owned(),
                },
            });
        turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "done".to_owned(),
        });

        coordinator
            .record_turn_closed(&history, &turn, 0)
            .expect("close persist");

        let closed_turn_path =
            coordinator.closed_turn_path(history.session_id(), &TurnId::new("turn-1"));
        let legacy_payload = fs::read_to_string(&closed_turn_path).expect("read closed turn");
        let legacy_payload = legacy_payload.replace("\"status\":\"Success\",", "");
        fs::write(&closed_turn_path, legacy_payload).expect("write legacy closed turn");

        let restored = coordinator
            .restore(history.session_id())
            .expect("restore legacy turn");
        assert_eq!(restored.closed_turns.len(), 1);
        assert_eq!(
            restored.closed_turns[0].tool_results[0].tool_result.status,
            freehand_contracts::ToolResultStatus::Success
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn restore_rejects_invalid_snapshot_coherence_explicitly() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let session_id = SessionId::new("session-1");
        let history = session_history();
        write_json_atomic(&coordinator.session_history_path(&session_id), &history)
            .expect("write history");
        write_json_atomic(
            &coordinator.cursor_path(&session_id),
            &ReasonPersistenceCursor {
                schema_version: PERSISTENCE_SCHEMA_VERSION,
                last_applied_reason_seq: 1,
                latest_turn_id: Some(TurnId::new("turn-1")),
                active_turn_id: Some(TurnId::new("turn-1")),
            },
        )
        .expect("write incoherent cursor");

        let err = coordinator
            .restore(&session_id)
            .expect_err("invalid snapshot coherence must fail recovery");
        assert_eq!(
            err,
            ReasonPersistenceError::InvalidCursorCoherence(
                "cursor references active turn but active-turn snapshot is missing".to_owned()
            )
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn restore_rejects_reason_ledger_sequence_gap_explicitly() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let turn = started_turn(&mut history);
        let turn_id = turn.request.turn_id.clone();
        let row = ReasonLedgerRow {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            seq: 2,
            created_at: 20,
            session_id: history.session_id().clone(),
            turn_id: Some(turn_id.clone()),
            cursor_after: ReasonPersistenceCursor {
                schema_version: PERSISTENCE_SCHEMA_VERSION,
                last_applied_reason_seq: 2,
                latest_turn_id: Some(turn_id.clone()),
                active_turn_id: Some(turn_id),
            },
            session_history: history.clone(),
            payload: ReasonLedgerPayload::TurnStarted {
                snapshot: ActiveTurnSnapshot {
                    turn,
                    schema_rejections: 0,
                },
            },
        };
        coordinator
            .append_row_only(history.session_id(), &row)
            .expect("append invalid gap row");

        let err = coordinator
            .restore(history.session_id())
            .expect_err("sequence gap must fail recovery");
        assert_eq!(
            err,
            ReasonPersistenceError::LedgerSequenceGap {
                expected: 1,
                actual: 2,
            }
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn restore_rejects_duplicate_reason_ledger_sequence_explicitly() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let turn = started_turn(&mut history);
        let turn_id = turn.request.turn_id.clone();
        let snapshot = ActiveTurnSnapshot {
            turn,
            schema_rejections: 0,
        };
        let first_row = ReasonLedgerRow {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            seq: 1,
            created_at: 30,
            session_id: history.session_id().clone(),
            turn_id: Some(turn_id.clone()),
            cursor_after: ReasonPersistenceCursor {
                schema_version: PERSISTENCE_SCHEMA_VERSION,
                last_applied_reason_seq: 1,
                latest_turn_id: Some(turn_id.clone()),
                active_turn_id: Some(turn_id.clone()),
            },
            session_history: history.clone(),
            payload: ReasonLedgerPayload::TurnStarted {
                snapshot: snapshot.clone(),
            },
        };
        let duplicate_row = ReasonLedgerRow {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            seq: 1,
            created_at: 31,
            session_id: history.session_id().clone(),
            turn_id: Some(turn_id.clone()),
            cursor_after: ReasonPersistenceCursor {
                schema_version: PERSISTENCE_SCHEMA_VERSION,
                last_applied_reason_seq: 1,
                latest_turn_id: Some(turn_id),
                active_turn_id: Some(TurnId::new("turn-1")),
            },
            session_history: history,
            payload: ReasonLedgerPayload::TurnStarted { snapshot },
        };
        coordinator
            .append_row_only(&first_row.session_id, &first_row)
            .expect("append first row");
        coordinator
            .append_row_only(&duplicate_row.session_id, &duplicate_row)
            .expect("append duplicate row");

        let err = coordinator
            .restore(&first_row.session_id)
            .expect_err("duplicate sequence must fail recovery");
        assert_eq!(
            err,
            ReasonPersistenceError::LedgerSequenceGap {
                expected: 2,
                actual: 1,
            }
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn restore_skips_stale_duplicate_row_when_later_expected_seq_exists() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let turn = started_turn(&mut history);
        let turn_id = turn.request.turn_id.clone();
        let snapshot = ActiveTurnSnapshot {
            turn,
            schema_rejections: 0,
        };
        let row_1 = ReasonLedgerRow {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            seq: 1,
            created_at: 40,
            session_id: history.session_id().clone(),
            turn_id: Some(turn_id.clone()),
            cursor_after: ReasonPersistenceCursor {
                schema_version: PERSISTENCE_SCHEMA_VERSION,
                last_applied_reason_seq: 1,
                latest_turn_id: Some(turn_id.clone()),
                active_turn_id: Some(turn_id.clone()),
            },
            session_history: history.clone(),
            payload: ReasonLedgerPayload::TurnStarted {
                snapshot: snapshot.clone(),
            },
        };
        let stale_duplicate = ReasonLedgerRow {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            seq: 1,
            created_at: 41,
            session_id: history.session_id().clone(),
            turn_id: Some(TurnId::new("stale-turn")),
            cursor_after: ReasonPersistenceCursor {
                schema_version: PERSISTENCE_SCHEMA_VERSION,
                last_applied_reason_seq: 1,
                latest_turn_id: Some(TurnId::new("stale-turn")),
                active_turn_id: Some(TurnId::new("stale-turn")),
            },
            session_history: history.clone(),
            payload: ReasonLedgerPayload::TurnStarted {
                snapshot: snapshot.clone(),
            },
        };
        let row_2 = ReasonLedgerRow {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            seq: 2,
            created_at: 42,
            session_id: history.session_id().clone(),
            turn_id: Some(turn_id.clone()),
            cursor_after: ReasonPersistenceCursor {
                schema_version: PERSISTENCE_SCHEMA_VERSION,
                last_applied_reason_seq: 2,
                latest_turn_id: Some(turn_id.clone()),
                active_turn_id: Some(turn_id),
            },
            session_history: history,
            payload: ReasonLedgerPayload::ProviderOutputApplied {
                output: ProviderSemanticOutput::SemanticEvent(ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    trace_id: TraceId::new("trace-1"),
                    feature_id: FeatureId::new("reason.persistence"),
                    agent_id: AgentId::new("agent-1"),
                    kind: SemanticEventKind::Text,
                    content: "continued".to_owned(),
                }),
                snapshot,
            },
        };

        coordinator
            .append_row_only(&row_1.session_id, &row_1)
            .expect("append first row");
        coordinator
            .append_row_only(&stale_duplicate.session_id, &stale_duplicate)
            .expect("append stale duplicate row");
        coordinator
            .append_row_only(&row_2.session_id, &row_2)
            .expect("append next authoritative row");

        let restored = coordinator
            .restore(&row_1.session_id)
            .expect("stale duplicate should be skipped when authoritative next seq exists");
        assert_eq!(restored.cursor.last_applied_reason_seq, 2);
        assert_eq!(
            restored.active_turn.expect("active").turn.request.turn_id,
            TurnId::new("turn-1")
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn ledger_only_rebuild_restores_state() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let turn = started_turn(&mut history);
        coordinator
            .record_turn_started(&history, &turn, 0)
            .expect("persist");

        fs::remove_dir_all(coordinator.session_dir(history.session_id()))
            .expect("remove snapshots");

        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert_eq!(restored.history, history);
        assert!(restored.active_turn.is_some());

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn provider_raw_only_debug_files_do_not_mask_missing_recovery_truth() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let session_id = SessionId::new("session-1");
        let provider_debug_path = runtime_home
            .join("ledgers")
            .join("providers")
            .join("anthropic")
            .join("agent-1")
            .join(session_id.as_str())
            .join("turn-1.jsonl");
        ensure_parent_dir(&provider_debug_path).expect("parent");
        fs::write(&provider_debug_path, "{\"raw\":\"provider event\"}\n").expect("write raw");

        let err = coordinator
            .restore(&session_id)
            .expect_err("provider raw only must not restore session truth");
        assert_eq!(
            err,
            ReasonPersistenceError::MissingRecoveryTruth(session_id.as_str().to_owned())
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn provider_raw_debug_files_do_not_become_session_truth() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let turn = started_turn(&mut history);
        coordinator
            .record_turn_started(&history, &turn, 0)
            .expect("persist");

        let provider_debug_path = runtime_home
            .join("ledgers")
            .join("providers")
            .join("anthropic")
            .join("agent-1")
            .join("session-1")
            .join("turn-1.jsonl");
        ensure_parent_dir(&provider_debug_path).expect("parent");
        fs::write(&provider_debug_path, "{\"raw\":\"provider event\"}\n").expect("write raw");

        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert_eq!(restored.closed_turns.len(), 0);
        assert!(restored.active_turn.is_some());

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn ui_sidecar_only_does_not_mask_missing_recovery_truth() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let session_id = SessionId::new("session-1");
        let sidecar_path = coordinator.ui_sidecar_path(&session_id);
        write_json_atomic(
            &sidecar_path,
            &PersistedSessionView {
                agent_id: AgentId::new("agent-1"),
                session_id: session_id.clone(),
                latest_turn_id: Some(TurnId::new("turn-sidecar")),
                active_turn_id: Some(TurnId::new("turn-sidecar")),
                projections: Vec::new(),
            },
        )
        .expect("write sidecar");

        let err = coordinator
            .restore(&session_id)
            .expect_err("ui sidecar only must not restore session truth");
        assert_eq!(
            err,
            ReasonPersistenceError::MissingRecoveryTruth(session_id.as_str().to_owned())
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn record_provider_raw_event_writes_separate_debug_ledger() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));

        coordinator
            .record_provider_raw_event(ProviderRawLedgerWrite {
                provider_family: ProviderFamily::Anthropic,
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                raw_kind: "response_body".to_owned(),
                scene: ProviderRawScenePosition {
                    crate_name: "freehand-provider-anthropic".to_owned(),
                    file: "src/lib.rs".to_owned(),
                    function: "AnthropicExecutor::execute_once_with_raw".to_owned(),
                    line: None,
                    raw_exchange_id: Some("response-body".to_owned()),
                },
                body: "{\"type\":\"message\"}".to_owned(),
                headers: BTreeMap::from([(
                    "content-type".to_owned(),
                    "application/json".to_owned(),
                )]),
            })
            .expect("write provider raw");

        let path = runtime_home
            .join("ledgers")
            .join("providers")
            .join("anthropic")
            .join("agent-1")
            .join("session-1")
            .join("turn-1.jsonl");
        let raw = fs::read_to_string(path).expect("read provider raw ledger");
        let rows = raw
            .lines()
            .map(|line| serde_json::from_str::<ProviderRawLedgerRow>(line).expect("decode row"))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].raw_kind, "response_body");
        assert_eq!(
            rows[0].scene.function,
            "AnthropicExecutor::execute_once_with_raw"
        );
        assert_eq!(rows[0].body, "{\"type\":\"message\"}");
        assert_eq!(
            rows[0].headers.get("content-type").map(String::as_str),
            Some("application/json")
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn atomic_snapshot_replace_overwrites_previous_state() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let turn = started_turn(&mut history);
        coordinator
            .record_turn_started(&history, &turn, 0)
            .expect("persist first");

        let mut updated_turn = turn.clone();
        updated_turn
            .semantic_events
            .push(ReasonResp01SemanticEvent {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("reason.persistence"),
                agent_id: AgentId::new("agent-1"),
                kind: SemanticEventKind::Reasoning,
                content: "second".to_owned(),
            });
        coordinator
            .record_provider_output_applied(
                &history,
                &updated_turn,
                &ProviderSemanticOutput::SemanticEvent(updated_turn.semantic_events[0].clone()),
                0,
            )
            .expect("persist second");

        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert_eq!(
            restored.active_turn.expect("active").turn.semantic_events[0].content,
            "second"
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn session_metadata_crud_persists_without_turn_truth_mutation() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let session_id = SessionId::new("session-metadata-1");

        let created = coordinator
            .create_session_metadata(
                session_id.clone(),
                Some("Draft title".to_owned()),
                Some("/tmp".to_owned()),
            )
            .expect("create metadata");
        assert_eq!(created.title.as_deref(), Some("Draft title"));
        assert!(!created.archived);

        let renamed = coordinator
            .rename_session(&session_id, "Renamed title".to_owned())
            .expect("rename metadata");
        assert_eq!(renamed.title.as_deref(), Some("Renamed title"));

        let archived = coordinator
            .archive_session(&session_id)
            .expect("archive metadata");
        assert!(archived.archived);

        let restored = coordinator
            .restore_session(&session_id)
            .expect("restore metadata");
        assert!(!restored.archived);

        let deleted = coordinator
            .delete_session(&session_id)
            .expect("delete archives metadata");
        assert!(deleted.archived);

        let reloaded =
            ReasonPersistence::new(&runtime_home, AgentId::new("agent-1")).load_session_metadata();
        let reloaded = reloaded.expect("reload metadata");
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].session_id, session_id);
        assert_eq!(reloaded[0].title.as_deref(), Some("Renamed title"));
        assert!(reloaded[0].archived);
        assert_eq!(
            coordinator
                .restore(&SessionId::new("session-metadata-1"))
                .expect_err("metadata must not become recovery truth"),
            ReasonPersistenceError::MissingRecoveryTruth("session-metadata-1".to_owned())
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn session_metadata_mutation_rejects_unknown_session() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let err = coordinator
            .rename_session(&SessionId::new("missing-session"), "Name".to_owned())
            .expect_err("unknown session must fail");
        assert_eq!(
            err,
            ReasonPersistenceError::SessionMetadataTargetNotFound("missing-session".to_owned())
        );
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn rollback_latest_session_turn_is_append_only_and_filters_effective_transcript() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        let mut first = started_turn_with_id(&mut history, "runtime-turn-1", "trace-1");
        first.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "first done".to_owned(),
        });
        coordinator
            .record_turn_closed(&history, &first, 0)
            .expect("persist first");

        let mut second = started_turn_with_id(&mut history, "runtime-turn-2", "trace-2");
        second.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-2"),
            trace_id: TraceId::new("trace-2"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "second precursor".to_owned(),
        });
        coordinator
            .record_turn_closed(&history, &second, 0)
            .expect("persist second");

        let mut second_continuation =
            started_turn_with_id(&mut history, "runtime-turn-2-r2", "trace-2-r2");
        second_continuation.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-2-r2"),
            trace_id: TraceId::new("trace-2-r2"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "second final".to_owned(),
        });
        coordinator
            .record_turn_closed(&history, &second_continuation, 0)
            .expect("persist continuation");

        let marker = coordinator
            .rollback_latest_session_turn(history.session_id())
            .expect("rollback latest");
        assert_eq!(marker.target_turn_id, TurnId::new("runtime-turn-2-r2"));
        assert_eq!(marker.target_logical_turn_key, "runtime-turn-2");
        assert_eq!(
            marker.previous_effective_head,
            Some(TurnId::new("runtime-turn-1"))
        );
        assert_eq!(marker.restored_user_text, "persist this");
        assert!(
            coordinator
                .rollback_markers_path(history.session_id())
                .is_file()
        );
        let persisted_markers = coordinator
            .load_session_rollback_markers(history.session_id())
            .expect("rollback marker sidecar");
        assert_eq!(persisted_markers, vec![marker.clone()]);

        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert_eq!(restored.closed_turns.len(), 1);
        assert_eq!(
            restored.closed_turns[0].request.turn_id,
            TurnId::new("runtime-turn-1")
        );
        assert!(
            coordinator
                .closed_turn_path(history.session_id(), &TurnId::new("runtime-turn-2"))
                .is_file()
        );
        assert!(
            coordinator
                .closed_turn_path(history.session_id(), &TurnId::new("runtime-turn-2-r2"))
                .is_file()
        );

        let ui_turns = coordinator
            .restore_turn_snapshots_for_ui(history.session_id())
            .expect("ui restore");
        assert_eq!(ui_turns.len(), 1);
        assert_eq!(ui_turns[0].request.turn_id, TurnId::new("runtime-turn-1"));
        let rows = coordinator
            .load_reason_ledger(history.session_id())
            .expect("ledger rows");
        assert!(
            rows.iter()
                .any(|row| matches!(row.payload, ReasonLedgerPayload::SessionRollback { .. }))
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn rollback_filters_model_visible_history_to_effective_turn_truth() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = SessionHistory::new(
            SessionId::new("session-1"),
            vec![
                stable_segment(
                    "memory-1",
                    ContextSegmentKind::SessionMemory,
                    "ordinary stable memory",
                ),
                historical_turn_segment(
                    "session-memory-runtime-turn-1",
                    "runtime-turn-1",
                    "effective historical turn",
                ),
                historical_turn_segment(
                    "session-memory-runtime-turn-2",
                    "runtime-turn-2",
                    "rolled-back historical turn",
                ),
                historical_turn_segment(
                    "session-memory-runtime-turn-99",
                    "runtime-turn-99",
                    "orphan historical turn",
                ),
            ],
        )
        .expect("history");

        for (turn_id, trace_id) in [("runtime-turn-1", "trace-1"), ("runtime-turn-2", "trace-2")] {
            let mut turn = started_turn_with_id(&mut history, turn_id, trace_id);
            turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
                session_id: history.session_id().clone(),
                turn_id: TurnId::new(turn_id),
                trace_id: TraceId::new(trace_id),
                feature_id: FeatureId::new("reason.persistence"),
                agent_id: AgentId::new("agent-1"),
                status: TerminalStatus::Success,
                summary: format!("{turn_id} done"),
            });
            coordinator
                .record_turn_closed(&history, &turn, 0)
                .expect("persist closed turn");
        }

        coordinator
            .rollback_latest_session_turn(history.session_id())
            .expect("rollback latest");

        let assert_effective_history = |restored: &RestoredReasonSession| {
            assert_eq!(
                restored
                    .history
                    .base_context_segments()
                    .iter()
                    .map(|segment| segment.segment_id.as_str())
                    .collect::<Vec<_>>(),
                vec!["memory-1", "session-memory-runtime-turn-1"]
            );
        };
        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert_effective_history(&restored);

        fs::remove_dir_all(coordinator.session_dir(history.session_id()))
            .expect("remove authoritative snapshots for ledger-only rebuild");
        let rebuilt = coordinator
            .restore(history.session_id())
            .expect("ledger-only restore");
        assert_effective_history(&rebuilt);

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn ui_restore_uses_complete_authoritative_snapshots_without_replaying_poisoned_ledger() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();

        let mut first = started_turn_with_id(&mut history, "runtime-turn-1", "trace-1");
        first.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "base round should be superseded".to_owned(),
        });
        coordinator
            .record_turn_closed(&history, &first, 0)
            .expect("persist base round");

        let mut repaired = started_turn_with_id(&mut history, "runtime-turn-1-r2", "trace-1-r2");
        repaired.semantic_events.push(ReasonResp01SemanticEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-1-r2"),
            trace_id: TraceId::new("trace-1-r2"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            kind: SemanticEventKind::Reasoning,
            content: "latest repaired round".to_owned(),
        });
        repaired.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-1-r2"),
            trace_id: TraceId::new("trace-1-r2"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "repaired round should remain".to_owned(),
        });
        coordinator
            .record_turn_closed(&history, &repaired, 0)
            .expect("persist repaired round");

        let mut second = started_turn_with_id(&mut history, "runtime-turn-2", "trace-2");
        second.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-2"),
            trace_id: TraceId::new("trace-2"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "second logical turn".to_owned(),
        });
        coordinator
            .record_turn_closed(&history, &second, 0)
            .expect("persist second turn");

        fs::write(
            coordinator.reason_ledger_path(history.session_id()),
            "{not valid ledger json}\n",
        )
        .expect("poison ledger to prove UI restore uses authoritative snapshots");

        let ui_turns = coordinator
            .restore_turn_snapshots_for_ui(history.session_id())
            .expect("ui restore from snapshots");
        assert_eq!(ui_turns.len(), 3);
        assert_eq!(ui_turns[0].request.turn_id, TurnId::new("runtime-turn-1"));
        assert_eq!(
            ui_turns[1].request.turn_id,
            TurnId::new("runtime-turn-1-r2")
        );
        assert_eq!(ui_turns[2].request.turn_id, TurnId::new("runtime-turn-2"));
        assert_eq!(
            ui_turns[1].semantic_events[0].content,
            "latest repaired round"
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn ui_restore_backfills_incomplete_authoritative_rounds_from_ledger() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();

        let mut first = started_turn_with_id(&mut history, "runtime-turn-1", "trace-1");
        first.semantic_events.push(ReasonResp01SemanticEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            kind: SemanticEventKind::Text,
            content: "first round tool precursor".to_owned(),
        });
        coordinator
            .record_turn_started(&history, &first, 0)
            .expect("persist first round active snapshot");

        let mut continuation =
            started_turn_with_id(&mut history, "runtime-turn-1-r2", "trace-1-r2");
        continuation.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-1-r2"),
            trace_id: TraceId::new("trace-1-r2"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "second round final".to_owned(),
        });
        coordinator
            .record_turn_started(&history, &continuation, 0)
            .expect("persist continuation active snapshot");
        coordinator
            .record_turn_closed(&history, &continuation, 0)
            .expect("persist only terminal round as authoritative closed truth");

        assert!(
            !coordinator
                .closed_turn_path(history.session_id(), &TurnId::new("runtime-turn-1"))
                .is_file(),
            "the authoritative closed-turn directory intentionally lacks the first round"
        );
        assert!(
            coordinator
                .closed_turn_path(history.session_id(), &TurnId::new("runtime-turn-1-r2"))
                .is_file()
        );

        let ui_turns = coordinator
            .restore_turn_snapshots_for_ui(history.session_id())
            .expect("ui restore backfills from ledger");
        assert_eq!(
            ui_turns
                .iter()
                .map(|turn| turn.request.turn_id.as_str())
                .collect::<Vec<_>>(),
            vec!["runtime-turn-1", "runtime-turn-1-r2"]
        );
        assert_eq!(
            ui_turns[0].semantic_events[0].content,
            "first round tool precursor"
        );
        assert_eq!(
            ui_turns[1]
                .terminal_event
                .as_ref()
                .map(|event| &event.status),
            Some(&TerminalStatus::Success)
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn restore_turn_start_snapshots_preserves_original_round_and_respects_rollback() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();

        let mut original = started_turn_with_id(&mut history, "runtime-turn-1", "trace-1");
        original.request.user_text = "original user objective".to_owned();
        coordinator
            .record_turn_started(&history, &original, 0)
            .expect("persist original turn start");

        let mut repaired = started_turn_with_id(&mut history, "runtime-turn-1-r2", "trace-1-r2");
        repaired.request.user_text = "internal repair prompt".to_owned();
        coordinator
            .record_turn_started(&history, &repaired, 0)
            .expect("persist repaired turn start");
        repaired.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("runtime-turn-1-r2"),
            trace_id: TraceId::new("trace-1-r2"),
            feature_id: FeatureId::new("reason.persistence"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Blocked,
            summary: "repair round closed".to_owned(),
        });
        coordinator
            .record_turn_closed(&history, &repaired, 0)
            .expect("persist repaired turn close");

        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert_eq!(restored.closed_turns.len(), 1);
        assert_eq!(
            restored.closed_turns[0].request.turn_id,
            TurnId::new("runtime-turn-1-r2")
        );
        let starts = coordinator
            .restore_turn_start_snapshots(history.session_id())
            .expect("restore turn starts");
        assert_eq!(
            starts
                .iter()
                .map(|turn| (
                    turn.request.turn_id.as_str(),
                    turn.request.user_text.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("runtime-turn-1", "original user objective"),
                ("runtime-turn-1-r2", "internal repair prompt"),
            ]
        );

        coordinator
            .rollback_latest_session_turn(history.session_id())
            .expect("rollback logical turn");
        assert!(
            coordinator
                .restore_turn_start_snapshots(history.session_id())
                .expect("restore turn starts after rollback")
                .is_empty(),
            "turn-start recovery must not resurrect rolled-back logical turns"
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn rollback_latest_session_turn_rejects_no_target_and_active_turn() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let session_id = SessionId::new("metadata-only-session");
        coordinator
            .create_session_metadata(session_id.clone(), Some("Metadata only".to_owned()), None)
            .expect("create metadata");

        let no_target = coordinator
            .rollback_latest_session_turn(&session_id)
            .expect_err("metadata-only rollback must fail");
        assert_eq!(
            no_target,
            ReasonPersistenceError::SessionRollbackTargetNotFound(
                "metadata-only-session".to_owned()
            )
        );

        let mut history = session_history();
        let active = started_turn_with_id(&mut history, "runtime-turn-active", "trace-active");
        coordinator
            .record_turn_started(&history, &active, 0)
            .expect("persist active turn");
        let active_err = coordinator
            .rollback_latest_session_turn(history.session_id())
            .expect_err("active turn rollback must fail");
        assert_eq!(
            active_err,
            ReasonPersistenceError::SessionRollbackActiveTurn("session-1".to_owned())
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }

    #[test]
    fn repeated_rollback_steps_backward_through_effective_turns_then_fails() {
        let runtime_home = temp_runtime_home();
        let coordinator = ReasonPersistence::new(&runtime_home, AgentId::new("agent-1"));
        let mut history = session_history();
        for (turn_id, trace_id, summary) in [
            ("runtime-turn-1", "trace-1", "first done"),
            ("runtime-turn-2", "trace-2", "second done"),
        ] {
            let mut turn = started_turn_with_id(&mut history, turn_id, trace_id);
            turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
                session_id: history.session_id().clone(),
                turn_id: TurnId::new(turn_id),
                trace_id: TraceId::new(trace_id),
                feature_id: FeatureId::new("reason.persistence"),
                agent_id: AgentId::new("agent-1"),
                status: TerminalStatus::Success,
                summary: summary.to_owned(),
            });
            coordinator
                .record_turn_closed(&history, &turn, 0)
                .expect("persist closed turn");
        }

        let latest = coordinator
            .rollback_latest_session_turn(history.session_id())
            .expect("rollback latest");
        assert_eq!(latest.target_turn_id, TurnId::new("runtime-turn-2"));

        let previous = coordinator
            .rollback_latest_session_turn(history.session_id())
            .expect("rollback previous effective turn");
        assert_eq!(previous.target_turn_id, TurnId::new("runtime-turn-1"));
        assert_eq!(previous.previous_effective_head, None);

        let exhausted = coordinator
            .rollback_latest_session_turn(history.session_id())
            .expect_err("all effective turns are rolled back");
        assert_eq!(
            exhausted,
            ReasonPersistenceError::SessionRollbackTargetNotFound("session-1".to_owned())
        );

        let restored = coordinator.restore(history.session_id()).expect("restore");
        assert!(restored.closed_turns.is_empty());
        assert!(
            coordinator
                .closed_turn_path(history.session_id(), &TurnId::new("runtime-turn-1"))
                .is_file()
        );
        assert!(
            coordinator
                .closed_turn_path(history.session_id(), &TurnId::new("runtime-turn-2"))
                .is_file()
        );

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }
}
