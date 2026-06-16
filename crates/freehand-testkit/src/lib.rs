//! Shared mocks, fixtures, runtime harnesses, and replay helpers for Freehand tests.

use std::path::Path;

use freehand_blocks::{
    RecoveryRewriteAction, RecoveryRewriteInput, RestoreStatus, RewritePolicyThresholds,
};
use freehand_contracts::{
    AgentId, ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
    ContextSegmentKind, ContextStability, FeatureId, SessionId, TokenUsage, TraceId, TurnId,
};
use freehand_provider_core::ProviderSemanticOutput;
use freehand_reason::{
    CompactionPolicyOutcome, CompactionPolicyRequest, CompactionRewritePayload, ReasonPersistence,
    ReasonRewriteRuntime, ReasonTurnEngine, RecoveryPolicyOutcome, RecoveryPolicyRequest,
    ResumeRebuildPayload, RewriteRuntimeError, RewriteRuntimeState, SessionHistory, TurnRecord,
    TurnStartInput,
};
use thiserror::Error;

pub struct ReasonRuntimeHarness {
    engine: ReasonTurnEngine,
    rewrite_runtime: ReasonRewriteRuntime,
    rewrite_state: RewriteRuntimeState,
    history: SessionHistory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessTurnStart {
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub user_text: String,
    pub model: String,
    pub planned_context_segments: Vec<ContextSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageDrivenCompactionInput {
    pub context_window_tokens: Option<u32>,
    pub estimated_stale_reclaim_tokens: Option<u32>,
    pub compaction_payload: Option<CompactionRewritePayload>,
    pub thresholds: RewritePolicyThresholds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeRebuildHarnessInput {
    pub restore_status: RestoreStatus,
    pub resume_rebuild_payload: Option<ResumeRebuildPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTurnHarnessOutcome {
    pub turn: TurnRecord,
    pub latest_usage: Option<TokenUsage>,
    pub compaction_outcome: Option<CompactionPolicyOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonRuntimeSmokeScenario {
    UsageCompaction,
    RecoveryBlock,
}

impl ReasonRuntimeSmokeScenario {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UsageCompaction => "usage-compaction",
            Self::RecoveryBlock => "recovery-block",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "usage-compaction" => Some(Self::UsageCompaction),
            "recovery-block" => Some(Self::RecoveryBlock),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasonRuntimeSmokeReport {
    pub scenario: ReasonRuntimeSmokeScenario,
    pub rewrite_action: String,
    pub rewrite_version: u64,
    pub latest_usage_tokens: Option<u64>,
    pub blocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasonPersistenceSmokeReport {
    pub restored_terminal_summary: String,
    pub reason_seq: u64,
    pub ui_sidecar_exists: bool,
    pub session_index_entries: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReasonRuntimeHarnessError {
    #[error("turn start failed: {0}")]
    TurnStartFailed(String),
    #[error("rewrite runtime failed: {0}")]
    RewriteRuntimeFailed(String),
    #[error("reason persistence failed: {0}")]
    ReasonPersistenceFailed(String),
}

impl ReasonRuntimeHarness {
    pub fn new(
        session_id: SessionId,
        base_context_segments: Vec<ContextSegment>,
    ) -> Result<Self, ReasonRuntimeHarnessError> {
        let history = SessionHistory::new(session_id, base_context_segments)
            .map_err(|err| ReasonRuntimeHarnessError::RewriteRuntimeFailed(err.to_string()))?;
        Ok(Self {
            engine: ReasonTurnEngine::new(),
            rewrite_runtime: ReasonRewriteRuntime::new(),
            rewrite_state: RewriteRuntimeState::default(),
            history,
        })
    }

    pub fn run_provider_turn(
        &mut self,
        agent_id: AgentId,
        feature_id: FeatureId,
        start: HarnessTurnStart,
        provider_outputs: impl IntoIterator<Item = ProviderSemanticOutput>,
        compaction_input: Option<UsageDrivenCompactionInput>,
    ) -> Result<ProviderTurnHarnessOutcome, ReasonRuntimeHarnessError> {
        let session_id = self.history.session_id().clone();
        let mut turn = self
            .engine
            .start_turn(
                &mut self.history,
                TurnStartInput {
                    session_id,
                    turn_id: start.turn_id,
                    trace_id: start.trace_id,
                    feature_id,
                    agent_id,
                    user_text: start.user_text,
                    planned_context_segments: start.planned_context_segments,
                    model: start.model,
                },
            )
            .map_err(|err| ReasonRuntimeHarnessError::TurnStartFailed(err.to_string()))?;

        for output in provider_outputs {
            self.engine.apply_provider_output(&mut turn, output);
        }

        let latest_usage = turn.usage_events.last().map(|event| event.usage.clone());

        let compaction_outcome = match (latest_usage.clone(), compaction_input) {
            (Some(provider_usage), Some(input)) => Some(
                self.rewrite_runtime
                    .apply_compaction_policy(
                        &mut self.history,
                        &mut self.rewrite_state,
                        CompactionPolicyRequest {
                            context_window_tokens: input.context_window_tokens,
                            prompt_tokens: None,
                            provider_usage: Some(provider_usage),
                            estimated_stale_reclaim_tokens: input.estimated_stale_reclaim_tokens,
                            compaction_payload: input.compaction_payload,
                            thresholds: input.thresholds,
                        },
                    )
                    .map_err(map_rewrite_runtime_error)?,
            ),
            _ => None,
        };

        Ok(ProviderTurnHarnessOutcome {
            turn,
            latest_usage,
            compaction_outcome,
        })
    }

    pub fn apply_resume_rebuild(
        &mut self,
        input: ResumeRebuildHarnessInput,
    ) -> Result<RecoveryPolicyOutcome, ReasonRuntimeHarnessError> {
        self.rewrite_runtime
            .apply_recovery_policy(
                &mut self.history,
                RecoveryPolicyRequest {
                    decision_input: RecoveryRewriteInput {
                        restore_status: input.restore_status,
                        latest_rewrite_regression: None,
                        rollback_snapshot_available: false,
                        rebuild_source_available: input.resume_rebuild_payload.is_some(),
                    },
                    rollback_payload: None,
                    resume_rebuild_payload: input.resume_rebuild_payload,
                },
            )
            .map_err(map_rewrite_runtime_error)
    }

    pub fn history(&self) -> &SessionHistory {
        &self.history
    }

    pub fn rewrite_state(&self) -> &RewriteRuntimeState {
        &self.rewrite_state
    }
}

pub fn run_reason_runtime_smoke(
    agent_name: &str,
    scenario: ReasonRuntimeSmokeScenario,
) -> Result<ReasonRuntimeSmokeReport, ReasonRuntimeHarnessError> {
    let mut harness = ReasonRuntimeHarness::new(
        SessionId::new("session-smoke"),
        vec![stable_test_segment(
            "memory-smoke",
            ContextSegmentKind::SessionMemory,
            "remember the current smoke runtime state",
        )],
    )?;
    match scenario {
        ReasonRuntimeSmokeScenario::UsageCompaction => {
            let outcome = harness.run_provider_turn(
                AgentId::new(agent_name),
                FeatureId::new("app.cli-runtime-smoke"),
                HarnessTurnStart {
                    turn_id: TurnId::new("turn-smoke-1"),
                    trace_id: TraceId::new("trace-smoke-1"),
                    user_text: "continue".to_owned(),
                    model: "smoke-model".to_owned(),
                    planned_context_segments: Vec::new(),
                },
                [ProviderSemanticOutput::Usage(
                    freehand_contracts::ReasonResp02UsageEvent {
                        session_id: SessionId::new("session-smoke"),
                        turn_id: TurnId::new("turn-smoke-1"),
                        trace_id: TraceId::new("trace-smoke-1"),
                        feature_id: FeatureId::new("app.cli-runtime-smoke"),
                        agent_id: AgentId::new(agent_name),
                        usage: TokenUsage {
                            input_tokens: 80,
                            output_tokens: 4,
                            total_tokens: Some(84),
                            reasoning_tokens: None,
                            cache_creation_tokens: 10,
                            cache_read_tokens: 70,
                            finish_reason: Some("stop".to_owned()),
                        },
                    },
                )],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: Some(0),
                    compaction_payload: Some(CompactionRewritePayload {
                        rewritten_base_segments: vec![stable_test_segment(
                            "summary-smoke-1",
                            ContextSegmentKind::SessionSummary,
                            "compacted summary",
                        )],
                        rewrite_reason: "usage pressure compacted context".to_owned(),
                    }),
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )?;
            let decision = outcome.compaction_outcome.expect("compaction outcome");
            Ok(ReasonRuntimeSmokeReport {
                scenario,
                rewrite_action: format!("{:?}", decision.decision.action),
                rewrite_version: harness.history().rewrite_version(),
                latest_usage_tokens: outcome.latest_usage.map(|usage| usage.input_tokens),
                blocked: false,
            })
        }
        ReasonRuntimeSmokeScenario::RecoveryBlock => {
            let outcome = harness.apply_resume_rebuild(ResumeRebuildHarnessInput {
                restore_status: RestoreStatus::PersistedStateMissing,
                resume_rebuild_payload: None,
            })?;
            Ok(ReasonRuntimeSmokeReport {
                scenario,
                rewrite_action: format!("{:?}", outcome.decision.action),
                rewrite_version: harness.history().rewrite_version(),
                latest_usage_tokens: None,
                blocked: outcome.decision.action == RecoveryRewriteAction::Block,
            })
        }
    }
}

pub fn run_reason_persistence_smoke(
    agent_name: &str,
    runtime_home: impl AsRef<Path>,
) -> Result<ReasonPersistenceSmokeReport, ReasonRuntimeHarnessError> {
    let runtime_home = runtime_home.as_ref();
    let agent_id = AgentId::new(agent_name);
    let session_id = SessionId::new("session-persist-smoke");
    let turn_id = TurnId::new("turn-persist-smoke-1");
    let trace_id = TraceId::new("trace-persist-smoke-1");
    let feature_id = FeatureId::new("reason.persistence");
    let mut history = SessionHistory::new(
        session_id.clone(),
        vec![stable_test_segment(
            "memory-persist-smoke",
            ContextSegmentKind::SessionMemory,
            "remember persisted smoke state",
        )],
    )
    .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;
    let persistence = ReasonPersistence::new(runtime_home, agent_id.clone());
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: trace_id.clone(),
                feature_id: feature_id.clone(),
                agent_id: agent_id.clone(),
                user_text: "persist the latest terminal summary".to_owned(),
                planned_context_segments: Vec::new(),
                model: "smoke-model".to_owned(),
            },
        )
        .map_err(|err| ReasonRuntimeHarnessError::TurnStartFailed(err.to_string()))?;
    persistence
        .record_turn_started(&history, &turn, 0)
        .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;

    let output =
        ProviderSemanticOutput::SemanticEvent(freehand_contracts::ReasonResp01SemanticEvent {
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            trace_id: trace_id.clone(),
            feature_id: feature_id.clone(),
            agent_id: agent_id.clone(),
            kind: freehand_contracts::SemanticEventKind::Text,
            content: "persisted smoke output".to_owned(),
        });
    engine.apply_provider_output(&mut turn, output.clone());
    persistence
        .record_provider_output_applied(&history, &turn, &output, 0)
        .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;

    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id,
        feature_id,
        agent_id,
        status: freehand_contracts::TerminalStatus::Success,
        summary: "persisted smoke terminal".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;

    let restored = persistence
        .restore(&session_id)
        .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))?;
    let index_path = runtime_home
        .join("cache")
        .join("session-index")
        .join(format!("{agent_name}.json"));
    let index_entries: Vec<freehand_reason::PersistedSessionIndexEntry> =
        std::fs::read_to_string(&index_path)
            .map_err(|err| ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string()))
            .and_then(|payload| {
                serde_json::from_str(&payload).map_err(|err| {
                    ReasonRuntimeHarnessError::ReasonPersistenceFailed(err.to_string())
                })
            })?;

    Ok(ReasonPersistenceSmokeReport {
        restored_terminal_summary: restored.closed_turns[0]
            .terminal_event
            .as_ref()
            .expect("terminal")
            .summary
            .clone(),
        reason_seq: restored.cursor.last_applied_reason_seq,
        ui_sidecar_exists: runtime_home
            .join("state")
            .join("ui")
            .join(agent_name)
            .join("session-persist-smoke.json")
            .is_file(),
        session_index_entries: index_entries.len(),
    })
}

pub fn stable_test_segment(id: &str, kind: ContextSegmentKind, content: &str) -> ContextSegment {
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
        _ => panic!("stable_test_segment requires a stable/session-stable segment kind"),
    };
    ContextSegment {
        segment_id: ContextSegmentId::new(id),
        kind,
        stability,
        cache_policy,
        role,
        content: content.to_owned(),
        token_budget: 128,
        provenance: ContextProvenance {
            source: "freehand_testkit".to_owned(),
            reference: None,
        },
    }
}

fn map_rewrite_runtime_error(err: RewriteRuntimeError) -> ReasonRuntimeHarnessError {
    ReasonRuntimeHarnessError::RewriteRuntimeFailed(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_blocks::{CompactionTriggerAction, RecoveryRewriteAction};
    use freehand_contracts::{
        ReasonResp02UsageEvent, SemanticEventKind, ToolCallContract, ToolCallId,
    };
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_runtime_home() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("freehand-testkit-runtime-{stamp}-{counter}"))
    }

    fn harness() -> ReasonRuntimeHarness {
        ReasonRuntimeHarness::new(
            SessionId::new("session-1"),
            vec![stable_test_segment(
                "memory-1",
                ContextSegmentKind::SessionMemory,
                "remember workspace state",
            )],
        )
        .expect("harness")
    }

    fn turn_start(turn_id: &str) -> HarnessTurnStart {
        HarnessTurnStart {
            turn_id: TurnId::new(turn_id),
            trace_id: TraceId::new(format!("{turn_id}-trace")),
            user_text: "continue".to_owned(),
            model: "model-a".to_owned(),
            planned_context_segments: Vec::new(),
        }
    }

    fn usage_output(input_tokens: u64) -> ProviderSemanticOutput {
        ProviderSemanticOutput::Usage(ReasonResp02UsageEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("turn-1-trace"),
            feature_id: FeatureId::new("reason.rewrite-policy"),
            agent_id: AgentId::new("agent-1"),
            usage: TokenUsage {
                input_tokens,
                output_tokens: 4,
                total_tokens: Some(input_tokens + 4),
                reasoning_tokens: None,
                cache_creation_tokens: 10,
                cache_read_tokens: input_tokens.saturating_sub(10),
                finish_reason: Some("stop".to_owned()),
            },
        })
    }

