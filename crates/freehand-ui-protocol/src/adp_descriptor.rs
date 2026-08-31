use crate::adp_wire::TurnProjectionInput;
use crate::dto::*;
use crate::{
    UiProtocolError, fail_waiting_tool_activities, human_friendly_terminal_text,
    tool_activities_from_input, validate_command,
};
use freehand_contracts::{SemanticEventKind, TerminalStatus};
use serde::{Deserialize, Serialize};

pub(crate) struct UiCommandDescriptor {
    pub(crate) serde_name: &'static str,
    pub(crate) semantic_kind: &'static str,
    pub(crate) frame_class: UiCommandFrameClass,
    pub(crate) target_owner_feature: &'static str,
    pub(crate) target_owner_module: &'static str,
    pub(crate) exposure: UiAdpCommandExposure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiAdpCommandExposure {
    Public,
    Internal,
}

pub(crate) const UI_COMMAND_DESCRIPTORS: &[UiCommandDescriptor] = &[
    UiCommandDescriptor {
        serde_name: "CreateSession",
        semantic_kind: "create_session",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.persistence",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "RenameSession",
        semantic_kind: "rename_session",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.persistence",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "ArchiveSession",
        semantic_kind: "archive_session",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.persistence",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "RestoreSession",
        semantic_kind: "restore_session",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.persistence",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "DeleteSession",
        semantic_kind: "delete_session",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.persistence",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "RollbackLatestSessionTurn",
        semantic_kind: "rollback_latest_session_turn",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.persistence",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "SubmitUserInput",
        semantic_kind: "submit_user_input",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.turn",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "SubscribeLatestActiveTurn",
        semantic_kind: "subscribe_latest_active_turn",
        frame_class: UiCommandFrameClass::Subscribe,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "SubscribeTurn",
        semantic_kind: "subscribe_turn",
        frame_class: UiCommandFrameClass::Subscribe,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "SubscribeNodeStatus",
        semantic_kind: "subscribe_node_status",
        frame_class: UiCommandFrameClass::Subscribe,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "SubscribeProgress",
        semantic_kind: "subscribe_progress",
        frame_class: UiCommandFrameClass::Subscribe,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "SubscribeTaskList",
        semantic_kind: "subscribe_task_list",
        frame_class: UiCommandFrameClass::Subscribe,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "SubscribeErrorCenterEvents",
        semantic_kind: "subscribe_error_center_events",
        frame_class: UiCommandFrameClass::Subscribe,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "SubscribeDebugState",
        semantic_kind: "subscribe_debug_state",
        frame_class: UiCommandFrameClass::Subscribe,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryLatestActiveTurn",
        semantic_kind: "query_latest_active_turn",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryTurn",
        semantic_kind: "query_turn",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QuerySessionListPage",
        semantic_kind: "query_session_list_page",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QuerySessionTurns",
        semantic_kind: "query_session_turns",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QuerySessionTurnsPage",
        semantic_kind: "query_session_turns_page",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QuerySessionSearch",
        semantic_kind: "query_session_search",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryMemory",
        semantic_kind: "query_memory",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "reason.persistence",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryConfigStatus",
        semantic_kind: "query_config_status",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryTaskList",
        semantic_kind: "query_task_list",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryTaskBoard",
        semantic_kind: "query_task_board",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryEventInbox",
        semantic_kind: "query_event_inbox",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryAgentBoard",
        semantic_kind: "query_agent_board",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryAgentLifecycle",
        semantic_kind: "query_agent_lifecycle",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryTaskHistory",
        semantic_kind: "query_task_history",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryWorkerControl",
        semantic_kind: "query_worker_control",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryTimerList",
        semantic_kind: "query_timer_list",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryToolRegistry",
        semantic_kind: "query_tool_registry",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryDiagnostics",
        semantic_kind: "query_diagnostics",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "PullAccountConfig",
        semantic_kind: "pull_account_config",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "config.account-config-sync",
        target_owner_module: "crates/freehand-account-config",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "PushAccountConfig",
        semantic_kind: "push_account_config",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "config.account-config-sync",
        target_owner_module: "crates/freehand-account-config",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "AddToMemory",
        semantic_kind: "add_to_memory",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.persistence",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryErrorCenterEvents",
        semantic_kind: "query_error_center_events",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "UpdateProviderConfig",
        semantic_kind: "update_provider_config",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "config.core",
        target_owner_module: "crates/freehand-config",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "UpsertProviderConfig",
        semantic_kind: "upsert_provider_config",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "config.core",
        target_owner_module: "crates/freehand-config",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "UpsertModelGroupConfig",
        semantic_kind: "upsert_model_group_config",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "config.core",
        target_owner_module: "crates/freehand-config",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "UpdateAgentModelGroupSelection",
        semantic_kind: "update_agent_model_group_selection",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "config.core",
        target_owner_module: "crates/freehand-config",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "TestProviderWebSearch",
        semantic_kind: "test_provider_web_search",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "provider.reason-live-bridge",
        target_owner_module: "crates/freehand-runtime",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "ScheduleTimer",
        semantic_kind: "schedule_timer",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "runtime.master-worker-loop",
        target_owner_module: "crates/freehand-runtime",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "CancelTimer",
        semantic_kind: "cancel_timer",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "runtime.master-worker-loop",
        target_owner_module: "crates/freehand-runtime",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "UpdateAgentProviderSelection",
        semantic_kind: "update_agent_provider_selection",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "config.core",
        target_owner_module: "crates/freehand-config",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "UpdateAgentResourceConfig",
        semantic_kind: "update_agent_resource_config",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "config.core",
        target_owner_module: "crates/freehand-config",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "CreateTask",
        semantic_kind: "create_task",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "CreateTaskAgent",
        semantic_kind: "create_task_agent",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "AssignTask",
        semantic_kind: "assign_task",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "ClaimNextTask",
        semantic_kind: "claim_next_task",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Internal,
    },
    UiCommandDescriptor {
        serde_name: "SubmitTaskReview",
        semantic_kind: "submit_task_review",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "RejectTaskReview",
        semantic_kind: "reject_task_review",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "ApproveTaskReview",
        semantic_kind: "approve_task_review",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "CloseTask",
        semantic_kind: "close_task",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "ApplyExecutionFact",
        semantic_kind: "apply_execution_fact",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Internal,
    },
    UiCommandDescriptor {
        serde_name: "RunSchedulerTick",
        semantic_kind: "run_scheduler_tick",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Internal,
    },
    UiCommandDescriptor {
        serde_name: "RunMasterPoll",
        semantic_kind: "run_master_poll",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Internal,
    },
    UiCommandDescriptor {
        serde_name: "QueryMasterPoll",
        semantic_kind: "query_master_poll",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "task.orchestration",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "WorkerControl",
        semantic_kind: "worker_control",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "worker.control",
        target_owner_module: "crates/freehand-task",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryNodeStatus",
        semantic_kind: "query_node_status",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryTaskProgress",
        semantic_kind: "query_task_progress",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryDebugState",
        semantic_kind: "query_debug_state",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryCheckpoints",
        semantic_kind: "query_checkpoints",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "SendDirectMessageToSlave",
        semantic_kind: "send_direct_message_to_slave",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "node.master-slave",
        target_owner_module: "crates/freehand-node",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "RewindCheckpoint",
        semantic_kind: "rewind_checkpoint",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "runtime.checkpoint-rewind",
        target_owner_module: "crates/freehand-runtime",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "CancelTurn",
        semantic_kind: "cancel_turn",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.turn",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "CancelLatestActiveTurn",
        semantic_kind: "cancel_latest_active_turn",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.turn",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "ResumeTurn",
        semantic_kind: "resume_turn",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.turn",
        target_owner_module: "crates/freehand-reason",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "CompactSessionContext",
        semantic_kind: "compact_session_context",
        frame_class: UiCommandFrameClass::Mutation,
        target_owner_feature: "reason.rewrite-policy",
        target_owner_module: "crates/freehand-blocks",
        exposure: UiAdpCommandExposure::Public,
    },
];

pub(crate) fn command_descriptor_by_serde_name(serde_name: &str) -> &'static UiCommandDescriptor {
    UI_COMMAND_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.serde_name == serde_name)
        .expect("UiCommand descriptor table must contain every UiCommand variant")
}

