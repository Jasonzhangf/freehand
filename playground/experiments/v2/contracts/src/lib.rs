use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_new(value).map_err(serde::de::Error::custom)
            }
        }

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, ContractError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(ContractError::EmptyId {
                        type_name: stringify!($name),
                    });
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

id_type!(CapabilityId);
id_type!(CorrelationId);
id_type!(EventId);
id_type!(NodeId);
id_type!(PluginId);
id_type!(SessionId);
id_type!(TurnId);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractError {
    #[error("{type_name} cannot be empty")]
    EmptyId { type_name: &'static str },
    #[error("payload wire value is empty")]
    EmptyPayload,
    #[error("invalid wire frame: {0}")]
    InvalidWireFrame(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImmutablePayloadValue {
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImmutablePayload(Arc<ImmutablePayloadValue>);

impl ImmutablePayload {
    pub fn new(body: impl Into<String>) -> Result<Self, ContractError> {
        let body = body.into();
        if body.is_empty() {
            return Err(ContractError::EmptyPayload);
        }
        Ok(Self(Arc::new(ImmutablePayloadValue { body })))
    }

    pub fn arc(&self) -> &Arc<ImmutablePayloadValue> {
        &self.0
    }

    pub fn body(&self) -> &str {
        &self.0.body
    }

    pub fn to_wire(&self) -> PayloadWire {
        PayloadWire {
            body: self.body().to_owned(),
        }
    }

    pub fn from_wire(wire: PayloadWire) -> Result<Self, ContractError> {
        if wire.body.is_empty() {
            return Err(ContractError::EmptyPayload);
        }
        Self::new(wire.body)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadWire {
    body: String,
}

impl<'de> Deserialize<'de> for PayloadWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PayloadWireFields {
            body: String,
        }

        let fields = PayloadWireFields::deserialize(deserializer)?;
        if fields.body.is_empty() {
            return Err(serde::de::Error::custom(ContractError::EmptyPayload));
        }
        Ok(Self { body: fields.body })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadRef {
    payload_id: String,
}

impl PayloadRef {
    pub fn new(payload_id: impl Into<String>) -> Result<Self, ContractError> {
        let payload_id = payload_id.into();
        if payload_id.is_empty() {
            return Err(ContractError::EmptyId {
                type_name: "PayloadRef",
            });
        }
        Ok(Self { payload_id })
    }
}

impl<'de> Deserialize<'de> for PayloadRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PayloadRefFields {
            payload_id: String,
        }

        let fields = PayloadRefFields::deserialize(deserializer)?;
        Self::new(fields.payload_id).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlKind {
    PluginInvoked,
    PluginCompleted,
    PluginFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlEvent {
    event_id: EventId,
    correlation_id: CorrelationId,
    kind: ControlKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_ref: Option<PayloadRef>,
}

impl ControlEvent {
    pub fn new(
        event_id: EventId,
        correlation_id: CorrelationId,
        kind: ControlKind,
        payload_ref: Option<PayloadRef>,
    ) -> Self {
        Self {
            event_id,
            correlation_id,
            kind,
            payload_ref,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    Rejected,
    InvalidPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorEvent {
    correlation_id: CorrelationId,
    kind: ErrorKind,
    message: String,
}

impl ErrorEvent {
    pub fn new(correlation_id: CorrelationId, kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            correlation_id,
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolVersion {
    V1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiCommand {
    correlation_id: CorrelationId,
    session_id: SessionId,
    capability_id: CapabilityId,
    payload: ImmutablePayload,
}

impl UiCommand {
    pub fn new(
        correlation_id: CorrelationId,
        session_id: SessionId,
        capability_id: CapabilityId,
        payload: ImmutablePayload,
    ) -> Self {
        Self {
            correlation_id,
            session_id,
            capability_id,
            payload,
        }
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }

    pub fn payload(&self) -> &ImmutablePayload {
        &self.payload
    }

    pub fn to_wire(&self) -> UiCommandWire {
        UiCommandWire {
            correlation_id: self.correlation_id.clone(),
            session_id: self.session_id.clone(),
            capability_id: self.capability_id.clone(),
            payload: self.payload.to_wire(),
        }
    }

    pub fn from_wire(wire: UiCommandWire) -> Result<Self, ContractError> {
        Ok(Self::new(
            wire.correlation_id,
            wire.session_id,
            wire.capability_id,
            ImmutablePayload::from_wire(wire.payload)?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiCommandWire {
    correlation_id: CorrelationId,
    session_id: SessionId,
    capability_id: CapabilityId,
    payload: PayloadWire,
}

impl UiCommandWire {
    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }

    pub fn payload(&self) -> &PayloadWire {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadFrame {
    version: ProtocolVersion,
    command: UiCommandWire,
}

impl PayloadFrame {
    pub fn new(version: ProtocolVersion, command: UiCommandWire) -> Self {
        Self { version, command }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "frame_class", content = "frame")]
pub enum WireFrame {
    Payload(PayloadFrame),
    Control(ControlEvent),
    Error(ErrorEvent),
}

impl WireFrame {
    pub fn decode(raw: &str) -> Result<Self, ContractError> {
        serde_json::from_str(raw)
            .map_err(|error| ContractError::InvalidWireFrame(error.to_string()))
    }
}
