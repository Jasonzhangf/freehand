use crate::adp_wire::UiProjection;
use crate::dto::*;
use freehand_blocks::{
    ToolDisplayOutcome, ToolDisplayProjection, parse_completion_submission_block,
    project_hosted_search_display, project_tool_call_display, project_tool_result_display,
    strip_completion_submission_block,
};
use freehand_contracts::{
    AgentId, ReasonReq04ToolCall, ReasonReq05ToolResultReentry, ReasonResp03TerminalEvent,
    SearchEvidenceDelivery, SessionId, TerminalStatus, ToolResultContract, ToolResultStatus,
    TurnId,
};
use freehand_control::{parse_control_status_block, strip_control_status_block};
use freehand_debug::DebugEvent;
use std::collections::BTreeMap;

pub fn terminal_text_projection(event: &ReasonResp03TerminalEvent) -> String {
    event.summary.clone()
}

pub fn human_friendly_terminal_text(
    text_chunks: &[String],
    event: &ReasonResp03TerminalEvent,
) -> String {
    if !is_raw_provider_stop_summary(&event.summary) {
        return event.summary.clone();
    }
    let raw_text = text_chunks.join("");
    if let Ok(submission) = parse_completion_submission_block(&raw_text)
        && let Some(summary) = submission
            .summary
            .as_deref()
            .or(submission.evidence.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        return summary.to_owned();
    }
    if let Ok(submission) = parse_control_status_block(&raw_text)
        && let Some(summary) = submission
            .status
            .summary
            .as_deref()
            .or(submission.status.evidence.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        return summary.to_owned();
    }
    let visible_text = strip_control_status_block(&strip_completion_submission_block(&raw_text));
    let visible_text = visible_text.trim();
    if !visible_text.is_empty() {
        return visible_text.to_owned();
    }
    event.summary.clone()
}

fn is_raw_provider_stop_summary(summary: &str) -> bool {
    matches!(
        summary.trim(),
        "stop"
            | "end_turn"
            | "tool_use"
            | "max_tokens"
            | "refusal"
            | "completed"
            | "complete"
            | "success"
    )
}

pub fn public_conversation_items(projection: &UiTurnProjection) -> Vec<UiConversationItem> {
    let mut items = Vec::new();
    if let Some(user_text) = &projection.user_text
        && !user_text.trim().is_empty()
    {
        items.push(UiConversationItem {
            kind: UiConversationItemKind::UserText,
            title: "User".to_owned(),
            body: user_text.clone(),
            status: "submitted".to_owned(),
            tool_call_id: None,
            display: None,
        });
    }
    for text in &projection.text {
        let public_text = strip_control_status_block(&strip_completion_submission_block(text));
        if !public_text.trim().is_empty() {
            items.push(UiConversationItem {
                kind: UiConversationItemKind::AssistantText,
                title: "Assistant".to_owned(),
                body: public_text,
                status: "streaming".to_owned(),
                tool_call_id: None,
                display: None,
            });
        }
    }
    for activity in &projection.tool_activities {
        let title = activity
            .display
            .as_ref()
            .map(|display| display.action.clone())
            .unwrap_or_else(|| activity.tool_name.clone());
        let body = tool_public_body(activity);
        items.push(UiConversationItem {
            kind: UiConversationItemKind::ToolSummary,
            title,
            body,
            status: activity.status.as_str().to_owned(),
            tool_call_id: Some(activity.tool_call_id.clone()),
            display: activity.display.clone(),
        });
    }
    if let Some(terminal_text) = &projection.terminal_text {
        let public_text =
            strip_control_status_block(&strip_completion_submission_block(terminal_text));
        if !public_text.trim().is_empty() {
            let status = match projection.terminal_status {
                Some(TerminalStatus::Cancelled) => "cancelled",
                Some(TerminalStatus::Failed) => "failed",
                Some(TerminalStatus::Blocked) => "blocked",
                Some(TerminalStatus::Interrupted) => "interrupted",
                Some(TerminalStatus::ToolPending) => "running",
                Some(TerminalStatus::AwaitingUserOptions) => "waiting_for_user_options",
                Some(TerminalStatus::Success) | None => "completed",
            };
            let title = if projection.terminal_status == Some(TerminalStatus::ToolPending) {
                "Lifecycle"
            } else {
                "Final"
            };
            items.push(UiConversationItem {
                kind: UiConversationItemKind::Terminal,
                title: title.to_owned(),
                body: public_text,
                status: status.to_owned(),
                tool_call_id: None,
                display: None,
            });
        }
    }
    for error in &projection.errors {
        items.push(UiConversationItem {
            kind: UiConversationItemKind::Error,
            title: "Error".to_owned(),
            body: error.clone(),
            status: "failed".to_owned(),
            tool_call_id: None,
            display: None,
        });
    }
    items
}