    #[test]
    fn provider_usage_drives_compaction_in_project_harness() {
        let mut harness = harness();
        let outcome = harness
            .run_provider_turn(
                AgentId::new("agent-1"),
                FeatureId::new("reason.rewrite-policy"),
                turn_start("turn-1"),
                [usage_output(80)],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: Some(0),
                    compaction_payload: Some(CompactionRewritePayload {
                        rewritten_base_segments: vec![stable_test_segment(
                            "summary-1",
                            ContextSegmentKind::SessionSummary,
                            "compacted summary",
                        )],
                        rewrite_reason: "usage pressure compacted context".to_owned(),
                    }),
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )
            .expect("run turn");

        assert_eq!(
            outcome
                .compaction_outcome
                .expect("compaction")
                .decision
                .action,
            CompactionTriggerAction::StageCompaction { force: false }
        );
        assert_eq!(harness.history().rewrite_version(), 1);
    }

    #[test]
    fn provider_usage_can_prefer_stale_prune_without_rewrite() {
        let mut harness = harness();
        let outcome = harness
            .run_provider_turn(
                AgentId::new("agent-1"),
                FeatureId::new("reason.rewrite-policy"),
                turn_start("turn-1"),
                [usage_output(81)],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: Some(5),
                    compaction_payload: None,
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )
            .expect("run turn");

        assert_eq!(
            outcome
                .compaction_outcome
                .expect("compaction")
                .decision
                .action,
            CompactionTriggerAction::PruneStaleOnly
        );
        assert_eq!(harness.history().rewrite_version(), 0);
    }