pub(crate) fn command_descriptor(command: &UiCommand) -> &'static UiCommandDescriptor {
    match command {
        UiCommand::CreateSession { .. } => command_descriptor_by_serde_name("CreateSession"),
        UiCommand::RenameSession { .. } => command_descriptor_by_serde_name("RenameSession"),
        UiCommand::ArchiveSession { .. } => command_descriptor_by_serde_name("ArchiveSession"),
        UiCommand::RestoreSession { .. } => command_descriptor_by_serde_name("RestoreSession"),
        UiCommand::DeleteSession { .. } => command_descriptor_by_serde_name("DeleteSession"),
        UiCommand::RollbackLatestSessionTurn { .. } => {
            command_descriptor_by_serde_name("RollbackLatestSessionTurn")
        }
        UiCommand::SubmitUserInput { .. } => command_descriptor_by_serde_name("SubmitUserInput"),
        UiCommand::SubscribeLatestActiveTurn { .. } => {
            command_descriptor_by_serde_name("SubscribeLatestActiveTurn")
        }
        UiCommand::SubscribeTurn { .. } => command_descriptor_by_serde_name("SubscribeTurn"),
        UiCommand::SubscribeNodeStatus => command_descriptor_by_serde_name("SubscribeNodeStatus"),
        UiCommand::SubscribeProgress => command_descriptor_by_serde_name("SubscribeProgress"),
        UiCommand::SubscribeTaskList { .. } => {
            command_descriptor_by_serde_name("SubscribeTaskList")
        }
        UiCommand::SubscribeErrorCenterEvents { .. } => {
            command_descriptor_by_serde_name("SubscribeErrorCenterEvents")
        }
        UiCommand::SubscribeDebugState { .. } => {
            command_descriptor_by_serde_name("SubscribeDebugState")
        }
        UiCommand::QueryLatestActiveTurn => {
            command_descriptor_by_serde_name("QueryLatestActiveTurn")
        }
        UiCommand::QueryTurn { .. } => command_descriptor_by_serde_name("QueryTurn"),
        UiCommand::QuerySessionListPage { .. } => {
            command_descriptor_by_serde_name("QuerySessionListPage")
        }
        UiCommand::QuerySessionTurns { .. } => {
            command_descriptor_by_serde_name("QuerySessionTurns")
        }
        UiCommand::QuerySessionTurnsPage { .. } => {
            command_descriptor_by_serde_name("QuerySessionTurnsPage")
        }
        UiCommand::QuerySessionSearch { .. } => {
            command_descriptor_by_serde_name("QuerySessionSearch")
        }
        UiCommand::QueryMemory { .. } => command_descriptor_by_serde_name("QueryMemory"),
        UiCommand::QueryConfigStatus => command_descriptor_by_serde_name("QueryConfigStatus"),
        UiCommand::QueryTaskList { .. } => command_descriptor_by_serde_name("QueryTaskList"),
        UiCommand::QueryTaskBoard { .. } => command_descriptor_by_serde_name("QueryTaskBoard"),
        UiCommand::QueryEventInbox { .. } => command_descriptor_by_serde_name("QueryEventInbox"),
        UiCommand::QueryAgentBoard => command_descriptor_by_serde_name("QueryAgentBoard"),
        UiCommand::QueryAgentLifecycle { .. } => {
            command_descriptor_by_serde_name("QueryAgentLifecycle")
        }
        UiCommand::QueryTaskHistory { .. } => command_descriptor_by_serde_name("QueryTaskHistory"),
        UiCommand::QueryWorkerControl { .. } => {
            command_descriptor_by_serde_name("QueryWorkerControl")
        }
        UiCommand::QueryTimerList { .. } => command_descriptor_by_serde_name("QueryTimerList"),
        UiCommand::QueryToolRegistry => command_descriptor_by_serde_name("QueryToolRegistry"),
        UiCommand::QueryDiagnostics => command_descriptor_by_serde_name("QueryDiagnostics"),
        UiCommand::PullAccountConfig => command_descriptor_by_serde_name("PullAccountConfig"),
        UiCommand::PushAccountConfig => command_descriptor_by_serde_name("PushAccountConfig"),
        UiCommand::AddToMemory { .. } => command_descriptor_by_serde_name("AddToMemory"),
        UiCommand::QueryErrorCenterEvents { .. } => {
            command_descriptor_by_serde_name("QueryErrorCenterEvents")
        }
        UiCommand::UpdateProviderConfig { .. } => {
            command_descriptor_by_serde_name("UpdateProviderConfig")
        }
        UiCommand::UpsertProviderConfig { .. } => {
            command_descriptor_by_serde_name("UpsertProviderConfig")
        }
        UiCommand::UpsertModelGroupConfig { .. } => {
            command_descriptor_by_serde_name("UpsertModelGroupConfig")
        }
        UiCommand::UpdateAgentModelGroupSelection { .. } => {
            command_descriptor_by_serde_name("UpdateAgentModelGroupSelection")
        }
        UiCommand::TestProviderWebSearch { .. } => {
            command_descriptor_by_serde_name("TestProviderWebSearch")
        }
        UiCommand::ScheduleTimer { .. } => command_descriptor_by_serde_name("ScheduleTimer"),
        UiCommand::CancelTimer { .. } => command_descriptor_by_serde_name("CancelTimer"),
        UiCommand::UpdateAgentProviderSelection { .. } => {
            command_descriptor_by_serde_name("UpdateAgentProviderSelection")
        }
        UiCommand::UpdateAgentResourceConfig { .. } => {
            command_descriptor_by_serde_name("UpdateAgentResourceConfig")
        }
        UiCommand::CreateTask { .. } => command_descriptor_by_serde_name("CreateTask"),
        UiCommand::CreateTaskAgent { .. } => command_descriptor_by_serde_name("CreateTaskAgent"),
        UiCommand::AssignTask { .. } => command_descriptor_by_serde_name("AssignTask"),
        UiCommand::ClaimNextTask { .. } => command_descriptor_by_serde_name("ClaimNextTask"),
        UiCommand::SubmitTaskReview { .. } => command_descriptor_by_serde_name("SubmitTaskReview"),
        UiCommand::RejectTaskReview { .. } => command_descriptor_by_serde_name("RejectTaskReview"),
        UiCommand::ApproveTaskReview { .. } => {
            command_descriptor_by_serde_name("ApproveTaskReview")
        }
        UiCommand::CloseTask { .. } => command_descriptor_by_serde_name("CloseTask"),
        UiCommand::ApplyExecutionFact { .. } => {
            command_descriptor_by_serde_name("ApplyExecutionFact")
        }
        UiCommand::RunSchedulerTick { .. } => command_descriptor_by_serde_name("RunSchedulerTick"),
        UiCommand::RunMasterPoll { .. } => command_descriptor_by_serde_name("RunMasterPoll"),
        UiCommand::QueryMasterPoll { .. } => command_descriptor_by_serde_name("QueryMasterPoll"),
        UiCommand::WorkerControl { .. } => command_descriptor_by_serde_name("WorkerControl"),
        UiCommand::QueryNodeStatus { .. } => command_descriptor_by_serde_name("QueryNodeStatus"),
        UiCommand::QueryTaskProgress { .. } => {
            command_descriptor_by_serde_name("QueryTaskProgress")
        }
        UiCommand::QueryDebugState { .. } => command_descriptor_by_serde_name("QueryDebugState"),
        UiCommand::QueryCheckpoints => command_descriptor_by_serde_name("QueryCheckpoints"),
        UiCommand::SendDirectMessageToSlave { .. } => {
            command_descriptor_by_serde_name("SendDirectMessageToSlave")
        }
        UiCommand::RewindCheckpoint { .. } => command_descriptor_by_serde_name("RewindCheckpoint"),
        UiCommand::CancelTurn { .. } => command_descriptor_by_serde_name("CancelTurn"),
        UiCommand::CancelLatestActiveTurn { .. } => {
            command_descriptor_by_serde_name("CancelLatestActiveTurn")
        }
        UiCommand::ResumeTurn { .. } => command_descriptor_by_serde_name("ResumeTurn"),
        UiCommand::CompactSessionContext { .. } => {
            command_descriptor_by_serde_name("CompactSessionContext")
        }
    }
}

