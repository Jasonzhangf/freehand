use std::collections::HashMap;
use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, CorrelationId, EventId, ImmutablePayload, PluginId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapabilityError {
    #[error("unsupported manifest version: {0}")]
    UnsupportedManifestVersion(u32),
    #[error("capability already registered: {0}")]
    DuplicateCapability(String),
    #[error("unknown capability: {0}")]
    UnknownCapability(String),
    #[error("undeclared input contract: {0}")]
    UndeclaredInputContract(String),
    #[error("invalid capability manifest: {0}")]
    InvalidManifest(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("capability invocation failed: {0}")]
    InvocationFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityManifest {
    manifest_version: u32,
    plugin_id: PluginId,
    capability_id: CapabilityId,
    input_contract: String,
    output_contract: String,
    events_emitted: Vec<String>,
    permissions: Vec<String>,
    scope: Option<String>,
}

impl CapabilityManifest {
    pub fn try_new(
        plugin_id: PluginId,
        capability_id: CapabilityId,
        input_contract: impl Into<String>,
        output_contract: impl Into<String>,
        events_emitted: Vec<String>,
        permissions: Vec<String>,
        scope: Option<String>,
    ) -> Result<Self, CapabilityError> {
        Self::validate_version(MANIFEST_VERSION)?;
        let input_contract = input_contract.into();
        let output_contract = output_contract.into();
        if input_contract.is_empty() {
            return Err(CapabilityError::InvalidManifest(
                "input_contract cannot be empty".to_owned(),
            ));
        }
        if output_contract.is_empty() {
            return Err(CapabilityError::InvalidManifest(
                "output_contract cannot be empty".to_owned(),
            ));
        }
        Ok(Self {
            manifest_version: MANIFEST_VERSION,
            plugin_id,
            capability_id,
            input_contract,
            output_contract,
            events_emitted,
            permissions,
            scope,
        })
    }

    pub fn validate_version(version: u32) -> Result<(), CapabilityError> {
        if version != MANIFEST_VERSION {
            return Err(CapabilityError::UnsupportedManifestVersion(version));
        }
        Ok(())
    }

    pub fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }

    pub fn input_contract(&self) -> &str {
        &self.input_contract
    }

    pub fn output_contract(&self) -> &str {
        &self.output_contract
    }

    pub fn events_emitted(&self) -> &[String] {
        &self.events_emitted
    }

    pub fn permissions(&self) -> &[String] {
        &self.permissions
    }

    pub fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityInvocation {
    result_id: EventId,
    correlation_id: CorrelationId,
    capability_id: CapabilityId,
    payload: Arc<ImmutablePayload>,
    success: bool,
}

impl CapabilityInvocation {
    pub fn new(
        result_id: EventId,
        correlation_id: CorrelationId,
        capability_id: CapabilityId,
        payload: Arc<ImmutablePayload>,
        success: bool,
    ) -> Self {
        Self {
            result_id,
            correlation_id,
            capability_id,
            payload,
            success,
        }
    }

    pub fn result_id(&self) -> &EventId {
        &self.result_id
    }

    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }

    pub fn payload(&self) -> &Arc<ImmutablePayload> {
        &self.payload
    }

    pub fn success(&self) -> bool {
        self.success
    }
}

pub trait CapabilityPlugin {
    fn capability_id(&self) -> &CapabilityId;

    fn manifest(&self) -> &CapabilityManifest;

    fn invoke(
        &self,
        correlation_id: &CorrelationId,
        payload: Arc<ImmutablePayload>,
    ) -> Result<CapabilityInvocation, CapabilityError>;
}

#[derive(Debug, Clone)]
pub struct LocalCapabilityPlugin {
    manifest: CapabilityManifest,
    fail_next: bool,
}

impl LocalCapabilityPlugin {
    pub fn new(manifest: CapabilityManifest) -> Result<Self, CapabilityError> {
        Ok(Self {
            manifest,
            fail_next: false,
        })
    }

    pub fn fail_next(mut self) -> Self {
        self.fail_next = true;
        self
    }
}

impl CapabilityPlugin for LocalCapabilityPlugin {
    fn capability_id(&self) -> &CapabilityId {
        self.manifest.capability_id()
    }

    fn manifest(&self) -> &CapabilityManifest {
        &self.manifest
    }

    fn invoke(
        &self,
        correlation_id: &CorrelationId,
        payload: Arc<ImmutablePayload>,
    ) -> Result<CapabilityInvocation, CapabilityError> {
        if self.fail_next {
            return Err(CapabilityError::InvocationFailed(
                self.manifest.capability_id().as_str().to_owned(),
            ));
        }
        let result_id = EventId::try_new(format!(
            "{}-{}",
            self.manifest.capability_id().as_str(),
            "result"
        ))
        .map_err(|err| CapabilityError::InvalidManifest(err.to_string()))?;
        Ok(CapabilityInvocation::new(
            result_id,
            correlation_id.clone(),
            self.manifest.capability_id().clone(),
            payload,
            true,
        ))
    }
}

#[derive(Default)]
pub struct CapabilityRegistry {
    plugins: HashMap<CapabilityId, Box<dyn CapabilityPlugin>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        plugin: Box<dyn CapabilityPlugin>,
    ) -> Result<&CapabilityManifest, CapabilityError> {
        let capability_id = plugin.capability_id().clone();
        if self.plugins.contains_key(&capability_id) {
            return Err(CapabilityError::DuplicateCapability(
                capability_id.as_str().to_owned(),
            ));
        }
        let manifest = plugin.manifest().clone();
        self.plugins.insert(capability_id, plugin);
        Ok(self.plugins[&manifest.capability_id().clone()].manifest())
    }

    pub fn get(&self, capability_id: &CapabilityId) -> Option<&dyn CapabilityPlugin> {
        self.plugins
            .get(capability_id)
            .map(|plugin| plugin.as_ref())
    }

    pub fn replace(
        &mut self,
        plugin: Box<dyn CapabilityPlugin>,
    ) -> Result<&CapabilityManifest, CapabilityError> {
        let capability_id = plugin.capability_id().clone();
        if !self.plugins.contains_key(&capability_id) {
            return Err(CapabilityError::UnknownCapability(
                capability_id.as_str().to_owned(),
            ));
        }
        let manifest = plugin.manifest().clone();
        self.plugins.insert(capability_id, plugin);
        Ok(self.plugins[&manifest.capability_id().clone()].manifest())
    }

    pub fn contains(&self, capability_id: &CapabilityId) -> bool {
        self.plugins.contains_key(capability_id)
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn unload(&mut self, capability_id: &CapabilityId) -> Result<(), CapabilityError> {
        if self.plugins.remove(capability_id).is_none() {
            return Err(CapabilityError::UnknownCapability(
                capability_id.as_str().to_owned(),
            ));
        }
        Ok(())
    }
}
