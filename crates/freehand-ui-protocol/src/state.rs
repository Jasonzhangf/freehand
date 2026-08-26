use crate::adp_wire::{UiProjection, UiQueryResult, UiSubscriptionEvent};
use crate::dto::*;
use crate::projection::{
    empty_checkpoint_snapshot, fail_waiting_tool_activities, human_friendly_terminal_text,
    merge_hosted_search_activities, preserve_live_activity_on_nonterminal_refresh,
    session_transcript_projection, tool_activity_detail_from_result,
    tool_activity_status_from_result, turn_is_nonterminal, upsert_tool_activity,
};
use freehand_blocks::{project_tool_call_display, project_tool_result_display};
use freehand_contracts::SearchEvidenceTurnDelivery;
use freehand_contracts::{
    AgentId, ErrorErr01RuntimeClassified, ReasonReq04ToolCall, ReasonReq05ToolResultReentry,
    ReasonResp01SemanticEvent, ReasonResp02UsageEvent, ReasonResp03TerminalEvent,
    SemanticEventKind, SessionId, TerminalStatus, TurnId,
};
use freehand_debug::{DebugEvent, DebugStateSnapshot};
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use thiserror::Error;
use tokio::sync::broadcast;

pub struct UiProtocolState {
    pub(crate) latest_active_turn_id: Option<TurnId>,
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
    #[error(
        "model group context window and compaction thresholds must be positive with compaction less than context window"
    )]
    InvalidModelCompactionThreshold,
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
    #[error("session turn page limit must be between 1 and 100")]
    InvalidTurnPageLimit,
    #[error("session turn page cursor is invalid")]
    InvalidTurnPageCursor,
    #[error("session list page limit must be between 1 and 100")]
    InvalidSessionListPageLimit,
    #[error("session list page cursor is invalid")]
    InvalidSessionListPageCursor,
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
        if let Some(cwd) = projection.cwd.clone() {
            self.session_cwds.insert(projection.session_id.clone(), cwd);
        }
        self.turns
            .insert(projection.turn_id.clone(), projection.clone());
        self.advance_latest_active_turn_id(&projection);
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

    pub fn preserve_live_activity_on_page_refresh(
        &self,
        projections: impl IntoIterator<Item = UiTurnProjection>,
    ) -> Vec<UiTurnProjection> {
        let mut projections = projections.into_iter().collect::<Vec<_>>();
        let Some(latest_turn_id) = projections
            .last()
            .map(|projection| projection.turn_id.clone())
        else {
            return projections;
        };
        let Some(previous) = self.turns.get(&latest_turn_id) else {
            return projections;
        };
        if let Some(latest) = projections.last_mut() {
            preserve_live_activity_on_nonterminal_refresh(latest, previous);
        }
        projections
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
        self.advance_latest_active_turn_id(&projection);
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    pub fn apply_search_evidence(
        &mut self,
        source_agent_id: AgentId,
        source_node_id: String,
        delivery: &SearchEvidenceTurnDelivery,
        slave_substream_card: bool,
    ) -> UiTurnProjection {
        let projection = {
            let projection = self.ensure_turn_projection(
                source_agent_id,
                source_node_id,
                &delivery.session_id,
                &delivery.turn_id,
                slave_substream_card,
            );
            projection.search_evidence = Some(UiSearchEvidenceProjection::from(delivery));
            merge_hosted_search_activities(
                &mut projection.tool_activities,
                projection.search_evidence.as_ref(),
            );
            projection.clone()
        };
        self.advance_latest_active_turn_id(&projection);
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
        self.advance_latest_active_turn_id(&projection);
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
        self.advance_latest_active_turn_id(&projection);
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
                "input={} output={} cache_create={} cache_read={} reasoning={}",
                event.usage.total_input_tokens(),
                event.usage.output_tokens,
                event.usage.cache_creation_tokens,
                event.usage.cache_read_tokens,
                event.usage.reasoning_tokens.unwrap_or(0)
            ));
            projection.usage_projection = Some(crate::dto::UiUsageProjection {
                input_tokens: event.usage.total_input_tokens(),
                output_tokens: event.usage.output_tokens,
                total_tokens: event.usage.resolved_total_tokens(),
                reasoning_tokens: event.usage.reasoning_tokens,
                cache_creation_tokens: event.usage.cache_creation_tokens,
                cache_read_tokens: event.usage.cache_read_tokens,
                cache_hit_rate_bps: (event.usage.cache_hit_rate() * 10000.0).round() as u64,
                context_tokens: event.usage.total_input_tokens(),
                compacted_tokens: 0,
                model_label: None,
            });
            projection.model_request = None;
            projection.clone()
        };
        self.advance_latest_active_turn_id(&projection);
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
        self.advance_latest_active_turn_id(&projection);
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
            projection.terminal_text = Some(human_friendly_terminal_text(&projection.text, event));
            projection.model_request = None;
            if event.status == TerminalStatus::Failed {
                fail_waiting_tool_activities(
                    &mut projection.tool_activities,
                    Some(event.summary.clone()),
                );
            }
            projection.clone()
        };
        self.advance_latest_active_turn_id(&projection);
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
        self.advance_latest_active_turn_id(&projection);
        self.publish_projection(UiProjection::Turn(projection.clone()));
        projection
    }

    fn advance_latest_active_turn_id(&mut self, incoming: &UiTurnProjection) {
        let Some(current_turn_id) = self.latest_active_turn_id.as_ref() else {
            self.latest_active_turn_id = Some(incoming.turn_id.clone());
            return;
        };
        let Some(current) = self.turns.get(current_turn_id) else {
            self.latest_active_turn_id = Some(incoming.turn_id.clone());
            return;
        };
        if current.turn_id != incoming.turn_id
            && crate::projection::turn_order_key(&incoming.turn_id)
                < crate::projection::turn_order_key(&current.turn_id)
        {
            return;
        }
        self.latest_active_turn_id = Some(incoming.turn_id.clone());
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
            UiCommand::QuerySessionTurns { session_id } => {
                Ok(UiQueryResult::SessionTurns(session_transcript_projection(
                    session_id,
                    &self.turns,
                    &self.session_cwds,
                    &self.session_metadata,
                )))
            }
            UiCommand::QuerySessionTurnsPage { .. } => Err(UiProtocolError::StreamKindMismatch),
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
                usage_projection: None,
                terminal_status: None,
                terminal_text: None,
                user_options: None,
                errors: Vec::new(),
                search_evidence: None,
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
