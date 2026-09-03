use std::io::{self, BufRead};
use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, ImmutablePayload, PluginId};
use freehand_v2_ui_adaptor::{ProjectionKind, SlotId};
use freehand_v2_ui_plugin_family::{
    InMemoryUiPlugin, UiPluginDefinition, UiPluginError, UiPluginSlotRegistry,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    plugin_id: Option<String>,
    slot: Option<String>,
    instance_id: Option<String>,
    contract_version: Option<u32>,
    capabilities: Option<Vec<String>>,
    payload: Option<String>,
    source: Option<String>,
    selection: Option<String>,
    projection_id: Option<String>,
}

fn main() -> io::Result<()> {
    let mut registry = UiPluginSlotRegistry::new();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let command: Command = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                output_error(format!("invalid command: {error}"));
                continue;
            }
        };
        match dispatch(&mut registry, command) {
            Ok(body) => {
                let mut object = serde_json::Map::new();
                object.insert("ok".to_owned(), serde_json::Value::Bool(true));
                for (key, value) in body {
                    object.insert(key, value);
                }
                println!("{}", serde_json::Value::Object(object));
            }
            Err(error) => output_error(error.to_string()),
        }
    }
    Ok(())
}

fn dispatch(
    registry: &mut UiPluginSlotRegistry,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, UiPluginError> {
    match command.action.as_str() {
        "mount" => {
            let plugin = in_memory_plugin(command)?;
            let definition = registry.mount(Box::new(plugin))?;
            Ok(map(vec![
                (
                    "plugin_id",
                    serde_json::json!(definition.plugin_id().as_str()),
                ),
                ("slot_id", serde_json::json!(definition.slot_id().as_str())),
            ]))
        }
        "replace" => {
            let plugin = in_memory_plugin(command)?;
            let view = registry.replace(Box::new(plugin))?;
            Ok(map(vec![("view", view.to_public_json())]))
        }
        "render" => {
            let slot = SlotId::try_new(command.slot.unwrap_or_default())
                .map_err(|err| UiPluginError::InvalidDefinition(err.to_string()))?;
            let payload = Arc::new(
                ImmutablePayload::new(command.payload.unwrap_or_default())
                    .map_err(|err| UiPluginError::InvalidDefinition(err.to_string()))?,
            );
            let projection = freehand_v2_ui_adaptor::UiProjection::new(
                command.projection_id.unwrap_or_else(|| "render".to_owned()),
                slot.clone(),
                ProjectionKind::Run,
                command
                    .source
                    .unwrap_or_else(|| "ui-plugins-public".to_owned()),
                payload,
            )
            .map_err(|err| UiPluginError::InvalidDefinition(err.to_string()))?;
            let view = registry.render(&slot, projection, command.selection)?;
            Ok(map(vec![("view", view.to_public_json())]))
        }
        "unmount" => {
            let slot = SlotId::try_new(command.slot.unwrap_or_default())
                .map_err(|err| UiPluginError::InvalidDefinition(err.to_string()))?;
            registry.unmount(&slot)?;
            Ok(map(vec![("slot_id", serde_json::json!(slot.as_str()))]))
        }
        "status" => {
            let slot = SlotId::try_new(command.slot.unwrap_or_default())
                .map_err(|err| UiPluginError::InvalidDefinition(err.to_string()))?;
            let view = registry
                .view(&slot)
                .ok_or_else(|| UiPluginError::UnknownSlot(slot.as_str().to_owned()))?;
            Ok(map(vec![("view", view.to_public_json())]))
        }
        other => Err(UiPluginError::InvalidDefinition(format!(
            "unknown action: {other}"
        ))),
    }
}

fn in_memory_plugin(command: Command) -> Result<InMemoryUiPlugin, UiPluginError> {
    let plugin_id = PluginId::try_new(command.plugin_id.unwrap_or_default())
        .map_err(|err| UiPluginError::InvalidDefinition(format!("plugin_id: {err}")))?;
    let slot_id = SlotId::try_new(command.slot.unwrap_or_default())
        .map_err(|err| UiPluginError::InvalidDefinition(format!("slot_id: {err}")))?;
    let mut capabilities = Vec::new();
    for capability in command.capabilities.unwrap_or_default() {
        capabilities.push(
            CapabilityId::try_new(capability)
                .map_err(|err| UiPluginError::InvalidDefinition(format!("capability: {err}")))?,
        );
    }
    let definition = UiPluginDefinition::try_new(
        plugin_id,
        slot_id,
        command.instance_id.unwrap_or_default(),
        command.contract_version.unwrap_or(1),
        capabilities,
    )?;
    let mut plugin = InMemoryUiPlugin::new(definition)?;
    if let Some(selection) = command.selection {
        plugin = plugin.with_selection(selection);
    }
    Ok(plugin)
}

fn map(values: Vec<(&str, serde_json::Value)>) -> serde_json::Map<String, serde_json::Value> {
    let mut object = serde_json::Map::new();
    for (key, value) in values {
        object.insert(key.to_owned(), value);
    }
    object
}

fn output_error(message: String) {
    println!("{}", serde_json::json!({ "ok": false, "error": message }));
}
