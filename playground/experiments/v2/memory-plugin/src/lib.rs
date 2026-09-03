use std::collections::HashSet;

use freehand_v2_contracts::SessionId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoryError {
    #[error("session not attached: {0}")]
    NotAttached(String),
    #[error("duplicate memory record: {0}")]
    Duplicate(String),
    #[error("memory summary cannot be empty")]
    EmptySummary,
    #[error("keyword cannot be empty")]
    EmptyKeyword,
    #[error("no memory records for session: {0}")]
    NoRecords(String),
    #[error("record id cannot be empty")]
    EmptyRecordId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryRecord {
    record_id: String,
    session_id: SessionId,
    summary: String,
    provenance: String,
    payload_ref: Option<String>,
}

impl MemoryRecord {
    pub fn new(
        record_id: impl Into<String>,
        session_id: SessionId,
        summary: impl Into<String>,
        provenance: impl Into<String>,
        payload_ref: Option<String>,
    ) -> Result<Self, MemoryError> {
        let record_id = record_id.into();
        let summary = summary.into();
        if record_id.is_empty() {
            return Err(MemoryError::EmptyRecordId);
        }
        if summary.is_empty() {
            return Err(MemoryError::EmptySummary);
        }
        Ok(Self {
            record_id,
            session_id,
            summary,
            provenance: provenance.into(),
            payload_ref,
        })
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    pub fn payload_ref(&self) -> Option<&str> {
        self.payload_ref.as_deref()
    }
}

#[derive(Default)]
pub struct MemoryPlugin {
    attached: HashSet<SessionId>,
    records: Vec<MemoryRecord>,
    revision: u64,
}

impl MemoryPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(&mut self, session_id: SessionId) {
        self.attached.insert(session_id);
        self.revision += 1;
    }

    pub fn summarize(&mut self, record: MemoryRecord) -> Result<(), MemoryError> {
        if !self.attached.contains(record.session_id()) {
            return Err(MemoryError::NotAttached(
                record.session_id().as_str().to_owned(),
            ));
        }
        if self
            .records
            .iter()
            .any(|r| r.record_id() == record.record_id())
        {
            return Err(MemoryError::Duplicate(record.record_id().to_owned()));
        }
        self.records.push(record);
        self.revision += 1;
        Ok(())
    }

    pub fn load(&self, session_id: &SessionId) -> Vec<&MemoryRecord> {
        self.records
            .iter()
            .filter(|r| r.session_id() == session_id)
            .collect()
    }

    pub fn search(&self, keyword: &str) -> Result<Vec<&MemoryRecord>, MemoryError> {
        if keyword.is_empty() {
            return Err(MemoryError::EmptyKeyword);
        }
        Ok(self
            .records
            .iter()
            .filter(|r| r.summary().contains(keyword))
            .collect())
    }

    pub fn export(&self, session_id: &SessionId) -> Result<Vec<&MemoryRecord>, MemoryError> {
        let records = self.load(session_id);
        if records.is_empty() {
            return Err(MemoryError::NoRecords(session_id.as_str().to_owned()));
        }
        Ok(records)
    }

    pub fn detach(&mut self, session_id: &SessionId) -> Result<(), MemoryError> {
        if !self.attached.remove(session_id) {
            return Err(MemoryError::NotAttached(session_id.as_str().to_owned()));
        }
        self.revision += 1;
        Ok(())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}
