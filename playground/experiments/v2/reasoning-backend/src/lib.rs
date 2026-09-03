use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use freehand_v2_contracts::{CorrelationId, EventId, ImmutablePayload, SessionId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReasoningError {
    #[error("{type_name} cannot be empty")]
    EmptyId { type_name: &'static str },
    #[error("unknown runtime group: {0}")]
    UnknownRuntimeGroup(String),
    #[error("runtime group already bound: {0}")]
    RuntimeGroupAlreadyBound(String),
    #[error("no active backend for runtime group: {0}")]
    NoActiveBackend(String),
    #[error("backend has in-flight reasoning for group: {0}")]
    BackendInFlight(String),
    #[error("unknown reasoning session: {0}")]
    UnknownSession(String),
    #[error("stale backend generation: expected {expected}, got {actual}")]
    StaleGeneration { expected: u64, actual: u64 },
    #[error("backend failure: {0}")]
    BackendFailure(String),
    #[error("open code state cannot be reconciled: {0}")]
    OpenCodeStateNotReconcilable(String),
    #[error("invalid reasoning cursor: {0}")]
    InvalidCursor(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct BackendId(String);

impl BackendId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ReasoningError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ReasoningError::EmptyId {
                type_name: "BackendId",
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for BackendId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct RuntimeGroupId(String);

impl RuntimeGroupId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ReasoningError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ReasoningError::EmptyId {
                type_name: "RuntimeGroupId",
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RuntimeGroupId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendCapability {
    backend_id: BackendId,
    provider: String,
    protocol_version: u32,
    can_resume: bool,
    can_subscribe: bool,
}

impl BackendCapability {
    pub fn new(
        backend_id: BackendId,
        provider: impl Into<String>,
        protocol_version: u32,
        can_resume: bool,
        can_subscribe: bool,
    ) -> Self {
        Self {
            backend_id,
            provider: provider.into(),
            protocol_version,
            can_resume,
            can_subscribe,
        }
    }

    pub fn backend_id(&self) -> &BackendId {
        &self.backend_id
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn protocol_version(&self) -> u32 {
        self.protocol_version
    }

    pub fn can_resume(&self) -> bool {
        self.can_resume
    }

    pub fn can_subscribe(&self) -> bool {
        self.can_subscribe
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningState {
    Idle,
    Starting,
    Running,
    Waiting,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningEventKind {
    Started,
    Delta,
    Response,
    Interrupted,
    Failed,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasoningRequest {
    session_id: SessionId,
    correlation_id: CorrelationId,
    payload: Arc<ImmutablePayload>,
    resume_from: Option<ReasoningCursor>,
}

impl ReasoningRequest {
    pub fn new(
        session_id: SessionId,
        correlation_id: CorrelationId,
        payload: Arc<ImmutablePayload>,
        resume_from: Option<ReasoningCursor>,
    ) -> Self {
        Self {
            session_id,
            correlation_id,
            payload,
            resume_from,
        }
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    pub fn payload(&self) -> &Arc<ImmutablePayload> {
        &self.payload
    }

    pub fn resume_from(&self) -> Option<&ReasoningCursor> {
        self.resume_from.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningCursor {
    session_id: SessionId,
    event_id: EventId,
    backend_id: BackendId,
    generation: u64,
    last_seq: u64,
}

impl ReasoningCursor {
    pub fn try_new(
        session_id: SessionId,
        event_id: EventId,
        backend_id: BackendId,
        generation: u64,
        last_seq: u64,
    ) -> Result<Self, ReasoningError> {
        if last_seq == 0 {
            return Err(ReasoningError::InvalidCursor(
                "cursor last_seq must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            session_id,
            event_id,
            backend_id,
            generation,
            last_seq,
        })
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn backend_id(&self) -> &BackendId {
        &self.backend_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasoningEvent {
    event_id: EventId,
    session_id: SessionId,
    correlation_id: CorrelationId,
    kind: ReasoningEventKind,
    backend_id: BackendId,
    generation: u64,
    payload: Arc<ImmutablePayload>,
}

impl ReasoningEvent {
    pub fn new(
        event_id: EventId,
        session_id: SessionId,
        correlation_id: CorrelationId,
        kind: ReasoningEventKind,
        backend_id: BackendId,
        generation: u64,
        payload: Arc<ImmutablePayload>,
    ) -> Self {
        Self {
            event_id,
            session_id,
            correlation_id,
            kind,
            backend_id,
            generation,
            payload,
        }
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    pub fn kind(&self) -> ReasoningEventKind {
        self.kind
    }

    pub fn backend_id(&self) -> &BackendId {
        &self.backend_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn payload(&self) -> &Arc<ImmutablePayload> {
        &self.payload
    }
}

pub trait ReasoningBackend {
    fn backend_id(&self) -> &BackendId;

    fn capability(&self) -> BackendCapability;

    fn start(
        &self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
        payload: &Arc<ImmutablePayload>,
        generation: u64,
    ) -> Result<ReasoningEvent, ReasoningError>;

    fn resume(
        &self,
        cursor: &ReasoningCursor,
        correlation_id: &CorrelationId,
        payload: &Arc<ImmutablePayload>,
    ) -> Result<ReasoningEvent, ReasoningError>;

    fn interrupt(
        &self,
        session_id: &SessionId,
        generation: u64,
    ) -> Result<ReasoningEvent, ReasoningError>;

    fn inspect(&self, session_id: &SessionId) -> Result<ReasoningState, ReasoningError>;

    fn subscribe(
        &self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
    ) -> Result<ReasoningEvent, ReasoningError>;
}

pub struct NativeBackend {
    id: BackendId,
    sessions: RefCell<HashMap<SessionId, u64>>,
    next_seq: RefCell<u64>,
}

impl NativeBackend {
    pub fn new() -> Result<Self, ReasoningError> {
        Ok(Self {
            id: BackendId::try_new("freehand-native")?,
            sessions: RefCell::default(),
            next_seq: RefCell::new(0),
        })
    }

    fn event(
        &self,
        kind: ReasoningEventKind,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
        generation: u64,
        payload: &Arc<ImmutablePayload>,
    ) -> ReasoningEvent {
        let mut seq = self.next_seq.borrow_mut();
        *seq += 1;
        ReasoningEvent::new(
            EventId::try_new(format!("native-ev-{}", *seq)).expect("event id"),
            session_id.clone(),
            correlation_id.clone(),
            kind,
            self.id.clone(),
            generation,
            Arc::clone(payload),
        )
    }
}

impl ReasoningBackend for NativeBackend {
    fn backend_id(&self) -> &BackendId {
        &self.id
    }

    fn capability(&self) -> BackendCapability {
        BackendCapability::new(self.id.clone(), "freehand-native", 1, true, true)
    }

    fn start(
        &self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
        payload: &Arc<ImmutablePayload>,
        generation: u64,
    ) -> Result<ReasoningEvent, ReasoningError> {
        self.sessions
            .borrow_mut()
            .insert(session_id.clone(), generation);
        Ok(self.event(
            ReasoningEventKind::Started,
            session_id,
            correlation_id,
            generation,
            payload,
        ))
    }

    fn resume(
        &self,
        cursor: &ReasoningCursor,
        correlation_id: &CorrelationId,
        payload: &Arc<ImmutablePayload>,
    ) -> Result<ReasoningEvent, ReasoningError> {
        let expected = self
            .sessions
            .borrow()
            .get(cursor.session_id())
            .copied()
            .ok_or_else(|| {
                ReasoningError::UnknownSession(cursor.session_id().as_str().to_owned())
            })?;
        if expected != cursor.generation() {
            return Err(ReasoningError::StaleGeneration {
                expected,
                actual: cursor.generation(),
            });
        }
        Ok(self.event(
            ReasoningEventKind::Delta,
            cursor.session_id(),
            correlation_id,
            cursor.generation(),
            payload,
        ))
    }

    fn interrupt(
        &self,
        session_id: &SessionId,
        generation: u64,
    ) -> Result<ReasoningEvent, ReasoningError> {
        let expected = self
            .sessions
            .borrow()
            .get(session_id)
            .copied()
            .ok_or_else(|| ReasoningError::UnknownSession(session_id.as_str().to_owned()))?;
        if expected != generation {
            return Err(ReasoningError::StaleGeneration {
                expected,
                actual: generation,
            });
        }
        self.sessions.borrow_mut().remove(session_id);
        Ok(self.event(
            ReasoningEventKind::Interrupted,
            session_id,
            &CorrelationId::try_new("interrupt").expect("correlation id"),
            generation,
            &Arc::new(ImmutablePayload::new("interrupted").expect("payload")),
        ))
    }

    fn inspect(&self, session_id: &SessionId) -> Result<ReasoningState, ReasoningError> {
        if self.sessions.borrow().contains_key(session_id) {
            Ok(ReasoningState::Running)
        } else {
            Ok(ReasoningState::Idle)
        }
    }

    fn subscribe(
        &self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
    ) -> Result<ReasoningEvent, ReasoningError> {
        let generation = self
            .sessions
            .borrow()
            .get(session_id)
            .copied()
            .ok_or_else(|| ReasoningError::UnknownSession(session_id.as_str().to_owned()))?;
        Ok(self.event(
            ReasoningEventKind::Response,
            session_id,
            correlation_id,
            generation,
            &Arc::new(ImmutablePayload::new("native:response").expect("payload")),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenCodeState {
    session_id: String,
    provider_generation: u64,
}

pub struct OpenCodeBackend {
    id: BackendId,
    state: RefCell<HashMap<SessionId, OpenCodeState>>,
    next_seq: RefCell<u64>,
}

impl OpenCodeBackend {
    pub fn new() -> Result<Self, ReasoningError> {
        Ok(Self {
            id: BackendId::try_new("opencode-adaptor")?,
            state: RefCell::default(),
            next_seq: RefCell::new(0),
        })
    }

    fn event(
        &self,
        kind: ReasoningEventKind,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
        generation: u64,
        payload: &Arc<ImmutablePayload>,
    ) -> ReasoningEvent {
        let mut seq = self.next_seq.borrow_mut();
        *seq += 1;
        ReasoningEvent::new(
            EventId::try_new(format!("opencode-ev-{}", *seq)).expect("event id"),
            session_id.clone(),
            correlation_id.clone(),
            kind,
            self.id.clone(),
            generation,
            Arc::clone(payload),
        )
    }
}

impl ReasoningBackend for OpenCodeBackend {
    fn backend_id(&self) -> &BackendId {
        &self.id
    }

    fn capability(&self) -> BackendCapability {
        BackendCapability::new(self.id.clone(), "opencode-adaptor", 1, true, true)
    }

    fn start(
        &self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
        payload: &Arc<ImmutablePayload>,
        generation: u64,
    ) -> Result<ReasoningEvent, ReasoningError> {
        self.state.borrow_mut().insert(
            session_id.clone(),
            OpenCodeState {
                session_id: session_id.as_str().to_owned(),
                provider_generation: generation,
            },
        );
        Ok(self.event(
            ReasoningEventKind::Started,
            session_id,
            correlation_id,
            generation,
            payload,
        ))
    }

    fn resume(
        &self,
        cursor: &ReasoningCursor,
        correlation_id: &CorrelationId,
        payload: &Arc<ImmutablePayload>,
    ) -> Result<ReasoningEvent, ReasoningError> {
        let state = self.state.borrow();
        let state = state.get(cursor.session_id()).ok_or_else(|| {
            ReasoningError::OpenCodeStateNotReconcilable(
                "no OpenCode session recorded for cursor".to_owned(),
            )
        })?;
        if state.provider_generation != cursor.generation() {
            return Err(ReasoningError::StaleGeneration {
                expected: state.provider_generation,
                actual: cursor.generation(),
            });
        }
        Ok(self.event(
            ReasoningEventKind::Delta,
            cursor.session_id(),
            correlation_id,
            cursor.generation(),
            payload,
        ))
    }

    fn interrupt(
        &self,
        session_id: &SessionId,
        generation: u64,
    ) -> Result<ReasoningEvent, ReasoningError> {
        let state = self.state.borrow();
        let state = state
            .get(session_id)
            .ok_or_else(|| ReasoningError::UnknownSession(session_id.as_str().to_owned()))?;
        if state.provider_generation != generation {
            return Err(ReasoningError::StaleGeneration {
                expected: state.provider_generation,
                actual: generation,
            });
        }
        self.state.borrow_mut().remove(session_id);
        Ok(self.event(
            ReasoningEventKind::Interrupted,
            session_id,
            &CorrelationId::try_new("interrupt").expect("correlation id"),
            generation,
            &Arc::new(ImmutablePayload::new("opencode:interrupted").expect("payload")),
        ))
    }

    fn inspect(&self, session_id: &SessionId) -> Result<ReasoningState, ReasoningError> {
        if self.state.borrow().contains_key(session_id) {
            Ok(ReasoningState::Running)
        } else {
            Ok(ReasoningState::Idle)
        }
    }

    fn subscribe(
        &self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
    ) -> Result<ReasoningEvent, ReasoningError> {
        let state = self.state.borrow();
        let state = state
            .get(session_id)
            .ok_or_else(|| ReasoningError::UnknownSession(session_id.as_str().to_owned()))?;
        Ok(self.event(
            ReasoningEventKind::Response,
            session_id,
            correlation_id,
            state.provider_generation,
            &Arc::new(ImmutablePayload::new("opencode:response").expect("payload")),
        ))
    }
}

struct Binding {
    backend: Box<dyn ReasoningBackend>,
    generation: u64,
}

#[derive(Clone)]
struct InFlight {
    group_id: RuntimeGroupId,
    generation: u64,
}

#[derive(Default)]
pub struct ReasoningService {
    bindings: HashMap<RuntimeGroupId, Binding>,
    in_flight: HashMap<SessionId, InFlight>,
}

impl ReasoningService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(
        &mut self,
        group_id: RuntimeGroupId,
        backend: Box<dyn ReasoningBackend>,
    ) -> Result<BackendCapability, ReasoningError> {
        if self.bindings.contains_key(&group_id) {
            return Err(ReasoningError::RuntimeGroupAlreadyBound(
                group_id.as_str().to_owned(),
            ));
        }
        let capability = backend.capability();
        self.bindings.insert(
            group_id,
            Binding {
                backend,
                generation: 1,
            },
        );
        Ok(capability)
    }

    pub fn replace_backend(
        &mut self,
        group_id: &RuntimeGroupId,
        backend: Box<dyn ReasoningBackend>,
    ) -> Result<BackendCapability, ReasoningError> {
        if self
            .in_flight
            .values()
            .any(|inflight| &inflight.group_id == group_id)
        {
            return Err(ReasoningError::BackendInFlight(
                group_id.as_str().to_owned(),
            ));
        }
        let old_generation = self
            .bindings
            .get(group_id)
            .map(|binding| binding.generation)
            .ok_or_else(|| ReasoningError::NoActiveBackend(group_id.as_str().to_owned()))?;
        let capability = backend.capability();
        self.bindings.insert(
            group_id.clone(),
            Binding {
                backend,
                generation: old_generation + 1,
            },
        );
        Ok(capability)
    }

    pub fn start(
        &mut self,
        group_id: &RuntimeGroupId,
        request: ReasoningRequest,
    ) -> Result<ReasoningEvent, ReasoningError> {
        let (generation, backend) = {
            let binding = self
                .bindings
                .get(group_id)
                .ok_or_else(|| ReasoningError::NoActiveBackend(group_id.as_str().to_owned()))?;
            (binding.generation, binding.backend.as_ref())
        };
        let session_id = request.session_id().clone();
        let correlation_id = request.correlation_id().clone();
        let payload = Arc::clone(request.payload());
        self.in_flight.insert(
            session_id.clone(),
            InFlight {
                group_id: group_id.clone(),
                generation,
            },
        );
        let result = backend.start(&session_id, &correlation_id, &payload, generation);
        if result.is_err() {
            self.in_flight.remove(&session_id);
        }
        result
    }

    pub fn resume(
        &mut self,
        cursor: ReasoningCursor,
        request: ReasoningRequest,
    ) -> Result<ReasoningEvent, ReasoningError> {
        let (group_id, generation) = {
            let (group_id, binding) = self
                .bindings
                .iter()
                .find(|(_, binding)| binding.backend.backend_id() == cursor.backend_id())
                .ok_or_else(|| {
                    ReasoningError::UnknownSession(cursor.session_id().as_str().to_owned())
                })?;
            (group_id.clone(), binding.generation)
        };
        if generation != cursor.generation() {
            return Err(ReasoningError::StaleGeneration {
                expected: generation,
                actual: cursor.generation(),
            });
        }
        let session_id = request.session_id().clone();
        let correlation_id = request.correlation_id().clone();
        let payload = Arc::clone(request.payload());
        self.in_flight.insert(
            session_id.clone(),
            InFlight {
                group_id: group_id.clone(),
                generation,
            },
        );
        let backend = self
            .bindings
            .get(&group_id)
            .ok_or_else(|| ReasoningError::UnknownSession(session_id.as_str().to_owned()))?;
        let result = backend.backend.resume(&cursor, &correlation_id, &payload);
        if result.is_err() {
            self.in_flight.remove(&session_id);
        }
        result
    }

    pub fn interrupt(&mut self, session_id: &SessionId) -> Result<ReasoningEvent, ReasoningError> {
        let inflight = self
            .in_flight
            .get(session_id)
            .ok_or_else(|| ReasoningError::UnknownSession(session_id.as_str().to_owned()))?;
        let group_id = inflight.group_id.clone();
        let generation = inflight.generation;
        let result = {
            let backend = self
                .bindings
                .get(&group_id)
                .ok_or_else(|| ReasoningError::NoActiveBackend(group_id.as_str().to_owned()))?;
            backend.backend.interrupt(session_id, generation)
        };
        self.in_flight.remove(session_id);
        result
    }

    pub fn inspect(&self, session_id: &SessionId) -> Result<ReasoningState, ReasoningError> {
        let inflight = self
            .in_flight
            .get(session_id)
            .ok_or_else(|| ReasoningError::UnknownSession(session_id.as_str().to_owned()))?;
        let binding = self.bindings.get(&inflight.group_id).ok_or_else(|| {
            ReasoningError::NoActiveBackend(inflight.group_id.as_str().to_owned())
        })?;
        binding.backend.inspect(session_id)
    }

    pub fn subscribe(
        &self,
        session_id: &SessionId,
        correlation_id: &CorrelationId,
    ) -> Result<ReasoningEvent, ReasoningError> {
        let inflight = self
            .in_flight
            .get(session_id)
            .ok_or_else(|| ReasoningError::UnknownSession(session_id.as_str().to_owned()))?;
        let binding = self.bindings.get(&inflight.group_id).ok_or_else(|| {
            ReasoningError::NoActiveBackend(inflight.group_id.as_str().to_owned())
        })?;
        binding.backend.subscribe(session_id, correlation_id)
    }
}
