//! Master/slave node runtime and topology contracts for Freehand.

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

use freehand_contracts::{AgentId, SessionId, TurnId};
use freehand_debug::{
    DebugEvent, DebugHub, DebugScenePosition, DebugSemanticPosition, DebugStateSnapshot,
    DebugTraceEnvelope,
};
use freehand_metadata::{
    MetadataCenter, MetadataEntry, MetadataEnvelope, MetadataError, MetadataId, MetadataKind,
    MetadataSubject, MetadataWriteNode, MetadataWriteOwner,
};
use freehand_ui_protocol::{
    NodeStatusSnapshot, TaskProgressSnapshot, UiProjection, UiProtocolState, UiSource,
    UiStreamKind, UiTurnProjection,
};
use serde_json::json;
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
    #[error("metadata write failed: {0}")]
    MetadataWriteFailed(String),
}

pub struct LocalNodeRuntime {
    master: MasterNodeConfig,
    slave: SlaveNodeConfig,
    slave_pairing_state: PairingState,
    active_pair_source_node_id: Option<String>,
    ui_state: UiProtocolState,
    subscribers: Mutex<Vec<SyncSender<UiProjection>>>,
    debug_hub: Option<std::sync::Arc<DebugHub>>,
    metadata_center: Option<std::sync::Arc<Mutex<MetadataCenter>>>,
}

impl std::fmt::Debug for LocalNodeRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalNodeRuntime")
            .field("master", &self.master)
            .field("slave", &self.slave)
            .field("slave_pairing_state", &self.slave_pairing_state)
            .field(
                "active_pair_source_node_id",
                &self.active_pair_source_node_id,
            )
            .field("ui_state", &self.ui_state)
            .field(
                "subscriber_count",
                &self.subscribers.lock().map(|v| v.len()).ok(),
            )
            .field(
                "debug_hub_enabled",
                &self.debug_hub.as_ref().map(|hub| hub.is_enabled()),
            )
            .field(
                "metadata_center_enabled",
                &self.metadata_center.as_ref().map(|_| true).unwrap_or(false),
            )
            .finish()
    }
}

impl LocalNodeRuntime {
    pub fn new(master: MasterNodeConfig, slave: SlaveNodeConfig) -> Result<Self, NodeRuntimeError> {
        Self::new_inner(master, slave, None, None)
    }

    pub fn with_debug_hub(
        master: MasterNodeConfig,
        slave: SlaveNodeConfig,
        debug_hub: std::sync::Arc<DebugHub>,
    ) -> Result<Self, NodeRuntimeError> {
        Self::new_inner(master, slave, None, Some(debug_hub))
    }

    pub fn with_metadata_center(
        master: MasterNodeConfig,
        slave: SlaveNodeConfig,
        metadata_center: std::sync::Arc<Mutex<MetadataCenter>>,
    ) -> Result<Self, NodeRuntimeError> {
        Self::new_inner(master, slave, Some(metadata_center), None)
    }

    pub fn with_debug_hub_and_metadata_center(
        master: MasterNodeConfig,
        slave: SlaveNodeConfig,
        debug_hub: std::sync::Arc<DebugHub>,
        metadata_center: std::sync::Arc<Mutex<MetadataCenter>>,
    ) -> Result<Self, NodeRuntimeError> {
        Self::new_inner(master, slave, Some(metadata_center), Some(debug_hub))
    }

