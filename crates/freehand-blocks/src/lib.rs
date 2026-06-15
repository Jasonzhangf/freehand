//! Shared pure builders, parsers, validators, and projectors for Freehand.

use freehand_contracts::{TerminalStatus, ToolArgument};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompletionClaim {
    Complete,
    Continue,
    Blocked,
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolArgumentsJsonError {
    #[error("tool arguments json parse failed: {0}")]
    InvalidJson(String),
    #[error("tool arguments json must be an object at the top level")]
    TopLevelMustBeObject,
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
}
