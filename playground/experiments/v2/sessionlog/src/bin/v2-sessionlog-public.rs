use std::io::{self, BufRead, Write};

use freehand_v2_contracts::{EventId, SessionId};
use freehand_v2_sessionlog::{EventKind, SessionLog};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeRequest {
    session_id: String,
    event_id: String,
    data: String,
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());

    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) if !line.trim().is_empty() => match run_probe(&line) {
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

fn run_probe(line: &str) -> Result<serde_json::Value, String> {
    let request: ProbeRequest = serde_json::from_str(line).map_err(|error| error.to_string())?;
    let session_id = SessionId::try_new(request.session_id).map_err(|error| error.to_string())?;
    let event_id = EventId::try_new(request.event_id).map_err(|error| error.to_string())?;

    let mut log = SessionLog::new();
    log.create_session(session_id.clone(), 1, Some("public-probe".to_owned()))
        .map_err(|error| error.to_string())?;
    let cursor = log
        .append_event(
            &session_id,
            event_id,
            2,
            EventKind::Input,
            request.data.clone(),
            None,
            vec![],
            false,
        )
        .map_err(|error| error.to_string())?;
    let surface = log
        .derive_surface(&session_id)
        .map_err(|error| error.to_string())?;

    Ok(json!({
        "accepted": true,
        "session_id": session_id.as_str(),
        "last_applied_seq": cursor.last_applied_seq(),
        "surface_nodes": surface.nodes.len(),
        "quoted_input_present": surface.nodes.iter().any(|node| node.content == request.data),
    }))
}
