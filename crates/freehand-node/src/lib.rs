//! Master/slave node runtime and topology contracts for Freehand.

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

use freehand_contracts::{AgentId, SessionId, TurnId};
use freehand_ui_protocol::{
    NodeStatusSnapshot, TaskProgressSnapshot, UiProjection, UiProtocolState, UiSource,
    UiStreamKind, UiTurnProjection,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterNodeConfig {
    pub node_id: String,
    pub agent_id: AgentId,
    pub paired_slave_node_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlaveNodeConfig {
    pub node_id: String,
    pub agent_id: AgentId,
    pub paired_master_node_id: String,
    pub pair_token: String,
    pub allowed_pair_ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingRequest {
    pub source_node_id: String,
    pub source_ip: Option<String>,
    pub presented_token: String,
    pub transport: PairingTransport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegatedTask {
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub status_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectMessageAck {
    pub source_node_id: String,
    pub target_node_id: String,
    pub echoed_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingTransport {
    WebSocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingState {
    Listening,
    Paired,
    Rejected,
}

impl PairingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Listening => "listening",
            Self::Paired => "paired",
            Self::Rejected => "rejected",
        }
    }

    pub fn healthy(self) -> bool {
        self != Self::Rejected
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NodeRuntimeError {
    #[error("master node id must not be empty")]
    EmptyMasterNodeId,
    #[error("slave node id must not be empty")]
    EmptySlaveNodeId,
    #[error("slave paired master node id must not be empty")]
    EmptyPairedMasterNodeId,
    #[error("master paired slave node id must not be empty")]
    EmptyPairedSlaveNodeId,
    #[error("slave pair token must not be empty")]
    EmptyPairToken,
    #[error("delegated task status text must not be empty")]
    EmptyTaskStatus,
    #[error("direct message text must not be empty")]
    EmptyDirectMessage,
    #[error("unsupported pairing transport")]
    UnsupportedTransport,
    #[error("pairing source node is not allowed")]
    UnauthorizedPairSourceNode,
    #[error("pairing source ip is not allowed")]
    UnauthorizedPairSourceIp,
    #[error("pairing token does not match")]
    PairTokenMismatch,
    #[error("slave is not currently paired")]
    SlaveNotPaired,
}

#[derive(Debug)]
pub struct LocalNodeRuntime {
    master: MasterNodeConfig,
    slave: SlaveNodeConfig,
    slave_pairing_state: PairingState,
    active_pair_source_node_id: Option<String>,
    ui_state: UiProtocolState,
    subscribers: Mutex<Vec<SyncSender<UiProjection>>>,
}

impl LocalNodeRuntime {
    pub fn new(master: MasterNodeConfig, slave: SlaveNodeConfig) -> Result<Self, NodeRuntimeError> {
        if master.node_id.trim().is_empty() {
            return Err(NodeRuntimeError::EmptyMasterNodeId);
        }
        if master.paired_slave_node_id.trim().is_empty() {
            return Err(NodeRuntimeError::EmptyPairedSlaveNodeId);
        }
        if slave.node_id.trim().is_empty() {
            return Err(NodeRuntimeError::EmptySlaveNodeId);
        }
        if slave.paired_master_node_id.trim().is_empty() {
            return Err(NodeRuntimeError::EmptyPairedMasterNodeId);
        }
        if slave.pair_token.trim().is_empty() {
            return Err(NodeRuntimeError::EmptyPairToken);
        }

        let mut runtime = Self {
            master,
            slave,
            slave_pairing_state: PairingState::Listening,
            active_pair_source_node_id: None,
            ui_state: UiProtocolState::default(),
            subscribers: Mutex::new(Vec::new()),
        };
        let listening_snapshot = runtime.slave_status_snapshot(PairingState::Listening);
        runtime.ui_state.set_node_status(listening_snapshot.clone());
        runtime.publish(UiProjection::NodeStatus(listening_snapshot));
        Ok(runtime)
    }

    pub fn subscribe(&self, capacity: usize) -> Receiver<UiProjection> {
        let (sender, receiver) = mpsc::sync_channel(capacity.max(1));
        self.subscribers
            .lock()
            .expect("lock subscribers")
            .push(sender);
        receiver
    }

    pub fn pair_slave(
        &mut self,
        request: PairingRequest,
    ) -> Result<NodeStatusSnapshot, NodeRuntimeError> {
        if request.transport != PairingTransport::WebSocket {
            self.mark_pairing_rejected();
            return Err(NodeRuntimeError::UnsupportedTransport);
        }
        if request.source_node_id != self.slave.paired_master_node_id {
            self.mark_pairing_rejected();
            return Err(NodeRuntimeError::UnauthorizedPairSourceNode);
        }
        if self
            .slave
            .allowed_pair_ip
            .as_ref()
            .is_some_and(|allowed_ip| request.source_ip.as_ref() != Some(allowed_ip))
        {
            self.mark_pairing_rejected();
            return Err(NodeRuntimeError::UnauthorizedPairSourceIp);
        }
        if request.presented_token != self.slave.pair_token {
            self.mark_pairing_rejected();
            return Err(NodeRuntimeError::PairTokenMismatch);
        }

        self.slave_pairing_state = PairingState::Paired;
        self.active_pair_source_node_id = Some(request.source_node_id);
        let snapshot = self.slave_status_snapshot(PairingState::Paired);
        self.ui_state.set_node_status(snapshot.clone());
        self.publish(UiProjection::NodeStatus(snapshot.clone()));
        Ok(snapshot)
    }

    pub fn lose_slave_pairing(&mut self) -> NodeStatusSnapshot {
        self.slave_pairing_state = PairingState::Listening;
        self.active_pair_source_node_id = None;
        let snapshot = self.slave_status_snapshot(PairingState::Listening);
        self.ui_state.set_node_status(snapshot.clone());
        self.publish(UiProjection::NodeStatus(snapshot.clone()));
        snapshot
    }

    pub fn delegate_task(
        &mut self,
        source_node_id: &str,
        task: DelegatedTask,
    ) -> Result<TaskProgressSnapshot, NodeRuntimeError> {
        self.ensure_authorized_slave_source(source_node_id)?;
        if task.status_text.trim().is_empty() {
            return Err(NodeRuntimeError::EmptyTaskStatus);
        }

        let snapshot = TaskProgressSnapshot {
            source: UiSource {
                source_agent_id: self.slave.agent_id.clone(),
                source_node_id: self.slave.node_id.clone(),
                source_turn_id: Some(task.turn_id.clone()),
                stream_kind: UiStreamKind::Progress,
            },
            turn_id: task.turn_id,
            status_text: task.status_text,
        };
        self.ui_state.set_progress(snapshot.clone());
        self.publish(UiProjection::Progress(snapshot.clone()));
        Ok(snapshot)
    }

    pub fn send_direct_message(
        &self,
        source_node_id: &str,
        text: &str,
    ) -> Result<DirectMessageAck, NodeRuntimeError> {
        self.ensure_authorized_slave_source(source_node_id)?;
        if text.trim().is_empty() {
            return Err(NodeRuntimeError::EmptyDirectMessage);
        }

        Ok(DirectMessageAck {
            source_node_id: self.master.node_id.clone(),
            target_node_id: self.slave.node_id.clone(),
            echoed_text: text.to_owned(),
        })
    }

    pub fn publish_slave_turn(
        &mut self,
        source_node_id: &str,
        projection: UiTurnProjection,
    ) -> Result<(), NodeRuntimeError> {
        self.ensure_authorized_slave_source(source_node_id)?;
        self.ui_state.apply_turn_projection(projection.clone());
        self.publish(UiProjection::Turn(projection));
        Ok(())
    }

    pub fn query_node_status(&self) -> Option<NodeStatusSnapshot> {
        self.ui_state
            .query(&freehand_ui_protocol::UiCommand::QueryNodeStatus {
                node_id: self.slave.node_id.clone(),
            })
            .ok()
            .and_then(|result| match result {
                freehand_ui_protocol::UiQueryResult::NodeStatus(snapshot) => snapshot,
                _ => None,
            })
    }

    pub fn query_task_progress(&self, turn_id: &TurnId) -> Option<TaskProgressSnapshot> {
        self.ui_state
            .query(&freehand_ui_protocol::UiCommand::QueryTaskProgress {
                turn_id: turn_id.clone(),
            })
            .ok()
            .and_then(|result| match result {
                freehand_ui_protocol::UiQueryResult::Progress(snapshot) => snapshot,
                _ => None,
            })
    }

    pub fn latest_slave_turn(&self) -> Option<UiTurnProjection> {
        self.ui_state
            .query(&freehand_ui_protocol::UiCommand::QueryLatestActiveTurn)
            .ok()
            .and_then(|result| match result {
                freehand_ui_protocol::UiQueryResult::Turn(snapshot) => snapshot,
                _ => None,
            })
    }

    fn ensure_authorized_slave_source(&self, source_node_id: &str) -> Result<(), NodeRuntimeError> {
        if self.slave_pairing_state != PairingState::Paired {
            return Err(NodeRuntimeError::SlaveNotPaired);
        }
        if self.active_pair_source_node_id.as_deref() != Some(source_node_id) {
            return Err(NodeRuntimeError::UnauthorizedPairSourceNode);
        }
        Ok(())
    }

    fn slave_status_snapshot(&self, pairing_state: PairingState) -> NodeStatusSnapshot {
        NodeStatusSnapshot {
            source: UiSource {
                source_agent_id: self.slave.agent_id.clone(),
                source_node_id: self.slave.node_id.clone(),
                source_turn_id: None,
                stream_kind: UiStreamKind::NodeStatus,
            },
            node_id: self.slave.node_id.clone(),
            healthy: pairing_state.healthy(),
            pairing_state: pairing_state.as_str().to_owned(),
        }
    }

    fn mark_pairing_rejected(&mut self) {
        self.slave_pairing_state = PairingState::Rejected;
        self.active_pair_source_node_id = None;
        let snapshot = self.slave_status_snapshot(PairingState::Rejected);
        self.ui_state.set_node_status(snapshot.clone());
        self.publish(UiProjection::NodeStatus(snapshot));
    }

    fn publish(&self, projection: UiProjection) {
        let mut subscribers = self.subscribers.lock().expect("lock subscribers");
        subscribers.retain(|sender| match sender.try_send(projection.clone()) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        ErrorClass, ErrorContract, FeatureId, ReasonReq04ToolCall, ReasonResp01SemanticEvent,
        ReasonResp02UsageEvent, ReasonResp03TerminalEvent, RecoveryPolicy, SemanticEventKind,
        TerminalStatus, TokenUsage, TraceId,
    };
    use freehand_ui_protocol::{TurnProjectionInput, turn_projection_from_events};

    fn sample_runtime() -> LocalNodeRuntime {
        LocalNodeRuntime::new(
            MasterNodeConfig {
                node_id: "master-node".to_owned(),
                agent_id: AgentId::new("master-agent"),
                paired_slave_node_id: "slave-node".to_owned(),
            },
            SlaveNodeConfig {
                node_id: "slave-node".to_owned(),
                agent_id: AgentId::new("slave-agent"),
                paired_master_node_id: "master-node".to_owned(),
                pair_token: "pair-secret".to_owned(),
                allowed_pair_ip: Some("127.0.0.1".to_owned()),
            },
        )
        .expect("runtime")
    }

    fn pair_request() -> PairingRequest {
        PairingRequest {
            source_node_id: "master-node".to_owned(),
            source_ip: Some("127.0.0.1".to_owned()),
            presented_token: "pair-secret".to_owned(),
            transport: PairingTransport::WebSocket,
        }
    }

    fn sample_slave_turn() -> UiTurnProjection {
        turn_projection_from_events(TurnProjectionInput {
            source_agent_id: AgentId::new("slave-agent"),
            source_node_id: "slave-node".to_owned(),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            user_text: Some("delegate to slave".to_owned()),
            semantic_events: vec![
                ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    trace_id: TraceId::new("trace-1"),
                    feature_id: FeatureId::new("reason.turn"),
                    agent_id: AgentId::new("slave-agent"),
                    kind: SemanticEventKind::Reasoning,
                    content: "thinking".to_owned(),
                },
                ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    trace_id: TraceId::new("trace-1"),
                    feature_id: FeatureId::new("reason.turn"),
                    agent_id: AgentId::new("slave-agent"),
                    kind: SemanticEventKind::Text,
                    content: "answer".to_owned(),
                },
            ],
            tool_calls: vec![ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("reason.turn"),
                agent_id: AgentId::new("slave-agent"),
                tool_call: freehand_contracts::ToolCallContract {
                    tool_call_id: freehand_contracts::ToolCallId::new("tool-1"),
                    tool_name: "search".to_owned(),
                    arguments: vec![],
                    arguments_complete: true,
                },
            }],
            usage_events: vec![ReasonResp02UsageEvent {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("reason.turn"),
                agent_id: AgentId::new("slave-agent"),
                usage: TokenUsage {
                    input_tokens: 10,
                    output_tokens: 4,
                    total_tokens: Some(14),
                    reasoning_tokens: Some(2),
                    cache_creation_tokens: 0,
                    cache_read_tokens: 0,
                    finish_reason: Some("stop".to_owned()),
                },
            }],
            terminal_event: Some(ReasonResp03TerminalEvent {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("reason.turn"),
                agent_id: AgentId::new("slave-agent"),
                status: TerminalStatus::Success,
                summary: "final answer".to_owned(),
            }),
            error_events: vec![freehand_contracts::ErrorErr01RuntimeClassified {
                session_id: Some(SessionId::new("session-1")),
                turn_id: Some(TurnId::new("turn-1")),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("reason.turn"),
                agent_id: Some(AgentId::new("slave-agent")),
                error: ErrorContract {
                    code: "warn".to_owned(),
                    class: ErrorClass::Protocol,
                    recovery: RecoveryPolicy::Recoverable,
                    message: "minor".to_owned(),
                },
            }],
            slave_substream_card: true,
        })
    }

    #[test]
    fn validates_slave_startup_config_permissions() {
        let err = LocalNodeRuntime::new(
            MasterNodeConfig {
                node_id: "master-node".to_owned(),
                agent_id: AgentId::new("master-agent"),
                paired_slave_node_id: "slave-node".to_owned(),
            },
            SlaveNodeConfig {
                node_id: "slave-node".to_owned(),
                agent_id: AgentId::new("slave-agent"),
                paired_master_node_id: "".to_owned(),
                pair_token: "pair-secret".to_owned(),
                allowed_pair_ip: None,
            },
        )
        .expect_err("must fail");
        assert_eq!(err, NodeRuntimeError::EmptyPairedMasterNodeId);
    }

    #[test]
    fn performs_local_websocket_pairing_and_reports_status() {
        let mut runtime = sample_runtime();
        let snapshot = runtime.pair_slave(pair_request()).expect("pair");
        assert!(snapshot.healthy);
        assert_eq!(snapshot.pairing_state, "paired");
        assert_eq!(
            runtime.query_node_status().expect("status").pairing_state,
            "paired"
        );
    }

    #[test]
    fn pairing_failure_is_explicit_and_visible() {
        let mut runtime = sample_runtime();
        let err = runtime
            .pair_slave(PairingRequest {
                presented_token: "wrong".to_owned(),
                ..pair_request()
            })
            .expect_err("must reject");
        assert_eq!(err, NodeRuntimeError::PairTokenMismatch);
        let snapshot = runtime.query_node_status().expect("status");
        assert!(!snapshot.healthy);
        assert_eq!(snapshot.pairing_state, "rejected");
    }

    #[test]
    fn paired_slave_restricts_input_to_authorized_source() {
        let mut runtime = sample_runtime();
        runtime.pair_slave(pair_request()).expect("pair");
        let err = runtime
            .delegate_task(
                "intruder-node",
                DelegatedTask {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    status_text: "accepted".to_owned(),
                },
            )
            .expect_err("must reject");
        assert_eq!(err, NodeRuntimeError::UnauthorizedPairSourceNode);
    }

    #[test]
    fn pairing_loss_returns_slave_to_listening_and_allows_repair() {
        let mut runtime = sample_runtime();
        runtime.pair_slave(pair_request()).expect("pair");
        let snapshot = runtime.lose_slave_pairing();
        assert!(snapshot.healthy);
        assert_eq!(snapshot.pairing_state, "listening");
        let repaired = runtime.pair_slave(pair_request()).expect("repair");
        assert_eq!(repaired.pairing_state, "paired");
    }

    #[test]
    fn master_delegate_and_progress_query_smoke() {
        let mut runtime = sample_runtime();
        runtime.pair_slave(pair_request()).expect("pair");
        let progress = runtime
            .delegate_task(
                "master-node",
                DelegatedTask {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    status_text: "delegated".to_owned(),
                },
            )
            .expect("delegated");
        assert_eq!(progress.status_text, "delegated");
        assert_eq!(
            runtime
                .query_task_progress(&TurnId::new("turn-1"))
                .expect("progress")
                .status_text,
            "delegated"
        );
    }

    #[test]
    fn master_can_subscribe_to_slave_turn_stream() {
        let mut runtime = sample_runtime();
        runtime.pair_slave(pair_request()).expect("pair");
        let receiver = runtime.subscribe(4);
        runtime
            .publish_slave_turn("master-node", sample_slave_turn())
            .expect("publish turn");
        let latest = runtime.latest_slave_turn().expect("latest turn");
        assert_eq!(latest.turn_id, TurnId::new("turn-1"));

        let mut saw_turn = false;
        for _ in 0..2 {
            let event = receiver.recv().expect("projection");
            if let UiProjection::Turn(turn) = event {
                saw_turn = true;
                assert_eq!(turn.source.source_node_id, "slave-node");
                assert!(turn.slave_substream_card);
                break;
            }
        }
        assert!(saw_turn, "expected turn projection on subscription");
    }

    #[test]
    fn direct_message_requires_pairing_and_non_empty_text() {
        let runtime = sample_runtime();
        let err = runtime
            .send_direct_message("master-node", "hello")
            .expect_err("must reject before pair");
        assert_eq!(err, NodeRuntimeError::SlaveNotPaired);

        let mut runtime = sample_runtime();
        runtime.pair_slave(pair_request()).expect("pair");
        let err = runtime
            .send_direct_message("master-node", " ")
            .expect_err("must reject");
        assert_eq!(err, NodeRuntimeError::EmptyDirectMessage);
    }
}
