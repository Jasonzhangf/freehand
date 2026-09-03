use std::io::{self, BufRead};
use std::sync::Arc;

use freehand_v2_contracts::{CorrelationId, EventId, ImmutablePayload, SessionId};
use freehand_v2_reasoning_backend::{
    BackendId, NativeBackend, OpenCodeBackend, ReasoningBackend, ReasoningCursor, ReasoningEvent,
    ReasoningRequest, ReasoningService, RuntimeGroupId,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
struct Command {
    action: String,
    group: Option<String>,
    backend: Option<String>,
    session_id: Option<String>,
    correlation_id: Option<String>,
    payload: Option<String>,
    generation: Option<u64>,
}

fn main() -> io::Result<()> {
    let mut service = ReasoningService::new();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let command: Command = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                output(&Err(format!("invalid command: {err}")));
                continue;
            }
        };
        let result = dispatch(&mut service, command);
        output(&result);
    }
    Ok(())
}

fn dispatch(service: &mut ReasoningService, command: Command) -> Result<ReasoningEvent, String> {
    match command.action.as_str() {
        "bind" => {
            let group = runtime_group(command.group)?;
            let backend: Box<dyn ReasoningBackend> = match command.backend.as_deref() {
                Some("native") => Box::new(NativeBackend::new().map_err(|err| err.to_string())?),
                Some("opencode") => {
                    Box::new(OpenCodeBackend::new().map_err(|err| err.to_string())?)
                }
                other => return Err(format!("unknown backend: {other:?}")),
            };
            let capability = service
                .bind(group, backend)
                .map_err(|err| err.to_string())?;
            Ok(ReasoningEvent::new(
                EventId::try_new("bind").expect("event id"),
                SessionId::try_new("bind").expect("session id"),
                CorrelationId::try_new("bind").expect("correlation id"),
                freehand_v2_reasoning_backend::ReasoningEventKind::Started,
                capability.backend_id().clone(),
                1,
                Arc::new(ImmutablePayload::new(capability.provider()).expect("payload")),
            ))
        }
        "start" => {
            let request_command = command.clone();
            let group = runtime_group(command.group)?;
            let request = request(request_command)?;
            service
                .start(&group, request)
                .map_err(|err| err.to_string())
        }
        "replace" => {
            let group = runtime_group(command.group)?;
            let backend: Box<dyn ReasoningBackend> = match command.backend.as_deref() {
                Some("native") => Box::new(NativeBackend::new().map_err(|err| err.to_string())?),
                Some("opencode") => {
                    Box::new(OpenCodeBackend::new().map_err(|err| err.to_string())?)
                }
                other => return Err(format!("unknown backend: {other:?}")),
            };
            let capability = service
                .replace_backend(&group, backend)
                .map_err(|err| err.to_string())?;
            Ok(ReasoningEvent::new(
                EventId::try_new("replace").expect("event id"),
                SessionId::try_new("replace").expect("session id"),
                CorrelationId::try_new("replace").expect("correlation id"),
                freehand_v2_reasoning_backend::ReasoningEventKind::Started,
                capability.backend_id().clone(),
                1,
                Arc::new(ImmutablePayload::new(capability.provider()).expect("payload")),
            ))
        }
        "resume" => {
            let request_command = command.clone();
            let session = SessionId::try_new(command.session_id.as_deref().unwrap_or(""))
                .expect("session id");
            let backend_id =
                BackendId::try_new(command.backend.as_deref().unwrap_or("")).expect("backend id");
            let cursor = ReasoningCursor::try_new(
                session.clone(),
                EventId::try_new("cursor").expect("event id"),
                backend_id,
                command.generation.unwrap_or(1),
                1,
            )
            .map_err(|err| err.to_string())?;
            let request = request(request_command)?;
            service
                .resume(cursor, request)
                .map_err(|err| err.to_string())
        }
        "interrupt" => {
            let session = session(command.session_id)?;
            service.interrupt(&session).map_err(|err| err.to_string())
        }
        "inspect" => {
            let session = session(command.session_id)?;
            let state = service.inspect(&session).map_err(|err| err.to_string())?;
            Ok(ReasoningEvent::new(
                EventId::try_new("inspect").expect("event id"),
                session,
                CorrelationId::try_new("inspect").expect("correlation id"),
                match state {
                    freehand_v2_reasoning_backend::ReasoningState::Running => {
                        freehand_v2_reasoning_backend::ReasoningEventKind::Delta
                    }
                    _ => freehand_v2_reasoning_backend::ReasoningEventKind::Response,
                },
                BackendId::try_new("inspect").expect("backend id"),
                0,
                Arc::new(
                    ImmutablePayload::new(serde_json::to_string(&state).expect("state"))
                        .expect("payload"),
                ),
            ))
        }
        "subscribe" => {
            let session = session(command.session_id)?;
            let correlation =
                CorrelationId::try_new(command.correlation_id.as_deref().unwrap_or("subscribe"))
                    .expect("correlation id");
            service
                .subscribe(&session, &correlation)
                .map_err(|err| err.to_string())
        }
        other => Err(format!("unknown action: {other}")),
    }
}

fn output(result: &Result<ReasoningEvent, String>) {
    match result {
        Ok(event) => {
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "event_id": event.event_id().as_str(),
                    "session_id": event.session_id().as_str(),
                    "correlation_id": event.correlation_id().as_str(),
                    "kind": format!("{:?}", event.kind()),
                    "backend": event.backend_id().as_str(),
                    "generation": event.generation(),
                    "payload": event.payload().body(),
                })
            );
        }
        Err(error) => {
            println!("{}", serde_json::json!({ "ok": false, "error": error }));
        }
    }
}

fn runtime_group(value: Option<String>) -> Result<RuntimeGroupId, String> {
    RuntimeGroupId::try_new(value.unwrap_or_default()).map_err(|err| err.to_string())
}

fn session(value: Option<String>) -> Result<SessionId, String> {
    SessionId::try_new(value.unwrap_or_default()).map_err(|err| err.to_string())
}

fn request(command: Command) -> Result<ReasoningRequest, String> {
    let session = session(command.session_id)?;
    let correlation = CorrelationId::try_new(command.correlation_id.unwrap_or_default())
        .map_err(|err| err.to_string())?;
    let payload = Arc::new(
        ImmutablePayload::new(command.payload.unwrap_or_default())
            .map_err(|err| err.to_string())?,
    );
    Ok(ReasoningRequest::new(session, correlation, payload, None))
}
