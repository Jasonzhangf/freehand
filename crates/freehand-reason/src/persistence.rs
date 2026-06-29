use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_blocks::CompletionSchemaRejection;
use freehand_contracts::{AgentId, SessionId, TraceId, TurnId};
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
    RewriteStateUpdated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonLedgerRow {
    pub schema_version: u32,
    pub seq: u64,
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

    pub fn restore_turn_snapshots_for_ui(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<TurnRecord>, ReasonPersistenceError> {
        let mut turns_by_id = BTreeMap::<TurnId, TurnRecord>::new();
        for row in self.load_reason_ledger(session_id)? {
            match row.payload {
                ReasonLedgerPayload::TurnStarted { snapshot }
                | ReasonLedgerPayload::ProviderOutputApplied { snapshot, .. }
                | ReasonLedgerPayload::CompletionRejected { snapshot, .. } => {
                    turns_by_id.insert(snapshot.turn.request.turn_id.clone(), snapshot.turn);
                }
                ReasonLedgerPayload::TurnClosed { turn, .. } => {
                    turns_by_id.insert(turn.request.turn_id.clone(), turn);
                }
                ReasonLedgerPayload::RewriteStateUpdated => {}
            }
        }
        if turns_by_id.is_empty() {
            let restored = self.restore(session_id)?;
            for turn in restored.closed_turns {
                turns_by_id.insert(turn.request.turn_id.clone(), turn);
            }
            if let Some(active) = restored.active_turn {
                turns_by_id.insert(active.turn.request.turn_id.clone(), active.turn);
            }
        }
        Ok(turns_by_id.into_values().collect())
    }

    fn persist_row(
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
        let cursor_after = ReasonPersistenceCursor {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            last_applied_reason_seq: next_seq,
            latest_turn_id: latest_turn_id.clone().or(current_cursor.latest_turn_id),
            active_turn_id: active_turn
                .as_ref()
                .map(|snapshot| snapshot.turn.request.turn_id.clone()),
        };
        let row = ReasonLedgerRow {
            schema_version: PERSISTENCE_SCHEMA_VERSION,
            seq: next_seq,
            session_id: history.session_id().clone(),
            turn_id: latest_turn_id,
            cursor_after: cursor_after.clone(),
            session_history: history.clone(),
            payload,
        };
        self.append_row_only(history.session_id(), &row)?;

        let mut closed_turns = self.load_closed_turns(history.session_id())?;
        if let Some(turn) = closed_turn {
            upsert_closed_turn(&mut closed_turns, turn);
        }

        let restored = RestoredReasonSession {
            history: history.clone(),
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
        let history = SessionHistory::load_from_path(self.session_history_path(session_id))
            .map_err(|err| ReasonPersistenceError::JsonParseFailed(err.to_string()))?;
        let cursor: ReasonPersistenceCursor = read_json_file(&self.cursor_path(session_id))?;
        let active_turn = if active_exists {
            Some(read_json_file(&self.active_turn_path(session_id))?)
        } else {
            None
        };
        let closed_turns = self.load_closed_turns(session_id)?;
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
                turns.push(read_json_file::<TurnRecord>(&entry.path())?);
            }
        }
        Ok(turns)
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

    fn reason_ledger_path(&self, session_id: &SessionId) -> PathBuf {
        self.runtime_home
            .join("ledgers")
            .join("reason")
            .join(self.agent_id.as_str())
            .join(format!("{}.jsonl", session_id.as_str()))
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
            restored.active_turn = Some(snapshot.clone());
        }
        ReasonLedgerPayload::TurnClosed { turn, .. } => {
            upsert_closed_turn(&mut restored.closed_turns, turn.clone());
            restored.active_turn = None;
        }
        ReasonLedgerPayload::RewriteStateUpdated => {}
    }
    validate_cursor(
        &restored.cursor,
        restored.active_turn.as_ref(),
        &restored.closed_turns,
    )
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
    serde_json::from_str(&payload)
        .map_err(|err| ReasonPersistenceError::JsonParseFailed(err.to_string()))
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
    use std::sync::atomic::{AtomicU64, Ordering};

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
            ContextSegmentKind::SessionMemory | ContextSegmentKind::SessionSummary => (
                ContextStability::SessionStable,
                ContextCachePolicy::Cacheable,
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
        ReasonTurnEngine::new()
            .start_turn(
                history,
                TurnStartInput {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    trace_id: TraceId::new("trace-1"),
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
        assert_eq!(
            restored.active_turn.expect("active").turn.request.turn_id,
            TurnId::new("turn-1")
        );

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
        assert!(
            coordinator
                .closed_turn_path(history.session_id(), &TurnId::new("turn-1"))
                .is_file()
        );

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
}
