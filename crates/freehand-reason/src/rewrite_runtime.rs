use freehand_blocks::{
    CompactionFollowUpDecision, CompactionFollowUpInput, CompactionTriggerDecision,
    CompactionTriggerInput, RecoveryRewriteDecision, RecoveryRewriteInput, RewritePolicyThresholds,
    assess_compaction_follow_up, decide_compaction_trigger, decide_recovery_rewrite,
    prompt_tokens_from_usage,
};
use freehand_contracts::{ContextSegment, TokenUsage, TurnId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{SessionHistory, SessionHistoryError, SessionRewriteRecord};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RewriteRuntimeState {
    soft_notice_emitted: bool,
    auto_compaction_paused: bool,
    consecutive_compactions: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionRewritePayload {
    pub rewritten_base_segments: Vec<ContextSegment>,
    pub rewrite_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionPolicyRequest {
    pub context_window_tokens: Option<u32>,
    pub prompt_tokens: Option<u32>,
    pub provider_usage: Option<TokenUsage>,
    pub estimated_stale_reclaim_tokens: Option<u32>,
    pub compaction_payload: Option<CompactionRewritePayload>,
    pub thresholds: RewritePolicyThresholds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionPolicyOutcome {
    pub decision: CompactionTriggerDecision,
    pub staged_record: Option<SessionRewriteRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackRewritePayload {
    pub rewritten_base_segments: Vec<ContextSegment>,
    pub rewrite_reason: String,
    pub reference_turn_id: TurnId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeRebuildPayload {
    pub rewritten_base_segments: Vec<ContextSegment>,
    pub rewrite_reason: String,
    pub resume_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPolicyRequest {
    pub decision_input: RecoveryRewriteInput,
    pub rollback_payload: Option<RollbackRewritePayload>,
    pub resume_rebuild_payload: Option<ResumeRebuildPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryPolicyOutcome {
    pub decision: RecoveryRewriteDecision,
    pub staged_record: Option<SessionRewriteRecord>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RewriteRuntimeError {
    #[error(
        "compaction policy received both prompt_tokens and provider_usage; provide one prompt usage source"
    )]
    ConflictingPromptUsageSources,
    #[error("provider usage cannot be used for rewrite policy: {0}")]
    InvalidProviderUsage(String),
    #[error("compaction policy selected rewrite but no compaction payload was provided")]
    MissingCompactionPayload,
    #[error("recovery policy selected rollback but no rollback payload was provided")]
    MissingRollbackPayload,
    #[error("recovery policy selected resume rebuild but no resume rebuild payload was provided")]
    MissingResumeRebuildPayload,
    #[error("session history rewrite failed: {0}")]
    SessionHistoryFailed(#[from] SessionHistoryError),
}

pub struct ReasonRewriteRuntime;

impl Default for ReasonRewriteRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ReasonRewriteRuntime {
    pub fn new() -> Self {
        Self
    }

    pub fn apply_compaction_policy(
        &self,
        history: &mut SessionHistory,
        state: &mut RewriteRuntimeState,
        request: CompactionPolicyRequest,
    ) -> Result<CompactionPolicyOutcome, RewriteRuntimeError> {
        let prompt_tokens =
            prompt_tokens_for_policy(request.prompt_tokens, request.provider_usage)?;
        let decision = decide_compaction_trigger(CompactionTriggerInput {
            current_rewrite_mode: history.current_rewrite_mode(),
            context_window_tokens: request.context_window_tokens,
            prompt_tokens,
            estimated_stale_reclaim_tokens: request.estimated_stale_reclaim_tokens,
            soft_notice_emitted: state.soft_notice_emitted,
            auto_compaction_paused: state.auto_compaction_paused,
            thresholds: request.thresholds,
        });

        use freehand_blocks::CompactionTriggerAction;
        use freehand_blocks::CompactionTriggerReason;

        let staged_record = match decision.action {
            CompactionTriggerAction::Hold => {
                if decision.reason == CompactionTriggerReason::BelowSoftThreshold {
                    state.reset();
                }
                None
            }
            CompactionTriggerAction::EmitSoftNotice => {
                state.soft_notice_emitted = true;
                None
            }
            CompactionTriggerAction::PruneStaleOnly => None,
            CompactionTriggerAction::StageCompaction { .. } => {
                let payload = request
                    .compaction_payload
                    .ok_or(RewriteRuntimeError::MissingCompactionPayload)?;
                Some(
                    history
                        .stage_compaction(payload.rewritten_base_segments, payload.rewrite_reason)?
                        .clone(),
                )
            }
        };

        Ok(CompactionPolicyOutcome {
            decision,
            staged_record,
        })
    }

    pub fn record_compaction_follow_up(
        &self,
        state: &mut RewriteRuntimeState,
        context_window_tokens: Option<u32>,
        post_compaction_prompt_tokens: Option<u32>,
        thresholds: RewritePolicyThresholds,
    ) -> CompactionFollowUpDecision {
        let next_consecutive_compactions = state.consecutive_compactions.saturating_add(1);
        let decision = assess_compaction_follow_up(CompactionFollowUpInput {
            context_window_tokens,
            post_compaction_prompt_tokens,
            consecutive_compactions: next_consecutive_compactions,
            thresholds,
        });

        use freehand_blocks::CompactionFollowUpAction;

        match decision.action {
            CompactionFollowUpAction::Hold => {}
            CompactionFollowUpAction::ResetAutoState => state.reset(),
            CompactionFollowUpAction::KeepAutoState => {
                state.soft_notice_emitted = false;
                state.consecutive_compactions = next_consecutive_compactions;
            }
            CompactionFollowUpAction::PauseAutoCompaction => {
                state.soft_notice_emitted = false;
                state.auto_compaction_paused = true;
                state.consecutive_compactions = next_consecutive_compactions;
            }
        }

        decision
    }

    pub fn apply_recovery_policy(
        &self,
        history: &mut SessionHistory,
        request: RecoveryPolicyRequest,
    ) -> Result<RecoveryPolicyOutcome, RewriteRuntimeError> {
        let decision = decide_recovery_rewrite(request.decision_input);

        use freehand_blocks::RecoveryRewriteAction;

        let staged_record = match decision.action {
            RecoveryRewriteAction::NoRewrite | RecoveryRewriteAction::Block => None,
            RecoveryRewriteAction::StageRollback => {
                let payload = request
                    .rollback_payload
                    .ok_or(RewriteRuntimeError::MissingRollbackPayload)?;
                Some(
                    history
                        .stage_rollback(
                            payload.rewritten_base_segments,
                            payload.rewrite_reason,
                            payload.reference_turn_id,
                        )?
                        .clone(),
                )
            }
            RecoveryRewriteAction::StageResumeRebuild => {
                let payload = request
                    .resume_rebuild_payload
                    .ok_or(RewriteRuntimeError::MissingResumeRebuildPayload)?;
                Some(
                    history
                        .stage_resume_rebuild(
                            payload.rewritten_base_segments,
                            payload.rewrite_reason,
                            payload.resume_source,
                        )?
                        .clone(),
                )
            }
        };

        Ok(RecoveryPolicyOutcome {
            decision,
            staged_record,
        })
    }
}

fn prompt_tokens_for_policy(
    prompt_tokens: Option<u32>,
    provider_usage: Option<TokenUsage>,
) -> Result<Option<u32>, RewriteRuntimeError> {
    match (prompt_tokens, provider_usage) {
        (Some(_), Some(_)) => Err(RewriteRuntimeError::ConflictingPromptUsageSources),
        (Some(tokens), None) => Ok(Some(tokens)),
        (None, Some(usage)) => prompt_tokens_from_usage(&usage)
            .map(Some)
            .map_err(|err| RewriteRuntimeError::InvalidProviderUsage(err.to_string())),
        (None, None) => Ok(None),
    }
}

impl RewriteRuntimeState {
    pub fn soft_notice_emitted(&self) -> bool {
        self.soft_notice_emitted
    }

    pub fn auto_compaction_paused(&self) -> bool {
        self.auto_compaction_paused
    }

    pub fn consecutive_compactions(&self) -> u8 {
        self.consecutive_compactions
    }

    fn reset(&mut self) {
        self.soft_notice_emitted = false;
        self.auto_compaction_paused = false;
        self.consecutive_compactions = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentId, FeatureId, ReasonTurnEngine, SessionId, TraceId, TurnId, TurnStartInput};
    use freehand_blocks::{
        CompactionFollowUpAction, CompactionTriggerAction, RecoveryRewriteAction, RestoreStatus,
        RewriteRegressionKind,
    };
    use freehand_contracts::{
        ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
        ContextSegmentKind, ContextStability,
    };

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
                source: "rewrite_runtime_test".to_owned(),
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
                "remember the repo shape",
            )],
        )
        .expect("session history")
    }

    fn thresholds() -> RewritePolicyThresholds {
        RewritePolicyThresholds::default()
    }

    fn start_input() -> TurnStartInput {
        TurnStartInput {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("reason.turn"),
            agent_id: AgentId::new("agent-1"),
            user_text: "continue".to_owned(),
            planned_context_segments: Vec::new(),
            model: "model-a".to_owned(),
        }
    }

    #[test]
    fn compaction_policy_stage_reaches_session_history_and_next_turn() {
        let runtime = ReasonRewriteRuntime::new();
        let engine = ReasonTurnEngine::new();
        let mut history = session_history();
        let mut state = RewriteRuntimeState::default();

        let outcome = runtime
            .apply_compaction_policy(
                &mut history,
                &mut state,
                CompactionPolicyRequest {
                    context_window_tokens: Some(100),
                    prompt_tokens: Some(80),
                    provider_usage: None,
                    estimated_stale_reclaim_tokens: Some(0),
                    compaction_payload: Some(CompactionRewritePayload {
                        rewritten_base_segments: vec![stable_segment(
                            "summary-1",
                            ContextSegmentKind::SessionSummary,
                            "compacted history",
                        )],
                        rewrite_reason: "compact stale context".to_owned(),
                    }),
                    thresholds: thresholds(),
                },
            )
            .expect("compaction outcome");

        assert_eq!(
            outcome.decision.action,
            CompactionTriggerAction::StageCompaction { force: false }
        );
        assert!(outcome.staged_record.is_some());
        assert_eq!(
            history.current_rewrite_mode(),
            freehand_contracts::ContextRewriteMode::Compaction
        );

        let turn = engine
            .start_turn(&mut history, start_input())
            .expect("turn after compaction");
        assert_eq!(turn.planned_context.diagnostics.rewrite_version, 1);
        assert_eq!(
            turn.planned_context.diagnostics.rewrite_mode,
            freehand_contracts::ContextRewriteMode::Compaction
        );
    }

    #[test]
    fn compaction_policy_requires_payload_when_rewrite_is_selected() {
        let runtime = ReasonRewriteRuntime::new();
        let mut history = session_history();
        let mut state = RewriteRuntimeState::default();

        let err = runtime
            .apply_compaction_policy(
                &mut history,
                &mut state,
                CompactionPolicyRequest {
                    context_window_tokens: Some(100),
                    prompt_tokens: Some(80),
                    provider_usage: None,
                    estimated_stale_reclaim_tokens: None,
                    compaction_payload: None,
                    thresholds: thresholds(),
                },
            )
            .expect_err("missing payload should fail");

        assert_eq!(err, RewriteRuntimeError::MissingCompactionPayload);
    }

    #[test]
    fn soft_notice_is_latched_without_rewrite() {
        let runtime = ReasonRewriteRuntime::new();
        let mut history = session_history();
        let mut state = RewriteRuntimeState::default();

        let outcome = runtime
            .apply_compaction_policy(
                &mut history,
                &mut state,
                CompactionPolicyRequest {
                    context_window_tokens: Some(100),
                    prompt_tokens: Some(60),
                    provider_usage: None,
                    estimated_stale_reclaim_tokens: None,
                    compaction_payload: None,
                    thresholds: thresholds(),
                },
            )
            .expect("soft notice");

        assert_eq!(
            outcome.decision.action,
            CompactionTriggerAction::EmitSoftNotice
        );
        assert!(state.soft_notice_emitted());
        assert_eq!(history.rewrite_version(), 0);
    }

    #[test]
    fn compaction_follow_up_pauses_auto_compaction_after_second_failure() {
        let runtime = ReasonRewriteRuntime::new();
        let mut state = RewriteRuntimeState::default();

        let first =
            runtime.record_compaction_follow_up(&mut state, Some(100), Some(82), thresholds());
        assert_eq!(first.action, CompactionFollowUpAction::KeepAutoState);
        assert_eq!(state.consecutive_compactions(), 1);
        assert!(!state.auto_compaction_paused());

        let second =
            runtime.record_compaction_follow_up(&mut state, Some(100), Some(82), thresholds());
        assert_eq!(second.action, CompactionFollowUpAction::PauseAutoCompaction);
        assert_eq!(state.consecutive_compactions(), 2);
        assert!(state.auto_compaction_paused());
    }

    #[test]
    fn compaction_policy_can_use_provider_usage_input_tokens() {
        let runtime = ReasonRewriteRuntime::new();
        let mut history = session_history();
        let mut state = RewriteRuntimeState::default();

        let outcome = runtime
            .apply_compaction_policy(
                &mut history,
                &mut state,
                CompactionPolicyRequest {
                    context_window_tokens: Some(100),
                    prompt_tokens: None,
                    provider_usage: Some(TokenUsage {
                        input_tokens: 60,
                        output_tokens: 5,
                        total_tokens: Some(65),
                        reasoning_tokens: None,
                        cache_creation_tokens: 10,
                        cache_read_tokens: 50,
                        finish_reason: Some("stop".to_owned()),
                    }),
                    estimated_stale_reclaim_tokens: None,
                    compaction_payload: None,
                    thresholds: thresholds(),
                },
            )
            .expect("usage-driven policy");

        assert_eq!(
            outcome.decision.action,
            CompactionTriggerAction::EmitSoftNotice
        );
    }

    #[test]
    fn compaction_policy_rejects_conflicting_prompt_usage_sources() {
        let runtime = ReasonRewriteRuntime::new();
        let mut history = session_history();
        let mut state = RewriteRuntimeState::default();

        let err = runtime
            .apply_compaction_policy(
                &mut history,
                &mut state,
                CompactionPolicyRequest {
                    context_window_tokens: Some(100),
                    prompt_tokens: Some(60),
                    provider_usage: Some(TokenUsage {
                        input_tokens: 60,
                        output_tokens: 5,
                        total_tokens: Some(65),
                        reasoning_tokens: None,
                        cache_creation_tokens: 10,
                        cache_read_tokens: 50,
                        finish_reason: Some("stop".to_owned()),
                    }),
                    estimated_stale_reclaim_tokens: None,
                    compaction_payload: None,
                    thresholds: thresholds(),
                },
            )
            .expect_err("conflicting usage source");

        assert_eq!(err, RewriteRuntimeError::ConflictingPromptUsageSources);
    }

    #[test]
    fn recovery_policy_stages_rollback_through_session_history() {
        let runtime = ReasonRewriteRuntime::new();
        let mut history = session_history();

        let outcome = runtime
            .apply_recovery_policy(
                &mut history,
                RecoveryPolicyRequest {
                    decision_input: RecoveryRewriteInput {
                        restore_status: RestoreStatus::RestoredCleanly,
                        latest_rewrite_regression: Some(
                            RewriteRegressionKind::ExplicitOperatorRequest,
                        ),
                        rollback_snapshot_available: true,
                        rebuild_source_available: true,
                    },
                    rollback_payload: Some(RollbackRewritePayload {
                        rewritten_base_segments: vec![stable_segment(
                            "memory-rollback",
                            ContextSegmentKind::SessionMemory,
                            "rollback memory",
                        )],
                        rewrite_reason: "rollback to last known good".to_owned(),
                        reference_turn_id: TurnId::new("turn-9"),
                    }),
                    resume_rebuild_payload: None,
                },
            )
            .expect("rollback");

        assert_eq!(
            outcome.decision.action,
            RecoveryRewriteAction::StageRollback
        );
        assert!(outcome.staged_record.is_some());
        assert_eq!(
            history.current_rewrite_mode(),
            freehand_contracts::ContextRewriteMode::Rollback
        );
    }

    #[test]
    fn recovery_policy_blocks_without_mutating_session_history() {
        let runtime = ReasonRewriteRuntime::new();
        let mut history = session_history();

        let outcome = runtime
            .apply_recovery_policy(
                &mut history,
                RecoveryPolicyRequest {
                    decision_input: RecoveryRewriteInput {
                        restore_status: RestoreStatus::PersistedStateMissing,
                        latest_rewrite_regression: None,
                        rollback_snapshot_available: false,
                        rebuild_source_available: false,
                    },
                    rollback_payload: None,
                    resume_rebuild_payload: None,
                },
            )
            .expect("block outcome");

        assert_eq!(outcome.decision.action, RecoveryRewriteAction::Block);
        assert!(outcome.staged_record.is_none());
        assert_eq!(history.rewrite_version(), 0);
        assert_eq!(
            history.current_rewrite_mode(),
            freehand_contracts::ContextRewriteMode::OrdinaryTurn
        );
    }
}
