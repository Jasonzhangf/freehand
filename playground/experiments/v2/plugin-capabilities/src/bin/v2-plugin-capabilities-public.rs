use std::io::{self, BufRead};
use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, CorrelationId, ImmutablePayload, PluginId};
use freehand_v2_plugin_capabilities::{
    CapabilityError, CapabilityManifest, CapabilityRegistry, LocalCapabilityPlugin,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    plugin_id: Option<String>,
    capability_id: Option<String>,
    input_contract: Option<String>,
    output_contract: Option<String>,
    events: Option<Vec<String>>,
    permissions: Option<Vec<String>>,
    scope: Option<String>,
    correlation_id: Option<String>,
    payload: Option<String>,
    fail_next: Option<bool>,
}

fn main() -> io::Result<()> {
    let mut registry = CapabilityRegistry::new();
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
    registry: &mut CapabilityRegistry,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, CapabilityError> {
    match command.action.as_str() {
        "register" => {
            let plugin_id = PluginId::try_new(command.plugin_id.unwrap_or_default())
                .map_err(|err| CapabilityError::InvalidManifest(format!("plugin_id: {err}")))?;
            let capability_id = capability_id(command.capability_id)?;
            let manifest = CapabilityManifest::try_new(
                plugin_id,
                capability_id.clone(),
                command.input_contract.unwrap_or_default(),
                command.output_contract.unwrap_or_default(),
                command.events.unwrap_or_default(),
                command.permissions.unwrap_or_default(),
                command.scope,
            )?;
            let mut plugin = LocalCapabilityPlugin::new(manifest)?;
            if command.fail_next.unwrap_or(false) {
                plugin = plugin.fail_next();
            }
            let registered = registry.register(Box::new(plugin))?;
            Ok(map(vec![
                ("capability_id", capability_id.as_str().to_owned()),
                ("input_contract", registered.input_contract().to_owned()),
                ("output_contract", registered.output_contract().to_owned()),
            ]))
        }
        "invoke" => {
            let capability_id = capability_id(command.capability_id)?;
            let correlation = CorrelationId::try_new(command.correlation_id.unwrap_or_default())
                .map_err(|err| CapabilityError::InvalidManifest(err.to_string()))?;
            let payload = Arc::new(
                ImmutablePayload::new(command.payload.unwrap_or_default())
                    .map_err(|err| CapabilityError::InvalidManifest(err.to_string()))?,
            );
            let plugin = registry.get(&capability_id).ok_or_else(|| {
                CapabilityError::UnknownCapability(capability_id.as_str().to_owned())
            })?;
            let invocation = plugin.invoke(&correlation, payload)?;
            Ok(map(vec![
                ("result_id", invocation.result_id().as_str().to_owned()),
                (
                    "capability_id",
                    invocation.capability_id().as_str().to_owned(),
                ),
                (
                    "correlation_id",
                    invocation.correlation_id().as_str().to_owned(),
                ),
                ("success", invocation.success().to_string()),
                ("payload", invocation.payload().body().to_owned()),
            ]))
        }
        "unload" => {
            let capability_id = capability_id(command.capability_id)?;
            registry.unload(&capability_id)?;
            Ok(map(vec![(
                "capability_id",
                capability_id.as_str().to_owned(),
            )]))
        }
        other => Err(CapabilityError::InvalidManifest(format!(
            "unknown action: {other}"
        ))),
    }
}

fn capability_id(value: Option<String>) -> Result<CapabilityId, CapabilityError> {
    let value = value.unwrap_or_default();
    CapabilityId::try_new(value)
        .map_err(|err| CapabilityError::InvalidManifest(format!("capability_id: {err}")))
}

fn map(values: Vec<(&str, String)>) -> serde_json::Map<String, serde_json::Value> {
    let mut object = serde_json::Map::new();
    for (key, value) in values {
        object.insert(key.to_owned(), serde_json::Value::String(value));
    }
    object
}

fn output_error(message: String) {
    println!("{}", serde_json::json!({ "ok": false, "error": message }));
}