pub(crate) fn tool_public_body(activity: &UiToolActivity) -> String {
    let semantic_body = activity.display.as_ref().and_then(tool_display_public_body);
    let detail = match activity.status {
        UiToolActivityStatus::Waiting if semantic_body.is_some() => None,
        UiToolActivityStatus::Waiting => activity.detail.as_deref(),
        UiToolActivityStatus::Completed | UiToolActivityStatus::Failed => {
            activity.detail.as_deref()
        }
    };
    tool_public_body_with_detail(
        semantic_body,
        detail,
        tool_status_fallback_body(activity.status),
    )
}

pub(crate) fn tool_public_body_with_detail(
    semantic_body: Option<String>,
    detail: Option<&str>,
    fallback: &str,
) -> String {
    let mut lines = Vec::new();
    if let Some(body) = semantic_body {
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_owned());
        }
    }
    if let Some(detail) = detail {
        let trimmed = detail.trim();
        if !trimmed.is_empty() && lines.iter().all(|line| line != trimmed) {
            lines.push(trimmed.to_owned());
        }
    }
    if lines.is_empty() {
        fallback.to_owned()
    } else {
        lines.join("\n")
    }
}

pub(crate) fn tool_status_fallback_body(status: UiToolActivityStatus) -> &'static str {
    match status {
        UiToolActivityStatus::Waiting => "waiting",
        UiToolActivityStatus::Completed => "completed",
        UiToolActivityStatus::Failed => "failed",
    }
}

pub(crate) fn tool_display_public_body(display: &ToolDisplayProjection) -> Option<String> {
    if let Some(diff) = &display.diff {
        return Some(format!(
            "diff: {}\n- {}\n+ {}",
            diff.target, diff.before, diff.after
        ));
    }
    if let Some(parameter_summary) = &display.parameter_summary
        && !parameter_summary.trim().is_empty()
    {
        return Some(parameter_summary.clone());
    }
    if !display.summary.trim().is_empty() {
        return Some(display.summary.clone());
    }
    if !display.fields.is_empty() {
        let compact_fields = display
            .fields
            .iter()
            .take(4)
            .map(|field| format!("{}: {}", field.label, field.value))
            .collect::<Vec<_>>()
            .join(" · ");
        if !compact_fields.trim().is_empty() {
            return Some(compact_fields);
        }
    }
    None
}

pub fn public_turn_projection(projection: UiTurnProjection) -> UiPublicTurnProjection {
    let public_conversation = public_conversation_items(&projection);
    UiPublicTurnProjection {
        turn: projection,
        public_conversation,
    }
}

pub fn checkpoint_projection_from_runtime_summary(
    source_agent_id: AgentId,
    source_node_id: String,
    summaries: Vec<UiCheckpointSummary>,
) -> UiCheckpointSnapshot {
    UiCheckpointSnapshot {
        source: UiSource {
            source_agent_id,
            source_node_id,
            source_turn_id: None,
            stream_kind: UiStreamKind::Checkpoint,
        },
        checkpoints: summaries,
    }
}

pub fn debug_projection_from_event(event: &DebugEvent) -> Option<UiProjection> {
    event.snapshot.clone().map(UiProjection::Debug)
}

pub(crate) fn empty_checkpoint_snapshot() -> UiCheckpointSnapshot {
    UiCheckpointSnapshot {
        source: UiSource {
            source_agent_id: AgentId::new("unknown"),
            source_node_id: "unknown".to_owned(),
            source_turn_id: None,
            stream_kind: UiStreamKind::Checkpoint,
        },
        checkpoints: Vec::new(),
    }
}