pub(crate) fn command_kind(command: &UiCommand) -> &'static str {
    command_descriptor(command).semantic_kind
}

pub(crate) fn is_command_ingress_kind(command: &UiCommand) -> bool {
    matches!(command_frame_class(command), UiCommandFrameClass::Mutation)
}

/// Frame-level read/write classification for every `UiCommand` variant.
///
/// Exhaustive on purpose: adding a variant without declaring its class is a
/// compile error, so the query-route mutation guard can never silently miss
/// a new command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiCommandFrameClass {
    Query,
    Subscribe,
    Mutation,
}

pub fn command_frame_class(command: &UiCommand) -> UiCommandFrameClass {
    command_descriptor(command).frame_class
}

pub(crate) fn is_public_adp_command_descriptor(descriptor: &UiCommandDescriptor) -> bool {
    matches!(descriptor.exposure, UiAdpCommandExposure::Public)
}

pub fn is_public_adp_command(command: &UiCommand) -> bool {
    is_public_adp_command_descriptor(command_descriptor(command))
}

pub fn is_internal_adp_command(command: &UiCommand) -> bool {
    matches!(
        command_descriptor(command).exposure,
        UiAdpCommandExposure::Internal
    )
}

/// Gate for the ADP query route: rejects any command whose frame class is not
/// `Query` with `direct_task_mutation_forbidden`, so mutations cannot ride the
/// query channel regardless of what downstream query ports accept.
pub fn accept_query_ingress(command: &UiCommand) -> Result<(), UiProtocolError> {
    validate_command(command)?;
    if command_frame_class(command) != UiCommandFrameClass::Query {
        return Err(UiProtocolError::QueryCommandKindMismatch);
    }
    Ok(())
}

