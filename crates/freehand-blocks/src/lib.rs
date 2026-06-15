//! Shared pure builders, parsers, validators, and projectors for Freehand.

use freehand_contracts::TerminalStatus;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
