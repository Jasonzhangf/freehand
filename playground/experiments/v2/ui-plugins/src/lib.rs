use std::collections::HashMap;

use freehand_v2_contracts::{CapabilityId, PluginId};
use freehand_v2_ui_adaptor::{SlotId, UiProjection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UiPluginError {
    #[error("duplicate slot: {0}")]
    DuplicateSlot(String),
    #[error("unknown slot: {0}")]
    UnknownSlot(String),
    #[error("invalid plugin definition: {0}")]
    InvalidDefinition(String),
    #[error("plugin cannot render: {0}")]
    CannotRender(String),
    #[error("plugin not mounted: {0}")]
    NotMounted(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiPluginDefinition {
    plugin_id: PluginId,
    slot_id: SlotId,
    instance_id: String,
    contract_version: u32,
    capabilities: Vec<CapabilityId>,
}

impl UiPluginDefinition {
    pub fn try_new(
        plugin_id: PluginId,
        slot_id: SlotId,
        instance_id: impl Into<String>,
        contract_version: u32,
        capabilities: Vec<CapabilityId>,
    ) -> Result<Self, UiPluginError> {
        let instance_id = instance_id.into();
        if instance_id.is_empty() {
            return Err(UiPluginError::InvalidDefinition(
                "instance_id cannot be empty".to_owned(),
            ));
        }
        if contract_version == 0 {
            return Err(UiPluginError::InvalidDefinition(
                "contract_version must be positive".to_owned(),
            ));
        }
        if capabilities.is_empty() {
            return Err(UiPluginError::InvalidDefinition(
                "capabilities cannot be empty".to_owned(),
            ));
        }
        Ok(Self {
            plugin_id,
            slot_id,
            instance_id,
            contract_version,
            capabilities,
        })
    }

    pub fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    pub fn slot_id(&self) -> &SlotId {
        &self.slot_id
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub fn contract_version(&self) -> u32 {
        self.contract_version
    }

    pub fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiPluginState {
    Loading,
    Ready,
    Empty,
    Unavailable,
    Disconnected,
    Error,
}

#[derive(Debug, Clone)]
pub struct UiPluginView {
    definition: UiPluginDefinition,
    state: UiPluginState,
    projection: Option<UiProjection>,
    selection: Option<String>,
}

impl UiPluginView {
    pub fn new(
        definition: UiPluginDefinition,
        state: UiPluginState,
        projection: Option<UiProjection>,
        selection: Option<String>,
    ) -> Self {
        Self {
            definition,
            state,
            projection,
            selection,
        }
    }

    pub fn definition(&self) -> &UiPluginDefinition {
        &self.definition
    }

    pub fn state(&self) -> UiPluginState {
        self.state
    }

    pub fn projection(&self) -> Option<&UiProjection> {
        self.projection.as_ref()
    }

    pub fn selection(&self) -> Option<&str> {
        self.selection.as_deref()
    }

    pub fn to_public_json(&self) -> serde_json::Value {
        serde_json::json!({
            "plugin_id": self.definition.plugin_id().as_str(),
            "slot_id": self.definition.slot_id().as_str(),
            "instance_id": self.definition.instance_id(),
            "state": match self.state {
                UiPluginState::Loading => "loading",
                UiPluginState::Ready => "ready",
                UiPluginState::Empty => "empty",
                UiPluginState::Unavailable => "unavailable",
                UiPluginState::Disconnected => "disconnected",
                UiPluginState::Error => "error",
            },
            "selection": self.selection,
            "projection_payload": self.projection.as_ref().map(|p| p.payload().body()),
            "projection_id": self.projection.as_ref().map(|p| p.projection_id()),
        })
    }
}

pub trait UiPlugin {
    fn definition(&self) -> &UiPluginDefinition;

    fn slot_id(&self) -> &SlotId {
        self.definition().slot_id()
    }

    fn mount(&mut self) -> Result<(), UiPluginError>;

    fn render(
        &mut self,
        projection: UiProjection,
        selection: Option<String>,
    ) -> Result<UiPluginView, UiPluginError>;

    fn unmount(&mut self) -> Result<(), UiPluginError>;
}

pub struct InMemoryUiPlugin {
    definition: UiPluginDefinition,
    state: UiPluginState,
    selection: Option<String>,
}

impl InMemoryUiPlugin {
    pub fn new(definition: UiPluginDefinition) -> Result<Self, UiPluginError> {
        Ok(Self {
            definition,
            state: UiPluginState::Loading,
            selection: None,
        })
    }

    pub fn with_selection(mut self, selection: impl Into<String>) -> Self {
        self.selection = Some(selection.into());
        self
    }
}

impl UiPlugin for InMemoryUiPlugin {
    fn definition(&self) -> &UiPluginDefinition {
        &self.definition
    }

    fn mount(&mut self) -> Result<(), UiPluginError> {
        self.state = UiPluginState::Ready;
        Ok(())
    }

    fn render(
        &mut self,
        projection: UiProjection,
        selection: Option<String>,
    ) -> Result<UiPluginView, UiPluginError> {
        if self.state == UiPluginState::Unavailable {
            return Err(UiPluginError::CannotRender(
                self.definition.slot_id().as_str().to_owned(),
            ));
        }
        self.state = UiPluginState::Ready;
        self.selection = selection.clone();
        Ok(UiPluginView::new(
            self.definition.clone(),
            UiPluginState::Ready,
            Some(projection),
            selection,
        ))
    }

    fn unmount(&mut self) -> Result<(), UiPluginError> {
        self.state = UiPluginState::Unavailable;
        Ok(())
    }
}

struct SlotEntry {
    plugin: Box<dyn UiPlugin>,
    view: Option<UiPluginView>,
}

#[derive(Default)]
pub struct UiPluginSlotRegistry {
    slots: HashMap<SlotId, SlotEntry>,
}

impl UiPluginSlotRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mount(
        &mut self,
        plugin: Box<dyn UiPlugin>,
    ) -> Result<&UiPluginDefinition, UiPluginError> {
        let slot_id = plugin.slot_id().clone();
        if self.slots.contains_key(&slot_id) {
            return Err(UiPluginError::DuplicateSlot(slot_id.as_str().to_owned()));
        }
        let mut plugin = plugin;
        plugin.mount()?;
        let definition = plugin.definition().clone();
        self.slots.insert(slot_id, SlotEntry { plugin, view: None });
        Ok(self
            .slots
            .get(definition.slot_id())
            .expect("slot inserted")
            .plugin
            .definition())
    }

    pub fn render(
        &mut self,
        slot_id: &SlotId,
        projection: UiProjection,
        selection: Option<String>,
    ) -> Result<UiPluginView, UiPluginError> {
        let entry = self
            .slots
            .get_mut(slot_id)
            .ok_or_else(|| UiPluginError::UnknownSlot(slot_id.as_str().to_owned()))?;
        let view = entry.plugin.render(projection, selection)?;
        entry.view = Some(view.clone());
        Ok(view)
    }

    pub fn replace(&mut self, plugin: Box<dyn UiPlugin>) -> Result<UiPluginView, UiPluginError> {
        let slot_id = plugin.slot_id().clone();
        let old = self
            .slots
            .get(&slot_id)
            .ok_or_else(|| UiPluginError::UnknownSlot(slot_id.as_str().to_owned()))?;
        let selection = old
            .view
            .as_ref()
            .and_then(|view| view.selection().map(str::to_owned));
        let projection = old
            .view
            .as_ref()
            .and_then(|view| view.projection().cloned());
        let mut plugin = plugin;
        plugin.mount()?;
        let mut view = if let Some(projection) = projection {
            plugin.render(projection, selection.clone())?
        } else {
            UiPluginView::new(
                plugin.definition().clone(),
                UiPluginState::Ready,
                None,
                selection.clone(),
            )
        };
        view.selection = selection;
        self.slots.insert(
            slot_id,
            SlotEntry {
                plugin,
                view: Some(view.clone()),
            },
        );
        Ok(view)
    }

    pub fn unmount(&mut self, slot_id: &SlotId) -> Result<(), UiPluginError> {
        let mut entry = self
            .slots
            .remove(slot_id)
            .ok_or_else(|| UiPluginError::UnknownSlot(slot_id.as_str().to_owned()))?;
        entry.plugin.unmount()?;
        Ok(())
    }

    pub fn get(&self, slot_id: &SlotId) -> Option<&dyn UiPlugin> {
        self.slots.get(slot_id).map(|entry| entry.plugin.as_ref())
    }

    pub fn view(&self, slot_id: &SlotId) -> Option<&UiPluginView> {
        self.slots
            .get(slot_id)
            .and_then(|entry| entry.view.as_ref())
    }

    pub fn contains(&self, slot_id: &SlotId) -> bool {
        self.slots.contains_key(slot_id)
    }
}
