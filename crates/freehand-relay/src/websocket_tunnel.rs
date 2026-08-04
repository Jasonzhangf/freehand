use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{OriginalUri, Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::model::{
    AgentHeartbeat, RELAY_TUNNEL_PROTOCOL_VERSION, RelayControlInFrame, RelayControlOutFrame,
    RelayDataFrameKind, RelayDataInFrame, RelayDataOutFrame, RelayDataProtocol, RelayErrorInFrame,
    RelayErrorOutFrame,
};
use crate::service::{
    RelayState, authenticated_account, error_response, raw_agent_route_path, record_disconnect,
    record_heartbeat,
};
use crate::store::RelayStoreError;
use crate::tunnel::{
    RelayDataTunnelSender, RelayErrorTunnelSender, RelayExchangeAdmissionError,
    RelayPendingResponse, RelayResponsePart, RelayRoutableExchange, RelayTunnelIdentity,
};

const RESPONSE_OPEN_TIMEOUT: Duration = Duration::from_secs(30);
const CONTROL_GENERATION_HEADER: &str = "x-freehand-relay-control-generation";

fn control_generation(headers: &HeaderMap) -> Result<u64, RelayStoreError> {
    headers
        .get(CONTROL_GENERATION_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            RelayStoreError::Invalid("Relay channel control generation is missing".to_owned())
        })?
        .parse()
        .map_err(|_| {
            RelayStoreError::Invalid("Relay channel control generation is invalid".to_owned())
        })
}

pub(crate) async fn control_tunnel(
    Path(agent_id): Path<String>,
    State(state): State<RelayState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let account_id = match authenticated_account(&state, &headers).await {
        Ok(account_id) => account_id,
        Err(error) => return error_response(error),
    };
    let identity = RelayTunnelIdentity {
        account_id,
        agent_id,
    };
    upgrade
        .on_upgrade(move |socket| run_control_socket(socket, state, identity))
        .into_response()
}

pub(crate) async fn data_tunnel(
    Path(agent_id): Path<String>,
    State(state): State<RelayState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let account_id = match authenticated_account(&state, &headers).await {
        Ok(account_id) => account_id,
        Err(error) => return error_response(error),
    };
    let identity = RelayTunnelIdentity {
        account_id,
        agent_id,
    };
    let control_generation = match control_generation(&headers) {
        Ok(generation) => generation,
        Err(error) => return error_response(error),
    };
    let (sender, receiver) = mpsc::channel(32);
    let generation = match attach_data(
        &state,
        identity.clone(),
        control_generation,
        RelayDataTunnelSender::new(sender),
    ) {
        Ok(generation) => generation,
        Err(error) => return error_response(RelayStoreError::Invalid(error)),
    };
    upgrade
        .on_upgrade(move |socket| run_data_socket(socket, state, identity, generation, receiver))
        .into_response()
}

pub(crate) async fn error_tunnel(
    Path(agent_id): Path<String>,
    State(state): State<RelayState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let account_id = match authenticated_account(&state, &headers).await {
        Ok(account_id) => account_id,
        Err(error) => return error_response(error),
    };
    let identity = RelayTunnelIdentity {
        account_id,
        agent_id,
    };
    let control_generation = match control_generation(&headers) {
        Ok(generation) => generation,
        Err(error) => return error_response(error),
    };
    let (sender, receiver) = mpsc::channel(32);
    let generation = match attach_error(
        &state,
        identity.clone(),
        control_generation,
        RelayErrorTunnelSender::new(sender),
    ) {
        Ok(generation) => generation,
        Err(error) => return error_response(RelayStoreError::Invalid(error)),
    };
    upgrade
        .on_upgrade(move |socket| run_error_socket(socket, state, identity, generation, receiver))
        .into_response()
}

pub(crate) async fn proxy_adp(
    Path(agent_id): Path<String>,
    State(state): State<RelayState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    proxy_websocket(
        state,
        agent_id,
        "/adp".to_owned(),
        RelayDataProtocol::Adp,
        headers,
        upgrade,
    )
    .await
}

