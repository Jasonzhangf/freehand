use crate::adp_descriptor::{command_dispatch_target, command_kind, is_command_ingress_kind};
use crate::adp_wire::{
    UiCommandDispatchEnvelope, UiCommandDispatchFailure, UiCommandIngressAck, UiProjection,
    UiProtocolRejection,
};
use crate::dto::*;
use crate::ports::SubscriptionSelector;
use crate::state::{UiCommandDispatchPortError, UiProtocolError};
use freehand_contracts::TurnId;

pub fn validate_command(command: &UiCommand) -> Result<(), UiProtocolError> {
    match command {
        UiCommand::CreateSession {
            session_id,
            title,
            cwd,
        } if session_id.as_str().trim().is_empty()
            || title.as_ref().is_some_and(|title| title.trim().is_empty())
            || cwd.as_ref().is_some_and(|cwd| cwd.trim().is_empty()) =>
        {
            if session_id.as_str().trim().is_empty() {
                Err(UiProtocolError::EmptySessionId)
            } else if cwd.as_ref().is_some_and(|cwd| cwd.trim().is_empty()) {
                Err(UiProtocolError::EmptySessionCwd)
            } else {
                Err(UiProtocolError::EmptySessionTitle)
            }
        }
        UiCommand::RenameSession { session_id, title }
            if session_id.as_str().trim().is_empty() || title.trim().is_empty() =>
        {
            if session_id.as_str().trim().is_empty() {
                Err(UiProtocolError::EmptySessionId)
            } else {
                Err(UiProtocolError::EmptySessionTitle)
            }
        }
        UiCommand::ArchiveSession { session_id }
        | UiCommand::RestoreSession { session_id }
        | UiCommand::DeleteSession { session_id }
        | UiCommand::RollbackLatestSessionTurn { session_id }
        | UiCommand::CompactSessionContext { session_id, .. }
            if session_id.as_str().trim().is_empty() =>
        {
            Err(UiProtocolError::EmptySessionId)
        }
        UiCommand::SubmitUserInput { text, metadata, .. }
            if text.trim().is_empty() && submit_metadata_attachments(metadata).is_empty() =>
        {
            Err(UiProtocolError::EmptyUserInput)
        }
        UiCommand::SubmitUserInput { metadata, .. }
            if submit_metadata_attachments(metadata)
                .iter()
                .any(invalid_input_attachment) =>
        {
            Err(UiProtocolError::InvalidInputAttachment)
        }
        UiCommand::SubmitUserInput { metadata, .. }
            if submit_metadata_attachments(metadata)
                .iter()
                .any(|attachment| attachment.kind != UiInputAttachmentKind::Image) =>
        {
            Err(UiProtocolError::UnsupportedInputAttachmentKind)
        }
        UiCommand::SubmitUserInput { cwd: Some(cwd), .. } if cwd.trim().is_empty() => {
            Err(UiProtocolError::EmptySessionCwd)
        }
        UiCommand::SendDirectMessageToSlave { text, .. } if text.trim().is_empty() => {
            Err(UiProtocolError::EmptySlaveMessage)
        }
        UiCommand::AddToMemory {
            session_id,
            content,
            ..
        } if session_id.as_str().trim().is_empty() => Err(UiProtocolError::EmptySessionId),
        UiCommand::AddToMemory { content, .. } if content.trim().is_empty() => {
            Err(UiProtocolError::EmptyMemoryContent)
        }
        UiCommand::RewindCheckpoint { checkpoint_id } if checkpoint_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyCheckpointId)
        }
        UiCommand::QueryTaskHistory { task_id } if task_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::QueryWorkerControl { task_id, .. } if task_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::QueryWorkerControl { execution_id, .. } if execution_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskExecutionId)
        }
        UiCommand::QueryEventInbox {
            after_cursor: Some(after_cursor),
            ..
        }
        | UiCommand::RunMasterPoll {
            after_cursor: Some(after_cursor),
            ..
        }
        | UiCommand::QueryMasterPoll {
            after_cursor: Some(after_cursor),
            ..
        } if after_cursor.trim().is_empty() => Err(UiProtocolError::EmptyEventCursor),
        UiCommand::RunMasterPoll {
            after_cursor: Some(_),
            replay_from_start: true,
            ..
        }
        | UiCommand::QueryMasterPoll {
            after_cursor: Some(_),
            replay_from_start: true,
            ..
        } => Err(UiProtocolError::ConflictingMasterPollCursorMode),
        UiCommand::CreateTask { task } if task.title.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskTitle)
        }
        UiCommand::CreateTask { task } if task.content.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskContent)
        }
        UiCommand::CreateTask { task } if task.goal.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskGoal)
        }
        UiCommand::CreateTask { task }
            if task
                .task_id
                .as_ref()
                .is_some_and(|task_id| task_id.trim().is_empty()) =>
        {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::CreateTask { task }
            if task
                .target_cwd
                .as_ref()
                .is_some_and(|cwd| cwd.trim().is_empty()) =>
        {
            Err(UiProtocolError::EmptySessionCwd)
        }
        UiCommand::SubmitTaskReview { review } if review.task_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::SubmitTaskReview { review } if review.summary.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskReviewSummary)
        }
        UiCommand::CreateTaskAgent { agent } if agent.agent_id.as_str().trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskAgentId)
        }
        UiCommand::CreateTaskAgent { agent } if agent.capabilities.is_empty() => {
            Err(UiProtocolError::EmptyTaskCapabilities)
        }
        UiCommand::AssignTask { assignment } if assignment.task_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::AssignTask { assignment } if assignment.agent_id.as_str().trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskAgentId)
        }
        UiCommand::ClaimNextTask { claim } if claim.agent_id.as_str().trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskAgentId)
        }
        UiCommand::ClaimNextTask { claim } if claim.execution_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskExecutionId)
        }
        UiCommand::RejectTaskReview { rejection } if rejection.task_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::RejectTaskReview { rejection } if rejection.reject_reason.trim().is_empty() => {
            Err(UiProtocolError::EmptyTaskReviewRejection)
        }
        UiCommand::RejectTaskReview { rejection } if rejection.next_requirements.is_empty() => {
            Err(UiProtocolError::EmptyTaskReviewRejection)
        }
        UiCommand::ApproveTaskReview { task_id } | UiCommand::CloseTask { task_id }
            if task_id.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyTaskId)
        }
        UiCommand::WorkerControl { control } => validate_worker_control_command(control),
        UiCommand::ScheduleTimer { timer } => validate_timer_schedule_command(timer),
        UiCommand::CancelTimer { timer_id } if timer_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyTimerId)
        }
        UiCommand::UpdateProviderConfig { update } | UiCommand::UpsertProviderConfig { update }
            if update.agent_name.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyConfigAgentName)
        }
        UiCommand::UpdateProviderConfig { update } | UiCommand::UpsertProviderConfig { update }
            if update.provider_id.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyProviderId)
        }
        UiCommand::UpdateProviderConfig { update } | UiCommand::UpsertProviderConfig { update }
            if update.provider_type.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyProviderType)
        }
        UiCommand::UpdateProviderConfig { update } | UiCommand::UpsertProviderConfig { update }
            if update.provider_protocol.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyProviderProtocol)
        }
        UiCommand::UpdateProviderConfig { update } | UiCommand::UpsertProviderConfig { update }
            if update.base_url.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyProviderBaseUrl)
        }
        UiCommand::UpdateProviderConfig { update } | UiCommand::UpsertProviderConfig { update }
            if update.default_model.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyProviderDefaultModel)
        }
        UiCommand::UpdateProviderConfig { update } | UiCommand::UpsertProviderConfig { update }
            if update.api_key_env.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyProviderApiKeyEnv)
        }
        UiCommand::UpsertModelGroupConfig { group } => validate_model_group_config_update(group),
        UiCommand::UpdateAgentModelGroupSelection { selection }
            if selection.agent_name.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyConfigAgentName)
        }
        UiCommand::UpdateAgentModelGroupSelection { selection }
            if selection
                .model_group_id
                .as_deref()
                .is_some_and(|value| value.trim().is_empty()) =>
        {
            Err(UiProtocolError::EmptyModelGroupId)
        }
        UiCommand::TestProviderWebSearch { provider_id, .. } if provider_id.trim().is_empty() => {
            Err(UiProtocolError::EmptyProviderId)
        }
        UiCommand::TestProviderWebSearch {
            query: Some(query), ..
        } if query.trim().is_empty() => Err(UiProtocolError::EmptyUserInput),
        UiCommand::QuerySessionSearch { query, .. } if query.trim().is_empty() => {
            Err(UiProtocolError::EmptyUserInput)
        }
        UiCommand::QueryMemory {
            query: Some(query), ..
        } if query.trim().is_empty() => Err(UiProtocolError::EmptyUserInput),
        UiCommand::QueryMemory {
            limit: Some(limit), ..
        } if !(1..=100).contains(limit) => Err(UiProtocolError::InvalidMemoryQueryLimit),
        UiCommand::QuerySessionTurnsPage { session_id, page }
            if session_id.as_str().trim().is_empty() =>
        {
            Err(UiProtocolError::EmptySessionId)
        }
        UiCommand::QuerySessionTurnsPage { page, .. } if !(1..=100).contains(&page.limit) => {
            Err(UiProtocolError::InvalidTurnPageLimit)
        }
        UiCommand::QuerySessionTurnsPage { page, .. }
            if matches!(page.direction, UiSessionTurnsPageDirection::Latest)
                && page.before_turn_id.is_some() =>
        {
            Err(UiProtocolError::InvalidTurnPageCursor)
        }
        UiCommand::QuerySessionTurnsPage { page, .. }
            if matches!(page.direction, UiSessionTurnsPageDirection::Older)
                && page.before_turn_id.is_none() =>
        {
            Err(UiProtocolError::InvalidTurnPageCursor)
        }
        UiCommand::QuerySessionListPage { page, .. } if !(1..=100).contains(&page.limit) => {
            Err(UiProtocolError::InvalidSessionListPageLimit)
        }
        UiCommand::QuerySessionListPage { page, .. }
            if matches!(page.direction, UiSessionListPageDirection::Latest)
                && page.cursor.is_some() =>
        {
            Err(UiProtocolError::InvalidSessionListPageCursor)
        }
        UiCommand::QuerySessionListPage { page, .. }
            if matches!(page.direction, UiSessionListPageDirection::Older)
                && page
                    .cursor
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty() =>
        {
            Err(UiProtocolError::InvalidSessionListPageCursor)
        }
        UiCommand::UpdateAgentProviderSelection { selection }
            if selection.agent_name.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyConfigAgentName)
        }
        UiCommand::UpdateAgentProviderSelection { selection }
            if selection.provider_id.trim().is_empty() =>
        {
            Err(UiProtocolError::EmptyProviderId)
        }
        UiCommand::UpdateAgentProviderSelection { selection }
            if selection
                .fallback_provider_id
                .as_deref()
                .is_some_and(|value| value.trim().is_empty()) =>
        {
            Err(UiProtocolError::EmptyProviderId)
        }
        UiCommand::UpdateAgentResourceConfig { update } if update.agent_name.trim().is_empty() => {
            Err(UiProtocolError::EmptyConfigAgentName)
        }
        UiCommand::UpdateAgentResourceConfig { update }
            if !(1..=5).contains(&update.resource_count) =>
        {
            Err(UiProtocolError::AgentResourceCountOutOfRange {
                resource_count: update.resource_count,
            })
        }
        UiCommand::QueryErrorCenterEvents { session_id, .. }
        | UiCommand::SubscribeErrorCenterEvents { session_id, .. }
            if session_id.as_str().trim().is_empty() =>
        {
            Err(UiProtocolError::EmptySessionId)
        }
        _ => Ok(()),
    }
}