    fn new_inner(
        master: MasterNodeConfig,
        slave: SlaveNodeConfig,
        metadata_center: Option<std::sync::Arc<Mutex<MetadataCenter>>>,
        debug_hub: Option<std::sync::Arc<DebugHub>>,
    ) -> Result<Self, NodeRuntimeError> {
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
            debug_hub,
            metadata_center,
        };
        let listening_snapshot = runtime.slave_status_snapshot(PairingState::Listening);
        runtime.write_metadata(NodeMetadataWriteSpec {
            metadata_id: format!("{}:bootstrap:listening", runtime.slave.node_id),
            kind: MetadataKind::RuntimeState,
            pipeline_node: "NodeReq01BootstrapListening",
            symbol_path: "LocalNodeRuntime::new_inner",
            trace_id: format!("node-bootstrap:{}:listening", runtime.slave.node_id),
            session_id: None,
            turn_id: None,
            entries: vec![
                MetadataEntry {
                    key: "node.pairing_state".to_owned(),
                    value: json!("listening"),
                },
                MetadataEntry {
                    key: "node.paired_master_node_id".to_owned(),
                    value: json!(runtime.slave.paired_master_node_id),
                },
                MetadataEntry {
                    key: "node.allowed_pair_ip_present".to_owned(),
                    value: json!(runtime.slave.allowed_pair_ip.is_some()),
                },
            ],
        })?;
        runtime.emit_debug(NodeDebugEmitSpec {
            session_id: runtime.synthetic_session_id(),
            turn_id: runtime.synthetic_turn_id("bootstrap-listening"),
            trace_id: runtime.synthetic_trace_id("bootstrap-listening"),
            pipeline_node: "NodeReq01BootstrapListening",
            function: "LocalNodeRuntime::new_inner",
            status_text: "node runtime bootstrapped in listening state",
            detail_lines: vec![
                "pairing_state=listening".to_owned(),
                format!(
                    "allowed_pair_ip_present={}",
                    runtime.slave.allowed_pair_ip.is_some()
                ),
            ],
        });
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
            self.mark_pairing_rejected(&request, "unsupported_transport")?;
            return Err(NodeRuntimeError::UnsupportedTransport);
        }
        if request.source_node_id != self.slave.paired_master_node_id {
            self.mark_pairing_rejected(&request, "unauthorized_source_node")?;
            return Err(NodeRuntimeError::UnauthorizedPairSourceNode);
        }
        if self
            .slave
            .allowed_pair_ip
            .as_ref()
            .is_some_and(|allowed_ip| request.source_ip.as_ref() != Some(allowed_ip))
        {
            self.mark_pairing_rejected(&request, "unauthorized_source_ip")?;
            return Err(NodeRuntimeError::UnauthorizedPairSourceIp);
        }
        if request.presented_token != self.slave.pair_token {
            self.mark_pairing_rejected(&request, "pair_token_mismatch")?;
            return Err(NodeRuntimeError::PairTokenMismatch);
        }

        let snapshot = self.slave_status_snapshot(PairingState::Paired);
        self.write_metadata(NodeMetadataWriteSpec {
            metadata_id: format!(
                "{}:pair:{}:accepted",
                self.slave.node_id, request.source_node_id
            ),
            kind: MetadataKind::Routing,
            pipeline_node: "NodeReq02PairingAccepted",
            symbol_path: "LocalNodeRuntime::pair_slave",
            trace_id: format!(
                "node-pair:{}:{}:accepted",
                self.slave.node_id, request.source_node_id
            ),
            session_id: None,
            turn_id: None,
            entries: vec![
                MetadataEntry {
                    key: "node.pairing_state".to_owned(),
                    value: json!("paired"),
                },
                MetadataEntry {
                    key: "node.source_node_id".to_owned(),
                    value: json!(request.source_node_id),
                },
                MetadataEntry {
                    key: "node.source_ip_present".to_owned(),
                    value: json!(request.source_ip.is_some()),
                },
                MetadataEntry {
                    key: "node.transport".to_owned(),
                    value: json!("websocket"),
                },
                MetadataEntry {
                    key: "node.allowed_pair_ip_filter".to_owned(),
                    value: json!(self.allowed_pair_ip_filter_state(&request)),
                },
            ],
        })?;
        self.emit_debug(NodeDebugEmitSpec {
            session_id: self.synthetic_session_id(),
            turn_id: self.synthetic_turn_id("pair-accepted"),
            trace_id: self.synthetic_trace_id("pair-accepted"),
            pipeline_node: "NodeReq02PairingAccepted",
            function: "LocalNodeRuntime::pair_slave",
            status_text: "node pairing accepted",
            detail_lines: vec![
                format!("source_node_id={}", request.source_node_id),
                format!(
                    "allowed_pair_ip_filter={}",
                    self.allowed_pair_ip_filter_state(&request)
                ),
            ],
        });
        self.slave_pairing_state = PairingState::Paired;
        self.active_pair_source_node_id = Some(request.source_node_id);
        self.ui_state.set_node_status(snapshot.clone());
        self.publish(UiProjection::NodeStatus(snapshot.clone()));
        Ok(snapshot)
    }

    pub fn lose_slave_pairing(&mut self) -> Result<NodeStatusSnapshot, NodeRuntimeError> {
        let previous_source = self.active_pair_source_node_id.clone();
        let snapshot = self.slave_status_snapshot(PairingState::Listening);
        self.write_metadata(NodeMetadataWriteSpec {
            metadata_id: format!("{}:pairing_lost:listening", self.slave.node_id),
            kind: MetadataKind::RuntimeState,
            pipeline_node: "NodeResp03PairingLostListening",
            symbol_path: "LocalNodeRuntime::lose_slave_pairing",
            trace_id: format!("node-pair-loss:{}:listening", self.slave.node_id),
            session_id: None,
            turn_id: None,
            entries: vec![
                MetadataEntry {
                    key: "node.pairing_state".to_owned(),
                    value: json!("listening"),
                },
                MetadataEntry {
                    key: "node.previous_state".to_owned(),
                    value: json!("paired"),
                },
                MetadataEntry {
                    key: "node.previous_source_node_id".to_owned(),
                    value: json!(previous_source),
                },
                MetadataEntry {
                    key: "node.relisten".to_owned(),
                    value: json!(true),
                },
            ],
        })?;
        self.emit_debug(NodeDebugEmitSpec {
            session_id: self.synthetic_session_id(),
            turn_id: self.synthetic_turn_id("pairing-lost"),
            trace_id: self.synthetic_trace_id("pairing-lost"),
            pipeline_node: "NodeResp03PairingLostListening",
            function: "LocalNodeRuntime::lose_slave_pairing",
            status_text: "node pairing lost and listening resumed",
            detail_lines: vec![
                "pairing_state=listening".to_owned(),
                "relisten=true".to_owned(),
            ],
        });
        self.slave_pairing_state = PairingState::Listening;
        self.active_pair_source_node_id = None;
        self.ui_state.set_node_status(snapshot.clone());
        self.publish(UiProjection::NodeStatus(snapshot.clone()));
        Ok(snapshot)
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
        let task_session_id = task.session_id.clone();
        let task_turn_id = task.turn_id.clone();

        let snapshot = TaskProgressSnapshot {
            source: UiSource {
                source_agent_id: self.slave.agent_id.clone(),
                source_node_id: self.slave.node_id.clone(),
                source_turn_id: Some(task_turn_id.clone()),
                stream_kind: UiStreamKind::Progress,
            },
            turn_id: task_turn_id.clone(),
            status_text: task.status_text,
        };
        self.write_metadata(NodeMetadataWriteSpec {
            metadata_id: format!(
                "{}:{}:delegated_task",
                task_session_id.as_str(),
                snapshot.turn_id.as_str()
            ),
            kind: MetadataKind::RuntimeState,
            pipeline_node: "NodeReq04DelegatedTaskAccepted",
            symbol_path: "LocalNodeRuntime::delegate_task",
            trace_id: format!(
                "node-task:{}:{}:delegated",
                task_session_id.as_str(),
                snapshot.turn_id.as_str()
            ),
            session_id: Some(task_session_id.clone()),
            turn_id: Some(snapshot.turn_id.clone()),
            entries: vec![
                MetadataEntry {
                    key: "node.source_node_id".to_owned(),
                    value: json!(source_node_id),
                },
                MetadataEntry {
                    key: "task.status_present".to_owned(),
                    value: json!(true),
                },
                MetadataEntry {
                    key: "task.status_changed".to_owned(),
                    value: json!(true),
                },
            ],
        })?;
        self.emit_debug(NodeDebugEmitSpec {
            session_id: task_session_id.clone(),
            turn_id: snapshot.turn_id.clone(),
            trace_id: freehand_contracts::TraceId::new(format!(
                "node-task:{}:{}:progress",
                task_session_id.as_str(),
                snapshot.turn_id.as_str()
            )),
            pipeline_node: "NodeReq04DelegatedTaskAccepted",
            function: "LocalNodeRuntime::delegate_task",
            status_text: "delegated task progress updated",
            detail_lines: vec![
                format!("source_node_id={source_node_id}"),
                "status_present=true".to_owned(),
                "status_changed=true".to_owned(),
            ],
        });
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
        self.write_metadata(NodeMetadataWriteSpec {
            metadata_id: format!(
                "{}:{}:slave_turn_published",
                projection.session_id.as_str(),
                projection.turn_id.as_str()
            ),
            kind: MetadataKind::RuntimeState,
            pipeline_node: "NodeResp05SlaveTurnPublished",
            symbol_path: "LocalNodeRuntime::publish_slave_turn",
            trace_id: format!(
                "node-slave-turn:{}:{}:published",
                projection.session_id.as_str(),
                projection.turn_id.as_str()
            ),
            session_id: Some(projection.session_id.clone()),
            turn_id: Some(projection.turn_id.clone()),
            entries: vec![
                MetadataEntry {
                    key: "node.source_node_id".to_owned(),
                    value: json!(source_node_id),
                },
                MetadataEntry {
                    key: "turn.reasoning_count".to_owned(),
                    value: json!(projection.reasoning.len()),
                },
                MetadataEntry {
                    key: "turn.text_count".to_owned(),
                    value: json!(projection.text.len()),
                },
                MetadataEntry {
                    key: "turn.tool_call_count".to_owned(),
                    value: json!(projection.tool_calls.len()),
                },
                MetadataEntry {
                    key: "turn.error_count".to_owned(),
                    value: json!(projection.errors.len()),
                },
                MetadataEntry {
                    key: "turn.terminal_status_present".to_owned(),
                    value: json!(projection.terminal_status.is_some()),
                },
                MetadataEntry {
                    key: "turn.slave_substream_card".to_owned(),
                    value: json!(projection.slave_substream_card),
                },
            ],
        })?;
        self.emit_debug(NodeDebugEmitSpec {
            session_id: projection.session_id.clone(),
            turn_id: projection.turn_id.clone(),
            trace_id: freehand_contracts::TraceId::new(format!(
                "node-slave-turn:{}:{}:published",
                projection.session_id.as_str(),
                projection.turn_id.as_str()
            )),
            pipeline_node: "NodeResp05SlaveTurnPublished",
            function: "LocalNodeRuntime::publish_slave_turn",
            status_text: "slave turn projection published",
            detail_lines: vec![
                format!("source_node_id={source_node_id}"),
                format!("tool_call_count={}", projection.tool_calls.len()),
                format!(
                    "terminal_status_present={}",
                    projection.terminal_status.is_some()
                ),
            ],
        });
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

    fn mark_pairing_rejected(
        &mut self,
        request: &PairingRequest,
        reason: &str,
    ) -> Result<(), NodeRuntimeError> {
        let snapshot = self.slave_status_snapshot(PairingState::Rejected);
        self.write_metadata(NodeMetadataWriteSpec {
            metadata_id: format!(
                "{}:pair:{}:rejected",
                self.slave.node_id, request.source_node_id
            ),
            kind: MetadataKind::Routing,
            pipeline_node: "NodeErr02PairingRejected",
            symbol_path: "LocalNodeRuntime::pair_slave",
            trace_id: format!(
                "node-pair:{}:{}:rejected",
                self.slave.node_id, request.source_node_id
            ),
            session_id: None,
            turn_id: None,
            entries: vec![
                MetadataEntry {
                    key: "node.pairing_state".to_owned(),
                    value: json!("rejected"),
                },
                MetadataEntry {
                    key: "node.reject_reason".to_owned(),
                    value: json!(reason),
                },
                MetadataEntry {
                    key: "node.source_node_id".to_owned(),
                    value: json!(request.source_node_id),
                },
                MetadataEntry {
                    key: "node.source_ip_present".to_owned(),
                    value: json!(request.source_ip.is_some()),
                },
                MetadataEntry {
                    key: "node.transport".to_owned(),
                    value: json!("websocket"),
                },
                MetadataEntry {
                    key: "node.presented_token_present".to_owned(),
                    value: json!(!request.presented_token.is_empty()),
                },
                MetadataEntry {
                    key: "node.allowed_pair_ip_filter".to_owned(),
                    value: json!(self.allowed_pair_ip_filter_state(request)),
                },
            ],
        })?;
        self.emit_debug(NodeDebugEmitSpec {
            session_id: self.synthetic_session_id(),
            turn_id: self.synthetic_turn_id("pair-rejected"),
            trace_id: self.synthetic_trace_id("pair-rejected"),
            pipeline_node: "NodeErr02PairingRejected",
            function: "LocalNodeRuntime::pair_slave",
            status_text: "node pairing rejected",
            detail_lines: vec![
                format!("source_node_id={}", request.source_node_id),
                format!("reject_reason={reason}"),
            ],
        });
        self.slave_pairing_state = PairingState::Rejected;
        self.active_pair_source_node_id = None;
        self.ui_state.set_node_status(snapshot.clone());
        self.publish(UiProjection::NodeStatus(snapshot));
        Ok(())
    }

    fn publish(&self, projection: UiProjection) {
        let mut subscribers = self.subscribers.lock().expect("lock subscribers");
        subscribers.retain(|sender| match sender.try_send(projection.clone()) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }

    fn allowed_pair_ip_filter_state(&self, request: &PairingRequest) -> &'static str {
        match self.slave.allowed_pair_ip.as_ref() {
            Some(allowed_ip) if request.source_ip.as_ref() == Some(allowed_ip) => "matched",
            Some(_) => "mismatched",
            None => "open",
        }
    }

    fn write_metadata(&self, spec: NodeMetadataWriteSpec<'_>) -> Result<(), NodeRuntimeError> {
        let Some(center) = &self.metadata_center else {
            return Ok(());
        };
        let envelope = MetadataEnvelope::new(
            MetadataId::new(spec.metadata_id),
            spec.kind,
            MetadataWriteOwner {
                feature_id: freehand_contracts::FeatureId::new("node.master-slave"),
                crate_name: "freehand-node".to_owned(),
                module_path: "freehand_node".to_owned(),
                symbol_path: spec.symbol_path.to_owned(),
            },
            MetadataWriteNode {
                pipeline_node: spec.pipeline_node.to_owned(),
                runtime_node_id: Some(self.slave.node_id.clone()),
            },
            MetadataSubject {
                agent_id: Some(self.slave.agent_id.clone()),
                session_id: spec.session_id,
                turn_id: spec.turn_id,
                trace_id: freehand_contracts::TraceId::new(spec.trace_id),
            },
            spec.entries,
        )
        .map_err(|err: MetadataError| NodeRuntimeError::MetadataWriteFailed(err.to_string()))?;
        center
            .lock()
            .map_err(|err: std::sync::PoisonError<_>| {
                NodeRuntimeError::MetadataWriteFailed(err.to_string())
            })?
            .write(envelope)
            .map_err(|err: MetadataError| NodeRuntimeError::MetadataWriteFailed(err.to_string()))
    }

    fn emit_debug(&self, spec: NodeDebugEmitSpec) {
        let Some(hub) = &self.debug_hub else {
            return;
        };
        let semantic = DebugSemanticPosition {
            feature_id: freehand_contracts::FeatureId::new("node.master-slave"),
            session_id: spec.session_id,
            turn_id: spec.turn_id,
            trace_id: spec.trace_id,
            agent_id: Some(self.slave.agent_id.clone()),
            pipeline_node: Some(spec.pipeline_node.to_owned()),
        };
        let scene = DebugScenePosition {
            crate_name: "freehand-node".to_owned(),
            file: "crates/freehand-node/src/lib.rs".to_owned(),
            function: spec.function.to_owned(),
            line: None,
            artifact_path: None,
            raw_exchange_id: None,
        };
        let snapshot = DebugStateSnapshot::new(
            semantic.clone(),
            scene.clone(),
            spec.status_text,
            spec.detail_lines,
        );
        let event = DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic,
                scene,
                input_hash: None,
                output_hash: None,
                artifact_path: None,
                timestamp: unix_timestamp_string(),
            },
            snapshot: Some(snapshot),
        };
        let _ = hub.emit(event);
    }

    fn synthetic_session_id(&self) -> SessionId {
        SessionId::new(format!("node-session-{}", self.slave.node_id))
    }

    fn synthetic_turn_id(&self, suffix: &str) -> TurnId {
        TurnId::new(format!("node-turn-{}-{suffix}", self.slave.node_id))
    }

    fn synthetic_trace_id(&self, suffix: &str) -> freehand_contracts::TraceId {
        freehand_contracts::TraceId::new(format!("node-trace-{}-{suffix}", self.slave.node_id))
    }
}

