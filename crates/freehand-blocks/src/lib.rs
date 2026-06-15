//! Shared pure builders, parsers, validators, and projectors for Freehand.

mod rewrite_policy;

use freehand_contracts::{
    ContextCachePolicy, ContextProvenance, ContextRewriteMode, ContextRole, ContextSegment,
    ContextSegmentId, ContextSegmentKind, ContextStability, TerminalStatus, ToolArgument,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

pub use rewrite_policy::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompletionClaim {
    Complete,
    Continue,
    Blocked,
}

impl CompletionClaim {
    fn parse(input: &str) -> Option<Self> {
        match input {
            "complete" => Some(Self::Complete),
            "continue" => Some(Self::Continue),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionSubmission {
    pub claim: CompletionClaim,
    pub completion_reason: Option<String>,
    pub evidence: Option<String>,
    pub summary: Option<String>,
    pub learned: Option<String>,
    pub next_step: Option<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionDecision {
    Completed {
        status: TerminalStatus,
        terminal_text: String,
    },
    ContinueWithNextStep {
        next_step: String,
    },
    Blocked {
        status: TerminalStatus,
        terminal_text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionValidationError {
    MissingField(&'static str),
    EmptyField(&'static str),
    MissingNextStep,
    MissingBlockedReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSchemaIssue {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSchemaRejection {
    pub issues: Vec<CompletionSchemaIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSchemaGuidance {
    pub prompt: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolArgumentsJsonError {
    #[error("tool arguments json parse failed: {0}")]
    InvalidJson(String),
    #[error("tool arguments json must be an object at the top level")]
    TopLevelMustBeObject,
}

pub fn completion_schema_guidance() -> CompletionSchemaGuidance {
    CompletionSchemaGuidance {
        prompt: concat!(
            "When you need to finish or continue this Freehand turn, include exactly one tagged JSON block:\n",
            "<freehand_completion>\n",
            "{\n",
            "  \"claim\": \"complete\" | \"continue\" | \"blocked\",\n",
            "  \"completion_reason\": \"required when claim=complete\",\n",
            "  \"evidence\": \"required when claim=complete\",\n",
            "  \"summary\": \"required when claim=complete\",\n",
            "  \"learned\": \"required when claim=complete\",\n",
            "  \"next_step\": \"required when claim=continue\",\n",
            "  \"blocked_reason\": \"required when claim=blocked\"\n",
            "}\n",
            "</freehand_completion>\n",
            "Do not omit the tag. Invalid or missing schema will be rejected with field-level feedback."
        )
        .to_owned(),
    }
}

pub fn completion_schema_rejection_feedback(rejection: &CompletionSchemaRejection) -> String {
    let issues = rejection
        .issues
        .iter()
        .map(|issue| format!("- `{}`: {}", issue.field, issue.message))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Your Freehand completion schema was rejected. Fix these schema entries:\n{issues}\n\n{}",
        completion_schema_guidance().prompt
    )
}

pub fn parse_completion_submission_block(
    text: &str,
) -> Result<CompletionSubmission, CompletionSchemaRejection> {
    let Some(raw_json) = extract_tagged_completion_json(text) else {
        return Err(schema_rejection([CompletionSchemaIssue {
            field: "freehand_completion".to_owned(),
            message: "missing `<freehand_completion>...</freehand_completion>` block".to_owned(),
        }]));
    };
    let value: Value = serde_json::from_str(raw_json.trim()).map_err(|err| {
        schema_rejection([CompletionSchemaIssue {
            field: "freehand_completion".to_owned(),
            message: format!("invalid JSON: {err}"),
        }])
    })?;
    let Some(object) = value.as_object() else {
        return Err(schema_rejection([CompletionSchemaIssue {
            field: "freehand_completion".to_owned(),
            message: "tagged JSON must be an object".to_owned(),
        }]));
    };

    let mut issues = Vec::new();
    let claim = match string_field(object, "claim") {
        Some(claim) => match CompletionClaim::parse(claim.as_str()) {
            Some(claim) => Some(claim),
            None => {
                issues.push(CompletionSchemaIssue {
                    field: "claim".to_owned(),
                    message: "must be one of `complete`, `continue`, or `blocked`".to_owned(),
                });
                None
            }
        },
        None => {
            issues.push(CompletionSchemaIssue {
                field: "claim".to_owned(),
                message: "is required".to_owned(),
            });
            None
        }
    };

    let Some(claim) = claim else {
        return Err(CompletionSchemaRejection { issues });
    };

    let submission = CompletionSubmission {
        claim,
        completion_reason: optional_string_field(object, "completion_reason"),
        evidence: optional_string_field(object, "evidence"),
        summary: optional_string_field(object, "summary"),
        learned: optional_string_field(object, "learned"),
        next_step: optional_string_field(object, "next_step"),
        blocked_reason: optional_string_field(object, "blocked_reason"),
    };

    let validation_issues = completion_submission_issues(&submission);
    if validation_issues.is_empty() {
        Ok(submission)
    } else {
        Err(CompletionSchemaRejection {
            issues: validation_issues,
        })
    }
}

pub fn strip_completion_submission_block(text: &str) -> String {
    let Some(raw_json_start) = text.find("<freehand_completion>") else {
        return text.trim().to_owned();
    };
    let before = text[..raw_json_start].trim();
    let after_start = raw_json_start + "<freehand_completion>".len();
    let Some(raw_json_end_rel) = text[after_start..].find("</freehand_completion>") else {
        return text.trim().to_owned();
    };
    let after = text[after_start + raw_json_end_rel + "</freehand_completion>".len()..].trim();
    match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (false, true) => before.to_owned(),
        (true, false) => after.to_owned(),
        (false, false) => format!("{before}\n{after}"),
    }
}

fn extract_tagged_completion_json(text: &str) -> Option<&str> {
    let start_tag = "<freehand_completion>";
    let end_tag = "</freehand_completion>";
    let start = text.find(start_tag)? + start_tag.len();
    let end = text[start..].find(end_tag)? + start;
    Some(&text[start..end])
}

fn optional_string_field(object: &Map<String, Value>, field: &'static str) -> Option<String> {
    string_field(object, field)
}

fn string_field(object: &Map<String, Value>, field: &'static str) -> Option<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn schema_rejection(
    issues: impl IntoIterator<Item = CompletionSchemaIssue>,
) -> CompletionSchemaRejection {
    CompletionSchemaRejection {
        issues: issues.into_iter().collect(),
    }
}

fn completion_submission_issues(submission: &CompletionSubmission) -> Vec<CompletionSchemaIssue> {
    match submission.claim {
        CompletionClaim::Complete => {
            let mut issues = Vec::new();
            collect_required_text_issue(
                &mut issues,
                submission.completion_reason.as_deref(),
                "completion_reason",
            );
            collect_required_text_issue(&mut issues, submission.evidence.as_deref(), "evidence");
            collect_required_text_issue(&mut issues, submission.summary.as_deref(), "summary");
            collect_required_text_issue(&mut issues, submission.learned.as_deref(), "learned");
            issues
        }
        CompletionClaim::Continue => {
            let mut issues = Vec::new();
            collect_required_text_issue(&mut issues, submission.next_step.as_deref(), "next_step");
            if issues.is_empty() {
                issues
            } else {
                vec![CompletionSchemaIssue {
                    field: "next_step".to_owned(),
                    message: "is required when `claim` is `continue`".to_owned(),
                }]
            }
        }
        CompletionClaim::Blocked => {
            let mut issues = Vec::new();
            collect_required_text_issue(
                &mut issues,
                submission.blocked_reason.as_deref(),
                "blocked_reason",
            );
            if issues.is_empty() {
                issues
            } else {
                vec![CompletionSchemaIssue {
                    field: "blocked_reason".to_owned(),
                    message: "is required when `claim` is `blocked`".to_owned(),
                }]
            }
        }
    }
}

fn collect_required_text_issue(
    issues: &mut Vec<CompletionSchemaIssue>,
    value: Option<&str>,
    field: &'static str,
) {
    match value {
        None => issues.push(CompletionSchemaIssue {
            field: field.to_owned(),
            message: "is required".to_owned(),
        }),
        Some(value) if value.trim().is_empty() => issues.push(CompletionSchemaIssue {
            field: field.to_owned(),
            message: "must not be empty".to_owned(),
        }),
        Some(_) => {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPlannerInput {
    pub candidate_segments: Vec<ContextSegment>,
    pub current_user_text: String,
    pub user_segment_id: ContextSegmentId,
    pub user_provenance: ContextProvenance,
    pub rewrite_mode: ContextRewriteMode,
    pub rewrite_version: u64,
    pub tool_schema_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSegmentTokenCost {
    pub segment_id: ContextSegmentId,
    pub estimated_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCacheDiagnostics {
    pub rewrite_mode: ContextRewriteMode,
    pub stable_prefix_hash: String,
    pub stable_segment_hashes: Vec<String>,
    pub tool_schema_hash: String,
    pub rewrite_version: u64,
    pub segment_token_costs: Vec<ContextSegmentTokenCost>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedContext {
    pub ordered_segments: Vec<ContextSegment>,
    pub diagnostics: ContextCacheDiagnostics,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextPlannerError {
    #[error("current user text must not be empty")]
    EmptyCurrentUserText,
    #[error("context candidate segment `{0}` has empty content")]
    EmptySegmentContent(String),
    #[error(
        "context candidate segment `{0}` may not use kind `user_turn_input`; planner owns current user segment admission"
    )]
    CandidateContainsUserTurnInput(String),
    #[error("subagent raw transcript is forbidden in parent context for segment `{0}`")]
    RawSubagentTranscriptRejected(String),
    #[error(
        "context segment `{segment_id}` exceeded token budget: estimated={estimated_tokens}, budget={token_budget}"
    )]
    SegmentTokenBudgetExceeded {
        segment_id: String,
        estimated_tokens: u32,
        token_budget: u32,
    },
    #[error("context segment `{segment_id}` violates segment contract: {reason}")]
    InvalidSegmentContract { segment_id: String, reason: String },
    #[error(
        "rewrite gate may only contain stable/session-stable segments; segment `{segment_id}` used forbidden kind `{kind}`"
    )]
    InvalidRewriteSegmentKind { segment_id: String, kind: String },
    #[error("ordinary turn is not a valid explicit rewrite gate mode")]
    OrdinaryModeIsNotRewriteGate,
}

pub fn validate_completion_submission(
    submission: &CompletionSubmission,
) -> Result<CompletionDecision, CompletionValidationError> {
    match submission.claim {
        CompletionClaim::Complete => {
            let completion_reason =
                required_text(submission.completion_reason.as_deref(), "completion_reason")?;
            let evidence = required_text(submission.evidence.as_deref(), "evidence")?;
            let summary = required_text(submission.summary.as_deref(), "summary")?;
            let learned = required_text(submission.learned.as_deref(), "learned")?;
            let terminal_text = format!(
                "Summary: {summary}\nEvidence: {evidence}\nLearned: {learned}\nCompletion reason: {completion_reason}"
            );
            Ok(CompletionDecision::Completed {
                status: TerminalStatus::Success,
                terminal_text,
            })
        }
        CompletionClaim::Continue => {
            let next_step = required_text(submission.next_step.as_deref(), "next_step")
                .map_err(|_| CompletionValidationError::MissingNextStep)?;
            Ok(CompletionDecision::ContinueWithNextStep { next_step })
        }
        CompletionClaim::Blocked => {
            let blocked_reason =
                required_text(submission.blocked_reason.as_deref(), "blocked_reason")
                    .map_err(|_| CompletionValidationError::MissingBlockedReason)?;
            Ok(CompletionDecision::Blocked {
                status: TerminalStatus::Blocked,
                terminal_text: format!("Blocked reason: {blocked_reason}"),
            })
        }
    }
}

fn required_text(
    value: Option<&str>,
    field: &'static str,
) -> Result<String, CompletionValidationError> {
    let value = value.ok_or(CompletionValidationError::MissingField(field))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CompletionValidationError::EmptyField(field));
    }
    Ok(trimmed.to_owned())
}

pub fn parse_tool_arguments_json(input: &str) -> Result<Vec<ToolArgument>, ToolArgumentsJsonError> {
    let value: Value = serde_json::from_str(input)
        .map_err(|err| ToolArgumentsJsonError::InvalidJson(err.to_string()))?;
    parse_tool_arguments_value(&value)
}

pub fn parse_tool_arguments_value(
    value: &Value,
) -> Result<Vec<ToolArgument>, ToolArgumentsJsonError> {
    let object = value
        .as_object()
        .ok_or(ToolArgumentsJsonError::TopLevelMustBeObject)?;
    Ok(object
        .iter()
        .map(|(name, value)| ToolArgument {
            name: name.clone(),
            value: value.clone(),
        })
        .collect())
}

pub fn render_tool_arguments_json(
    arguments: &[ToolArgument],
) -> Result<String, ToolArgumentsJsonError> {
    let mut object = Map::new();
    for argument in arguments {
        object.insert(argument.name.clone(), argument.value.clone());
    }
    serde_json::to_string(&Value::Object(object))
        .map_err(|err| ToolArgumentsJsonError::InvalidJson(err.to_string()))
}

pub fn plan_context(input: ContextPlannerInput) -> Result<PlannedContext, ContextPlannerError> {
    let user_text = input.current_user_text.trim();
    if user_text.is_empty() {
        return Err(ContextPlannerError::EmptyCurrentUserText);
    }

    let mut ordered_segments = Vec::with_capacity(input.candidate_segments.len() + 1);
    for segment in input.candidate_segments {
        validate_candidate_segment(&segment)?;
        ordered_segments.push(segment);
    }

    ordered_segments.push(ContextSegment {
        segment_id: input.user_segment_id,
        kind: ContextSegmentKind::UserTurnInput,
        stability: ContextStability::TurnVolatile,
        cache_policy: ContextCachePolicy::NoCache,
        role: ContextRole::User,
        content: user_text.to_owned(),
        token_budget: estimate_tokens(user_text),
        provenance: input.user_provenance,
    });

    ordered_segments.sort_by_key(|segment| segment_order_key(segment.kind));

    let diagnostics = build_context_cache_diagnostics(
        &ordered_segments,
        input.rewrite_mode,
        input.rewrite_version,
        input.tool_schema_fingerprint.as_deref(),
    )?;

    Ok(PlannedContext {
        ordered_segments,
        diagnostics,
    })
}

pub fn validate_rewrite_base_segments(
    segments: &[ContextSegment],
) -> Result<Vec<ContextSegment>, ContextPlannerError> {
    let mut ordered_segments = Vec::with_capacity(segments.len());
    for segment in segments {
        validate_candidate_segment(segment)?;
        if matches!(
            segment.kind,
            ContextSegmentKind::SubagentConclusion
                | ContextSegmentKind::ToolResultEvidence
                | ContextSegmentKind::UserTurnInput
        ) {
            return Err(ContextPlannerError::InvalidRewriteSegmentKind {
                segment_id: segment.segment_id.as_str().to_owned(),
                kind: context_segment_kind_label(segment.kind).to_owned(),
            });
        }
        ordered_segments.push(segment.clone());
    }
    ordered_segments.sort_by_key(|segment| segment_order_key(segment.kind));
    Ok(ordered_segments)
}

pub fn inspect_context_cache_diagnostics(
    ordered_segments: &[ContextSegment],
    rewrite_mode: ContextRewriteMode,
    rewrite_version: u64,
    tool_schema_fingerprint: Option<&str>,
) -> Result<ContextCacheDiagnostics, ContextPlannerError> {
    build_context_cache_diagnostics(
        ordered_segments,
        rewrite_mode,
        rewrite_version,
        tool_schema_fingerprint,
    )
}

pub fn render_context_segments_as_text(segments: &[ContextSegment]) -> String {
    segments
        .iter()
        .map(render_context_segment)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_context_segment(segment: &ContextSegment) -> String {
    let reference = segment
        .provenance
        .reference
        .as_deref()
        .map(|reference| format!(" reference=\"{reference}\""))
        .unwrap_or_default();
    format!(
        "<segment id=\"{}\" kind=\"{}\" role=\"{}\" stability=\"{}\" cache=\"{}\" source=\"{}\"{}>\n{}\n</segment>",
        segment.segment_id.as_str(),
        context_segment_kind_label(segment.kind),
        context_role_label(segment.role),
        context_stability_label(segment.stability),
        context_cache_policy_label(segment.cache_policy),
        segment.provenance.source,
        reference,
        segment.content,
    )
}

fn context_segment_kind_label(kind: ContextSegmentKind) -> &'static str {
    match kind {
        ContextSegmentKind::SystemAnchor => "system_anchor",
        ContextSegmentKind::DeveloperPolicy => "developer_policy",
        ContextSegmentKind::SessionMemory => "session_memory",
        ContextSegmentKind::SessionSummary => "session_summary",
        ContextSegmentKind::SubagentConclusion => "subagent_conclusion",
        ContextSegmentKind::ToolResultEvidence => "tool_result_evidence",
        ContextSegmentKind::UserTurnInput => "user_turn_input",
        ContextSegmentKind::CompletionContract => "completion_contract",
    }
}

fn context_stability_label(stability: ContextStability) -> &'static str {
    match stability {
        ContextStability::Stable => "stable",
        ContextStability::SessionStable => "session_stable",
        ContextStability::TurnVolatile => "turn_volatile",
    }
}

fn context_cache_policy_label(policy: ContextCachePolicy) -> &'static str {
    match policy {
        ContextCachePolicy::CacheAnchor => "cache_anchor",
        ContextCachePolicy::Cacheable => "cacheable",
        ContextCachePolicy::NoCache => "no_cache",
    }
}

fn context_role_label(role: ContextRole) -> &'static str {
    match role {
        ContextRole::System => "system",
        ContextRole::Developer => "developer",
        ContextRole::User => "user",
        ContextRole::Tool => "tool",
    }
}

fn validate_candidate_segment(segment: &ContextSegment) -> Result<(), ContextPlannerError> {
    if segment.content.trim().is_empty() {
        return Err(ContextPlannerError::EmptySegmentContent(
            segment.segment_id.as_str().to_owned(),
        ));
    }
    if segment.kind == ContextSegmentKind::UserTurnInput {
        return Err(ContextPlannerError::CandidateContainsUserTurnInput(
            segment.segment_id.as_str().to_owned(),
        ));
    }
    if segment.kind == ContextSegmentKind::SubagentConclusion
        && segment
            .provenance
            .source
            .to_ascii_lowercase()
            .contains("transcript")
    {
        return Err(ContextPlannerError::RawSubagentTranscriptRejected(
            segment.segment_id.as_str().to_owned(),
        ));
    }

    validate_segment_contract(segment)?;

    let estimated_tokens = estimate_tokens(&segment.content);
    if estimated_tokens > segment.token_budget {
        return Err(ContextPlannerError::SegmentTokenBudgetExceeded {
            segment_id: segment.segment_id.as_str().to_owned(),
            estimated_tokens,
            token_budget: segment.token_budget,
        });
    }
    Ok(())
}

fn validate_segment_contract(segment: &ContextSegment) -> Result<(), ContextPlannerError> {
    let (expected_stability, expected_cache_policy, expected_role) =
        expected_segment_contract(segment.kind);
    if segment.stability != expected_stability {
        return Err(ContextPlannerError::InvalidSegmentContract {
            segment_id: segment.segment_id.as_str().to_owned(),
            reason: format!(
                "expected stability `{}`, got `{}`",
                context_stability_label(expected_stability),
                context_stability_label(segment.stability)
            ),
        });
    }
    if segment.cache_policy != expected_cache_policy {
        return Err(ContextPlannerError::InvalidSegmentContract {
            segment_id: segment.segment_id.as_str().to_owned(),
            reason: format!(
                "expected cache policy `{}`, got `{}`",
                context_cache_policy_label(expected_cache_policy),
                context_cache_policy_label(segment.cache_policy)
            ),
        });
    }
    if let Some(expected_role) = expected_role
        && segment.role != expected_role
    {
        return Err(ContextPlannerError::InvalidSegmentContract {
            segment_id: segment.segment_id.as_str().to_owned(),
            reason: format!(
                "expected role `{}`, got `{}`",
                context_role_label(expected_role),
                context_role_label(segment.role)
            ),
        });
    }
    Ok(())
}

fn expected_segment_contract(
    kind: ContextSegmentKind,
) -> (ContextStability, ContextCachePolicy, Option<ContextRole>) {
    match kind {
        ContextSegmentKind::SystemAnchor => (
            ContextStability::Stable,
            ContextCachePolicy::CacheAnchor,
            Some(ContextRole::System),
        ),
        ContextSegmentKind::DeveloperPolicy => (
            ContextStability::Stable,
            ContextCachePolicy::CacheAnchor,
            Some(ContextRole::Developer),
        ),
        ContextSegmentKind::SessionMemory => (
            ContextStability::SessionStable,
            ContextCachePolicy::Cacheable,
            None,
        ),
        ContextSegmentKind::SessionSummary => (
            ContextStability::SessionStable,
            ContextCachePolicy::Cacheable,
            None,
        ),
        ContextSegmentKind::SubagentConclusion => (
            ContextStability::TurnVolatile,
            ContextCachePolicy::NoCache,
            None,
        ),
        ContextSegmentKind::ToolResultEvidence => (
            ContextStability::TurnVolatile,
            ContextCachePolicy::NoCache,
            Some(ContextRole::Tool),
        ),
        ContextSegmentKind::UserTurnInput => (
            ContextStability::TurnVolatile,
            ContextCachePolicy::NoCache,
            Some(ContextRole::User),
        ),
        ContextSegmentKind::CompletionContract => (
            ContextStability::Stable,
            ContextCachePolicy::CacheAnchor,
            Some(ContextRole::Developer),
        ),
    }
}

fn build_context_cache_diagnostics(
    ordered_segments: &[ContextSegment],
    rewrite_mode: ContextRewriteMode,
    rewrite_version: u64,
    tool_schema_fingerprint: Option<&str>,
) -> Result<ContextCacheDiagnostics, ContextPlannerError> {
    let stable_segments = ordered_segments
        .iter()
        .take_while(|segment| {
            matches!(
                segment.stability,
                ContextStability::Stable | ContextStability::SessionStable
            )
        })
        .collect::<Vec<_>>();

    let stable_segment_hashes = stable_segments
        .iter()
        .map(|segment| segment_cache_hash(segment))
        .collect::<Vec<_>>();
    let stable_prefix_hash = fnv1a_hex(
        stable_segment_hashes
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("|")
            .as_bytes(),
    );
    let tool_schema_hash = fnv1a_hex(tool_schema_fingerprint.unwrap_or("").as_bytes());
    let mut segment_token_costs = Vec::with_capacity(ordered_segments.len());
    for segment in ordered_segments {
        let estimated_tokens = estimate_tokens(&segment.content);
        if estimated_tokens > segment.token_budget {
            return Err(ContextPlannerError::SegmentTokenBudgetExceeded {
                segment_id: segment.segment_id.as_str().to_owned(),
                estimated_tokens,
                token_budget: segment.token_budget,
            });
        }
        segment_token_costs.push(ContextSegmentTokenCost {
            segment_id: segment.segment_id.clone(),
            estimated_tokens,
        });
    }

    Ok(ContextCacheDiagnostics {
        rewrite_mode,
        stable_prefix_hash,
        stable_segment_hashes,
        tool_schema_hash,
        rewrite_version,
        segment_token_costs,
    })
}

fn segment_cache_hash(segment: &ContextSegment) -> String {
    let reference = segment.provenance.reference.as_deref().unwrap_or("");
    let materialized = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        segment.segment_id.as_str(),
        context_segment_kind_label(segment.kind),
        context_stability_label(segment.stability),
        context_cache_policy_label(segment.cache_policy),
        context_role_label(segment.role),
        segment.provenance.source,
        reference,
        segment.content,
    );
    fnv1a_hex(materialized.as_bytes())
}

fn segment_order_key(kind: ContextSegmentKind) -> u8 {
    match kind {
        ContextSegmentKind::SystemAnchor => 0,
        ContextSegmentKind::DeveloperPolicy => 1,
        ContextSegmentKind::CompletionContract => 2,
        ContextSegmentKind::SessionMemory => 3,
        ContextSegmentKind::SessionSummary => 4,
        ContextSegmentKind::SubagentConclusion => 5,
        ContextSegmentKind::ToolResultEvidence => 6,
        ContextSegmentKind::UserTurnInput => 7,
    }
}

fn estimate_tokens(content: &str) -> u32 {
    let chars = content.chars().count();
    let estimated = chars.div_ceil(4).max(1);
    u32::try_from(estimated).unwrap_or(u32::MAX)
}

fn fnv1a_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_completed_submission_with_terminal_text() {
        let decision = validate_completion_submission(&CompletionSubmission {
            claim: CompletionClaim::Complete,
            completion_reason: Some("task finished".to_owned()),
            evidence: Some("file updated".to_owned()),
            summary: Some("done".to_owned()),
            learned: Some("keep harness strict".to_owned()),
            next_step: None,
            blocked_reason: None,
        })
        .expect("valid");

        match decision {
            CompletionDecision::Completed {
                status,
                terminal_text,
            } => {
                assert_eq!(status, TerminalStatus::Success);
                assert!(terminal_text.contains("Summary: done"));
                assert!(terminal_text.contains("Evidence: file updated"));
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn rejects_completed_submission_without_evidence() {
        let err = validate_completion_submission(&CompletionSubmission {
            claim: CompletionClaim::Complete,
            completion_reason: Some("task finished".to_owned()),
            evidence: None,
            summary: Some("done".to_owned()),
            learned: Some("keep harness strict".to_owned()),
            next_step: None,
            blocked_reason: None,
        })
        .expect_err("should fail");
        assert_eq!(err, CompletionValidationError::MissingField("evidence"));
    }

    #[test]
    fn accepts_blocked_submission() {
        let decision = validate_completion_submission(&CompletionSubmission {
            claim: CompletionClaim::Blocked,
            completion_reason: None,
            evidence: None,
            summary: None,
            learned: None,
            next_step: None,
            blocked_reason: Some("waiting on upstream".to_owned()),
        })
        .expect("valid");
        match decision {
            CompletionDecision::Blocked {
                status,
                terminal_text,
            } => {
                assert_eq!(status, TerminalStatus::Blocked);
                assert!(terminal_text.contains("waiting on upstream"));
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn parses_tool_arguments_json_into_structured_arguments() {
        let arguments = parse_tool_arguments_json(r#"{"query":"rust","limit":3,"strict":true}"#)
            .expect("valid");
        assert_eq!(arguments.len(), 3);
        assert!(
            arguments
                .iter()
                .any(|argument| argument.name == "query" && argument.value == json!("rust"))
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument.name == "limit" && argument.value == json!(3))
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument.name == "strict" && argument.value == json!(true))
        );
    }

    #[test]
    fn renders_tool_arguments_json_from_contract_arguments() {
        let rendered = render_tool_arguments_json(&[
            ToolArgument {
                name: "query".to_owned(),
                value: json!("rust"),
            },
            ToolArgument {
                name: "filters".to_owned(),
                value: json!({"fresh": true}),
            },
        ])
        .expect("rendered");

        let round_trip = parse_tool_arguments_json(&rendered).expect("round trip");
        assert_eq!(round_trip.len(), 2);
        assert!(round_trip.iter().any(|argument| {
            argument.name == "filters" && argument.value == json!({"fresh": true})
        }));
    }

    #[test]
    fn parses_tool_arguments_directly_from_json_value() {
        let arguments =
            parse_tool_arguments_value(&json!({"query":"rust","filters":{"fresh":true}}))
                .expect("valid");
        assert_eq!(arguments.len(), 2);
        assert!(arguments.iter().any(|argument| {
            argument.name == "filters" && argument.value == json!({"fresh": true})
        }));
    }

    #[test]
    fn renders_context_segments_with_explicit_labels() {
        let rendered = render_context_segments_as_text(&[ContextSegment {
            segment_id: freehand_contracts::ContextSegmentId::new("segment-user"),
            kind: ContextSegmentKind::UserTurnInput,
            stability: ContextStability::TurnVolatile,
            cache_policy: ContextCachePolicy::NoCache,
            role: ContextRole::User,
            content: "hello".to_owned(),
            token_budget: 64,
            provenance: freehand_contracts::ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
        }]);

        assert!(rendered.contains("kind=\"user_turn_input\""));
        assert!(rendered.contains("role=\"user\""));
        assert!(rendered.contains("\nhello\n"));
    }

    fn segment(
        id: &str,
        kind: ContextSegmentKind,
        content: &str,
        token_budget: u32,
        source: &str,
    ) -> ContextSegment {
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
            ContextSegmentKind::SubagentConclusion => (
                ContextStability::TurnVolatile,
                ContextCachePolicy::NoCache,
                ContextRole::Developer,
            ),
            ContextSegmentKind::ToolResultEvidence => (
                ContextStability::TurnVolatile,
                ContextCachePolicy::NoCache,
                ContextRole::Tool,
            ),
            ContextSegmentKind::UserTurnInput => (
                ContextStability::TurnVolatile,
                ContextCachePolicy::NoCache,
                ContextRole::User,
            ),
        };
        ContextSegment {
            segment_id: ContextSegmentId::new(id),
            kind,
            stability,
            cache_policy,
            role,
            content: content.to_owned(),
            token_budget,
            provenance: ContextProvenance {
                source: source.to_owned(),
                reference: None,
            },
        }
    }

    #[test]
    fn planner_orders_stable_prefix_before_volatile_tail() {
        let planned = plan_context(ContextPlannerInput {
            candidate_segments: vec![
                segment(
                    "tail-sub",
                    ContextSegmentKind::SubagentConclusion,
                    "search done",
                    16,
                    "subagent_report",
                ),
                segment(
                    "head-system",
                    ContextSegmentKind::SystemAnchor,
                    "sys",
                    8,
                    "system",
                ),
                segment(
                    "head-memory",
                    ContextSegmentKind::SessionMemory,
                    "mem",
                    8,
                    "memory",
                ),
            ],
            current_user_text: "hello planner".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: Some("tool-v1".to_owned()),
        })
        .expect("planned");

        let ordered_ids = planned
            .ordered_segments
            .iter()
            .map(|segment| segment.segment_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ordered_ids,
            vec!["head-system", "head-memory", "tail-sub", "turn-user"]
        );
        assert_eq!(
            planned.diagnostics.rewrite_mode,
            ContextRewriteMode::OrdinaryTurn
        );
        assert_eq!(planned.diagnostics.stable_segment_hashes.len(), 2);
    }

    #[test]
    fn planner_rejects_raw_subagent_transcript_source() {
        let err = plan_context(ContextPlannerInput {
            candidate_segments: vec![segment(
                "sub-raw",
                ContextSegmentKind::SubagentConclusion,
                "raw child transcript",
                16,
                "subagent_transcript",
            )],
            current_user_text: "hello".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: None,
        })
        .expect_err("should fail");
        assert!(matches!(
            err,
            ContextPlannerError::RawSubagentTranscriptRejected(id) if id == "sub-raw"
        ));
    }

    #[test]
    fn planner_rejects_segments_that_exceed_token_budget() {
        let err = plan_context(ContextPlannerInput {
            candidate_segments: vec![segment(
                "memory-tight",
                ContextSegmentKind::SessionMemory,
                "01234567890123456789",
                1,
                "memory",
            )],
            current_user_text: "hello".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: None,
        })
        .expect_err("should fail");
        assert!(matches!(
            err,
            ContextPlannerError::SegmentTokenBudgetExceeded { segment_id, .. } if segment_id == "memory-tight"
        ));
    }

    #[test]
    fn planner_diagnostics_drift_when_stable_prefix_changes() {
        let planned_a = plan_context(ContextPlannerInput {
            candidate_segments: vec![segment(
                "memory-a",
                ContextSegmentKind::SessionMemory,
                "memory-a",
                8,
                "memory",
            )],
            current_user_text: "hello".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user-a"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: Some("tool-v1".to_owned()),
        })
        .expect("planned a");
        let planned_b = plan_context(ContextPlannerInput {
            candidate_segments: vec![segment(
                "memory-b",
                ContextSegmentKind::SessionMemory,
                "memory-b",
                8,
                "memory",
            )],
            current_user_text: "hello".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user-b"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: Some("tool-v1".to_owned()),
        })
        .expect("planned b");

        assert_ne!(
            planned_a.diagnostics.stable_prefix_hash,
            planned_b.diagnostics.stable_prefix_hash
        );
    }

    #[test]
    fn rewrite_base_rejects_turn_volatile_segments() {
        let err = validate_rewrite_base_segments(&[segment(
            "tool-evidence",
            ContextSegmentKind::ToolResultEvidence,
            "tool output",
            16,
            "tool",
        )])
        .expect_err("should fail");
        assert!(matches!(
            err,
            ContextPlannerError::InvalidRewriteSegmentKind { segment_id, .. } if segment_id == "tool-evidence"
        ));
    }

    #[test]
    fn rewrite_cache_diagnostics_keep_explicit_mode() {
        let ordered_segments = validate_rewrite_base_segments(&[segment(
            "session-summary",
            ContextSegmentKind::SessionSummary,
            "summary",
            8,
            "compaction",
        )])
        .expect("ordered");
        let diagnostics = inspect_context_cache_diagnostics(
            &ordered_segments,
            ContextRewriteMode::Compaction,
            2,
            Some("tool-v2"),
        )
        .expect("diagnostics");

        assert_eq!(diagnostics.rewrite_mode, ContextRewriteMode::Compaction);
        assert_eq!(diagnostics.rewrite_version, 2);
    }

    #[test]
    fn parses_tagged_completion_block() {
        let parsed = parse_completion_submission_block(
            "pong\n<freehand_completion>\n{\"claim\":\"complete\",\"completion_reason\":\"done\",\"evidence\":\"provider returned pong\",\"summary\":\"pong\",\"learned\":\"keep tagged completion strict\"}\n</freehand_completion>",
        )
        .expect("parsed");

        assert_eq!(parsed.claim, CompletionClaim::Complete);
        assert_eq!(parsed.summary.as_deref(), Some("pong"));
    }

    #[test]
    fn rejects_missing_completion_tag() {
        let err = parse_completion_submission_block("pong").expect_err("must fail");
        assert_eq!(err.issues.len(), 1);
        assert_eq!(err.issues[0].field, "freehand_completion");
    }

    #[test]
    fn rejects_invalid_completion_json() {
        let err = parse_completion_submission_block(
            "<freehand_completion>\n{\"claim\":\"complete\"\n</freehand_completion>",
        )
        .expect_err("must fail");
        assert_eq!(err.issues[0].field, "freehand_completion");
        assert!(err.issues[0].message.contains("invalid JSON"));
    }

    #[test]
    fn reports_multiple_missing_complete_fields() {
        let err = parse_completion_submission_block(
            "<freehand_completion>\n{\"claim\":\"complete\",\"summary\":\"pong\"}\n</freehand_completion>",
        )
        .expect_err("must fail");
        let fields = err
            .issues
            .iter()
            .map(|issue| issue.field.as_str())
            .collect::<Vec<_>>();
        assert!(fields.contains(&"completion_reason"));
        assert!(fields.contains(&"evidence"));
        assert!(fields.contains(&"learned"));
    }

    #[test]
    fn rejects_continue_without_next_step() {
        let err = parse_completion_submission_block(
            "<freehand_completion>\n{\"claim\":\"continue\"}\n</freehand_completion>",
        )
        .expect_err("must fail");
        assert_eq!(err.issues.len(), 1);
        assert_eq!(err.issues[0].field, "next_step");
    }

    #[test]
    fn strips_completion_block_from_visible_text() {
        let visible = strip_completion_submission_block(
            "pong\n<freehand_completion>\n{\"claim\":\"complete\",\"completion_reason\":\"done\",\"evidence\":\"provider returned pong\",\"summary\":\"pong\",\"learned\":\"keep tagged completion strict\"}\n</freehand_completion>",
        );
        assert_eq!(visible, "pong");
    }
}
