use freehand_contracts::{ContextRewriteMode, TokenUsage};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewritePolicyThresholds {
    pub soft_compaction_ratio_bps: u16,
    pub auto_compaction_ratio_bps: u16,
    pub force_compaction_ratio_bps: u16,
    pub target_tail_ratio_bps: u16,
    pub max_tail_tokens: u32,
    pub max_consecutive_compactions: u8,
}

impl Default for RewritePolicyThresholds {
    fn default() -> Self {
        Self {
            soft_compaction_ratio_bps: 5_000,
            auto_compaction_ratio_bps: 8_000,
            force_compaction_ratio_bps: 9_000,
            target_tail_ratio_bps: 5_000,
            max_tail_tokens: 16_384,
            max_consecutive_compactions: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionTriggerInput {
    pub current_rewrite_mode: ContextRewriteMode,
    pub context_window_tokens: Option<u32>,
    pub prompt_tokens: Option<u32>,
    pub estimated_stale_reclaim_tokens: Option<u32>,
    pub soft_notice_emitted: bool,
    pub auto_compaction_paused: bool,
    pub thresholds: RewritePolicyThresholds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionTriggerAction {
    Hold,
    EmitSoftNotice,
    PruneStaleOnly,
    StageCompaction { force: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionTriggerReason {
    RewriteAlreadyPending,
    MissingContextWindow,
    MissingPromptUsage,
    BelowSoftThreshold,
    BetweenSoftAndAutoThreshold,
    AutoCompactionPaused,
    StalePruneCanClearThreshold,
    AutoCompactionThresholdReached,
    ForceCompactionThresholdReached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionTriggerDecision {
    pub action: CompactionTriggerAction,
    pub reason: CompactionTriggerReason,
    pub soft_threshold_tokens: Option<u32>,
    pub auto_threshold_tokens: Option<u32>,
    pub force_threshold_tokens: Option<u32>,
    pub target_tail_tokens: Option<u32>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RewritePolicyUsageError {
    #[error("provider usage input tokens must be greater than zero")]
    ZeroPromptTokens,
    #[error("provider usage input tokens exceed rewrite policy token range")]
    PromptTokensOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionFollowUpInput {
    pub context_window_tokens: Option<u32>,
    pub post_compaction_prompt_tokens: Option<u32>,
    pub consecutive_compactions: u8,
    pub thresholds: RewritePolicyThresholds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionFollowUpAction {
    Hold,
    ResetAutoState,
    KeepAutoState,
    PauseAutoCompaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactionFollowUpReason {
    MissingContextWindow,
    MissingPromptUsage,
    PromptDroppedBelowThreshold,
    PromptStillAboveThreshold,
    ConsecutiveCompactionLimitReached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionFollowUpDecision {
    pub action: CompactionFollowUpAction,
    pub reason: CompactionFollowUpReason,
    pub auto_threshold_tokens: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestoreStatus {
    FreshStart,
    RestoredCleanly,
    PersistedStateMissing,
    PersistedStateInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RewriteRegressionKind {
    ExplicitOperatorRequest,
    LatestRewriteReferenceInvalid,
    LatestRewriteAppliedTurnMissing,
    LatestRewriteSemanticRegression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRewriteInput {
    pub restore_status: RestoreStatus,
    pub latest_rewrite_regression: Option<RewriteRegressionKind>,
    pub rollback_snapshot_available: bool,
    pub rebuild_source_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryRewriteAction {
    NoRewrite,
    StageRollback,
    StageResumeRebuild,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryRewriteReason {
    HealthyState,
    RewriteRegressionWithRollbackSnapshot,
    RewriteRegressionNeedsResumeRebuild,
    RestoreMissingNeedsResumeRebuild,
    RestoreInvalidNeedsResumeRebuild,
    MissingRecoverySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRewriteDecision {
    pub action: RecoveryRewriteAction,
    pub reason: RecoveryRewriteReason,
}

pub fn decide_compaction_trigger(input: CompactionTriggerInput) -> CompactionTriggerDecision {
    if input.current_rewrite_mode != ContextRewriteMode::OrdinaryTurn {
        return CompactionTriggerDecision {
            action: CompactionTriggerAction::Hold,
            reason: CompactionTriggerReason::RewriteAlreadyPending,
            soft_threshold_tokens: None,
            auto_threshold_tokens: None,
            force_threshold_tokens: None,
            target_tail_tokens: None,
        };
    }

    let Some(context_window_tokens) = input.context_window_tokens.filter(|value| *value > 0) else {
        return CompactionTriggerDecision {
            action: CompactionTriggerAction::Hold,
            reason: CompactionTriggerReason::MissingContextWindow,
            soft_threshold_tokens: None,
            auto_threshold_tokens: None,
            force_threshold_tokens: None,
            target_tail_tokens: None,
        };
    };

    let Some(prompt_tokens) = input.prompt_tokens.filter(|value| *value > 0) else {
        return CompactionTriggerDecision {
            action: CompactionTriggerAction::Hold,
            reason: CompactionTriggerReason::MissingPromptUsage,
            soft_threshold_tokens: None,
            auto_threshold_tokens: None,
            force_threshold_tokens: None,
            target_tail_tokens: None,
        };
    };

    let soft_threshold = ratio_tokens(
        context_window_tokens,
        input.thresholds.soft_compaction_ratio_bps,
    );
    let auto_threshold = ratio_tokens(
        context_window_tokens,
        input.thresholds.auto_compaction_ratio_bps,
    );
    let force_threshold = ratio_tokens(
        context_window_tokens,
        input.thresholds.force_compaction_ratio_bps,
    );

    if prompt_tokens < soft_threshold {
        return CompactionTriggerDecision {
            action: CompactionTriggerAction::Hold,
            reason: CompactionTriggerReason::BelowSoftThreshold,
            soft_threshold_tokens: Some(soft_threshold),
            auto_threshold_tokens: Some(auto_threshold),
            force_threshold_tokens: Some(force_threshold),
            target_tail_tokens: None,
        };
    }

    if prompt_tokens < auto_threshold {
        return CompactionTriggerDecision {
            action: if input.soft_notice_emitted {
                CompactionTriggerAction::Hold
            } else {
                CompactionTriggerAction::EmitSoftNotice
            },
            reason: CompactionTriggerReason::BetweenSoftAndAutoThreshold,
            soft_threshold_tokens: Some(soft_threshold),
            auto_threshold_tokens: Some(auto_threshold),
            force_threshold_tokens: Some(force_threshold),
            target_tail_tokens: None,
        };
    }

    if input.auto_compaction_paused {
        return CompactionTriggerDecision {
            action: CompactionTriggerAction::Hold,
            reason: CompactionTriggerReason::AutoCompactionPaused,
            soft_threshold_tokens: Some(soft_threshold),
            auto_threshold_tokens: Some(auto_threshold),
            force_threshold_tokens: Some(force_threshold),
            target_tail_tokens: None,
        };
    }

    let force = prompt_tokens >= force_threshold;
    let estimated_stale_reclaim_tokens = input.estimated_stale_reclaim_tokens.unwrap_or(0);
    if !force
        && estimated_stale_reclaim_tokens > 0
        && prompt_tokens.saturating_sub(estimated_stale_reclaim_tokens) < auto_threshold
    {
        return CompactionTriggerDecision {
            action: CompactionTriggerAction::PruneStaleOnly,
            reason: CompactionTriggerReason::StalePruneCanClearThreshold,
            soft_threshold_tokens: Some(soft_threshold),
            auto_threshold_tokens: Some(auto_threshold),
            force_threshold_tokens: Some(force_threshold),
            target_tail_tokens: None,
        };
    }

    CompactionTriggerDecision {
        action: CompactionTriggerAction::StageCompaction { force },
        reason: if force {
            CompactionTriggerReason::ForceCompactionThresholdReached
        } else {
            CompactionTriggerReason::AutoCompactionThresholdReached
        },
        soft_threshold_tokens: Some(soft_threshold),
        auto_threshold_tokens: Some(auto_threshold),
        force_threshold_tokens: Some(force_threshold),
        target_tail_tokens: Some(compaction_target_tail_tokens(
            context_window_tokens,
            input.thresholds,
        )),
    }
}

pub fn prompt_tokens_from_usage(usage: &TokenUsage) -> Result<u32, RewritePolicyUsageError> {
    if usage.input_tokens == 0 {
        return Err(RewritePolicyUsageError::ZeroPromptTokens);
    }
    u32::try_from(usage.input_tokens).map_err(|_| RewritePolicyUsageError::PromptTokensOverflow)
}

pub fn assess_compaction_follow_up(input: CompactionFollowUpInput) -> CompactionFollowUpDecision {
    let Some(context_window_tokens) = input.context_window_tokens.filter(|value| *value > 0) else {
        return CompactionFollowUpDecision {
            action: CompactionFollowUpAction::Hold,
            reason: CompactionFollowUpReason::MissingContextWindow,
            auto_threshold_tokens: None,
        };
    };
    let Some(post_compaction_prompt_tokens) = input
        .post_compaction_prompt_tokens
        .filter(|value| *value > 0)
    else {
        return CompactionFollowUpDecision {
            action: CompactionFollowUpAction::Hold,
            reason: CompactionFollowUpReason::MissingPromptUsage,
            auto_threshold_tokens: None,
        };
    };

    let auto_threshold = ratio_tokens(
        context_window_tokens,
        input.thresholds.auto_compaction_ratio_bps,
    );
    if post_compaction_prompt_tokens < auto_threshold {
        return CompactionFollowUpDecision {
            action: CompactionFollowUpAction::ResetAutoState,
            reason: CompactionFollowUpReason::PromptDroppedBelowThreshold,
            auto_threshold_tokens: Some(auto_threshold),
        };
    }

    if input.consecutive_compactions >= input.thresholds.max_consecutive_compactions {
        return CompactionFollowUpDecision {
            action: CompactionFollowUpAction::PauseAutoCompaction,
            reason: CompactionFollowUpReason::ConsecutiveCompactionLimitReached,
            auto_threshold_tokens: Some(auto_threshold),
        };
    }

    CompactionFollowUpDecision {
        action: CompactionFollowUpAction::KeepAutoState,
        reason: CompactionFollowUpReason::PromptStillAboveThreshold,
        auto_threshold_tokens: Some(auto_threshold),
    }
}

pub fn decide_recovery_rewrite(input: RecoveryRewriteInput) -> RecoveryRewriteDecision {
    if let Some(_regression) = input.latest_rewrite_regression {
        if input.rollback_snapshot_available {
            return RecoveryRewriteDecision {
                action: RecoveryRewriteAction::StageRollback,
                reason: RecoveryRewriteReason::RewriteRegressionWithRollbackSnapshot,
            };
        }
        if input.rebuild_source_available {
            return RecoveryRewriteDecision {
                action: RecoveryRewriteAction::StageResumeRebuild,
                reason: RecoveryRewriteReason::RewriteRegressionNeedsResumeRebuild,
            };
        }
        return RecoveryRewriteDecision {
            action: RecoveryRewriteAction::Block,
            reason: RecoveryRewriteReason::MissingRecoverySource,
        };
    }

    match input.restore_status {
        RestoreStatus::FreshStart | RestoreStatus::RestoredCleanly => RecoveryRewriteDecision {
            action: RecoveryRewriteAction::NoRewrite,
            reason: RecoveryRewriteReason::HealthyState,
        },
        RestoreStatus::PersistedStateMissing if input.rebuild_source_available => {
            RecoveryRewriteDecision {
                action: RecoveryRewriteAction::StageResumeRebuild,
                reason: RecoveryRewriteReason::RestoreMissingNeedsResumeRebuild,
            }
        }
        RestoreStatus::PersistedStateInvalid if input.rebuild_source_available => {
            RecoveryRewriteDecision {
                action: RecoveryRewriteAction::StageResumeRebuild,
                reason: RecoveryRewriteReason::RestoreInvalidNeedsResumeRebuild,
            }
        }
        RestoreStatus::PersistedStateMissing | RestoreStatus::PersistedStateInvalid => {
            RecoveryRewriteDecision {
                action: RecoveryRewriteAction::Block,
                reason: RecoveryRewriteReason::MissingRecoverySource,
            }
        }
    }
}

fn ratio_tokens(context_window_tokens: u32, ratio_bps: u16) -> u32 {
    let value = (u64::from(context_window_tokens) * u64::from(ratio_bps)) / 10_000;
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn compaction_target_tail_tokens(
    context_window_tokens: u32,
    thresholds: RewritePolicyThresholds,
) -> u32 {
    ratio_tokens(context_window_tokens, thresholds.target_tail_ratio_bps)
        .min(thresholds.max_tail_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thresholds() -> RewritePolicyThresholds {
        RewritePolicyThresholds::default()
    }

    #[test]
    fn compaction_holds_below_soft_threshold() {
        let decision = decide_compaction_trigger(CompactionTriggerInput {
            current_rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            context_window_tokens: Some(100),
            prompt_tokens: Some(49),
            estimated_stale_reclaim_tokens: None,
            soft_notice_emitted: false,
            auto_compaction_paused: false,
            thresholds: thresholds(),
        });

        assert_eq!(decision.action, CompactionTriggerAction::Hold);
        assert_eq!(decision.reason, CompactionTriggerReason::BelowSoftThreshold);
    }

    #[test]
    fn compaction_emits_soft_notice_once() {
        let decision = decide_compaction_trigger(CompactionTriggerInput {
            current_rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            context_window_tokens: Some(100),
            prompt_tokens: Some(60),
            estimated_stale_reclaim_tokens: None,
            soft_notice_emitted: false,
            auto_compaction_paused: false,
            thresholds: thresholds(),
        });

        assert_eq!(decision.action, CompactionTriggerAction::EmitSoftNotice);
        assert_eq!(
            decision.reason,
            CompactionTriggerReason::BetweenSoftAndAutoThreshold
        );
    }

    #[test]
    fn compaction_prefers_stale_prune_when_it_clears_threshold() {
        let decision = decide_compaction_trigger(CompactionTriggerInput {
            current_rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            context_window_tokens: Some(100),
            prompt_tokens: Some(81),
            estimated_stale_reclaim_tokens: Some(5),
            soft_notice_emitted: true,
            auto_compaction_paused: false,
            thresholds: thresholds(),
        });

        assert_eq!(decision.action, CompactionTriggerAction::PruneStaleOnly);
        assert_eq!(
            decision.reason,
            CompactionTriggerReason::StalePruneCanClearThreshold
        );
    }

    #[test]
    fn compaction_stages_auto_compaction_at_threshold() {
        let decision = decide_compaction_trigger(CompactionTriggerInput {
            current_rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            context_window_tokens: Some(100),
            prompt_tokens: Some(80),
            estimated_stale_reclaim_tokens: Some(0),
            soft_notice_emitted: true,
            auto_compaction_paused: false,
            thresholds: thresholds(),
        });

        assert_eq!(
            decision.action,
            CompactionTriggerAction::StageCompaction { force: false }
        );
        assert_eq!(
            decision.reason,
            CompactionTriggerReason::AutoCompactionThresholdReached
        );
        assert_eq!(decision.target_tail_tokens, Some(50));
    }

    #[test]
    fn compaction_stages_force_compaction_at_force_threshold() {
        let decision = decide_compaction_trigger(CompactionTriggerInput {
            current_rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            context_window_tokens: Some(100),
            prompt_tokens: Some(95),
            estimated_stale_reclaim_tokens: Some(20),
            soft_notice_emitted: true,
            auto_compaction_paused: false,
            thresholds: thresholds(),
        });

        assert_eq!(
            decision.action,
            CompactionTriggerAction::StageCompaction { force: true }
        );
        assert_eq!(
            decision.reason,
            CompactionTriggerReason::ForceCompactionThresholdReached
        );
    }

    #[test]
    fn compaction_holds_when_rewrite_is_pending() {
        let decision = decide_compaction_trigger(CompactionTriggerInput {
            current_rewrite_mode: ContextRewriteMode::Compaction,
            context_window_tokens: Some(100),
            prompt_tokens: Some(95),
            estimated_stale_reclaim_tokens: Some(20),
            soft_notice_emitted: true,
            auto_compaction_paused: false,
            thresholds: thresholds(),
        });

        assert_eq!(decision.action, CompactionTriggerAction::Hold);
        assert_eq!(
            decision.reason,
            CompactionTriggerReason::RewriteAlreadyPending
        );
    }

    #[test]
    fn prompt_tokens_from_usage_reads_provider_input_tokens() {
        let usage = TokenUsage {
            input_tokens: 88,
            output_tokens: 12,
            total_tokens: Some(100),
            reasoning_tokens: None,
            cache_creation_tokens: 8,
            cache_read_tokens: 80,
            finish_reason: Some("stop".to_owned()),
        };

        assert_eq!(prompt_tokens_from_usage(&usage), Ok(88));
    }

    #[test]
    fn prompt_tokens_from_usage_rejects_zero_usage() {
        let usage = TokenUsage {
            input_tokens: 0,
            output_tokens: 12,
            total_tokens: Some(12),
            reasoning_tokens: None,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            finish_reason: Some("stop".to_owned()),
        };

        assert_eq!(
            prompt_tokens_from_usage(&usage),
            Err(RewritePolicyUsageError::ZeroPromptTokens)
        );
    }

    #[test]
    fn compaction_follow_up_pauses_after_two_ineffective_compactions() {
        let decision = assess_compaction_follow_up(CompactionFollowUpInput {
            context_window_tokens: Some(100),
            post_compaction_prompt_tokens: Some(82),
            consecutive_compactions: 2,
            thresholds: thresholds(),
        });

        assert_eq!(
            decision.action,
            CompactionFollowUpAction::PauseAutoCompaction
        );
        assert_eq!(
            decision.reason,
            CompactionFollowUpReason::ConsecutiveCompactionLimitReached
        );
    }

    #[test]
    fn recovery_prefers_rollback_when_snapshot_exists() {
        let decision = decide_recovery_rewrite(RecoveryRewriteInput {
            restore_status: RestoreStatus::RestoredCleanly,
            latest_rewrite_regression: Some(RewriteRegressionKind::ExplicitOperatorRequest),
            rollback_snapshot_available: true,
            rebuild_source_available: true,
        });

        assert_eq!(decision.action, RecoveryRewriteAction::StageRollback);
        assert_eq!(
            decision.reason,
            RecoveryRewriteReason::RewriteRegressionWithRollbackSnapshot
        );
    }

    #[test]
    fn recovery_uses_resume_rebuild_for_invalid_restore() {
        let decision = decide_recovery_rewrite(RecoveryRewriteInput {
            restore_status: RestoreStatus::PersistedStateInvalid,
            latest_rewrite_regression: None,
            rollback_snapshot_available: false,
            rebuild_source_available: true,
        });

        assert_eq!(decision.action, RecoveryRewriteAction::StageResumeRebuild);
        assert_eq!(
            decision.reason,
            RecoveryRewriteReason::RestoreInvalidNeedsResumeRebuild
        );
    }

    #[test]
    fn recovery_blocks_when_no_recovery_truth_exists() {
        let decision = decide_recovery_rewrite(RecoveryRewriteInput {
            restore_status: RestoreStatus::PersistedStateMissing,
            latest_rewrite_regression: None,
            rollback_snapshot_available: false,
            rebuild_source_available: false,
        });

        assert_eq!(decision.action, RecoveryRewriteAction::Block);
        assert_eq!(
            decision.reason,
            RecoveryRewriteReason::MissingRecoverySource
        );
    }
}
