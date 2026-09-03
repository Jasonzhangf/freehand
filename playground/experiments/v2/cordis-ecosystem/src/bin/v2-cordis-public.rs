use std::io::{self, BufRead};
use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, CorrelationId, ImmutablePayload, PluginId};
use freehand_v2_cordis_ecosystem::{CordisContext, CordisError, public_result};
use freehand_v2_plugin_capabilities::{CapabilityManifest, LocalCapabilityPlugin};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
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
    let mut context = CordisContext::new();
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
        match dispatch(&mut context, command) {
            Ok(value) => {
                let mut object = serde_json::Map::new();
                object.insert("ok".to_owned(), serde_json::Value::Bool(true));
                for (key, item) in value {
                    object.insert(key, item);
                }
                println!("{}", serde_json::Value::Object(object));
            }
            Err(error) => output_error(error.to_string()),
        }
    }
    Ok(())
}

fn dispatch(
    context: &mut CordisContext,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, CordisError> {
    match command.action.as_str() {
        "register" | "replace" => {
            let plugin = build_plugin(command.clone())?;
            let registered = if command.action == "replace" {
                context.replace_capability(plugin)?
            } else {
                context.register(plugin)?
            };
            Ok(map(vec![
                (
                    "capability_id",
                    registered.capability_id().as_str().to_owned(),
                ),
                ("input_contract", registered.input_contract().to_owned()),
                ("output_contract", registered.output_contract().to_owned()),
                ("plugin_id", registered.plugin_id().as_str().to_owned()),
            ]))
        }
        "compose" => {
            let capability_id = CapabilityId::try_new(command.capability_id.unwrap_or_default())
                .map_err(|err| CordisError::EventId(err.to_string()))?;
            let correlation = CorrelationId::try_new(command.correlation_id.unwrap_or_default())
                .map_err(|err| CordisError::EventId(err.to_string()))?;
            let payload = Arc::new(
                ImmutablePayload::new(command.payload.unwrap_or_default())
                    .map_err(|err| CordisError::EventId(err.to_string()))?,
            );
            let result = context.invoke(correlation, capability_id, payload)?;
            let mut object = public_result(result.invocation())
                .as_object()
                .cloned()
                .unwrap_or_default();
            object.insert(
                "events".to_owned(),
                serde_json::json!(
                    result
                        .event_sequence()
                        .iter()
                        .map(|event| format!("{event:?}"))
                        .collect::<Vec<_>>()
                ),
            );
            Ok(object)
        }
        "unload" => {
            let capability_id = CapabilityId::try_new(command.capability_id.unwrap_or_default())
                .map_err(|err| CordisError::EventId(err.to_string()))?;
            context.unload(&capability_id)?;
            Ok(map(vec![(
                "capability_id",
                capability_id.as_str().to_owned(),
            )]))
        }
        other => Err(CordisError::Capability(
            freehand_v2_plugin_capabilities::CapabilityError::InvalidManifest(format!(
                "unknown action: {other}"
            )),
        )),
    }
}

fn build_plugin(
    command: Command,
) -> Result<Box<dyn freehand_v2_plugin_capabilities::CapabilityPlugin>, CordisError> {
    let plugin_id = PluginId::try_new(command.plugin_id.unwrap_or_default())
        .map_err(|err| CordisError::EventId(err.to_string()))?;
    let capability_id = CapabilityId::try_new(command.capability_id.unwrap_or_default())
        .map_err(|err| CordisError::EventId(err.to_string()))?;
    let manifest = CapabilityManifest::try_new(
        plugin_id,
        capability_id,
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
    Ok(Box::new(plugin))
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
