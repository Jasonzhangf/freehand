use std::io::{self, BufRead};

use freehand_v2_contracts::PluginId;
use freehand_v2_notification_plugin::{Importance, NotificationPlugin};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    notification_id: Option<String>,
    source: Option<String>,
    importance: Option<String>,
    occurred_at: Option<u64>,
    payload_ref: Option<String>,
}

fn main() -> io::Result<()> {
    let mut plugin = NotificationPlugin::new();
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
        match dispatch(&mut plugin, command) {
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
    plugin: &mut NotificationPlugin,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match command.action.as_str() {
        "admit" => {
            let source = PluginId::try_new(command.source.unwrap_or_default())
                .map_err(|err| err.to_string())?;
            let importance = match command.importance.as_deref().unwrap_or_default() {
                "critical" => Importance::Critical,
                "high" => Importance::High,
                "medium" => Importance::Medium,
                "low" => Importance::Low,
                other => return Err(format!("unknown importance: {other}")),
            };
            let item = plugin
                .admit(
                    command.notification_id.unwrap_or_default(),
                    source,
                    importance,
                    command.occurred_at.unwrap_or(0),
                    command.payload_ref,
                )
                .map_err(|err| err.to_string())?;
            Ok(map(vec![
                ("notification_id", serde_json::json!(item.notification_id())),
                (
                    "importance",
                    serde_json::json!(format!("{:?}", item.importance()).to_lowercase()),
                ),
            ]))
        }
        "list" => {
            let projection = plugin.publish();
            Ok(map(vec![
                ("revision", serde_json::json!(projection.revision())),
                (
                    "items",
                    serde_json::to_value(projection.items()).unwrap_or_default(),
                ),
            ]))
        }
        "ack" => {
            plugin
                .acknowledge(command.notification_id.as_deref().unwrap_or_default())
                .map_err(|err| err.to_string())?;
            Ok(map(vec![("acknowledged", serde_json::Value::Bool(true))]))
        }
        "snooze" => {
            plugin
                .snooze(command.notification_id.as_deref().unwrap_or_default())
                .map_err(|err| err.to_string())?;
            Ok(map(vec![("snoozed", serde_json::Value::Bool(true))]))
        }
        "archive" => {
            plugin
                .archive(command.notification_id.as_deref().unwrap_or_default())
                .map_err(|err| err.to_string())?;
            Ok(map(vec![("archived", serde_json::Value::Bool(true))]))
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

fn output_error(message: String) {
    println!("{}", serde_json::json!({ "ok": false, "error": message }));
}
