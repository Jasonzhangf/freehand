//! Freehand ACP v1 agent surface.
//!
//! Implements the Agent Client Protocol (ACP) v1 — JSON-RPC 2.0 over NDJSON —
//! by delegating wire schema, JSON-RPC framing, and stdio transport to the
//! official `agent-client-protocol` SDK. Freehand provides the Agent component
//! that adapts the runtime live-reason turn mainline
//! (`run_live_reason_turn_with_hooks`) onto the ACP method surface, including
//! streaming `session/update` projections of reasoning, message, tool, and
//! tool-result events. Usage events are not projected because the runtime
//! lacks a provider context-window projection. Session lifecycle is limited
//! to transport-local session/new, prompt, and cancel; list/load/resume/close
//! are out of scope. The internal ADP WebUI surface is left untouched.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::v1::{
    AgentCapabilities, CancelNotification, ContentBlock, ContentChunk, Implementation,
    InitializeRequest, InitializeResponse, MessageId, NewSessionRequest, NewSessionResponse,
    PromptCapabilities, PromptRequest, PromptResponse, SessionId, SessionNotification,
    SessionUpdate, StopReason, TextContent, ToolCall, ToolCallId, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind,
};
use agent_client_protocol::{Agent, Client, ConnectTo, ConnectionTo, Result};
use freehand_contracts::{
    ReasonReq04ToolCall, ReasonReq05ToolResultReentry, ReasonResp01SemanticEvent,
    ReasonResp02UsageEvent, SemanticEventKind, SessionId as RuntimeSessionId, ToolArgument,
    ToolCallContract, ToolResultStatus, TraceId, TurnId,
};
use freehand_runtime::{
    LiveReasonExecutionProfile, LiveReasonTurnRequest, ReasonBroadcastEvent,
    RuntimeLiveBridgeError, load_default_runtime_agent, run_live_reason_turn_with_hooks,
};

