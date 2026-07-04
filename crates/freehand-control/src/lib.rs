//! Passive control-center semantics for Freehand status blocks.
//!
//! This crate parses and validates hidden model status feedback. It does not
//! execute task mutations, write turn truth, or render UI.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const STATUS_START_TAG: &str = "<<<freehand_status>>>";
const STATUS_END_TAG: &str = "<</freehand_status>>>";
const STATUS_END_TAG_SYMMETRIC: &str = "<<</freehand_status>>>";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStatusSubmission {
    pub schema_version: u32,
    pub status: ControlInteractionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ControlInteractionStatus {
    pub kind: Option<String>,
    pub reason: Option<String>,
    pub target_cwd: Option<String>,
    pub next_expected_tool: Option<String>,
    pub simple_request: Option<bool>,
    pub task_complete: Option<bool>,
    pub evidence: Option<String>,
    pub summary: Option<String>,
    pub learned: Option<String>,
    pub needs_record: Option<bool>,
    pub next_step: Option<String>,
    pub blocked: Option<bool>,
    pub blocked_reason: Option<String>,
    pub needs_user_involvement: Option<bool>,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStatusIssue {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStatusRejection {
    pub issues: Vec<ControlStatusIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlRhythmDecision {
    AllowNaturalStop,
    AllowTaskCompletion,
    ContinueWithNextStep(String),
    StopBlocked(String),
    StopForUserOptions(Vec<String>),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ControlStatusError {
    #[error("freehand status block missing")]
    MissingBlock,
    #[error("freehand status block is not closed")]
    UnclosedBlock,
    #[error("freehand status JSON is invalid: {0}")]
    InvalidJson(String),
    #[error("freehand status JSON must be an object")]
    TopLevelMustBeObject,
}

pub fn parse_control_status_block(
    text: &str,
) -> Result<ControlStatusSubmission, ControlStatusRejection> {
    let raw = extract_control_status_json(text).map_err(status_block_error_rejection)?;
    let value: Value = serde_json::from_str(raw.trim()).map_err(|err| {
        status_rejection([ControlStatusIssue {
            field: "freehand_status".to_owned(),
            message: format!("invalid JSON: {err}"),
        }])
    })?;
    let Some(object) = value.as_object() else {
        return Err(status_rejection([ControlStatusIssue {
            field: "freehand_status".to_owned(),
            message: "tagged JSON must be an object".to_owned(),
        }]));
    };

    let mut issues = Vec::new();
    let schema_version = match object.get("schema_version").and_then(Value::as_u64) {
        Some(1) => 1,
        Some(_) => {
            issues.push(ControlStatusIssue {
                field: "schema_version".to_owned(),
                message: "must be 1".to_owned(),
            });
            0
        }
        None => {
            issues.push(ControlStatusIssue {
                field: "schema_version".to_owned(),
                message: "is required".to_owned(),
            });
            0
        }
    };

    let status_value = object.get("status");
    let Some(status_object) = status_value.and_then(Value::as_object) else {
        issues.push(ControlStatusIssue {
            field: "status".to_owned(),
            message: "is required and must be an object".to_owned(),
        });
        return Err(ControlStatusRejection { issues });
    };

    let status = ControlInteractionStatus {
        kind: optional_string(&mut issues, status_object, "kind"),
        reason: optional_string(&mut issues, status_object, "reason"),
        target_cwd: optional_string(&mut issues, status_object, "target_cwd"),
        next_expected_tool: optional_string(&mut issues, status_object, "next_expected_tool"),
        simple_request: optional_bool(&mut issues, status_object, "simple_request"),
        task_complete: optional_bool(&mut issues, status_object, "task_complete"),
        evidence: optional_string(&mut issues, status_object, "evidence"),
        summary: optional_string(&mut issues, status_object, "summary"),
        learned: optional_string(&mut issues, status_object, "learned"),
        needs_record: optional_bool(&mut issues, status_object, "needs_record"),
        next_step: optional_string(&mut issues, status_object, "next_step"),
        blocked: optional_bool(&mut issues, status_object, "blocked"),
        blocked_reason: optional_string(&mut issues, status_object, "blocked_reason"),
        needs_user_involvement: optional_bool(&mut issues, status_object, "needs_user_involvement"),
        options: optional_string_array(&mut issues, status_object, "options"),
    };

    if !issues.is_empty() {
        return Err(ControlStatusRejection { issues });
    }
    let submission = ControlStatusSubmission {
        schema_version,
        status,
    };
    validate_control_status_submission(&submission).map(|_| submission)
}

pub fn control_status_rhythm_decision(
    submission: &ControlStatusSubmission,
) -> Result<ControlRhythmDecision, ControlStatusRejection> {
    validate_control_status_submission(submission)
}

pub fn strip_control_status_block(text: &str) -> String {
    let Some(start) = text.find(STATUS_START_TAG) else {
        return text.trim().to_owned();
    };
    let content_start = start + STATUS_START_TAG.len();
    let Some((end_start, end_len)) = find_status_end_tag(text, content_start) else {
        return text.trim().to_owned();
    };
    let before = text[..start].trim();
    let after = text[end_start + end_len..].trim();
    match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (false, true) => before.to_owned(),
        (true, false) => after.to_owned(),
        (false, false) => format!("{before}\n{after}"),
    }
}

fn extract_control_status_json(text: &str) -> Result<&str, ControlStatusError> {
    let Some(start) = text.find(STATUS_START_TAG) else {
        return Err(ControlStatusError::MissingBlock);
    };
    let content_start = start + STATUS_START_TAG.len();
    let Some((end_start, _end_len)) = find_status_end_tag(text, content_start) else {
        return Err(ControlStatusError::UnclosedBlock);
    };
    Ok(&text[content_start..end_start])
}

fn find_status_end_tag(text: &str, search_start: usize) -> Option<(usize, usize)> {
    let documented = text[search_start..]
        .find(STATUS_END_TAG)
        .map(|offset| (search_start + offset, STATUS_END_TAG.len()));
    let symmetric = text[search_start..]
        .find(STATUS_END_TAG_SYMMETRIC)
        .map(|offset| (search_start + offset, STATUS_END_TAG_SYMMETRIC.len()));
    match (documented, symmetric) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn validate_control_status_submission(
    submission: &ControlStatusSubmission,
) -> Result<ControlRhythmDecision, ControlStatusRejection> {
    let status = &submission.status;
    if submission.schema_version != 1 {
        return Err(status_rejection([ControlStatusIssue {
            field: "schema_version".to_owned(),
            message: "must be 1".to_owned(),
        }]));
    }
    if status.simple_request == Some(true) {
        return Ok(ControlRhythmDecision::AllowNaturalStop);
    }
    if status.needs_user_involvement == Some(true) {
        if status.options.iter().any(|item| !item.trim().is_empty()) {
            return Ok(ControlRhythmDecision::StopForUserOptions(
                status.options.clone(),
            ));
        }
        return Err(status_rejection([ControlStatusIssue {
            field: "options".to_owned(),
            message: "is required when `needs_user_involvement` is true".to_owned(),
        }]));
    }
    if status.task_complete == Some(true) {
        if required_text(status.evidence.as_deref()) {
            return Ok(ControlRhythmDecision::AllowTaskCompletion);
        }
        return Err(status_rejection([ControlStatusIssue {
            field: "evidence".to_owned(),
            message: "is required when `task_complete` is true".to_owned(),
        }]));
    }
    if status.blocked == Some(true) {
        if let Some(blocked_reason) = status
            .blocked_reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(ControlRhythmDecision::StopBlocked(
                blocked_reason.to_owned(),
            ));
        }
        return Err(status_rejection([ControlStatusIssue {
            field: "blocked_reason".to_owned(),
            message: "is required when `blocked` is true".to_owned(),
        }]));
    }
    if let Some(next_step) = status
        .next_step
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(ControlRhythmDecision::ContinueWithNextStep(
            next_step.to_owned(),
        ));
    }
    Err(status_rejection([ControlStatusIssue {
        field: "status".to_owned(),
        message: "must set simple_request=true, task_complete=true with evidence, blocked=true with blocked_reason, needs_user_involvement=true with options, or next_step".to_owned(),
    }]))
}

fn optional_string(
    issues: &mut Vec<ControlStatusIssue>,
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Option<String> {
    match object.get(field) {
        None => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(value) => {
            issues.push(ControlStatusIssue {
                field: field.to_owned(),
                message: format!("must be a string, got {}", value_type_label(value)),
            });
            None
        }
    }
}

fn optional_bool(
    issues: &mut Vec<ControlStatusIssue>,
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Option<bool> {
    match object.get(field) {
        None => None,
        Some(Value::Bool(value)) => Some(*value),
        Some(value) => {
            issues.push(ControlStatusIssue {
                field: field.to_owned(),
                message: format!("must be a boolean, got {}", value_type_label(value)),
            });
            None
        }
    }
}

fn optional_string_array(
    issues: &mut Vec<ControlStatusIssue>,
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Vec<String> {
    match object.get(field) {
        None => Vec::new(),
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(|value| match value {
                Value::String(item) => Some(item.clone()),
                other => {
                    issues.push(ControlStatusIssue {
                        field: field.to_owned(),
                        message: format!(
                            "must contain only strings, got {}",
                            value_type_label(other)
                        ),
                    });
                    None
                }
            })
            .collect(),
        Some(value) => {
            issues.push(ControlStatusIssue {
                field: field.to_owned(),
                message: format!("must be an array, got {}", value_type_label(value)),
            });
            Vec::new()
        }
    }
}

fn required_text(value: Option<&str>) -> bool {
    value.map(str::trim).is_some_and(|value| !value.is_empty())
}

fn status_block_error_rejection(err: ControlStatusError) -> ControlStatusRejection {
    status_rejection([ControlStatusIssue {
        field: "freehand_status".to_owned(),
        message: err.to_string(),
    }])
}

fn status_rejection(
    issues: impl IntoIterator<Item = ControlStatusIssue>,
) -> ControlStatusRejection {
    ControlStatusRejection {
        issues: issues.into_iter().collect(),
    }
}

fn value_type_label(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_request_status_and_allows_stop() {
        let raw = r#"public
<<<freehand_status>>>
{"schema_version":1,"status":{"simple_request":true}}
<</freehand_status>>>"#;

        let submission = parse_control_status_block(raw).expect("status");

        assert_eq!(
            control_status_rhythm_decision(&submission),
            Ok(ControlRhythmDecision::AllowNaturalStop)
        );
        assert_eq!(strip_control_status_block(raw), "public");
    }

    #[test]
    fn rejects_task_complete_without_evidence() {
        let raw = r#"<<<freehand_status>>>
{"schema_version":1,"status":{"task_complete":true}}
<</freehand_status>>>"#;

        let rejection = parse_control_status_block(raw).expect_err("missing evidence");

        assert!(rejection.issues.iter().any(|issue| {
            issue.field == "evidence"
                && issue
                    .message
                    .contains("is required when `task_complete` is true")
        }));
    }

    #[test]
    fn continues_from_next_step_without_side_effect() {
        let raw = r#"<<<freehand_status>>>
{"schema_version":1,"status":{"task_complete":false,"next_step":"inspect owner map"}}
<</freehand_status>>>"#;

        let submission = parse_control_status_block(raw).expect("status");

        assert_eq!(
            control_status_rhythm_decision(&submission),
            Ok(ControlRhythmDecision::ContinueWithNextStep(
                "inspect owner map".to_owned()
            ))
        );
    }
}
