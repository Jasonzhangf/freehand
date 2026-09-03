use std::collections::{HashMap, HashSet};

use freehand_v2_contracts::{ControlKind, CorrelationId, ErrorKind, EventId, PayloadRef, PluginId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventState {
    Accepted,
    Acknowledged,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    seq: u64,
    event_id: EventId,
    correlation_id: CorrelationId,
    kind: ControlKind,
    owner: PluginId,
    payload_ref: Option<PayloadRef>,
    state: EventState,
}

impl EventRecord {
    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    pub fn kind(&self) -> ControlKind {
        self.kind
    }

    pub fn owner(&self) -> &PluginId {
        &self.owner
    }

    pub fn payload_ref(&self) -> Option<&PayloadRef> {
        self.payload_ref.as_ref()
    }

    pub fn state(&self) -> EventState {
        self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorRecord {
    seq: u64,
    correlation_id: CorrelationId,
    kind: ErrorKind,
    message: String,
    source_event_id: Option<EventId>,
}

impl ErrorRecord {
    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn source_event_id(&self) -> Option<&EventId> {
        self.source_event_id.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerCursor {
    last_applied_seq: u64,
}

impl LedgerCursor {
    pub fn new(last_applied_seq: u64) -> Self {
        Self { last_applied_seq }
    }

    pub fn last_applied_seq(&self) -> u64 {
        self.last_applied_seq
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EventLedgerError {
    #[error("duplicate event id: {0}")]
    DuplicateEventId(String),
    #[error("event not found: {0}")]
    EventNotFound(String),
    #[error("correlation already terminal: {0}")]
    AlreadyTerminal(String),
    #[error("event already acknowledged: {0}")]
    AlreadyAcknowledged(String),
    #[error("unknown correlation: {0}")]
    UnknownCorrelation(String),
    #[error("invalid replay cursor: {0}")]
    InvalidCursor(String),
}

#[derive(Debug, Default)]
pub struct EventLedger {
    events: Vec<EventRecord>,
    errors: Vec<ErrorRecord>,
    event_index: HashMap<EventId, u64>,
    terminal_correlations: HashSet<CorrelationId>,
    ack_cursor: u64,
}

impl EventLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(
        &mut self,
        event_id: EventId,
        correlation_id: CorrelationId,
        kind: ControlKind,
        owner: PluginId,
        payload_ref: Option<PayloadRef>,
    ) -> Result<LedgerCursor, EventLedgerError> {
        if self.event_index.contains_key(&event_id) {
            return Err(EventLedgerError::DuplicateEventId(
                event_id.as_str().to_owned(),
            ));
        }
        if self.terminal_correlations.contains(&correlation_id) {
            return Err(EventLedgerError::AlreadyTerminal(
                correlation_id.as_str().to_owned(),
            ));
        }
        let seq = self.events.len() as u64;
        self.event_index.insert(event_id.clone(), seq);
        self.events.push(EventRecord {
            seq,
            event_id,
            correlation_id,
            kind,
            owner,
            payload_ref,
            state: EventState::Accepted,
        });
        Ok(LedgerCursor::new(self.events.len() as u64))
    }

    pub fn reject(
        &mut self,
        correlation_id: CorrelationId,
        kind: ErrorKind,
        message: impl Into<String>,
        source_event_id: Option<EventId>,
    ) -> Result<ErrorRecord, EventLedgerError> {
        if self.terminal_correlations.contains(&correlation_id) {
            return Err(EventLedgerError::AlreadyTerminal(
                correlation_id.as_str().to_owned(),
            ));
        }
        if let Some(source_event_id) = &source_event_id
            && !self.event_index.contains_key(source_event_id)
        {
            return Err(EventLedgerError::EventNotFound(
                source_event_id.as_str().to_owned(),
            ));
        }
        let seq = self.errors.len() as u64;
        let record = ErrorRecord {
            seq,
            correlation_id,
            kind,
            message: message.into(),
            source_event_id,
        };
        self.errors.push(record.clone());
        Ok(record)
    }

    pub fn acknowledge(&mut self, event_id: &EventId) -> Result<LedgerCursor, EventLedgerError> {
        let idx = self
            .event_index
            .get(event_id)
            .copied()
            .ok_or_else(|| EventLedgerError::EventNotFound(event_id.as_str().to_owned()))?;
        let event = &mut self.events[idx as usize];
        match event.state {
            EventState::Terminal => {
                return Err(EventLedgerError::AlreadyTerminal(
                    event.correlation_id().as_str().to_owned(),
                ));
            }
            EventState::Acknowledged => {
                return Err(EventLedgerError::AlreadyAcknowledged(
                    event.event_id().as_str().to_owned(),
                ));
            }
            EventState::Accepted => {
                event.state = EventState::Acknowledged;
                self.ack_cursor = self.ack_cursor.max(event.seq + 1);
            }
        }
        Ok(LedgerCursor::new(self.ack_cursor))
    }

    pub fn complete(&mut self, correlation_id: &CorrelationId) -> Result<u64, EventLedgerError> {
        let mut matched = false;
        let mut all_terminal = true;
        for event in &mut self.events {
            if &event.correlation_id == correlation_id {
                matched = true;
                if event.state != EventState::Terminal {
                    all_terminal = false;
                }
            }
        }
        if !matched {
            return Err(EventLedgerError::UnknownCorrelation(
                correlation_id.as_str().to_owned(),
            ));
        }
        if all_terminal {
            return Err(EventLedgerError::AlreadyTerminal(
                correlation_id.as_str().to_owned(),
            ));
        }

        let mut count = 0;
        for event in &mut self.events {
            if &event.correlation_id == correlation_id && event.state != EventState::Terminal {
                event.state = EventState::Terminal;
                count += 1;
            }
        }
        self.terminal_correlations.insert(correlation_id.clone());
        Ok(count)
    }

    pub fn replay_from(&self, cursor: &LedgerCursor) -> Result<Vec<EventRecord>, EventLedgerError> {
        let len = self.events.len() as u64;
        if cursor.last_applied_seq > len {
            return Err(EventLedgerError::InvalidCursor(format!(
                "cursor {} is beyond durable boundary {}",
                cursor.last_applied_seq, len
            )));
        }
        Ok(self
            .events
            .iter()
            .filter(|event| event.seq >= cursor.last_applied_seq)
            .cloned()
            .collect())
    }

    pub fn owner_events(&self, owner: &PluginId) -> Vec<&EventRecord> {
        self.events
            .iter()
            .filter(|event| &event.owner == owner)
            .collect()
    }

    pub fn is_terminal(&self, correlation_id: &CorrelationId) -> bool {
        self.terminal_correlations.contains(correlation_id)
    }

    pub fn events(&self) -> &[EventRecord] {
        &self.events
    }

    pub fn errors(&self) -> &[ErrorRecord] {
        &self.errors
    }

    pub fn next_seq(&self) -> u64 {
        self.events.len() as u64
    }

    pub fn ack_cursor(&self) -> u64 {
        self.ack_cursor
    }
}