/// One ACP session binding: working directory and the cancel token shared
/// with the runtime live-reason turn mainline. Session and turn truth lives
/// in the runtime reason persistence; this registry only holds
/// transport-local handles.
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
                    let capabilities = AgentCapabilities::new()
                        .load_session(false)
                        .prompt_capabilities(
                            PromptCapabilities::new()
                                .image(false)
                                .audio(false)
                                .embedded_context(false),
                        );
                    let resp = InitializeResponse::new(req.protocol_version)
                        .agent_info(
                            Implementation::new("freehand", env!("CARGO_PKG_VERSION"))
                                .title("Freehand"),
                        )
                        .agent_capabilities(capabilities)
                        .auth_methods(Vec::new());
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
                    async move |req: PromptRequest, responder, cx| {
                        let session = sessions
                            .lock()
                            .unwrap()
                            .get(&req.session_id)
                            .cloned()
                            .ok_or_else(|| {
                                agent_client_protocol::Error::invalid_params().data(
                                    serde_json::Value::String(format!(
                                        "unknown session: {}",
                                        req.session_id.0
                                    )),
                                )
                            })?;
                        let prompt_text = extract_text(&req.prompt);

                        let stop_reason =
                            run_prompt_with_reset(&session, &prompt_text, run_turn_blocking, cx)
                                .await;
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

/// A turn runner decides what happens for one prompt. It receives the ACP
/// connection so the runtime broadcast hooks can stream `session/update`
/// notifications back to the client during the turn.
pub(crate) type TurnRunner<C> = fn(&Arc<AcpSession>, &str, &C) -> Result<(), TurnError>;

/// Run a prompt through the configured turn runner and reset the session
/// cancel token after the turn returns so a stale cancel cannot brick the
/// next prompt.
pub(crate) async fn run_prompt_with_reset<C>(
    session: &Arc<AcpSession>,
    prompt_text: &str,
    turn: TurnRunner<C>,
    cx: C,
) -> StopReason
where
    C: Send + 'static,
{
    let stop_reason = match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            let task = {
                let session = Arc::clone(session);
                let prompt_text = prompt_text.to_owned();
                handle.spawn_blocking(move || turn(&session, &prompt_text, &cx))
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
/// Streams runtime broadcast events to the client as ACP `session/update`
/// notifications and returns `Ok(())` when the turn completed normally,
/// `Err(Cancelled)` when the runtime observed a cancel token flip, and
/// `Err(Refusal)` for any other runtime/bootstrap failure.
fn run_turn_blocking(
    session: &Arc<AcpSession>,
    prompt: &str,
    cx: &ConnectionTo<Client>,
) -> Result<(), TurnError> {
    let bootstrap = load_default_runtime_agent("master").map_err(|_| TurnError::Refusal)?;
    let request = LiveReasonTurnRequest {
        runtime_home: bootstrap.runtime_home.clone(),
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
    let cx = cx.clone();
    let mut broadcaster = AcpBroadcaster {
        session_id: session.session_id.clone(),
        cx,
        error_seen: false,
        send_failed: false,
    };
    match run_live_reason_turn_with_hooks(
        &bootstrap.selected_agent,
        request,
        |event| broadcaster.on_broadcast(event),
        |_debug| {},
        |_projection| {},
    ) {
        Ok(_) => {
            if broadcaster.send_failed {
                // A streaming notification failed to reach the client; do not
                // report a clean turn when the client missed part of the stream.
                return Err(TurnError::Refusal);
            }
            if broadcaster.error_seen {
                eprintln!("[freehand-acp] runtime error event observed during turn");
            }
            Ok(())
        }
        Err(RuntimeLiveBridgeError::Cancelled) => Err(TurnError::Cancelled),
        Err(_) => Err(TurnError::Refusal),
    }
}

/// Streams runtime broadcast events to the ACP client as `session/update`
/// notifications, grouping assistant message deltas by a stable `messageId`
/// and tool lifecycle by `toolCallId` (matching the pocketcode display model).
struct AcpBroadcaster {
    session_id: SessionId,
    cx: ConnectionTo<Client>,
    error_seen: bool,
    send_failed: bool,
}

impl AcpBroadcaster {
    fn on_broadcast(&mut self, event: &ReasonBroadcastEvent) {
        if matches!(event, ReasonBroadcastEvent::Error(_)) {
            self.error_seen = true;
            return;
        }
        let notifications = project_broadcast(&self.session_id, event);
        for notification in notifications {
            if let Err(err) = self.cx.send_notification(notification) {
                self.send_failed = true;
                eprintln!("[freehand-acp] send_notification failed: {err}");
            }
        }
    }
}

/// Project one runtime broadcast event into zero or more ACP `session/update`
/// notifications.
fn project_broadcast(
    session_id: &SessionId,
    event: &ReasonBroadcastEvent,
) -> Vec<SessionNotification> {
    match event {
        ReasonBroadcastEvent::Semantic(semantic) => project_semantic(session_id, semantic),
        ReasonBroadcastEvent::Tool(tool) => project_tool_call(session_id, tool),
        ReasonBroadcastEvent::ToolResult(result) => project_tool_result(session_id, result),
        ReasonBroadcastEvent::Usage(usage) => project_usage(session_id, usage),
        // Completion-schema-rejected, model-continuation-waiting, and terminal
        // events are runtime control signals with no ACP session/update
        // projection; Error is recorded by AcpBroadcaster (error_seen) for
        // diagnostics and is not projected into a business notification.
        ReasonBroadcastEvent::CompletionSchemaRejected(_)
        | ReasonBroadcastEvent::SearchEvidenceSchemaRejected(_)
        | ReasonBroadcastEvent::ModelContinuationWaiting(_)
        | ReasonBroadcastEvent::Terminal(_)
        | ReasonBroadcastEvent::SearchEvidence(_)
        | ReasonBroadcastEvent::Error(_) => Vec::new(),
    }
}

pub(crate) fn project_semantic(
    session_id: &SessionId,
    semantic: &ReasonResp01SemanticEvent,
) -> Vec<SessionNotification> {
    if semantic.content.trim().is_empty() {
        return Vec::new();
    }
    let content = ContentBlock::Text(TextContent::new(semantic.content.clone()));
    // PocketCode groups chunks by messageId; keep thought and answer in
    // distinct message groups so the client never merges reasoning into the
    // final answer. The kind prefix is stable per semantic class.
    let kind_tag = match semantic.kind {
        SemanticEventKind::Reasoning => "thought",
        _ => "answer",
    };
    let chunk = ContentChunk::new(content).message_id(MessageId::new(format!(
        "acp-{kind_tag}-{}",
        semantic.turn_id.as_str()
    )));
    let update = match semantic.kind {
        SemanticEventKind::Reasoning => SessionUpdate::AgentThoughtChunk(chunk),
        _ => SessionUpdate::AgentMessageChunk(chunk),
    };
    vec![SessionNotification::new(session_id.clone(), update)]
}

pub(crate) fn project_tool_call(
    session_id: &SessionId,
    tool: &ReasonReq04ToolCall,
) -> Vec<SessionNotification> {
    let contract: &ToolCallContract = &tool.tool_call;
    let kind = tool_kind_for(&contract.tool_name, &contract.arguments);
    let raw_input = serde_json::json!({
        "name": contract.tool_name,
        "arguments": contract
            .arguments
            .iter()
            .map(|arg| (arg.name.clone(), arg.value.clone()))
            .collect::<Vec<_>>(),
    });
    let status = if contract.arguments_complete {
        ToolCallStatus::InProgress
    } else {
        ToolCallStatus::Pending
    };
    let tool_call = ToolCall::new(
        ToolCallId::new(contract.tool_call_id.as_str().to_owned()),
        contract.tool_name.clone(),
    )
    .kind(kind)
    .status(status)
    .raw_input(raw_input);
    vec![SessionNotification::new(
        session_id.clone(),
        SessionUpdate::ToolCall(tool_call),
    )]
}

pub(crate) fn project_tool_result(
    session_id: &SessionId,
    result: &ReasonReq05ToolResultReentry,
) -> Vec<SessionNotification> {
    let status = match result.tool_result.status {
        ToolResultStatus::Success => ToolCallStatus::Completed,
        ToolResultStatus::Failed => ToolCallStatus::Failed,
    };
    let update = ToolCallUpdate::new(
        ToolCallId::new(result.tool_result.tool_call_id.as_str().to_owned()),
        ToolCallUpdateFields::new().status(status).content(vec![
            ContentBlock::Text(TextContent::new(result.tool_result.output.clone())).into(),
        ]),
    );
    vec![SessionNotification::new(
        session_id.clone(),
        SessionUpdate::ToolCallUpdate(update),
    )]
}

pub(crate) fn project_usage(
    _session_id: &SessionId,
    _usage: &ReasonResp02UsageEvent,
) -> Vec<SessionNotification> {
    // The runtime does not yet expose a provider context-window size, and the
    // ACP SDK requires `size` to be set. Rather than fabricate a window size
    // we cannot prove (which would show a misleading 100%-full progress bar),
    // do not emit a usage_update until the reason.persistence owner exposes a
    // typed context-window projection.
    Vec::new()
}

pub(crate) fn tool_kind_for(name: &str, arguments: &[ToolArgument]) -> ToolKind {
    // Tool display classification is owned by `tool.display`
    // (crates/freehand-blocks::tool_display). ACP maps that owner's typed
    // display kind onto the ACP ToolKind enum; it does not re-classify tool
    // names itself, so the mapping stays consistent with the one classifier.
    match freehand_runtime::classify_tool_display_kind(name, arguments) {
        freehand_runtime::ToolDisplayKind::ReadFile | freehand_runtime::ToolDisplayKind::List => {
            ToolKind::Read
        }
        freehand_runtime::ToolDisplayKind::FileMutation => ToolKind::Edit,
        freehand_runtime::ToolDisplayKind::Search => ToolKind::Search,
        freehand_runtime::ToolDisplayKind::Shell => ToolKind::Execute,
        freehand_runtime::ToolDisplayKind::Plan
        | freehand_runtime::ToolDisplayKind::Task
        | freehand_runtime::ToolDisplayKind::Timer
        | freehand_runtime::ToolDisplayKind::Generic => ToolKind::Other,
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
