//! Internal metadata center for control/provenance data.
//!
//! Metadata is not request content. This crate owns metadata envelopes,
//! writer provenance, write-node provenance, and validation before metadata
//! can be stored or forwarded to observers.

use freehand_contracts::{AgentId, FeatureId, SessionId, TraceId, TurnId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MetadataId(String);

impl MetadataId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataKind {
    Control,
    Routing,
    Provider,
    Cache,
    DebugLink,
    RuntimeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataWriteOwner {
    pub feature_id: FeatureId,
    pub crate_name: String,
    pub module_path: String,
    pub symbol_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataWriteNode {
    pub pipeline_node: String,
    pub runtime_node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataSubject {
    pub agent_id: Option<AgentId>,
    pub session_id: Option<SessionId>,
    pub turn_id: Option<TurnId>,
    pub trace_id: TraceId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataEnvelope {
    pub metadata_id: MetadataId,
    pub kind: MetadataKind,
    pub owner: MetadataWriteOwner,
    pub write_node: MetadataWriteNode,
    pub subject: MetadataSubject,
    pub entries: Vec<MetadataEntry>,
}

impl MetadataEnvelope {
    pub fn new(
        metadata_id: MetadataId,
        kind: MetadataKind,
        owner: MetadataWriteOwner,
        write_node: MetadataWriteNode,
        subject: MetadataSubject,
        entries: Vec<MetadataEntry>,
    ) -> Result<Self, MetadataError> {
        let envelope = Self {
            metadata_id,
            kind,
            owner,
            write_node,
            subject,
            entries,
        };
        validate_metadata_envelope(&envelope)?;
        Ok(envelope)
    }
}

#[derive(Debug, Default)]
pub struct MetadataCenter {
    records: Vec<MetadataEnvelope>,
}

impl MetadataCenter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(&mut self, envelope: MetadataEnvelope) -> Result<(), MetadataError> {
        validate_metadata_envelope(&envelope)?;
        self.records.push(envelope);
        Ok(())
    }

    pub fn records(&self) -> &[MetadataEnvelope] {
        &self.records
    }

    pub fn by_trace(&self, trace_id: &TraceId) -> Vec<&MetadataEnvelope> {
        self.records
            .iter()
            .filter(|record| &record.subject.trace_id == trace_id)
            .collect()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetadataError {
    #[error("metadata_id must be non-empty")]
    EmptyMetadataId,
    #[error("metadata owner feature_id must be non-empty")]
    EmptyOwnerFeatureId,
    #[error("metadata owner crate_name must be non-empty")]
    EmptyOwnerCrateName,
    #[error("metadata owner module_path must be non-empty")]
    EmptyOwnerModulePath,
    #[error("metadata owner symbol_path must be non-empty")]
    EmptyOwnerSymbolPath,
    #[error("metadata write_node pipeline_node must be non-empty")]
    EmptyPipelineNode,
    #[error("metadata subject trace_id must be non-empty")]
    EmptyTraceId,
    #[error("metadata entries must not be empty")]
    EmptyEntries,
    #[error("metadata entry key must be non-empty")]
    EmptyEntryKey,
    #[error("metadata entry key `{0}` is reserved for request data")]
    ReservedRequestDataKey(String),
}

pub fn validate_metadata_envelope(envelope: &MetadataEnvelope) -> Result<(), MetadataError> {
    require_non_empty(
        envelope.metadata_id.as_str(),
        MetadataError::EmptyMetadataId,
    )?;
    require_non_empty(
        envelope.owner.feature_id.as_str(),
        MetadataError::EmptyOwnerFeatureId,
    )?;
    require_non_empty(
        &envelope.owner.crate_name,
        MetadataError::EmptyOwnerCrateName,
    )?;
    require_non_empty(
        &envelope.owner.module_path,
        MetadataError::EmptyOwnerModulePath,
    )?;
    require_non_empty(
        &envelope.owner.symbol_path,
        MetadataError::EmptyOwnerSymbolPath,
    )?;
    require_non_empty(
        &envelope.write_node.pipeline_node,
        MetadataError::EmptyPipelineNode,
    )?;
    require_non_empty(
        envelope.subject.trace_id.as_str(),
        MetadataError::EmptyTraceId,
    )?;
    if envelope.entries.is_empty() {
        return Err(MetadataError::EmptyEntries);
    }
    for entry in &envelope.entries {
        require_non_empty(&entry.key, MetadataError::EmptyEntryKey)?;
        if is_reserved_request_key(&entry.key) {
            return Err(MetadataError::ReservedRequestDataKey(entry.key.clone()));
        }
    }
    Ok(())
}

fn require_non_empty(value: &str, err: MetadataError) -> Result<(), MetadataError> {
    if value.trim().is_empty() {
        Err(err)
    } else {
        Ok(())
    }
}

fn is_reserved_request_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "request" | "payload" | "prompt" | "messages" | "message" | "input" | "content" | "text"
    ) || normalized.starts_with("request.")
        || normalized.starts_with("payload.")
        || normalized.starts_with("prompt.")
        || normalized.starts_with("message.")
        || normalized.starts_with("input.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn metadata_envelope_records_writer_owner_and_write_node() {
        let envelope = sample_envelope();

        assert_eq!(envelope.owner.feature_id, FeatureId::new("reason.turn"));
        assert_eq!(envelope.owner.crate_name, "freehand-reason");
        assert_eq!(envelope.write_node.pipeline_node, "ReasonResp02Usage");
        assert_eq!(
            envelope.write_node.runtime_node_id.as_deref(),
            Some("master-node")
        );
    }

    #[test]
    fn metadata_center_writes_and_queries_by_trace() {
        let trace_id = TraceId::new("trace-1");
        let mut center = MetadataCenter::new();

        center.write(sample_envelope()).expect("write metadata");

        assert_eq!(center.records().len(), 1);
        assert_eq!(center.by_trace(&trace_id).len(), 1);
        assert!(center.by_trace(&TraceId::new("trace-2")).is_empty());
    }

    #[test]
    fn metadata_rejects_missing_owner() {
        let mut envelope = sample_envelope();
        envelope.owner.symbol_path = " ".to_owned();

        let err = validate_metadata_envelope(&envelope).expect_err("missing owner must fail");

        assert_eq!(err, MetadataError::EmptyOwnerSymbolPath);
    }

    #[test]
    fn metadata_rejects_missing_write_node() {
        let mut envelope = sample_envelope();
        envelope.write_node.pipeline_node.clear();

        let err = validate_metadata_envelope(&envelope).expect_err("missing node must fail");

        assert_eq!(err, MetadataError::EmptyPipelineNode);
    }

    #[test]
    fn metadata_rejects_request_data_keys() {
        let mut envelope = sample_envelope();
        envelope.entries = vec![MetadataEntry {
            key: "request.prompt".to_owned(),
            value: json!("do not put prompt text in metadata"),
        }];

        let err = validate_metadata_envelope(&envelope).expect_err("request-data key must fail");

        assert_eq!(
            err,
            MetadataError::ReservedRequestDataKey("request.prompt".to_owned())
        );
    }

    #[test]
    fn metadata_round_trips_as_json_without_request_payload_fields() {
        let envelope = sample_envelope();

        let encoded = serde_json::to_string(&envelope).expect("encode metadata");
        let decoded: MetadataEnvelope = serde_json::from_str(&encoded).expect("decode metadata");

        assert_eq!(decoded, envelope);
        assert!(!encoded.contains("user prompt"));
    }

    fn sample_envelope() -> MetadataEnvelope {
        MetadataEnvelope::new(
            MetadataId::new("meta-1"),
            MetadataKind::Cache,
            MetadataWriteOwner {
                feature_id: FeatureId::new("reason.turn"),
                crate_name: "freehand-reason".to_owned(),
                module_path: "freehand_reason".to_owned(),
                symbol_path: "ReasonTurnEngine::apply_provider_output".to_owned(),
            },
            MetadataWriteNode {
                pipeline_node: "ReasonResp02Usage".to_owned(),
                runtime_node_id: Some("master-node".to_owned()),
            },
            MetadataSubject {
                agent_id: Some(AgentId::new("master")),
                session_id: Some(SessionId::new("session-1")),
                turn_id: Some(TurnId::new("turn-1")),
                trace_id: TraceId::new("trace-1"),
            },
            vec![MetadataEntry {
                key: "cache.hit_rate".to_owned(),
                value: json!(0.5),
            }],
        )
        .expect("sample metadata")
    }
}