pub(crate) fn command_dispatch_target(command: &UiCommand) -> (&'static str, &'static str) {
    let descriptor = command_descriptor(command);
    (
        descriptor.target_owner_feature,
        descriptor.target_owner_module,
    )
}

pub fn turn_projection_from_events(input: TurnProjectionInput) -> UiTurnProjection {
    let mut reasoning = Vec::new();
    let mut text = Vec::new();
    for event in &input.semantic_events {
        match event.kind {
            SemanticEventKind::Reasoning => reasoning.push(event.content.clone()),
            SemanticEventKind::Text => text.push(event.content.clone()),
            _ => {}
        }
    }
    let mut tool_activities = tool_activities_from_input(&input.tool_calls, &input.tool_results);
    if matches!(
        input.terminal_event.as_ref().map(|event| &event.status),
        Some(TerminalStatus::Failed)
    ) {
        let detail = input
            .terminal_event
            .as_ref()
            .map(|event| event.summary.clone())
            .or_else(|| {
                input
                    .error_events
                    .first()
                    .map(|event| event.error.message.clone())
            });
        fail_waiting_tool_activities(&mut tool_activities, detail);
    }
    UiTurnProjection {
        source: UiSource {
            source_agent_id: input.source_agent_id,
            source_node_id: input.source_node_id,
            source_turn_id: Some(input.turn_id.clone()),
            stream_kind: UiStreamKind::Turn,
        },
        session_id: input.session_id,
        turn_id: input.turn_id,
        created_at: input.created_at,
        timing: input.timing,
        cwd: input.cwd,
        user_text: input.user_text,
        attachments: Vec::new(),
        model_request: None,
        reasoning,
        text,
        tool_calls: input
            .tool_calls
            .iter()
            .map(|call| call.tool_call.tool_name.clone())
            .collect(),
        tool_activities,
        usage: input
            .usage_events
            .iter()
            .map(|usage| {
                format!(
                    "input={} output={} cache_create={} cache_read={} reasoning={}",
                    usage.usage.total_input_tokens(),
                    usage.usage.output_tokens,
                    usage.usage.cache_creation_tokens,
                    usage.usage.cache_read_tokens,
                    usage.usage.reasoning_tokens.unwrap_or(0)
                )
            })
            .collect(),
        usage_projection: input.usage_events.last().map(|usage| UiUsageProjection {
            input_tokens: usage.usage.total_input_tokens(),
            output_tokens: usage.usage.output_tokens,
            total_tokens: usage.usage.resolved_total_tokens(),
            reasoning_tokens: usage.usage.reasoning_tokens,
            cache_creation_tokens: usage.usage.cache_creation_tokens,
            cache_read_tokens: usage.usage.cache_read_tokens,
            cache_hit_rate_bps: (usage.usage.cache_hit_rate() * 10000.0).round() as u64,
            context_tokens: usage.usage.total_input_tokens(),
            compacted_tokens: 0,
            model_label: None,
        }),
        terminal_status: input
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        terminal_text: {
            let text_chunks: Vec<String> = input
                .semantic_events
                .iter()
                .filter_map(|event| {
                    if event.kind == SemanticEventKind::Text {
                        Some(event.content.clone())
                    } else {
                        None
                    }
                })
                .collect();
            input
                .terminal_event
                .as_ref()
                .map(|event| human_friendly_terminal_text(&text_chunks, event))
        },
        user_options: input
            .terminal_event
            .as_ref()
            .and_then(|event| event.user_options.clone())
            .filter(|opts| !opts.is_empty()),
        errors: input
            .error_events
            .iter()
            .map(|error| error.error.message.clone())
            .collect(),
        search_evidence: None,
        slave_substream_card: input.slave_substream_card,
    }
}

pub fn turn_projection_for_client(
    projection: UiTurnProjection,
    client: UiClientKind,
) -> UiTurnProjection {
    if client == UiClientKind::Cli && projection.slave_substream_card {
        UiTurnProjection {
            slave_substream_card: false,
            ..projection
        }
    } else {
        projection
    }
}