struct NodeMetadataWriteSpec<'a> {
    metadata_id: String,
    kind: MetadataKind,
    pipeline_node: &'a str,
    symbol_path: &'a str,
    trace_id: String,
    session_id: Option<SessionId>,
    turn_id: Option<TurnId>,
    entries: Vec<MetadataEntry>,
}

struct NodeDebugEmitSpec {
    session_id: SessionId,
    turn_id: TurnId,
    trace_id: freehand_contracts::TraceId,
    pipeline_node: &'static str,
    function: &'static str,
    status_text: &'static str,
    detail_lines: Vec<String>,
}

fn unix_timestamp_string() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_owned(),
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
    use freehand_debug::{DebugSink, DebugSinkError, DebugSinkKind};
    use freehand_ui_protocol::{TurnProjectionInput, turn_projection_from_events};
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use std::thread;

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

    fn sample_runtime_with_metadata(center: Arc<Mutex<MetadataCenter>>) -> LocalNodeRuntime {
        LocalNodeRuntime::with_metadata_center(
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
            center,
        )
        .expect("runtime")
    }

    fn sample_runtime_with_debug(hub: Arc<DebugHub>) -> LocalNodeRuntime {
        LocalNodeRuntime::with_debug_hub(
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
            hub,
        )
        .expect("runtime")
    }

    struct FailingDebugSink;

    impl DebugSink for FailingDebugSink {
        fn kind(&self) -> DebugSinkKind {
            DebugSinkKind::ReplayCapture
        }

        fn handle(&self, _event: &DebugEvent) -> Result<(), DebugSinkError> {
            Err(DebugSinkError::Io("node debug sink failed".to_owned()))
        }
    }

    fn metadata_entry<'a>(entries: &'a [MetadataEntry], key: &str) -> &'a Value {
        &entries
            .iter()
            .find(|entry| entry.key == key)
            .unwrap_or_else(|| panic!("missing metadata entry `{key}`"))
            .value
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
    fn metadata_bootstrap_writes_listening_state_without_request_content() {
        let center = Arc::new(Mutex::new(MetadataCenter::new()));
        let _runtime = sample_runtime_with_metadata(Arc::clone(&center));

        let records = center.lock().expect("lock center");
        assert_eq!(records.records().len(), 1);
        let record = &records.records()[0];
        assert_eq!(record.owner.feature_id, FeatureId::new("node.master-slave"));
        assert_eq!(record.owner.crate_name, "freehand-node");
        assert_eq!(
            record.write_node.pipeline_node,
            "NodeReq01BootstrapListening"
        );
        assert_eq!(record.subject.agent_id, Some(AgentId::new("slave-agent")));
        assert_eq!(
            metadata_entry(&record.entries, "node.pairing_state"),
            &json!("listening")
        );
        let serialized = serde_json::to_string(record).expect("serialize metadata");
        assert!(!serialized.contains("pair-secret"));
        assert!(!serialized.contains("delegate to slave"));
    }

    #[test]
    fn debug_bootstrap_emits_listening_snapshot_without_secret_leakage() {
        let hub = Arc::new(DebugHub::new(true));
        let receiver = hub.subscribe(1);

        let _runtime = sample_runtime_with_debug(Arc::clone(&hub));

        let event = receiver.recv().expect("bootstrap debug event");
        assert_eq!(
            event.envelope.semantic.feature_id,
            FeatureId::new("node.master-slave")
        );
        assert_eq!(
            event.envelope.semantic.pipeline_node.as_deref(),
            Some("NodeReq01BootstrapListening")
        );
        assert_eq!(event.envelope.scene.function, "LocalNodeRuntime::new_inner");
        assert_eq!(
            event
                .snapshot
                .as_ref()
                .expect("bootstrap snapshot")
                .status_text,
            "node runtime bootstrapped in listening state"
        );
        let serialized = serde_json::to_string(&event).expect("serialize debug event");
        assert!(!serialized.contains("pair-secret"));
        assert!(!serialized.contains("delegate to slave"));
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
    fn pairing_metadata_records_owner_node_and_excludes_token_text() {
        let center = Arc::new(Mutex::new(MetadataCenter::new()));
        let mut runtime = sample_runtime_with_metadata(Arc::clone(&center));

        runtime.pair_slave(pair_request()).expect("pair");

        let records = center.lock().expect("lock center");
        assert_eq!(records.records().len(), 2);
        let record = &records.records()[1];
        assert_eq!(record.write_node.pipeline_node, "NodeReq02PairingAccepted");
        assert_eq!(
            metadata_entry(&record.entries, "node.source_node_id"),
            &json!("master-node")
        );
        assert_eq!(
            metadata_entry(&record.entries, "node.allowed_pair_ip_filter"),
            &json!("matched")
        );
        let serialized = serde_json::to_string(record).expect("serialize metadata");
        assert!(!serialized.contains("pair-secret"));
    }

    #[test]
    fn pairing_rejection_debug_snapshot_excludes_presented_token_text() {
        let hub = Arc::new(DebugHub::new(true));
        let receiver = hub.subscribe(4);
        let mut runtime = sample_runtime_with_debug(Arc::clone(&hub));
        let _ = receiver.recv().expect("bootstrap event");

        let attempted_token = "wrong-secret".to_owned();
        let err = runtime
            .pair_slave(PairingRequest {
                presented_token: attempted_token.clone(),
                ..pair_request()
            })
            .expect_err("must reject");

        assert_eq!(err, NodeRuntimeError::PairTokenMismatch);
        let event = receiver.recv().expect("pair rejection debug event");
        assert_eq!(
            event.envelope.semantic.pipeline_node.as_deref(),
            Some("NodeErr02PairingRejected")
        );
        assert_eq!(
            event.envelope.scene.function,
            "LocalNodeRuntime::pair_slave"
        );
        assert_eq!(
            event
                .snapshot
                .as_ref()
                .expect("pair rejection snapshot")
                .detail_lines,
            vec![
                "source_node_id=master-node".to_owned(),
                "reject_reason=pair_token_mismatch".to_owned(),
            ]
        );
        let serialized = serde_json::to_string(&event).expect("serialize debug event");
        assert!(!serialized.contains(&attempted_token));
    }

    #[test]
    fn pairing_rejects_unauthorized_source_ip_explicitly() {
        let mut runtime = sample_runtime();
        let err = runtime
            .pair_slave(PairingRequest {
                source_ip: Some("10.0.0.8".to_owned()),
                ..pair_request()
            })
            .expect_err("unauthorized ip must reject");

        assert_eq!(err, NodeRuntimeError::UnauthorizedPairSourceIp);
        let snapshot = runtime.query_node_status().expect("status");
        assert!(!snapshot.healthy);
        assert_eq!(snapshot.pairing_state, "rejected");
    }

    #[test]
    fn pairing_rejects_unauthorized_source_node_explicitly() {
        let mut runtime = sample_runtime();
        let err = runtime
            .pair_slave(PairingRequest {
                source_node_id: "intruder-node".to_owned(),
                ..pair_request()
            })
            .expect_err("unauthorized source node must reject");

        assert_eq!(err, NodeRuntimeError::UnauthorizedPairSourceNode);
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
    fn delegated_task_rejects_empty_status_text_explicitly() {
        let mut runtime = sample_runtime();
        runtime.pair_slave(pair_request()).expect("pair");

        let err = runtime
            .delegate_task(
                "master-node",
                DelegatedTask {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    status_text: " ".to_owned(),
                },
            )
            .expect_err("empty task status must reject");

        assert_eq!(err, NodeRuntimeError::EmptyTaskStatus);
        assert!(
            runtime
                .query_task_progress(&TurnId::new("turn-1"))
                .is_none()
        );
    }

    #[test]
    fn delegated_task_metadata_records_progress_without_status_text_leakage() {
        let center = Arc::new(Mutex::new(MetadataCenter::new()));
        let mut runtime = sample_runtime_with_metadata(Arc::clone(&center));
        runtime.pair_slave(pair_request()).expect("pair");
        let status_text = "dispatching work".to_owned();

        runtime
            .delegate_task(
                "master-node",
                DelegatedTask {
                    session_id: SessionId::new("session-1"),
                    turn_id: TurnId::new("turn-1"),
                    status_text: status_text.clone(),
                },
            )
            .expect("delegated");

        let records = center.lock().expect("lock center");
        let record = records
            .records()
            .iter()
            .find(|record| record.write_node.pipeline_node == "NodeReq04DelegatedTaskAccepted")
            .expect("delegated metadata");
        assert_eq!(
            metadata_entry(&record.entries, "task.status_present"),
            &json!(true)
        );
        let serialized = serde_json::to_string(record).expect("serialize metadata");
        assert!(!serialized.contains(&status_text));
    }

    #[test]
    fn debug_sink_failure_is_observable_without_blocking_node_truth() {
        let hub = Arc::new(DebugHub::new(true));
        let receiver = hub.subscribe(4);
        let failure_receiver = hub.subscribe_failures(4);
        hub.add_sink(FailingDebugSink);
        let mut runtime = sample_runtime_with_debug(Arc::clone(&hub));

        let bootstrap_event = receiver.recv().expect("bootstrap event");
        let bootstrap_failure = failure_receiver.recv().expect("bootstrap failure");
        assert_eq!(bootstrap_failure.sink_kind, DebugSinkKind::ReplayCapture);
        assert_eq!(
            bootstrap_failure
                .event_envelope
                .semantic
                .pipeline_node
                .as_deref(),
            Some("NodeReq01BootstrapListening")
        );
        assert_eq!(
            bootstrap_event.envelope.semantic.pipeline_node.as_deref(),
            Some("NodeReq01BootstrapListening")
        );
        assert_eq!(
            runtime
                .query_node_status()
                .expect("bootstrap status")
                .pairing_state,
            "listening"
        );

        let paired = runtime
            .pair_slave(pair_request())
            .expect("pair still succeeds");
        assert_eq!(paired.pairing_state, "paired");
        let pair_event = receiver.recv().expect("pair event");
        let pair_failure = failure_receiver.recv().expect("pair failure");
        assert_eq!(
            pair_event.envelope.semantic.pipeline_node.as_deref(),
            Some("NodeReq02PairingAccepted")
        );
        assert_eq!(
            pair_failure
                .event_envelope
                .semantic
                .pipeline_node
                .as_deref(),
            Some("NodeReq02PairingAccepted")
        );
        assert_eq!(pair_failure.message, "io failure: node debug sink failed");
        assert_eq!(
            runtime
                .query_node_status()
                .expect("paired status")
                .pairing_state,
            "paired"
        );
    }

    #[test]
    fn pairing_loss_returns_slave_to_listening_and_allows_repair() {
        let mut runtime = sample_runtime();
        runtime.pair_slave(pair_request()).expect("pair");
        let snapshot = runtime.lose_slave_pairing().expect("lose pairing");
        assert!(snapshot.healthy);
        assert_eq!(snapshot.pairing_state, "listening");
        let repaired = runtime.pair_slave(pair_request()).expect("repair");
        assert_eq!(repaired.pairing_state, "paired");
    }

    #[test]
    fn metadata_write_failure_blocks_rejected_status_materialization() {
        let center = Arc::new(Mutex::new(MetadataCenter::new()));
        let mut runtime = sample_runtime_with_metadata(Arc::clone(&center));
        let poison_center = Arc::clone(&center);
        let _ = thread::spawn(move || {
            let _guard = poison_center.lock().expect("lock for poison");
            panic!("poison metadata center");
        })
        .join();

        let err = runtime
            .pair_slave(PairingRequest {
                source_ip: Some("10.0.0.8".to_owned()),
                ..pair_request()
            })
            .expect_err("metadata write must fail before rejected status");

        assert!(matches!(err, NodeRuntimeError::MetadataWriteFailed(_)));
        let snapshot = runtime.query_node_status().expect("status");
        assert!(snapshot.healthy);
        assert_eq!(snapshot.pairing_state, "listening");
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
    fn slave_turn_metadata_records_projection_flags_without_turn_text() {
        let center = Arc::new(Mutex::new(MetadataCenter::new()));
        let mut runtime = sample_runtime_with_metadata(Arc::clone(&center));
        runtime.pair_slave(pair_request()).expect("pair");

        runtime
            .publish_slave_turn("master-node", sample_slave_turn())
            .expect("publish turn");

        let records = center.lock().expect("lock center");
        let record = records
            .records()
            .iter()
            .find(|record| record.write_node.pipeline_node == "NodeResp05SlaveTurnPublished")
            .expect("slave turn metadata");
        assert_eq!(
            metadata_entry(&record.entries, "turn.slave_substream_card"),
            &json!(true)
        );
        assert_eq!(
            metadata_entry(&record.entries, "turn.terminal_status_present"),
            &json!(true)
        );
        let serialized = serde_json::to_string(record).expect("serialize metadata");
        assert!(!serialized.contains("delegate to slave"));
        assert!(!serialized.contains("final answer"));
    }

    #[test]
    fn slave_turn_debug_snapshot_omits_user_and_terminal_text() {
        let hub = Arc::new(DebugHub::new(true));
        let receiver = hub.subscribe(8);
        let mut runtime = sample_runtime_with_debug(Arc::clone(&hub));
        let _ = receiver.recv().expect("bootstrap event");
        runtime.pair_slave(pair_request()).expect("pair");
        let _ = receiver.recv().expect("pair event");

        runtime
            .publish_slave_turn("master-node", sample_slave_turn())
            .expect("publish turn");

        let event = receiver.recv().expect("slave turn debug event");
        assert_eq!(
            event.envelope.semantic.pipeline_node.as_deref(),
            Some("NodeResp05SlaveTurnPublished")
        );
        assert_eq!(
            event
                .snapshot
                .as_ref()
                .expect("slave turn snapshot")
                .detail_lines,
            vec![
                "source_node_id=master-node".to_owned(),
                "tool_call_count=1".to_owned(),
                "terminal_status_present=true".to_owned(),
            ]
        );
        let serialized = serde_json::to_string(&event).expect("serialize debug event");
        assert!(!serialized.contains("delegate to slave"));
        assert!(!serialized.contains("thinking"));
        assert!(!serialized.contains("answer"));
        assert!(!serialized.contains("final answer"));
    }

    #[test]
    fn publish_slave_turn_requires_authorized_pairing_source() {
        let mut runtime = sample_runtime();
        let err = runtime
            .publish_slave_turn("master-node", sample_slave_turn())
            .expect_err("must reject before pair");
        assert_eq!(err, NodeRuntimeError::SlaveNotPaired);
        assert!(runtime.latest_slave_turn().is_none());

        runtime.pair_slave(pair_request()).expect("pair");
        let err = runtime
            .publish_slave_turn("intruder-node", sample_slave_turn())
            .expect_err("intruder must reject");
        assert_eq!(err, NodeRuntimeError::UnauthorizedPairSourceNode);
        assert!(runtime.latest_slave_turn().is_none());
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