pub(crate) fn session_transcript_projection(
    session_id: &SessionId,
    turns: &BTreeMap<TurnId, UiTurnProjection>,
    session_cwds: &BTreeMap<SessionId, String>,
    session_metadata: &BTreeMap<SessionId, UiSessionMetadataProjection>,
) -> UiSessionTranscriptProjection {
    let metadata = session_metadata.get(session_id);
    let cwd = session_cwds
        .get(session_id)
        .cloned()
        .or_else(|| metadata.and_then(|metadata| metadata.cwd.clone()));
    let mut session_turns = turns
        .values()
        .filter(|turn| &turn.session_id == session_id)
        .cloned()
        .collect::<Vec<_>>();
    for turn in &mut session_turns {
        if turn.cwd.is_none() {
            turn.cwd = cwd.clone();
        }
    }
    session_turns
        .sort_by(|left, right| turn_order_key(&left.turn_id).cmp(&turn_order_key(&right.turn_id)));
    UiSessionTranscriptProjection {
        session_id: session_id.clone(),
        title: metadata.and_then(|metadata| metadata.title.clone()),
        archived: metadata.is_some_and(|metadata| metadata.archived),
        cwd,
        turns: session_turns,
    }
}

pub(crate) fn turn_is_nonterminal(turn: &UiTurnProjection) -> bool {
    turn.terminal_status.is_none() && turn.terminal_text.is_none()
}

pub(crate) fn turn_order_key(turn_id: &TurnId) -> (String, u64, u64, String) {
    let raw = turn_id.as_str();
    if let Some(rest) = raw.strip_prefix("runtime-turn-") {
        let (ordinal_part, round_part) = match rest.split_once("-r") {
            Some((ordinal, round)) => (ordinal, round),
            None => (rest, "1"),
        };
        let ordinal = ordinal_part.parse::<u64>().unwrap_or(u64::MAX);
        let round = round_part.parse::<u64>().unwrap_or(1);
        return ("runtime-turn-".to_owned(), ordinal, round, raw.to_owned());
    }
    let split_at = raw
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_ascii_digit())
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let (prefix, digits) = raw.split_at(split_at);
    let ordinal = if digits.is_empty() {
        0
    } else {
        digits.parse::<u64>().unwrap_or(u64::MAX)
    };
    (prefix.to_owned(), ordinal, 1, raw.to_owned())
}

pub(crate) fn tool_activities_from_input(
    tool_calls: &[ReasonReq04ToolCall],
    tool_results: &[ReasonReq05ToolResultReentry],
) -> Vec<UiToolActivity> {
    let mut activities = Vec::new();
    for call in tool_calls {
        upsert_tool_activity(
            &mut activities,
            call.tool_call.tool_call_id.as_str().to_owned(),
            call.tool_call.tool_name.clone(),
            UiToolActivityStatus::Waiting,
            Some("waiting for tool execution".to_owned()),
            Some(project_tool_call_display(
                &call.tool_call.tool_name,
                &call.tool_call.arguments,
            )),
        );
    }
    for result in tool_results {
        let tool_call_id = result.tool_result.tool_call_id.as_str().to_owned();
        let tool_name = tool_calls
            .iter()
            .find(|call| call.tool_call.tool_call_id.as_str() == tool_call_id)
            .map(|call| call.tool_call.tool_name.clone())
            .unwrap_or_else(|| "tool".to_owned());
        let display = activities
            .iter()
            .find(|activity| activity.tool_call_id == tool_call_id)
            .and_then(|activity| activity.display.clone())
            .map(|display| project_tool_result_display(display, &result.tool_result));
        upsert_tool_activity(
            &mut activities,
            tool_call_id,
            tool_name,
            tool_activity_status_from_result(result.tool_result.status),
            Some(tool_activity_detail_from_result(&result.tool_result)),
            display,
        );
    }
    activities
}

pub(crate) fn upsert_tool_activity(
    activities: &mut Vec<UiToolActivity>,
    tool_call_id: String,
    tool_name: String,
    status: UiToolActivityStatus,
    detail: Option<String>,
    display: Option<ToolDisplayProjection>,
) {
    if let Some(activity) = activities
        .iter_mut()
        .find(|activity| activity.tool_call_id == tool_call_id)
    {
        activity.tool_name = tool_name;
        activity.status = match (activity.status, status) {
            (UiToolActivityStatus::Completed, UiToolActivityStatus::Waiting) => {
                UiToolActivityStatus::Completed
            }
            (UiToolActivityStatus::Completed, UiToolActivityStatus::Failed) => {
                UiToolActivityStatus::Completed
            }
            _ => status,
        };
        activity.detail = detail;
        if display.is_some() {
            activity.display = display;
        }
        return;
    }

    activities.push(UiToolActivity {
        tool_call_id,
        tool_name,
        status,
        detail,
        display,
    });
}