fn submit_metadata_attachments(metadata: &Option<UiSubmitMetadata>) -> &[UiInputAttachment] {
    metadata
        .as_ref()
        .map(|metadata| metadata.attachments.as_slice())
        .unwrap_or(&[])
}

fn invalid_input_attachment(attachment: &UiInputAttachment) -> bool {
    attachment.attachment_id.trim().is_empty()
        || attachment.name.trim().is_empty()
        || attachment.media_type.trim().is_empty()
        || attachment
            .data_base64
            .as_deref()
            .is_none_or(|data| data.trim().is_empty())
}

fn validate_model_group_config_update(
    group: &UiModelGroupConfigUpdate,
) -> Result<(), UiProtocolError> {
    if group.agent_name.trim().is_empty() {
        return Err(UiProtocolError::EmptyConfigAgentName);
    }
    if group.group_id.trim().is_empty() {
        return Err(UiProtocolError::EmptyModelGroupId);
    }
    if group.context_window_tokens == 0
        || group.compaction_threshold_tokens == 0
        || group.compaction_threshold_tokens >= group.context_window_tokens
    {
        return Err(UiProtocolError::InvalidModelCompactionThreshold);
    }
    validate_model_route_update(&group.primary)?;
    for route in [
        group.sub.as_ref(),
        group.search.as_ref(),
        group.title.as_ref(),
        group.fallback.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_model_route_update(route)?;
    }
    for route in &group.load_balance {
        validate_model_route_fields(&route.provider_id, &route.model)?;
        if route.weight == 0 {
            return Err(UiProtocolError::EmptyModelRouteWeight);
        }
    }
    Ok(())
}

