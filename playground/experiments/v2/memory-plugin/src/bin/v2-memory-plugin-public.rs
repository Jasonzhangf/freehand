use std::io::{self, BufRead};

use freehand_v2_contracts::SessionId;
use freehand_v2_memory_plugin::{MemoryPlugin, MemoryRecord};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    session_id: Option<String>,
    record_id: Option<String>,
    summary: Option<String>,
    provenance: Option<String>,
    payload_ref: Option<String>,
    keyword: Option<String>,
}

fn main() -> io::Result<()> {
    let mut plugin = MemoryPlugin::new();
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
    plugin: &mut MemoryPlugin,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let session = command
        .session_id
        .map(SessionId::try_new)
        .transpose()
        .map_err(|e| e.to_string())?;
    match command.action.as_str() {
        "attach" => {
            plugin.attach(session.expect("session_id required"));
            Ok(map(vec![("attached", serde_json::Value::Bool(true))]))
        }
        "save" => {
            let record = MemoryRecord::new(
                command.record_id.unwrap_or_default(),
                session.expect("session_id required"),
                command.summary.unwrap_or_default(),
                command.provenance.unwrap_or_default(),
                command.payload_ref,
            )
            .map_err(|e| e.to_string())?;
            plugin.summarize(record).map_err(|e| e.to_string())?;
            Ok(map(vec![("saved", serde_json::Value::Bool(true))]))
        }
        "load" => Ok(map(vec![(
            "records",
            serde_json::to_value(plugin.load(&session.expect("session_id required")))
                .unwrap_or_default(),
        )])),
        "search" => Ok(map(vec![(
            "records",
            serde_json::to_value(
                plugin
                    .search(command.keyword.as_deref().unwrap_or_default())
                    .map_err(|e| e.to_string())?,
            )
            .unwrap_or_default(),
        )])),
        "export" => Ok(map(vec![(
            "export",
            serde_json::to_value(
                plugin
                    .export(&session.expect("session_id required"))
                    .map_err(|e| e.to_string())?,
            )
            .unwrap_or_default(),
        )])),
        "detach" => {
            plugin
                .detach(&session.expect("session_id required"))
                .map_err(|e| e.to_string())?;
            Ok(map(vec![("detached", serde_json::Value::Bool(true))]))
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
