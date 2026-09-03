use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use freehand_v2_contracts::{
    CapabilityId, ControlKind, CorrelationId, ErrorKind, EventId, ImmutablePayload,
};
use freehand_v2_control_events::{EventLedger, EventLedgerError};
use freehand_v2_plugin_capabilities::{
    CapabilityInvocation, CapabilityManifest, CapabilityPlugin, CapabilityRegistry,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CordisError {
    #[error("capability error: {0}")]
    Capability(#[from] freehand_v2_plugin_capabilities::CapabilityError),
    #[error("event ledger error: {0}")]
    EventLedger(#[from] EventLedgerError),
    #[error("unknown runtime scope: {0}")]
    UnknownScope(String),
    #[error("plugin has in-flight operation: {0}")]
    PluginInFlight(String),
    #[error("correlation already in flight: {0}")]
    AlreadyInFlight(String),
    #[error("event id generation failed: {0}")]
    EventId(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScopeId(String);

impl ScopeId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, CordisError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CordisError::UnknownScope(
                "scope id cannot be empty".to_owned(),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionResult {
    invocation: CapabilityInvocation,
    event_sequence: Vec<CompositionEvent>,
}

impl CompositionResult {
    pub fn invocation(&self) -> &CapabilityInvocation {
        &self.invocation
    }

    pub fn event_sequence(&self) -> &[CompositionEvent] {
        &self.event_sequence
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionEvent {
    Invoked,
    Completed,
    Rejected,
}

#[derive(Default)]
pub struct CordisRoot {
    scopes: HashMap<ScopeId, CordisContext>,
}

impl CordisRoot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scope(&mut self, scope_id: ScopeId) -> &mut CordisContext {
        self.scopes.entry(scope_id).or_default()
    }

    pub fn replace_scope(&mut self, scope_id: ScopeId, context: CordisContext) {
        self.scopes.insert(scope_id, context);
    }
}

#[derive(Default)]
pub struct CordisContext {
    capabilities: CapabilityRegistry,
    events: EventLedger,
    in_flight: HashSet<CorrelationId>,
}

impl CordisContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        plugin: Box<dyn CapabilityPlugin>,
    ) -> Result<CapabilityManifest, CordisError> {
        let manifest = self.capabilities.register(plugin)?.clone();
        Ok(manifest)
    }

    pub fn replace_capability(
        &mut self,
        plugin: Box<dyn CapabilityPlugin>,
    ) -> Result<CapabilityManifest, CordisError> {
        if !self.in_flight.is_empty() {
            return Err(CordisError::PluginInFlight(
                "cannot replace while operations are pending".to_owned(),
            ));
        }
        let manifest = self.capabilities.replace(plugin)?.clone();
        Ok(manifest)
    }

    pub fn unload(&mut self, capability_id: &CapabilityId) -> Result<(), CordisError> {
        if !self.in_flight.is_empty() {
            return Err(CordisError::PluginInFlight(
                capability_id.as_str().to_owned(),
            ));
        }
        self.capabilities.unload(capability_id)?;
        Ok(())
    }

    pub fn invoke(
        &mut self,
        correlation_id: CorrelationId,
        capability_id: CapabilityId,
        payload: Arc<ImmutablePayload>,
    ) -> Result<CompositionResult, CordisError> {
        if self.in_flight.contains(&correlation_id) {
            return Err(CordisError::AlreadyInFlight(
                correlation_id.as_str().to_owned(),
            ));
        }
        let plugin = self.capabilities.get(&capability_id).ok_or_else(|| {
            CordisError::Capability(
                freehand_v2_plugin_capabilities::CapabilityError::UnknownCapability(
                    capability_id.as_str().to_owned(),
                ),
            )
        })?;
        let plugin_id = plugin.manifest().plugin_id().clone();
        let invoked_event_id = self.next_event_id("invoked")?;
        self.events.emit(
            invoked_event_id.clone(),
            correlation_id.clone(),
            ControlKind::PluginInvoked,
            plugin_id.clone(),
            None,
        )?;
        self.in_flight.insert(correlation_id.clone());

        let invocation = match plugin.invoke(&correlation_id, payload) {
            Ok(invocation) => invocation,
            Err(error) => {
                let source_event_id = Some(invoked_event_id);
                let _ = self.events.reject(
                    correlation_id.clone(),
                    ErrorKind::InvalidPayload,
                    error.to_string(),
                    source_event_id,
                );
                self.in_flight.remove(&correlation_id);
                return Err(error.into());
            }
        };
        let mut sequence = vec![CompositionEvent::Invoked];
        if invocation.success() {
            let completed_event_id = self.next_event_id("completed")?;
            self.events.emit(
                completed_event_id,
                correlation_id.clone(),
                ControlKind::PluginCompleted,
                plugin_id,
                None,
            )?;
            self.events.complete(&correlation_id)?;
            sequence.push(CompositionEvent::Completed);
        } else {
            self.events.reject(
                correlation_id.clone(),
                ErrorKind::Rejected,
                "capability invocation returned success=false",
                Some(invoked_event_id),
            )?;
            sequence.push(CompositionEvent::Rejected);
        }
        self.in_flight.remove(&correlation_id);
        Ok(CompositionResult {
            invocation,
            event_sequence: sequence,
        })
    }

    pub fn events(&self) -> &EventLedger {
        &self.events
    }

    pub fn is_in_flight(&self, correlation_id: &CorrelationId) -> bool {
        self.in_flight.contains(correlation_id)
    }

    fn next_event_id(&self, label: &str) -> Result<EventId, CordisError> {
        EventId::try_new(format!("cordis-{label}-{}", self.events.next_seq() + 1))
            .map_err(|err| CordisError::EventId(err.to_string()))
    }
}

pub fn public_result(invocation: &CapabilityInvocation) -> serde_json::Value {
    serde_json::json!({
        "result_id": invocation.result_id().as_str(),
        "capability_id": invocation.capability_id().as_str(),
        "correlation_id": invocation.correlation_id().as_str(),
        "success": invocation.success(),
        "payload": invocation.payload().body(),
    })
}
