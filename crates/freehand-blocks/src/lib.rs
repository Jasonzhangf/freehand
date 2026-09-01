//! Shared pure builders, parsers, validators, and projectors for Freehand.

mod rewrite_policy;
mod tool_display;

use freehand_contracts::{
    ContextCachePolicy, ContextProvenance, ContextRewriteMode, ContextRole, ContextSegment,
    ContextSegmentId, ContextSegmentKind, ContextStability, SearchAccessStatus,
    SearchCandidateStatus, SearchDiscoveryCandidate, SearchDiscoveryChannel,
    SearchDiscoveryDelivery, SearchDomain, SearchDomainPlanDelivery, SearchEvidenceDelivery,
    SearchEvidenceTerminal, SearchEvidenceTurnDelivery, SearchEvidenceTurnStatus,
    SearchFinalClaimStatus, SearchFinalDelivery, SearchSocialPlatform, SearchVerificationDelivery,
    SocialSupplementDecisionDelivery, TerminalStatus, ToolArgument, TurnId,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

pub use rewrite_policy::*;
pub use tool_display::*;

pub const SEARCH_DOMAIN_PLAN_SCHEMA: &str = "search_evidence.domain_plan.v1";
const SEARCH_DISCOVERY_SCHEMA: &str = "search_evidence.discovery.v1";
const SEARCH_VERIFICATION_SCHEMA: &str = "search_evidence.verification.v1";
pub const SEARCH_SUPPLEMENT_SCHEMA: &str = "search_evidence.supplement_decision.v1";
pub const SEARCH_FINAL_SCHEMA: &str = "search_evidence.final.v1";
const SEARCH_TURN_SCHEMA: &str = "search_evidence.turn.v1";

pub fn search_evidence_model_delivery_examples() -> Result<String, serde_json::Error> {
    use freehand_contracts::{
        SearchClaimDelivery, SearchDomain, SearchDomainPlanDelivery, SearchFinalClaimStatus,
        SearchFinalDelivery, SearchSocialPlatform, SearchSupplementReason,
        SocialSupplementDecisionDelivery,
    };
    let plan = SearchDomainPlanDelivery {
        schema: SEARCH_DOMAIN_PLAN_SCHEMA.to_owned(),
        delivery_id: "plan-news-001".to_owned(),
        domain: SearchDomain::News,
        preferred_source_kinds: vec![
            "official_publication".to_owned(),
            "mainstream_news".to_owned(),
        ],
        social_platform_priority: vec![SearchSocialPlatform::Weibo, SearchSocialPlatform::X],
        minimum_verified_sources: 2,
        policy_version: "2026-08-15".to_owned(),
    };
    let supplement = SocialSupplementDecisionDelivery {
        schema: SEARCH_SUPPLEMENT_SCHEMA.to_owned(),
        delivery_id: "supplement-news-001".to_owned(),
        domain_plan_ref: "plan-news-001".to_owned(),
        required: true,
        reasons: vec![SearchSupplementReason::InsufficientVerifiedSources],
        platforms: vec![SearchSocialPlatform::Weibo],
    };
    let final_delivery = SearchFinalDelivery {
        schema: SEARCH_FINAL_SCHEMA.to_owned(),
        delivery_id: "final-news-001".to_owned(),
        domain_plan_ref: "plan-news-001".to_owned(),
        claim: SearchFinalClaimStatus::Complete,
        summary: Some("Supported claim summary".to_owned()),
        claims: vec![SearchClaimDelivery {
            claim_id: "claim-news-001".to_owned(),
            text: "Supported claim".to_owned(),
            source_ids: vec!["src-official-1".to_owned()],
        }],
        unconfirmed: Vec::new(),
        blocked_reason: None,
    };
    let plan_json = serde_json::to_string(&plan)?;
    let supplement_json = serde_json::to_string(&supplement)?;
    let final_json = serde_json::to_string(&final_delivery)?;
    Ok(format!(
        "{}\n{}\n{}",
        plan_json.replace('\n', " "),
        supplement_json.replace('\n', " "),
        final_json.replace('\n', " "),
    ))
}

pub fn search_evidence_contract_guidance() -> Result<String, serde_json::Error> {
    Ok(format!(
        "Worker search evidence delivery contract:\n\
         - Each search delivery emits exactly one <freehand_search_delivery>...</freehand_search_delivery> block containing valid JSON only: double-quoted keys, double-quoted string values, no comments, no trailing commas, no markdown fence inside the tags.\n\
         - Domain plan schema: {SEARCH_DOMAIN_PLAN_SCHEMA}. Required keys: delivery_id, domain, preferred_source_kinds, social_platform_priority, minimum_verified_sources, policy_version.\n\
         - `minimum_verified_sources` is a JSON number. `domain` is one of: news, tutorial, operations, technical, policy, local_review, general. For news the first `social_platform_priority` must be `weibo`; for tutorial/operations the first must be `xhs`.\n\
         - Supplement decision schema: {SEARCH_SUPPLEMENT_SCHEMA}. When `required` is false, `reasons` and `platforms` must both be empty. Valid reasons: missing_original_urls, insufficient_verified_sources, low_weight_coverage, single_source_only, source_conflict, insufficient_evidence, user_requested_more_sources, user_requested_social_source.\n\
         - Final delivery schema: {SEARCH_FINAL_SCHEMA}. Required keys are schema, delivery_id, domain_plan_ref, claim, claims (always an array), unconfirmed (always an array), and either summary (for `claim=complete`) or blocked_reason (for `claim=blocked`). Each `unconfirmed` item requires both `source_id` and `reason`. Never omit `unconfirmed` and never use a non-array value for `claims` or `unconfirmed`.\n\
         - Canonical JSON examples generated from the same contract types:\n{}\n\
         - Never invent URLs, access results, page titles, excerpts, verified evidence, or source ids.\n\
         - If no usable evidence is available, report the gap in the final delivery instead of fabricating sources.",
        search_evidence_model_delivery_examples()?,
    ))
}

pub fn web_fetch_search_discovery(
    domain_plan_ref: &str,
    url: &str,
    title: &str,
    snippet: &str,
) -> SearchDiscoveryDelivery {
    SearchDiscoveryDelivery {
        schema: SEARCH_DISCOVERY_SCHEMA.to_owned(),
        delivery_id: format!("discovery-web-fetch-{}", fnv1a_hex(url.as_bytes())),
        discovery_channel: SearchDiscoveryChannel::WebFetch,
        domain_plan_ref: Some(domain_plan_ref.to_owned()),
        hosted_search_attempt: None,
        candidates: vec![SearchDiscoveryCandidate {
            candidate_id: format!("web-fetch-{}", fnv1a_hex(url.as_bytes())),
            status: SearchCandidateStatus::Usable,
            original_url: Some(url.to_owned()),
            title: title.to_owned(),
            snippet: snippet.chars().take(512).collect(),
            discovered_by: Some(SearchDiscoveryChannel::WebFetch),
            platform: None,
            source_weight: Some(50),
            reason: None,
        }],
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SearchEvidenceValidationError {
    #[error("invalid search evidence field `{field}`: {reason}")]
    InvalidField { field: String, reason: String },
    #[error("search evidence transition `{from:?}` -> `{to:?}` is not adjacent")]
    InvalidTransition {
        from: SearchEvidenceTurnStatus,
        to: SearchEvidenceTurnStatus,
    },
    #[error("final claim references unknown or unverified source `{0}`")]
    InvalidSourceReference(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchEvidenceSchemaRejection {
    pub category: SearchEvidenceSchemaRejectionCategory,
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEvidenceSchemaRejectionCategory {
    TaggedBlock,
    JsonSyntax,
    Decode,
    Validation,
    StateTransition,
    SourceReference,
    StageMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchEvidenceModelStage {
    DomainPlan,
    SupplementDecision,
    FinalDelivery,
}

pub fn search_evidence_schema_rejection_feedback(
    rejection: &SearchEvidenceSchemaRejection,
) -> String {
    format!(
        "Search evidence delivery rejected. Fix `{}`: {}. Emit exactly one <freehand_search_delivery> JSON block for the current search stage; do not invent URLs, access results, or verified evidence.",
        rejection.field, rejection.message
    )
}

pub fn parse_search_evidence_delivery_block(
    text: &str,
) -> Result<SearchEvidenceDelivery, SearchEvidenceSchemaRejection> {
    let raw_json = extract_single_tagged_json(
        text,
        "<freehand_search_delivery>",
        "</freehand_search_delivery>",
    )?;
    let value: Value =
        serde_json::from_str(raw_json.trim()).map_err(|error| SearchEvidenceSchemaRejection {
            category: SearchEvidenceSchemaRejectionCategory::JsonSyntax,
            field: "freehand_search_delivery".to_owned(),
            message: format!("invalid JSON: {error}"),
        })?;
    let schema = value
        .as_object()
        .and_then(|object| object.get("schema"))
        .and_then(Value::as_str)
        .ok_or_else(|| SearchEvidenceSchemaRejection {
            category: SearchEvidenceSchemaRejectionCategory::Decode,
            field: "schema".to_owned(),
            message: "is required and must be a string".to_owned(),
        })?;
    let delivery = match schema {
        SEARCH_DOMAIN_PLAN_SCHEMA => decode_search_delivery::<SearchDomainPlanDelivery>(value)
            .map(SearchEvidenceDelivery::DomainPlan),
        SEARCH_DISCOVERY_SCHEMA => decode_search_delivery::<SearchDiscoveryDelivery>(value)
            .map(SearchEvidenceDelivery::Discovery),
        SEARCH_VERIFICATION_SCHEMA => decode_search_delivery::<SearchVerificationDelivery>(value)
            .map(SearchEvidenceDelivery::Verification),
        SEARCH_SUPPLEMENT_SCHEMA => {
            decode_search_delivery::<SocialSupplementDecisionDelivery>(value)
                .map(SearchEvidenceDelivery::SupplementDecision)
        }
        SEARCH_FINAL_SCHEMA => {
            decode_search_delivery::<SearchFinalDelivery>(value).map(SearchEvidenceDelivery::Final)
        }
        _ => {
            return Err(SearchEvidenceSchemaRejection {
                category: SearchEvidenceSchemaRejectionCategory::Decode,
                field: "schema".to_owned(),
                message: format!("unsupported search evidence schema `{schema}`"),
            });
        }
    }?;
    validate_parsed_search_delivery(&delivery).map_err(schema_rejection_from_validation)?;
    Ok(delivery)
}

fn decode_search_delivery<T>(value: Value) -> Result<T, SearchEvidenceSchemaRejection>
where
    T: DeserializeOwned,
{
    let serialized = value.to_string();
    let mut deserializer = serde_json::Deserializer::from_str(&serialized);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let inner = error.inner().to_string();
        SearchEvidenceSchemaRejection {
            category: SearchEvidenceSchemaRejectionCategory::Decode,
            field: if path == "." {
                missing_field_name(&inner)
                    .unwrap_or("freehand_search_delivery")
                    .to_owned()
            } else {
                path
            },
            message: format!("schema decode failed: {inner}"),
        }
    })
}

fn missing_field_name(message: &str) -> Option<&str> {
    message
        .strip_prefix("missing field `")
        .and_then(|rest| rest.split_once('`').map(|(field, _)| field))
}

fn schema_rejection_from_validation(
    error: SearchEvidenceValidationError,
) -> SearchEvidenceSchemaRejection {
    match error {
        SearchEvidenceValidationError::InvalidField { field, reason } => {
            SearchEvidenceSchemaRejection {
                category: SearchEvidenceSchemaRejectionCategory::Validation,
                field,
                message: reason,
            }
        }
        SearchEvidenceValidationError::InvalidTransition { .. } => SearchEvidenceSchemaRejection {
            category: SearchEvidenceSchemaRejectionCategory::StateTransition,
            field: "state_transition".to_owned(),
            message: error.to_string(),
        },
        SearchEvidenceValidationError::InvalidSourceReference(source_id) => {
            SearchEvidenceSchemaRejection {
                category: SearchEvidenceSchemaRejectionCategory::SourceReference,
                field: "claims.source_ids".to_owned(),
                message: format!("references unknown or unverified source `{source_id}`"),
            }
        }
    }
}

fn extract_single_tagged_json<'a>(
    text: &'a str,
    start_tag: &str,
    end_tag: &str,
) -> Result<&'a str, SearchEvidenceSchemaRejection> {
    let Some(start_offset) = text.find(start_tag) else {
        return Err(SearchEvidenceSchemaRejection {
            category: SearchEvidenceSchemaRejectionCategory::TaggedBlock,
            field: "freehand_search_delivery".to_owned(),
            message: "missing `<freehand_search_delivery>...</freehand_search_delivery>` block"
                .to_owned(),
        });
    };
    if text[start_offset + start_tag.len()..].contains(start_tag) {
        return Err(SearchEvidenceSchemaRejection {
            category: SearchEvidenceSchemaRejectionCategory::TaggedBlock,
            field: "freehand_search_delivery".to_owned(),
            message: "must contain exactly one delivery block".to_owned(),
        });
    }
    let json_start = start_offset + start_tag.len();
    let Some(end_offset) = text[json_start..]
        .find(end_tag)
        .map(|offset| json_start + offset)
    else {
        return Err(SearchEvidenceSchemaRejection {
            category: SearchEvidenceSchemaRejectionCategory::TaggedBlock,
            field: "freehand_search_delivery".to_owned(),
            message: "missing closing `</freehand_search_delivery>` tag".to_owned(),
        });
    };
    if text[end_offset + end_tag.len()..].contains(end_tag) {
        return Err(SearchEvidenceSchemaRejection {
            category: SearchEvidenceSchemaRejectionCategory::TaggedBlock,
            field: "freehand_search_delivery".to_owned(),
            message: "must contain exactly one delivery block".to_owned(),
        });
    }
    Ok(&text[json_start..end_offset])
}

fn validate_parsed_search_delivery(
    delivery: &SearchEvidenceDelivery,
) -> Result<(), SearchEvidenceValidationError> {
    match delivery {
        SearchEvidenceDelivery::DomainPlan(delivery) => {
            validate_search_domain_plan_delivery(delivery)
        }
        SearchEvidenceDelivery::Discovery(delivery) => validate_search_discovery_delivery(delivery),
        SearchEvidenceDelivery::Verification(delivery) => {
            validate_search_verification_delivery(delivery)
        }
        SearchEvidenceDelivery::SupplementDecision(delivery) => {
            require_schema(&delivery.schema, SEARCH_SUPPLEMENT_SCHEMA)?;
            require_text(&delivery.delivery_id, "delivery_id")?;
            require_text(&delivery.domain_plan_ref, "domain_plan_ref")
        }
        SearchEvidenceDelivery::Final(delivery) => {
            require_schema(&delivery.schema, SEARCH_FINAL_SCHEMA)?;
            require_text(&delivery.delivery_id, "delivery_id")?;
            require_text(&delivery.domain_plan_ref, "domain_plan_ref")
        }
    }
}

pub fn validate_search_evidence_model_stage(
    stage: SearchEvidenceModelStage,
    delivery: &SearchEvidenceDelivery,
) -> Result<(), SearchEvidenceSchemaRejection> {
    let accepted = matches!(
        (stage, delivery),
        (
            SearchEvidenceModelStage::DomainPlan,
            SearchEvidenceDelivery::DomainPlan(_)
        ) | (
            SearchEvidenceModelStage::SupplementDecision,
            SearchEvidenceDelivery::SupplementDecision(_)
        ) | (
            SearchEvidenceModelStage::FinalDelivery,
            SearchEvidenceDelivery::Final(_)
        )
    );
    if accepted {
        Ok(())
    } else {
        Err(SearchEvidenceSchemaRejection {
            category: SearchEvidenceSchemaRejectionCategory::StageMismatch,
            field: "delivery_type".to_owned(),
            message: format!("delivery is not allowed in the current `{stage:?}` stage"),
        })
    }
}

pub fn validate_search_domain_plan_delivery(
    delivery: &SearchDomainPlanDelivery,
) -> Result<(), SearchEvidenceValidationError> {
    require_schema(&delivery.schema, SEARCH_DOMAIN_PLAN_SCHEMA)?;
    require_text(&delivery.delivery_id, "delivery_id")?;
    require_text(&delivery.policy_version, "policy_version")?;
    if delivery.preferred_source_kinds.is_empty()
        || delivery
            .preferred_source_kinds
            .iter()
            .any(|kind| kind.trim().is_empty())
    {
        return invalid_search_field(
            "preferred_source_kinds",
            "must contain non-empty source kinds",
        );
    }
    if delivery.minimum_verified_sources == 0 {
        return invalid_search_field("minimum_verified_sources", "must be at least 1");
    }
    match delivery.domain {
        SearchDomain::News => require_platform_first(
            &delivery.social_platform_priority,
            SearchSocialPlatform::Weibo,
            "social_platform_priority",
        ),
        SearchDomain::Tutorial | SearchDomain::Operations => require_platform_first(
            &delivery.social_platform_priority,
            SearchSocialPlatform::Xhs,
            "social_platform_priority",
        ),
        _ => Ok(()),
    }
}

pub fn validate_search_discovery_delivery(
    delivery: &SearchDiscoveryDelivery,
) -> Result<(), SearchEvidenceValidationError> {
    require_schema(&delivery.schema, SEARCH_DISCOVERY_SCHEMA)?;
    require_text(&delivery.delivery_id, "delivery_id")?;
    match delivery.discovery_channel {
        SearchDiscoveryChannel::HostedWebSearch => {
            let attempt = delivery.hosted_search_attempt.as_ref().ok_or_else(|| {
                SearchEvidenceValidationError::InvalidField {
                    field: "hosted_search_attempt".to_owned(),
                    reason: "is required for hosted discovery".to_owned(),
                }
            })?;
            require_text(&attempt.query, "hosted_search_attempt.query")?;
            require_text(&attempt.provider, "hosted_search_attempt.provider")?;
        }
        SearchDiscoveryChannel::CamoSocialSearch => {
            // Camo social discovery always belongs to a domain plan; hosted
            // discovery may be emitted without a plan (non-sourced profile),
            // so only camo requires the plan reference.
            require_text(
                delivery.domain_plan_ref.as_deref().unwrap_or_default(),
                "domain_plan_ref",
            )?;
            if delivery.hosted_search_attempt.is_some() {
                return invalid_search_field(
                    "hosted_search_attempt",
                    "must be absent for camo social discovery",
                );
            }
        }
        SearchDiscoveryChannel::WebFetch => {
            require_text(
                delivery.domain_plan_ref.as_deref().unwrap_or_default(),
                "domain_plan_ref",
            )?;
            if delivery.hosted_search_attempt.is_some() {
                return invalid_search_field(
                    "hosted_search_attempt",
                    "must be absent for web_fetch discovery",
                );
            }
        }
    }
    for candidate in &delivery.candidates {
        require_text(&candidate.candidate_id, "candidates.candidate_id")?;
        match candidate.status {
            SearchCandidateStatus::Usable => {
                let url = candidate.original_url.as_deref().ok_or_else(|| {
                    SearchEvidenceValidationError::InvalidField {
                        field: "candidates.original_url".to_owned(),
                        reason: "is required for usable candidates".to_owned(),
                    }
                })?;
                require_http_url(url, "candidates.original_url")?;
                if candidate.discovered_by != Some(delivery.discovery_channel) {
                    return invalid_search_field(
                        "candidates.discovered_by",
                        "must match discovery_channel for usable candidates",
                    );
                }
            }
            SearchCandidateStatus::UnusableMissingUrl => {
                if candidate.original_url.is_some() {
                    return invalid_search_field(
                        "candidates.original_url",
                        "must be absent for unusable_missing_url",
                    );
                }
            }
            SearchCandidateStatus::UnusableOther => {}
        }
    }
    Ok(())
}

pub fn validate_search_verification_delivery(
    delivery: &SearchVerificationDelivery,
) -> Result<(), SearchEvidenceValidationError> {
    require_schema(&delivery.schema, SEARCH_VERIFICATION_SCHEMA)?;
    require_text(&delivery.delivery_id, "delivery_id")?;
    require_text(&delivery.source_id, "source_id")?;
    require_http_url(&delivery.original_url, "original_url")?;
    require_text(&delivery.camo_profile, "camo_profile")?;
    require_text(&delivery.accessed_at, "accessed_at")?;
    if delivery.access_attempts.is_empty() {
        return invalid_search_field("access_attempts", "must not be empty");
    }
    for attempt in &delivery.access_attempts {
        require_text(&attempt.attempt_id, "access_attempts.attempt_id")?;
        if attempt.channel != "camo" {
            return invalid_search_field("access_attempts.channel", "must be `camo`");
        }
        require_text(&attempt.accessed_at, "access_attempts.accessed_at")?;
    }
    if delivery.access_status == SearchAccessStatus::Verified {
        if delivery.verified_by.as_deref() != Some("camo") {
            return invalid_search_field("verified_by", "must be `camo` for verified sources");
        }
        require_text(
            delivery.evidence_excerpt.as_deref().unwrap_or_default(),
            "evidence_excerpt",
        )?;
        if delivery.error.is_some() {
            return invalid_search_field("error", "must be absent for verified sources");
        }
    } else if delivery.verified_by.is_some() {
        return invalid_search_field("verified_by", "must be absent when access is not verified");
    }
    Ok(())
}

pub fn validate_social_supplement_decision_delivery(
    plan: &SearchDomainPlanDelivery,
    delivery: &SocialSupplementDecisionDelivery,
) -> Result<(), SearchEvidenceValidationError> {
    require_schema(&delivery.schema, SEARCH_SUPPLEMENT_SCHEMA)?;
    require_text(&delivery.delivery_id, "delivery_id")?;
    if delivery.domain_plan_ref != plan.delivery_id {
        return invalid_search_field("domain_plan_ref", "must reference the current domain plan");
    }
    if delivery.required && (delivery.reasons.is_empty() || delivery.platforms.is_empty()) {
        return invalid_search_field(
            "reasons/platforms",
            "must be non-empty when supplement is required",
        );
    }
    if !delivery.required && (!delivery.reasons.is_empty() || !delivery.platforms.is_empty()) {
        return invalid_search_field(
            "reasons/platforms",
            "must be empty when supplement is not required",
        );
    }
    if delivery.required {
        match plan.domain {
            SearchDomain::News => require_platform_first(
                &delivery.platforms,
                SearchSocialPlatform::Weibo,
                "platforms",
            ),
            SearchDomain::Tutorial | SearchDomain::Operations => {
                require_platform_first(&delivery.platforms, SearchSocialPlatform::Xhs, "platforms")
            }
            _ => Ok(()),
        }
    } else {
        Ok(())
    }
}

pub fn validate_search_final_delivery(
    plan: &SearchDomainPlanDelivery,
    verified_sources: &[SearchVerificationDelivery],
    delivery: &SearchFinalDelivery,
) -> Result<(), SearchEvidenceValidationError> {
    require_schema(&delivery.schema, SEARCH_FINAL_SCHEMA)?;
    require_text(&delivery.delivery_id, "delivery_id")?;
    if delivery.domain_plan_ref != plan.delivery_id {
        return invalid_search_field("domain_plan_ref", "must reference the current domain plan");
    }
    match delivery.claim {
        SearchFinalClaimStatus::Complete => {
            require_text(delivery.summary.as_deref().unwrap_or_default(), "summary")?;
            if delivery.claims.is_empty() {
                return invalid_search_field("claims", "must not be empty for complete");
            }
            let verified_count = verified_sources
                .iter()
                .filter(|source| source.access_status == SearchAccessStatus::Verified)
                .count();
            if verified_count < plan.minimum_verified_sources as usize {
                return invalid_search_field(
                    "verified_sources",
                    "does not meet minimum_verified_sources",
                );
            }
            for claim in &delivery.claims {
                require_text(&claim.claim_id, "claims.claim_id")?;
                require_text(&claim.text, "claims.text")?;
                if claim.source_ids.is_empty() {
                    return invalid_search_field("claims.source_ids", "must not be empty");
                }
                for source_id in &claim.source_ids {
                    if !verified_sources.iter().any(|source| {
                        source.source_id == *source_id
                            && source.access_status == SearchAccessStatus::Verified
                            && source.verified_by.as_deref() == Some("camo")
                    }) {
                        return Err(SearchEvidenceValidationError::InvalidSourceReference(
                            source_id.clone(),
                        ));
                    }
                }
            }
            if delivery.blocked_reason.is_some() {
                return invalid_search_field("blocked_reason", "must be absent for complete");
            }
        }
        SearchFinalClaimStatus::Blocked => {
            require_text(
                delivery.blocked_reason.as_deref().unwrap_or_default(),
                "blocked_reason",
            )?;
            if delivery.summary.is_some() || !delivery.claims.is_empty() {
                return invalid_search_field(
                    "summary/claims",
                    "must be absent or empty for blocked",
                );
            }
        }
    }
    Ok(())
}

pub fn validate_search_evidence_transition(
    from: SearchEvidenceTurnStatus,
    to: SearchEvidenceTurnStatus,
    supplement_required: bool,
) -> Result<(), SearchEvidenceValidationError> {
    let allowed = matches!(
        (from, to),
        (
            SearchEvidenceTurnStatus::DomainPlanValidated,
            SearchEvidenceTurnStatus::HostedDiscoveryValidated
        ) | (
            SearchEvidenceTurnStatus::HostedDiscoveryValidated,
            SearchEvidenceTurnStatus::CamoVerificationRequired
        ) | (
            SearchEvidenceTurnStatus::CamoVerificationRequired,
            SearchEvidenceTurnStatus::CamoVerificationValidated
        ) | (
            SearchEvidenceTurnStatus::CamoVerificationValidated,
            SearchEvidenceTurnStatus::SupplementDecisionValidated
        ) | (
            SearchEvidenceTurnStatus::SocialDiscoveryValidated,
            SearchEvidenceTurnStatus::CamoVerificationValidated
        ) | (
            SearchEvidenceTurnStatus::HostedDiscoveryValidated,
            SearchEvidenceTurnStatus::SupplementDecisionValidated
        ) | (
            SearchEvidenceTurnStatus::FinalValidated,
            SearchEvidenceTurnStatus::TurnTerminalSuccess
        )
    ) || (from == SearchEvidenceTurnStatus::SupplementDecisionValidated
        && (to == SearchEvidenceTurnStatus::Blocked
            || (supplement_required && to == SearchEvidenceTurnStatus::SocialDiscoveryValidated)
            || (!supplement_required && to == SearchEvidenceTurnStatus::FinalValidated)))
        || (from == SearchEvidenceTurnStatus::CamoVerificationValidated
            && matches!(
                to,
                SearchEvidenceTurnStatus::FinalValidated | SearchEvidenceTurnStatus::Blocked
            ));
    if allowed {
        Ok(())
    } else {
        Err(SearchEvidenceValidationError::InvalidTransition { from, to })
    }
}

pub fn validate_search_evidence_stage_append(
    existing: &[SearchEvidenceDelivery],
    next: &SearchEvidenceDelivery,
) -> Result<(), SearchEvidenceValidationError> {
    if existing
        .iter()
        .any(|delivery| matches!(delivery, SearchEvidenceDelivery::Final(_)))
    {
        return invalid_search_field("deliveries", "cannot append after final delivery");
    }
    match next {
        SearchEvidenceDelivery::DomainPlan(plan) => {
            if !existing.is_empty() {
                return invalid_search_field("domain_plan", "must be the first delivery");
            }
            validate_search_domain_plan_delivery(plan)
        }
        SearchEvidenceDelivery::Discovery(discovery) => {
            validate_search_discovery_delivery(discovery)?;
            match discovery.discovery_channel {
                SearchDiscoveryChannel::HostedWebSearch => {
                    match discovery.domain_plan_ref.as_deref() {
                        Some(plan_ref) => {
                            let plan = search_domain_plan(existing)?;
                            if plan_ref != plan.delivery_id {
                                return invalid_search_field(
                                    "domain_plan_ref",
                                    "must reference the persisted domain plan",
                                );
                            }
                            if existing.len() != 1
                                || existing.iter().any(|delivery| {
                                    matches!(delivery, SearchEvidenceDelivery::Discovery(_))
                                })
                            {
                                return invalid_search_field(
                                    "discovery_channel",
                                    "hosted discovery must immediately follow the domain plan",
                                );
                            }
                        }
                        None => {
                            // Non-sourced hosted discovery carries no domain plan. A single
                            // provider response may contain more than one hosted
                            // `server_tool_use` / `web_search_tool_result` pair (Anthropic
                            // and OpenAI both support that shape), so each pair is emitted
                            // as its own SearchDiscoveryDelivery. Multiple contiguous
                            // HostedWebSearch deliveries are accepted, but no other stage
                            // may open or re-open the non-sourced stream.
                            let allowed_prev = match existing.last() {
                                None => true,
                                Some(SearchEvidenceDelivery::Discovery(previous)) => {
                                    previous.discovery_channel
                                        == SearchDiscoveryChannel::HostedWebSearch
                                }
                                _ => false,
                            };
                            if !allowed_prev {
                                return invalid_search_field(
                                    "domain_plan_ref",
                                    "non-sourced hosted discovery must precede any other stage",
                                );
                            }
                        }
                    }
                }
                SearchDiscoveryChannel::CamoSocialSearch => {
                    let plan = search_domain_plan(existing)?;
                    if discovery.domain_plan_ref.as_deref() != Some(plan.delivery_id.as_str()) {
                        return invalid_search_field(
                            "domain_plan_ref",
                            "must reference the persisted domain plan",
                        );
                    }
                    let supplement = search_supplement_decision(existing)?;
                    if !supplement.required {
                        return invalid_search_field(
                            "discovery_channel",
                            "social discovery requires a required supplement decision",
                        );
                    }
                    ensure_all_usable_candidates_attempted(existing)?;
                    for candidate in &discovery.candidates {
                        let platform = candidate.platform.ok_or_else(|| {
                            SearchEvidenceValidationError::InvalidField {
                                field: "candidates.platform".to_owned(),
                                reason: "is required for camo social candidates".to_owned(),
                            }
                        })?;
                        if !supplement.platforms.contains(&platform) {
                            return invalid_search_field(
                                "candidates.platform",
                                "must be requested by the supplement decision",
                            );
                        }
                    }
                }
                SearchDiscoveryChannel::WebFetch => {
                    let plan = search_domain_plan(existing)?;
                    if discovery.domain_plan_ref.as_deref() != Some(plan.delivery_id.as_str()) {
                        return invalid_search_field(
                            "domain_plan_ref",
                            "must reference the persisted domain plan",
                        );
                    }
                    if existing.len() != 1
                        || existing.iter().any(|delivery| {
                            matches!(delivery, SearchEvidenceDelivery::Discovery(_))
                        })
                    {
                        return invalid_search_field(
                            "discovery_channel",
                            "web_fetch recovery must immediately follow the domain plan",
                        );
                    }
                }
            }
            Ok(())
        }
        SearchEvidenceDelivery::Verification(verification) => {
            validate_search_verification_delivery(verification)?;
            let candidate = existing
                .iter()
                .filter_map(|delivery| match delivery {
                    SearchEvidenceDelivery::Discovery(discovery) => Some(discovery),
                    _ => None,
                })
                .flat_map(|discovery| discovery.candidates.iter())
                .find(|candidate| candidate.candidate_id == verification.source_id)
                .ok_or_else(|| {
                    SearchEvidenceValidationError::InvalidSourceReference(
                        verification.source_id.clone(),
                    )
                })?;
            if candidate.status != SearchCandidateStatus::Usable
                || candidate.original_url.as_deref() != Some(verification.original_url.as_str())
            {
                return invalid_search_field(
                    "original_url",
                    "must match one persisted usable discovery candidate",
                );
            }
            if existing.iter().any(|delivery| {
                matches!(delivery, SearchEvidenceDelivery::Verification(previous)
                    if previous.source_id == verification.source_id)
            }) {
                return invalid_search_field(
                    "source_id",
                    "already has a persisted verification attempt",
                );
            }
            Ok(())
        }
        SearchEvidenceDelivery::SupplementDecision(supplement) => {
            let plan = search_domain_plan(existing)?;
            validate_social_supplement_decision_delivery(plan, supplement)?;
            if existing
                .iter()
                .any(|delivery| matches!(delivery, SearchEvidenceDelivery::SupplementDecision(_)))
            {
                return invalid_search_field(
                    "supplement_decision",
                    "only one supplement decision is allowed",
                );
            }
            if !existing.iter().any(|delivery| {
                matches!(delivery, SearchEvidenceDelivery::Discovery(discovery)
                    if discovery.discovery_channel == SearchDiscoveryChannel::HostedWebSearch)
            }) {
                return invalid_search_field(
                    "supplement_decision",
                    "requires hosted discovery first",
                );
            }
            ensure_all_usable_candidates_attempted(existing)
        }
        SearchEvidenceDelivery::Final(_) => invalid_search_field(
            "final",
            "must be applied through the final-delivery owner gate",
        ),
    }
}

pub fn project_search_evidence_stage_status(
    existing: &[SearchEvidenceDelivery],
    next: &SearchEvidenceDelivery,
) -> Result<SearchEvidenceTurnStatus, SearchEvidenceValidationError> {
    validate_search_evidence_stage_append(existing, next)?;
    match next {
        SearchEvidenceDelivery::DomainPlan(_) => Ok(SearchEvidenceTurnStatus::DomainPlanValidated),
        SearchEvidenceDelivery::Discovery(discovery) => {
            if discovery
                .candidates
                .iter()
                .any(|candidate| candidate.status == SearchCandidateStatus::Usable)
            {
                Ok(SearchEvidenceTurnStatus::CamoVerificationRequired)
            } else {
                Ok(match discovery.discovery_channel {
                    SearchDiscoveryChannel::HostedWebSearch => {
                        SearchEvidenceTurnStatus::HostedDiscoveryValidated
                    }
                    SearchDiscoveryChannel::CamoSocialSearch => {
                        SearchEvidenceTurnStatus::SocialDiscoveryValidated
                    }
                    SearchDiscoveryChannel::WebFetch => {
                        SearchEvidenceTurnStatus::CamoVerificationRequired
                    }
                })
            }
        }
        SearchEvidenceDelivery::Verification(_) => {
            let mut with_next = existing.to_vec();
            with_next.push(next.clone());
            if ensure_all_usable_candidates_attempted(&with_next).is_ok() {
                Ok(SearchEvidenceTurnStatus::CamoVerificationValidated)
            } else {
                Ok(SearchEvidenceTurnStatus::CamoVerificationRequired)
            }
        }
        SearchEvidenceDelivery::SupplementDecision(_) => {
            Ok(SearchEvidenceTurnStatus::SupplementDecisionValidated)
        }
        SearchEvidenceDelivery::Final(_) => invalid_search_field(
            "final",
            "must be applied through the final-delivery owner gate",
        ),
    }
}

pub fn build_search_evidence_turn_delivery(
    session_id: freehand_contracts::SessionId,
    turn_id: TurnId,
    mut deliveries: Vec<SearchEvidenceDelivery>,
    final_delivery: SearchFinalDelivery,
) -> Result<SearchEvidenceTurnDelivery, SearchEvidenceValidationError> {
    let mut validated = Vec::with_capacity(deliveries.len());
    for delivery in &deliveries {
        validate_search_evidence_stage_append(&validated, delivery)?;
        validated.push(delivery.clone());
    }
    let domain_plan = search_domain_plan(&deliveries)?.clone();
    let supplement = search_supplement_decision(&deliveries)?;
    ensure_all_usable_candidates_attempted(&deliveries)?;
    if final_delivery.claim == SearchFinalClaimStatus::Complete && supplement.required {
        for platform in &supplement.platforms {
            if !deliveries.iter().any(|delivery| {
                matches!(delivery, SearchEvidenceDelivery::Discovery(discovery)
                    if discovery.discovery_channel == SearchDiscoveryChannel::CamoSocialSearch
                        && discovery.candidates.iter().any(|candidate| candidate.platform == Some(*platform)))
            }) {
                return invalid_search_field(
                    "social_discovery",
                    format!("missing required `{platform:?}` social discovery"),
                );
            }
        }
    }
    let verified_sources = deliveries
        .iter()
        .filter_map(|delivery| match delivery {
            SearchEvidenceDelivery::Verification(source)
                if source.access_status == SearchAccessStatus::Verified =>
            {
                Some(source.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    validate_search_final_delivery(&domain_plan, &verified_sources, &final_delivery)?;
    let (status, summary_ready, terminal) = match final_delivery.claim {
        SearchFinalClaimStatus::Complete => (SearchEvidenceTurnStatus::FinalValidated, true, None),
        SearchFinalClaimStatus::Blocked => (
            SearchEvidenceTurnStatus::Blocked,
            false,
            Some(SearchEvidenceTerminal::Blocked),
        ),
    };
    let summary = final_delivery.summary.clone();
    let blocked_reason = final_delivery.blocked_reason.clone();
    let claims = final_delivery.claims.clone();
    let unconfirmed = final_delivery.unconfirmed.clone();
    deliveries.push(SearchEvidenceDelivery::Final(final_delivery));
    let turn_delivery = SearchEvidenceTurnDelivery {
        schema: SEARCH_TURN_SCHEMA.to_owned(),
        session_id,
        turn_id,
        domain_plan: Some(domain_plan),
        deliveries,
        verified_sources,
        unconfirmed,
        claims,
        status,
        summary_ready,
        summary,
        blocked_reason,
        terminal,
    };
    validate_search_evidence_turn_delivery(&turn_delivery)?;
    Ok(turn_delivery)
}

fn search_domain_plan(
    deliveries: &[SearchEvidenceDelivery],
) -> Result<&SearchDomainPlanDelivery, SearchEvidenceValidationError> {
    match deliveries.first() {
        Some(SearchEvidenceDelivery::DomainPlan(plan)) => Ok(plan),
        _ => invalid_search_field("domain_plan", "must be the first persisted delivery"),
    }
}

fn search_supplement_decision(
    deliveries: &[SearchEvidenceDelivery],
) -> Result<&SocialSupplementDecisionDelivery, SearchEvidenceValidationError> {
    deliveries
        .iter()
        .find_map(|delivery| match delivery {
            SearchEvidenceDelivery::SupplementDecision(decision) => Some(decision),
            _ => None,
        })
        .ok_or_else(|| SearchEvidenceValidationError::InvalidField {
            field: "supplement_decision".to_owned(),
            reason: "must be persisted before final delivery".to_owned(),
        })
}

fn ensure_all_usable_candidates_attempted(
    deliveries: &[SearchEvidenceDelivery],
) -> Result<(), SearchEvidenceValidationError> {
    let missing = deliveries
        .iter()
        .filter_map(|delivery| match delivery {
            SearchEvidenceDelivery::Discovery(discovery) => Some(discovery),
            _ => None,
        })
        .flat_map(|discovery| discovery.candidates.iter())
        .filter(|candidate| candidate.status == SearchCandidateStatus::Usable)
        .find(|candidate| {
            !deliveries.iter().any(|delivery| {
                matches!(delivery, SearchEvidenceDelivery::Verification(verification)
                    if verification.source_id == candidate.candidate_id)
            })
        });
    if let Some(candidate) = missing {
        Err(SearchEvidenceValidationError::InvalidSourceReference(
            candidate.candidate_id.clone(),
        ))
    } else {
        Ok(())
    }
}

pub fn validate_search_evidence_turn_delivery(
    delivery: &SearchEvidenceTurnDelivery,
) -> Result<(), SearchEvidenceValidationError> {
    require_schema(&delivery.schema, SEARCH_TURN_SCHEMA)?;
    if let Some(plan) = &delivery.domain_plan {
        validate_search_domain_plan_delivery(plan)?;
    }
    match delivery.terminal {
        Some(SearchEvidenceTerminal::Success) => {
            if !delivery.summary_ready
                || delivery.status != SearchEvidenceTurnStatus::TurnTerminalSuccess
                || delivery
                    .summary
                    .as_deref()
                    .is_none_or(|summary| summary.trim().is_empty())
            {
                return invalid_search_field(
                    "summary_ready/status/summary",
                    "must represent a validated complete final delivery",
                );
            }
        }
        Some(SearchEvidenceTerminal::Blocked) => {
            if delivery.summary_ready
                || delivery.status != SearchEvidenceTurnStatus::Blocked
                || delivery
                    .blocked_reason
                    .as_deref()
                    .is_none_or(|reason| reason.trim().is_empty())
            {
                return invalid_search_field(
                    "summary_ready/status/blocked_reason",
                    "must represent an explicit blocked final delivery",
                );
            }
        }
        None => {}
    }
    if delivery.status == SearchEvidenceTurnStatus::FinalValidated
        && (!delivery.summary_ready
            || delivery.terminal.is_some()
            || delivery
                .summary
                .as_deref()
                .is_none_or(|summary| summary.trim().is_empty()))
    {
        return invalid_search_field(
            "status/summary_ready/terminal",
            "final_validated must be summary-ready and non-terminal",
        );
    }
    Ok(())
}

fn require_schema(
    value: &str,
    expected: &'static str,
) -> Result<(), SearchEvidenceValidationError> {
    if value == expected {
        Ok(())
    } else {
        invalid_search_field("schema", format!("must be `{expected}`"))
    }
}

fn require_text(value: &str, field: &'static str) -> Result<(), SearchEvidenceValidationError> {
    if value.trim().is_empty() {
        invalid_search_field(field, "must not be empty")
    } else {
        Ok(())
    }
}

fn require_http_url(value: &str, field: &'static str) -> Result<(), SearchEvidenceValidationError> {
    let authority = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"));
    if authority.is_some_and(|authority| {
        let host = authority.split(['/', '?', '#']).next().unwrap_or_default();
        !host.is_empty() && !host.chars().any(char::is_whitespace)
    }) {
        Ok(())
    } else {
        invalid_search_field(field, "must be an http or https URL")
    }
}

fn require_platform_first(
    platforms: &[SearchSocialPlatform],
    expected: SearchSocialPlatform,
    field: &'static str,
) -> Result<(), SearchEvidenceValidationError> {
    if platforms.first() == Some(&expected) {
        Ok(())
    } else {
        invalid_search_field(field, format!("must start with `{expected:?}`"))
    }
}

fn invalid_search_field<T>(
    field: impl Into<String>,
    reason: impl Into<String>,
) -> Result<T, SearchEvidenceValidationError> {
    Err(SearchEvidenceValidationError::InvalidField {
        field: field.into(),
        reason: reason.into(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompletionClaim {
    Complete,
    Continue,
    Waiting,
    Blocked,
}

impl CompletionClaim {
    fn parse(input: &str) -> Option<Self> {
        match input {
            "complete" => Some(Self::Complete),
            "continue" => Some(Self::Continue),
            "waiting" => Some(Self::Waiting),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_options: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionDecision {
    Completed {
        status: TerminalStatus,
        terminal_text: String,
    },
    Waiting {
        status: TerminalStatus,
        terminal_text: String,
        user_options: Option<Vec<String>>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionSchemaIssue {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
            "<freehand_completion>\n{\n",
            "  \"claim\": \"complete\" | \"continue\" | \"waiting\" | \"blocked\"\n",
            "}\n</freehand_completion>\n",
            "The block content must be valid JSON only: double-quoted keys, double-quoted string values, no comments, no trailing commas, no markdown fence inside the tag.\n",
            "Required fields per claim:\n",
            "- claim=complete: completion_reason, evidence, summary, learned (all plain strings)\n",
            "- claim=continue: next_step (plain string)\n",
            "- claim=waiting: next_step (plain string); optionally user_options (array of strings)\n",
            "- claim=blocked: blocked_reason (plain string)\n",
            "Valid complete example: <freehand_completion>\n{\"claim\":\"complete\",\"completion_reason\":\"provider returned the requested result\",\"evidence\":\"verified online sample matched expected output\",\"summary\":\"finished the requested task\",\"learned\":\"keep tagged completion JSON strict and escaped\"}\n</freehand_completion>\n",
            "Valid blocked example: <freehand_completion>\n{\"claim\":\"blocked\",\"blocked_reason\":\"missing required capability after checking Master and Worker tool surfaces\"}\n</freehand_completion>\n",
            "Valid waiting example: <freehand_completion>\n{\"claim\":\"waiting\",\"next_step\":\"Worker task is assigned; re-check TaskBoard and review the result before final synthesis\"}\n</freehand_completion>\n",
            "String values are literal JSON strings. If a value must contain a double quote, escape it with a backslash: \\\" inside the string, for example \"reason\": \"cannot do \\\"web search\\\" now\". ",
            "Never place a bare double quote inside a string value.\n",
            "Use plain string values for required text fields; do not emit arrays or objects for those fields.\n",
            "Do not explain schema repair in prose instead of emitting the fixed JSON block. A sentence such as `I need to fix the JSON syntax...` is not a valid completion by itself.\n",
            "Claim semantics: `continue` means Freehand should immediately run another model round in this same turn. ",
            "Do not use `continue` to wait for a Worker, timer, user, or external future event. ",
            "`complete` means the user's requested outcome is actually finished with evidence. ",
            "Dispatching a Worker task, scheduling a timer, or waiting for external lifecycle truth is not user-task completion. ",
            "After dispatching work or scheduling a needed timer, finish the current turn with `claim=\"waiting\"` and put the exact lifecycle follow-up in `next_step`. ",
            "Use `waiting` only when Freehand/Task Center/timer truth will continue the lifecycle without another user message."
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
    let trimmed = raw_json.trim();
    let value: Value = match serde_json::from_str(trimmed) {
        Ok(value) => value,
        Err(err) => {
            let repaired = tolerant_json_repair(trimmed);
            match serde_json::from_str(&repaired) {
                Ok(value) => value,
                Err(repair_err) => {
                    return Err(schema_rejection([CompletionSchemaIssue {
                        field: "freehand_completion".to_owned(),
                        message: format!(
                            "invalid JSON: {err} (tolerant repair also failed: {repair_err})"
                        ),
                    }]));
                }
            }
        }
    };
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
                    message: "must be one of `complete`, `continue`, `waiting`, or `blocked`"
                        .to_owned(),
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
        completion_reason: optional_string_field(&mut issues, object, "completion_reason"),
        evidence: optional_string_field(&mut issues, object, "evidence"),
        summary: optional_string_field(&mut issues, object, "summary"),
        learned: optional_string_field(&mut issues, object, "learned"),
        next_step: optional_string_field(&mut issues, object, "next_step"),
        blocked_reason: optional_string_field(&mut issues, object, "blocked_reason"),
        user_options: optional_string_list_field(&mut issues, object, "user_options"),
    };

    if !issues.is_empty() {
        return Err(CompletionSchemaRejection { issues });
    }

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

/// Best-effort repair of common model-emitted JSON syntax mistakes, applied only
/// when the strict parser rejects the block. Each repair preserves semantics:
/// - bare double-quote inside a string value -> escaped `\"` (the quote stays part
///   of the value, matching what the model meant to write);
/// - trailing comma before `}` or `]` -> removed;
/// - bare `None` / `True` / `False` tokens -> canonical `null` / `true` / `false`.
///
/// If the input cannot be repaired into valid JSON the function returns the input
/// unchanged, so the caller still reports invalid JSON.
fn tolerant_json_repair(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            if escaped {
                out.push(c);
                escaped = false;
                i += 1;
                continue;
            }
            match c {
                '\\' => {
                    out.push(c);
                    escaped = true;
                    i += 1;
                }
                '"' => {
                    // Determine whether this quote terminates the string. It is a
                    // terminator only when the next non-whitespace char is a JSON
                    // structural delimiter or the input ends.
                    let mut j = i + 1;
                    while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                        j += 1;
                    }
                    let terminates = j >= chars.len() || matches!(chars[j], ',' | '}' | ']' | ':');
                    if terminates {
                        out.push('"');
                        in_string = false;
                    } else {
                        // Unescaped quote inside the value: escape it so the value
                        // keeps the quote as literal text.
                        out.push('\\');
                        out.push('"');
                    }
                    i += 1;
                }
                _ => {
                    out.push(c);
                    i += 1;
                }
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
                i += 1;
            }
            ',' => {
                // Trailing comma before a closing brace/bracket: drop it.
                let mut j = i + 1;
                while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                    j += 1;
                }
                if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                    // skip the comma
                } else {
                    out.push(c);
                }
                i += 1;
            }
            'N' if is_word_at(&chars, i, "None") => {
                out.push_str("null");
                i += 4;
            }
            'T' if is_word_at(&chars, i, "True") => {
                out.push_str("true");
                i += 4;
            }
            'F' if is_word_at(&chars, i, "False") => {
                out.push_str("false");
                i += 5;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn is_word_at(chars: &[char], start: usize, word: &str) -> bool {
    let w: Vec<char> = word.chars().collect();
    if start + w.len() > chars.len() {
        return false;
    }
    for (idx, wc) in w.iter().enumerate() {
        if chars[start + idx] != *wc {
            return false;
        }
    }
    // Boundary: the token must not be followed by an identifier char.
    if let Some(&next) = chars.get(start + w.len())
        && (next.is_alphanumeric() || next == '_')
    {
        return false;
    }
    true
}

fn optional_string_field(
    issues: &mut Vec<CompletionSchemaIssue>,
    object: &Map<String, Value>,
    field: &'static str,
) -> Option<String> {
    match object.get(field) {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(value) => {
            issues.push(CompletionSchemaIssue {
                field: field.to_owned(),
                message: format!("must be a string, got {}", schema_value_type_label(value)),
            });
            None
        }
    }
}

fn string_field(object: &Map<String, Value>, field: &'static str) -> Option<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn optional_string_list_field(
    issues: &mut Vec<CompletionSchemaIssue>,
    object: &Map<String, Value>,
    field: &'static str,
) -> Option<Vec<String>> {
    match object.get(field) {
        None | Some(Value::Null) => None,
        Some(Value::Array(values)) => {
            let mut strings = Vec::with_capacity(values.len());
            for value in values {
                match value {
                    Value::String(text) => strings.push(text.clone()),
                    other => {
                        issues.push(CompletionSchemaIssue {
                            field: field.to_owned(),
                            message: format!(
                                "must be an array of strings, got {}",
                                schema_value_type_label(other)
                            ),
                        });
                        return None;
                    }
                }
            }
            if strings.is_empty() {
                None
            } else {
                Some(strings)
            }
        }
        Some(other) => {
            issues.push(CompletionSchemaIssue {
                field: field.to_owned(),
                message: format!(
                    "must be an array of strings, got {}",
                    schema_value_type_label(other)
                ),
            });
            None
        }
    }
}

fn schema_value_type_label(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
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
        CompletionClaim::Continue | CompletionClaim::Waiting => {
            let mut issues = Vec::new();
            collect_required_text_issue(&mut issues, submission.next_step.as_deref(), "next_step");
            if issues.is_empty() {
                issues
            } else {
                vec![CompletionSchemaIssue {
                    field: "next_step".to_owned(),
                    message: format!(
                        "is required when `claim` is `{}`",
                        claim_label(submission.claim)
                    ),
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

fn claim_label(claim: CompletionClaim) -> &'static str {
    match claim {
        CompletionClaim::Complete => "complete",
        CompletionClaim::Continue => "continue",
        CompletionClaim::Waiting => "waiting",
        CompletionClaim::Blocked => "blocked",
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
        CompletionClaim::Waiting => {
            let next_step = required_text(submission.next_step.as_deref(), "next_step")
                .map_err(|_| CompletionValidationError::MissingNextStep)?;
            let user_options = submission
                .user_options
                .clone()
                .filter(|options| !options.is_empty());
            let (status, prefix) = if user_options.is_some() {
                (
                    TerminalStatus::AwaitingUserOptions,
                    "Waiting for user options",
                )
            } else {
                (TerminalStatus::ToolPending, "Waiting for lifecycle")
            };
            Ok(CompletionDecision::Waiting {
                status,
                terminal_text: format!("{prefix}: {next_step}"),
                user_options,
            })
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
        if segment.stability == ContextStability::TurnVolatile {
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
        ContextSegmentKind::InstructionCapability => "instruction_capability",
        ContextSegmentKind::TaskContract => "task_contract",
        ContextSegmentKind::TaskSpaceSnapshot => "task_space_snapshot",
        ContextSegmentKind::CurrentTime => "current_time",
        ContextSegmentKind::AttentionResolution => "attention_resolution",
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
        ContextSegmentKind::InstructionCapability => (
            ContextStability::SessionStable,
            ContextCachePolicy::Cacheable,
            Some(ContextRole::Developer),
        ),
        ContextSegmentKind::TaskContract => (
            ContextStability::SessionStable,
            ContextCachePolicy::Cacheable,
            Some(ContextRole::Developer),
        ),
        ContextSegmentKind::TaskSpaceSnapshot => (
            ContextStability::TurnVolatile,
            ContextCachePolicy::NoCache,
            Some(ContextRole::Developer),
        ),
        ContextSegmentKind::CurrentTime => (
            ContextStability::TurnVolatile,
            ContextCachePolicy::NoCache,
            Some(ContextRole::Developer),
        ),
        ContextSegmentKind::AttentionResolution => (
            ContextStability::TurnVolatile,
            ContextCachePolicy::NoCache,
            Some(ContextRole::Developer),
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
        ContextSegmentKind::InstructionCapability => 3,
        ContextSegmentKind::TaskContract => 4,
        ContextSegmentKind::SessionMemory => 5,
        ContextSegmentKind::SessionSummary => 6,
        ContextSegmentKind::TaskSpaceSnapshot => 7,
        ContextSegmentKind::CurrentTime => 8,
        ContextSegmentKind::AttentionResolution => 9,
        ContextSegmentKind::SubagentConclusion => 10,
        ContextSegmentKind::ToolResultEvidence => 11,
        ContextSegmentKind::UserTurnInput => 12,
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
    use freehand_contracts::SessionId;
    use serde_json::json;

    #[test]
    fn search_evidence_contract_guidance_uses_typed_examples_without_control_semantics() {
        let guidance = search_evidence_contract_guidance().expect("contract guidance");

        for schema in [
            SEARCH_DOMAIN_PLAN_SCHEMA,
            SEARCH_SUPPLEMENT_SCHEMA,
            SEARCH_FINAL_SCHEMA,
        ] {
            assert!(guidance.contains(schema), "missing schema `{schema}`");
        }
        assert!(guidance.contains("\"unconfirmed\":[]"));
        assert!(guidance.contains("\"minimum_verified_sources\":2"));
        assert!(guidance.contains("Never invent URLs"));
        assert!(!guidance.contains("Stage order:"));
        assert!(!guidance.contains("current model-authored stage"));
        assert!(!guidance.contains("retry"));
        assert!(!guidance.contains("provider selection"));
    }

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
            user_options: None,
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
            user_options: None,
        })
        .expect_err("should fail");
        assert_eq!(err, CompletionValidationError::MissingField("evidence"));
    }

    #[test]
    fn accepts_waiting_submission_as_tool_pending_not_success() {
        let decision = validate_completion_submission(&CompletionSubmission {
            claim: CompletionClaim::Waiting,
            completion_reason: None,
            evidence: None,
            summary: None,
            learned: None,
            next_step: Some(
                "Worker task is assigned; timer will re-check TaskBoard before final answer"
                    .to_owned(),
            ),
            user_options: None,
            blocked_reason: None,
        })
        .expect("valid waiting claim");

        match decision {
            CompletionDecision::Waiting {
                status,
                terminal_text,
                user_options: _,
            } => {
                assert_eq!(status, TerminalStatus::ToolPending);
                assert!(terminal_text.contains("Waiting for lifecycle"));
                assert!(terminal_text.contains("Worker task is assigned"));
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn waiting_submission_with_user_options_projects_awaiting_user_options() {
        let decision = validate_completion_submission(&CompletionSubmission {
            claim: CompletionClaim::Waiting,
            completion_reason: None,
            evidence: None,
            summary: None,
            learned: None,
            next_step: Some("Please choose how to proceed".to_owned()),
            user_options: Some(vec!["Retry".to_owned(), "Cancel".to_owned()]),
            blocked_reason: None,
        })
        .expect("valid waiting claim with user options");

        match decision {
            CompletionDecision::Waiting {
                status,
                terminal_text,
                user_options,
            } => {
                assert_eq!(status, TerminalStatus::AwaitingUserOptions);
                assert!(terminal_text.contains("Waiting for user options"));
                assert!(terminal_text.contains("Please choose how to proceed"));
                assert_eq!(
                    user_options,
                    Some(vec!["Retry".to_owned(), "Cancel".to_owned()])
                );
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn waiting_submission_with_empty_user_options_falls_back_to_tool_pending() {
        let decision = validate_completion_submission(&CompletionSubmission {
            claim: CompletionClaim::Waiting,
            completion_reason: None,
            evidence: None,
            summary: None,
            learned: None,
            next_step: Some("Waiting on lifecycle".to_owned()),
            user_options: Some(Vec::new()),
            blocked_reason: None,
        })
        .expect("valid waiting claim");

        match decision {
            CompletionDecision::Waiting {
                status,
                terminal_text,
                user_options,
            } => {
                assert_eq!(status, TerminalStatus::ToolPending);
                assert!(terminal_text.contains("Waiting for lifecycle"));
                assert_eq!(user_options, None);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn completion_guidance_forbids_dispatch_as_user_completion() {
        let guidance = completion_schema_guidance().prompt;
        assert!(guidance.contains("\"waiting\""));
        assert!(guidance.contains("Dispatching a Worker task"));
        assert!(guidance.contains("is not user-task completion"));
        assert!(guidance.contains("claim=\"waiting\""));
        assert!(guidance.contains("valid JSON only"));
        assert!(guidance.contains("no trailing commas"));
        assert!(guidance.contains("Do not explain schema repair in prose"));
        assert!(guidance.contains("Valid blocked example"));
        assert!(guidance.contains("Valid waiting example"));
    }

    #[test]
    fn rejects_completion_schema_with_non_string_evidence() {
        let err = parse_completion_submission_block(
            r#"
<freehand_completion>
{
  "claim": "complete",
  "completion_reason": "done",
  "evidence": ["pwd", "/tmp"],
  "summary": "ok",
  "learned": "keep evidence compact"
}
</freehand_completion>
"#,
        )
        .expect_err("should fail");
        assert_eq!(err.issues.len(), 1);
        assert_eq!(err.issues[0].field, "evidence");
        assert!(err.issues[0].message.contains("must be a string"));
        assert!(err.issues[0].message.contains("array"));
    }

    #[test]
    fn accepts_null_for_unused_optional_completion_fields() {
        let parsed = parse_completion_submission_block(
            r#"
<freehand_completion>
{
  "claim": "complete",
  "completion_reason": "done",
  "evidence": "verified",
  "summary": "ok",
  "learned": "keep schema typed",
  "next_step": null,
  "blocked_reason": null
}
</freehand_completion>
"#,
        )
        .expect("nullable optional fields");

        assert_eq!(parsed.next_step, None);
        assert_eq!(parsed.blocked_reason, None);
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
            user_options: None,
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
            ContextSegmentKind::SessionMemory
            | ContextSegmentKind::SessionSummary
            | ContextSegmentKind::InstructionCapability
            | ContextSegmentKind::TaskContract => (
                ContextStability::SessionStable,
                ContextCachePolicy::Cacheable,
                ContextRole::Developer,
            ),
            ContextSegmentKind::TaskSpaceSnapshot => (
                ContextStability::TurnVolatile,
                ContextCachePolicy::NoCache,
                ContextRole::Developer,
            ),
            ContextSegmentKind::CurrentTime => (
                ContextStability::TurnVolatile,
                ContextCachePolicy::NoCache,
                ContextRole::Developer,
            ),
            ContextSegmentKind::AttentionResolution => (
                ContextStability::TurnVolatile,
                ContextCachePolicy::NoCache,
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
                segment(
                    "task-contract",
                    ContextSegmentKind::TaskContract,
                    "target contract",
                    16,
                    "task_space",
                ),
                segment(
                    "task-snapshot",
                    ContextSegmentKind::TaskSpaceSnapshot,
                    "phase executing",
                    16,
                    "task_space",
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
            vec![
                "head-system",
                "task-contract",
                "head-memory",
                "task-snapshot",
                "tail-sub",
                "turn-user"
            ]
        );
        assert_eq!(
            planned.diagnostics.rewrite_mode,
            ContextRewriteMode::OrdinaryTurn
        );
        assert_eq!(planned.diagnostics.stable_segment_hashes.len(), 3);
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
    fn instruction_capability_segment_is_stable_typed_context() {
        let planned_a = plan_context(ContextPlannerInput {
            candidate_segments: vec![segment(
                "instruction-capability",
                ContextSegmentKind::InstructionCapability,
                "manifest fingerprint: abc",
                16,
                "instruction_capability",
            )],
            current_user_text: "continue".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: None,
        })
        .expect("planned a");
        let planned_b = plan_context(ContextPlannerInput {
            candidate_segments: vec![segment(
                "instruction-capability",
                ContextSegmentKind::InstructionCapability,
                "manifest fingerprint: def",
                16,
                "instruction_capability",
            )],
            current_user_text: "continue".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: None,
        })
        .expect("planned b");

        let instruction_segment = planned_a
            .ordered_segments
            .iter()
            .find(|segment| segment.segment_id.as_str() == "instruction-capability")
            .expect("instruction segment");
        assert_eq!(
            instruction_segment.kind,
            ContextSegmentKind::InstructionCapability
        );
        assert_eq!(
            instruction_segment.stability,
            ContextStability::SessionStable
        );
        assert_eq!(
            instruction_segment.cache_policy,
            ContextCachePolicy::Cacheable
        );
        assert_eq!(instruction_segment.role, ContextRole::Developer);
        assert!(
            render_context_segments_as_text(&planned_a.ordered_segments)
                .contains("kind=\"instruction_capability\"")
        );
        assert_ne!(
            planned_a.diagnostics.stable_prefix_hash,
            planned_b.diagnostics.stable_prefix_hash
        );
    }

    #[test]
    fn attention_resolution_segment_is_turn_volatile_typed_context() {
        let planned = plan_context(ContextPlannerInput {
            candidate_segments: vec![
                segment(
                    "task-snapshot",
                    ContextSegmentKind::TaskSpaceSnapshot,
                    "phase: attention resolved",
                    16,
                    "task_space",
                ),
                segment(
                    "attention-resolution:event-1",
                    ContextSegmentKind::AttentionResolution,
                    r#"{"attention_event_id":"event-1","decision_kind":"task_advanced","changed_task_ids":["task-1"],"changed_constraints":[],"resume_from":{"work_id":"work-1","session_id":"session-1","logical_turn_id":"runtime-turn-1","trace_id":"runtime-trace-1"}}"#,
                    128,
                    "master_work.attention_resolution",
                ),
            ],
            current_user_text: "continue original work".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: None,
        })
        .expect("attention resolution planned");

        let resolution = planned
            .ordered_segments
            .iter()
            .find(|segment| segment.kind == ContextSegmentKind::AttentionResolution)
            .expect("attention resolution segment");
        assert_eq!(resolution.stability, ContextStability::TurnVolatile);
        assert_eq!(resolution.cache_policy, ContextCachePolicy::NoCache);
        assert_eq!(resolution.role, ContextRole::Developer);
        assert!(
            render_context_segments_as_text(&planned.ordered_segments)
                .contains("kind=\"attention_resolution\"")
        );
        assert!(
            planned
                .ordered_segments
                .iter()
                .position(|segment| segment.kind == ContextSegmentKind::TaskSpaceSnapshot)
                < planned
                    .ordered_segments
                    .iter()
                    .position(|segment| segment.kind == ContextSegmentKind::AttentionResolution)
        );
    }

    #[test]
    fn attention_resolution_segment_is_rejected_from_rewrite_base() {
        let error = validate_rewrite_base_segments(&[segment(
            "attention-resolution:event-1",
            ContextSegmentKind::AttentionResolution,
            r#"{"attention_event_id":"event-1"}"#,
            16,
            "master_work.attention_resolution",
        )])
        .expect_err("attention resolution must remain turn volatile");

        assert!(matches!(
            error,
            ContextPlannerError::InvalidRewriteSegmentKind {
                segment_id,
                kind
            } if segment_id == "attention-resolution:event-1" && kind == "attention_resolution"
        ));
    }

    #[test]
    fn task_contract_changes_stable_prefix_but_snapshot_does_not() {
        let planned_a = plan_context(ContextPlannerInput {
            candidate_segments: vec![
                segment(
                    "task-contract",
                    ContextSegmentKind::TaskContract,
                    "target: audit context ordering",
                    16,
                    "task_space",
                ),
                segment(
                    "task-snapshot",
                    ContextSegmentKind::TaskSpaceSnapshot,
                    "phase: inspect",
                    16,
                    "task_space",
                ),
            ],
            current_user_text: "continue".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: None,
        })
        .expect("planned a");
        let planned_snapshot_changed = plan_context(ContextPlannerInput {
            candidate_segments: vec![
                segment(
                    "task-contract",
                    ContextSegmentKind::TaskContract,
                    "target: audit context ordering",
                    16,
                    "task_space",
                ),
                segment(
                    "task-snapshot",
                    ContextSegmentKind::TaskSpaceSnapshot,
                    "phase: validate",
                    16,
                    "task_space",
                ),
            ],
            current_user_text: "continue".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: None,
        })
        .expect("planned snapshot changed");
        let planned_contract_changed = plan_context(ContextPlannerInput {
            candidate_segments: vec![
                segment(
                    "task-contract",
                    ContextSegmentKind::TaskContract,
                    "target: implement context ordering",
                    16,
                    "task_space",
                ),
                segment(
                    "task-snapshot",
                    ContextSegmentKind::TaskSpaceSnapshot,
                    "phase: inspect",
                    16,
                    "task_space",
                ),
            ],
            current_user_text: "continue".to_owned(),
            user_segment_id: ContextSegmentId::new("turn-user"),
            user_provenance: ContextProvenance {
                source: "turn_input".to_owned(),
                reference: None,
            },
            rewrite_mode: ContextRewriteMode::OrdinaryTurn,
            rewrite_version: 0,
            tool_schema_fingerprint: None,
        })
        .expect("planned contract changed");

        assert_eq!(
            planned_a.diagnostics.stable_prefix_hash,
            planned_snapshot_changed.diagnostics.stable_prefix_hash
        );
        assert_ne!(
            planned_a.diagnostics.stable_prefix_hash,
            planned_contract_changed.diagnostics.stable_prefix_hash
        );
    }

    #[test]
    fn planner_diagnostics_drift_when_tool_schema_fingerprint_changes() {
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
            tool_schema_fingerprint: Some("tool-v2".to_owned()),
        })
        .expect("planned b");

        assert_eq!(
            planned_a.diagnostics.stable_prefix_hash,
            planned_b.diagnostics.stable_prefix_hash
        );
        assert_ne!(
            planned_a.diagnostics.tool_schema_hash,
            planned_b.diagnostics.tool_schema_hash
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
    fn recovers_unescaped_quote_inside_string_value() {
        // Reproduces the real r3 failure: the blocked_reason value contained bare
        // Chinese double quotes around `web search`, which strict JSON rejects.
        let submission = parse_completion_submission_block(
            "<freehand_completion>\n{\"claim\":\"blocked\",\"blocked_reason\":\"Master本地与Worker均无可用的真实网页搜索工具...无法为用户完成\\\"为什么你自己不去核实并进行真实搜索\\\"的请求,需要用户在工具能力或继续重试搜索任务之间做出选择。\"}\n</freehand_completion>",
        )
        .expect("must tolerate unescaped quote inside string value");
        assert_eq!(submission.claim, CompletionClaim::Blocked);
        assert!(
            submission
                .blocked_reason
                .as_deref()
                .unwrap()
                .contains("\"为什么你自己不去核实并进行真实搜索\"")
        );
    }

    #[test]
    fn recovers_trailing_comma_before_closing_brace() {
        let submission = parse_completion_submission_block(
            "<freehand_completion>\n{\"claim\":\"blocked\",\"blocked_reason\":\"missing capability\",}\n</freehand_completion>",
        )
        .expect("must tolerate trailing comma");
        assert_eq!(submission.claim, CompletionClaim::Blocked);
        assert_eq!(
            submission.blocked_reason.as_deref(),
            Some("missing capability")
        );
    }

    #[test]
    fn recovers_python_none_true_false_tokens() {
        // Python-style literals inside a JSON object are canonicalized to their
        // JSON equivalents; the repaired text must parse as valid JSON.
        let raw = "{\"claim\":\"continue\",\"next_step\":\"poll TaskBoard\",\"user_options\":[\"retry\",None,True,False]}";
        let repaired = tolerant_json_repair(raw);
        assert_eq!(
            repaired,
            "{\"claim\":\"continue\",\"next_step\":\"poll TaskBoard\",\"user_options\":[\"retry\",null,true,false]}"
        );
        let parsed: serde_json::Value = serde_json::from_str(&repaired).unwrap();
        assert_eq!(parsed["user_options"][1], serde_json::Value::Null);
        assert_eq!(parsed["user_options"][2], serde_json::Value::Bool(true));
        assert_eq!(parsed["user_options"][3], serde_json::Value::Bool(false));
    }

    #[test]
    fn tolerant_repair_preserves_already_valid_json() {
        let raw = "{\"claim\":\"blocked\",\"blocked_reason\":\"missing capability\"}";
        assert_eq!(tolerant_json_repair(raw), raw);
    }

    #[test]
    fn tolerant_repair_does_not_escape_proper_string_terminators() {
        // A normal string value with a colon/comma/brace right after its closing
        // quote must remain untouched.
        let raw = "{\"claim\":\"blocked\",\"blocked_reason\":\"a: b, c\"}";
        assert_eq!(tolerant_json_repair(raw), raw);
        let parsed: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed["blocked_reason"], "a: b, c");
    }

    #[test]
    fn completion_schema_feedback_matrix_is_type_aware() {
        let cases = [
            (
                "missing tag",
                "pong",
                "freehand_completion",
                "missing `<freehand_completion>...</freehand_completion>` block",
            ),
            (
                "invalid json",
                "<freehand_completion>\n{\"claim\":\"complete\"\n</freehand_completion>",
                "freehand_completion",
                "invalid JSON",
            ),
            (
                "invalid claim",
                "<freehand_completion>\n{\"claim\":\"done\"}\n</freehand_completion>",
                "claim",
                "must be one of `complete`, `continue`, `waiting`, or `blocked`",
            ),
            (
                "missing complete field",
                "<freehand_completion>\n{\"claim\":\"complete\",\"summary\":\"pong\"}\n</freehand_completion>",
                "evidence",
                "is required",
            ),
            (
                "type mismatch",
                "<freehand_completion>\n{\"claim\":\"complete\",\"completion_reason\":\"done\",\"evidence\":[\"pwd\"],\"summary\":\"pong\",\"learned\":\"keep schema strict\"}\n</freehand_completion>",
                "evidence",
                "must be a string, got array",
            ),
            (
                "continue missing next_step",
                "<freehand_completion>\n{\"claim\":\"continue\"}\n</freehand_completion>",
                "next_step",
                "is required when `claim` is `continue`",
            ),
            (
                "blocked missing reason",
                "<freehand_completion>\n{\"claim\":\"blocked\"}\n</freehand_completion>",
                "blocked_reason",
                "is required when `claim` is `blocked`",
            ),
            (
                "waiting missing next_step",
                "<freehand_completion>\n{\"claim\":\"waiting\"}\n</freehand_completion>",
                "next_step",
                "is required when `claim` is `waiting`",
            ),
        ];

        for (label, input, field, message) in cases {
            let err = parse_completion_submission_block(input).expect_err(label);
            assert!(
                err.issues
                    .iter()
                    .any(|issue| issue.field == field && issue.message.contains(message)),
                "{label} feedback mismatch: {err:?}"
            );

            let feedback = completion_schema_rejection_feedback(&err);
            assert!(
                feedback.contains(field),
                "{label} feedback missing field name: {feedback}"
            );
            assert!(
                feedback.contains(message),
                "{label} feedback missing guidance: {feedback}"
            );
        }
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

    fn news_plan() -> SearchDomainPlanDelivery {
        SearchDomainPlanDelivery {
            schema: SEARCH_DOMAIN_PLAN_SCHEMA.to_owned(),
            delivery_id: "domain-1".to_owned(),
            domain: SearchDomain::News,
            preferred_source_kinds: vec!["official_publication".to_owned()],
            social_platform_priority: vec![SearchSocialPlatform::Weibo],
            minimum_verified_sources: 1,
            policy_version: "2026-08-15".to_owned(),
        }
    }

    fn verified_source() -> SearchVerificationDelivery {
        SearchVerificationDelivery {
            schema: SEARCH_VERIFICATION_SCHEMA.to_owned(),
            delivery_id: "verify-c1".to_owned(),
            source_id: "c1".to_owned(),
            original_url: "https://example.com/news".to_owned(),
            camo_profile: "news".to_owned(),
            accessed_at: "2026-08-15T12:00:00Z".to_owned(),
            access_status: SearchAccessStatus::Verified,
            page_title: Some("News".to_owned()),
            evidence_excerpt: Some("Page evidence".to_owned()),
            verified_by: Some("camo".to_owned()),
            access_attempts: vec![freehand_contracts::SearchAccessAttempt {
                attempt_id: "attempt-1".to_owned(),
                channel: "camo".to_owned(),
                status: SearchAccessStatus::Verified,
                accessed_at: "2026-08-15T12:00:00Z".to_owned(),
                error: None,
            }],
            error: None,
        }
    }

    fn hosted_discovery() -> SearchDiscoveryDelivery {
        SearchDiscoveryDelivery {
            schema: SEARCH_DISCOVERY_SCHEMA.to_owned(),
            delivery_id: "hosted-1".to_owned(),
            discovery_channel: SearchDiscoveryChannel::HostedWebSearch,
            domain_plan_ref: Some("domain-1".to_owned()),
            hosted_search_attempt: Some(freehand_contracts::SearchHostedAttempt {
                tool_call_id: None,
                status: None,
                result_count: None,
                query: "news".to_owned(),
                provider: "openai_responses".to_owned(),
            }),
            candidates: vec![freehand_contracts::SearchDiscoveryCandidate {
                candidate_id: "c1".to_owned(),
                status: SearchCandidateStatus::Usable,
                original_url: Some("https://example.com/news".to_owned()),
                title: "News".to_owned(),
                snippet: "search snippet".to_owned(),
                discovered_by: Some(SearchDiscoveryChannel::HostedWebSearch),
                platform: Some(SearchSocialPlatform::Web),
                source_weight: Some(90),
                reason: None,
            }],
        }
    }

    fn no_supplement() -> SocialSupplementDecisionDelivery {
        SocialSupplementDecisionDelivery {
            schema: SEARCH_SUPPLEMENT_SCHEMA.to_owned(),
            delivery_id: "supplement-1".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            required: false,
            reasons: Vec::new(),
            platforms: Vec::new(),
        }
    }

    #[test]
    fn search_domain_policy_enforces_social_platform_priority() {
        validate_search_domain_plan_delivery(&news_plan()).expect("valid news plan");
        let mut invalid_news = news_plan();
        invalid_news.social_platform_priority = vec![SearchSocialPlatform::Xhs];
        assert!(validate_search_domain_plan_delivery(&invalid_news).is_err());

        let mut tutorial = news_plan();
        tutorial.domain = SearchDomain::Tutorial;
        tutorial.social_platform_priority = vec![SearchSocialPlatform::Xhs];
        validate_search_domain_plan_delivery(&tutorial).expect("valid tutorial plan");
        tutorial.social_platform_priority = vec![SearchSocialPlatform::Weibo];
        assert!(validate_search_domain_plan_delivery(&tutorial).is_err());
    }

    #[test]
    fn search_delivery_parser_is_strict_and_schema_tagged() {
        let json = serde_json::to_string(&news_plan()).expect("serialize plan");
        let parsed = parse_search_evidence_delivery_block(&format!(
            "<freehand_search_delivery>{json}</freehand_search_delivery>"
        ))
        .expect("strict tagged delivery");
        assert!(matches!(parsed, SearchEvidenceDelivery::DomainPlan(_)));

        let malformed = parse_search_evidence_delivery_block(
            "<freehand_search_delivery>{\"schema\":\"search_evidence.domain_plan.v1\",}</freehand_search_delivery>",
        )
        .expect_err("trailing comma must not be repaired");
        assert!(malformed.message.contains("invalid JSON"));

        let unknown = parse_search_evidence_delivery_block(
            "<freehand_search_delivery>{\"schema\":\"search_evidence.domain_plan.v1\",\"delivery_id\":\"domain-1\",\"domain\":\"news\",\"preferred_source_kinds\":[\"official\"],\"social_platform_priority\":[\"weibo\"],\"minimum_verified_sources\":1,\"policy_version\":\"2026-08-15\",\"unexpected\":true}</freehand_search_delivery>",
        )
        .expect_err("unknown field must fail closed");
        assert_eq!(unknown.field, "unexpected");
        assert!(unknown.message.contains("unknown field"));

        let nested_type = parse_search_evidence_delivery_block(
            "<freehand_search_delivery>{\"schema\":\"search_evidence.discovery.v1\",\"delivery_id\":\"discovery-1\",\"discovery_channel\":\"hosted_web_search\",\"domain_plan_ref\":\"domain-1\",\"hosted_search_attempt\":{\"query\":\"news\",\"provider\":\"openai_responses\"},\"candidates\":[{\"candidate_id\":\"c1\",\"status\":\"usable\",\"original_url\":7,\"title\":\"Title\",\"snippet\":\"Snippet\",\"discovered_by\":\"hosted_web_search\",\"platform\":\"web\",\"source_weight\":90,\"reason\":null}]}</freehand_search_delivery>",
        )
        .expect_err("nested wrong type must report its path");
        assert_eq!(nested_type.field, "candidates[0].original_url");
        assert!(nested_type.message.contains("invalid type"));

        assert!(
            parse_search_evidence_delivery_block(
                "<freehand_search_delivery>{}</freehand_search_delivery>"
            )
            .is_err()
        );
    }

    #[test]
    fn search_evidence_decode_rejection_reports_missing_and_nested_fields() {
        let missing = parse_search_evidence_delivery_block(
            "<freehand_search_delivery>{\"schema\":\"search_evidence.domain_plan.v1\",\"delivery_id\":\"domain-1\",\"domain\":\"news\",\"preferred_source_kinds\":[\"official\"],\"social_platform_priority\":[\"weibo\"],\"minimum_verified_sources\":1}</freehand_search_delivery>",
        )
        .expect_err("missing field must fail");
        assert_eq!(
            missing.category,
            SearchEvidenceSchemaRejectionCategory::Decode
        );
        assert_eq!(missing.field, "policy_version");

        let nested = parse_search_evidence_delivery_block(
            "<freehand_search_delivery>{\"schema\":\"search_evidence.discovery.v1\",\"delivery_id\":\"discovery-1\",\"discovery_channel\":\"hosted_web_search\",\"hosted_search_attempt\":{\"query\":\"news\",\"provider\":\"openai_responses\"},\"candidates\":[{\"candidate_id\":\"c1\",\"status\":\"usable\",\"original_url\":7,\"title\":\"Title\",\"snippet\":\"Snippet\",\"discovered_by\":\"hosted_web_search\"}]}</freehand_search_delivery>",
        )
        .expect_err("nested wrong type must fail");
        assert_eq!(
            nested.category,
            SearchEvidenceSchemaRejectionCategory::Decode
        );
        assert_eq!(nested.field, "candidates[0].original_url");
    }

    #[test]
    fn search_evidence_model_stage_accepts_only_matching_delivery() {
        let plan = SearchEvidenceDelivery::DomainPlan(news_plan());
        validate_search_evidence_model_stage(SearchEvidenceModelStage::DomainPlan, &plan)
            .expect("matching stage");
        let rejection =
            validate_search_evidence_model_stage(SearchEvidenceModelStage::FinalDelivery, &plan)
                .expect_err("wrong stage must fail");
        assert_eq!(
            rejection.category,
            SearchEvidenceSchemaRejectionCategory::StageMismatch
        );
        assert_eq!(rejection.field, "delivery_type");
    }

    #[test]
    fn search_evidence_url_requires_non_empty_authority() {
        let mut source = verified_source();
        source.original_url = "https://".to_owned();
        let rejection = validate_search_verification_delivery(&source)
            .expect_err("empty URL authority must fail");
        assert_eq!(
            rejection,
            SearchEvidenceValidationError::InvalidField {
                field: "original_url".to_owned(),
                reason: "must be an http or https URL".to_owned(),
            }
        );
    }

    #[test]
    fn search_web_fetch_discovery_still_requires_camo_verification() {
        let discovery = SearchDiscoveryDelivery {
            schema: SEARCH_DISCOVERY_SCHEMA.to_owned(),
            delivery_id: "discovery-web-fetch-1".to_owned(),
            discovery_channel: SearchDiscoveryChannel::WebFetch,
            domain_plan_ref: Some("domain-1".to_owned()),
            hosted_search_attempt: None,
            candidates: vec![freehand_contracts::SearchDiscoveryCandidate {
                candidate_id: "web-fetch-1".to_owned(),
                status: SearchCandidateStatus::Usable,
                original_url: Some("https://example.com/news".to_owned()),
                title: "News".to_owned(),
                snippet: "Fetched page evidence".to_owned(),
                discovered_by: Some(SearchDiscoveryChannel::WebFetch),
                platform: None,
                source_weight: Some(50),
                reason: None,
            }],
        };

        assert_eq!(
            project_search_evidence_stage_status(
                &[SearchEvidenceDelivery::DomainPlan(news_plan())],
                &SearchEvidenceDelivery::Discovery(discovery),
            ),
            Ok(SearchEvidenceTurnStatus::CamoVerificationRequired)
        );
    }

    #[test]
    fn search_verification_rejects_non_camo_or_empty_evidence() {
        validate_search_verification_delivery(&verified_source()).expect("valid source");
        let mut invalid = verified_source();
        invalid.verified_by = Some("web_fetch".to_owned());
        assert!(validate_search_verification_delivery(&invalid).is_err());
        invalid.verified_by = Some("camo".to_owned());
        invalid.evidence_excerpt = Some(" ".to_owned());
        assert!(validate_search_verification_delivery(&invalid).is_err());
    }

    #[test]
    fn non_sourced_search_evidence_accepts_multiple_contiguous_hosted_deliveries() {
        let mut first = hosted_discovery();
        first.domain_plan_ref = None;
        let mut second = hosted_discovery();
        second.domain_plan_ref = None;
        second.delivery_id = "hosted-2".to_owned();
        second.candidates[0].candidate_id = "c2".to_owned();
        second.candidates[0].original_url = Some("https://example.com/news-2".to_owned());

        validate_search_evidence_stage_append(
            &[SearchEvidenceDelivery::Discovery(first.clone())],
            &SearchEvidenceDelivery::Discovery(second.clone()),
        )
        .expect("non-sourced provider response may preserve more than one hosted attempt");
        assert_eq!(first.domain_plan_ref, None);
        assert_eq!(second.delivery_id, "hosted-2");
        assert_eq!(second.candidates[0].candidate_id, "c2");
    }

    #[test]
    fn search_state_accepts_adjacent_path_and_rejects_hosted_to_final() {
        validate_search_evidence_transition(
            SearchEvidenceTurnStatus::DomainPlanValidated,
            SearchEvidenceTurnStatus::HostedDiscoveryValidated,
            false,
        )
        .expect("domain to hosted");
        validate_search_evidence_transition(
            SearchEvidenceTurnStatus::HostedDiscoveryValidated,
            SearchEvidenceTurnStatus::CamoVerificationRequired,
            false,
        )
        .expect("hosted to camo required");
        validate_search_evidence_transition(
            SearchEvidenceTurnStatus::HostedDiscoveryValidated,
            SearchEvidenceTurnStatus::SupplementDecisionValidated,
            false,
        )
        .expect("hosted without usable candidates may decide supplement");
        validate_search_evidence_transition(
            SearchEvidenceTurnStatus::FinalValidated,
            SearchEvidenceTurnStatus::TurnTerminalSuccess,
            false,
        )
        .expect("validated final terminalizes success");
        assert!(
            validate_search_evidence_transition(
                SearchEvidenceTurnStatus::HostedDiscoveryValidated,
                SearchEvidenceTurnStatus::FinalValidated,
                false,
            )
            .is_err()
        );
        assert!(
            validate_search_evidence_transition(
                SearchEvidenceTurnStatus::SupplementDecisionValidated,
                SearchEvidenceTurnStatus::FinalValidated,
                true,
            )
            .is_err()
        );
        validate_search_evidence_transition(
            SearchEvidenceTurnStatus::SupplementDecisionValidated,
            SearchEvidenceTurnStatus::Blocked,
            true,
        )
        .expect("unsupported required supplement may block explicitly");
    }

    #[test]
    fn search_final_accepts_verified_source_and_rejects_unknown_source() {
        let final_delivery = SearchFinalDelivery {
            schema: SEARCH_FINAL_SCHEMA.to_owned(),
            delivery_id: "final-1".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            claim: SearchFinalClaimStatus::Complete,
            summary: Some("Summary".to_owned()),
            claims: vec![freehand_contracts::SearchClaimDelivery {
                claim_id: "claim-1".to_owned(),
                text: "Claim".to_owned(),
                source_ids: vec!["c1".to_owned()],
            }],
            unconfirmed: Vec::new(),
            blocked_reason: None,
        };
        validate_search_final_delivery(&news_plan(), &[verified_source()], &final_delivery)
            .expect("valid final");

        let mut unknown = final_delivery;
        unknown.claims[0].source_ids = vec!["missing".to_owned()];
        assert_eq!(
            validate_search_final_delivery(&news_plan(), &[verified_source()], &unknown),
            Err(SearchEvidenceValidationError::InvalidSourceReference(
                "missing".to_owned()
            ))
        );
    }

    #[test]
    fn search_turn_delivery_builds_success_and_blocks_unverified_complete() {
        let source = verified_source();
        let final_delivery = SearchFinalDelivery {
            schema: SEARCH_FINAL_SCHEMA.to_owned(),
            delivery_id: "final-1".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            claim: SearchFinalClaimStatus::Complete,
            summary: Some("Summary".to_owned()),
            claims: vec![freehand_contracts::SearchClaimDelivery {
                claim_id: "claim-1".to_owned(),
                text: "Claim".to_owned(),
                source_ids: vec!["c1".to_owned()],
            }],
            unconfirmed: Vec::new(),
            blocked_reason: None,
        };
        let turn = build_search_evidence_turn_delivery(
            SessionId::new("session-1"),
            TurnId::new("turn-1"),
            vec![
                SearchEvidenceDelivery::DomainPlan(news_plan()),
                SearchEvidenceDelivery::Discovery(hosted_discovery()),
                SearchEvidenceDelivery::Verification(source),
                SearchEvidenceDelivery::SupplementDecision(no_supplement()),
            ],
            final_delivery.clone(),
        )
        .expect("valid turn delivery");
        assert!(turn.summary_ready);
        assert_eq!(turn.status, SearchEvidenceTurnStatus::FinalValidated);
        assert_eq!(turn.terminal, None);

        assert!(
            build_search_evidence_turn_delivery(
                SessionId::new("session-1"),
                TurnId::new("turn-2"),
                vec![
                    SearchEvidenceDelivery::DomainPlan(news_plan()),
                    SearchEvidenceDelivery::Discovery(hosted_discovery()),
                    SearchEvidenceDelivery::SupplementDecision(no_supplement()),
                ],
                final_delivery,
            )
            .is_err()
        );
    }

    #[test]
    fn search_turn_delivery_allows_explicit_no_source_block() {
        let mut discovery = hosted_discovery();
        discovery.candidates[0].status = SearchCandidateStatus::UnusableMissingUrl;
        discovery.candidates[0].original_url = None;
        discovery.candidates[0].discovered_by = None;
        discovery.candidates[0].reason =
            Some("hosted_search_did_not_return_original_url".to_owned());
        let supplement = SocialSupplementDecisionDelivery {
            schema: SEARCH_SUPPLEMENT_SCHEMA.to_owned(),
            delivery_id: "supplement-1".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            required: true,
            reasons: vec![freehand_contracts::SearchSupplementReason::MissingOriginalUrls],
            platforms: vec![SearchSocialPlatform::Weibo],
        };
        let final_delivery = SearchFinalDelivery {
            schema: SEARCH_FINAL_SCHEMA.to_owned(),
            delivery_id: "final-blocked".to_owned(),
            domain_plan_ref: "domain-1".to_owned(),
            claim: SearchFinalClaimStatus::Blocked,
            summary: None,
            claims: Vec::new(),
            unconfirmed: Vec::new(),
            blocked_reason: Some("required_social_platform_unsupported".to_owned()),
        };

        let turn = build_search_evidence_turn_delivery(
            SessionId::new("session-1"),
            TurnId::new("turn-blocked"),
            vec![
                SearchEvidenceDelivery::DomainPlan(news_plan()),
                SearchEvidenceDelivery::Discovery(discovery),
                SearchEvidenceDelivery::SupplementDecision(supplement),
            ],
            final_delivery,
        )
        .expect("explicit blocked final");

        assert_eq!(turn.status, SearchEvidenceTurnStatus::Blocked);
        assert_eq!(turn.terminal, Some(SearchEvidenceTerminal::Blocked));
        assert!(!turn.summary_ready);
    }
}