/// Project provider-hosted web-search observations into tool activities using the
/// `freehand-blocks::tool_display` owner for display semantics. Consumes only the
/// typed `SearchEvidenceDelivery` side channel, never provider reasoning text.
pub fn merge_hosted_search_activities(
    activities: &mut Vec<UiToolActivity>,
    search_evidence: Option<&UiSearchEvidenceProjection>,
) {
    let Some(search_evidence) = search_evidence else {
        return;
    };
    for delivery in &search_evidence.deliveries {
        let SearchEvidenceDelivery::Discovery(discovery) = delivery else {
            continue;
        };
        let Some(attempt) = discovery.hosted_search_attempt.as_ref() else {
            continue;
        };
        let display = project_hosted_search_display(discovery);
        let tool_call_id = attempt
            .tool_call_id
            .clone()
            .unwrap_or_else(|| discovery.delivery_id.clone());
        let status = tool_activity_status_from_outcome(display.outcome);
        upsert_tool_activity(
            activities,
            tool_call_id,
            "web_search".to_owned(),
            status,
            Some(attempt.query.clone()),
            Some(display),
        );
    }
}

pub(crate) fn tool_activity_status_from_outcome(
    outcome: ToolDisplayOutcome,
) -> UiToolActivityStatus {
    match outcome {
        ToolDisplayOutcome::Waiting => UiToolActivityStatus::Waiting,
        ToolDisplayOutcome::Success => UiToolActivityStatus::Completed,
        ToolDisplayOutcome::Failed => UiToolActivityStatus::Failed,
    }
}

pub(crate) fn preserve_live_activity_on_nonterminal_refresh(
    replacement: &mut UiTurnProjection,
    previous: &UiTurnProjection,
) {
    if replacement.session_id != previous.session_id || replacement.turn_id != previous.turn_id {
        return;
    }
    if replacement.terminal_status.is_some() || replacement.terminal_text.is_some() {
        return;
    }
    if previous.terminal_status.is_some() || previous.terminal_text.is_some() {
        return;
    }
    if replacement.model_request.is_none() {
        replacement.model_request = previous.model_request.clone();
    }
    for previous_activity in &previous.tool_activities {
        merge_tool_activity(&mut replacement.tool_activities, previous_activity);
    }
}

pub(crate) fn merge_tool_activity(activities: &mut Vec<UiToolActivity>, incoming: &UiToolActivity) {
    if let Some(activity) = activities
        .iter_mut()
        .find(|activity| activity.tool_call_id == incoming.tool_call_id)
    {
        if tool_activity_rank(incoming.status) > tool_activity_rank(activity.status) {
            activity.status = incoming.status;
        }
        if activity.tool_name == "tool" && incoming.tool_name != "tool" {
            activity.tool_name = incoming.tool_name.clone();
        }
        if activity.detail.is_none() || tool_activity_rank(incoming.status) > 0 {
            activity.detail = incoming.detail.clone().or_else(|| activity.detail.clone());
        }
        if activity.display.is_none() || tool_activity_rank(incoming.status) > 0 {
            activity.display = incoming
                .display
                .clone()
                .or_else(|| activity.display.clone());
        }
        return;
    }
    activities.push(incoming.clone());
}

pub(crate) fn tool_activity_rank(status: UiToolActivityStatus) -> u8 {
    match status {
        UiToolActivityStatus::Waiting => 0,
        UiToolActivityStatus::Completed => 1,
        UiToolActivityStatus::Failed => 2,
    }
}

pub(crate) fn tool_activity_status_from_result(status: ToolResultStatus) -> UiToolActivityStatus {
    match status {
        ToolResultStatus::Success => UiToolActivityStatus::Completed,
        ToolResultStatus::Failed => UiToolActivityStatus::Failed,
    }
}

pub(crate) fn tool_activity_detail_from_result(result: &ToolResultContract) -> String {
    let prefix = match result.status {
        ToolResultStatus::Success => "result",
        ToolResultStatus::Failed => "failure",
    };
    if result.output.trim().is_empty() {
        return match result.status {
            ToolResultStatus::Success => "result: <empty>".to_owned(),
            ToolResultStatus::Failed => "failure: <empty>".to_owned(),
        };
    }
    format!("{prefix}: {}", result.output)
}

pub(crate) fn fail_waiting_tool_activities(
    activities: &mut [UiToolActivity],
    detail: Option<String>,
) {
    for activity in activities {
        if activity.status == UiToolActivityStatus::Waiting {
            activity.status = UiToolActivityStatus::Failed;
            activity.detail = detail.clone();
            if let Some(display) = &mut activity.display {
                display.outcome = ToolDisplayOutcome::Failed;
                display.result_summary = detail.clone();
            }
        }
    }
}