    #[test]
    fn resume_rebuild_blocks_when_source_is_missing() {
        let mut harness = harness();
        let outcome = harness
            .apply_resume_rebuild(ResumeRebuildHarnessInput {
                restore_status: RestoreStatus::PersistedStateMissing,
                resume_rebuild_payload: None,
            })
            .expect("block outcome");

        assert_eq!(outcome.decision.action, RecoveryRewriteAction::Block);
        assert_eq!(harness.history().rewrite_version(), 0);
    }

    #[test]
    fn provider_text_event_does_not_trigger_rewrite_without_usage() {
        let mut harness = harness();
        let outcome = harness
            .run_provider_turn(
                AgentId::new("agent-1"),
                FeatureId::new("reason.rewrite-policy"),
                turn_start("turn-1"),
                [ProviderSemanticOutput::SemanticEvent(
                    freehand_contracts::ReasonResp01SemanticEvent {
                        session_id: SessionId::new("session-1"),
                        turn_id: TurnId::new("turn-1"),
                        trace_id: TraceId::new("turn-1-trace"),
                        feature_id: FeatureId::new("reason.rewrite-policy"),
                        agent_id: AgentId::new("agent-1"),
                        kind: SemanticEventKind::Text,
                        content: "answer".to_owned(),
                    },
                )],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: None,
                    compaction_payload: None,
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )
            .expect("run turn");

        assert!(outcome.compaction_outcome.is_none());
        assert_eq!(harness.history().rewrite_version(), 0);
    }

