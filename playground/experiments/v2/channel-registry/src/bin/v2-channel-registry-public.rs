use std::io::{self, BufRead};

use freehand_v2_channel_registry::{BearerToken, ChannelRegistry, FrameKind};
use freehand_v2_contracts::{CapabilityId, CorrelationId, NodeId};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    endpoint_id: Option<String>,
    node_id: Option<String>,
    token: Option<String>,
    protocol_version: Option<u32>,
    capabilities: Option<Vec<String>>,
    session_id: Option<String>,
    connection_id: Option<String>,
    kind: Option<String>,
    correlation_id: Option<String>,
    payload_ref: Option<String>,
    message: Option<String>,
    cursor: Option<u64>,
}

fn main() -> io::Result<()> {
    let mut registry = ChannelRegistry::new();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let command: Command = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                println!(
                    "{}",
                    serde_json::json!({ "ok": false, "error": error.to_string() })
                );
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
            Err(error) => println!("{}", serde_json::json!({ "ok": false, "error": error })),
        }
    }
    Ok(())
}

fn dispatch(
    registry: &mut ChannelRegistry,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match command.action.as_str() {
        "register" => {
            let token = BearerToken::try_new(command.token.unwrap_or_default())
                .map_err(|e| e.to_string())?;
            let capabilities = command
                .capabilities
                .unwrap_or_default()
                .into_iter()
                .map(|c| CapabilityId::try_new(c).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, String>>()?;
            let manifest = registry
                .register(
                    command.endpoint_id.unwrap_or_default(),
                    NodeId::try_new(command.node_id.unwrap_or_default())
                        .map_err(|e| e.to_string())?,
                    token,
                    command.protocol_version.unwrap_or(1),
                    capabilities,
                )
                .map_err(|e| e.to_string())?;
            Ok(map(vec![
                ("endpoint_id", serde_json::json!(manifest.endpoint_id())),
                ("generation", serde_json::json!(manifest.generation())),
            ]))
        }
        "open" => {
            let token = BearerToken::try_new(command.token.unwrap_or_default())
                .map_err(|e| e.to_string())?;
            let session = registry
                .open_session(
                    command.session_id.unwrap_or_default(),
                    command.endpoint_id.as_deref().unwrap_or_default(),
                    &token,
                )
                .map_err(|e| e.to_string())?;
            Ok(map(vec![
                ("session_id", serde_json::json!(session.session_id())),
                (
                    "state",
                    serde_json::json!(format!("{:?}", session.state()).to_lowercase()),
                ),
            ]))
        }
        "attach" => {
            let connection = registry
                .attach_connection(
                    command.session_id.as_deref().unwrap_or_default(),
                    command.connection_id.unwrap_or_default(),
                )
                .map_err(|e| e.to_string())?;
            Ok(map(vec![
                (
                    "connection_id",
                    serde_json::json!(connection.connection_id()),
                ),
                ("generation", serde_json::json!(connection.generation())),
            ]))
        }
        "replace" => {
            let connection = registry
                .replace_connection(
                    command.session_id.as_deref().unwrap_or_default(),
                    command.connection_id.unwrap_or_default(),
                )
                .map_err(|e| e.to_string())?;
            Ok(map(vec![
                (
                    "connection_id",
                    serde_json::json!(connection.connection_id()),
                ),
                ("generation", serde_json::json!(connection.generation())),
            ]))
        }
        "suspend" => {
            registry
                .suspend(command.session_id.as_deref().unwrap_or_default())
                .map_err(|e| e.to_string())?;
            Ok(map(vec![("suspended", serde_json::Value::Bool(true))]))
        }
        "reattach" => {
            let connection = registry
                .reattach(
                    command.session_id.as_deref().unwrap_or_default(),
                    command.connection_id.unwrap_or_default(),
                )
                .map_err(|e| e.to_string())?;
            Ok(map(vec![
                (
                    "connection_id",
                    serde_json::json!(connection.connection_id()),
                ),
                ("generation", serde_json::json!(connection.generation())),
            ]))
        }
        "send" => {
            let kind = match command.kind.as_deref().unwrap_or_default() {
                "control" => FrameKind::Control,
                "payload" => FrameKind::Payload,
                "error" => FrameKind::Error,
                other => return Err(format!("unknown kind: {other}")),
            };
            let correlation = CorrelationId::try_new(command.correlation_id.unwrap_or_default())
                .map_err(|e| e.to_string())?;
            let frame = registry
                .send(
                    command.session_id.as_deref().unwrap_or_default(),
                    kind,
                    correlation,
                    command.payload_ref,
                    command.message,
                )
                .map_err(|e| e.to_string())?;
            Ok(map(vec![
                ("seq", serde_json::json!(frame.seq())),
                ("event_id", serde_json::json!(frame.event_id().as_str())),
            ]))
        }
        "replay" => {
            let frames = registry
                .replay(
                    command.session_id.as_deref().unwrap_or_default(),
                    command.cursor.unwrap_or(0),
                )
                .map_err(|e| e.to_string())?;
            Ok(map(vec![
                ("count", serde_json::json!(frames.len())),
                ("frames", serde_json::to_value(frames).unwrap_or_default()),
            ]))
        }
        other => Err(format!("unknown action: {other}")),
    }
}

fn map(values: Vec<(&str, serde_json::Value)>) -> serde_json::Map<String, serde_json::Value> {
    let mut object = serde_json::Map::new();
    for (key, value) in values {
        object.insert(key.to_owned(), value);
    }
    object
}