pub(crate) async fn proxy_websocket_root(
    Path(agent_id): Path<String>,
    OriginalUri(uri): OriginalUri,
    State(state): State<RelayState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let local_path = match raw_agent_path_and_query(&agent_id, &uri) {
        Ok(path) => path,
        Err(error) => return error_response(error),
    };
    proxy_websocket(
        state,
        agent_id,
        local_path,
        RelayDataProtocol::WebSocket,
        headers,
        upgrade,
    )
    .await
}

pub(crate) async fn proxy_websocket_path(
    Path((agent_id, _path)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    State(state): State<RelayState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let local_path = match raw_agent_path_and_query(&agent_id, &uri) {
        Ok(path) => path,
        Err(error) => return error_response(error),
    };
    proxy_websocket(
        state,
        agent_id,
        local_path,
        RelayDataProtocol::WebSocket,
        headers,
        upgrade,
    )
    .await
}

fn raw_agent_path_and_query(
    agent_id: &str,
    uri: &axum::http::Uri,
) -> Result<String, RelayStoreError> {
    let path = raw_agent_route_path(agent_id, uri, Some("connect"))?;
    Ok(match uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path,
    })
}

async fn proxy_websocket(
    state: RelayState,
    agent_id: String,
    local_path: String,
    protocol: RelayDataProtocol,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !valid_websocket_target_path(&local_path)
        || (protocol == RelayDataProtocol::Adp && local_path != "/adp")
    {
        return error_response(RelayStoreError::Invalid(
            "WebSocket tunnel target path is invalid".to_owned(),
        ));
    }
    let account_id = match authenticated_account(&state, &headers).await {
        Ok(account_id) => account_id,
        Err(error) => return error_response(error),
    };
    let identity = RelayTunnelIdentity {
        account_id,
        agent_id,
    };
    let exchange_id = format!(
        "websocket-{}",
        state.exchange_sequence.fetch_add(1, Ordering::Relaxed)
    );
    let RelayRoutableExchange {
        data_sender: sender,
        pending,
        ..
    } = match open_websocket_exchange(&state, &identity, &exchange_id) {
        Ok(exchange) => exchange,
        Err(error) => return error_response(error),
    };
    if let Err(error) = sender
        .send(RelayDataOutFrame::RequestOpen {
            exchange_id: exchange_id.clone(),
            protocol,
            method: None,
            path_and_query: local_path,
            headers: Vec::new(),
        })
        .await
    {
        if let Err(cleanup) =
            fail_active_exchange(&state, &identity, &exchange_id, error.clone()).await
        {
            return error_response(RelayStoreError::Upstream(format!(
                "{error}; Relay exchange cleanup failed: {cleanup}"
            )));
        }
        return error_response(RelayStoreError::Upstream(error));
    }
    upgrade
        .on_upgrade(move |socket| {
            bridge_websocket(socket, state, identity, sender, pending, exchange_id)
        })
        .into_response()
}

fn valid_websocket_target_path(path: &str) -> bool {
    let decoded = percent_encoding::percent_decode_str(path).decode_utf8_lossy();
    decoded.starts_with('/')
        && !decoded.contains("://")
        && !decoded.split('/').any(|part| part == "..")
}

fn attach_control(
    state: &RelayState,
    identity: RelayTunnelIdentity,
) -> Result<crate::tunnel::RelayControlAdmission, String> {
    state
        .tunnels
        .lock()
        .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())?
        .attach_control(identity)
}

fn attach_data(
    state: &RelayState,
    identity: RelayTunnelIdentity,
    control_generation: u64,
    sender: RelayDataTunnelSender,
) -> Result<u64, String> {
    let mut tunnels = state
        .tunnels
        .lock()
        .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())?;
    if !tunnels.has_control(&identity) {
        return Err("Relay data tunnel requires an active control tunnel".to_owned());
    }
    tunnels.attach_data_for_control(identity, control_generation, sender)
}

fn attach_error(
    state: &RelayState,
    identity: RelayTunnelIdentity,
    control_generation: u64,
    sender: RelayErrorTunnelSender,
) -> Result<u64, String> {
    state
        .tunnels
        .lock()
        .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())?
        .admit_error_for_control(identity, control_generation, sender)
}