    #[test]
    fn tool_call_without_usage_does_not_trigger_rewrite() {
        let mut harness = harness();
        let outcome = harness
            .run_provider_turn(
                AgentId::new("agent-1"),
                FeatureId::new("reason.rewrite-policy"),
                turn_start("turn-1"),
                [ProviderSemanticOutput::ToolCall(
                    freehand_contracts::ReasonReq04ToolCall {
                        session_id: SessionId::new("session-1"),
                        turn_id: TurnId::new("turn-1"),
                        trace_id: TraceId::new("turn-1-trace"),
                        feature_id: FeatureId::new("reason.rewrite-policy"),
                        agent_id: AgentId::new("agent-1"),
                        tool_call: ToolCallContract {
                            tool_call_id: ToolCallId::new("tool-1"),
                            tool_name: "search".to_owned(),
                            arguments: Vec::new(),
                            arguments_complete: true,
                        },
                    },
                )],
                Some(UsageDrivenCompactionInput {
                    context_window_tokens: Some(100),
                    estimated_stale_reclaim_tokens: None,
                    compaction_payload: None,
                    thresholds: RewritePolicyThresholds::default(),
                }),
            )
            .expect("run turn");

        assert!(outcome.compaction_outcome.is_none());
        assert_eq!(harness.history().rewrite_version(), 0);
    }

    #[test]
    fn reason_persistence_smoke_recovers_terminal_turn() {
        let runtime_home = temp_runtime_home();
        let report =
            run_reason_persistence_smoke("agent-1", &runtime_home).expect("persistence smoke");

        assert_eq!(report.restored_terminal_summary, "persisted smoke terminal");
        assert_eq!(report.reason_seq, 3);
        assert!(report.ui_sidecar_exists);
        assert_eq!(report.session_index_entries, 1);

        std::fs::remove_dir_all(runtime_home).expect("cleanup");
    }
}
