use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SearchError {
    #[error("keyword cannot be empty")]
    EmptyKeyword,
    #[error("record id cannot be empty")]
    EmptyRecord,
    #[error("duplicate record: {0}")]
    Duplicate(String),
    #[error("unknown record: {0}")]
    Unknown(String),
    #[error("index is empty")]
    EmptyIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRecord {
    record_id: String,
    kind: String,
    source_identity: String,
    keywords: Vec<String>,
    payload_ref: Option<String>,
}

impl SearchRecord {
    pub fn new(
        record_id: impl Into<String>,
        kind: impl Into<String>,
        source_identity: impl Into<String>,
        keywords: Vec<String>,
        payload_ref: Option<String>,
    ) -> Result<Self, SearchError> {
        let record_id = record_id.into();
        if record_id.is_empty() {
            return Err(SearchError::EmptyRecord);
        }
        Ok(Self {
            record_id,
            kind: kind.into(),
            source_identity: source_identity.into(),
            keywords,
            payload_ref,
        })
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn source_identity(&self) -> &str {
        &self.source_identity
    }

    pub fn keywords(&self) -> &[String] {
        &self.keywords
    }

    pub fn payload_ref(&self) -> Option<&str> {
        self.payload_ref.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchResult {
    record: SearchRecord,
    classified_kind: String,
    score: u64,
}

impl SearchResult {
    pub fn record(&self) -> &SearchRecord {
        &self.record
    }

    pub fn classified_kind(&self) -> &str {
        &self.classified_kind
    }

    pub fn score(&self) -> u64 {
        self.score
    }
}

#[derive(Default)]
pub struct SearchPlugin {
    records: HashMap<String, SearchRecord>,
    cache: Option<Vec<SearchResult>>,
    revision: u64,
}

impl SearchPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn index(&mut self, record: SearchRecord) -> Result<(), SearchError> {
        if self.records.contains_key(record.record_id()) {
            return Err(SearchError::Duplicate(record.record_id().to_owned()));
        }
        self.records.insert(record.record_id().to_owned(), record);
        self.cache = None;
        self.revision += 1;
        Ok(())
    }

    pub fn query(&mut self, keyword: &str) -> Result<Vec<SearchResult>, SearchError> {
        if keyword.is_empty() {
            return Err(SearchError::EmptyKeyword);
        }
        if let Some(cache) = &self.cache {
            return Ok(cache.clone());
        }
        if self.records.is_empty() {
            return Err(SearchError::EmptyIndex);
        }
        let mut results: Vec<SearchResult> = self
            .records
            .values()
            .filter(|record| {
                record
                    .keywords()
                    .iter()
                    .any(|k| k.to_lowercase().contains(&keyword.to_lowercase()))
                    || record
                        .kind()
                        .to_lowercase()
                        .contains(&keyword.to_lowercase())
            })
            .map(|record| SearchResult {
                record: record.clone(),
                classified_kind: record.kind().to_owned(),
                score: record
                    .keywords()
                    .iter()
                    .filter(|k| k.contains(keyword))
                    .count() as u64,
            })
            .collect();
        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.record().record_id().cmp(b.record().record_id()))
        });
        self.cache = Some(results.clone());
        Ok(results)
    }

    pub fn invalidate(&mut self) {
        self.cache = None;
        self.revision += 1;
    }

    pub fn rebuild(&mut self, records: Vec<SearchRecord>) -> Result<(), SearchError> {
        self.records.clear();
        for record in records {
            self.index(record)?;
        }
        Ok(())
    }

    pub fn get(&self, record_id: &str) -> Option<&SearchRecord> {
        self.records.get(record_id)
    }
}