fn open_websocket_exchange(
    state: &RelayState,
    identity: &RelayTunnelIdentity,
    exchange_id: &str,
) -> Result<RelayRoutableExchange, RelayStoreError> {
    let mut tunnels = state
        .tunnels
        .lock()
        .map_err(|_| RelayStoreError::Io("Relay tunnel registry lock poisoned".to_owned()))?;
    tunnels
        .open_routable_exchange(identity.clone(), exchange_id.to_owned())
        .map_err(|error| match error {
            RelayExchangeAdmissionError::DataTunnelUnavailable => RelayStoreError::AgentNotFound,
            RelayExchangeAdmissionError::ErrorTunnelUnavailable
            | RelayExchangeAdmissionError::Invalid(_) => {
                RelayStoreError::Upstream(error.to_string())
            }
        })
}

async fn fail_active_exchange(
    state: &RelayState,
    identity: &RelayTunnelIdentity,
    exchange_id: &str,
    message: String,
) -> Result<(), String> {
    let delivery = state
        .tunnels
        .lock()
        .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())?
        .fail_exchange(identity, exchange_id, message)?;
    if let Some(delivery) = delivery {
        delivery.deliver().await?;
    }
    Ok(())
}

async fn run_control_socket(
    mut socket: WebSocket,
    state: RelayState,
    identity: RelayTunnelIdentity,
) {
    let mut heartbeat: Option<AgentHeartbeat> = None;
    let mut control_generation = None;
    while let Some(Ok(message)) = socket.next().await {
        let Message::Text(text) = message else {
            break;
        };
        let frame: RelayControlInFrame = match serde_json::from_str(text.as_str()) {
            Ok(frame) => frame,
            Err(_) => break,
        };
        let next = match frame {
            RelayControlInFrame::AgentIdentity {
                agent_id,
                display_name,
                node_id,
                role,
                status,
                active_session_count,
            } if heartbeat.is_none() && agent_id == identity.agent_id => AgentHeartbeat {
                agent_id,
                display_name,
                node_id,
                role,
                status,
                active_session_count,
            },
            RelayControlInFrame::PresenceHeartbeat {
                status,
                active_session_count,
            } => match heartbeat.clone() {
                Some(mut current) => {
                    current.status = status;
                    current.active_session_count = active_session_count;
                    current
                }
                None => break,
            },
            _ => break,
        };
        if control_generation.is_none() {
            match attach_control(&state, identity.clone()) {
                Ok(admission) => {
                    control_generation = Some(admission.generation);
                    for delivery in admission.replaced_deliveries {
                        if let Err(error) = delivery.deliver().await {
                            eprintln!(
                                "Relay stale control replacement delivery failed for {identity:?}: {error}"
                            );
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        } else if !state
            .tunnels
            .lock()
            .map(|tunnels| {
                tunnels.has_control_generation(
                    &identity,
                    control_generation.expect("control generation is present"),
                )
            })
            .unwrap_or(false)
        {
            break;
        }
        if record_heartbeat(&state, identity.account_id.clone(), next.clone())
            .await
            .is_err()
        {
            break;
        }
        if heartbeat.is_none() {
            let frame = RelayControlOutFrame::IdentityAccepted {
                protocol_version: RELAY_TUNNEL_PROTOCOL_VERSION,
                agent_id: identity.agent_id.clone(),
                control_generation: control_generation.expect("admitted control generation"),
            };
            let Ok(text) = serde_json::to_string(&frame) else {
                break;
            };
            if socket.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
        heartbeat = Some(next);
    }
    let cleanup_result = match control_generation {
        Some(generation) => state
            .tunnels
            .lock()
            .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())
            .and_then(|mut tunnels| {
                if !tunnels.detach_control(&identity, generation) {
                    return Ok(None);
                }
                tunnels.detach_current_error(&identity);
                tunnels.detach_current_data(&identity).map(Some)
            }),
        None => Ok(None),
    };
    match cleanup_result {
        Ok(Some(deliveries)) => {
            for delivery in deliveries {
                if let Err(error) = delivery.deliver().await {
                    eprintln!(
                        "Relay control tunnel cleanup delivery failed for {identity:?}: {error}"
                    );
                }
            }
        }
        Ok(None) => return,
        Err(error) => eprintln!("Relay control tunnel cleanup failed for {identity:?}: {error}"),
    }
    let account_id = identity.account_id.clone();
    let agent_id = identity.agent_id.clone();
    if let Err(error) = record_disconnect(&state, account_id, agent_id).await {
        eprintln!("Relay control presence disconnect failed for {identity:?}: {error}");
    }
}

#[derive(Debug, PartialEq, Eq)]
enum RelayWebSocketResponse {
    Frame(RelayDataFrameKind, Vec<u8>),
    SuccessEnd,
    ProtocolFailure(String),
    ErrorChainFailure(String),
}

fn classify_websocket_response(
    response: Option<Result<RelayResponsePart, String>>,
) -> RelayWebSocketResponse {
    match response {
        Some(Ok(RelayResponsePart::Chunk {
            frame_kind: Some(kind),
            bytes,
        })) => RelayWebSocketResponse::Frame(kind, bytes),
        Some(Ok(RelayResponsePart::End)) => RelayWebSocketResponse::SuccessEnd,
        Some(Ok(RelayResponsePart::Chunk {
            frame_kind: None, ..
        })) => RelayWebSocketResponse::ProtocolFailure(
            "Relay WebSocket exchange received an HTTP chunk".to_owned(),
        ),
        Some(Err(error)) => RelayWebSocketResponse::ErrorChainFailure(error),
        None => RelayWebSocketResponse::ErrorChainFailure(
            "Relay WebSocket response channel closed before response-end".to_owned(),
        ),
    }
}

async fn run_data_socket(
    socket: WebSocket,
    state: RelayState,
    identity: RelayTunnelIdentity,
    generation: u64,
    mut outbound: mpsc::Receiver<RelayDataOutFrame>,
) {
    let (mut sink, mut stream) = socket.split();
    let terminal_error = loop {
        tokio::select! {
            outgoing = outbound.recv() => match outgoing {
                Some(frame) => {
                    let text = match serde_json::to_string(&frame) {
                        Ok(text) => text,
                        Err(error) => break Some(format!("Relay data frame serialization failed: {error}")),
                    };
                    if let Err(error) = sink.send(Message::Text(text.into())).await {
                        break Some(format!("Relay data tunnel send failed: {error}"));
                    }
                }
                None => break None,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    let frame: RelayDataInFrame = match serde_json::from_str(text.as_str()) {
                        Ok(frame) => frame,
                        Err(error) => break Some(format!("Relay data frame is invalid: {error}")),
                    };
                    let delivery = state.tunnels.lock()
                        .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())
                        .and_then(|mut tunnels| {
                            tunnels.accept_data_generation(&identity, generation, frame)
                        });
                    let delivery = match delivery {
                        Ok(delivery) => delivery,
                        Err(error) => break Some(error),
                    };
                    if let Err(error) = delivery.deliver().await {
                        break Some(error);
                    }
                }
                Some(Ok(_)) => break Some("Relay data tunnel received a non-text contract frame".to_owned()),
                Some(Err(error)) => break Some(format!("Relay data tunnel receive failed: {error}")),
                None => break None,
            }
        }
    };
    if let Some(message) = terminal_error {
        let error_sender = state
            .tunnels
            .lock()
            .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())
            .and_then(|tunnels| tunnels.error_sender_for_data_generation(&identity, generation));
        match error_sender {
            Ok(sender) => {
                if let Err(error) = sender
                    .send(RelayErrorOutFrame::Terminal {
                        code: "relay_data_tunnel_terminal".to_owned(),
                        message,
                    })
                    .await
                {
                    eprintln!("Relay data terminal delivery failed for {identity:?}: {error}");
                }
            }
            Err(error) => {
                eprintln!("Relay data terminal owner unavailable for {identity:?}: {error}");
            }
        }
    }
    let cleanup_result = state
        .tunnels
        .lock()
        .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())
        .and_then(|mut tunnels| tunnels.detach_data(&identity, generation));
    match cleanup_result {
        Ok(deliveries) => {
            for delivery in deliveries {
                if let Err(error) = delivery.deliver().await {
                    eprintln!(
                        "Relay data tunnel cleanup delivery failed for {identity:?}: {error}"
                    );
                }
            }
        }
        Err(error) => eprintln!("Relay data tunnel cleanup failed for {identity:?}: {error}"),
    }
}

