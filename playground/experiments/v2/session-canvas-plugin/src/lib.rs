use freehand_v2_contracts::{SessionId, TurnId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CanvasError {
    #[error("session id cannot be empty")]
    EmptySession,
    #[error("unknown focus session: {0}")]
    UnknownFocus(String),
    #[error("edge references unknown session")]
    OrphanEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasBand {
    Active,
    Recent,
    History,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasNode {
    session_id: SessionId,
    turn_id: TurnId,
    band: CanvasBand,
    parent_session_id: Option<SessionId>,
}

impl CanvasNode {
    pub fn new(
        session_id: SessionId,
        turn_id: TurnId,
        band: CanvasBand,
        parent_session_id: Option<SessionId>,
    ) -> Self {
        Self {
            session_id,
            turn_id,
            band,
            parent_session_id,
        }
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn turn_id(&self) -> &TurnId {
        &self.turn_id
    }

    pub fn band(&self) -> CanvasBand {
        self.band
    }

    pub fn parent_session_id(&self) -> Option<&SessionId> {
        self.parent_session_id.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasEdge {
    source: SessionId,
    target: SessionId,
}

impl CanvasEdge {
    pub fn new(source: SessionId, target: SessionId) -> Self {
        Self { source, target }
    }

    pub fn source(&self) -> &SessionId {
        &self.source
    }

    pub fn target(&self) -> &SessionId {
        &self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasProjection {
    revision: u64,
    nodes: Vec<CanvasNode>,
    edges: Vec<CanvasEdge>,
    focus: Option<SessionId>,
}

impl CanvasProjection {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn nodes(&self) -> &[CanvasNode] {
        &self.nodes
    }

    pub fn edges(&self) -> &[CanvasEdge] {
        &self.edges
    }

    pub fn focus(&self) -> Option<&SessionId> {
        self.focus.as_ref()
    }
}

#[derive(Default)]
pub struct SessionCanvasPlugin {
    nodes: Vec<CanvasNode>,
    edges: Vec<CanvasEdge>,
    revision: u64,
    focus: Option<SessionId>,
}

impl SessionCanvasPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn derive(
        &mut self,
        nodes: Vec<CanvasNode>,
        edges: Vec<CanvasEdge>,
    ) -> Result<(), CanvasError> {
        let ids: std::collections::HashSet<_> =
            nodes.iter().map(|n| n.session_id().clone()).collect();
        if edges
            .iter()
            .any(|e| !ids.contains(e.source()) || !ids.contains(e.target()))
        {
            return Err(CanvasError::OrphanEdge);
        }
        self.nodes = nodes;
        self.edges = edges;
        self.revision += 1;
        Ok(())
    }

    pub fn publish(&self) -> CanvasProjection {
        CanvasProjection {
            revision: self.revision,
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            focus: self.focus.clone(),
        }
    }

    pub fn focus(&mut self, session_id: SessionId) -> Result<(), CanvasError> {
        if !self.nodes.iter().any(|n| n.session_id() == &session_id) {
            return Err(CanvasError::UnknownFocus(session_id.as_str().to_owned()));
        }
        self.focus = Some(session_id);
        self.revision += 1;
        Ok(())
    }

    pub fn filter(&mut self, band: CanvasBand) -> Vec<CanvasNode> {
        self.nodes
            .iter()
            .filter(|n| n.band() == band)
            .cloned()
            .collect()
    }
}