fn validate_model_route_update(route: &UiModelRouteUpdate) -> Result<(), UiProtocolError> {
    validate_model_route_fields(&route.provider_id, &route.model)
}

fn validate_model_route_fields(provider_id: &str, model: &str) -> Result<(), UiProtocolError> {
    if provider_id.trim().is_empty() {
        return Err(UiProtocolError::EmptyModelRouteProvider);
    }
    if model.trim().is_empty() {
        return Err(UiProtocolError::EmptyModelRouteModel);
    }
    Ok(())
}

fn validate_timer_schedule_command(timer: &UiTimerScheduleCommand) -> Result<(), UiProtocolError> {
    if timer
        .timer_id
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(UiProtocolError::EmptyTimerId);
    }
    if timer.mode.trim().is_empty() {
        return Err(UiProtocolError::EmptyTimerMode);
    }
    if timer.reason.trim().is_empty() {
        return Err(UiProtocolError::EmptyTimerReason);
    }
    if timer.prompt.trim().is_empty() {
        return Err(UiProtocolError::EmptyTimerPrompt);
    }
    if timer
        .source_session_id
        .as_ref()
        .is_some_and(|session_id| session_id.as_str().trim().is_empty())
    {
        return Err(UiProtocolError::EmptySessionId);
    }
    if timer.max_runs == Some(0) {
        return Err(UiProtocolError::InvalidTimerRepeat(
            "max_runs must be greater than zero".to_owned(),
        ));
    }
    match timer.mode.trim() {
        "relative" => {
            if timer.delay_seconds.unwrap_or(0) == 0 {
                return Err(UiProtocolError::MissingTimerDelay);
            }
        }
        "absolute" => {
            if timer.run_at_unix_seconds.is_none() {
                return Err(UiProtocolError::MissingTimerRunAt);
            }
        }
        "recurring" => {
            let Some(repeat) = timer.repeat.as_ref() else {
                return Err(UiProtocolError::MissingTimerRepeat);
            };
            validate_timer_repeat_command(repeat)?;
        }
        other => return Err(UiProtocolError::UnknownTimerMode(other.to_owned())),
    }
    Ok(())
}