async fn run_error_socket(
    socket: WebSocket,
    state: RelayState,
    identity: RelayTunnelIdentity,
    generation: u64,
    mut outbound: mpsc::Receiver<RelayErrorOutFrame>,
) {
    let (mut sink, mut stream) = socket.split();
    let result = async {
        loop {
        let frame = tokio::select! {
            outgoing = outbound.recv() => match outgoing {
                Some(frame) => {
                    let text = serde_json::to_string(&frame)
                        .map_err(|error| format!("Relay error frame serialization failed: {error}"))?;
                    sink.send(Message::Text(text.into()))
                        .await
                        .map_err(|error| format!("Relay error tunnel send failed: {error}"))?;
                    continue;
                }
                None => return Ok::<(), String>(()),
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => serde_json::from_str(text.as_str())
                    .map_err(|error| format!("Relay error tunnel received an invalid frame: {error}"))?,
                Some(Ok(Message::Close(_))) | None => return Ok(()),
                Some(Ok(_)) => return Err("Relay error tunnel received a non-text contract frame".to_owned()),
                Some(Err(error)) => return Err(format!("Relay error tunnel receive failed: {error}")),
            }
        };
        let RelayErrorInFrame::TunnelFailure {
            exchange_id,
            code,
            message,
        } = frame;
        let exchange_id = exchange_id
            .ok_or_else(|| "Relay Agent error frame is missing its exchange id".to_owned())?;
        let delivery = state
            .tunnels
            .lock()
            .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())
            .and_then(|mut tunnels| {
                tunnels.fail_exchange_from_error_generation(
                    &identity,
                    generation,
                    &exchange_id,
                    format!("{code}: {message}"),
                )
            })?;
        if let Some(delivery) = delivery {
            delivery.deliver().await?;
        }
        }
    }
    .await;
    if let Err(error) = result {
        let terminal = RelayErrorOutFrame::Terminal {
            code: "relay_error_tunnel_terminal".to_owned(),
            message: error.clone(),
        };
        match serde_json::to_string(&terminal) {
            Ok(text) => {
                if let Err(delivery_error) = sink.send(Message::Text(text.into())).await {
                    eprintln!(
                        "Relay error tunnel terminal delivery failed for {identity:?}: {error}; {delivery_error}"
                    );
                } else {
                    eprintln!("Relay error tunnel terminated for {identity:?}: {error}");
                }
            }
            Err(serialization_error) => eprintln!(
                "Relay error tunnel terminal serialization failed for {identity:?}: {error}; {serialization_error}"
            ),
        }
    }
    match state.tunnels.lock() {
        Ok(mut tunnels) => {
            tunnels.detach_error(&identity, generation);
        }
        Err(_) => {
            eprintln!("Relay error tunnel detach failed for {identity:?}: registry lock poisoned")
        }
    }
}

