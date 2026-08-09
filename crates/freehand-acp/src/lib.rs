//! Freehand ACP v1 agent surface.
//!
//! Implements the Agent Client Protocol (ACP) v1 — JSON-RPC 2.0 over NDJSON —
//! by delegating wire schema, JSON-RPC framing, and stdio transport to the
//! official `agent-client-protocol` SDK. Freehand only provides the Agent
//! component that adapts the runtime live-reason turn mainline
//! (`run_live_reason_turn`) onto the ACP method surface. The internal ADP WebUI
//! surface is left untouched.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::v1::{
    AgentCapabilities, CancelNotification, ContentBlock, Implementation, InitializeRequest,
    InitializeResponse, NewSessionRequest, NewSessionResponse, PromptCapabilities, PromptRequest,
    PromptResponse, SessionId, StopReason,
};
use agent_client_protocol::{Agent, Client, ConnectTo, Result};
use freehand_contracts::{SessionId as RuntimeSessionId, TraceId, TurnId};
use freehand_runtime::{
    LiveReasonExecutionProfile, LiveReasonTurnRequest, RuntimeLiveBridgeError,
    load_default_runtime_agent, run_live_reason_turn,
};

/// One ACP session binding: working directory plus cancel token shared with the
/// runtime live-reason turn mainline.
#[derive(Debug)]
struct AcpSession {
    session_id: SessionId,
    cwd: PathBuf,
    cancel: Arc<AtomicBool>,
}

type SessionRegistry = Arc<Mutex<HashMap<SessionId, Arc<AcpSession>>>>;

/// ACP v1 agent component backed by the Freehand runtime live-reason turn
/// mainline. Implements `ConnectTo<Client>` so a daemon can host it over stdio.
pub struct FreehandAgent {
    sessions: SessionRegistry,
}

impl Default for FreehandAgent {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl ConnectTo<Client> for FreehandAgent {
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<()> {
        let sessions = Arc::clone(&self.sessions);

        Agent
            .builder()
            .name("freehand")
            .on_receive_request(
                async move |req: InitializeRequest, responder, _cx| {
                    let resp = InitializeResponse::new(req.protocol_version)
                        .agent_info(
                            Implementation::new("freehand", env!("CARGO_PKG_VERSION"))
                                .title("Freehand"),
                        )
                        .agent_capabilities(AgentCapabilities::new().prompt_capabilities(
                            PromptCapabilities::new().image(false).audio(false),
                        ));
                    responder.respond(resp)?;
                    Ok(())
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                {
                    let sessions = Arc::clone(&sessions);
                    async move |req: NewSessionRequest, responder, _cx| {
                        let session_id = SessionId::new(format!("acp-{}", monotonic_id()));
                        let acp_session = Arc::new(AcpSession {
                            session_id: session_id.clone(),
                            cwd: req.cwd,
                            cancel: Arc::new(AtomicBool::new(false)),
                        });
                        sessions
                            .lock()
                            .unwrap()
                            .insert(session_id.clone(), Arc::clone(&acp_session));
                        responder.respond(NewSessionResponse::new(session_id))?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                {
                    let sessions = Arc::clone(&sessions);
                    async move |notif: CancelNotification, _cx| {
                        if let Some(session) = sessions.lock().unwrap().get(&notif.session_id) {
                            session.cancel.store(true, Ordering::SeqCst);
                        }
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                {
                    let sessions = Arc::clone(&sessions);
                    async move |req: PromptRequest, responder, _cx| {
                        let session = sessions
                            .lock()
                            .unwrap()
                            .get(&req.session_id)
                            .cloned()
                            .ok_or_else(|| {
                                agent_client_protocol::Error::invalid_params()
                                    .data(format!("unknown session: {}", req.session_id))
                            })?;
                        let prompt_text = extract_text(&req.prompt);

                        let stop_reason =
                            run_prompt_with_reset(&session, &prompt_text, run_turn_blocking).await;
                        responder.respond(PromptResponse::new(stop_reason))?;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_to(client)
            .await
    }
}

#[derive(Debug)]
enum TurnError {
    Cancelled,
    Refusal,
}

/// Join the textual blocks of a prompt into a single string for the runtime.
fn extract_text(blocks: &[ContentBlock]) -> String {
    let mut out = String::new();
    for block in blocks {
        if let ContentBlock::Text(text) = block {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&text.text);
        }
    }
    out
}

/// A turn runner decides what happens for one prompt.
pub(crate) type TurnRunner = fn(&AcpSession, &str) -> Result<(), TurnError>;

/// Run a prompt through the configured turn runner and reset the session
/// cancel token after the turn returns so a stale cancel cannot brick the
/// next prompt.
pub(crate) async fn run_prompt_with_reset(
    session: &Arc<AcpSession>,
    prompt_text: &str,
    turn: TurnRunner,
) -> StopReason {
    let stop_reason = match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            let task = {
                let session = Arc::clone(session);
                let prompt_text = prompt_text.to_owned();
                handle.spawn_blocking(move || turn(&session, &prompt_text))
            };
            match task.await {
                Ok(Ok(())) => StopReason::EndTurn,
                Ok(Err(TurnError::Cancelled)) => StopReason::Cancelled,
                Ok(Err(TurnError::Refusal)) => StopReason::Refusal,
                Err(e) => {
                    eprintln!("[freehand-acp] spawn_blocking join error: {e}");
                    StopReason::Refusal
                }
            }
        }
        Err(_) => StopReason::Refusal,
    };
    session.cancel.store(false, Ordering::SeqCst);
    stop_reason
}

/// Drive one live-reason turn synchronously on the current blocking pool.
/// Returns `Ok(())` when the turn completed normally, `Err(Cancelled)` when the
/// runtime observed a cancel token flip, and `Err(Refusal)` for any other
/// runtime/bootstrap failure.
fn run_turn_blocking(session: &AcpSession, prompt: &str) -> Result<(), TurnError> {
    let bootstrap = load_default_runtime_agent("master").map_err(|_| TurnError::Refusal)?;
    let request = LiveReasonTurnRequest {
        runtime_home: bootstrap.runtime_home,
        session_id: RuntimeSessionId::new(session.session_id.to_string()),
        turn_id: TurnId::new(format!("acp-turn-{}", monotonic_id())),
        trace_id: TraceId::new(format!("acp-trace-{}", monotonic_id())),
        prompt: prompt.to_owned(),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: Some(session.cwd.clone()),
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream: false,
        cancel_token: Some(Arc::clone(&session.cancel)),
    };
    match run_live_reason_turn(&bootstrap.selected_agent, request) {
        Ok(_) => Ok(()),
        Err(RuntimeLiveBridgeError::Cancelled) => Err(TurnError::Cancelled),
        Err(_) => Err(TurnError::Refusal),
    }
}

/// Monotonic id generator used for ACP session/turn/trace ids.
fn monotonic_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

#[cfg(test)]
mod tests;