fn validate_timer_repeat_command(repeat: &UiTimerRepeatCommand) -> Result<(), UiProtocolError> {
    match repeat {
        UiTimerRepeatCommand::Interval {
            interval_seconds,
            max_runs,
        } => {
            if *interval_seconds == 0 {
                return Err(UiProtocolError::InvalidTimerRepeat(
                    "interval_seconds must be greater than zero".to_owned(),
                ));
            }
            if *max_runs == Some(0) {
                return Err(UiProtocolError::InvalidTimerRepeat(
                    "repeat max_runs must be greater than zero".to_owned(),
                ));
            }
        }
        UiTimerRepeatCommand::Daily {
            time_of_day_seconds_local,
            max_runs,
            ..
        } => {
            if *time_of_day_seconds_local >= 86_400 {
                return Err(UiProtocolError::InvalidTimerRepeat(
                    "time_of_day_seconds_local must be in 0..86400".to_owned(),
                ));
            }
            if *max_runs == Some(0) {
                return Err(UiProtocolError::InvalidTimerRepeat(
                    "repeat max_runs must be greater than zero".to_owned(),
                ));
            }
        }
        UiTimerRepeatCommand::Weekly {
            time_of_day_seconds_local,
            weekdays,
            max_runs,
        } => {
            if *time_of_day_seconds_local >= 86_400 {
                return Err(UiProtocolError::InvalidTimerRepeat(
                    "time_of_day_seconds_local must be in 0..86400".to_owned(),
                ));
            }
            if weekdays.is_empty() || weekdays.iter().any(|day| *day > 6) {
                return Err(UiProtocolError::InvalidTimerRepeat(
                    "weekdays must contain integers 0..6".to_owned(),
                ));
            }
            if *max_runs == Some(0) {
                return Err(UiProtocolError::InvalidTimerRepeat(
                    "repeat max_runs must be greater than zero".to_owned(),
                ));
            }
        }
        UiTimerRepeatCommand::Cron {
            expression,
            max_runs,
        } => {
            if expression.trim().is_empty() {
                return Err(UiProtocolError::InvalidTimerRepeat(
                    "cron expression must be non-empty".to_owned(),
                ));
            }
            if *max_runs == Some(0) {
                return Err(UiProtocolError::InvalidTimerRepeat(
                    "repeat max_runs must be greater than zero".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_worker_control_command(
    control: &UiWorkerControlCommand,
) -> Result<(), UiProtocolError> {
    if control
        .control_id
        .as_ref()
        .is_some_and(|control_id| control_id.trim().is_empty())
    {
        return Err(UiProtocolError::EmptyWorkerControlId);
    }
    if control.task_id.trim().is_empty() {
        return Err(UiProtocolError::EmptyTaskId);
    }
    if control.execution_id.trim().is_empty() {
        return Err(UiProtocolError::EmptyTaskExecutionId);
    }
    if control.agent_id.as_str().trim().is_empty() {
        return Err(UiProtocolError::EmptyTaskAgentId);
    }
    let op = control.op.trim();
    if op.is_empty() {
        return Err(UiProtocolError::EmptyWorkerControlOp);
    }
    if !matches!(
        op,
        "query_status"
            | "ask_at_safe_point"
            | "add_constraint"
            | "request_checkpoint"
            | "request_submission_now"
            | "pause"
            | "resume"
            | "cancel"
    ) {
        return Err(UiProtocolError::UnknownWorkerControlOp(control.op.clone()));
    }
    if control
        .question
        .as_ref()
        .is_some_and(|question| question.trim().is_empty())
        || (op == "ask_at_safe_point" && control.question.is_none())
    {
        return Err(UiProtocolError::EmptyWorkerControlQuestion);
    }
    if control
        .constraint
        .as_ref()
        .is_some_and(|constraint| constraint.trim().is_empty())
        || (op == "add_constraint" && control.constraint.is_none())
    {
        return Err(UiProtocolError::EmptyWorkerControlConstraint);
    }
    if control
        .note
        .as_ref()
        .is_some_and(|note| note.trim().is_empty())
    {
        return Err(UiProtocolError::EmptyWorkerControlNote);
    }
    Ok(())
}

pub fn accept_command_ingress(command: &UiCommand) -> Result<UiCommandIngressAck, UiProtocolError> {
    validate_command(command)?;
    if !is_command_ingress_kind(command) {
        return Err(UiProtocolError::IngressCommandKindMismatch);
    }
    Ok(UiCommandIngressAck {
        command_kind: command_kind(command).to_owned(),
        accepted: true,
        status_text: "command accepted for owner-module handling".to_owned(),
        mutation_authority: "owner_modules".to_owned(),
    })
}

pub fn protocol_rejection(err: UiProtocolError) -> UiProtocolRejection {
    let code = match err {
        UiProtocolError::EmptySessionId => "empty_session_id",
        UiProtocolError::InvalidTurnPageLimit => "invalid_turn_page_limit",
        UiProtocolError::InvalidTurnPageCursor => "invalid_turn_page_cursor",
        UiProtocolError::EmptySessionTitle => "empty_session_title",
        UiProtocolError::EmptyMemoryContent => "empty_memory_content",
        UiProtocolError::EmptyUserInput => "empty_user_input",
        UiProtocolError::InvalidInputAttachment => "invalid_input_attachment",
        UiProtocolError::UnsupportedInputAttachmentKind => "unsupported_input_attachment_kind",
        UiProtocolError::EmptySessionCwd => "empty_session_cwd",
        UiProtocolError::EmptySlaveMessage => "empty_slave_message",
        UiProtocolError::EmptyCheckpointId => "empty_checkpoint_id",
        UiProtocolError::EmptyTaskId => "empty_task_id",
        UiProtocolError::EmptyTaskTitle => "empty_task_title",
        UiProtocolError::EmptyTaskContent => "empty_task_content",
        UiProtocolError::EmptyTaskGoal => "empty_task_goal",
        UiProtocolError::EmptyTaskReviewSummary => "empty_task_review_summary",
        UiProtocolError::EmptyTaskReviewRejection => "empty_task_review_rejection",
        UiProtocolError::EmptyTaskExecutionId => "empty_task_execution_id",
        UiProtocolError::EmptyWorkerControlId => "empty_worker_control_id",
        UiProtocolError::EmptyWorkerControlOp => "empty_worker_control_op",
        UiProtocolError::UnknownWorkerControlOp(_) => "unknown_worker_control_op",
        UiProtocolError::EmptyWorkerControlQuestion => "empty_worker_control_question",
        UiProtocolError::EmptyWorkerControlConstraint => "empty_worker_control_constraint",
        UiProtocolError::EmptyWorkerControlNote => "empty_worker_control_note",
        UiProtocolError::EmptyTimerId => "empty_timer_id",
        UiProtocolError::EmptyTimerMode => "empty_timer_mode",
        UiProtocolError::UnknownTimerMode(_) => "unknown_timer_mode",
        UiProtocolError::EmptyTimerReason => "empty_timer_reason",
        UiProtocolError::EmptyTimerPrompt => "empty_timer_prompt",
        UiProtocolError::MissingTimerDelay => "missing_timer_delay",
        UiProtocolError::MissingTimerRunAt => "missing_timer_run_at",
        UiProtocolError::MissingTimerRepeat => "missing_timer_repeat",
        UiProtocolError::InvalidTimerRepeat(_) => "invalid_timer_repeat",
        UiProtocolError::EmptyEventCursor => "empty_event_cursor",
        UiProtocolError::ConflictingMasterPollCursorMode => "conflicting_master_poll_cursor_mode",
        UiProtocolError::EmptyTaskAgentId => "empty_task_agent_id",
        UiProtocolError::EmptyTaskCapabilities => "empty_task_capabilities",
        UiProtocolError::EmptyConfigAgentName => "empty_config_agent_name",
        UiProtocolError::EmptyProviderId => "empty_provider_id",
        UiProtocolError::EmptyProviderType => "empty_provider_type",
        UiProtocolError::EmptyProviderProtocol => "empty_provider_protocol",
        UiProtocolError::EmptyProviderBaseUrl => "empty_provider_base_url",
        UiProtocolError::EmptyProviderDefaultModel => "empty_provider_default_model",
        UiProtocolError::EmptyProviderApiKeyEnv => "empty_provider_api_key_env",
        UiProtocolError::EmptyModelGroupId => "empty_model_group_id",
        UiProtocolError::InvalidModelCompactionThreshold => "invalid_model_compaction_threshold",
        UiProtocolError::EmptyModelRouteProvider => "empty_model_route_provider",
        UiProtocolError::EmptyModelRouteModel => "empty_model_route_model",
        UiProtocolError::EmptyModelRouteWeight => "empty_model_route_weight",
        UiProtocolError::AgentResourceCountOutOfRange { .. } => "agent_resource_count_out_of_range",
        UiProtocolError::IngressCommandKindMismatch => "ingress_command_kind_mismatch",
        UiProtocolError::QueryCommandKindMismatch => "direct_task_mutation_forbidden",
        UiProtocolError::StreamKindMismatch => "stream_kind_mismatch",
        UiProtocolError::InvalidSessionListPageLimit => "invalid_session_list_page_limit",
        UiProtocolError::InvalidSessionListPageCursor => "invalid_session_list_page_cursor",
        UiProtocolError::InvalidMemoryQueryLimit => "invalid_memory_query_limit",
    };
    UiProtocolRejection {
        code: code.to_owned(),
        message: err.to_string(),
    }
}

pub fn build_command_dispatch_envelope(
    command: &UiCommand,
) -> Result<UiCommandDispatchEnvelope, UiProtocolError> {
    let ingress = accept_command_ingress(command)?;
    let (target_feature_id, target_owner_module) = command_dispatch_target(command);
    Ok(UiCommandDispatchEnvelope {
        ingress,
        command: command.clone(),
        target_feature_id: target_feature_id.to_owned(),
        target_owner_module: target_owner_module.to_owned(),
    })
}

pub fn dispatch_port_failure(err: UiCommandDispatchPortError) -> UiCommandDispatchFailure {
    match err {
        UiCommandDispatchPortError::DispatchFailed(message) => UiCommandDispatchFailure {
            code: "command_dispatch_port_failure".to_owned(),
            message: format!("dispatch port failure: {message}"),
            retryable: true,
        },
        UiCommandDispatchPortError::TargetNotFound(message) => UiCommandDispatchFailure {
            code: "command_dispatch_target_not_found".to_owned(),
            message: format!("dispatch target not found: {message}"),
            retryable: false,
        },
        UiCommandDispatchPortError::Unsupported(message) => UiCommandDispatchFailure {
            code: "command_dispatch_unsupported".to_owned(),
            message: format!("dispatch path unsupported: {message}"),
            retryable: false,
        },
    }
}

pub fn subscription_selector(command: &UiCommand) -> Option<SubscriptionSelector> {
    match command {
        UiCommand::SubscribeLatestActiveTurn { client } => Some(SubscriptionSelector {
            client: *client,
            stream_kind: UiStreamKind::Turn,
            target_turn_id: None,
        }),
        UiCommand::SubscribeTurn { client, turn_id } => Some(SubscriptionSelector {
            client: *client,
            stream_kind: UiStreamKind::Turn,
            target_turn_id: Some(turn_id.clone()),
        }),
        UiCommand::SubscribeNodeStatus => Some(SubscriptionSelector {
            client: UiClientKind::WebUi,
            stream_kind: UiStreamKind::NodeStatus,
            target_turn_id: None,
        }),
        UiCommand::SubscribeProgress => Some(SubscriptionSelector {
            client: UiClientKind::WebUi,
            stream_kind: UiStreamKind::Progress,
            target_turn_id: None,
        }),
        UiCommand::SubscribeTaskList { .. } => Some(SubscriptionSelector {
            client: UiClientKind::WebUi,
            stream_kind: UiStreamKind::TaskList,
            target_turn_id: None,
        }),
        UiCommand::SubscribeErrorCenterEvents { .. } => Some(SubscriptionSelector {
            client: UiClientKind::WebUi,
            stream_kind: UiStreamKind::ErrorCenter,
            target_turn_id: None,
        }),
        UiCommand::SubscribeDebugState { client, turn_id } => Some(SubscriptionSelector {
            client: *client,
            stream_kind: UiStreamKind::Debug,
            target_turn_id: Some(turn_id.clone()),
        }),
        _ => None,
    }
}

pub fn subscription_matches(
    selector: &SubscriptionSelector,
    projection: &UiProjection,
    latest_active_turn_id: Option<&TurnId>,
) -> bool {
    match (selector.stream_kind, projection) {
        (UiStreamKind::Turn, UiProjection::Turn(turn)) => match selector.target_turn_id.as_ref() {
            Some(target) => target == &turn.turn_id,
            None => latest_active_turn_id == Some(&turn.turn_id),
        },
        (UiStreamKind::Progress, UiProjection::Progress(_)) => true,
        (UiStreamKind::NodeStatus, UiProjection::NodeStatus(_)) => true,
        (UiStreamKind::Debug, UiProjection::Debug(debug)) => {
            selector.target_turn_id.as_ref() == Some(&debug.semantic.turn_id)
        }
        (UiStreamKind::Checkpoint, UiProjection::Checkpoints(_)) => true,
        (UiStreamKind::TaskList, UiProjection::TaskList(_)) => true,
        (UiStreamKind::ErrorCenter, UiProjection::ErrorCenterEvents(_)) => true,
        _ => false,
    }
}