async fn bridge_websocket(
    socket: WebSocket,
    state: RelayState,
    identity: RelayTunnelIdentity,
    sender: RelayDataTunnelSender,
    mut pending: RelayPendingResponse,
    exchange_id: String,
) {
    match tokio::time::timeout(RESPONSE_OPEN_TIMEOUT, pending.open).await {
        Ok(Ok(Ok(crate::tunnel::RelayResponseOpen { status: None, .. }))) => {}
        Err(_) => {
            if let Err(error) = fail_active_exchange(
                &state,
                &identity,
                &exchange_id,
                "Relay WebSocket response-open timed out".to_owned(),
            )
            .await
            {
                eprintln!("Relay WebSocket timeout cleanup failed for {exchange_id}: {error}");
            }
            return;
        }
        Ok(Ok(Ok(_))) => {
            if let Err(error) = fail_active_exchange(
                &state,
                &identity,
                &exchange_id,
                "Relay WebSocket response-open contained HTTP status semantics".to_owned(),
            )
            .await
            {
                eprintln!("Relay WebSocket protocol cleanup failed for {exchange_id}: {error}");
            }
            return;
        }
        Ok(Ok(Err(_))) | Ok(Err(_)) => return,
    }
    let (mut sink, mut stream) = socket.split();
    let mut client_open = true;
    let mut request_ended = false;
    let bridge_end = loop {
        tokio::select! {
            incoming = stream.next(), if client_open => match incoming {
                Some(Ok(message)) => {
                    let terminal = matches!(message, Message::Close(_));
                    let (frame_kind, bytes) = encode_websocket_message(message);
                    if let Err(error) = sender.send(RelayDataOutFrame::RequestChunk {
                        exchange_id: exchange_id.clone(),
                        frame_kind: Some(frame_kind),
                        bytes,
                    }).await {
                        break RelayWebSocketResponse::ProtocolFailure(format!(
                            "Relay WebSocket request forwarding failed: {error}"
                        ));
                    }
                    if terminal {
                        if let Err(error) = sender.send(RelayDataOutFrame::RequestEnd {
                            exchange_id: exchange_id.clone(),
                        }).await {
                            break RelayWebSocketResponse::ProtocolFailure(format!(
                                "Relay WebSocket request-end failed: {error}"
                            ));
                        }
                        request_ended = true;
                        client_open = false;
                    }
                }
                Some(Err(_)) | None => {
                    if let Err(error) = sender.send(RelayDataOutFrame::RequestEnd {
                        exchange_id: exchange_id.clone(),
                    }).await {
                        break RelayWebSocketResponse::ProtocolFailure(format!(
                            "Relay WebSocket request-end failed: {error}"
                        ));
                    }
                    request_ended = true;
                    client_open = false;
                    continue;
                }
            },
            outgoing = pending.parts.recv() => match classify_websocket_response(outgoing) {
                RelayWebSocketResponse::Frame(kind, bytes) => {
                    let message = match decode_websocket_message(kind, bytes) {
                        Ok(message) => message,
                        Err(error) => break RelayWebSocketResponse::ProtocolFailure(error),
                    };
                    if client_open && sink.send(message).await.is_err() {
                        if let Err(error) = sender.send(RelayDataOutFrame::RequestEnd {
                            exchange_id: exchange_id.clone(),
                        }).await {
                            break RelayWebSocketResponse::ProtocolFailure(format!(
                                "Relay WebSocket request-end failed: {error}"
                            ));
                        }
                        request_ended = true;
                        client_open = false;
                    }
                }
                RelayWebSocketResponse::SuccessEnd => break RelayWebSocketResponse::SuccessEnd,
                RelayWebSocketResponse::ProtocolFailure(error) => {
                    break RelayWebSocketResponse::ProtocolFailure(error);
                }
                RelayWebSocketResponse::ErrorChainFailure(error) => {
                    break RelayWebSocketResponse::ErrorChainFailure(error);
                }
            }
        }
    };
    match bridge_end {
        RelayWebSocketResponse::ProtocolFailure(reason) => {
            if let Err(error) = fail_active_exchange(&state, &identity, &exchange_id, reason).await
            {
                eprintln!("Relay WebSocket client cleanup failed for {exchange_id}: {error}");
            }
        }
        RelayWebSocketResponse::ErrorChainFailure(error) => {
            eprintln!("Relay WebSocket error chain terminated {exchange_id}: {error}");
        }
        RelayWebSocketResponse::SuccessEnd | RelayWebSocketResponse::Frame(_, _) => {}
    }
    if !request_ended
        && let Err(error) = sender
            .send(RelayDataOutFrame::RequestEnd {
                exchange_id: exchange_id.clone(),
            })
            .await
    {
        eprintln!("Relay WebSocket request-end failed for {exchange_id}: {error}");
    }
}

