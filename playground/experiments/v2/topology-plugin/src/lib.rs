use freehand_v2_contracts::{CapabilityId, NodeId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TopologyError {
    #[error("machine id cannot be empty")]
    EmptyMachine,
    #[error("agent id cannot be empty")]
    EmptyAgent,
    #[error("channel id cannot be empty")]
    EmptyChannel,
    #[error("unknown focus identity: {0}")]
    UnknownFocus(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopologyNode {
    machine_id: String,
    node_id: NodeId,
    agent_id: String,
    channel_id: String,
    capabilities: Vec<CapabilityId>,
}

impl TopologyNode {
    pub fn new(
        machine_id: impl Into<String>,
        node_id: NodeId,
        agent_id: impl Into<String>,
        channel_id: impl Into<String>,
        capabilities: Vec<CapabilityId>,
    ) -> Result<Self, TopologyError> {
        let machine_id = machine_id.into();
        let agent_id = agent_id.into();
        let channel_id = channel_id.into();
        if machine_id.is_empty() {
            return Err(TopologyError::EmptyMachine);
        }
        if agent_id.is_empty() {
            return Err(TopologyError::EmptyAgent);
        }
        if channel_id.is_empty() {
            return Err(TopologyError::EmptyChannel);
        }
        Ok(Self {
            machine_id,
            node_id,
            agent_id,
            channel_id,
            capabilities,
        })
    }

    pub fn machine_id(&self) -> &str {
        &self.machine_id
    }

    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    pub fn channel_id(&self) -> &str {
        &self.channel_id
    }

    pub fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopologyProjection {
    revision: u64,
    nodes: Vec<TopologyNode>,
    focus: Option<String>,
}

impl TopologyProjection {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn nodes(&self) -> &[TopologyNode] {
        &self.nodes
    }

    pub fn focus(&self) -> Option<&str> {
        self.focus.as_deref()
    }
}

#[derive(Default)]
pub struct TopologyPlugin {
    nodes: Vec<TopologyNode>,
    revision: u64,
    focus: Option<String>,
}

impl TopologyPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, nodes: Vec<TopologyNode>) {
        self.nodes = nodes;
        self.revision += 1;
    }

    pub fn reconcile(&mut self, generation: u64, nodes: Vec<TopologyNode>) {
        if generation != self.revision + 1 {
            return;
        }
        self.nodes = nodes;
        self.revision = generation;
    }

    pub fn publish(&self) -> TopologyProjection {
        TopologyProjection {
            revision: self.revision,
            nodes: self.nodes.clone(),
            focus: self.focus.clone(),
        }
    }

    pub fn focus(&mut self, identity: impl Into<String>) -> Result<(), TopologyError> {
        let identity = identity.into();
        if !self
            .nodes
            .iter()
            .any(|n| n.agent_id() == identity || n.channel_id() == identity)
        {
            return Err(TopologyError::UnknownFocus(identity));
        }
        self.focus = Some(identity);
        self.revision += 1;
        Ok(())
    }
}
