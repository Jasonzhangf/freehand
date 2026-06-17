use std::fs;
use std::path::Path;

use freehand_blocks::{inspect_context_cache_diagnostics, validate_rewrite_base_segments};
use freehand_contracts::{ContextRewriteMode, ContextSegment, SessionId, TurnId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionHistory {
    session_id: SessionId,
    rewrite_version: u64,
    current_rewrite_mode: ContextRewriteMode,
    base_context_segments: Vec<ContextSegment>,
    rewrite_ledger: Vec<SessionRewriteRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRewriteRecord {
    pub rewrite_version: u64,
    pub rewrite_mode: ContextRewriteMode,
    pub reason: String,
    pub reference: Option<String>,
    pub applied_turn_id: Option<TurnId>,
    pub diagnostics: RewriteDiagnosticsSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewriteDiagnosticsSnapshot {
    pub rewrite_mode: ContextRewriteMode,
    pub rewrite_version: u64,
    pub stable_prefix_hash: String,
    pub stable_segment_hashes: Vec<String>,
    pub tool_schema_hash: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionHistoryError {
    #[error("session rewrite reason must not be empty")]
    EmptyRewriteReason,
    #[error("session rewrite failed: {0}")]
    RewriteRejected(String),
    #[error("session history json parse failed: {0}")]
    InvalidPersistedState(String),
    #[error("session history json render failed: {0}")]
    PersistedStateRenderFailed(String),
    #[error("session history file io failed: {0}")]
    FileIoFailed(String),
    #[error("persisted session history state is inconsistent: {0}")]
    InvalidPersistedCoherence(String),
}

impl SessionHistory {
    pub fn new(
        session_id: SessionId,
        base_context_segments: Vec<ContextSegment>,
    ) -> Result<Self, SessionHistoryError> {
        let base_context_segments = validate_rewrite_base_segments(&base_context_segments)
            .map_err(|err| SessionHistoryError::RewriteRejected(err.to_string()))?;
        Ok(Self {
            session_id,
            rewrite_version: 0,
            current_rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            base_context_segments,
            rewrite_ledger: Vec::new(),
        })
    }

    pub fn stage_compaction(
        &mut self,
        rewritten_base_segments: Vec<ContextSegment>,
        reason: impl Into<String>,
    ) -> Result<&SessionRewriteRecord, SessionHistoryError> {
        self.stage_rewrite(
            ContextRewriteMode::Compaction,
            rewritten_base_segments,
            reason,
            None,
        )
    }

    pub fn stage_rollback(
        &mut self,
        rewritten_base_segments: Vec<ContextSegment>,
        reason: impl Into<String>,
        reference_turn_id: TurnId,
    ) -> Result<&SessionRewriteRecord, SessionHistoryError> {
        self.stage_rewrite(
            ContextRewriteMode::Rollback,
            rewritten_base_segments,
            reason,
            Some(reference_turn_id.as_str().to_owned()),
        )
    }

    pub fn stage_resume_rebuild(
        &mut self,
        rewritten_base_segments: Vec<ContextSegment>,
        reason: impl Into<String>,
        resume_source: impl Into<String>,
    ) -> Result<&SessionRewriteRecord, SessionHistoryError> {
        self.stage_rewrite(
            ContextRewriteMode::ResumeRebuild,
            rewritten_base_segments,
            reason,
            Some(resume_source.into()),
        )
    }

    pub fn commit_turn_start(&mut self, turn_id: &TurnId) {
        if self.current_rewrite_mode == ContextRewriteMode::OrdinaryTurn {
            return;
        }
        if let Some(record) = self.rewrite_ledger.iter_mut().rev().find(|record| {
            record.rewrite_version == self.rewrite_version
                && record.rewrite_mode == self.current_rewrite_mode
                && record.applied_turn_id.is_none()
        }) {
            record.applied_turn_id = Some(turn_id.clone());
        }
        self.current_rewrite_mode = ContextRewriteMode::OrdinaryTurn;
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn rewrite_version(&self) -> u64 {
        self.rewrite_version
    }

    pub fn current_rewrite_mode(&self) -> ContextRewriteMode {
        self.current_rewrite_mode
    }

    pub fn base_context_segments(&self) -> &[ContextSegment] {
        &self.base_context_segments
    }

    pub fn rewrite_ledger(&self) -> &[SessionRewriteRecord] {
        &self.rewrite_ledger
    }

    pub fn persist_json(&self) -> Result<String, SessionHistoryError> {
        serde_json::to_string_pretty(self)
            .map_err(|err| SessionHistoryError::PersistedStateRenderFailed(err.to_string()))
    }

    pub fn from_persisted_json(input: &str) -> Result<Self, SessionHistoryError> {
        let mut history: Self = serde_json::from_str(input)
            .map_err(|err| SessionHistoryError::InvalidPersistedState(err.to_string()))?;
        history.validate_persisted_state()?;
        Ok(history)
    }

    pub fn persist_to_path(&self, path: impl AsRef<Path>) -> Result<(), SessionHistoryError> {
        let path = path.as_ref();
        let payload = self.persist_json()?;
        fs::write(path, payload).map_err(|err| SessionHistoryError::FileIoFailed(err.to_string()))
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, SessionHistoryError> {
        let payload = fs::read_to_string(path.as_ref())
            .map_err(|err| SessionHistoryError::FileIoFailed(err.to_string()))?;
        Self::from_persisted_json(&payload)
    }

    fn stage_rewrite(
        &mut self,
        rewrite_mode: ContextRewriteMode,
        rewritten_base_segments: Vec<ContextSegment>,
        reason: impl Into<String>,
        reference: Option<String>,
    ) -> Result<&SessionRewriteRecord, SessionHistoryError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(SessionHistoryError::EmptyRewriteReason);
        }
        let ordered_segments = validate_rewrite_base_segments(&rewritten_base_segments)
            .map_err(|err| SessionHistoryError::RewriteRejected(err.to_string()))?;
        let next_version = self.rewrite_version.saturating_add(1);
        let diagnostics =
            inspect_context_cache_diagnostics(&ordered_segments, rewrite_mode, next_version, None)
                .map_err(|err| SessionHistoryError::RewriteRejected(err.to_string()))?;

        self.base_context_segments = ordered_segments;
        self.rewrite_version = next_version;
        self.current_rewrite_mode = rewrite_mode;
        self.rewrite_ledger.push(SessionRewriteRecord {
            rewrite_version: next_version,
            rewrite_mode,
            reason: reason.trim().to_owned(),
            reference,
            applied_turn_id: None,
            diagnostics: RewriteDiagnosticsSnapshot {
                rewrite_mode: diagnostics.rewrite_mode,
                rewrite_version: diagnostics.rewrite_version,
                stable_prefix_hash: diagnostics.stable_prefix_hash,
                stable_segment_hashes: diagnostics.stable_segment_hashes,
                tool_schema_hash: diagnostics.tool_schema_hash,
            },
        });

        self.rewrite_ledger.last().ok_or_else(|| {
            SessionHistoryError::RewriteRejected("missing rewrite ledger".to_owned())
        })
    }

    fn validate_persisted_state(&mut self) -> Result<(), SessionHistoryError> {
        self.base_context_segments = validate_rewrite_base_segments(&self.base_context_segments)
            .map_err(|err| SessionHistoryError::InvalidPersistedCoherence(err.to_string()))?;
        if self.current_rewrite_mode == ContextRewriteMode::OrdinaryTurn {
            return Ok(());
        }
        let Some(last_record) = self.rewrite_ledger.last() else {
            return Err(SessionHistoryError::InvalidPersistedCoherence(
                "non-ordinary rewrite mode requires rewrite ledger evidence".to_owned(),
            ));
        };
        if last_record.rewrite_version != self.rewrite_version {
            return Err(SessionHistoryError::InvalidPersistedCoherence(
                "current rewrite version does not match latest rewrite ledger entry".to_owned(),
            ));
        }
        if last_record.rewrite_mode != self.current_rewrite_mode {
            return Err(SessionHistoryError::InvalidPersistedCoherence(
                "current rewrite mode does not match latest rewrite ledger entry".to_owned(),
            ));
        }
        if last_record.applied_turn_id.is_some() {
            return Err(SessionHistoryError::InvalidPersistedCoherence(
                "applied rewrite gate may not remain in non-ordinary mode".to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
        ContextSegmentKind, ContextStability,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

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
                source: "session_history".to_owned(),
                reference: None,
            },
        }
    }

    fn invalid_rewrite_segment(id: &str) -> ContextSegment {
        ContextSegment {
            segment_id: ContextSegmentId::new(id),
            kind: ContextSegmentKind::UserTurnInput,
            stability: ContextStability::TurnVolatile,
            cache_policy: ContextCachePolicy::NoCache,
            role: ContextRole::User,
            content: "user text must not enter rewrite base".to_owned(),
            token_budget: 16,
            provenance: ContextProvenance {
                source: "invalid-rewrite".to_owned(),
                reference: None,
            },
        }
    }

    #[test]
    fn compaction_bumps_rewrite_version_and_records_diagnostics() {
        let mut history =
            SessionHistory::new(SessionId::new("session-1"), Vec::new()).expect("new");
        history
            .stage_compaction(
                vec![stable_segment(
                    "summary-1",
                    ContextSegmentKind::SessionSummary,
                    "compact summary",
                )],
                "compact stale context",
            )
            .expect("rewrite");
        let record = history.rewrite_ledger.last().cloned().expect("record");

        assert_eq!(history.rewrite_version, 1);
        assert_eq!(history.current_rewrite_mode, ContextRewriteMode::Compaction);
        assert_eq!(record.rewrite_mode, ContextRewriteMode::Compaction);
        assert_eq!(record.diagnostics.rewrite_version, 1);
    }

    #[test]
    fn rollback_and_resume_rebuild_are_explicit_modes() {
        let mut history =
            SessionHistory::new(SessionId::new("session-1"), Vec::new()).expect("new");
        history
            .stage_rollback(
                vec![stable_segment(
                    "memory-1",
                    ContextSegmentKind::SessionMemory,
                    "rollback memory",
                )],
                "rollback to safe baseline",
                TurnId::new("turn-9"),
            )
            .expect("rollback");
        assert_eq!(history.current_rewrite_mode, ContextRewriteMode::Rollback);

        history.commit_turn_start(&TurnId::new("turn-10"));
        history
            .stage_resume_rebuild(
                vec![stable_segment(
                    "memory-2",
                    ContextSegmentKind::SessionMemory,
                    "resume memory",
                )],
                "resume after restart",
                "~/.freehand/state/turns/session-1.json",
            )
            .expect("resume");
        assert_eq!(
            history.current_rewrite_mode,
            ContextRewriteMode::ResumeRebuild
        );
        assert_eq!(history.rewrite_version, 2);
    }

    #[test]
    fn commit_turn_start_returns_mode_to_ordinary() {
        let mut history =
            SessionHistory::new(SessionId::new("session-1"), Vec::new()).expect("new");
        history
            .stage_compaction(
                vec![stable_segment(
                    "summary-1",
                    ContextSegmentKind::SessionSummary,
                    "compact summary",
                )],
                "compact stale context",
            )
            .expect("rewrite");
        history.commit_turn_start(&TurnId::new("turn-1"));

        assert_eq!(
            history.current_rewrite_mode,
            ContextRewriteMode::OrdinaryTurn
        );
        assert_eq!(history.rewrite_version, 1);
        assert_eq!(
            history.rewrite_ledger[0].applied_turn_id,
            Some(TurnId::new("turn-1"))
        );
    }

    #[test]
    fn persisted_json_round_trip_preserves_rewrite_truth() {
        let mut history = SessionHistory::new(
            SessionId::new("session-1"),
            vec![stable_segment(
                "memory-1",
                ContextSegmentKind::SessionMemory,
                "remember this",
            )],
        )
        .expect("new");
        history
            .stage_compaction(
                vec![stable_segment(
                    "summary-1",
                    ContextSegmentKind::SessionSummary,
                    "compact summary",
                )],
                "compact stale context",
            )
            .expect("rewrite");
        let rendered = history.persist_json().expect("json");
        let restored = SessionHistory::from_persisted_json(&rendered).expect("restored");

        assert_eq!(restored.rewrite_version, 1);
        assert_eq!(
            restored.current_rewrite_mode,
            ContextRewriteMode::Compaction
        );
        assert_eq!(restored.base_context_segments.len(), 1);
    }

    #[test]
    fn persisted_file_round_trip_preserves_session_history() {
        let history = SessionHistory::new(
            SessionId::new("session-1"),
            vec![stable_segment(
                "memory-1",
                ContextSegmentKind::SessionMemory,
                "remember this",
            )],
        )
        .expect("new");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("freehand-session-history-{unique}.json"));

        history.persist_to_path(&path).expect("persist");
        let restored = SessionHistory::load_from_path(&path).expect("load");
        fs::remove_file(&path).expect("cleanup");

        assert_eq!(restored.session_id, SessionId::new("session-1"));
        assert_eq!(restored.base_context_segments.len(), 1);
    }

    #[test]
    fn rejects_persisted_non_ordinary_mode_without_matching_ledger() {
        let err = SessionHistory::from_persisted_json(
            r#"{
  "session_id":"session-1",
  "rewrite_version":1,
  "current_rewrite_mode":"Compaction",
  "base_context_segments":[],
  "rewrite_ledger":[]
}"#,
        )
        .expect_err("should fail");
        assert!(matches!(
            err,
            SessionHistoryError::InvalidPersistedCoherence(message)
            if message.contains("rewrite ledger evidence")
        ));
    }

    #[test]
    fn rejects_empty_rewrite_reason_explicitly() {
        let mut history =
            SessionHistory::new(SessionId::new("session-1"), Vec::new()).expect("new");

        let err = history
            .stage_compaction(
                vec![stable_segment(
                    "summary-1",
                    ContextSegmentKind::SessionSummary,
                    "compact summary",
                )],
                "   ",
            )
            .expect_err("empty rewrite reason must fail");

        assert_eq!(err, SessionHistoryError::EmptyRewriteReason);
    }

    #[test]
    fn rejects_forbidden_rewrite_base_segments_explicitly() {
        let mut history =
            SessionHistory::new(SessionId::new("session-1"), Vec::new()).expect("new");

        let err = history
            .stage_compaction(
                vec![invalid_rewrite_segment("segment-user-turn")],
                "compact stale context",
            )
            .expect_err("forbidden rewrite base segment must fail");

        assert!(
            matches!(err, SessionHistoryError::RewriteRejected(message) if message.contains("user_turn_input"))
        );
    }

    #[test]
    fn rejects_invalid_persisted_json_explicitly() {
        let err =
            SessionHistory::from_persisted_json("{").expect_err("invalid persisted json must fail");

        assert!(matches!(err, SessionHistoryError::InvalidPersistedState(_)));
    }

    #[test]
    fn persist_to_path_reports_file_io_failure_explicitly() {
        let history = SessionHistory::new(SessionId::new("session-1"), Vec::new()).expect("new");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let parent = std::env::temp_dir().join(format!("freehand-session-parent-{unique}"));
        fs::write(&parent, "not-a-directory").expect("write parent file");
        let path = parent.join("session.json");

        let err = history
            .persist_to_path(&path)
            .expect_err("persist to non-directory parent must fail");

        assert!(matches!(err, SessionHistoryError::FileIoFailed(_)));
        fs::remove_file(&parent).expect("cleanup");
    }

    #[test]
    fn load_from_path_reports_file_io_failure_explicitly() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("freehand-session-missing-load-{unique}.json"));

        let err =
            SessionHistory::load_from_path(&path).expect_err("loading missing file must fail");

        assert!(matches!(err, SessionHistoryError::FileIoFailed(_)));
    }
}
