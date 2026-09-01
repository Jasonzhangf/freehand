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

pub const FREEHAND_REMOTE_ACCESS_SCOPE_HEADER: &str = "x-freehand-relay-access-scope";
pub const FREEHAND_REMOTE_ACCESS_SCOPE_VALUE: &str = "remote";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextSegmentKind {
    SystemAnchor,
    DeveloperPolicy,
    SessionMemory,
    SessionSummary,
    InstructionCapability,
    TaskContract,
    TaskSpaceSnapshot,
    CurrentTime,
    AttentionResolution,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputAttachmentKind {
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputAttachmentMetadata {
    pub attachment_id: String,
    pub kind: InputAttachmentKind,
    pub media_type: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchDomain {
    News,
    Tutorial,
    Operations,
    Technical,
    Policy,
    LocalReview,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSocialPlatform {
    Web,
    Xhs,
    Weibo,
    X,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchDomainPlanDelivery {
    pub schema: String,
    pub delivery_id: String,
    pub domain: SearchDomain,
    pub preferred_source_kinds: Vec<String>,
    pub social_platform_priority: Vec<SearchSocialPlatform>,
    pub minimum_verified_sources: u32,
    pub policy_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchDiscoveryChannel {
    HostedWebSearch,
    CamoSocialSearch,
    WebFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchCandidateStatus {
    Usable,
    UnusableMissingUrl,
    UnusableOther,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchHostedAttempt {
    pub query: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchDiscoveryCandidate {
    pub candidate_id: String,
    pub status: SearchCandidateStatus,
    pub original_url: Option<String>,
    pub title: String,
    pub snippet: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovered_by: Option<SearchDiscoveryChannel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<SearchSocialPlatform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_weight: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchDiscoveryDelivery {
    pub schema: String,
    pub delivery_id: String,
    pub discovery_channel: SearchDiscoveryChannel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_plan_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosted_search_attempt: Option<SearchHostedAttempt>,
    pub candidates: Vec<SearchDiscoveryCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchAccessStatus {
    Verified,
    HttpError,
    Timeout,
    Blocked,
    NotAccessed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchEvidenceError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchAccessAttempt {
    pub attempt_id: String,
    pub channel: String,
    pub status: SearchAccessStatus,
    pub accessed_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<SearchEvidenceError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchVerificationDelivery {
    pub schema: String,
    pub delivery_id: String,
    pub source_id: String,
    pub original_url: String,
    pub camo_profile: String,
    pub accessed_at: String,
    pub access_status: SearchAccessStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_by: Option<String>,
    pub access_attempts: Vec<SearchAccessAttempt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<SearchEvidenceError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSupplementReason {
    MissingOriginalUrls,
    InsufficientVerifiedSources,
    LowWeightCoverage,
    SingleSourceOnly,
    SourceConflict,
    InsufficientEvidence,
    UserRequestedMoreSources,
    UserRequestedSocialSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocialSupplementDecisionDelivery {
    pub schema: String,
    pub delivery_id: String,
    pub domain_plan_ref: String,
    pub required: bool,
    pub reasons: Vec<SearchSupplementReason>,
    pub platforms: Vec<SearchSocialPlatform>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchFinalClaimStatus {
    Complete,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchClaimDelivery {
    pub claim_id: String,
    pub text: String,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchUnconfirmedDelivery {
    pub source_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchFinalDelivery {
    pub schema: String,
    pub delivery_id: String,
    pub domain_plan_ref: String,
    pub claim: SearchFinalClaimStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub claims: Vec<SearchClaimDelivery>,
    pub unconfirmed: Vec<SearchUnconfirmedDelivery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "delivery_type", content = "delivery", rename_all = "snake_case")]
pub enum SearchEvidenceDelivery {
    DomainPlan(SearchDomainPlanDelivery),
    Discovery(SearchDiscoveryDelivery),
    Verification(SearchVerificationDelivery),
    SupplementDecision(SocialSupplementDecisionDelivery),
    Final(SearchFinalDelivery),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEvidenceTurnStatus {
    DomainPlanValidated,
    HostedDiscoveryValidated,
    CamoVerificationRequired,
    CamoVerificationValidated,
    SupplementDecisionValidated,
    SocialDiscoveryValidated,
    FinalValidated,
    TurnTerminalSuccess,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEvidenceTerminal {
    Success,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchEvidenceTurnDelivery {
    pub schema: String,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_plan: Option<SearchDomainPlanDelivery>,
    pub deliveries: Vec<SearchEvidenceDelivery>,
    pub verified_sources: Vec<SearchVerificationDelivery>,
    pub unconfirmed: Vec<SearchUnconfirmedDelivery>,
    pub claims: Vec<SearchClaimDelivery>,
    pub status: SearchEvidenceTurnStatus,
    pub summary_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<SearchEvidenceTerminal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultContract {
    pub tool_call_id: ToolCallId,
    #[serde(default = "default_tool_result_status")]
    pub status: ToolResultStatus,
    pub output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_evidence: Option<SearchEvidenceDelivery>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolResultStatus {
    Success,
    Failed,
}

fn default_tool_result_status() -> ToolResultStatus {
    ToolResultStatus::Success
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
    /// Provider-reported input count. OpenAI-compatible responses report the
    /// full input total; Anthropic-compatible providers expose both observed
    /// uncached-input and total-input shapes.
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    /// Explicit provider-normalized total input. New records carry the exact
    /// denominator selected by the adapter through the contracts-owned rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_input_tokens: Option<u64>,
    pub finish_reason: Option<String>,
}

impl TokenUsage {
    pub fn resolve_reported_input_tokens(
        input_tokens: u64,
        cache_creation_tokens: u64,
        cache_read_tokens: u64,
    ) -> u64 {
        let cache_tokens = cache_creation_tokens.saturating_add(cache_read_tokens);
        if cache_tokens > input_tokens {
            input_tokens.saturating_add(cache_tokens)
        } else {
            input_tokens
        }
    }

    /// Provider-normalized total input. New records carry an explicit value.
    /// For legacy records, cache counters are added only when they exceed the
    /// reported input and therefore cannot already be categories within it.
    pub fn total_input_tokens(&self) -> u64 {
        self.normalized_input_tokens.unwrap_or_else(|| {
            Self::resolve_reported_input_tokens(
                self.input_tokens,
                self.cache_creation_tokens,
                self.cache_read_tokens,
            )
        })
    }

    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.total_input_tokens();
        if total == 0 {
            0.0
        } else {
            self.cache_read_tokens as f64 / total as f64
        }
    }

    pub fn resolved_total_tokens(&self) -> u64 {
        let normalized_total = self.total_input_tokens().saturating_add(self.output_tokens);
        self.total_tokens
            .filter(|total| *total >= normalized_total)
            .unwrap_or(normalized_total)
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
    AwaitingUserOptions,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_options: Option<Vec<String>>,
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
    fn task_context_segment_kinds_round_trip_through_json() {
        let task_contract = ContextSegment {
            segment_id: ContextSegmentId::new("task-contract"),
            kind: ContextSegmentKind::TaskContract,
            stability: ContextStability::SessionStable,
            cache_policy: ContextCachePolicy::Cacheable,
            role: ContextRole::Developer,
            content: "target: keep task aligned".to_owned(),
            token_budget: 128,
            provenance: ContextProvenance {
                source: "task_space".to_owned(),
                reference: Some("task-contract:v1".to_owned()),
            },
        };
        let task_snapshot = ContextSegment {
            segment_id: ContextSegmentId::new("task-space-snapshot"),
            kind: ContextSegmentKind::TaskSpaceSnapshot,
            stability: ContextStability::TurnVolatile,
            cache_policy: ContextCachePolicy::NoCache,
            role: ContextRole::Developer,
            content: "phase: executing".to_owned(),
            token_budget: 128,
            provenance: ContextProvenance {
                source: "task_space".to_owned(),
                reference: Some("snapshot:turn".to_owned()),
            },
        };

        for segment in [task_contract, task_snapshot] {
            let json = serde_json::to_string(&segment).expect("serialize");
            let decoded: ContextSegment = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(decoded, segment);
        }
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
            normalized_input_tokens: Some(100),
            finish_reason: Some("stop".to_owned()),
        };
        assert!((usage.cache_hit_rate() - 0.8).abs() < f64::EPSILON);
        assert_eq!(usage.total_input_tokens(), 100);
        assert_eq!(usage.resolved_total_tokens(), 150);
        assert_eq!(usage.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn cache_hit_rate_includes_uncached_input_in_denominator() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 5,
            total_tokens: Some(105),
            reasoning_tokens: None,
            cache_creation_tokens: 0,
            cache_read_tokens: 80,
            normalized_input_tokens: Some(100),
            finish_reason: None,
        };
        assert!((usage.cache_hit_rate() - 0.8).abs() < f64::EPSILON);
        assert_eq!(usage.total_input_tokens(), 100);
        assert_eq!(usage.resolved_total_tokens(), 105);
    }

    #[test]
    fn legacy_anthropic_usage_reconstructs_normalized_total_without_guessing() {
        let legacy = serde_json::from_value::<TokenUsage>(json!({
            "input_tokens": 14,
            "output_tokens": 82,
            "total_tokens": 96,
            "reasoning_tokens": null,
            "cache_creation_tokens": 0,
            "cache_read_tokens": 32,
            "finish_reason": "end_turn"
        }))
        .expect("legacy usage");

        assert_eq!(legacy.input_tokens, 14);
        assert_eq!(legacy.total_input_tokens(), 46);
        assert!((legacy.cache_hit_rate() - 32.0 / 46.0).abs() < f64::EPSILON);
        assert_eq!(legacy.resolved_total_tokens(), 128);
    }

    #[test]
    fn legacy_usage_preserves_reported_total_when_cache_is_already_a_subset() {
        let legacy = serde_json::from_value::<TokenUsage>(json!({
            "input_tokens": 19474,
            "output_tokens": 1574,
            "total_tokens": 21048,
            "reasoning_tokens": null,
            "cache_creation_tokens": 0,
            "cache_read_tokens": 15125,
            "finish_reason": "end_turn"
        }))
        .expect("legacy usage with total input");

        assert_eq!(legacy.total_input_tokens(), 19474);
        assert!((legacy.cache_hit_rate() - 15125.0 / 19474.0).abs() < f64::EPSILON);
        assert_eq!(legacy.resolved_total_tokens(), 21048);
    }

    #[test]
    fn normalized_input_tokens_round_trips_through_serialization() {
        let usage = TokenUsage {
            input_tokens: 14,
            output_tokens: 82,
            total_tokens: Some(128),
            reasoning_tokens: None,
            cache_creation_tokens: 0,
            cache_read_tokens: 32,
            normalized_input_tokens: Some(46),
            finish_reason: Some("end_turn".to_owned()),
        };
        let encoded = serde_json::to_value(&usage).expect("encode");
        assert_eq!(encoded.get("normalized_input_tokens"), Some(&json!(46)));
        let decoded: TokenUsage = serde_json::from_value(encoded).expect("decode");
        assert_eq!(decoded.total_input_tokens(), 46);
        assert_eq!(decoded.cache_read_tokens, 32);
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

    #[test]
    fn search_evidence_deliveries_round_trip_through_json() {
        let plan = SearchDomainPlanDelivery {
            schema: "search_evidence.domain_plan.v1".to_owned(),
            delivery_id: "domain-1".to_owned(),
            domain: SearchDomain::News,
            preferred_source_kinds: vec!["official_publication".to_owned()],
            social_platform_priority: vec![SearchSocialPlatform::Weibo],
            minimum_verified_sources: 1,
            policy_version: "2026-08-15".to_owned(),
        };
        let source = SearchVerificationDelivery {
            schema: "search_evidence.verification.v1".to_owned(),
            delivery_id: "verify-1".to_owned(),
            source_id: "source-1".to_owned(),
            original_url: "https://example.com/news".to_owned(),
            camo_profile: "news".to_owned(),
            accessed_at: "2026-08-15T12:00:00Z".to_owned(),
            access_status: SearchAccessStatus::Verified,
            page_title: Some("News".to_owned()),
            evidence_excerpt: Some("Verified page evidence".to_owned()),
            verified_by: Some("camo".to_owned()),
            access_attempts: vec![SearchAccessAttempt {
                attempt_id: "attempt-1".to_owned(),
                channel: "camo".to_owned(),
                status: SearchAccessStatus::Verified,
                accessed_at: "2026-08-15T12:00:00Z".to_owned(),
                error: None,
            }],
            error: None,
        };
        let delivery = SearchEvidenceTurnDelivery {
            schema: "search_evidence.turn.v1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            domain_plan: Some(plan.clone()),
            deliveries: vec![
                SearchEvidenceDelivery::DomainPlan(plan),
                SearchEvidenceDelivery::Verification(source.clone()),
            ],
            verified_sources: vec![source],
            unconfirmed: Vec::new(),
            claims: vec![SearchClaimDelivery {
                claim_id: "claim-1".to_owned(),
                text: "Claim".to_owned(),
                source_ids: vec!["source-1".to_owned()],
            }],
            status: SearchEvidenceTurnStatus::TurnTerminalSuccess,
            summary_ready: true,
            summary: Some("Summary".to_owned()),
            blocked_reason: None,
            terminal: Some(SearchEvidenceTerminal::Success),
        };

        let json = serde_json::to_string(&delivery).expect("serialize");
        let decoded: SearchEvidenceTurnDelivery = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, delivery);
    }

    #[test]
    fn search_evidence_delivery_rejects_unknown_fields() {
        let err = serde_json::from_value::<SearchDomainPlanDelivery>(json!({
            "schema": "search_evidence.domain_plan.v1",
            "delivery_id": "domain-1",
            "domain": "news",
            "preferred_source_kinds": ["official_publication"],
            "social_platform_priority": ["weibo"],
            "minimum_verified_sources": 1,
            "policy_version": "2026-08-15",
            "control_retry": true
        }))
        .expect_err("unknown fields must fail");
        assert!(err.to_string().contains("unknown field `control_retry`"));
    }
}
