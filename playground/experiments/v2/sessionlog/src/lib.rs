use std::collections::HashMap;

use freehand_v2_contracts::{EventId, SessionId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CURRENT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionLogError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("session already exists: {0}")]
    SessionAlreadyExists(String),
    #[error("duplicate event id: {0}")]
    DuplicateEventId(String),
    #[error("sequence gap: expected {expected}, got {actual}")]
    SequenceGap { expected: u64, actual: u64 },
    #[error("invalid cursor for {session_id}: {reason}")]
    InvalidCursor { session_id: String, reason: String },
    #[error("unsupported format version: {0}")]
    UnsupportedFormatVersion(u32),
    #[error("fork inside open turn")]
    ForkInsideOpenTurn,
    #[error("fork from another session")]
    ForkFromAnotherSession,
    #[error("malformed event envelope: {0}")]
    MalformedEnvelope(String),
    #[error("corrupt committed record: {0}")]
    CorruptRecord(String),
    #[error("in-place mutation forbidden: {0}")]
    InPlaceMutationForbidden(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionHeader {
    session_id: SessionId,
    format_version: u32,
    created_at_ms: u64,
    parent_session_id: Option<SessionId>,
    seed_length: Option<u64>,
    runtime_identity: Option<String>,
}

impl SessionHeader {
    pub fn new(
        session_id: SessionId,
        created_at_ms: u64,
        parent_session_id: Option<SessionId>,
        seed_length: Option<u64>,
        runtime_identity: Option<String>,
    ) -> Result<Self, SessionLogError> {
        Self::validate_format(CURRENT_FORMAT_VERSION)?;
        Ok(Self {
            session_id,
            format_version: CURRENT_FORMAT_VERSION,
            created_at_ms,
            parent_session_id,
            seed_length,
            runtime_identity,
        })
    }

    pub fn validate_format(version: u32) -> Result<(), SessionLogError> {
        if version != CURRENT_FORMAT_VERSION {
            return Err(SessionLogError::UnsupportedFormatVersion(version));
        }
        Ok(())
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn parent_session_id(&self) -> Option<&SessionId> {
        self.parent_session_id.as_ref()
    }

    pub fn seed_length(&self) -> Option<u64> {
        self.seed_length
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Input,
    Surface,
    Result,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceOp {
    Replace,
    Reorder,
    Undo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionEvent {
    seq: u64,
    event_id: EventId,
    timestamp_ms: u64,
    kind: EventKind,
    data: String,
    surface_op: Option<SurfaceOp>,
    source_refs: Vec<u64>,
    ignorable: bool,
}

impl SessionEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        seq: u64,
        event_id: EventId,
        timestamp_ms: u64,
        kind: EventKind,
        data: impl Into<String>,
        surface_op: Option<SurfaceOp>,
        source_refs: Vec<u64>,
        ignorable: bool,
    ) -> Self {
        Self {
            seq,
            event_id,
            timestamp_ms,
            kind,
            data: data.into(),
            surface_op,
            source_refs,
            ignorable,
        }
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn kind(&self) -> EventKind {
        self.kind
    }

    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.kind, EventKind::Result | EventKind::Recovery)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCursor {
    session_id: SessionId,
    last_applied_seq: u64,
    format_version: u32,
}

impl SessionCursor {
    pub fn try_new(session_id: SessionId, last_applied_seq: u64) -> Result<Self, SessionLogError> {
        SessionHeader::validate_format(CURRENT_FORMAT_VERSION)?;
        Ok(Self {
            session_id,
            last_applied_seq,
            format_version: CURRENT_FORMAT_VERSION,
        })
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn last_applied_seq(&self) -> u64 {
        self.last_applied_seq
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceNode {
    pub node_id: String,
    pub seq: u64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceProjection {
    pub session_id: SessionId,
    pub nodes: Vec<SurfaceNode>,
    pub generation: u64,
}

#[derive(Debug, Clone, Default)]
pub struct SessionLog {
    sessions: HashMap<SessionId, SessionState>,
}

#[derive(Debug, Clone)]
struct SessionState {
    header: SessionHeader,
    events: Vec<SessionEvent>,
    next_seq: u64,
}

impl SessionLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(
        &mut self,
        session_id: SessionId,
        created_at_ms: u64,
        runtime_identity: Option<String>,
    ) -> Result<SessionCursor, SessionLogError> {
        self.ensure_missing(&session_id)?;
        self.insert_state(SessionHeader::new(
            session_id.clone(),
            created_at_ms,
            None,
            None,
            runtime_identity,
        )?);
        Ok(SessionCursor {
            session_id,
            last_applied_seq: 0,
            format_version: CURRENT_FORMAT_VERSION,
        })
    }

    pub fn create_child_session(
        &mut self,
        child_session_id: SessionId,
        parent_session_id: &SessionId,
        seed_length: u64,
        created_at_ms: u64,
        runtime_identity: Option<String>,
    ) -> Result<SessionCursor, SessionLogError> {
        let parent = self.state(parent_session_id)?;
        let is_closed = parent
            .events
            .last()
            .map(|event| event.is_terminal())
            .unwrap_or(false);
        if !is_closed {
            return Err(SessionLogError::ForkInsideOpenTurn);
        }
        self.ensure_missing(&child_session_id)?;
        self.insert_state_with_next(
            SessionHeader::new(
                child_session_id.clone(),
                created_at_ms,
                Some(parent_session_id.clone()),
                Some(seed_length),
                runtime_identity,
            )?,
            seed_length,
        );
        Ok(SessionCursor {
            session_id: child_session_id,
            last_applied_seq: seed_length,
            format_version: CURRENT_FORMAT_VERSION,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_event(
        &mut self,
        session_id: &SessionId,
        event_id: EventId,
        timestamp_ms: u64,
        kind: EventKind,
        data: impl Into<String>,
        surface_op: Option<SurfaceOp>,
        source_refs: Vec<u64>,
        ignorable: bool,
    ) -> Result<SessionCursor, SessionLogError> {
        let state = self.state_mut(session_id)?;
        let seq = state.next_seq;
        Self::append_raw_event(
            state,
            seq,
            event_id,
            timestamp_ms,
            kind,
            data,
            surface_op,
            source_refs,
            ignorable,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_event_with_seq(
        &mut self,
        session_id: &SessionId,
        seq: u64,
        event_id: EventId,
        timestamp_ms: u64,
        kind: EventKind,
        data: impl Into<String>,
        surface_op: Option<SurfaceOp>,
        source_refs: Vec<u64>,
        ignorable: bool,
    ) -> Result<SessionCursor, SessionLogError> {
        let state = self.state_mut(session_id)?;
        Self::append_raw_event(
            state,
            seq,
            event_id,
            timestamp_ms,
            kind,
            data,
            surface_op,
            source_refs,
            ignorable,
        )
    }

    pub fn read_session(&self, session_id: &SessionId) -> Result<&[SessionEvent], SessionLogError> {
        Ok(&self.state(session_id)?.events)
    }

    pub fn replay_from(
        &self,
        cursor: &SessionCursor,
    ) -> Result<Vec<SessionEvent>, SessionLogError> {
        let state = self.state(cursor.session_id())?;
        if cursor.format_version != CURRENT_FORMAT_VERSION {
            return Err(SessionLogError::UnsupportedFormatVersion(
                cursor.format_version,
            ));
        }
        if cursor.last_applied_seq > state.next_seq {
            return Err(SessionLogError::InvalidCursor {
                session_id: cursor.session_id().as_str().to_owned(),
                reason: "cursor is beyond next durable sequence".to_owned(),
            });
        }
        Ok(state
            .events
            .iter()
            .filter(|event| event.seq >= cursor.last_applied_seq)
            .cloned()
            .collect())
    }

    pub fn derive_surface(
        &self,
        session_id: &SessionId,
    ) -> Result<SurfaceProjection, SessionLogError> {
        let state = self.state(session_id)?;
        let mut nodes = Vec::<SurfaceNode>::new();
        let mut generation = 0_u64;

        for event in &state.events {
            match event.surface_op {
                Some(SurfaceOp::Replace) => {
                    generation += 1;
                    let node_id = format!("seq-{}", event.seq);
                    upsert_node(
                        &mut nodes,
                        SurfaceNode {
                            node_id,
                            seq: event.seq,
                            content: event.data.clone(),
                        },
                    );
                }
                Some(SurfaceOp::Undo) => {
                    generation += 1;
                    nodes.retain(|node| !event.source_refs.contains(&node.seq));
                }
                Some(SurfaceOp::Reorder) => {
                    generation += 1;
                    if !event.source_refs.is_empty() {
                        let ids: Vec<String> = event
                            .source_refs
                            .iter()
                            .map(|seq| format!("seq-{seq}"))
                            .collect();
                        nodes.sort_by_key(|node| {
                            ids.iter()
                                .position(|id| id == &node.node_id)
                                .unwrap_or(usize::MAX)
                        });
                    }
                }
                None if matches!(
                    event.kind(),
                    EventKind::Input | EventKind::Surface | EventKind::Result
                ) =>
                {
                    generation += 1;
                    upsert_node(
                        &mut nodes,
                        SurfaceNode {
                            node_id: format!("seq-{}", event.seq),
                            seq: event.seq,
                            content: event.data.clone(),
                        },
                    );
                }
                None => {}
            }
        }

        Ok(SurfaceProjection {
            session_id: session_id.clone(),
            nodes,
            generation,
        })
    }

    pub fn export_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionEvent>, SessionLogError> {
        self.state(session_id).map(|state| state.events.clone())
    }

    pub fn ensure_format_version(version: u32) -> Result<(), SessionLogError> {
        SessionHeader::validate_format(version)
    }

    fn ensure_missing(&self, session_id: &SessionId) -> Result<(), SessionLogError> {
        if self.sessions.contains_key(session_id) {
            return Err(SessionLogError::SessionAlreadyExists(
                session_id.as_str().to_owned(),
            ));
        }
        Ok(())
    }

    fn state(&self, session_id: &SessionId) -> Result<&SessionState, SessionLogError> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| SessionLogError::SessionNotFound(session_id.as_str().to_owned()))
    }

    fn state_mut(&mut self, session_id: &SessionId) -> Result<&mut SessionState, SessionLogError> {
        self.sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionLogError::SessionNotFound(session_id.as_str().to_owned()))
    }

    fn insert_state(&mut self, header: SessionHeader) {
        self.insert_state_with_next(header, 0);
    }

    fn insert_state_with_next(&mut self, header: SessionHeader, next_seq: u64) {
        let session_id = header.session_id().clone();
        self.sessions.insert(
            session_id,
            SessionState {
                header,
                events: Vec::new(),
                next_seq,
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn append_raw_event(
        state: &mut SessionState,
        seq: u64,
        event_id: EventId,
        timestamp_ms: u64,
        kind: EventKind,
        data: impl Into<String>,
        surface_op: Option<SurfaceOp>,
        source_refs: Vec<u64>,
        ignorable: bool,
    ) -> Result<SessionCursor, SessionLogError> {
        if state.events.iter().any(|event| event.event_id == event_id) {
            return Err(SessionLogError::DuplicateEventId(
                event_id.as_str().to_owned(),
            ));
        }
        if seq != state.next_seq {
            return Err(SessionLogError::SequenceGap {
                expected: state.next_seq,
                actual: seq,
            });
        }
        let event = SessionEvent::new(
            seq,
            event_id,
            timestamp_ms,
            kind,
            data,
            surface_op,
            source_refs,
            ignorable,
        );
        state.events.push(event);
        state.next_seq += 1;
        Ok(SessionCursor {
            session_id: state.header.session_id().clone(),
            last_applied_seq: state.next_seq,
            format_version: CURRENT_FORMAT_VERSION,
        })
    }
}

fn upsert_node(nodes: &mut Vec<SurfaceNode>, node: SurfaceNode) {
    if let Some(existing) = nodes
        .iter_mut()
        .find(|existing| existing.node_id == node.node_id)
    {
        *existing = node;
    } else {
        nodes.push(node);
    }
}
