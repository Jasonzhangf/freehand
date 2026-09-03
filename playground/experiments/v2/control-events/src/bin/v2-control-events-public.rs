use std::io::{self, BufRead, Write};

use freehand_v2_contracts::{ControlKind, CorrelationId, ErrorKind, EventId, PayloadRef, PluginId};
use freehand_v2_control_events::{EventLedger, LedgerCursor};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeRequest {
    action: String,
    event_id: Option<String>,
    correlation_id: Option<String>,
    owner: Option<String>,
    kind: Option<String>,
    payload_ref: Option<String>,
    cursor: Option<u64>,
    error_kind: Option<String>,
    message: Option<String>,
}

trait ParseId {
    fn parse(value: &str) -> Result<Self, String>
    where
        Self: Sized;
}

impl ParseId for EventId {
    fn parse(value: &str) -> Result<Self, String> {
        EventId::try_new(value).map_err(|error| error.to_string())
    }
}

impl ParseId for CorrelationId {
    fn parse(value: &str) -> Result<Self, String> {
        CorrelationId::try_new(value).map_err(|error| error.to_string())
    }
}

impl ParseId for PluginId {
    fn parse(value: &str) -> Result<Self, String> {
        PluginId::try_new(value).map_err(|error| error.to_string())
    }
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut ledger = EventLedger::new();

    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) if !line.trim().is_empty() => match run_probe(&mut ledger, &line) {
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

fn run_probe(ledger: &mut EventLedger, line: &str) -> Result<serde_json::Value, String> {
    let request: ProbeRequest = serde_json::from_str(line).map_err(|error| error.to_string())?;

    match request.action.as_str() {
        "emit" => {
            let event_id = parse_id::<EventId>(request.event_id.as_deref())?;
            let correlation_id = parse_id::<CorrelationId>(request.correlation_id.as_deref())?;
            let owner = parse_id::<PluginId>(request.owner.as_deref())?;
            let kind = parse_kind(request.kind.as_deref())?;
            let payload_ref = request
                .payload_ref
                .map(|value| PayloadRef::new(value).map_err(|error| error.to_string()))
                .transpose()?;
            let cursor = ledger
                .emit(event_id, correlation_id, kind, owner, payload_ref)
                .map_err(|error| error.to_string())?;
            Ok(json!({
                "accepted": true,
                "last_applied_seq": cursor.last_applied_seq(),
                "event_count": ledger.events().len(),
            }))
        }
        "ack" => {
            let event_id = parse_id::<EventId>(request.event_id.as_deref())?;
            let cursor = ledger
                .acknowledge(&event_id)
                .map_err(|error| error.to_string())?;
            Ok(json!({
                "accepted": true,
                "ack_cursor": cursor.last_applied_seq(),
            }))
        }
        "complete" => {
            let correlation_id = parse_id::<CorrelationId>(request.correlation_id.as_deref())?;
            let count = ledger
                .complete(&correlation_id)
                .map_err(|error| error.to_string())?;
            Ok(json!({"accepted": true, "terminalized": count}))
        }
        "reject" => {
            let correlation_id = parse_id::<CorrelationId>(request.correlation_id.as_deref())?;
            let kind = parse_error_kind(request.error_kind.as_deref())?;
            let message = request.message.unwrap_or_else(|| "rejected".to_owned());
            let source_event_id = request
                .event_id
                .as_ref()
                .map(|value| parse_id::<EventId>(Some(value)))
                .transpose()
                .map_err(|error| error.to_string())?;
            let error = ledger
                .reject(correlation_id, kind, message, source_event_id)
                .map_err(|error| error.to_string())?;
            Ok(json!({
                "accepted": false,
                "error_seq": error.seq(),
                "error_message": error.message(),
            }))
        }
        "replay" => {
            let cursor = LedgerCursor::new(request.cursor.unwrap_or(0));
            let events = ledger
                .replay_from(&cursor)
                .map_err(|error| error.to_string())?;
            Ok(json!({
                "accepted": true,
                "replayed": events.len(),
                "seqs": events.iter().map(|event| event.seq()).collect::<Vec<_>>(),
            }))
        }
        other => Err(format!("unknown action: {other}")),
    }
}

fn parse_id<T>(value: Option<&str>) -> Result<T, String>
where
    T: ParseId,
{
    let value = value.ok_or("missing id")?;
    T::parse(value)
}

fn parse_kind(value: Option<&str>) -> Result<ControlKind, String> {
    match value {
        Some("PluginInvoked") => Ok(ControlKind::PluginInvoked),
        Some("PluginCompleted") => Ok(ControlKind::PluginCompleted),
        Some("PluginFailed") => Ok(ControlKind::PluginFailed),
        other => Err(format!(
            "unknown control kind: {}",
            other.unwrap_or("missing")
        )),
    }
}

fn parse_error_kind(value: Option<&str>) -> Result<ErrorKind, String> {
    match value {
        Some("Rejected") => Ok(ErrorKind::Rejected),
        Some("InvalidPayload") => Ok(ErrorKind::InvalidPayload),
        other => Err(format!(
            "unknown error kind: {}",
            other.unwrap_or("missing")
        )),
    }
}
