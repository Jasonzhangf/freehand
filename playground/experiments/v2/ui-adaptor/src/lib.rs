use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, CorrelationId, EventId, ImmutablePayload, UiCommand};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UiError {
    #[error("slot id cannot be empty")]
    EmptySlotId,
    #[error("projection id cannot be empty")]
    EmptyProjectionId,
    #[error("query id cannot be empty")]
    EmptyQueryId,
    #[error("subscription id cannot be empty")]
    EmptySubscriptionId,
    #[error("unknown slot: {0}")]
    UnknownSlot(String),
    #[error("command already accepted: {0}")]
    DuplicateCommand(String),
    #[error("invalid projection: {0}")]
    InvalidProjection(String),
    #[error("event id generation failed: {0}")]
    EventId(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SlotId(String);

impl SlotId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, UiError> {
        let value = value.into();
        if value.is_empty() {
            return Err(UiError::EmptySlotId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionKind {
    Run,
    Sessions,
    Attention,
    Location,
    More,
    Detail,
    Search,
    Memory,
}

#[derive(Debug, Clone)]
pub struct UiProjection {
    projection_id: String,
    slot_id: SlotId,
    kind: ProjectionKind,
    revision: u64,
    source: String,
    payload: Arc<ImmutablePayload>,
}

impl UiProjection {
    pub fn new(
        projection_id: impl Into<String>,
        slot_id: SlotId,
        kind: ProjectionKind,
        source: impl Into<String>,
        payload: Arc<ImmutablePayload>,
    ) -> Result<Self, UiError> {
        let projection_id = projection_id.into();
        let source = source.into();
        if projection_id.is_empty() {
            return Err(UiError::EmptyProjectionId);
        }
        if source.is_empty() {
            return Err(UiError::InvalidProjection(
                "source identity cannot be empty".to_owned(),
            ));
        }
        Ok(Self {
            projection_id,
            slot_id,
            kind,
            revision: 0,
            source,
            payload,
        })
    }

    pub fn with_revision(mut self, revision: u64) -> Self {
        self.revision = revision;
        self
    }

    pub fn projection_id(&self) -> &str {
        &self.projection_id
    }

    pub fn slot_id(&self) -> &SlotId {
        &self.slot_id
    }

    pub fn kind(&self) -> ProjectionKind {
        self.kind
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn payload(&self) -> &Arc<ImmutablePayload> {
        &self.payload
    }

    pub fn to_wire(&self) -> UiProjectionWire {
        UiProjectionWire {
            projection_id: self.projection_id.clone(),
            slot_id: self.slot_id.clone(),
            kind: self.kind,
            revision: self.revision,
            source: self.source.clone(),
            payload_body: self.payload.body().to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UiProjectionWire {
    projection_id: String,
    slot_id: SlotId,
    kind: ProjectionKind,
    revision: u64,
    source: String,
    payload_body: String,
}

impl UiProjectionWire {
    pub fn projection_id(&self) -> &str {
        &self.projection_id
    }

    pub fn slot_id(&self) -> &SlotId {
        &self.slot_id
    }

    pub fn kind(&self) -> ProjectionKind {
        self.kind
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn payload_body(&self) -> &str {
        &self.payload_body
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiCommandReceiptStatus {
    Accepted,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiCommandReceipt {
    receipt_id: String,
    slot_id: SlotId,
    status: UiCommandReceiptStatus,
    message: String,
}

impl UiCommandReceipt {
    pub fn new(
        receipt_id: impl Into<String>,
        slot_id: SlotId,
        status: UiCommandReceiptStatus,
        message: impl Into<String>,
    ) -> Self {
        Self {
            receipt_id: receipt_id.into(),
            slot_id,
            status,
            message: message.into(),
        }
    }

    pub fn receipt_id(&self) -> &str {
        &self.receipt_id
    }

    pub fn slot_id(&self) -> &SlotId {
        &self.slot_id
    }

    pub fn status(&self) -> UiCommandReceiptStatus {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiQuery {
    query_id: String,
    slot_id: SlotId,
    scope: String,
    filter: String,
}

impl UiQuery {
    pub fn new(
        query_id: impl Into<String>,
        slot_id: SlotId,
        scope: impl Into<String>,
        filter: impl Into<String>,
    ) -> Result<Self, UiError> {
        let query_id = query_id.into();
        if query_id.is_empty() {
            return Err(UiError::EmptyQueryId);
        }
        Ok(Self {
            query_id,
            slot_id,
            scope: scope.into(),
            filter: filter.into(),
        })
    }

    pub fn query_id(&self) -> &str {
        &self.query_id
    }

    pub fn slot_id(&self) -> &SlotId {
        &self.slot_id
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiSubscribe {
    subscription_id: String,
    slot_id: SlotId,
    cursor: u64,
}

impl UiSubscribe {
    pub fn new(
        subscription_id: impl Into<String>,
        slot_id: SlotId,
        cursor: u64,
    ) -> Result<Self, UiError> {
        let subscription_id = subscription_id.into();
        if subscription_id.is_empty() {
            return Err(UiError::EmptySubscriptionId);
        }
        Ok(Self {
            subscription_id,
            slot_id,
            cursor,
        })
    }

    pub fn subscription_id(&self) -> &str {
        &self.subscription_id
    }

    pub fn slot_id(&self) -> &SlotId {
        &self.slot_id
    }

    pub fn cursor(&self) -> u64 {
        self.cursor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiConnectionState {
    Local,
    Remote,
    Disconnected,
    Unavailable,
    ProtocolError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiCapabilityAvailability {
    slot_id: SlotId,
    capability_id: CapabilityId,
    available: bool,
}

impl UiCapabilityAvailability {
    pub fn new(slot_id: SlotId, capability_id: CapabilityId, available: bool) -> Self {
        Self {
            slot_id,
            capability_id,
            available,
        }
    }

    pub fn slot_id(&self) -> &SlotId {
        &self.slot_id
    }

    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }

    pub fn available(&self) -> bool {
        self.available
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiControlEventKind {
    CommandAccepted,
    ProjectionUpdated,
    SubscriptionAttached,
    SubscriptionDetached,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiControlEvent {
    event_id: EventId,
    correlation_id: CorrelationId,
    slot_id: SlotId,
    kind: UiControlEventKind,
}

impl UiControlEvent {
    pub fn new(
        event_id: EventId,
        correlation_id: CorrelationId,
        slot_id: SlotId,
        kind: UiControlEventKind,
    ) -> Self {
        Self {
            event_id,
            correlation_id,
            slot_id,
            kind,
        }
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    pub fn slot_id(&self) -> &SlotId {
        &self.slot_id
    }

    pub fn kind(&self) -> UiControlEventKind {
        self.kind
    }
}

#[derive(Default)]
pub struct UiAdaptor {
    projections: HashMap<SlotId, Vec<UiProjection>>,
    accepted_commands: HashSet<CorrelationId>,
    subscriptions: Vec<UiSubscribe>,
    events: Vec<UiControlEvent>,
    next_event_seq: u64,
}

impl UiAdaptor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accept_command(
        &mut self,
        slot_id: SlotId,
        command: &UiCommand,
    ) -> Result<UiCommandReceipt, UiError> {
        let correlation_id = command.correlation_id();
        if self.accepted_commands.contains(correlation_id) {
            return Err(UiError::DuplicateCommand(
                correlation_id.as_str().to_owned(),
            ));
        }
        let receipt_id = format!("cmd-receipt-{}", correlation_id.as_str());
        self.accepted_commands.insert(correlation_id.clone());
        self.emit(
            correlation_id.clone(),
            slot_id.clone(),
            UiControlEventKind::CommandAccepted,
        )?;
        Ok(UiCommandReceipt::new(
            receipt_id,
            slot_id,
            UiCommandReceiptStatus::Accepted,
            "command accepted",
        ))
    }

    pub fn publish_projection(
        &mut self,
        slot_id: SlotId,
        kind: ProjectionKind,
        source: impl Into<String>,
        payload: Arc<ImmutablePayload>,
    ) -> Result<UiProjection, UiError> {
        let revision = self
            .projections
            .get(&slot_id)
            .map_or(0, |entries| entries.len() as u64);
        let projection_id = format!("projection-{}-{}", slot_id.as_str(), revision);
        let projection = UiProjection::new(
            projection_id.clone(),
            slot_id.clone(),
            kind,
            source,
            payload,
        )?
        .with_revision(revision);
        self.projections
            .entry(slot_id.clone())
            .or_default()
            .push(projection.clone());
        let correlation = CorrelationId::try_new(format!("projection-{}", projection_id))
            .map_err(|err| UiError::EventId(err.to_string()))?;
        self.emit(correlation, slot_id, UiControlEventKind::ProjectionUpdated)?;
        Ok(projection)
    }

    pub fn query(&self, slot_id: &SlotId) -> Result<UiProjection, UiError> {
        self.projections
            .get(slot_id)
            .and_then(|entries| entries.last())
            .cloned()
            .ok_or_else(|| UiError::UnknownSlot(slot_id.as_str().to_owned()))
    }

    pub fn subscribe(&mut self, subscription: UiSubscribe) -> Result<(), UiError> {
        if !self.projections.contains_key(subscription.slot_id()) {
            return Err(UiError::UnknownSlot(
                subscription.slot_id().as_str().to_owned(),
            ));
        }
        let slot_id = subscription.slot_id().clone();
        let correlation =
            CorrelationId::try_new(format!("subscribe-{}", subscription.subscription_id()))
                .map_err(|err| UiError::EventId(err.to_string()))?;
        self.subscriptions.push(subscription);
        self.emit(
            correlation,
            slot_id,
            UiControlEventKind::SubscriptionAttached,
        )
    }

    pub fn events(&self) -> &[UiControlEvent] {
        &self.events
    }

    pub fn projection_count(&self, slot_id: &SlotId) -> usize {
        self.projections.get(slot_id).map_or(0, Vec::len)
    }

    fn emit(
        &mut self,
        correlation_id: CorrelationId,
        slot_id: SlotId,
        kind: UiControlEventKind,
    ) -> Result<(), UiError> {
        let seq = self.next_event_seq;
        self.next_event_seq += 1;
        let event_id = EventId::try_new(format!("ui-control-{seq}"))
            .map_err(|err| UiError::EventId(err.to_string()))?;
        self.events
            .push(UiControlEvent::new(event_id, correlation_id, slot_id, kind));
        Ok(())
    }
}

pub fn projection_wire_to_json(wire: &UiProjectionWire) -> serde_json::Value {
    serde_json::to_value(wire).unwrap_or(serde_json::Value::Null)
}
