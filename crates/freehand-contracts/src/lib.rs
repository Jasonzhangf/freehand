//! Global semantic contracts and pipeline node types for Freehand.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

id_type!(AgentId);
id_type!(SessionId);
id_type!(TurnId);
id_type!(TraceId);
id_type!(FeatureId);
id_type!(ToolCallId);
id_type!(ContextSegmentId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextSegmentKind {
    SystemAnchor,
    DeveloperPolicy,
    SessionMemory,
    SessionSummary,
    SubagentConclusion,
    ToolResultEvidence,
    UserTurnInput,
    CompletionContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextStability {
    Stable,
    SessionStable,
    TurnVolatile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextCachePolicy {
    CacheAnchor,
    Cacheable,
    NoCache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextRewriteMode {
    OrdinaryTurn,
    Compaction,
    Rollback,
    ResumeRebuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextRole {
    System,
    Developer,
    User,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextProvenance {
    pub source: String,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSegment {
    pub segment_id: ContextSegmentId,
    pub kind: ContextSegmentKind,
    pub stability: ContextStability,
    pub cache_policy: ContextCachePolicy,
    pub role: ContextRole,
    pub content: String,
    pub token_budget: u32,
    pub provenance: ContextProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonReq01UserRawInput {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonReq02ContextComposedInput {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub user_text: String,
    pub context_segments: Vec<ContextSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonReq03ProviderPayload {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub model: String,
    pub input_segments: Vec<ContextSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolArgument {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallContract {
    pub tool_call_id: ToolCallId,
    pub tool_name: String,
    pub arguments: Vec<ToolArgument>,
    pub arguments_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonReq04ToolCall {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub tool_call: ToolCallContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultContract {
    pub tool_call_id: ToolCallId,
    pub output: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolPreviewChangeKind {
    Create,
    Modify,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPreviewFileChange {
    pub locked_path: String,
    pub kind: ToolPreviewChangeKind,
    pub before_text: Option<String>,
    pub after_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPreviewContract {
    pub tool_call_id: ToolCallId,
    pub changes: Vec<ToolPreviewFileChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonReq05ToolResultReentry {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub tool_result: ToolResultContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub finish_reason: Option<String>,
}

impl TokenUsage {
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_creation_tokens + self.cache_read_tokens;
        if total == 0 {
            0.0
        } else {
            self.cache_read_tokens as f64 / total as f64
        }
    }

    pub fn resolved_total_tokens(&self) -> u64 {
        self.total_tokens
            .unwrap_or(self.input_tokens.saturating_add(self.output_tokens))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalStatus {
    Success,
    ToolPending,
    Blocked,
    Interrupted,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticEventKind {
    Reasoning,
    Text,
    ToolCall,
    ToolResult,
    Usage,
    Terminal,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonResp01SemanticEvent {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub kind: SemanticEventKind,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonResp02UsageEvent {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonResp03TerminalEvent {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: AgentId,
    pub status: TerminalStatus,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryPolicy {
    Recoverable,
    Unrecoverable,
    PeriodicRecoverable { retry_after_seconds: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorClass {
    Auth,
    RateLimit,
    Upstream,
    Protocol,
    Stream,
    Unsupported,
    UserConfig,
    Contract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorContract {
    pub code: String,
    pub class: ErrorClass,
    pub recovery: RecoveryPolicy,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleErrorBase {
    pub feature_id: FeatureId,
    pub trace_id: TraceId,
    pub detail: ErrorContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorErr01RuntimeClassified {
    pub session_id: Option<SessionId>,
    pub turn_id: Option<TurnId>,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
    pub agent_id: Option<AgentId>,
    pub error: ErrorContract,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractValidationError {
    #[error("required string field `{field}` must not be empty")]
    EmptyField { field: &'static str },
    #[error("required collection `{field}` must not be empty")]
    EmptyCollection { field: &'static str },
    #[error("context-composed request must include a user-turn-input segment")]
    MissingUserTurnInputSegment,
}

pub fn validate_reason_req01(
    input: &ReasonReq01UserRawInput,
) -> Result<(), ContractValidationError> {
    if input.text.trim().is_empty() {
        return Err(ContractValidationError::EmptyField { field: "text" });
    }
    Ok(())
}

pub fn validate_reason_req02(
    input: &ReasonReq02ContextComposedInput,
) -> Result<(), ContractValidationError> {
    if input.user_text.trim().is_empty() {
        return Err(ContractValidationError::EmptyField { field: "user_text" });
    }
    if input.context_segments.is_empty() {
        return Err(ContractValidationError::EmptyCollection {
            field: "context_segments",
        });
    }
    if input
        .context_segments
        .iter()
        .any(|segment| segment.content.trim().is_empty())
    {
        return Err(ContractValidationError::EmptyField {
            field: "context_segments.content",
        });
    }
    if !input
        .context_segments
        .iter()
        .any(|segment| segment.kind == ContextSegmentKind::UserTurnInput)
    {
        return Err(ContractValidationError::MissingUserTurnInputSegment);
    }
    Ok(())
}

pub fn validate_reason_req03(
    payload: &ReasonReq03ProviderPayload,
) -> Result<(), ContractValidationError> {
    if payload.model.trim().is_empty() {
        return Err(ContractValidationError::EmptyField { field: "model" });
    }
    if payload.input_segments.is_empty() {
        return Err(ContractValidationError::EmptyCollection {
            field: "input_segments",
        });
    }
    if payload
        .input_segments
        .iter()
        .any(|segment| segment.content.trim().is_empty())
    {
        return Err(ContractValidationError::EmptyField {
            field: "input_segments.content",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_ids() -> (AgentId, SessionId, TurnId, TraceId, FeatureId) {
        (
            AgentId::new("agent-1"),
            SessionId::new("session-1"),
            TurnId::new("turn-1"),
            TraceId::new("trace-1"),
            FeatureId::new("contracts.core"),
        )
    }

    #[test]
    fn shared_contracts_round_trip_through_json() {
        let (agent_id, session_id, turn_id, trace_id, feature_id) = sample_ids();
        let contract = ReasonReq02ContextComposedInput {
            session_id,
            turn_id,
            trace_id,
            feature_id,
            agent_id,
            user_text: "hello".to_owned(),
            context_segments: vec![
                ContextSegment {
                    segment_id: ContextSegmentId::new("segment-memory"),
                    kind: ContextSegmentKind::SessionMemory,
                    stability: ContextStability::SessionStable,
                    cache_policy: ContextCachePolicy::Cacheable,
                    role: ContextRole::Developer,
                    content: "context".to_owned(),
                    token_budget: 128,
                    provenance: ContextProvenance {
                        source: "memory".to_owned(),
                        reference: Some("memory:1".to_owned()),
                    },
                },
                ContextSegment {
                    segment_id: ContextSegmentId::new("segment-user"),
                    kind: ContextSegmentKind::UserTurnInput,
                    stability: ContextStability::TurnVolatile,
                    cache_policy: ContextCachePolicy::NoCache,
                    role: ContextRole::User,
                    content: "hello".to_owned(),
                    token_budget: 64,
                    provenance: ContextProvenance {
                        source: "turn_input".to_owned(),
                        reference: None,
                    },
                },
            ],
        };

        let json = serde_json::to_string(&contract).expect("serialize");
        let decoded: ReasonReq02ContextComposedInput =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, contract);
    }

    #[test]
    fn shared_ids_are_distinct_and_stable() {
        let (agent_id, session_id, turn_id, trace_id, feature_id) = sample_ids();
        assert_eq!(agent_id.as_str(), "agent-1");
        assert_eq!(session_id.as_str(), "session-1");
        assert_eq!(turn_id.as_str(), "turn-1");
        assert_eq!(trace_id.as_str(), "trace-1");
        assert_eq!(feature_id.as_str(), "contracts.core");
    }

    #[test]
    fn error_contract_round_trip_and_policy_survive_serialization() {
        let contract = ErrorErr01RuntimeClassified {
            session_id: Some(SessionId::new("session-1")),
            turn_id: Some(TurnId::new("turn-1")),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("provider.semantic"),
            agent_id: Some(AgentId::new("agent-1")),
            error: ErrorContract {
                code: "RATE_LIMIT".to_owned(),
                class: ErrorClass::RateLimit,
                recovery: RecoveryPolicy::PeriodicRecoverable {
                    retry_after_seconds: 1800,
                },
                message: "retry later".to_owned(),
            },
        };

        let json = serde_json::to_string(&contract).expect("serialize");
        let decoded: ErrorErr01RuntimeClassified =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, contract);
    }

    #[test]
    fn validates_non_empty_user_text() {
        let (agent_id, session_id, turn_id, trace_id, feature_id) = sample_ids();
        let input = ReasonReq01UserRawInput {
            session_id,
            turn_id,
            trace_id,
            feature_id,
            agent_id,
            text: " ".to_owned(),
        };

        let err = validate_reason_req01(&input).expect_err("should fail");
        assert!(matches!(
            err,
            ContractValidationError::EmptyField { field } if field == "text"
        ));
    }

    #[test]
    fn validates_context_composed_request_has_user_segment() {
        let (agent_id, session_id, turn_id, trace_id, feature_id) = sample_ids();
        let input = ReasonReq02ContextComposedInput {
            session_id,
            turn_id,
            trace_id,
            feature_id,
            agent_id,
            user_text: "hello".to_owned(),
            context_segments: vec![ContextSegment {
                segment_id: ContextSegmentId::new("segment-memory"),
                kind: ContextSegmentKind::SessionMemory,
                stability: ContextStability::SessionStable,
                cache_policy: ContextCachePolicy::Cacheable,
                role: ContextRole::Developer,
                content: "context".to_owned(),
                token_budget: 128,
                provenance: ContextProvenance {
                    source: "memory".to_owned(),
                    reference: None,
                },
            }],
        };

        let err = validate_reason_req02(&input).expect_err("should fail");
        assert_eq!(err, ContractValidationError::MissingUserTurnInputSegment);
    }

    #[test]
    fn validates_provider_payload_has_segments() {
        let (agent_id, session_id, turn_id, trace_id, feature_id) = sample_ids();
        let payload = ReasonReq03ProviderPayload {
            session_id,
            turn_id,
            trace_id,
            feature_id,
            agent_id,
            model: "gpt-test".to_owned(),
            input_segments: vec![ContextSegment {
                segment_id: ContextSegmentId::new("segment-user"),
                kind: ContextSegmentKind::UserTurnInput,
                stability: ContextStability::TurnVolatile,
                cache_policy: ContextCachePolicy::NoCache,
                role: ContextRole::User,
                content: "hello".to_owned(),
                token_budget: 64,
                provenance: ContextProvenance {
                    source: "turn_input".to_owned(),
                    reference: None,
                },
            }],
        };

        validate_reason_req03(&payload).expect("valid payload");
    }

    #[test]
    fn token_usage_exposes_cache_hit_rate() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: Some(150),
            reasoning_tokens: Some(12),
            cache_creation_tokens: 20,
            cache_read_tokens: 80,
            finish_reason: Some("stop".to_owned()),
        };
        assert!((usage.cache_hit_rate() - 0.8).abs() < f64::EPSILON);
        assert_eq!(usage.resolved_total_tokens(), 150);
        assert_eq!(usage.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn tool_argument_round_trip_preserves_structured_json_values() {
        let tool_call = ToolCallContract {
            tool_call_id: ToolCallId::new("tool-1"),
            tool_name: "search".to_owned(),
            arguments: vec![
                ToolArgument {
                    name: "query".to_owned(),
                    value: json!("rust"),
                },
                ToolArgument {
                    name: "filters".to_owned(),
                    value: json!({"fresh": true, "count": 3}),
                },
            ],
            arguments_complete: true,
        };

        let json = serde_json::to_string(&tool_call).expect("serialize");
        let decoded: ToolCallContract = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, tool_call);
    }

    #[test]
    fn tool_preview_contract_round_trip_preserves_change_images() {
        let preview = ToolPreviewContract {
            tool_call_id: ToolCallId::new("tool-1"),
            changes: vec![
                ToolPreviewFileChange {
                    locked_path: "/tmp/workspace/docs/new.txt".to_owned(),
                    kind: ToolPreviewChangeKind::Create,
                    before_text: None,
                    after_text: Some("hello".to_owned()),
                },
                ToolPreviewFileChange {
                    locked_path: "/tmp/workspace/docs/old.txt".to_owned(),
                    kind: ToolPreviewChangeKind::Modify,
                    before_text: Some("old".to_owned()),
                    after_text: Some("new".to_owned()),
                },
            ],
        };

        let json = serde_json::to_string(&preview).expect("serialize");
        let decoded: ToolPreviewContract = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, preview);
    }
}
