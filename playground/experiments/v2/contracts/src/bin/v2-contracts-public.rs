use std::io::{self, BufRead, Write};
use std::sync::Arc;

use freehand_v2_contracts::{
    CapabilityId, ControlEvent, ControlKind, CorrelationId, EventId, ImmutablePayload, PayloadRef,
    SessionId, UiCommand,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeRequest {
    correlation_id: String,
    session_id: String,
    capability_id: String,
    payload: String,
}

fn handle_line(line: &str) -> Result<serde_json::Value, String> {
    let request: ProbeRequest = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let correlation_id =
        CorrelationId::try_new(request.correlation_id).map_err(|error| error.to_string())?;
    let session_id = SessionId::try_new(request.session_id).map_err(|error| error.to_string())?;
    let capability_id =
        CapabilityId::try_new(request.capability_id).map_err(|error| error.to_string())?;
    let payload = ImmutablePayload::new(request.payload).map_err(|error| error.to_string())?;
    let command = UiCommand::new(
        correlation_id.clone(),
        session_id,
        capability_id,
        payload.clone(),
    );
    let control = ControlEvent::new(
        EventId::try_new("event-public-probe").map_err(|error| error.to_string())?,
        correlation_id,
        ControlKind::PluginInvoked,
        Some(PayloadRef::new("payload-public-probe").map_err(|error| error.to_string())?),
    );
    let control_json = serde_json::to_string(&control).map_err(|error| error.to_string())?;

    Ok(json!({
        "accepted": true,
        "payload": command.payload().body(),
        "payload_shared_locally": Arc::ptr_eq(payload.arc(), command.payload().arc()),
        "control_event": {
            "json": control_json,
            "contains_business_payload": control_json.contains(command.payload().body()),
        },
    }))
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());

    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) if !line.trim().is_empty() => match handle_line(&line) {
                Ok(value) => value,
                Err(error) => json!({"accepted": false, "error": error}),
            },
            Ok(_) => continue,
            Err(error) => json!({"accepted": false, "error": error.to_string()}),
        };
        serde_json::to_writer(&mut stdout, &response).expect("write response");
        stdout.write_all(b"\n").expect("write response newline");
        stdout.flush().expect("flush response");
    }
}
