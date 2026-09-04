use std::io::{self, BufRead, Write};
use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, CorrelationId, ImmutablePayload, SessionId, UiCommand};
use freehand_v2_public_vertical_slice::PublicVerticalSlice;
use freehand_v2_ui_adaptor::{ProjectionKind, SlotId};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    action: String,
    slot: Option<String>,
    correlation_id: Option<String>,
    session_id: Option<String>,
    capability_id: Option<String>,
    payload: Option<String>,
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut slice = PublicVerticalSlice::new();

    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) if !line.trim().is_empty() => match serde_json::from_str(&line) {
                Ok(command) => match dispatch(&mut slice, command) {
                    Ok(value) => value,
                    Err(error) => json!({"ok": false, "error": error}),
                },
                Err(error) => json!({"ok": false, "error": format!("invalid command: {error}")}),
            },
            Ok(_) => continue,
            Err(error) => json!({"ok": false, "error": error.to_string()}),
        };
        serde_json::to_writer(&mut stdout, &response).expect("write response");
        stdout.write_all(b"\n").expect("write response newline");
        stdout.flush().expect("flush response");
    }
}

fn dispatch(slice: &mut PublicVerticalSlice, command: Command) -> Result<Value, String> {
    let slot = slot(command.slot.as_deref().unwrap_or("run"))?;
    let action = command.action.clone();
    match action.as_str() {
        "submit" | "begin" => {
            let ui_command = ui_command(command)?;
            let payload_arc = Arc::clone(ui_command.payload().arc());
            let session_id = ui_command.session_id().clone();
            let outcome = if action == "submit" {
                slice
                    .submit(ui_command)
                    .map_err(|error| error.to_string())?
            } else {
                slice.begin(ui_command).map_err(|error| error.to_string())?
            };
            Ok(outcome_json(
                slice,
                &outcome,
                &session_id,
                &slot,
                Arc::ptr_eq(&payload_arc, outcome.payload_arc().arc()),
            ))
        }
        "resume" => {
            let correlation = CorrelationId::try_new(command.correlation_id.unwrap_or_default())
                .map_err(|error| error.to_string())?;
            let outcome = slice
                .resume(&correlation)
                .map_err(|error| error.to_string())?;
            let session_id = SessionId::try_new(command.session_id.unwrap_or_default())
                .map_err(|error| error.to_string())?;
            Ok(outcome_json(slice, &outcome, &session_id, &slot, true))
        }
        "query" => {
            let projection = slice
                .query_projection(&slot)
                .map_err(|error| error.to_string())?;
            Ok(json!({
                "ok": true,
                "projection": projection.to_wire(),
            }))
        }
        "status" => {
            let session_id = SessionId::try_new(command.session_id.unwrap_or_default())
                .map_err(|error| error.to_string())?;
            Ok(json!({
                "ok": true,
                "projection_count": slice.projection_count(&slot),
                "control_event_count": slice.control_events().len(),
                "session_event_count": slice.session_events(&session_id).len(),
            }))
        }
        other => Err(format!("unknown action: {other}")),
    }
}

fn ui_command(command: Command) -> Result<UiCommand, String> {
    let correlation_id = CorrelationId::try_new(command.correlation_id.unwrap_or_default())
        .map_err(|error| format!("correlation_id: {error}"))?;
    let session_id = SessionId::try_new(command.session_id.unwrap_or_default())
        .map_err(|error| format!("session_id: {error}"))?;
    let capability_id = CapabilityId::try_new(command.capability_id.unwrap_or_default())
        .map_err(|error| format!("capability_id: {error}"))?;
    let payload = ImmutablePayload::new(command.payload.unwrap_or_default())
        .map_err(|error| format!("payload: {error}"))?;
    Ok(UiCommand::new(
        correlation_id,
        session_id,
        capability_id,
        payload,
    ))
}

fn outcome_json(
    slice: &mut PublicVerticalSlice,
    outcome: &freehand_v2_public_vertical_slice::TurnOutcome,
    session_id: &SessionId,
    slot: &SlotId,
    payload_shared_locally: bool,
) -> Value {
    json!({
        "ok": true,
        "receipt": {
            "receipt_id": outcome.receipt().receipt_id(),
            "status": match outcome.receipt().status() {
                freehand_v2_ui_adaptor::UiCommandReceiptStatus::Accepted => "accepted",
                freehand_v2_ui_adaptor::UiCommandReceiptStatus::Rejected => "rejected",
                freehand_v2_ui_adaptor::UiCommandReceiptStatus::Failed => "failed",
            },
            "message": outcome.receipt().message(),
        },
        "waiting": outcome.is_waiting(),
        "payload": outcome.payload_arc().body(),
        "payload_shared_locally": payload_shared_locally,
        "projection": if outcome.is_waiting() {
            serde_json::Value::Null
        } else {
            serde_json::to_value(outcome.projection().to_wire()).unwrap_or(serde_json::Value::Null)
        },
        "projection_count": slice.projection_count(slot),
        "control_event_count": slice.control_events().len(),
        "session_event_count": slice.session_events(session_id).len(),
        "projection_kind": ProjectionKind::Run,
    })
}

fn slot(value: &str) -> Result<SlotId, String> {
    SlotId::try_new(value).map_err(|error| error.to_string())
}