fn encode_websocket_message(message: Message) -> (RelayDataFrameKind, Vec<u8>) {
    match message {
        Message::Text(value) => (RelayDataFrameKind::Text, value.as_str().as_bytes().to_vec()),
        Message::Binary(value) => (RelayDataFrameKind::Binary, value.to_vec()),
        Message::Ping(value) => (RelayDataFrameKind::Ping, value.to_vec()),
        Message::Pong(value) => (RelayDataFrameKind::Pong, value.to_vec()),
        Message::Close(frame) => (
            RelayDataFrameKind::Close,
            frame.map_or_else(Vec::new, |frame| {
                let mut bytes = frame.code.to_be_bytes().to_vec();
                bytes.extend_from_slice(frame.reason.as_bytes());
                bytes
            }),
        ),
    }
}

fn decode_websocket_message(kind: RelayDataFrameKind, bytes: Vec<u8>) -> Result<Message, String> {
    Ok(match kind {
        RelayDataFrameKind::Text => Message::Text(
            String::from_utf8(bytes)
                .map_err(|_| "Relay received invalid UTF-8 text-frame bytes".to_owned())?
                .into(),
        ),
        RelayDataFrameKind::Binary => Message::Binary(bytes.into()),
        RelayDataFrameKind::Ping => Message::Ping(bytes.into()),
        RelayDataFrameKind::Pong => Message::Pong(bytes.into()),
        RelayDataFrameKind::Close => {
            let frame = match bytes.len() {
                0 => None,
                1 => return Err("Relay received an invalid close-frame payload".to_owned()),
                _ => Some(CloseFrame {
                    code: u16::from_be_bytes([bytes[0], bytes[1]]),
                    reason: String::from_utf8(bytes[2..].to_vec())
                        .map_err(|_| {
                            "Relay received invalid UTF-8 close-frame reason bytes".to_owned()
                        })?
                        .into(),
                }),
            };
            Message::Close(frame)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_target_and_close_frame_preserve_transport_semantics() {
        let uri: axum::http::Uri = "/relay/agents/studio/connect/files/a%3Fb?mode=opaque"
            .parse()
            .expect("URI");
        assert_eq!(
            raw_agent_path_and_query("studio", &uri).expect("raw target"),
            "/files/a%3Fb?mode=opaque"
        );

        let encoded_uri: axum::http::Uri =
            "/relay/agents/studio%20one/connect/files/a%3Fb?mode=opaque"
                .parse()
                .expect("encoded URI");
        assert_eq!(
            raw_agent_path_and_query("studio one", &encoded_uri).expect("encoded Agent route"),
            "/files/a%3Fb?mode=opaque"
        );
        assert!(raw_agent_path_and_query("different-agent", &encoded_uri).is_err());
        let wrong_namespace: axum::http::Uri = "/relay/agents/studio%20one/connected/files"
            .parse()
            .expect("wrong namespace URI");
        assert!(raw_agent_path_and_query("studio one", &wrong_namespace).is_err());

        let original = Message::Close(Some(CloseFrame {
            code: 4001,
            reason: "policy".into(),
        }));
        let (kind, bytes) = encode_websocket_message(original.clone());
        assert_eq!(
            decode_websocket_message(kind, bytes).expect("valid close frame"),
            original
        );
    }

    #[test]
    fn websocket_frame_decode_preserves_valid_bytes_and_rejects_malformed_text() {
        let text = decode_websocket_message(RelayDataFrameKind::Text, b"relay text".to_vec())
            .expect("valid text frame");
        assert_eq!(text, Message::Text("relay text".into()));
        assert_eq!(
            decode_websocket_message(RelayDataFrameKind::Text, vec![0xff]).unwrap_err(),
            "Relay received invalid UTF-8 text-frame bytes"
        );
        assert_eq!(
            decode_websocket_message(RelayDataFrameKind::Close, vec![0x03]).unwrap_err(),
            "Relay received an invalid close-frame payload"
        );
        assert_eq!(
            decode_websocket_message(RelayDataFrameKind::Close, vec![0x03, 0xe8, 0xff])
                .unwrap_err(),
            "Relay received invalid UTF-8 close-frame reason bytes"
        );
    }

    #[test]
    fn websocket_response_error_and_incomplete_channel_never_classify_as_success() {
        assert!(matches!(
            classify_websocket_response(Some(Err("bridge failed".to_owned()))),
            RelayWebSocketResponse::ErrorChainFailure(error) if error == "bridge failed"
        ));
        assert!(matches!(
            classify_websocket_response(None),
            RelayWebSocketResponse::ErrorChainFailure(error)
                if error == "Relay WebSocket response channel closed before response-end"
        ));
        assert_eq!(
            classify_websocket_response(Some(Ok(RelayResponsePart::End))),
            RelayWebSocketResponse::SuccessEnd
        );
    }
}
