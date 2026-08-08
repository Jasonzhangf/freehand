//! UI-facing commands, events, and projections for Freehand.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, TryRecvError};

use freehand_blocks::{
    ToolDisplayOutcome, ToolDisplayProjection, project_tool_call_display,
    project_tool_result_display, strip_completion_submission_block,
};
use freehand_contracts::{
    AgentId, ErrorErr01RuntimeClassified, ReasonReq04ToolCall, ReasonReq05ToolResultReentry,
    ReasonResp01SemanticEvent, ReasonResp02UsageEvent, ReasonResp03TerminalEvent,
    SemanticEventKind, SessionId, TerminalStatus, ToolResultContract, ToolResultStatus, TurnId,
};
use freehand_control::strip_control_status_block;
pub use freehand_debug::{
    DebugEvent, DebugScenePosition, DebugSemanticPosition, DebugStateSnapshot, DebugTraceEnvelope,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use tokio::sync::broadcast;

mod dto;
pub use dto::*;

mod adp_wire;
pub use adp_wire::*;

pub trait UiCommandDispatchPort: Send + Sync {
    fn dispatch(
        &self,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError>;
}

pub trait UiRuntimeQueryPort: Send + Sync {
    fn query_runtime(
        &self,
        command: &UiCommand,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError>;

    fn query_runtime_with_scope(
        &self,
        command: &UiCommand,
        _scope: UiQueryAccessScope,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError> {
        self.query_runtime(command)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiQueryAccessScope {
    LocalLoopback,
    Remote,
}

pub struct UiProtocolOnlyQueryPort;

impl UiRuntimeQueryPort for UiProtocolOnlyQueryPort {
    fn query_runtime(
        &self,
        _command: &UiCommand,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError> {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct StaticUiCommandDispatchPort {
    dispatch_status: String,
}

impl Default for StaticUiCommandDispatchPort {
    fn default() -> Self {
        Self {
            dispatch_status: "queued_by_static_dispatch_port".to_owned(),
        }
    }
}

impl StaticUiCommandDispatchPort {
    pub fn new(dispatch_status: impl Into<String>) -> Self {
        Self {
            dispatch_status: dispatch_status.into(),
        }
    }
}

impl UiCommandDispatchPort for StaticUiCommandDispatchPort {
    fn dispatch(
        &self,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            dispatch_status: self.dispatch_status.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionSelector {
    pub client: UiClientKind,
    pub stream_kind: UiStreamKind,
    pub target_turn_id: Option<TurnId>,
}

#[derive(Debug)]
pub struct UiProtocolState {
    latest_active_turn_id: Option<TurnId>,
    turns: BTreeMap<TurnId, UiTurnProjection>,
    session_cwds: BTreeMap<SessionId, String>,
    session_metadata: BTreeMap<SessionId, UiSessionMetadataProjection>,
    node_status: BTreeMap<String, NodeStatusSnapshot>,
    progress: BTreeMap<TurnId, TaskProgressSnapshot>,
    debug: BTreeMap<TurnId, DebugStateSnapshot>,
    checkpoints: Option<UiCheckpointSnapshot>,
    subscription_tx: broadcast::Sender<UiSubscriptionEvent>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UiProtocolError {
    #[error("session command requires non-empty session id")]
    EmptySessionId,
    #[error("session title must be non-empty when provided")]
    EmptySessionTitle,
    #[error("submit user input command requires non-empty text")]
    EmptyUserInput,
    #[error("input attachment requires non-empty id, name, media type, and base64 data")]
    InvalidInputAttachment,
    #[error("input attachment kind is not supported")]
    UnsupportedInputAttachmentKind,
    #[error("session cwd must be non-empty when provided")]
    EmptySessionCwd,
    #[error("direct slave message requires non-empty text")]
    EmptySlaveMessage,
    #[error("rewind checkpoint command requires non-empty checkpoint id")]
    EmptyCheckpointId,
    #[error("task query requires non-empty task id")]
    EmptyTaskId,
    #[error("task create requires non-empty title")]
    EmptyTaskTitle,
    #[error("task create requires non-empty content")]
    EmptyTaskContent,
    #[error("task create requires non-empty goal")]
    EmptyTaskGoal,
    #[error("task review requires non-empty summary")]
    EmptyTaskReviewSummary,
    #[error("task review rejection requires non-empty reason")]
    EmptyTaskReviewRejection,
    #[error("task claim requires non-empty execution id")]
    EmptyTaskExecutionId,
    #[error("worker-control command requires non-empty control id when provided")]
    EmptyWorkerControlId,
    #[error("worker-control command requires non-empty op")]
    EmptyWorkerControlOp,
    #[error("worker-control command has unknown op `{0}`")]
    UnknownWorkerControlOp(String),
    #[error("worker-control ask_at_safe_point requires non-empty question")]
    EmptyWorkerControlQuestion,
    #[error("worker-control add_constraint requires non-empty constraint")]
    EmptyWorkerControlConstraint,
    #[error("worker-control command requires non-empty note when provided")]
    EmptyWorkerControlNote,
    #[error("timer command requires non-empty timer id when provided")]
    EmptyTimerId,
    #[error("timer schedule requires non-empty mode")]
    EmptyTimerMode,
    #[error("timer schedule mode `{0}` is not supported")]
    UnknownTimerMode(String),
    #[error("timer schedule requires non-empty reason")]
    EmptyTimerReason,
    #[error("timer schedule requires non-empty prompt")]
    EmptyTimerPrompt,
    #[error("timer schedule requires a positive delay_seconds for relative mode")]
    MissingTimerDelay,
    #[error("timer schedule requires run_at_unix_seconds for absolute mode")]
    MissingTimerRunAt,
    #[error("timer recurring schedule requires a repeat rule")]
    MissingTimerRepeat,
    #[error("timer repeat rule is invalid: {0}")]
    InvalidTimerRepeat(String),
    #[error("event cursor must be non-empty when provided")]
    EmptyEventCursor,
    #[error("master poll replay_from_start cannot be combined with after_cursor")]
    ConflictingMasterPollCursorMode,
    #[error("task agent command requires non-empty agent id")]
    EmptyTaskAgentId,
    #[error("task agent command requires at least one capability")]
    EmptyTaskCapabilities,
    #[error("config update requires non-empty agent name")]
    EmptyConfigAgentName,
    #[error("config update requires non-empty provider id")]
    EmptyProviderId,
    #[error("config update requires non-empty provider type")]
    EmptyProviderType,
    #[error("config update requires non-empty provider protocol")]
    EmptyProviderProtocol,
    #[error("config update requires non-empty provider base URL")]
    EmptyProviderBaseUrl,
    #[error("config update requires non-empty default model")]
    EmptyProviderDefaultModel,
    #[error("config update requires non-empty API key environment variable name")]
    EmptyProviderApiKeyEnv,
    #[error("model group update requires non-empty group id")]
    EmptyModelGroupId,
    #[error("model group route requires non-empty provider id")]
    EmptyModelRouteProvider,
    #[error("model group route requires non-empty model")]
    EmptyModelRouteModel,
    #[error("model group load-balance route requires a positive weight")]
    EmptyModelRouteWeight,
    #[error("agent resource count must be between 1 and 5, received {resource_count}")]
    AgentResourceCountOutOfRange { resource_count: usize },
    #[error("command ingress route only accepts mutation-intent commands")]
    IngressCommandKindMismatch,
    #[error("query route only accepts read-only commands; mutations must use the command frame")]
    QueryCommandKindMismatch,
    #[error("stream kind mismatch for requested projection")]
    StreamKindMismatch,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UiCommandDispatchPortError {
    #[error("dispatch port failure: {0}")]
    DispatchFailed(String),
    #[error("dispatch target not found: {0}")]
    TargetNotFound(String),
    #[error("dispatch path unsupported: {0}")]
    Unsupported(String),
}

impl UiProtocolState {
    pub fn new() -> Self {
        let (subscription_tx, _subscription_rx) = broadcast::channel(256);
        Self {
            latest_active_turn_id: None,
            turns: BTreeMap::new(),
            session_cwds: BTreeMap::new(),
            session_metadata: BTreeMap::new(),
            node_status: BTreeMap::new(),
            progress: BTreeMap::new(),
            debug: BTreeMap::new(),
            checkpoints: None,
            subscription_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<UiSubscriptionEvent> {
        self.subscription_tx.subscribe()
    }

    pub fn publish_task_list_projection(&self, projection: UiTaskListProjection) {
        self.publish_projection(UiProjection::TaskList(projection));
    }

    pub fn publish_error_center_events_projection(
        &self,
        projection: UiErrorCenterEventListProjection,
    ) {
        self.publish_projection(UiProjection::ErrorCenterEvents(projection));
    }

    pub fn apply_turn_projection(&mut self, projection: UiTurnProjection) {
        self.latest_active_turn_id = Some(projection.turn_id.clone());
        if let Some(cwd) = projection.cwd.clone() {
            self.session_cwds.insert(projection.session_id.clone(), cwd);
        }
        self.turns
            .insert(projection.turn_id.clone(), projection.clone());
        self.publish_projection(UiProjection::Turn(projection));
    }

    pub fn replace_session_turn_projections(
        &mut self,
        session_id: &SessionId,
        projections: impl IntoIterator<Item = UiTurnProjection>,
    ) {
        let projections = projections.into_iter().collect::<Vec<_>>();
        let live_preserve_turn_id = projections
            .last()
            .filter(|projection| turn_is_nonterminal(projection))
            .map(|projection| projection.turn_id.clone());
        let existing = self
            .turns
            .values()
            .filter(|projection| &projection.session_id == session_id)
            .map(|projection| (projection.turn_id.clone(), projection.clone()))
            .collect::<BTreeMap<_, _>>();
        self.turns
            .retain(|_, projection| &projection.session_id != session_id);
        let mut latest_session_turn_id = None;
        for mut projection in projections {
            if live_preserve_turn_id.as_ref() == Some(&projection.turn_id)
                && let Some(previous) = existing.get(&projection.turn_id)
            {
                preserve_live_activity_on_nonterminal_refresh(&mut projection, previous);
            }
            latest_session_turn_id = Some(projection.turn_id.clone());
            if let Some(cwd) = projection.cwd.clone() {
                self.session_cwds.insert(projection.session_id.clone(), cwd);
            }
            self.turns
                .insert(projection.turn_id.clone(), projection.clone());
            self.publish_projection(UiProjection::Turn(projection));
        }
        if self
            .latest_active_turn_id
            .as_ref()
            .is_some_and(|turn_id| !self.turns.contains_key(turn_id))
        {
            self.latest_active_turn_id = latest_session_turn_id.or_else(|| {
                self.turns
                    .values()
                    .last()
                    .map(|projection| projection.turn_id.clone())
            });
        }
    }

    pub fn merge_persisted_turn_projections_without_publish(
        &mut self,
        projections: impl IntoIterator<Item = UiTurnProjection>,
    ) {
        for projection in projections {
            let should_replace =
                !self.turns.contains_key(&projection.turn_id) || turn_is_terminal(&projection);
            if should_replace {
                self.turns.insert(projection.turn_id.clone(), projection);
            }
        }
    }

    pub fn replace_persisted_turn_projections_without_publish(
        &mut self,
        session_id: &SessionId,
        projections: impl IntoIterator<Item = UiTurnProjection>,
    ) {
        let projections = projections.into_iter().collect::<Vec<_>>();
        let persisted_ids = projections
            .iter()
            .map(|projection| projection.turn_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        self.turns.retain(|turn_id, projection| {
            projection.session_id != *session_id
                || persisted_ids.contains(turn_id)
                || !turn_is_terminal(projection)
        });
        self.merge_persisted_turn_projections_without_publish(projections);
        if self
            .latest_active_turn_id
            .as_ref()
            .is_some_and(|turn_id| !self.turns.contains_key(turn_id))
        {
            self.latest_active_turn_id = self
                .turns
                .values()
                .rfind(|projection| !turn_is_terminal(projection))
                .map(|projection| projection.turn_id.clone());
        }
    }

    pub fn set_session_cwd(&mut self, session_id: SessionId, cwd: impl Into<String>) {
        let cwd = cwd.into();
        self.session_cwds.insert(session_id.clone(), cwd.clone());
        self.session_metadata
            .entry(session_id.clone())
            .and_modify(|metadata| metadata.cwd = Some(cwd.clone()))
            .or_insert_with(|| UiSessionMetadataProjection {
                session_id: session_id.clone(),
                title: None,
                archived: false,
                cwd: Some(cwd.clone()),
            });
        for projection in self.turns.values_mut() {
            if projection.session_id == session_id {
                projection.cwd = Some(cwd.clone());
            }
        }
    }

    pub fn set_session_metadata(&mut self, metadata: UiSessionMetadataProjection) {
        if let Some(cwd) = metadata.cwd.clone() {
            self.session_cwds
                .insert(metadata.session_id.clone(), cwd.clone());
            for projection in self.turns.values_mut() {
                if projection.session_id == metadata.session_id && projection.cwd.is_none() {
                    projection.cwd = Some(cwd.clone());
                }
            }
        }
        self.session_metadata
            .insert(metadata.session_id.clone(), metadata);
    }

    pub fn set_session_metadata_entries(
        &mut self,
        entries: impl IntoIterator<Item = UiSessionMetadataProjection>,
    ) {
        for entry in entries {
            self.set_session_metadata(entry);
        }
    }

    pub fn apply_semantic_event(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonResp01SemanticEvent,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &event.session_id,
                &event.turn_id,
                slave_substream_card,
            );
            match event.kind {
                SemanticEventKind::Reasoning => projection.reasoning.push(event.content.clone()),
                SemanticEventKind::Text => projection.text.push(event.content.clone()),
                _ => {}
            }
            projection.model_request = None;
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_tool_call(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonReq04ToolCall,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &event.session_id,
                &event.turn_id,
                slave_substream_card,
            );
            projection
                .tool_calls
                .push(event.tool_call.tool_name.clone());
            upsert_tool_activity(
                &mut projection.tool_activities,
                event.tool_call.tool_call_id.as_str().to_owned(),
                event.tool_call.tool_name.clone(),
                UiToolActivityStatus::Waiting,
                Some("waiting for tool execution".to_owned()),
                Some(project_tool_call_display(
                    &event.tool_call.tool_name,
                    &event.tool_call.arguments,
                )),
            );
            projection.model_request = None;
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_model_request_waiting(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        session_id: &SessionId,
        turn_id: &TurnId,
        detail: Option<String>,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        self.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id,
            source_node_id,
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail,
            transport: None,
            slave_substream_card,
        })
    }

    pub fn apply_model_request_waiting_kind(
        &mut self,
        waiting: UiModelRequestWaiting,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                waiting.source_agent_id,
                waiting.source_node_id,
                &waiting.session_id,
                &waiting.turn_id,
                waiting.slave_substream_card,
            );
            projection.model_request = Some(model_request_activity_from_waiting(
                waiting.kind,
                waiting.detail,
                waiting.transport,
            ));
            projection.clone()
        };
        self.latest_active_turn_id = Some(waiting.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_completion_schema_retry_waiting(
        &mut self,
        waiting: UiCompletionSchemaRetryWaiting,
    ) -> UiTurnProjection {
        let detail = format!(
            "schema polishing #{}: {}",
            waiting.retry_index, waiting.issue_summary
        );
        self.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: waiting.source_agent_id,
            source_node_id: waiting.source_node_id,
            session_id: waiting.session_id,
            turn_id: waiting.turn_id,
            kind: UiModelRequestKind::SchemaRetry,
            detail: Some(detail),
            transport: None,
            slave_substream_card: waiting.slave_substream_card,
        })
    }

    pub fn apply_usage_event(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonResp02UsageEvent,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &event.session_id,
                &event.turn_id,
                slave_substream_card,
            );
            projection.usage.push(format!(
                "input={} output={}",
                event.usage.input_tokens, event.usage.output_tokens
            ));
            projection.model_request = None;
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_tool_result(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonReq05ToolResultReentry,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &event.session_id,
                &event.turn_id,
                slave_substream_card,
            );
            let tool_call_id = event.tool_result.tool_call_id.as_str().to_owned();
            let tool_name = projection
                .tool_activities
                .iter()
                .find(|activity| activity.tool_call_id == tool_call_id)
                .map(|activity| activity.tool_name.clone())
                .unwrap_or_else(|| "tool".to_owned());
            let display = projection
                .tool_activities
                .iter()
                .find(|activity| activity.tool_call_id == tool_call_id)
                .and_then(|activity| activity.display.clone())
                .map(|display| project_tool_result_display(display, &event.tool_result));
            upsert_tool_activity(
                &mut projection.tool_activities,
                tool_call_id,
                tool_name,
                tool_activity_status_from_result(event.tool_result.status),
                Some(tool_activity_detail_from_result(&event.tool_result)),
                display,
            );
            projection.model_request = None;
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_terminal_event(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ReasonResp03TerminalEvent,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &event.session_id,
                &event.turn_id,
                slave_substream_card,
            );
            projection.terminal_status = Some(event.status.clone());
            projection.terminal_text = Some(terminal_text_projection(event));
            projection.model_request = None;
            if event.status == TerminalStatus::Failed {
                fail_waiting_tool_activities(
                    &mut projection.tool_activities,
                    Some(event.summary.clone()),
                );
            }
            projection.clone()
        };
        self.latest_active_turn_id = Some(event.turn_id.clone());
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_error_event(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        event: &ErrorErr01RuntimeClassified,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let session_id = event
            .session_id
            .clone()
            .expect("ui turn error projection requires session_id");
        let turn_id = event
            .turn_id
            .clone()
            .expect("ui turn error projection requires turn_id");
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &session_id,
                &turn_id,
                slave_substream_card,
            );
            projection.errors.push(event.error.message.clone());
            projection.model_request = None;
            projection.clone()
        };
        self.latest_active_turn_id = Some(turn_id);
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn set_node_status(&mut self, snapshot: NodeStatusSnapshot) {
        self.node_status
            .insert(snapshot.node_id.clone(), snapshot.clone());
        self.publish_projection(UiProjection::NodeStatus(snapshot));
    }

    pub fn set_progress(&mut self, snapshot: TaskProgressSnapshot) {
        self.progress
            .insert(snapshot.turn_id.clone(), snapshot.clone());
        self.publish_projection(UiProjection::Progress(snapshot));
    }

    pub fn set_debug_state(&mut self, snapshot: DebugStateSnapshot) {
        self.debug
            .insert(snapshot.semantic.turn_id.clone(), snapshot.clone());
        self.publish_projection(UiProjection::Debug(snapshot));
    }

    pub fn set_checkpoint_snapshot(&mut self, snapshot: UiCheckpointSnapshot) {
        self.checkpoints = Some(snapshot.clone());
        self.publish_projection(UiProjection::Checkpoints(snapshot));
    }

    pub fn apply_debug_event(&mut self, event: &DebugEvent) -> bool {
        let Some(snapshot) = event.snapshot.clone() else {
            return false;
        };
        self.set_debug_state(snapshot);
        true
    }

    pub fn drain_debug_receiver(&mut self, receiver: &Receiver<DebugEvent>) -> usize {
        let mut applied = 0;
        loop {
            match receiver.try_recv() {
                Ok(event) => {
                    if self.apply_debug_event(&event) {
                        applied += 1;
                    }
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return applied,
            }
        }
    }

    pub fn query(&self, command: &UiCommand) -> Result<UiQueryResult, UiProtocolError> {
        match command {
            UiCommand::QueryLatestActiveTurn => {
                let result = self
                    .latest_active_turn_id
                    .as_ref()
                    .and_then(|turn_id| self.turns.get(turn_id).cloned());
                Ok(UiQueryResult::Turn(result))
            }
            UiCommand::QueryTurn { turn_id } => {
                Ok(UiQueryResult::Turn(self.turns.get(turn_id).cloned()))
            }
            UiCommand::QuerySessionList => Ok(UiQueryResult::SessionList(session_list_projection(
                &self.turns,
                &self.session_cwds,
                &self.session_metadata,
                self.latest_active_turn_id.as_ref(),
                false,
            ))),
            UiCommand::QueryArchivedSessionList => {
                Ok(UiQueryResult::SessionList(session_list_projection(
                    &self.turns,
                    &self.session_cwds,
                    &self.session_metadata,
                    self.latest_active_turn_id.as_ref(),
                    true,
                )))
            }
            UiCommand::QuerySessionTurns { session_id } => {
                Ok(UiQueryResult::SessionTurns(session_transcript_projection(
                    session_id,
                    &self.turns,
                    &self.session_cwds,
                    &self.session_metadata,
                )))
            }
            UiCommand::QuerySessionSearch { .. }
            | UiCommand::QueryTaskList { .. }
            | UiCommand::QueryTaskBoard { .. }
            | UiCommand::QueryEventInbox { .. }
            | UiCommand::QueryAgentBoard
            | UiCommand::QueryAgentLifecycle { .. }
            | UiCommand::QueryTaskHistory { .. }
            | UiCommand::QueryWorkerControl { .. }
            | UiCommand::QueryTimerList { .. }
            | UiCommand::QueryToolRegistry
            | UiCommand::QueryDiagnostics
            | UiCommand::RunMasterPoll { .. }
            | UiCommand::QueryMasterPoll { .. }
            | UiCommand::WorkerControl { .. }
            | UiCommand::TestProviderWebSearch { .. }
            | UiCommand::ScheduleTimer { .. }
            | UiCommand::CancelTimer { .. }
            | UiCommand::QueryConfigStatus
            | UiCommand::QueryErrorCenterEvents { .. } => Err(UiProtocolError::StreamKindMismatch),
            UiCommand::QueryNodeStatus { node_id } => Ok(UiQueryResult::NodeStatus(
                self.node_status.get(node_id).cloned(),
            )),
            UiCommand::QueryTaskProgress { turn_id } => {
                Ok(UiQueryResult::Progress(self.progress.get(turn_id).cloned()))
            }
            UiCommand::QueryDebugState { turn_id } => {
                Ok(UiQueryResult::Debug(self.debug.get(turn_id).cloned()))
            }
            UiCommand::QueryCheckpoints => Ok(UiQueryResult::Checkpoints(
                self.checkpoints
                    .clone()
                    .unwrap_or_else(empty_checkpoint_snapshot),
            )),
            _ => Err(UiProtocolError::StreamKindMismatch),
        }
    }

    fn ensure_turn_projection(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        session_id: &SessionId,
        turn_id: &TurnId,
        slave_substream_card: bool,
    ) -> &mut UiTurnProjection {
        self.turns
            .entry(turn_id.clone())
            .or_insert_with(|| UiTurnProjection {
                source: UiSource {
                    source_agent_id,
                    source_node_id,
                    source_turn_id: Some(turn_id.clone()),
                    stream_kind: UiStreamKind::Turn,
                },
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                created_at: None,
                timing: None,
                cwd: self.session_cwds.get(session_id).cloned(),
                user_text: None,
                attachments: Vec::new(),
                model_request: None,
                reasoning: Vec::new(),
                text: Vec::new(),
                tool_calls: Vec::new(),
                tool_activities: Vec::new(),
                usage: Vec::new(),
                terminal_status: None,
                terminal_text: None,
                errors: Vec::new(),
                slave_substream_card,
            })
    }

    fn publish_projection(&self, projection: UiProjection) {
        let _ = self.subscription_tx.send(UiSubscriptionEvent {
            projection,
            latest_active_turn_id: self.latest_active_turn_id.clone(),
        });
    }
}

fn model_request_activity_from_waiting(
    kind: UiModelRequestKind,
    detail: Option<String>,
    transport: Option<UiModelTransportActivity>,
) -> UiModelRequestActivity {
    UiModelRequestActivity {
        status: UiModelRequestStatus::Waiting,
        kind,
        detail,
        transport,
    }
}

impl Default for UiProtocolState {
    fn default() -> Self {
        Self::new()
    }
}

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
        UiProtocolError::EmptySessionTitle => "empty_session_title",
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
        UiProtocolError::EmptyModelRouteProvider => "empty_model_route_provider",
        UiProtocolError::EmptyModelRouteModel => "empty_model_route_model",
        UiProtocolError::EmptyModelRouteWeight => "empty_model_route_weight",
        UiProtocolError::AgentResourceCountOutOfRange { .. } => "agent_resource_count_out_of_range",
        UiProtocolError::IngressCommandKindMismatch => "ingress_command_kind_mismatch",
        UiProtocolError::QueryCommandKindMismatch => "direct_task_mutation_forbidden",
        UiProtocolError::StreamKindMismatch => "stream_kind_mismatch",
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

pub fn terminal_text_projection(event: &ReasonResp03TerminalEvent) -> String {
    event.summary.clone()
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

fn tool_public_body(activity: &UiToolActivity) -> String {
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

fn tool_public_body_with_detail(
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

fn tool_status_fallback_body(status: UiToolActivityStatus) -> &'static str {
    match status {
        UiToolActivityStatus::Waiting => "waiting",
        UiToolActivityStatus::Completed => "completed",
        UiToolActivityStatus::Failed => "failed",
    }
}

fn tool_display_public_body(display: &ToolDisplayProjection) -> Option<String> {
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

fn empty_checkpoint_snapshot() -> UiCheckpointSnapshot {
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

fn session_list_projection(
    turns: &BTreeMap<TurnId, UiTurnProjection>,
    session_cwds: &BTreeMap<SessionId, String>,
    session_metadata: &BTreeMap<SessionId, UiSessionMetadataProjection>,
    latest_active_turn_id: Option<&TurnId>,
    archived: bool,
) -> UiSessionListProjection {
    let mut sessions = session_metadata
        .values()
        .filter(|metadata| {
            metadata.archived == archived && user_visible_session_id(&metadata.session_id)
        })
        .map(|metadata| {
            let mut session_turns = turns
                .values()
                .filter(|turn| turn.session_id == metadata.session_id)
                .collect::<Vec<_>>();
            session_turns.sort_by(|left, right| {
                turn_order_key(&left.turn_id).cmp(&turn_order_key(&right.turn_id))
            });
            let latest = session_turns.last().copied();
            let active_turn_id = latest_active_turn_id.and_then(|turn_id| {
                session_turns
                    .iter()
                    .find(|turn| {
                        &turn.turn_id == turn_id
                            && latest.is_some_and(|latest| latest.turn_id == turn.turn_id)
                            && turn_is_nonterminal(turn)
                    })
                    .map(|_| turn_id.clone())
            });
            let cwd = session_cwds
                .get(&metadata.session_id)
                .cloned()
                .or_else(|| metadata.cwd.clone())
                .or_else(|| latest.and_then(|turn| turn.cwd.clone()));
            UiSessionSummary {
                session_id: metadata.session_id.clone(),
                title: metadata.title.clone(),
                archived: metadata.archived,
                cwd,
                latest_turn_id: latest.map(|turn| turn.turn_id.clone()),
                active_turn_id,
                turn_count: session_turns.len(),
                latest_status: latest
                    .map(session_latest_status)
                    .unwrap_or_else(|| "empty".to_owned()),
                latest_summary: latest.and_then(session_latest_summary),
            }
        })
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| left.session_id.as_str().cmp(right.session_id.as_str()));
    UiSessionListProjection { sessions }
}

fn user_visible_session_id(session_id: &SessionId) -> bool {
    let value = session_id.as_str();
    !value.starts_with("master-lifecycle-")
        && !value.starts_with("master-timer-")
        && !value.starts_with("worker-task-")
}

fn session_transcript_projection(
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

fn session_latest_status(turn: &UiTurnProjection) -> String {
    if let Some(status) = &turn.terminal_status {
        return format!("{status:?}").to_lowercase();
    }
    if turn
        .tool_activities
        .iter()
        .any(|activity| activity.status == UiToolActivityStatus::Waiting)
    {
        return "tool_running".to_owned();
    }
    if turn.model_request.is_some() {
        return "waiting_model".to_owned();
    }
    if !turn.text.is_empty() || !turn.reasoning.is_empty() {
        return "active".to_owned();
    }
    "submitted".to_owned()
}

fn turn_is_nonterminal(turn: &UiTurnProjection) -> bool {
    turn.terminal_status.is_none() && turn.terminal_text.is_none()
}

fn turn_is_terminal(turn: &UiTurnProjection) -> bool {
    !turn_is_nonterminal(turn)
}

fn session_latest_summary(turn: &UiTurnProjection) -> Option<String> {
    turn.terminal_text
        .clone()
        .or_else(|| turn.text.last().cloned())
        .or_else(|| turn.user_text.clone())
}

fn turn_order_key(turn_id: &TurnId) -> (String, u64, u64, String) {
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

fn tool_activities_from_input(
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

fn upsert_tool_activity(
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

fn preserve_live_activity_on_nonterminal_refresh(
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

fn merge_tool_activity(activities: &mut Vec<UiToolActivity>, incoming: &UiToolActivity) {
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

fn tool_activity_rank(status: UiToolActivityStatus) -> u8 {
    match status {
        UiToolActivityStatus::Waiting => 0,
        UiToolActivityStatus::Completed => 1,
        UiToolActivityStatus::Failed => 2,
    }
}

fn tool_activity_status_from_result(status: ToolResultStatus) -> UiToolActivityStatus {
    match status {
        ToolResultStatus::Success => UiToolActivityStatus::Completed,
        ToolResultStatus::Failed => UiToolActivityStatus::Failed,
    }
}

fn tool_activity_detail_from_result(result: &ToolResultContract) -> String {
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

fn fail_waiting_tool_activities(activities: &mut [UiToolActivity], detail: Option<String>) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UiCommandDescriptor {
    serde_name: &'static str,
    semantic_kind: &'static str,
    frame_class: UiCommandFrameClass,
    target_owner_feature: &'static str,
    target_owner_module: &'static str,
    exposure: UiAdpCommandExposure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiAdpCommandExposure {
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
        serde_name: "QuerySessionList",
        semantic_kind: "query_session_list",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
        exposure: UiAdpCommandExposure::Public,
    },
    UiCommandDescriptor {
        serde_name: "QueryArchivedSessionList",
        semantic_kind: "query_archived_session_list",
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
        serde_name: "QuerySessionSearch",
        semantic_kind: "query_session_search",
        frame_class: UiCommandFrameClass::Query,
        target_owner_feature: "ui.protocol",
        target_owner_module: "crates/freehand-ui-protocol",
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
];

fn command_descriptor_by_serde_name(serde_name: &str) -> &'static UiCommandDescriptor {
    UI_COMMAND_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.serde_name == serde_name)
        .expect("UiCommand descriptor table must contain every UiCommand variant")
}

fn command_descriptor(command: &UiCommand) -> &'static UiCommandDescriptor {
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
        UiCommand::QuerySessionList => command_descriptor_by_serde_name("QuerySessionList"),
        UiCommand::QueryArchivedSessionList => {
            command_descriptor_by_serde_name("QueryArchivedSessionList")
        }
        UiCommand::QuerySessionTurns { .. } => {
            command_descriptor_by_serde_name("QuerySessionTurns")
        }
        UiCommand::QuerySessionSearch { .. } => {
            command_descriptor_by_serde_name("QuerySessionSearch")
        }
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
    }
}

fn command_kind(command: &UiCommand) -> &'static str {
    command_descriptor(command).semantic_kind
}

fn is_command_ingress_kind(command: &UiCommand) -> bool {
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

fn command_dispatch_target(command: &UiCommand) -> (&'static str, &'static str) {
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
                    "input={} output={}",
                    usage.usage.input_tokens, usage.usage.output_tokens
                )
            })
            .collect(),
        terminal_status: input
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        terminal_text: input.terminal_event.as_ref().map(terminal_text_projection),
        errors: input
            .error_events
            .iter()
            .map(|error| error.error.message.clone())
            .collect(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        ErrorClass, ErrorContract, FeatureId, RecoveryPolicy, TerminalStatus, TraceId,
    };
    use freehand_debug::DebugHub;

    fn base_source(stream_kind: UiStreamKind) -> UiSource {
        UiSource {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            source_turn_id: Some(TurnId::new("turn-1")),
            stream_kind,
        }
    }

    fn sample_turn_projection(slave_substream_card: bool) -> UiTurnProjection {
        turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            created_at: Some(10),
            timing: Some(UiTurnTimingProjection {
                turn_started_at_ms: Some(10_000),
                first_response_at_ms: Some(11_250),
                completed_at_ms: Some(12_500),
                time_to_first_response_ms: Some(1_250),
                total_elapsed_ms: Some(2_500),
            }),
            cwd: None,
            user_text: Some("run the task".to_owned()),
            semantic_events: vec![
                ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    trace_id: TraceId::new("trace-1"),
                    feature_id: FeatureId::new("ui.protocol"),
                    agent_id: AgentId::new("agent-1"),
                    kind: SemanticEventKind::Reasoning,
                    content: "thinking".to_owned(),
                },
                ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    trace_id: TraceId::new("trace-1"),
                    feature_id: FeatureId::new("ui.protocol"),
                    agent_id: AgentId::new("agent-1"),
                    kind: SemanticEventKind::Text,
                    content: "answer".to_owned(),
                },
            ],
            tool_calls: vec![ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_call: freehand_contracts::ToolCallContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    tool_name: "search".to_owned(),
                    arguments: vec![],
                    arguments_complete: true,
                },
            }],
            tool_results: Vec::new(),
            usage_events: vec![ReasonResp02UsageEvent {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                usage: freehand_contracts::TokenUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: Some(15),
                    reasoning_tokens: Some(3),
                    cache_creation_tokens: 0,
                    cache_read_tokens: 0,
                    finish_reason: Some("stop".to_owned()),
                },
            }],
            terminal_event: Some(ReasonResp03TerminalEvent {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                status: TerminalStatus::Success,
                summary: "final text".to_owned(),
            }),
            error_events: vec![ErrorErr01RuntimeClassified {
                session_id: Some(SessionId::new("session-1")),
                turn_id: Some(TurnId::new("turn-1")),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: Some(AgentId::new("agent-1")),
                error: ErrorContract {
                    code: "warn".to_owned(),
                    class: ErrorClass::Protocol,
                    recovery: RecoveryPolicy::Recoverable,
                    message: "minor".to_owned(),
                },
            }],
            slave_substream_card,
        })
    }

    fn active_refresh_projection(session_id: &SessionId, turn_id: &TurnId) -> UiTurnProjection {
        turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            created_at: Some(90),
            timing: None,
            cwd: None,
            user_text: Some("run active work".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        })
    }

    fn terminal_refresh_projection(
        session_id: &SessionId,
        turn_id: &TurnId,
        status: TerminalStatus,
    ) -> UiTurnProjection {
        turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            created_at: Some(91),
            timing: None,
            cwd: None,
            user_text: Some("run active work".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: Some(ReasonResp03TerminalEvent {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: TraceId::new("trace-refresh-terminal"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                status,
                summary: "terminal refresh truth".to_owned(),
            }),
            error_events: Vec::new(),
            slave_substream_card: false,
        })
    }

    fn ui_tool_call(session_id: &SessionId, turn_id: &TurnId) -> ReasonReq04ToolCall {
        ReasonReq04ToolCall {
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            trace_id: TraceId::new("trace-refresh-tool"),
            feature_id: FeatureId::new("ui.protocol"),
            agent_id: AgentId::new("agent-1"),
            tool_call: freehand_contracts::ToolCallContract {
                tool_call_id: freehand_contracts::ToolCallId::new("tool-refresh-1"),
                tool_name: "task".to_owned(),
                arguments: vec![],
                arguments_complete: true,
            },
        }
    }

    fn sample_debug_snapshot() -> DebugStateSnapshot {
        DebugStateSnapshot::new(
            freehand_debug::DebugSemanticPosition {
                feature_id: FeatureId::new("ui.protocol"),
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                agent_id: Some(AgentId::new("agent-1")),
                pipeline_node: Some("UiDebugState".to_owned()),
            },
            freehand_debug::DebugScenePosition {
                crate_name: "freehand-ui-protocol".to_owned(),
                file: "src/lib.rs".to_owned(),
                function: "sample_debug_snapshot".to_owned(),
                line: None,
                artifact_path: None,
                raw_exchange_id: None,
            },
            "planner locked stable prefix",
            vec![
                "rewrite_mode=ordinary".to_owned(),
                "rewrite_version=0".to_owned(),
            ],
        )
    }

    #[test]
    fn command_to_projection_smoke() {
        validate_command(&UiCommand::SubmitUserInput {
            text: "hello".to_owned(),
            session_id: None,
            cwd: None,
            metadata: None,
        })
        .expect("valid");

        let projection = sample_turn_projection(false);
        assert_eq!(projection.created_at, Some(10));
        assert_eq!(projection.reasoning, vec!["thinking"]);
        assert_eq!(projection.text, vec!["answer"]);
        assert_eq!(projection.tool_activities.len(), 1);
        assert_eq!(
            projection.tool_activities[0].status,
            UiToolActivityStatus::Waiting
        );
    }

    #[test]
    fn submit_user_input_accepts_optional_session_id() {
        let command = UiCommand::SubmitUserInput {
            text: "hello new session".to_owned(),
            session_id: Some(SessionId::new("webui-session-test")),
            cwd: None,
            metadata: None,
        };
        validate_command(&command).expect("valid command");
        let encoded = serde_json::to_string(&command).expect("json");
        assert!(encoded.contains("webui-session-test"));
        let decoded: UiCommand = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, command);
    }

    #[test]
    fn submit_user_input_carries_session_cwd_and_rejects_empty_cwd() {
        let command = UiCommand::SubmitUserInput {
            text: "hello cwd session".to_owned(),
            session_id: Some(SessionId::new("webui-session-cwd")),
            cwd: Some("/tmp/freehand-cwd".to_owned()),
            metadata: None,
        };
        validate_command(&command).expect("valid cwd command");
        let encoded = serde_json::to_string(&command).expect("json");
        assert!(encoded.contains("/tmp/freehand-cwd"));
        let decoded: UiCommand = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, command);

        let err = validate_command(&UiCommand::SubmitUserInput {
            text: "bad cwd".to_owned(),
            session_id: None,
            cwd: Some("   ".to_owned()),
            metadata: None,
        })
        .expect_err("blank cwd must be rejected");
        assert_eq!(err, UiProtocolError::EmptySessionCwd);
        assert_eq!(protocol_rejection(err).code, "empty_session_cwd");
    }

    #[test]
    fn submit_user_input_accepts_image_only_metadata() {
        let command = UiCommand::SubmitUserInput {
            text: "   ".to_owned(),
            session_id: Some(SessionId::new("webui-session-image")),
            cwd: None,
            metadata: Some(UiSubmitMetadata {
                attachments: vec![UiInputAttachment {
                    attachment_id: "att-1".to_owned(),
                    kind: UiInputAttachmentKind::Image,
                    media_type: "image/png".to_owned(),
                    name: "screen.png".to_owned(),
                    size_bytes: Some(42),
                    data_base64: Some("aW1hZ2U=".to_owned()),
                }],
            }),
        };
        validate_command(&command).expect("image-only metadata is valid");

        let encoded = serde_json::to_string(&command).expect("json");
        assert!(encoded.contains("\"metadata\""));
        assert!(encoded.contains("\"data_base64\""));
        let decoded: UiCommand = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, command);
    }

    #[test]
    fn submit_user_input_rejects_metadata_image_without_payload() {
        let err = validate_command(&UiCommand::SubmitUserInput {
            text: "look".to_owned(),
            session_id: Some(SessionId::new("webui-session-image")),
            cwd: None,
            metadata: Some(UiSubmitMetadata {
                attachments: vec![UiInputAttachment {
                    attachment_id: "att-1".to_owned(),
                    kind: UiInputAttachmentKind::Image,
                    media_type: "image/png".to_owned(),
                    name: "screen.png".to_owned(),
                    size_bytes: Some(42),
                    data_base64: None,
                }],
            }),
        })
        .expect_err("image payload must be present on submit");
        assert_eq!(err, UiProtocolError::InvalidInputAttachment);
        assert_eq!(protocol_rejection(err).code, "invalid_input_attachment");
    }

    #[test]
    fn create_session_rejects_empty_cwd() {
        let command = UiCommand::CreateSession {
            session_id: SessionId::new("webui-task-session"),
            title: Some("Task".to_owned()),
            cwd: Some("/tmp/freehand-cwd".to_owned()),
        };
        validate_command(&command).expect("valid task cwd command");
        let encoded = serde_json::to_string(&command).expect("json");
        assert!(encoded.contains("/tmp/freehand-cwd"));

        let err = validate_command(&UiCommand::CreateSession {
            session_id: SessionId::new("webui-task-empty-cwd"),
            title: Some("Task".to_owned()),
            cwd: Some("   ".to_owned()),
        })
        .expect_err("blank task cwd must be rejected");
        assert_eq!(err, UiProtocolError::EmptySessionCwd);
        assert_eq!(protocol_rejection(err).code, "empty_session_cwd");
    }

    #[test]
    fn phase2a_task_commands_validate_and_route_to_task_orchestration() {
        let commands = vec![
            UiCommand::CreateTaskAgent {
                agent: UiTaskAgentCreateCommand {
                    agent_id: AgentId::new("worker-phase2a"),
                    capabilities: vec!["code_edit".to_owned()],
                },
            },
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some("task-phase2a".to_owned()),
                    title: "Phase2A".to_owned(),
                    content: "Phase2A content".to_owned(),
                    goal: "prove worker loop".to_owned(),
                    deliverables: vec!["loop".to_owned()],
                    acceptance: vec!["closed".to_owned()],
                    priority: 90,
                    target_cwd: None,
                    execution_profile: "workspace".to_owned(),
                    session_id: Some(SessionId::new("session-phase2a")),
                    turn_id: Some(TurnId::new("turn-phase2a")),
                    dispatch: Some(UiTaskDispatchCommand::None),
                },
            },
            UiCommand::AssignTask {
                assignment: UiTaskAssignCommand {
                    task_id: "task-phase2a".to_owned(),
                    agent_id: AgentId::new("worker-phase2a"),
                },
            },
            UiCommand::ClaimNextTask {
                claim: UiTaskClaimCommand {
                    agent_id: AgentId::new("worker-phase2a"),
                    execution_id: "exec-phase2a".to_owned(),
                    ttl_seconds: Some(300),
                },
            },
            UiCommand::RejectTaskReview {
                rejection: UiTaskReviewRejectionCommand {
                    task_id: "task-phase2a".to_owned(),
                    reject_reason: "missing evidence".to_owned(),
                    next_requirements: vec!["add evidence".to_owned()],
                },
            },
        ];
        for command in commands {
            validate_command(&command).expect("valid phase2a task command");
            let envelope =
                build_command_dispatch_envelope(&command).expect("task command envelope");
            assert_eq!(envelope.target_feature_id, "task.orchestration");
            assert_eq!(envelope.target_owner_module, "crates/freehand-task");
        }
    }

    #[test]
    fn phase2a_task_commands_reject_missing_worker_execution_and_review_fields() {
        let err = validate_command(&UiCommand::CreateTaskAgent {
            agent: UiTaskAgentCreateCommand {
                agent_id: AgentId::new("worker-empty-capabilities"),
                capabilities: Vec::new(),
            },
        })
        .expect_err("worker capabilities required");
        assert_eq!(err, UiProtocolError::EmptyTaskCapabilities);
        assert_eq!(protocol_rejection(err).code, "empty_task_capabilities");

        let err = validate_command(&UiCommand::ClaimNextTask {
            claim: UiTaskClaimCommand {
                agent_id: AgentId::new("worker-phase2a"),
                execution_id: " ".to_owned(),
                ttl_seconds: Some(300),
            },
        })
        .expect_err("execution id required");
        assert_eq!(err, UiProtocolError::EmptyTaskExecutionId);
        assert_eq!(protocol_rejection(err).code, "empty_task_execution_id");

        let err = validate_command(&UiCommand::RejectTaskReview {
            rejection: UiTaskReviewRejectionCommand {
                task_id: "task-phase2a".to_owned(),
                reject_reason: " ".to_owned(),
                next_requirements: vec!["retry".to_owned()],
            },
        })
        .expect_err("reject reason required");
        assert_eq!(err, UiProtocolError::EmptyTaskReviewRejection);
        assert_eq!(protocol_rejection(err).code, "empty_task_review_rejection");
    }

    #[test]
    fn phase2b_event_inbox_and_master_poll_validate_and_route_to_task_orchestration() {
        let query = UiCommand::QueryEventInbox {
            after_cursor: Some("cursor-1".to_owned()),
            limit: Some(20),
        };
        validate_command(&query).expect("valid event inbox query");
        assert_eq!(
            accept_command_ingress(&query).expect_err("query cannot enter command ingress"),
            UiProtocolError::IngressCommandKindMismatch
        );
        assert_eq!(
            UiProtocolState::default()
                .query(&query)
                .expect_err("runtime-owned query"),
            UiProtocolError::StreamKindMismatch
        );
        let query_frame = UiAdpRequest::Query {
            request_id: "phase2b-query".to_owned(),
            query: query.clone(),
        };
        let encoded = serde_json::to_string(&query_frame).expect("query frame json");
        assert!(encoded.contains("QueryEventInbox"));
        let decoded: UiAdpRequest = serde_json::from_str(&encoded).expect("query frame decode");
        assert_eq!(decoded, query_frame);

        let poll = UiCommand::RunMasterPoll {
            after_cursor: Some("cursor-1".to_owned()),
            limit: Some(50),
            include_terminal: false,
            replay_from_start: false,
        };
        validate_command(&poll).expect("valid master poll command");
        let envelope = build_command_dispatch_envelope(&poll).expect("master poll envelope");
        assert_eq!(envelope.ingress.command_kind, "run_master_poll");
        assert_eq!(envelope.target_feature_id, "task.orchestration");
        assert_eq!(envelope.target_owner_module, "crates/freehand-task");
        let legacy_poll_json = r#"{
            "RunMasterPoll": {
                "after_cursor": null,
                "limit": null,
                "include_terminal": true
            }
        }"#;
        let legacy_poll: UiCommand =
            serde_json::from_str(legacy_poll_json).expect("legacy poll decodes");
        assert_eq!(
            legacy_poll,
            UiCommand::RunMasterPoll {
                after_cursor: None,
                limit: None,
                include_terminal: true,
                replay_from_start: false,
            }
        );

        let err = validate_command(&UiCommand::QueryEventInbox {
            after_cursor: Some("  ".to_owned()),
            limit: Some(10),
        })
        .expect_err("empty event cursor rejected");
        assert_eq!(err, UiProtocolError::EmptyEventCursor);
        assert_eq!(protocol_rejection(err).code, "empty_event_cursor");
        let err = validate_command(&UiCommand::RunMasterPoll {
            after_cursor: Some("  ".to_owned()),
            limit: Some(10),
            include_terminal: false,
            replay_from_start: false,
        })
        .expect_err("empty master poll cursor rejected");
        assert_eq!(err, UiProtocolError::EmptyEventCursor);
        let err = validate_command(&UiCommand::RunMasterPoll {
            after_cursor: Some("cursor-1".to_owned()),
            limit: None,
            include_terminal: false,
            replay_from_start: true,
        })
        .expect_err("conflicting master poll cursor mode rejected");
        assert_eq!(err, UiProtocolError::ConflictingMasterPollCursorMode);

        accept_query_ingress(&UiCommand::QueryMasterPoll {
            after_cursor: None,
            limit: None,
            include_terminal: true,
            replay_from_start: false,
        })
        .expect("read-only master poll passes query ingress");
        let err =
            accept_query_ingress(&poll).expect_err("mutating master poll rejected on query route");
        assert_eq!(err, UiProtocolError::QueryCommandKindMismatch);
        assert_eq!(
            protocol_rejection(err).code,
            "direct_task_mutation_forbidden"
        );
        for mutation in [
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: "exec-1".to_owned(),
                    task_id: "task-1".to_owned(),
                    agent_id: AgentId::new("worker-1"),
                    turn_id: None,
                    kind: UiExecutionFactKind::Blocked {
                        reason: "query-route probe".to_owned(),
                        evidence: Vec::new(),
                    },
                },
            },
            UiCommand::CloseTask {
                task_id: "task-1".to_owned(),
            },
            UiCommand::SubmitUserInput {
                text: "hello".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            },
        ] {
            let err =
                accept_query_ingress(&mutation).expect_err("mutation rejected on query route");
            assert_eq!(err, UiProtocolError::QueryCommandKindMismatch);
        }
        assert_eq!(
            command_frame_class(&UiCommand::QueryTaskBoard {
                status: None,
                agent_id: None,
                include_terminal: false,
            }),
            UiCommandFrameClass::Query
        );
        assert_eq!(
            command_frame_class(&UiCommand::SubscribeProgress),
            UiCommandFrameClass::Subscribe
        );

        let inbox = UiTaskEventInboxProjection {
            source_agent_id: AgentId::new("master"),
            generated_at: 10,
            source_cursor: Some("cursor-0".to_owned()),
            next_cursor: Some("cursor-1".to_owned()),
            events: vec![UiTaskEventInboxEntryProjection {
                cursor: "cursor-1".to_owned(),
                event_id: "event-1".to_owned(),
                kind: "review_ready".to_owned(),
                task_id: "task-1".to_owned(),
                execution_id: Some("exec-1".to_owned()),
                agent_id: Some(AgentId::new("worker-1")),
                created_at: 10,
                payload: serde_json::json!({"summary": "ready"}),
            }],
        };
        let result = UiQueryResult::MasterPoll(UiMasterPollProjection {
            source_agent_id: AgentId::new("master"),
            generated_at: 10,
            source_cursor: Some("cursor-0".to_owned()),
            next_cursor: Some("cursor-1".to_owned()),
            persisted_cursor: Some("cursor-1".to_owned()),
            event_inbox: inbox,
            task_board: UiTaskBoardProjection {
                source_agent_id: AgentId::new("master"),
                status_filter: None,
                agent_filter: None,
                include_terminal: false,
                tasks: Vec::new(),
                agents: Vec::new(),
                blocked: Vec::new(),
                review_ready: Vec::new(),
                stale: Vec::new(),
            },
            agent_board: UiAgentBoardProjection {
                source_agent_id: AgentId::new("master"),
                agents: Vec::new(),
            },
            classifications: vec![UiMasterPollClassificationProjection {
                kind: "review_ready".to_owned(),
                summary: "task task-1 is ready for review".to_owned(),
                task_id: Some("task-1".to_owned()),
                execution_id: Some("exec-1".to_owned()),
                agent_id: Some(AgentId::new("worker-1")),
                recommended_actions: vec![
                    "inspect_submission".to_owned(),
                    "approve_submission".to_owned(),
                    "reject_submission".to_owned(),
                ],
            }],
        });
        let response = UiAdpResponse::QueryResult {
            request_id: "phase2b-poll".to_owned(),
            result,
        };
        let encoded = serde_json::to_string(&response).expect("poll result json");
        assert!(encoded.contains("MasterPoll"));
        assert!(encoded.contains("review_ready"));
        let decoded: UiAdpResponse = serde_json::from_str(&encoded).expect("poll result decode");
        assert_eq!(decoded, response);
    }

    #[test]
    fn worker_control_command_validates_and_routes_to_worker_control() {
        let command = UiCommand::WorkerControl {
            control: UiWorkerControlCommand {
                control_id: Some("wctl-phase2c-1".to_owned()),
                task_id: "task-phase2c".to_owned(),
                execution_id: "exec-phase2c".to_owned(),
                agent_id: AgentId::new("worker-phase2c"),
                op: "ask_at_safe_point".to_owned(),
                question: Some("what is blocking the task?".to_owned()),
                constraint: None,
                note: Some("master runtime query".to_owned()),
            },
        };
        validate_command(&command).expect("valid worker control command");
        let envelope = build_command_dispatch_envelope(&command).expect("worker control envelope");
        assert_eq!(envelope.ingress.command_kind, "worker_control");
        assert_eq!(envelope.target_feature_id, "worker.control");
        assert_eq!(envelope.target_owner_module, "crates/freehand-task");

        let query = UiCommand::QueryWorkerControl {
            task_id: "task-phase2c".to_owned(),
            execution_id: "exec-phase2c".to_owned(),
        };
        validate_command(&query).expect("valid worker control query");
        assert_eq!(
            accept_command_ingress(&query).expect_err("query cannot enter command ingress"),
            UiProtocolError::IngressCommandKindMismatch
        );
        assert_eq!(
            UiProtocolState::default()
                .query(&query)
                .expect_err("runtime-owned query"),
            UiProtocolError::StreamKindMismatch
        );
    }

    #[test]
    fn worker_control_command_rejects_missing_fields() {
        let base = UiWorkerControlCommand {
            control_id: Some("wctl-phase2c-1".to_owned()),
            task_id: "task-phase2c".to_owned(),
            execution_id: "exec-phase2c".to_owned(),
            agent_id: AgentId::new("worker-phase2c"),
            op: "query_status".to_owned(),
            question: None,
            constraint: None,
            note: None,
        };

        let mut missing_task = base.clone();
        missing_task.task_id = " ".to_owned();
        let err = validate_command(&UiCommand::WorkerControl {
            control: missing_task,
        })
        .expect_err("missing task id");
        assert_eq!(err, UiProtocolError::EmptyTaskId);

        let mut missing_execution = base.clone();
        missing_execution.execution_id = " ".to_owned();
        let err = validate_command(&UiCommand::WorkerControl {
            control: missing_execution,
        })
        .expect_err("missing execution id");
        assert_eq!(err, UiProtocolError::EmptyTaskExecutionId);

        let mut missing_control_id = base.clone();
        missing_control_id.control_id = Some(" ".to_owned());
        let err = validate_command(&UiCommand::WorkerControl {
            control: missing_control_id,
        })
        .expect_err("missing control id");
        assert_eq!(err, UiProtocolError::EmptyWorkerControlId);

        let mut missing_op = base.clone();
        missing_op.op = " ".to_owned();
        let err = validate_command(&UiCommand::WorkerControl {
            control: missing_op,
        })
        .expect_err("missing op");
        assert_eq!(err, UiProtocolError::EmptyWorkerControlOp);

        let mut unknown_op = base.clone();
        unknown_op.op = "teleport".to_owned();
        let err = validate_command(&UiCommand::WorkerControl {
            control: unknown_op,
        })
        .expect_err("unknown op");
        assert_eq!(
            err,
            UiProtocolError::UnknownWorkerControlOp("teleport".to_owned())
        );
        assert_eq!(protocol_rejection(err).code, "unknown_worker_control_op");

        let mut missing_question = base.clone();
        missing_question.op = "ask_at_safe_point".to_owned();
        let err = validate_command(&UiCommand::WorkerControl {
            control: missing_question,
        })
        .expect_err("missing safe-point question");
        assert_eq!(err, UiProtocolError::EmptyWorkerControlQuestion);

        let mut missing_constraint = base.clone();
        missing_constraint.op = "add_constraint".to_owned();
        let err = validate_command(&UiCommand::WorkerControl {
            control: missing_constraint,
        })
        .expect_err("missing constraint");
        assert_eq!(err, UiProtocolError::EmptyWorkerControlConstraint);

        let err = validate_command(&UiCommand::QueryWorkerControl {
            task_id: "task-phase2c".to_owned(),
            execution_id: " ".to_owned(),
        })
        .expect_err("query missing execution");
        assert_eq!(err, UiProtocolError::EmptyTaskExecutionId);
    }

    #[test]
    fn worker_control_adp_roundtrip_carries_projection() {
        let request = UiAdpRequest::Command {
            request_id: "phase2c-worker-control-command".to_owned(),
            command: UiCommand::WorkerControl {
                control: UiWorkerControlCommand {
                    control_id: Some("wctl-phase2c-1".to_owned()),
                    task_id: "task-phase2c".to_owned(),
                    execution_id: "exec-phase2c".to_owned(),
                    agent_id: AgentId::new("worker-phase2c"),
                    op: "query_status".to_owned(),
                    question: None,
                    constraint: None,
                    note: None,
                },
            },
        };
        let encoded = serde_json::to_string(&request).expect("worker control request json");
        assert!(encoded.contains("WorkerControl"));
        assert!(encoded.contains("query_status"));
        let decoded: UiAdpRequest = serde_json::from_str(&encoded).expect("request decode");
        assert_eq!(decoded, request);

        let response = UiAdpResponse::QueryResult {
            request_id: "phase2c-worker-control-query".to_owned(),
            result: UiQueryResult::WorkerControl(Box::new(UiWorkerControlProjection {
                source_agent_id: AgentId::new("master"),
                generated_at: 10,
                event: Some(UiWorkerControlEventProjection {
                    control_id: "wctl-phase2c-1".to_owned(),
                    op: "query_status".to_owned(),
                    status: "observed".to_owned(),
                    task_id: "task-phase2c".to_owned(),
                    execution_id: "exec-phase2c".to_owned(),
                    agent_id: AgentId::new("worker-phase2c"),
                    created_at: 10,
                    summary: "queried status".to_owned(),
                    payload: serde_json::json!({"task_status": "running"}),
                }),
                events: Vec::new(),
                task: None,
                agent: None,
                lifecycle: None,
                task_event: None,
            })),
        };
        let encoded = serde_json::to_string(&response).expect("worker control response json");
        assert!(encoded.contains("WorkerControl"));
        assert!(encoded.contains("observed"));
        let decoded: UiAdpResponse = serde_json::from_str(&encoded).expect("response decode");
        assert_eq!(decoded, response);
    }

    #[test]
    fn timer_commands_validate_and_route_to_runtime_master_worker_loop() {
        let schedule = UiCommand::ScheduleTimer {
            timer: UiTimerScheduleCommand {
                timer_id: Some("timer-ui-phase2".to_owned()),
                mode: "relative".to_owned(),
                delay_seconds: Some(180),
                run_at_unix_seconds: None,
                repeat: None,
                max_runs: Some(1),
                reason: "recheck worker status".to_owned(),
                prompt: "Inspect TaskBoard and decide whether the waiting work is complete."
                    .to_owned(),
                source_session_id: Some(SessionId::new("session-ui-phase2")),
            },
        };

        validate_command(&schedule).expect("valid timer schedule");
        let envelope = build_command_dispatch_envelope(&schedule).expect("timer schedule envelope");
        assert_eq!(envelope.ingress.command_kind, "schedule_timer");
        assert_eq!(envelope.target_feature_id, "runtime.master-worker-loop");
        assert_eq!(envelope.target_owner_module, "crates/freehand-runtime");

        let cancel = UiCommand::CancelTimer {
            timer_id: "timer-ui-phase2".to_owned(),
        };
        validate_command(&cancel).expect("valid timer cancel");
        let envelope = build_command_dispatch_envelope(&cancel).expect("timer cancel envelope");
        assert_eq!(envelope.ingress.command_kind, "cancel_timer");
        assert_eq!(envelope.target_feature_id, "runtime.master-worker-loop");

        let query = UiCommand::QueryTimerList {
            include_terminal: true,
        };
        validate_command(&query).expect("valid timer query");
        let err = accept_command_ingress(&query).expect_err("timer query is not command ingress");
        assert_eq!(err, UiProtocolError::IngressCommandKindMismatch);
        let err = UiProtocolState::default()
            .query(&query)
            .expect_err("protocol-only state cannot answer runtime timer query");
        assert_eq!(err, UiProtocolError::StreamKindMismatch);
    }

    #[test]
    fn timer_commands_reject_bad_shapes() {
        let base = UiTimerScheduleCommand {
            timer_id: Some("timer-ui-phase2".to_owned()),
            mode: "relative".to_owned(),
            delay_seconds: Some(180),
            run_at_unix_seconds: None,
            repeat: None,
            max_runs: Some(1),
            reason: "recheck worker status".to_owned(),
            prompt: "Inspect TaskBoard and decide what to do next.".to_owned(),
            source_session_id: Some(SessionId::new("session-ui-phase2")),
        };

        let mut zero_delay = base.clone();
        zero_delay.delay_seconds = Some(0);
        let err = validate_command(&UiCommand::ScheduleTimer { timer: zero_delay })
            .expect_err("zero delay rejected");
        assert_eq!(protocol_rejection(err).code, "missing_timer_delay");

        let mut blank_prompt = base.clone();
        blank_prompt.prompt = " ".to_owned();
        let err = validate_command(&UiCommand::ScheduleTimer {
            timer: blank_prompt,
        })
        .expect_err("blank prompt rejected");
        assert_eq!(protocol_rejection(err).code, "empty_timer_prompt");

        let mut bad_weekly = base.clone();
        bad_weekly.mode = "recurring".to_owned();
        bad_weekly.repeat = Some(UiTimerRepeatCommand::Weekly {
            time_of_day_seconds_local: 9 * 60 * 60,
            weekdays: vec![1, 7],
            max_runs: Some(2),
        });
        let err = validate_command(&UiCommand::ScheduleTimer { timer: bad_weekly })
            .expect_err("bad weekday rejected");
        assert_eq!(protocol_rejection(err).code, "invalid_timer_repeat");

        let err = validate_command(&UiCommand::CancelTimer {
            timer_id: " ".to_owned(),
        })
        .expect_err("empty cancel id rejected");
        assert_eq!(protocol_rejection(err).code, "empty_timer_id");
    }

    #[test]
    fn timer_adp_roundtrip_carries_owner_projection() {
        let request = UiAdpRequest::Command {
            request_id: "timer-schedule-command".to_owned(),
            command: UiCommand::ScheduleTimer {
                timer: UiTimerScheduleCommand {
                    timer_id: Some("timer-cron-ui-phase2".to_owned()),
                    mode: "recurring".to_owned(),
                    delay_seconds: None,
                    run_at_unix_seconds: None,
                    repeat: Some(UiTimerRepeatCommand::Cron {
                        expression: "*/15 9-17 * * 1-5".to_owned(),
                        max_runs: Some(4),
                    }),
                    max_runs: Some(4),
                    reason: "business hours review".to_owned(),
                    prompt: "Inspect current session and decide whether scheduled work closed."
                        .to_owned(),
                    source_session_id: Some(SessionId::new("session-timer-ui")),
                },
            },
        };
        let encoded = serde_json::to_string(&request).expect("timer request json");
        assert!(encoded.contains("ScheduleTimer"));
        assert!(encoded.contains("timer-cron-ui-phase2"));
        let decoded: UiAdpRequest = serde_json::from_str(&encoded).expect("request decode");
        assert_eq!(decoded, request);

        let response = UiAdpResponse::QueryResult {
            request_id: "timer-list-query".to_owned(),
            result: UiQueryResult::TimerList(UiTimerListProjection {
                source_agent_id: AgentId::new("master"),
                generated_at: 100,
                include_terminal: true,
                timers: vec![UiTimerProjection {
                    timer_id: "timer-cron-ui-phase2".to_owned(),
                    agent_id: AgentId::new("master"),
                    status: "active".to_owned(),
                    reason: "business hours review".to_owned(),
                    prompt: "Inspect current session and decide whether scheduled work closed."
                        .to_owned(),
                    next_due_at: 120,
                    created_at: 100,
                    updated_at: 100,
                    fired_count: 0,
                    max_runs: 4,
                    repeat_kind: "cron".to_owned(),
                    repeat_summary: "cron `*/15 9-17 * * 1-5`".to_owned(),
                    source_session_id: Some(SessionId::new("session-timer-ui")),
                    source_turn_id: Some(TurnId::new("runtime-turn-1")),
                }],
                events: vec![UiTimerEventProjection {
                    event_id: "timer-event-1".to_owned(),
                    timer_id: "timer-cron-ui-phase2".to_owned(),
                    event_type: "TimerScheduled".to_owned(),
                    occurred_at: 100,
                    summary: "scheduled next_due_at=120 max_runs=4".to_owned(),
                }],
            }),
        };
        let encoded = serde_json::to_string(&response).expect("timer response json");
        assert!(encoded.contains("TimerList"));
        assert!(encoded.contains("TimerScheduled"));
        let decoded: UiAdpResponse = serde_json::from_str(&encoded).expect("response decode");
        assert_eq!(decoded, response);
    }

    #[test]
    fn tool_registry_query_stays_runtime_owned_and_projects_safe_surface() {
        let query = UiCommand::QueryToolRegistry;
        validate_command(&query).expect("valid tool registry query");
        let err =
            accept_command_ingress(&query).expect_err("tool registry query is not command ingress");
        assert_eq!(err, UiProtocolError::IngressCommandKindMismatch);
        let err = UiProtocolState::default()
            .query(&query)
            .expect_err("protocol state cannot answer tool registry query locally");
        assert_eq!(err, UiProtocolError::StreamKindMismatch);
        assert_eq!(command_kind(&query), "query_tool_registry");

        let response = UiAdpResponse::QueryResult {
            request_id: "tool-registry-query".to_owned(),
            result: UiQueryResult::ToolRegistry(UiToolRegistryProjection {
                source_agent_id: AgentId::new("master"),
                generated_at: 100,
                registry_version: "reasonix-aligned-v1".to_owned(),
                guidance: vec![
                    "Provider-hosted broad search is not a Freehand local function tool named web_search.".to_owned(),
                ],
                tools: vec![
                    UiToolRegistryToolProjection {
                        name: "task".to_owned(),
                        description: "Task Center call shape is strict.".to_owned(),
                        input_schema: serde_json::json!({"type": "object", "required": ["op"]}),
                        read_only: false,
                        implemented: true,
                        execution_scope: "framework".to_owned(),
                        exposed_to_master: true,
                        exposed_to_worker: false,
                        examples: vec![r#"{"op":"assign","task_id":"task-123","agent_id":"worker-1"}"#.to_owned()],
                        guidance: vec!["Every task call must include top-level op.".to_owned()],
                    },
                    UiToolRegistryToolProjection {
                        name: "glob".to_owned(),
                        description: "Find files inside the current locked workspace.".to_owned(),
                        input_schema: serde_json::json!({"type": "object", "required": ["pattern"]}),
                        read_only: true,
                        implemented: true,
                        execution_scope: "workspace".to_owned(),
                        exposed_to_master: true,
                        exposed_to_worker: true,
                        examples: vec![r#"{"pattern":"/absolute/or/symlink/workspace/**/*.rs"}"#.to_owned()],
                        guidance: vec![
                            "Leading-~ and absolute paths are valid only when canonical/symlink resolution stays inside the locked workspace.".to_owned(),
                        ],
                    },
                ],
            }),
        };
        let encoded = serde_json::to_string(&response).expect("tool registry response json");
        assert!(encoded.contains("ToolRegistry"));
        assert!(encoded.contains("locked workspace"));
        assert!(!encoded.contains("\"web_search\",\"description\""));
        let decoded: UiAdpResponse =
            serde_json::from_str(&encoded).expect("tool registry response decode");
        assert_eq!(decoded, response);
    }

    #[test]
    fn diagnostics_query_stays_runtime_owned_and_projects_safe_log_surface() {
        let query = UiCommand::QueryDiagnostics;
        validate_command(&query).expect("valid diagnostics query");
        let err =
            accept_command_ingress(&query).expect_err("diagnostics query is not command ingress");
        assert_eq!(err, UiProtocolError::IngressCommandKindMismatch);
        let err = UiProtocolState::default()
            .query(&query)
            .expect_err("protocol state cannot answer diagnostics locally");
        assert_eq!(err, UiProtocolError::StreamKindMismatch);
        assert_eq!(command_kind(&query), "query_diagnostics");

        let response = UiAdpResponse::QueryResult {
            request_id: "diagnostics-query".to_owned(),
            result: UiQueryResult::Diagnostics(UiDiagnosticsProjection {
                source_agent_id: AgentId::new("master"),
                generated_at: 100,
                runtime_home: "~/.freehand".to_owned(),
                logs_dir: "logs".to_owned(),
                files: vec![UiDiagnosticLogFileProjection {
                    name: "daemonS.stdout.log".to_owned(),
                    relative_path: "logs/daemonS.stdout.log".to_owned(),
                    size_bytes: 42,
                    modified_at: Some(99),
                    tail_lines: vec!["service ready".to_owned()],
                }],
            }),
        };
        let encoded = serde_json::to_string(&response).expect("diagnostics response json");
        assert!(encoded.contains("Diagnostics"));
        assert!(encoded.contains("daemonS.stdout.log"));
        assert!(!encoded.contains("/Users/"));
        let decoded: UiAdpResponse =
            serde_json::from_str(&encoded).expect("diagnostics response decode");
        assert_eq!(decoded, response);
    }

    #[test]
    fn session_list_and_transcript_project_session_cwd() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("webui-session-cwd");
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: session_id.clone(),
            title: Some("Cwd session".to_owned()),
            archived: false,
            cwd: None,
        });
        state.apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: TurnId::new("turn-cwd-1"),
            created_at: Some(11),
            timing: None,
            cwd: Some("/tmp/freehand-cwd".to_owned()),
            user_text: Some("run in cwd".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        }));

        match state.query(&UiCommand::QuerySessionList).expect("list") {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions[0].cwd.as_deref(), Some("/tmp/freehand-cwd"));
            }
            other => panic!("unexpected list result: {other:?}"),
        }
        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.cwd.as_deref(), Some("/tmp/freehand-cwd"));
                assert_eq!(
                    transcript.turns[0].cwd.as_deref(),
                    Some("/tmp/freehand-cwd")
                );
            }
            other => panic!("unexpected transcript result: {other:?}"),
        }
    }

    #[test]
    fn session_list_hides_internal_lifecycle_sessions_but_transcript_is_queryable() {
        let mut state = UiProtocolState::default();
        let user_session_id = SessionId::new("webui-visible-session");
        let lifecycle_session_id = SessionId::new("master-lifecycle-task-1");
        let timer_session_id = SessionId::new("master-timer-timer-1");
        let worker_session_id = SessionId::new("worker-task-task-1");
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: user_session_id.clone(),
            title: Some("Visible session".to_owned()),
            archived: false,
            cwd: None,
        });
        state.apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("master"),
            source_node_id: "node-1".to_owned(),
            session_id: user_session_id.clone(),
            turn_id: TurnId::new("turn-visible-1"),
            created_at: Some(12),
            timing: None,
            cwd: None,
            user_text: Some("visible user turn".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        }));
        state.apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("master"),
            source_node_id: "node-1".to_owned(),
            session_id: lifecycle_session_id.clone(),
            turn_id: TurnId::new("turn-lifecycle-1"),
            created_at: Some(13),
            timing: None,
            cwd: None,
            user_text: Some("internal lifecycle decision".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        }));
        state.apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("master"),
            source_node_id: "node-1".to_owned(),
            session_id: timer_session_id.clone(),
            turn_id: TurnId::new("turn-timer-1"),
            created_at: Some(14),
            timing: None,
            cwd: None,
            user_text: Some("internal timer wakeup".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        }));
        state.apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("worker"),
            source_node_id: "node-worker".to_owned(),
            session_id: worker_session_id.clone(),
            turn_id: TurnId::new("turn-worker-1"),
            created_at: Some(15),
            timing: None,
            cwd: None,
            user_text: Some("internal worker execution".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        }));

        match state.query(&UiCommand::QuerySessionList).expect("list") {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 1);
                assert_eq!(list.sessions[0].session_id, user_session_id);
            }
            other => panic!("unexpected list result: {other:?}"),
        }
        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: lifecycle_session_id.clone(),
            })
            .expect("transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.session_id, lifecycle_session_id);
                assert_eq!(transcript.turns.len(), 1);
                assert_eq!(
                    transcript.turns[0].user_text.as_deref(),
                    Some("internal lifecycle decision")
                );
            }
            other => panic!("unexpected transcript result: {other:?}"),
        }
        for internal_session_id in [timer_session_id, worker_session_id] {
            match state
                .query(&UiCommand::QuerySessionTurns {
                    session_id: internal_session_id.clone(),
                })
                .expect("internal transcript")
            {
                UiQueryResult::SessionTurns(transcript) => {
                    assert_eq!(transcript.session_id, internal_session_id);
                    assert_eq!(transcript.turns.len(), 1);
                }
                other => panic!("unexpected internal transcript result: {other:?}"),
            }
        }
    }

    #[test]
    fn session_search_query_is_runtime_owned_and_validated() {
        let command = UiCommand::QuerySessionSearch {
            query: "roadmap".to_owned(),
            limit: Some(10),
        };
        validate_command(&command).expect("valid search query");
        assert_eq!(command_kind(&command), "query_session_search");
        assert_eq!(
            accept_command_ingress(&command).expect_err("query is not command ingress"),
            UiProtocolError::IngressCommandKindMismatch
        );
        assert_eq!(
            UiProtocolState::default()
                .query(&command)
                .expect_err("runtime-owned query must not use local UI truth"),
            UiProtocolError::StreamKindMismatch
        );

        let err = validate_command(&UiCommand::QuerySessionSearch {
            query: "   ".to_owned(),
            limit: None,
        })
        .expect_err("empty query rejected");
        assert_eq!(err, UiProtocolError::EmptyUserInput);
        assert_eq!(protocol_rejection(err).code, "empty_user_input");
    }

    #[test]
    fn session_list_exposes_only_persisted_metadata_sessions() {
        let mut state = UiProtocolState::default();
        let persisted_session_id = SessionId::new("persisted-session");
        let turn_only_session_id = SessionId::new("turn-only-session");
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: persisted_session_id.clone(),
            title: Some("Persisted".to_owned()),
            archived: false,
            cwd: None,
        });
        for (index, session_id) in [persisted_session_id.clone(), turn_only_session_id.clone()]
            .into_iter()
            .enumerate()
        {
            state.apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
                source_agent_id: AgentId::new("master"),
                source_node_id: "node-1".to_owned(),
                session_id,
                turn_id: TurnId::new(format!("turn-metadata-only-{index}")),
                created_at: Some(20 + index as u64),
                timing: None,
                cwd: None,
                user_text: Some("turn text".to_owned()),
                semantic_events: Vec::new(),
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                usage_events: Vec::new(),
                terminal_event: None,
                error_events: Vec::new(),
                slave_substream_card: false,
            }));
        }

        match state.query(&UiCommand::QuerySessionList).expect("list") {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 1);
                assert_eq!(list.sessions[0].session_id, persisted_session_id);
            }
            other => panic!("unexpected list result: {other:?}"),
        }
        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: turn_only_session_id.clone(),
            })
            .expect("transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.session_id, turn_only_session_id);
                assert_eq!(transcript.turns.len(), 1);
            }
            other => panic!("unexpected transcript result: {other:?}"),
        }
    }

    #[test]
    fn session_list_active_turn_id_tracks_only_nonterminal_turns() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-active-terminal-filter");
        let active_turn_id = TurnId::new("runtime-turn-1");
        let terminal_turn_id = TurnId::new("runtime-turn-2");
        let next_active_turn_id = TurnId::new("runtime-turn-3");
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: session_id.clone(),
            title: Some("Active terminal filter".to_owned()),
            archived: false,
            cwd: None,
        });

        state.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: active_turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail: Some("Waiting for model response.".to_owned()),
            transport: Some(UiModelTransportActivity {
                kind: UiModelTransportKind::ProviderRetry,
                detail: Some("provider retry 2/10".to_owned()),
            }),
            slave_substream_card: false,
        });
        match state.query(&UiCommand::QuerySessionList).expect("list") {
            UiQueryResult::SessionList(list) => {
                assert_eq!(
                    list.sessions[0].active_turn_id.as_ref(),
                    Some(&active_turn_id)
                );
            }
            other => panic!("unexpected list result: {other:?}"),
        }

        state.replace_session_turn_projections(
            &session_id,
            vec![
                active_refresh_projection(&session_id, &active_turn_id),
                terminal_refresh_projection(
                    &session_id,
                    &terminal_turn_id,
                    TerminalStatus::Success,
                ),
            ],
        );
        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                let stale_round = transcript
                    .turns
                    .iter()
                    .find(|turn| turn.turn_id == active_turn_id)
                    .expect("stale round");
                assert_eq!(stale_round.model_request, None);
            }
            other => panic!("unexpected transcript result: {other:?}"),
        }
        match state.query(&UiCommand::QuerySessionList).expect("list") {
            UiQueryResult::SessionList(list) => {
                assert_eq!(
                    list.sessions[0].latest_turn_id.as_ref(),
                    Some(&terminal_turn_id)
                );
                assert_eq!(list.sessions[0].latest_status, "success");
                assert_eq!(list.sessions[0].active_turn_id, None);
            }
            other => panic!("unexpected list result: {other:?}"),
        }

        state.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: next_active_turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail: Some("provider request built".to_owned()),
            transport: None,
            slave_substream_card: false,
        });
        match state.query(&UiCommand::QuerySessionList).expect("list") {
            UiQueryResult::SessionList(list) => {
                assert_eq!(
                    list.sessions[0].active_turn_id.as_ref(),
                    Some(&next_active_turn_id)
                );
                assert_eq!(list.sessions[0].latest_status, "waiting_model");
            }
            other => panic!("unexpected list result: {other:?}"),
        }
    }

    #[test]
    fn tool_activity_waits_until_matching_result_reentry() {
        let mut projection = sample_turn_projection(false);
        projection.terminal_text = None;
        projection.terminal_status = None;
        let items = public_conversation_items(&projection);
        let tool = items
            .iter()
            .find(|item| item.kind == UiConversationItemKind::ToolSummary)
            .expect("tool item");
        assert_eq!(tool.status, "waiting");
        assert_eq!(tool.title, "Run tool");
        assert_eq!(tool.body, "Run tool: search");
        assert_eq!(
            tool.display.as_ref().map(|display| display.kind.as_str()),
            Some("generic")
        );

        let completed = turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            created_at: Some(30),
            timing: None,
            cwd: None,
            user_text: Some("run the task".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: vec![ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_call: freehand_contracts::ToolCallContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    tool_name: "grep".to_owned(),
                    arguments: vec![freehand_contracts::ToolArgument {
                        name: "pattern".to_owned(),
                        value: serde_json::json!("needle"),
                    }],
                    arguments_complete: true,
                },
            }],
            tool_results: vec![ReasonReq05ToolResultReentry {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_result: freehand_contracts::ToolResultContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    status: freehand_contracts::ToolResultStatus::Success,
                    output: "result body rendered in public summary".to_owned(),
                },
            }],
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        });
        let completed_tool = public_conversation_items(&completed)
            .into_iter()
            .find(|item| item.kind == UiConversationItemKind::ToolSummary)
            .expect("completed tool item");
        assert_eq!(completed_tool.status, "completed");
        assert_eq!(completed_tool.title, "Search text");
        assert_eq!(
            completed_tool.body,
            "pattern=needle\nresult: result body rendered in public summary"
        );
        assert_eq!(
            completed_tool
                .display
                .as_ref()
                .map(|display| display.kind.as_str()),
            Some("search")
        );
    }

    #[test]
    fn framework_tool_public_projection_uses_task_and_timer_display_semantics() {
        let projection = turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("master"),
            source_node_id: "master-node".to_owned(),
            session_id: SessionId::new("session-framework-tools"),
            turn_id: TurnId::new("runtime-turn-framework-tools"),
            created_at: Some(40),
            timing: None,
            cwd: None,
            user_text: Some("delegate work and schedule a check".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: vec![
                ReasonReq04ToolCall {
                    session_id: SessionId::new("session-framework-tools"),
                    turn_id: TurnId::new("runtime-turn-framework-tools"),
                    trace_id: TraceId::new("trace-framework-tools"),
                    feature_id: FeatureId::new("ui.protocol"),
                    agent_id: AgentId::new("master"),
                    tool_call: freehand_contracts::ToolCallContract {
                        tool_call_id: freehand_contracts::ToolCallId::new("tool-task"),
                        tool_name: "task".to_owned(),
                        arguments: vec![
                            freehand_contracts::ToolArgument {
                                name: "op".to_owned(),
                                value: serde_json::json!("assign"),
                            },
                            freehand_contracts::ToolArgument {
                                name: "task_id".to_owned(),
                                value: serde_json::json!("task-123"),
                            },
                            freehand_contracts::ToolArgument {
                                name: "agent_id".to_owned(),
                                value: serde_json::json!("worker-alpha"),
                            },
                        ],
                        arguments_complete: true,
                    },
                },
                ReasonReq04ToolCall {
                    session_id: SessionId::new("session-framework-tools"),
                    turn_id: TurnId::new("runtime-turn-framework-tools"),
                    trace_id: TraceId::new("trace-framework-tools"),
                    feature_id: FeatureId::new("ui.protocol"),
                    agent_id: AgentId::new("master"),
                    tool_call: freehand_contracts::ToolCallContract {
                        tool_call_id: freehand_contracts::ToolCallId::new("tool-timer"),
                        tool_name: "timer".to_owned(),
                        arguments: vec![
                            freehand_contracts::ToolArgument {
                                name: "op".to_owned(),
                                value: serde_json::json!("schedule"),
                            },
                            freehand_contracts::ToolArgument {
                                name: "delay_seconds".to_owned(),
                                value: serde_json::json!(300),
                            },
                            freehand_contracts::ToolArgument {
                                name: "reason".to_owned(),
                                value: serde_json::json!("re-check worker review"),
                            },
                            freehand_contracts::ToolArgument {
                                name: "prompt".to_owned(),
                                value: serde_json::json!(
                                    "Read TaskBoard and decide the next step."
                                ),
                            },
                        ],
                        arguments_complete: true,
                    },
                },
            ],
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        });

        let items = public_conversation_items(&projection);
        let task = items
            .iter()
            .find(|item| item.tool_call_id.as_deref() == Some("tool-task"))
            .expect("task tool");
        assert_eq!(task.title, "Assign Worker task");
        assert_eq!(task.body, "op=assign · task=task-123 · agent=worker-alpha");
        assert_eq!(
            task.display.as_ref().map(|display| display.kind.as_str()),
            Some("task")
        );

        let timer = items
            .iter()
            .find(|item| item.tool_call_id.as_deref() == Some("tool-timer"))
            .expect("timer tool");
        assert_eq!(timer.title, "Schedule timer");
        assert!(timer.body.contains("when=in 300s"));
        assert!(timer.body.contains("reason=re-check worker review"));
        assert_eq!(
            timer.display.as_ref().map(|display| display.kind.as_str()),
            Some("timer")
        );
    }

    #[test]
    fn public_conversation_strips_hidden_control_status_blocks() {
        let mut projection = sample_turn_projection(false);
        projection.text = vec![
            concat!(
                "answer\n",
                "<<<freehand_status>>>\n",
                "{\"schema_version\":1,\"status\":{\"simple_question\":true}}\n",
                "<</freehand_status>>>"
            )
            .to_owned(),
        ];
        projection.terminal_text = Some(
            concat!(
                "final\n",
                "<<<freehand_status>>>\n",
                "{\"schema_version\":1,\"status\":{\"simple_question\":true}}\n",
                "<</freehand_status>>>"
            )
            .to_owned(),
        );

        let items = public_conversation_items(&projection);
        let encoded = serde_json::to_string(&items).expect("items json");

        assert!(!encoded.contains("freehand_status"));
        assert!(encoded.contains("answer"));
        assert!(encoded.contains("final"));
    }

    #[test]
    fn failed_tool_result_updates_same_activity_without_error_projection() {
        let projection = turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            created_at: Some(50),
            timing: None,
            cwd: None,
            user_text: Some("run the task".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: vec![ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_call: freehand_contracts::ToolCallContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    tool_name: "read_file".to_owned(),
                    arguments: vec![freehand_contracts::ToolArgument {
                        name: "path".to_owned(),
                        value: serde_json::json!("missing.txt"),
                    }],
                    arguments_complete: true,
                },
            }],
            tool_results: vec![ReasonReq05ToolResultReentry {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_result: freehand_contracts::ToolResultContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    status: freehand_contracts::ToolResultStatus::Failed,
                    output: "failure body rendered in public summary".to_owned(),
                },
            }],
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        });

        assert_eq!(projection.tool_activities.len(), 1);
        assert_eq!(
            projection.tool_activities[0].status,
            UiToolActivityStatus::Failed
        );
        assert_eq!(projection.terminal_status, None);
        assert!(projection.errors.is_empty());
        let cards = public_conversation_items(&projection);
        let tool_cards = cards
            .iter()
            .filter(|item| item.kind == UiConversationItemKind::ToolSummary)
            .collect::<Vec<_>>();
        assert_eq!(tool_cards.len(), 1);
        assert_eq!(tool_cards[0].status, "failed");
        assert_eq!(tool_cards[0].title, "Read file");
        assert_eq!(
            tool_cards[0].body,
            "path=missing.txt\nfailure: failure body rendered in public summary"
        );
        assert!(
            cards
                .iter()
                .all(|item| item.kind != UiConversationItemKind::Error)
        );
    }

    #[test]
    fn failed_terminal_marks_waiting_tool_activity_failed() {
        let projection = turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            created_at: Some(60),
            timing: None,
            cwd: None,
            user_text: Some("run the task".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: vec![ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_call: freehand_contracts::ToolCallContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    tool_name: "ls".to_owned(),
                    arguments: vec![],
                    arguments_complete: true,
                },
            }],
            tool_results: Vec::new(),
            usage_events: Vec::new(),
            terminal_event: Some(ReasonResp03TerminalEvent {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                status: TerminalStatus::Failed,
                summary: "tool failed explicitly".to_owned(),
            }),
            error_events: Vec::new(),
            slave_substream_card: false,
        });

        assert_eq!(
            projection.tool_activities[0].status,
            UiToolActivityStatus::Failed
        );
        let tool = public_conversation_items(&projection)
            .into_iter()
            .find(|item| item.kind == UiConversationItemKind::ToolSummary)
            .expect("tool item");
        assert_eq!(tool.status, "failed");
        assert_eq!(tool.title, "List directory");
        assert_eq!(tool.body, "path=.\ntool failed explicitly");
    }

    #[test]
    fn session_latest_status_does_not_call_text_only_turn_streaming() {
        let projection = UiTurnProjection {
            source: base_source(UiStreamKind::Turn),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            created_at: Some(70),
            timing: None,
            cwd: None,
            user_text: Some("run the task".to_owned()),
            attachments: Vec::new(),
            model_request: None,
            reasoning: vec!["thinking".to_owned()],
            text: vec!["answer".to_owned()],
            tool_calls: Vec::new(),
            tool_activities: Vec::new(),
            usage: Vec::new(),
            terminal_status: None,
            terminal_text: None,
            errors: Vec::new(),
            slave_substream_card: false,
        };

        assert_eq!(session_latest_status(&projection), "active");
    }

    #[test]
    fn turn_projection_preserves_durable_timing() {
        let projection = sample_turn_projection(false);
        let timing = projection.timing.as_ref().expect("turn timing projection");

        assert_eq!(timing.turn_started_at_ms, Some(10_000));
        assert_eq!(timing.first_response_at_ms, Some(11_250));
        assert_eq!(timing.completed_at_ms, Some(12_500));
        assert_eq!(timing.time_to_first_response_ms, Some(1_250));
        assert_eq!(timing.total_elapsed_ms, Some(2_500));

        let encoded = serde_json::to_string(&projection).expect("serialize projection");
        assert!(encoded.contains("time_to_first_response_ms"));
        assert!(encoded.contains("total_elapsed_ms"));
    }

    #[test]
    fn duplicate_tool_call_projection_updates_one_activity_card() {
        let tool_call = ReasonReq04ToolCall {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("ui.protocol"),
            agent_id: AgentId::new("agent-1"),
            tool_call: freehand_contracts::ToolCallContract {
                tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                tool_name: "ls".to_owned(),
                arguments: vec![],
                arguments_complete: true,
            },
        };
        let projection = turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            created_at: Some(80),
            timing: None,
            cwd: None,
            user_text: Some("run the task".to_owned()),
            semantic_events: Vec::new(),
            tool_calls: vec![tool_call.clone(), tool_call],
            tool_results: vec![ReasonReq05ToolResultReentry {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                tool_result: freehand_contracts::ToolResultContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    status: freehand_contracts::ToolResultStatus::Success,
                    output: "private output".to_owned(),
                },
            }],
            usage_events: Vec::new(),
            terminal_event: None,
            error_events: Vec::new(),
            slave_substream_card: false,
        });

        assert_eq!(projection.tool_activities.len(), 1);
        assert_eq!(
            projection.tool_activities[0].status,
            UiToolActivityStatus::Completed
        );
        let tool_cards = public_conversation_items(&projection)
            .into_iter()
            .filter(|item| item.kind == UiConversationItemKind::ToolSummary)
            .collect::<Vec<_>>();
        assert_eq!(tool_cards.len(), 1);
        assert_eq!(tool_cards[0].status, "completed");
        assert_eq!(tool_cards[0].tool_call_id.as_deref(), Some("tool-1"));
    }

    #[test]
    fn model_request_waiting_projection_clears_on_response_event() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-model-wait");
        let turn_id = TurnId::new("turn-model-wait");
        let waiting = state.apply_model_request_waiting(
            AgentId::new("agent-1"),
            "node-1".to_owned(),
            &session_id,
            &turn_id,
            Some("provider request built".to_owned()),
            false,
        );
        assert_eq!(
            waiting
                .model_request
                .as_ref()
                .map(|activity| activity.status),
            Some(UiModelRequestStatus::Waiting)
        );
        assert_eq!(
            waiting.model_request.as_ref().map(|activity| activity.kind),
            Some(UiModelRequestKind::Thinking)
        );
        assert_eq!(
            waiting
                .model_request
                .as_ref()
                .and_then(|activity| activity.detail.as_deref()),
            Some("provider request built")
        );

        let responded = state.apply_semantic_event(
            AgentId::new("agent-1"),
            "node-1".to_owned(),
            &ReasonResp01SemanticEvent {
                session_id,
                turn_id,
                trace_id: TraceId::new("trace-model-wait"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                kind: SemanticEventKind::Text,
                content: "model response arrived".to_owned(),
            },
            false,
        );
        assert_eq!(responded.model_request, None);
        assert_eq!(responded.text, vec!["model response arrived".to_owned()]);
    }

    #[test]
    fn provider_recovery_activity_updates_in_place_and_clears_on_response() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-provider-recovery");
        let turn_id = TurnId::new("turn-provider-recovery");
        state.replace_session_turn_projections(
            &session_id,
            vec![active_refresh_projection(&session_id, &turn_id)],
        );

        let retrying = (1..=3)
            .map(|retry_index| {
                state.apply_model_request_waiting_kind(UiModelRequestWaiting {
                    source_agent_id: AgentId::new("agent-1"),
                    source_node_id: "node-1".to_owned(),
                    session_id: session_id.clone(),
                    turn_id: turn_id.clone(),
                    kind: UiModelRequestKind::Thinking,
                    detail: Some("Waiting for model response.".to_owned()),
                    transport: Some(UiModelTransportActivity {
                        kind: UiModelTransportKind::ProviderRetry,
                        detail: Some(format!("provider request retry {retry_index}/10")),
                    }),
                    slave_substream_card: false,
                })
            })
            .last()
            .expect("provider retry projection");
        assert_eq!(
            retrying
                .model_request
                .as_ref()
                .map(|activity| activity.kind),
            Some(UiModelRequestKind::Thinking)
        );
        let retry_transport = retrying
            .model_request
            .as_ref()
            .and_then(|activity| activity.transport.as_ref())
            .expect("provider retry transport activity");
        assert_eq!(retry_transport.kind, UiModelTransportKind::ProviderRetry);
        assert_eq!(
            retry_transport.detail.as_deref(),
            Some("provider request retry 3/10")
        );
        assert_eq!(retrying.user_text.as_deref(), Some("run active work"));
        assert!(retrying.errors.is_empty());
        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("provider retry transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 1);
                let public_items = public_conversation_items(&transcript.turns[0]);
                assert_eq!(
                    public_items
                        .iter()
                        .filter(|item| item.kind == UiConversationItemKind::UserText)
                        .count(),
                    1
                );
            }
            other => panic!("unexpected transcript query: {other:?}"),
        }

        let failover = state.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail: Some("Waiting for model response.".to_owned()),
            transport: Some(UiModelTransportActivity {
                kind: UiModelTransportKind::ProviderFailover,
                detail: Some("provider route switched to fallback".to_owned()),
            }),
            slave_substream_card: false,
        });
        assert_eq!(
            failover
                .model_request
                .as_ref()
                .map(|activity| activity.kind),
            Some(UiModelRequestKind::Thinking)
        );
        assert_eq!(
            failover
                .model_request
                .as_ref()
                .and_then(|activity| activity.transport.as_ref())
                .map(|transport| transport.kind),
            Some(UiModelTransportKind::ProviderFailover)
        );
        assert!(failover.errors.is_empty());

        let recovered = state.apply_semantic_event(
            AgentId::new("agent-1"),
            "node-1".to_owned(),
            &ReasonResp01SemanticEvent {
                session_id,
                turn_id,
                trace_id: TraceId::new("trace-provider-recovery"),
                feature_id: FeatureId::new("ui.protocol"),
                agent_id: AgentId::new("agent-1"),
                kind: SemanticEventKind::Text,
                content: "provider recovered".to_owned(),
            },
            false,
        );
        assert!(recovered.model_request.is_none());
        assert!(recovered.errors.is_empty());
    }

    #[test]
    fn session_refresh_preserves_active_model_request_activity() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-active-refresh");
        let turn_id = TurnId::new("runtime-turn-1");

        state.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail: Some("Waiting for model response.".to_owned()),
            transport: Some(UiModelTransportActivity {
                kind: UiModelTransportKind::ProviderRetry,
                detail: Some("provider retry 6/10".to_owned()),
            }),
            slave_substream_card: false,
        });
        state.replace_session_turn_projections(
            &session_id,
            vec![active_refresh_projection(&session_id, &turn_id)],
        );

        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("query refreshed transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 1);
                let activity = transcript.turns[0]
                    .model_request
                    .as_ref()
                    .expect("active provider activity must survive refresh");
                assert_eq!(activity.kind, UiModelRequestKind::Thinking);
                let transport = activity
                    .transport
                    .as_ref()
                    .expect("active provider retry transport survives refresh");
                assert_eq!(transport.kind, UiModelTransportKind::ProviderRetry);
                assert_eq!(transport.detail.as_deref(), Some("provider retry 6/10"));
            }
            other => panic!("unexpected transcript query: {other:?}"),
        }
    }

    #[test]
    fn session_refresh_preserves_active_tool_activity_cards() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-tool-refresh");
        let turn_id = TurnId::new("runtime-turn-2");
        let tool_call = ui_tool_call(&session_id, &turn_id);

        state.apply_tool_call(
            AgentId::new("agent-1"),
            "node-1".to_owned(),
            &tool_call,
            false,
        );
        state.replace_session_turn_projections(
            &session_id,
            vec![active_refresh_projection(&session_id, &turn_id)],
        );

        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("query refreshed transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 1);
                assert_eq!(transcript.turns[0].tool_activities.len(), 1);
                assert_eq!(
                    transcript.turns[0].tool_activities[0].status,
                    UiToolActivityStatus::Waiting
                );
                let tool_cards = public_conversation_items(&transcript.turns[0])
                    .into_iter()
                    .filter(|item| item.kind == UiConversationItemKind::ToolSummary)
                    .collect::<Vec<_>>();
                assert_eq!(tool_cards.len(), 1);
                assert_eq!(tool_cards[0].title, "Run task operation");
                assert_eq!(tool_cards[0].status, "waiting");
            }
            other => panic!("unexpected transcript query: {other:?}"),
        }
    }

    #[test]
    fn terminal_session_refresh_drops_stale_live_activity() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-terminal-refresh");
        let turn_id = TurnId::new("runtime-turn-3");

        state.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail: Some("Waiting for model response.".to_owned()),
            transport: Some(UiModelTransportActivity {
                kind: UiModelTransportKind::ProviderRetry,
                detail: Some("provider retry 9/10".to_owned()),
            }),
            slave_substream_card: false,
        });
        state.replace_session_turn_projections(
            &session_id,
            vec![terminal_refresh_projection(
                &session_id,
                &turn_id,
                TerminalStatus::Failed,
            )],
        );

        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("query refreshed transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 1);
                assert!(transcript.turns[0].model_request.is_none());
                assert_eq!(
                    transcript.turns[0].terminal_status,
                    Some(TerminalStatus::Failed)
                );
                assert!(
                    transcript.turns[0]
                        .terminal_text
                        .as_deref()
                        .is_some_and(|text| text.contains("terminal refresh truth"))
                );
            }
            other => panic!("unexpected transcript query: {other:?}"),
        }
    }

    #[test]
    fn persisted_session_merge_is_silent_and_preserves_live_projection() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-background-refresh");
        let turn_id = TurnId::new("runtime-turn-1");
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: session_id.clone(),
            title: Some("Background refresh".to_owned()),
            archived: false,
            cwd: None,
        });
        state.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail: Some("live request".to_owned()),
            transport: None,
            slave_substream_card: false,
        });
        let mut receiver = state.subscribe();
        state.merge_persisted_turn_projections_without_publish(vec![active_refresh_projection(
            &session_id,
            &turn_id,
        )]);

        assert!(receiver.try_recv().is_err());
        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert!(transcript.turns[0].model_request.is_some());
            }
            other => panic!("unexpected transcript: {other:?}"),
        }
        match state.query(&UiCommand::QuerySessionList).expect("list") {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions[0].active_turn_id.as_ref(), Some(&turn_id));
            }
            other => panic!("unexpected list: {other:?}"),
        }
    }

    #[test]
    fn persisted_terminal_merge_silently_closes_live_projection() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-background-terminal");
        let turn_id = TurnId::new("runtime-turn-2");
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: session_id.clone(),
            title: Some("Background terminal".to_owned()),
            archived: false,
            cwd: None,
        });
        state.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail: Some("live request".to_owned()),
            transport: None,
            slave_substream_card: false,
        });
        let mut receiver = state.subscribe();
        state.merge_persisted_turn_projections_without_publish(vec![terminal_refresh_projection(
            &session_id,
            &turn_id,
            TerminalStatus::Success,
        )]);

        assert!(receiver.try_recv().is_err());
        match state
            .query(&UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            })
            .expect("transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert!(transcript.turns[0].model_request.is_none());
                assert_eq!(
                    transcript.turns[0].terminal_status,
                    Some(TerminalStatus::Success)
                );
            }
            other => panic!("unexpected transcript: {other:?}"),
        }
        match state.query(&UiCommand::QuerySessionList).expect("list") {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions[0].active_turn_id, None);
                assert_eq!(list.sessions[0].latest_status, "success");
            }
            other => panic!("unexpected list: {other:?}"),
        }
    }

    #[test]
    fn schema_mismatch_projects_as_model_polishing_activity() {
        let mut state = UiProtocolState::default();
        let session_id = SessionId::new("session-schema-retry");
        let turn_id = TurnId::new("turn-schema-retry");

        let waiting = state.apply_completion_schema_retry_waiting(UiCompletionSchemaRetryWaiting {
            source_agent_id: AgentId::new("agent-1"),
            source_node_id: "node-1".to_owned(),
            session_id,
            turn_id,
            retry_index: 2,
            issue_summary: "evidence must be a string, got array".to_owned(),
            slave_substream_card: false,
        });

        let activity = waiting.model_request.expect("model request activity");
        assert_eq!(activity.status, UiModelRequestStatus::Waiting);
        assert_eq!(activity.kind, UiModelRequestKind::SchemaRetry);
        let detail = activity.detail.expect("detail");
        assert!(detail.contains("schema polishing #2"));
        assert!(detail.contains("evidence must be a string"));
        assert!(!detail.contains("Feedback sent to the model"));
    }

    #[test]
    fn slave_turn_subscription_smoke() {
        let projection = sample_turn_projection(true);
        let selector = subscription_selector(&UiCommand::SubscribeTurn {
            client: UiClientKind::WebUi,
            turn_id: TurnId::new("turn-1"),
        })
        .expect("selector");
        let event = UiProjection::Turn(projection.clone());
        assert!(subscription_matches(
            &selector,
            &event,
            Some(&TurnId::new("turn-1"))
        ));
        let cli_projection = turn_projection_for_client(projection, UiClientKind::Cli);
        assert!(!cli_projection.slave_substream_card);
    }

    #[test]
    fn node_status_query_smoke() {
        let mut state = UiProtocolState::default();
        state.set_node_status(NodeStatusSnapshot {
            source: base_source(UiStreamKind::NodeStatus),
            node_id: "node-1".to_owned(),
            healthy: true,
            pairing_state: "paired".to_owned(),
        });
        let result = state
            .query(&UiCommand::QueryNodeStatus {
                node_id: "node-1".to_owned(),
            })
            .expect("query");
        match result {
            UiQueryResult::NodeStatus(Some(snapshot)) => {
                assert!(snapshot.healthy);
                assert_eq!(snapshot.pairing_state, "paired");
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }

    #[test]
    fn session_queries_return_ordered_transcript_without_cross_session_leakage() {
        let mut state = UiProtocolState::default();
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: SessionId::new("session-a"),
            title: Some("Session A".to_owned()),
            archived: false,
            cwd: None,
        });
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: SessionId::new("session-b"),
            title: Some("Session B".to_owned()),
            archived: false,
            cwd: None,
        });
        let mut first = sample_turn_projection(false);
        first.session_id = SessionId::new("session-a");
        first.turn_id = TurnId::new("runtime-turn-1-r2");
        first.source.source_turn_id = Some(first.turn_id.clone());
        first.user_text = Some("first prompt".to_owned());
        first.terminal_text = Some("first answer".to_owned());

        let mut second = sample_turn_projection(false);
        second.session_id = SessionId::new("session-a");
        second.turn_id = TurnId::new("runtime-turn-2-r2");
        second.source.source_turn_id = Some(second.turn_id.clone());
        second.user_text = Some("second prompt".to_owned());
        second.terminal_text = Some("second answer".to_owned());

        let mut tenth = sample_turn_projection(false);
        tenth.session_id = SessionId::new("session-a");
        tenth.turn_id = TurnId::new("runtime-turn-10-r2");
        tenth.source.source_turn_id = Some(tenth.turn_id.clone());
        tenth.user_text = Some("tenth prompt".to_owned());
        tenth.terminal_text = Some("tenth answer".to_owned());

        let mut other = sample_turn_projection(false);
        other.session_id = SessionId::new("session-b");
        other.turn_id = TurnId::new("runtime-turn-3");
        other.source.source_turn_id = Some(other.turn_id.clone());
        other.user_text = Some("other prompt".to_owned());

        state.apply_turn_projection(second.clone());
        state.apply_turn_projection(other);
        state.apply_turn_projection(tenth.clone());
        state.apply_turn_projection(first.clone());

        let list = state
            .query(&UiCommand::QuerySessionList)
            .expect("session list query");
        match list {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 2);
                let session_a = list
                    .sessions
                    .iter()
                    .find(|session| session.session_id.as_str() == "session-a")
                    .expect("session-a summary");
                assert_eq!(session_a.turn_count, 3);
                assert_eq!(
                    session_a.latest_turn_id.as_ref(),
                    Some(&TurnId::new("runtime-turn-10-r2"))
                );
            }
            other => panic!("unexpected query result: {other:?}"),
        }

        let transcript = state
            .query(&UiCommand::QuerySessionTurns {
                session_id: SessionId::new("session-a"),
            })
            .expect("session turns query");
        match transcript {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.session_id, SessionId::new("session-a"));
                assert_eq!(transcript.turns.len(), 3);
                assert_eq!(
                    transcript.turns[0].turn_id,
                    TurnId::new("runtime-turn-1-r2")
                );
                assert_eq!(
                    transcript.turns[1].turn_id,
                    TurnId::new("runtime-turn-2-r2")
                );
                assert_eq!(
                    transcript.turns[2].turn_id,
                    TurnId::new("runtime-turn-10-r2")
                );
                assert!(
                    transcript
                        .turns
                        .iter()
                        .all(|turn| turn.session_id.as_str() == "session-a")
                );
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }

    #[test]
    fn terminal_result_projection_smoke() {
        let event = ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("ui.protocol"),
            agent_id: AgentId::new("agent-1"),
            status: TerminalStatus::Success,
            summary: "only final text".to_owned(),
        };
        assert_eq!(terminal_text_projection(&event), "only final text");
    }

    #[test]
    fn cancelled_terminal_status_projects_to_public_conversation() {
        let mut projection = sample_turn_projection(false);
        projection.terminal_status = Some(TerminalStatus::Cancelled);
        projection.terminal_text = Some("cancelled by ui command".to_owned());

        let items = public_conversation_items(&projection);
        let terminal = items
            .iter()
            .find(|item| item.kind == UiConversationItemKind::Terminal)
            .expect("terminal item");

        assert_eq!(terminal.status, "cancelled");
        assert_eq!(terminal.body, "cancelled by ui command");
    }

    #[test]
    fn tool_pending_terminal_projects_as_lifecycle_running_not_final_completed() {
        let mut projection = sample_turn_projection(false);
        projection.terminal_status = Some(TerminalStatus::ToolPending);
        projection.terminal_text = Some("Waiting for lifecycle: worker task assigned".to_owned());

        let items = public_conversation_items(&projection);
        let terminal = items
            .iter()
            .find(|item| item.kind == UiConversationItemKind::Terminal)
            .expect("terminal item");

        assert_eq!(terminal.title, "Lifecycle");
        assert_eq!(terminal.status, "running");
        assert_eq!(terminal.body, "Waiting for lifecycle: worker task assigned");
    }

    #[test]
    fn public_conversation_projection_hides_internal_reasoning_usage_and_completion_schema() {
        let mut projection = sample_turn_projection(false);
        projection.text = vec![concat!(
            "Visible answer\n",
            "<freehand_completion>",
            "{\"claim\":\"complete\",\"completion_reason\":\"done\",\"evidence\":\"proof\",\"summary\":\"summary\",\"learned\":\"lesson\"}",
            "</freehand_completion>"
        )
        .to_owned()];
        projection.reasoning = vec!["private chain".to_owned()];
        projection.usage = vec!["input=10 output=5".to_owned()];

        let items = public_conversation_items(&projection);
        let rendered = items
            .iter()
            .map(|item| item.body.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(items[0].kind, UiConversationItemKind::UserText);
        assert_eq!(items[0].body, "run the task");
        assert!(rendered.contains("Visible answer"));
        assert!(rendered.contains("run the task"));
        assert!(!rendered.contains("freehand_completion"));
        assert!(!rendered.contains("private chain"));
        assert!(!rendered.contains("input=10"));

        let public_turn = public_turn_projection(projection);
        assert_eq!(public_turn.public_conversation, items);
    }

    #[test]
    fn tool_summary_carries_tool_call_identity() {
        let projection = sample_turn_projection(false);
        let tool = public_conversation_items(&projection)
            .into_iter()
            .find(|item| item.kind == UiConversationItemKind::ToolSummary)
            .expect("tool item");
        assert_eq!(tool.tool_call_id.as_deref(), Some("tool-1"));
    }

    #[test]
    fn latest_active_turn_and_stream_kind_routing() {
        let mut state = UiProtocolState::default();
        let projection = sample_turn_projection(false);
        state.apply_turn_projection(projection.clone());
        let result = state
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        match result {
            UiQueryResult::Turn(Some(snapshot)) => assert_eq!(snapshot.turn_id, projection.turn_id),
            other => panic!("unexpected query result: {other:?}"),
        }

        let selector = subscription_selector(&UiCommand::SubscribeLatestActiveTurn {
            client: UiClientKind::Cli,
        })
        .expect("selector");
        assert!(subscription_matches(
            &selector,
            &UiProjection::Turn(projection),
            state.latest_active_turn_id.as_ref()
        ));
    }

    #[test]
    fn debug_state_query_and_subscription_smoke() {
        let mut state = UiProtocolState::default();
        let debug = sample_debug_snapshot();
        state.set_debug_state(debug.clone());

        let result = state
            .query(&UiCommand::QueryDebugState {
                turn_id: TurnId::new("turn-1"),
            })
            .expect("query");
        match result {
            UiQueryResult::Debug(Some(snapshot)) => {
                assert_eq!(snapshot.status_text, "planner locked stable prefix");
                assert_eq!(
                    snapshot.detail_lines,
                    vec!["rewrite_mode=ordinary", "rewrite_version=0"]
                );
            }
            other => panic!("unexpected query result: {other:?}"),
        }

        let selector = subscription_selector(&UiCommand::SubscribeDebugState {
            client: UiClientKind::Cli,
            turn_id: TurnId::new("turn-1"),
        })
        .expect("selector");
        assert!(subscription_matches(
            &selector,
            &UiProjection::Debug(debug),
            state.latest_active_turn_id.as_ref()
        ));
    }

    #[test]
    fn checkpoint_summary_query_smoke() {
        let mut state = UiProtocolState::default();
        let snapshot = checkpoint_projection_from_runtime_summary(
            AgentId::new("agent-1"),
            "node-1".to_owned(),
            vec![UiCheckpointSummary {
                checkpoint_id: "checkpoint-1".to_owned(),
                agent_id: AgentId::new("agent-1"),
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                tool_call_id: "tool-1".to_owned(),
                changed_paths: vec!["scratch/file.txt".to_owned()],
                latest_status: "restored".to_owned(),
                latest_detail: None,
                updated_unix_seconds: 42,
            }],
        );
        state.set_checkpoint_snapshot(snapshot.clone());

        let result = state
            .query(&UiCommand::QueryCheckpoints)
            .expect("checkpoint query");
        match result {
            UiQueryResult::Checkpoints(returned) => assert_eq!(returned, snapshot),
            other => panic!("unexpected checkpoint query result: {other:?}"),
        }
    }

    #[test]
    fn command_ingress_rejects_checkpoint_query_route_misuse() {
        let err = accept_command_ingress(&UiCommand::QueryCheckpoints).expect_err("must reject");
        assert_eq!(err, UiProtocolError::IngressCommandKindMismatch);
    }

    #[test]
    fn debug_subscription_rejects_other_turns() {
        let selector = subscription_selector(&UiCommand::SubscribeDebugState {
            client: UiClientKind::WebUi,
            turn_id: TurnId::new("turn-1"),
        })
        .expect("selector");
        let other = DebugStateSnapshot::new(
            freehand_debug::DebugSemanticPosition {
                turn_id: TurnId::new("turn-2"),
                ..sample_debug_snapshot().semantic
            },
            sample_debug_snapshot().scene,
            "planner locked stable prefix",
            vec![
                "rewrite_mode=ordinary".to_owned(),
                "rewrite_version=0".to_owned(),
            ],
        );
        assert!(!subscription_matches(
            &selector,
            &UiProjection::Debug(other),
            None
        ));
    }

    #[test]
    fn debug_receiver_drain_updates_queryable_state() {
        let hub = DebugHub::new(true);
        let receiver = hub.subscribe(4);
        let snapshot = sample_debug_snapshot();
        let event = DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic: snapshot.semantic.clone(),
                scene: snapshot.scene.clone(),
                input_hash: None,
                output_hash: None,
                artifact_path: None,
                timestamp: "2026-06-16T00:00:00Z".to_owned(),
            },
            snapshot: Some(snapshot),
        };
        hub.emit(event).expect("emit");

        let mut state = UiProtocolState::default();
        let applied = state.drain_debug_receiver(&receiver);
        assert_eq!(applied, 1);

        let result = state
            .query(&UiCommand::QueryDebugState {
                turn_id: TurnId::new("turn-1"),
            })
            .expect("query");
        match result {
            UiQueryResult::Debug(Some(snapshot)) => {
                assert_eq!(snapshot.status_text, "planner locked stable prefix");
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }

    #[test]
    fn debug_event_without_snapshot_does_not_update_state() {
        let snapshot = sample_debug_snapshot();
        let event = DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic: snapshot.semantic,
                scene: snapshot.scene,
                input_hash: None,
                output_hash: None,
                artifact_path: None,
                timestamp: "2026-06-16T00:00:00Z".to_owned(),
            },
            snapshot: None,
        };

        let mut state = UiProtocolState::default();
        assert!(!state.apply_debug_event(&event));
        let result = state
            .query(&UiCommand::QueryDebugState {
                turn_id: TurnId::new("turn-1"),
            })
            .expect("query");
        assert_eq!(result, UiQueryResult::Debug(None));
        assert!(debug_projection_from_event(&event).is_none());
    }

    #[test]
    fn command_ingress_accepts_mutation_intent_without_writing_truth() {
        let ack = accept_command_ingress(&UiCommand::SubmitUserInput {
            text: "ship it".to_owned(),
            session_id: None,
            cwd: None,
            metadata: None,
        })
        .expect("ack");
        assert!(ack.accepted);
        assert_eq!(ack.command_kind, "submit_user_input");
        assert_eq!(ack.mutation_authority, "owner_modules");
    }

    #[test]
    fn command_ingress_accepts_rewind_checkpoint() {
        let ack = accept_command_ingress(&UiCommand::RewindCheckpoint {
            checkpoint_id: "checkpoint-1".to_owned(),
        })
        .expect("ack");
        assert!(ack.accepted);
        assert_eq!(ack.command_kind, "rewind_checkpoint");
    }

    #[test]
    fn command_ingress_rejects_empty_checkpoint_id() {
        let err = accept_command_ingress(&UiCommand::RewindCheckpoint {
            checkpoint_id: "   ".to_owned(),
        })
        .expect_err("must reject");
        assert_eq!(err, UiProtocolError::EmptyCheckpointId);
        let rejection = protocol_rejection(err);
        assert_eq!(rejection.code, "empty_checkpoint_id");
    }

    #[test]
    fn command_ingress_rejects_query_commands() {
        let err =
            accept_command_ingress(&UiCommand::QueryLatestActiveTurn).expect_err("must reject");
        assert_eq!(err, UiProtocolError::IngressCommandKindMismatch);
        let rejection = protocol_rejection(err);
        assert_eq!(rejection.code, "ingress_command_kind_mismatch");
    }

    #[test]
    fn command_dispatch_envelope_routes_submit_input_to_reason_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
            text: "run task".to_owned(),
            session_id: None,
            cwd: None,
            metadata: None,
        })
        .expect("envelope");
        assert_eq!(envelope.ingress.command_kind, "submit_user_input");
        assert_eq!(envelope.target_feature_id, "reason.turn");
        assert_eq!(envelope.target_owner_module, "crates/freehand-reason");
    }

    #[test]
    fn command_dispatch_envelope_routes_slave_message_to_node_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::SendDirectMessageToSlave {
            node_id: "node-1".to_owned(),
            text: "ping".to_owned(),
        })
        .expect("envelope");
        assert_eq!(envelope.target_feature_id, "node.master-slave");
        assert_eq!(envelope.target_owner_module, "crates/freehand-node");
    }

    #[test]
    fn command_dispatch_envelope_routes_rewind_checkpoint_to_runtime_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::RewindCheckpoint {
            checkpoint_id: "checkpoint-1".to_owned(),
        })
        .expect("envelope");
        assert_eq!(envelope.ingress.command_kind, "rewind_checkpoint");
        assert_eq!(envelope.target_feature_id, "runtime.checkpoint-rewind");
        assert_eq!(envelope.target_owner_module, "crates/freehand-runtime");
    }

    #[test]
    fn command_dispatch_envelope_routes_session_crud_to_persistence_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::RenameSession {
            session_id: SessionId::new("session-crud"),
            title: "Renamed".to_owned(),
        })
        .expect("envelope");
        assert_eq!(envelope.ingress.command_kind, "rename_session");
        assert_eq!(envelope.target_feature_id, "reason.persistence");
        assert_eq!(envelope.target_owner_module, "crates/freehand-reason");
    }

    #[test]
    fn command_dispatch_envelope_routes_session_rollback_to_persistence_owner() {
        let envelope = build_command_dispatch_envelope(&UiCommand::RollbackLatestSessionTurn {
            session_id: SessionId::new("session-rollback"),
        })
        .expect("envelope");
        assert_eq!(
            envelope.ingress.command_kind,
            "rollback_latest_session_turn"
        );
        assert_eq!(envelope.target_feature_id, "reason.persistence");
        assert_eq!(envelope.target_owner_module, "crates/freehand-reason");

        let err = accept_command_ingress(&UiCommand::RollbackLatestSessionTurn {
            session_id: SessionId::new("   "),
        })
        .expect_err("blank session must be rejected");
        assert_eq!(err, UiProtocolError::EmptySessionId);
    }

    #[test]
    fn session_crud_validation_rejects_empty_title() {
        let err = accept_command_ingress(&UiCommand::RenameSession {
            session_id: SessionId::new("session-crud"),
            title: "   ".to_owned(),
        })
        .expect_err("empty title must fail");
        assert_eq!(err, UiProtocolError::EmptySessionTitle);
        assert_eq!(protocol_rejection(err).code, "empty_session_title");
    }

    #[test]
    fn session_metadata_projection_includes_empty_and_archived_sessions() {
        let mut state = UiProtocolState::default();
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: SessionId::new("session-empty"),
            title: Some("Empty session".to_owned()),
            archived: false,
            cwd: Some("/tmp".to_owned()),
        });
        state.set_session_metadata(UiSessionMetadataProjection {
            session_id: SessionId::new("session-archived"),
            title: Some("Archived session".to_owned()),
            archived: true,
            cwd: None,
        });

        match state
            .query(&UiCommand::QuerySessionList)
            .expect("active list")
        {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 1);
                assert_eq!(list.sessions[0].session_id, SessionId::new("session-empty"));
                assert_eq!(list.sessions[0].title.as_deref(), Some("Empty session"));
                assert!(!list.sessions[0].archived);
                assert_eq!(list.sessions[0].turn_count, 0);
            }
            other => panic!("unexpected query result: {other:?}"),
        }

        match state
            .query(&UiCommand::QueryArchivedSessionList)
            .expect("archived list")
        {
            UiQueryResult::SessionList(list) => {
                assert_eq!(list.sessions.len(), 1);
                assert_eq!(
                    list.sessions[0].session_id,
                    SessionId::new("session-archived")
                );
                assert!(list.sessions[0].archived);
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }

    #[test]
    fn static_dispatch_port_returns_dispatch_receipt() {
        let envelope = build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
            text: "run task".to_owned(),
            session_id: None,
            cwd: None,
            metadata: None,
        })
        .expect("envelope");
        let port = StaticUiCommandDispatchPort::new("queued_by_test_port");
        let receipt = port.dispatch(envelope).expect("receipt");
        assert_eq!(receipt.dispatch_status, "queued_by_test_port");
        assert_eq!(receipt.target_feature_id, "reason.turn");
    }

    #[test]
    fn dispatch_failure_mapping_preserves_retryability() {
        let not_found = dispatch_port_failure(UiCommandDispatchPortError::TargetNotFound(
            "turn-404".to_owned(),
        ));
        assert_eq!(not_found.code, "command_dispatch_target_not_found");
        assert!(!not_found.retryable);

        let unsupported =
            dispatch_port_failure(UiCommandDispatchPortError::Unsupported("resume".to_owned()));
        assert_eq!(unsupported.code, "command_dispatch_unsupported");
        assert!(!unsupported.retryable);
    }

    #[test]
    fn state_subscription_receives_turn_and_debug_updates() {
        let mut state = UiProtocolState::default();
        let mut receiver = state.subscribe();

        let projection = sample_turn_projection(false);
        state.apply_turn_projection(projection.clone());
        let event = receiver.try_recv().expect("turn event");
        assert_eq!(
            event,
            UiSubscriptionEvent {
                projection: UiProjection::Turn(projection.clone()),
                latest_active_turn_id: Some(projection.turn_id.clone()),
            }
        );

        let debug = sample_debug_snapshot();
        state.set_debug_state(debug.clone());
        let event = receiver.try_recv().expect("debug event");
        assert_eq!(
            event,
            UiSubscriptionEvent {
                projection: UiProjection::Debug(debug),
                latest_active_turn_id: Some(projection.turn_id),
            }
        );
    }

    #[test]
    fn task_list_subscription_matches_runtime_projection_only() {
        let selector = subscription_selector(&UiCommand::SubscribeTaskList {
            status: Some("waiting_agent".to_owned()),
            agent_id: Some(AgentId::new("worker-1")),
        })
        .expect("task list selector");
        assert_eq!(selector.stream_kind, UiStreamKind::TaskList);
        assert_eq!(selector.target_turn_id, None);

        let projection = UiTaskListProjection {
            source_agent_id: AgentId::new("master"),
            status_filter: Some("waiting_agent".to_owned()),
            agent_filter: Some(AgentId::new("worker-1")),
            tasks: Vec::new(),
        };
        assert!(subscription_matches(
            &selector,
            &UiProjection::TaskList(projection),
            None,
        ));
        assert!(!subscription_matches(
            &selector,
            &UiProjection::Progress(TaskProgressSnapshot {
                source: UiSource {
                    source_agent_id: AgentId::new("master"),
                    source_node_id: "master-node".to_owned(),
                    source_turn_id: Some(TurnId::new("turn-1")),
                    stream_kind: UiStreamKind::Progress,
                },
                turn_id: TurnId::new("turn-1"),
                status_text: "running".to_owned(),
            }),
            None,
        ));

        let err = UiProtocolState::default()
            .query(&UiCommand::QueryTaskList {
                status: None,
                agent_id: None,
            })
            .expect_err("task query must stay runtime-owned");
        assert_eq!(err, UiProtocolError::StreamKindMismatch);
    }

    #[test]
    fn config_status_query_stays_runtime_owned_and_secret_free() {
        validate_command(&UiCommand::QueryConfigStatus).expect("valid query");
        let ingress_err = accept_command_ingress(&UiCommand::QueryConfigStatus)
            .expect_err("config status query must not enter command ingress");
        assert_eq!(ingress_err, UiProtocolError::IngressCommandKindMismatch);

        let query_err = UiProtocolState::default()
            .query(&UiCommand::QueryConfigStatus)
            .expect_err("config status must stay runtime-owned");
        assert_eq!(query_err, UiProtocolError::StreamKindMismatch);

        let result = UiQueryResult::ConfigStatus(UiConfigStatusProjection {
            agent_name: "master".to_owned(),
            agent_mode: "master".to_owned(),
            node_id: "master-node".to_owned(),
            paired_agents: vec![UiConfigPeerProjection {
                agent_name: "worker".to_owned(),
                agent_mode: "slave".to_owned(),
                node_id: "worker-node".to_owned(),
                provider_id: "minimonth".to_owned(),
                fallback_provider_id: None,
                model_group_id: None,
                local_web_url: None,
            }],
            local_agent_directory: Vec::new(),
            provider_registry: vec![UiProviderConfigSummaryProjection {
                provider_id: "minimonth".to_owned(),
                enabled: true,
                provider_type: "anthropic".to_owned(),
                provider_protocol: "messages".to_owned(),
                provider_base_url: "https://api.example.test/anthropic".to_owned(),
                provider_base_url_host: "api.example.test".to_owned(),
                default_model: "MiniMax-M2".to_owned(),
                provider_web_search: "auto".to_owned(),
                provider_web_search_effective: "unsupported".to_owned(),
                provider_web_search_reason: "unsupported provider/protocol/model".to_owned(),
                provider_auth_type: "apikey".to_owned(),
                provider_auth_source: "env".to_owned(),
            }],
            model_group_registry: vec![UiModelGroupConfigProjection {
                group_id: "default".to_owned(),
                enabled: true,
                label: "Default".to_owned(),
                primary: UiModelRouteProjection {
                    provider_id: "minimonth".to_owned(),
                    model: "MiniMax-M2".to_owned(),
                },
                sub: Some(UiModelRouteProjection {
                    provider_id: "minimonth".to_owned(),
                    model: "MiniMax-sub".to_owned(),
                }),
                search: Some(UiModelRouteProjection {
                    provider_id: "minimonth".to_owned(),
                    model: "MiniMax-search".to_owned(),
                }),
                title: Some(UiModelRouteProjection {
                    provider_id: "minimonth".to_owned(),
                    model: "MiniMax-title".to_owned(),
                }),
                fallback: None,
                load_balance: vec![UiModelWeightedRouteProjection {
                    provider_id: "minimonth".to_owned(),
                    model: "MiniMax-M2".to_owned(),
                    weight: 1,
                }],
            }],
            agent_resource_count: 1,
            agent_resource_limit: 5,
            agent_resource_provider_mode: "shared".to_owned(),
            agent_resource_provider_id: Some("minimonth".to_owned()),
            provider_id: "minimonth".to_owned(),
            fallback_provider_id: None,
            model_group_id: Some("default".to_owned()),
            provider_type: "anthropic".to_owned(),
            provider_protocol: "messages".to_owned(),
            provider_base_url: "https://api.example.test/anthropic".to_owned(),
            provider_base_url_host: "api.example.test".to_owned(),
            default_model: "MiniMax-M2".to_owned(),
            provider_web_search: "auto".to_owned(),
            provider_web_search_effective: "unsupported".to_owned(),
            provider_web_search_reason: "unsupported provider/protocol/model".to_owned(),
            provider_web_search_route_summary: "no hosted web_search route".to_owned(),
            provider_auth_type: "apikey".to_owned(),
            provider_auth_source: "env".to_owned(),
            restart_required_on_change: true,
        });
        let encoded = serde_json::to_string(&result).expect("config status json");
        assert!(encoded.contains("ConfigStatus"));
        assert!(encoded.contains("provider_auth_source"));
        assert!(encoded.contains("provider_registry"));
        assert!(encoded.contains("model_group_registry"));
        assert!(encoded.contains("model_group_id"));
        assert!(encoded.contains("provider_base_url"));
        assert!(encoded.contains("provider_web_search_effective"));
        assert!(encoded.contains("provider_web_search_reason"));
        assert!(encoded.contains("provider_web_search_route_summary"));
        assert!(encoded.contains("agent_resource_count"));
        assert!(encoded.contains("agent_resource_provider_id"));
        assert!(!encoded.contains("api_key"));
        assert!(!encoded.contains("pair_token"));
        assert!(!encoded.contains("secret"));
    }

    #[test]
    fn provider_config_update_routes_to_config_owner_and_rejects_empty_fields() {
        let command = UiCommand::UpdateProviderConfig {
            update: UiProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "minimax".to_owned(),
                provider_type: "openai".to_owned(),
                provider_protocol: "responses".to_owned(),
                base_url: "https://api.minimaxi.com/v1".to_owned(),
                default_model: "MiniMax-M3".to_owned(),
                web_search: "auto".to_owned(),
                api_key_env: "MINIMAX_API_KEY".to_owned(),
            },
        };
        validate_command(&command).expect("valid provider update command");
        let envelope = build_command_dispatch_envelope(&command).expect("dispatch envelope");
        assert_eq!(envelope.target_feature_id, "config.core");
        assert_eq!(envelope.target_owner_module, "crates/freehand-config");
        assert_eq!(envelope.ingress.command_kind, "update_provider_config");

        let err = validate_command(&UiCommand::UpdateProviderConfig {
            update: UiProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "minimax".to_owned(),
                provider_type: "openai".to_owned(),
                provider_protocol: "responses".to_owned(),
                base_url: "https://api.minimaxi.com/v1".to_owned(),
                default_model: String::new(),
                web_search: "auto".to_owned(),
                api_key_env: "MINIMAX_API_KEY".to_owned(),
            },
        })
        .expect_err("empty model rejected");
        assert_eq!(err, UiProtocolError::EmptyProviderDefaultModel);
        assert_eq!(protocol_rejection(err).code, "empty_provider_default_model");
    }

    #[test]
    fn provider_config_update_serialization_does_not_include_secret_field() {
        let command = UiCommand::UpdateProviderConfig {
            update: UiProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "minimax".to_owned(),
                provider_type: "openai".to_owned(),
                provider_protocol: "responses".to_owned(),
                base_url: "https://api.minimaxi.com/v1".to_owned(),
                default_model: "MiniMax-M3".to_owned(),
                web_search: "auto".to_owned(),
                api_key_env: "MINIMAX_API_KEY".to_owned(),
            },
        };
        let encoded = serde_json::to_string(&command).expect("update command json");
        assert!(encoded.contains("UpdateProviderConfig"));
        assert!(encoded.contains("api_key_env"));
        assert!(!encoded.contains("api_key\""));
        assert!(!encoded.contains("secret"));
        assert!(!encoded.contains("sk-"));
    }

    #[test]
    fn provider_config_upsert_and_selection_route_to_config_owner() {
        let upsert = UiCommand::UpsertProviderConfig {
            update: UiProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "cc".to_owned(),
                provider_type: "openai".to_owned(),
                provider_protocol: "responses".to_owned(),
                base_url: "https://api.anyint.ai/openai/v1".to_owned(),
                default_model: "gpt-5.5".to_owned(),
                web_search: "auto".to_owned(),
                api_key_env: "FREEHAND_CC_API_KEY".to_owned(),
            },
        };
        validate_command(&upsert).expect("valid provider upsert");
        let envelope = build_command_dispatch_envelope(&upsert).expect("upsert envelope");
        assert_eq!(envelope.target_feature_id, "config.core");
        assert_eq!(envelope.target_owner_module, "crates/freehand-config");
        assert_eq!(envelope.ingress.command_kind, "upsert_provider_config");

        let selection = UiCommand::UpdateAgentProviderSelection {
            selection: UiAgentProviderSelectionUpdate {
                agent_name: "master".to_owned(),
                provider_id: "cc".to_owned(),
                fallback_provider_id: Some("minimax".to_owned()),
            },
        };
        validate_command(&selection).expect("valid provider selection");
        let envelope = build_command_dispatch_envelope(&selection).expect("selection envelope");
        assert_eq!(envelope.target_feature_id, "config.core");
        assert_eq!(envelope.target_owner_module, "crates/freehand-config");
        assert_eq!(
            envelope.ingress.command_kind,
            "update_agent_provider_selection"
        );

        let empty_selection = validate_command(&UiCommand::UpdateAgentProviderSelection {
            selection: UiAgentProviderSelectionUpdate {
                agent_name: "master".to_owned(),
                provider_id: String::new(),
                fallback_provider_id: None,
            },
        })
        .expect_err("empty provider rejected");
        assert_eq!(empty_selection, UiProtocolError::EmptyProviderId);
        assert_eq!(
            protocol_rejection(empty_selection).code,
            "empty_provider_id"
        );
    }

    #[test]
    fn model_group_upsert_and_selection_route_to_config_owner() {
        let upsert = UiCommand::UpsertModelGroupConfig {
            group: UiModelGroupConfigUpdate {
                agent_name: "master".to_owned(),
                group_id: "research".to_owned(),
                enabled: true,
                label: "Research".to_owned(),
                primary: UiModelRouteUpdate {
                    provider_id: "cc".to_owned(),
                    model: "gpt-5.5-main".to_owned(),
                },
                sub: Some(UiModelRouteUpdate {
                    provider_id: "cc".to_owned(),
                    model: "gpt-5.5-sub".to_owned(),
                }),
                search: Some(UiModelRouteUpdate {
                    provider_id: "cc".to_owned(),
                    model: "gpt-5.5-search".to_owned(),
                }),
                title: Some(UiModelRouteUpdate {
                    provider_id: "minimax".to_owned(),
                    model: "MiniMax-title".to_owned(),
                }),
                fallback: Some(UiModelRouteUpdate {
                    provider_id: "minimax".to_owned(),
                    model: "MiniMax-fallback".to_owned(),
                }),
                load_balance: vec![UiModelWeightedRouteUpdate {
                    provider_id: "cc".to_owned(),
                    model: "gpt-5.5-main".to_owned(),
                    weight: 2,
                }],
            },
        };
        validate_command(&upsert).expect("valid model group upsert");
        let envelope = build_command_dispatch_envelope(&upsert).expect("upsert envelope");
        assert_eq!(envelope.target_feature_id, "config.core");
        assert_eq!(envelope.target_owner_module, "crates/freehand-config");
        assert_eq!(envelope.ingress.command_kind, "upsert_model_group_config");

        let selection = UiCommand::UpdateAgentModelGroupSelection {
            selection: UiAgentModelGroupSelectionUpdate {
                agent_name: "master".to_owned(),
                model_group_id: Some("research".to_owned()),
            },
        };
        validate_command(&selection).expect("valid model group selection");
        let envelope = build_command_dispatch_envelope(&selection).expect("selection envelope");
        assert_eq!(envelope.target_feature_id, "config.core");
        assert_eq!(envelope.target_owner_module, "crates/freehand-config");
        assert_eq!(
            envelope.ingress.command_kind,
            "update_agent_model_group_selection"
        );

        let empty_group = validate_command(&UiCommand::UpsertModelGroupConfig {
            group: UiModelGroupConfigUpdate {
                agent_name: "master".to_owned(),
                group_id: String::new(),
                enabled: true,
                label: String::new(),
                primary: UiModelRouteUpdate {
                    provider_id: "cc".to_owned(),
                    model: "gpt-5.5-main".to_owned(),
                },
                sub: None,
                search: None,
                title: None,
                fallback: None,
                load_balance: Vec::new(),
            },
        })
        .expect_err("empty group rejected");
        assert_eq!(empty_group, UiProtocolError::EmptyModelGroupId);
        assert_eq!(protocol_rejection(empty_group).code, "empty_model_group_id");

        let bad_route = validate_command(&UiCommand::UpsertModelGroupConfig {
            group: UiModelGroupConfigUpdate {
                agent_name: "master".to_owned(),
                group_id: "research".to_owned(),
                enabled: true,
                label: String::new(),
                primary: UiModelRouteUpdate {
                    provider_id: "cc".to_owned(),
                    model: String::new(),
                },
                sub: None,
                search: None,
                title: None,
                fallback: None,
                load_balance: Vec::new(),
            },
        })
        .expect_err("empty route model rejected");
        assert_eq!(bad_route, UiProtocolError::EmptyModelRouteModel);
    }

    #[test]
    fn provider_web_search_test_routes_to_runtime_owner() {
        let command = UiCommand::TestProviderWebSearch {
            provider_id: "minimax".to_owned(),
            query: Some("Freehand provider web_search live capability test".to_owned()),
        };
        validate_command(&command).expect("valid web_search test command");
        let envelope = build_command_dispatch_envelope(&command).expect("dispatch envelope");
        assert_eq!(envelope.target_feature_id, "provider.reason-live-bridge");
        assert_eq!(envelope.target_owner_module, "crates/freehand-runtime");
        assert_eq!(envelope.ingress.command_kind, "test_provider_web_search");

        let err = validate_command(&UiCommand::TestProviderWebSearch {
            provider_id: String::new(),
            query: None,
        })
        .expect_err("empty provider rejected");
        assert_eq!(err, UiProtocolError::EmptyProviderId);
    }

    #[test]
    fn agent_resource_config_update_routes_to_config_owner_and_rejects_out_of_range() {
        let command = UiCommand::UpdateAgentResourceConfig {
            update: UiAgentResourceConfigUpdate {
                agent_name: "master".to_owned(),
                resource_count: 5,
            },
        };
        validate_command(&command).expect("valid Agent resource update");
        let envelope = build_command_dispatch_envelope(&command).expect("dispatch envelope");
        assert_eq!(envelope.target_feature_id, "config.core");
        assert_eq!(envelope.target_owner_module, "crates/freehand-config");
        assert_eq!(
            envelope.ingress.command_kind,
            "update_agent_resource_config"
        );

        for resource_count in [0, 6] {
            let err = validate_command(&UiCommand::UpdateAgentResourceConfig {
                update: UiAgentResourceConfigUpdate {
                    agent_name: "master".to_owned(),
                    resource_count,
                },
            })
            .expect_err("out-of-range resource count rejected");
            assert_eq!(
                err,
                UiProtocolError::AgentResourceCountOutOfRange { resource_count }
            );
            assert_eq!(
                protocol_rejection(err).code,
                "agent_resource_count_out_of_range"
            );
        }

        let encoded = serde_json::to_string(&command).expect("resource update JSON");
        assert!(encoded.contains("UpdateAgentResourceConfig"));
        assert!(encoded.contains("resource_count"));
        assert!(!encoded.contains("provider_api_key"));
    }

    #[test]
    fn incremental_turn_projection_updates_from_shared_contract_events() {
        let mut state = UiProtocolState::default();
        let mut receiver = state.subscribe();

        let semantic = ReasonResp01SemanticEvent {
            session_id: SessionId::new("session-2"),
            turn_id: TurnId::new("turn-2"),
            trace_id: TraceId::new("trace-2"),
            feature_id: FeatureId::new("reason.turn"),
            agent_id: AgentId::new("agent-2"),
            kind: SemanticEventKind::Reasoning,
            content: "step one".to_owned(),
        };
        let projection = state.apply_semantic_event(
            AgentId::new("agent-2"),
            "node-2".to_owned(),
            &semantic,
            false,
        );
        assert_eq!(projection.reasoning, vec!["step one"]);
        let event = receiver.try_recv().expect("semantic publish");
        assert_eq!(event.latest_active_turn_id, Some(TurnId::new("turn-2")));

        let terminal = ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-2"),
            turn_id: TurnId::new("turn-2"),
            trace_id: TraceId::new("trace-2"),
            feature_id: FeatureId::new("reason.turn"),
            agent_id: AgentId::new("agent-2"),
            status: TerminalStatus::Success,
            summary: "done".to_owned(),
        };
        let projection = state.apply_terminal_event(
            AgentId::new("agent-2"),
            "node-2".to_owned(),
            &terminal,
            false,
        );
        assert_eq!(projection.terminal_text.as_deref(), Some("done"));
        let event = receiver.try_recv().expect("terminal publish");
        match event.projection {
            UiProjection::Turn(turn) => {
                assert_eq!(turn.turn_id, TurnId::new("turn-2"));
                assert_eq!(turn.terminal_text.as_deref(), Some("done"));
            }
            other => panic!("unexpected projection: {other:?}"),
        }
    }

    #[test]
    fn adp_request_and_response_frames_roundtrip() {
        let request = UiAdpRequest::Query {
            request_id: "req-1".to_owned(),
            query: UiCommand::QueryConfigStatus,
        };
        let request_json = serde_json::to_string(&request).expect("request json");
        assert!(request_json.contains("\"protocol_version\":3"));
        assert!(request_json.contains("\"kind\":\"query\""));
        assert!(request_json.contains("QueryConfigStatus"));
        let decoded_request: UiAdpRequest =
            serde_json::from_str(&request_json).expect("decoded request");
        assert_eq!(decoded_request, request);

        let handshake = UiAdpRequest::Handshake {
            request_id: "hello-1".to_owned(),
            client_name: "test-client".to_owned(),
            capabilities: vec![UI_ADP_HANDSHAKE_CAPABILITY.to_owned()],
        };
        let handshake_json = serde_json::to_string(&handshake).expect("handshake json");
        assert!(handshake_json.contains("\"kind\":\"handshake\""));
        assert_eq!(
            serde_json::from_str::<UiAdpRequest>(&handshake_json).expect("handshake decode"),
            handshake
        );

        let response = UiAdpResponse::Failure {
            request_id: "req-1".to_owned(),
            failure: UiAdpFailure {
                code: "protocol_mismatch".to_owned(),
                message: "query frame rejected".to_owned(),
                retryable: false,
            },
        };
        let response_json = serde_json::to_string(&response).expect("response json");
        assert!(response_json.contains("\"protocol_version\":3"));
        assert!(response_json.contains("\"kind\":\"failure\""));
        let decoded_response: UiAdpResponse =
            serde_json::from_str(&response_json).expect("decoded response");
        assert_eq!(decoded_response, response);

        let accepted = UiAdpResponse::HandshakeAccepted {
            request_id: "hello-1".to_owned(),
            server_capabilities: adp_server_capabilities(),
        };
        let accepted_json = serde_json::to_string(&accepted).expect("accepted json");
        assert!(accepted_json.contains("\"kind\":\"handshake_accepted\""));
        assert_eq!(
            serde_json::from_str::<UiAdpResponse>(&accepted_json).expect("accepted decode"),
            accepted
        );
    }

    #[test]
    fn adp_frames_require_supported_protocol_version() {
        let missing_version =
            r#"{"kind":"query","request_id":"req-1","query":"QueryConfigStatus"}"#;
        let missing_error = serde_json::from_str::<UiAdpRequest>(missing_version)
            .expect_err("missing protocol_version must fail");
        assert!(missing_error.to_string().contains("protocol_version"));

        let wrong_version = r#"{"kind":"query","protocol_version":99,"request_id":"req-1","query":"QueryConfigStatus"}"#;
        let wrong_error = serde_json::from_str::<UiAdpRequest>(wrong_version)
            .expect_err("wrong protocol_version must fail");
        assert!(
            wrong_error
                .to_string()
                .contains("unsupported ADP protocol_version")
        );
    }

    #[test]
    fn adp_protocol_manifest_covers_all_command_variants() {
        let manifest = adp_protocol_manifest();
        assert_eq!(manifest.protocol_version, UI_ADP_PROTOCOL_VERSION);
        assert_eq!(manifest.handshake_capability, UI_ADP_HANDSHAKE_CAPABILITY);
        assert!(
            manifest
                .request_kinds
                .iter()
                .any(|kind| kind == "handshake")
        );
        assert!(
            manifest
                .response_kinds
                .iter()
                .any(|kind| kind == "handshake_accepted")
        );
        assert_eq!(
            manifest.commands.len(),
            UI_COMMAND_DESCRIPTORS
                .iter()
                .filter(|descriptor| is_public_adp_command_descriptor(descriptor))
                .count()
        );
        let names: std::collections::BTreeSet<_> = manifest
            .commands
            .iter()
            .map(|entry| entry.serde_name.as_str())
            .collect();
        for descriptor in UI_COMMAND_DESCRIPTORS {
            if is_public_adp_command_descriptor(descriptor) {
                assert!(
                    names.contains(descriptor.serde_name),
                    "missing command {}",
                    descriptor.serde_name
                );
            } else {
                assert!(
                    !names.contains(descriptor.serde_name),
                    "internal command {} must not be public ADP",
                    descriptor.serde_name
                );
            }
        }
        let query_master = manifest
            .commands
            .iter()
            .find(|entry| entry.serde_name == "QueryMasterPoll")
            .expect("QueryMasterPoll");
        assert_eq!(query_master.frame_class, UiCommandFrameClass::Query);
        assert!(!names.contains("ClaimNextTask"));
        assert!(!names.contains("ApplyExecutionFact"));
        assert!(!names.contains("RunSchedulerTick"));
        assert!(!names.contains("RunMasterPoll"));
        let js = adp_protocol_webui_module();
        assert!(js.contains("export function adpQueryOf"));
        assert!(js.contains("export function adpCommandOf"));
        assert!(js.contains("\"protocol_version\": 3"));
        assert!(js.contains("QueryConfigStatus"));
        assert!(!js.contains("target_owner_module"));
        assert!(!adp_protocol_manifest_json().contains("target_owner_module"));
        let receipt_response_json = serde_json::to_string(&UiAdpResponse::CommandReceipt {
            request_id: "req-public-receipt".to_owned(),
            receipt: UiCommandDispatchReceipt {
                ingress: UiCommandIngressAck {
                    command_kind: "create_session".to_owned(),
                    accepted: true,
                    status_text: "accepted".to_owned(),
                    mutation_authority: "owner_modules".to_owned(),
                },
                target_feature_id: "reason.persistence".to_owned(),
                dispatch_status: "accepted".to_owned(),
            },
        })
        .expect("ADP receipt response must serialize");
        assert!(!receipt_response_json.contains("target_owner_module"));
        assert!(!receipt_response_json.contains("crates/freehand-"));
    }
}
