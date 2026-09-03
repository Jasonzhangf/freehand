use std::io::{self, BufRead};

use freehand_v2_contracts::{SessionId, TurnId};
use freehand_v2_session_canvas_plugin::{CanvasBand, CanvasEdge, CanvasNode, SessionCanvasPlugin};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    session_id: Option<String>,
    turn_id: Option<String>,
    band: Option<String>,
    parent_session_id: Option<String>,
    edge_source: Option<String>,
    edge_target: Option<String>,
}

fn main() -> io::Result<()> {
    let mut plugin = SessionCanvasPlugin::new();
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
    plugin: &mut SessionCanvasPlugin,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match command.action.as_str() {
        "derive" => {
            let session = SessionId::try_new(command.session_id.unwrap_or_default())
                .map_err(|e| e.to_string())?;
            let turn =
                TurnId::try_new(command.turn_id.unwrap_or_default()).map_err(|e| e.to_string())?;
            let band = match command.band.as_deref().unwrap_or_default() {
                "active" => CanvasBand::Active,
                "recent" => CanvasBand::Recent,
                "history" => CanvasBand::History,
                other => return Err(format!("unknown band: {other}")),
            };
            let parent = command
                .parent_session_id
                .map(SessionId::try_new)
                .transpose()
                .map_err(|e| e.to_string())?;
            plugin
                .derive(vec![CanvasNode::new(session, turn, band, parent)], vec![])
                .map_err(|e| e.to_string())?;
            Ok(map(vec![("derived", serde_json::Value::Bool(true))]))
        }
        "edge" => {
            let source = SessionId::try_new(command.edge_source.unwrap_or_default())
                .map_err(|e| e.to_string())?;
            let target = SessionId::try_new(command.edge_target.unwrap_or_default())
                .map_err(|e| e.to_string())?;
            let nodes = plugin.publish().nodes().to_vec();
            let mut edges = plugin.publish().edges().to_vec();
            edges.push(CanvasEdge::new(source, target));
            plugin.derive(nodes, edges).map_err(|e| e.to_string())?;
            Ok(map(vec![("edge", serde_json::Value::Bool(true))]))
        }
        "focus" => {
            let session = SessionId::try_new(command.session_id.unwrap_or_default())
                .map_err(|e| e.to_string())?;
            plugin.focus(session).map_err(|e| e.to_string())?;
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
