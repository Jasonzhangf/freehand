use crate::UiCommandDispatchPortError;
use crate::adp_wire::{UiCommandDispatchEnvelope, UiCommandDispatchReceipt, UiQueryResult};
use crate::dto::*;
use freehand_contracts::TurnId;
use serde::{Deserialize, Serialize};

pub trait UiCommandDispatchPort: Send + Sync {
    fn dispatch(
        &self,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError>;
}

pub trait UiRuntimeQueryPort: Send + Sync {
    fn query_runtime(
        &self,
        command: &UiCommand,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError>;

    fn query_runtime_with_scope(
        &self,
        command: &UiCommand,
        _scope: UiQueryAccessScope,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError> {
        self.query_runtime(command)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiQueryAccessScope {
    LocalLoopback,
    Remote,
}

pub struct UiProtocolOnlyQueryPort;

impl UiRuntimeQueryPort for UiProtocolOnlyQueryPort {
    fn query_runtime(
        &self,
        _command: &UiCommand,
    ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError> {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct StaticUiCommandDispatchPort {
    dispatch_status: String,
}

impl Default for StaticUiCommandDispatchPort {
    fn default() -> Self {
        Self {
            dispatch_status: "queued_by_static_dispatch_port".to_owned(),
        }
    }
}

impl StaticUiCommandDispatchPort {
    pub fn new(dispatch_status: impl Into<String>) -> Self {
        Self {
            dispatch_status: dispatch_status.into(),
        }
    }
}

impl UiCommandDispatchPort for StaticUiCommandDispatchPort {
    fn dispatch(
        &self,
        envelope: UiCommandDispatchEnvelope,
    ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
        Ok(UiCommandDispatchReceipt {
            ingress: envelope.ingress,
            target_feature_id: envelope.target_feature_id,
            dispatch_status: self.dispatch_status.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionSelector {
    pub client: UiClientKind,
    pub stream_kind: UiStreamKind,
    pub target_turn_id: Option<TurnId>,
}
