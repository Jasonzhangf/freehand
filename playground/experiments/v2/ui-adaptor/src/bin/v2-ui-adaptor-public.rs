use std::io::{self, BufRead};
use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, CorrelationId, ImmutablePayload, SessionId, UiCommand};
use freehand_v2_ui_adaptor::{ProjectionKind, SlotId, UiAdaptor, UiError, UiSubscribe};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Command {
    action: String,
    slot: Option<String>,
    kind: Option<String>,
    source: Option<String>,
    payload: Option<String>,
    correlation_id: Option<String>,
    session_id: Option<String>,
    capability_id: Option<String>,
    subscription_id: Option<String>,
    cursor: Option<u64>,
}

fn main() -> io::Result<()> {
    let mut adaptor = UiAdaptor::new();
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
        match dispatch(&mut adaptor, command) {
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
    adaptor: &mut UiAdaptor,
    command: Command,
) -> Result<serde_json::Map<String, serde_json::Value>, UiError> {
    match command.action.as_str() {
        "accept" => {
            let slot = slot(command.slot)?;
            let correlation = CorrelationId::try_new(command.correlation_id.unwrap_or_default())
                .map_err(|err| UiError::InvalidProjection(format!("correlation_id: {err}")))?;
            let session = SessionId::try_new(command.session_id.unwrap_or_default())
                .map_err(|err| UiError::InvalidProjection(format!("session_id: {err}")))?;
            let capability = CapabilityId::try_new(command.capability_id.unwrap_or_default())
                .map_err(|err| UiError::InvalidProjection(format!("capability_id: {err}")))?;
            let payload = Arc::new(
                ImmutablePayload::new(command.payload.unwrap_or_default())
                    .map_err(|err| UiError::InvalidProjection(err.to_string()))?,
            );
            let ui_command = UiCommand::new(correlation, session, capability, (*payload).clone());
            let receipt = adaptor.accept_command(slot, &ui_command)?;
            Ok(map(vec![
                ("receipt_id", serde_json::json!(receipt.receipt_id())),
                (
                    "status",
                    serde_json::json!(match receipt.status() {
                        freehand_v2_ui_adaptor::UiCommandReceiptStatus::Accepted => "accepted",
                        freehand_v2_ui_adaptor::UiCommandReceiptStatus::Rejected => "rejected",
                        freehand_v2_ui_adaptor::UiCommandReceiptStatus::Failed => "failed",
                    }),
                ),
                ("message", serde_json::json!(receipt.message())),
            ]))
        }
        "publish" => {
            let slot = slot(command.slot)?;
            let kind = projection_kind(command.kind)?;
            let source = command.source.unwrap_or_default();
            let payload = Arc::new(
                ImmutablePayload::new(command.payload.unwrap_or_default())
                    .map_err(|err| UiError::InvalidProjection(err.to_string()))?,
            );
            let projection = adaptor.publish_projection(slot, kind, source, payload)?;
            Ok(map(vec![(
                "projection",
                serde_json::to_value(projection.to_wire()).unwrap_or(serde_json::Value::Null),
            )]))
        }
        "query" => {
            let slot = slot(command.slot)?;
            let projection = adaptor.query(&slot)?;
            Ok(map(vec![(
                "projection",
                serde_json::to_value(projection.to_wire()).unwrap_or(serde_json::Value::Null),
            )]))
        }
        "subscribe" => {
            let slot = slot(command.slot)?;
            let subscription = UiSubscribe::new(
                command.subscription_id.unwrap_or_default(),
                slot,
                command.cursor.unwrap_or(0),
            )?;
            adaptor.subscribe(subscription)?;
            Ok(map(vec![("subscribed", serde_json::Value::Bool(true))]))
        }
        other => Err(UiError::InvalidProjection(format!(
            "unknown action: {other}"
        ))),
    }
}

fn slot(value: Option<String>) -> Result<SlotId, UiError> {
    SlotId::try_new(value.unwrap_or_default())
}

fn projection_kind(value: Option<String>) -> Result<ProjectionKind, UiError> {
    match value.as_deref().unwrap_or_default() {
        "run" => Ok(ProjectionKind::Run),
        "sessions" => Ok(ProjectionKind::Sessions),
        "attention" => Ok(ProjectionKind::Attention),
        "location" => Ok(ProjectionKind::Location),
        "more" => Ok(ProjectionKind::More),
        "detail" => Ok(ProjectionKind::Detail),
        "search" => Ok(ProjectionKind::Search),
        "memory" => Ok(ProjectionKind::Memory),
        other => Err(UiError::InvalidProjection(format!(
            "unknown projection kind: {other}"
        ))),
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
