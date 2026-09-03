use std::io::{self, BufRead};

use freehand_v2_search_plugin::{SearchPlugin, SearchRecord};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    record_id: Option<String>,
    kind: Option<String>,
    source_identity: Option<String>,
    keywords: Option<Vec<String>>,
    payload_ref: Option<String>,
    keyword: Option<String>,
}

fn main() -> io::Result<()> {
    let mut plugin = SearchPlugin::new();
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
    plugin: &mut SearchPlugin,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match command.action.as_str() {
        "index" => {
            let record = SearchRecord::new(
                command.record_id.unwrap_or_default(),
                command.kind.unwrap_or_default(),
                command.source_identity.unwrap_or_default(),
                command.keywords.unwrap_or_default(),
                command.payload_ref,
            )
            .map_err(|e| e.to_string())?;
            plugin.index(record).map_err(|e| e.to_string())?;
            Ok(map(vec![("indexed", serde_json::Value::Bool(true))]))
        }
        "query" => {
            let results = plugin
                .query(command.keyword.as_deref().unwrap_or_default())
                .map_err(|e| e.to_string())?;
            Ok(map(vec![
                ("count", serde_json::json!(results.len())),
                ("results", serde_json::to_value(results).unwrap_or_default()),
            ]))
        }
        "invalidate" => {
            plugin.invalidate();
            Ok(map(vec![("invalidated", serde_json::Value::Bool(true))]))
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
