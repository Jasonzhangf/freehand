use std::collections::HashMap;

use freehand_v2_contracts::{CapabilityId, CorrelationId, EventId, NodeId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChannelError {
    #[error("endpoint id cannot be empty")]
    EmptyEndpoint,
    #[error("token cannot be empty")]
    EmptyToken,
    #[error("endpoint already registered: {0}")]
    DuplicateEndpoint(String),
    #[error("unknown endpoint: {0}")]
    UnknownEndpoint(String),
    #[error("invalid token for endpoint: {0}")]
    InvalidToken(String),
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(String),
    #[error("unknown session: {0}")]
    UnknownSession(String),
    #[error("session is not open: {0}")]
    SessionClosed(String),
    #[error("stale connection generation")]
    StaleGeneration,
    #[error("session is suspended: {0}")]
    Suspended(String),
    #[error("invalid replay cursor")]
    InvalidCursor,
    #[error("event id generation failed: {0}")]
    EventId(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BearerToken(String);

impl BearerToken {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ChannelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ChannelError::EmptyToken);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointManifest {
    endpoint_id: String,
    node_id: NodeId,
    protocol_version: u32,
    capabilities: Vec<CapabilityId>,
    generation: u64,
}

impl EndpointManifest {
    pub fn endpoint_id(&self) -> &str {
        &self.endpoint_id
    }

    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    pub fn protocol_version(&self) -> u32 {
        self.protocol_version
    }

    pub fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSessionState {
    Open,
    Suspended,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelSession {
    session_id: String,
    endpoint_id: String,
    state: ChannelSessionState,
    generation: u64,
    connection_id: Option<String>,
    event_count: u64,
}

impl ChannelSession {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn endpoint_id(&self) -> &str {
        &self.endpoint_id
    }

    pub fn state(&self) -> ChannelSessionState {
        self.state
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn connection_id(&self) -> Option<&str> {
        self.connection_id.as_deref()
    }

    pub fn event_count(&self) -> u64 {
        self.event_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    connection_id: String,
    session_id: String,
    generation: u64,
}

impl Connection {
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameKind {
    Control,
    Payload,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelFrame {
    seq: u64,
    event_id: EventId,
    kind: FrameKind,
    correlation_id: CorrelationId,
    payload_ref: Option<String>,
    message: Option<String>,
}

impl ChannelFrame {
    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn kind(&self) -> FrameKind {
        self.kind
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    pub fn payload_ref(&self) -> Option<&str> {
        self.payload_ref.as_deref()
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

#[derive(Default)]
pub struct ChannelRegistry {
    endpoints: HashMap<String, EndpointManifest>,
    tokens: HashMap<String, BearerToken>,
    sessions: HashMap<String, ChannelSession>,
    session_frames: HashMap<String, Vec<ChannelFrame>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        endpoint_id: impl Into<String>,
        node_id: NodeId,
        token: BearerToken,
        protocol_version: u32,
        capabilities: Vec<CapabilityId>,
    ) -> Result<EndpointManifest, ChannelError> {
        if protocol_version != 1 {
            return Err(ChannelError::UnsupportedVersion(
                protocol_version.to_string(),
            ));
        }
        let endpoint_id = endpoint_id.into();
        if endpoint_id.is_empty() {
            return Err(ChannelError::EmptyEndpoint);
        }
        if self.endpoints.contains_key(&endpoint_id) {
            return Err(ChannelError::DuplicateEndpoint(endpoint_id));
        }
        let manifest = EndpointManifest {
            endpoint_id: endpoint_id.clone(),
            node_id,
            protocol_version,
            capabilities,
            generation: 1,
        };
        self.endpoints.insert(endpoint_id.clone(), manifest.clone());
        self.tokens.insert(endpoint_id, token);
        Ok(manifest)
    }

    pub fn discover(&self, endpoint_id: &str) -> Option<&EndpointManifest> {
        self.endpoints.get(endpoint_id)
    }

    pub fn publish_change(
        &mut self,
        endpoint_id: &str,
        capabilities: Vec<CapabilityId>,
    ) -> Result<u64, ChannelError> {
        let manifest = self
            .endpoints
            .get_mut(endpoint_id)
            .ok_or_else(|| ChannelError::UnknownEndpoint(endpoint_id.to_owned()))?;
        manifest.capabilities = capabilities;
        manifest.generation += 1;
        Ok(manifest.generation)
    }

    pub fn open_session(
        &mut self,
        session_id: impl Into<String>,
        endpoint_id: &str,
        token: &BearerToken,
    ) -> Result<ChannelSession, ChannelError> {
        let session_id = session_id.into();
        let stored = self
            .tokens
            .get(endpoint_id)
            .ok_or_else(|| ChannelError::UnknownEndpoint(endpoint_id.to_owned()))?;
        if stored != token {
            return Err(ChannelError::InvalidToken(endpoint_id.to_owned()));
        }
        let session = ChannelSession {
            session_id: session_id.clone(),
            endpoint_id: endpoint_id.to_owned(),
            state: ChannelSessionState::Open,
            generation: 1,
            connection_id: None,
            event_count: 0,
        };
        self.sessions.insert(session_id.clone(), session.clone());
        self.session_frames.insert(session_id, Vec::new());
        Ok(session)
    }

    pub fn attach_connection(
        &mut self,
        session_id: &str,
        connection_id: impl Into<String>,
    ) -> Result<Connection, ChannelError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| ChannelError::UnknownSession(session_id.to_owned()))?;
        if session.state != ChannelSessionState::Open {
            return Err(ChannelError::Suspended(session_id.to_owned()));
        }
        let connection_id = connection_id.into();
        let generation = session.generation;
        session.connection_id = Some(connection_id.clone());
        Ok(Connection {
            connection_id,
            session_id: session_id.to_owned(),
            generation,
        })
    }

    pub fn replace_connection(
        &mut self,
        session_id: &str,
        connection_id: impl Into<String>,
    ) -> Result<Connection, ChannelError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| ChannelError::UnknownSession(session_id.to_owned()))?;
        if session.state != ChannelSessionState::Open {
            return Err(ChannelError::Suspended(session_id.to_owned()));
        }
        session.generation += 1;
        let connection_id = connection_id.into();
        let generation = session.generation;
        session.connection_id = Some(connection_id.clone());
        Ok(Connection {
            connection_id,
            session_id: session_id.to_owned(),
            generation,
        })
    }

    pub fn suspend(&mut self, session_id: &str) -> Result<(), ChannelError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| ChannelError::UnknownSession(session_id.to_owned()))?;
        if session.state == ChannelSessionState::Closed {
            return Err(ChannelError::SessionClosed(session_id.to_owned()));
        }
        session.state = ChannelSessionState::Suspended;
        session.connection_id = None;
        Ok(())
    }

    pub fn reattach(
        &mut self,
        session_id: &str,
        connection_id: impl Into<String>,
    ) -> Result<Connection, ChannelError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| ChannelError::UnknownSession(session_id.to_owned()))?;
        if session.state != ChannelSessionState::Suspended {
            return Err(ChannelError::SessionClosed(session_id.to_owned()));
        }
        session.state = ChannelSessionState::Open;
        session.generation += 1;
        let connection_id = connection_id.into();
        session.connection_id = Some(connection_id.clone());
        Ok(Connection {
            connection_id,
            session_id: session_id.to_owned(),
            generation: session.generation,
        })
    }

    pub fn send(
        &mut self,
        session_id: &str,
        kind: FrameKind,
        correlation_id: CorrelationId,
        payload_ref: Option<String>,
        message: Option<String>,
    ) -> Result<ChannelFrame, ChannelError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| ChannelError::UnknownSession(session_id.to_owned()))?;
        if session.state != ChannelSessionState::Open {
            return Err(ChannelError::SessionClosed(session_id.to_owned()));
        }
        let seq = session.event_count;
        session.event_count += 1;
        let event_id = EventId::try_new(format!("channel-{session_id}-{seq}"))
            .map_err(|err| ChannelError::EventId(err.to_string()))?;
        let frame = ChannelFrame {
            seq,
            event_id,
            kind,
            correlation_id,
            payload_ref,
            message,
        };
        self.session_frames
            .get_mut(session_id)
            .expect("session frames exist")
            .push(frame.clone());
        Ok(frame)
    }

    pub fn replay(&self, session_id: &str, cursor: u64) -> Result<Vec<ChannelFrame>, ChannelError> {
        let frames = self
            .session_frames
            .get(session_id)
            .ok_or_else(|| ChannelError::UnknownSession(session_id.to_owned()))?;
        if cursor > frames.len() as u64 {
            return Err(ChannelError::InvalidCursor);
        }
        Ok(frames
            .iter()
            .filter(|frame| frame.seq() >= cursor)
            .cloned()
            .collect())
    }

    pub fn session(&self, session_id: &str) -> Option<&ChannelSession> {
        self.sessions.get(session_id)
    }

    pub fn close(&mut self, session_id: &str) -> Result<(), ChannelError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| ChannelError::UnknownSession(session_id.to_owned()))?;
        session.state = ChannelSessionState::Closed;
        session.connection_id = None;
        Ok(())
    }
}
