use std::io::{self, BufRead};

use freehand_v2_contracts::{CapabilityId, NodeId};
use freehand_v2_topology_plugin::{TopologyNode, TopologyPlugin};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    machine_id: Option<String>,
    node_id: Option<String>,
    agent_id: Option<String>,
    channel_id: Option<String>,
    capabilities: Option<Vec<String>>,
    focus: Option<String>,
}

fn main() -> io::Result<()> {
    let mut plugin = TopologyPlugin::new();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let command: Command = serde_json::from_str(&line).unwrap_or(Command {
            action: String::new(),
            machine_id: None,
            node_id: None,
            agent_id: None,
            channel_id: None,
            capabilities: None,
            focus: None,
        });
        match dispatch(&mut plugin, command) {
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
    plugin: &mut TopologyPlugin,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match command.action.as_str() {
        "load" => {
            let node = TopologyNode::new(
                command.machine_id.unwrap_or_default(),
                NodeId::try_new(command.node_id.unwrap_or_default()).map_err(|e| e.to_string())?,
                command.agent_id.unwrap_or_default(),
                command.channel_id.unwrap_or_default(),
                command
                    .capabilities
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| CapabilityId::try_new(c).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, String>>()?,
            )
            .map_err(|e| e.to_string())?;
            plugin.load(vec![node]);
            Ok(map(vec![("loaded", serde_json::Value::Bool(true))]))
        }
        "focus" => {
            plugin
                .focus(command.focus.unwrap_or_default())
                .map_err(|e| e.to_string())?;
            Ok(map(vec![("focused", serde_json::Value::Bool(true))]))
        }
        "list" => Ok(map(vec![(
            "projection",
            serde_json::to_value(plugin.publish()).unwrap_or_default(),
        )])),
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
