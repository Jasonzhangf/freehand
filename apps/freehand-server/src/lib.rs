mod assets;
mod page;

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use freehand_contracts::{
    AgentId, FeatureId, ReasonResp01SemanticEvent, ReasonResp03TerminalEvent, SemanticEventKind,
    SessionId, TerminalStatus, TraceId, TurnId,
};
use freehand_ui_protocol::{
    DebugScenePosition, DebugSemanticPosition, DebugStateSnapshot, SubscriptionSelector,
    TurnProjectionInput, UiAdpFailure, UiAdpRequest, UiAdpResponse, UiCheckpointSnapshot,
    UiClientKind, UiCommand, UiCommandDispatchFailure, UiCommandDispatchPort,
    UiCommandDispatchReceipt, UiProjection, UiProtocolState, UiPublicTurnProjection, UiQueryResult,
    UiRuntimeQueryPort, UiSubscriptionEvent, UiTurnProjection, build_command_dispatch_envelope,
    checkpoint_projection_from_runtime_summary, dispatch_port_failure, protocol_rejection,
    public_turn_projection, subscription_matches, subscription_selector,
    turn_projection_for_client, turn_projection_from_events,
};
use futures_util::stream;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

#[derive(Clone)]
struct WebUiState {
    protocol_state: Arc<Mutex<UiProtocolState>>,
    command_dispatch_port: Arc<dyn UiCommandDispatchPort>,
    runtime_query_port: Arc<dyn UiRuntimeQueryPort>,
}

pub fn usage(binary_name: &str) -> String {
    format!("usage: {binary_name} webui-smoke | webui-serve-smoke [--bind HOST:PORT]")
}

pub fn parse_bind_arg(mut args: impl Iterator<Item = String>) -> Result<SocketAddr, String> {
    let mut bind_addr: SocketAddr = "127.0.0.1:3400"
        .parse()
        .expect("default bind address must be valid");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value after --bind".to_owned())?;
                bind_addr = value
                    .parse()
                    .map_err(|_| format!("invalid bind address `{value}`"))?;
            }
            _ => return Err(usage("freehand-server")),
        }
    }
    Ok(bind_addr)
}

pub fn build_webui_router(
    protocol_state: Arc<Mutex<UiProtocolState>>,
    command_dispatch_port: Arc<dyn UiCommandDispatchPort>,
    runtime_query_port: Arc<dyn UiRuntimeQueryPort>,
) -> Router {
    Router::new()
        .route("/", get(handle_root))
        .route("/mock/android", get(handle_android_mock))
        .route("/assets/{*path}", get(handle_asset))
        .route("/health", get(handle_health))
        .route("/ui/command", post(handle_command_ingress))
        .route(
            "/ui/query/latest-active-turn",
            get(handle_query_latest_active_turn),
        )
        .route("/ui/query/checkpoints", get(handle_query_checkpoints))
        .route("/ui/query/debug/{turn_id}", get(handle_query_debug_state))
        .route(
            "/ui/subscribe/turn/latest",
            get(handle_subscribe_latest_turn),
        )
        .route(
            "/ui/subscribe/debug/{turn_id}",
            get(handle_subscribe_debug_state),
        )
        .route("/adp", get(handle_adp_socket))
        .with_state(WebUiState {
            protocol_state,
            command_dispatch_port,
            runtime_query_port,
        })
}

pub async fn serve_webui_listener<F>(
    listener: TcpListener,
    protocol_state: Arc<Mutex<UiProtocolState>>,
    command_dispatch_port: Arc<dyn UiCommandDispatchPort>,
    runtime_query_port: Arc<dyn UiRuntimeQueryPort>,
    shutdown: F,
) -> std::io::Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    axum::serve(
        listener,
        build_webui_router(protocol_state, command_dispatch_port, runtime_query_port),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

pub fn render_webui_smoke() -> String {
    page::render_webui_smoke()
}

pub fn render_webui_smoke_for_client(client: Option<&str>) -> String {
    page::render_webui_smoke_for_client(client)
}

pub fn seed_webui_protocol_state() -> UiProtocolState {
    let mut state = UiProtocolState::default();
    let projection = sample_slave_turn_projection();
    state.apply_turn_projection(projection);
    state.set_debug_state(sample_debug_snapshot());
    state.set_checkpoint_snapshot(sample_checkpoint_snapshot());
    state
}

async fn handle_root(Query(params): Query<HashMap<String, String>>) -> Html<String> {
    Html(render_webui_smoke_for_client(
        params.get("client").map(String::as_str),
    ))
}

async fn handle_android_mock() -> Html<String> {
    Html(include_str!("../assets/mocks/android/mobile-mock.html").to_owned())
}

async fn handle_asset(Path(path): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    assets::asset_response(&path)
}

async fn handle_health() -> &'static str {
    "ok"
}

async fn handle_command_ingress(
    State(state): State<WebUiState>,
    Json(command): Json<UiCommand>,
) -> Result<
    (StatusCode, Json<UiCommandDispatchReceipt>),
    (StatusCode, Json<UiCommandDispatchFailure>),
> {
    let envelope = build_command_dispatch_envelope(&command).map_err(|err| {
        let rejection = protocol_rejection(err);
        (
            StatusCode::BAD_REQUEST,
            Json(UiCommandDispatchFailure {
                code: rejection.code,
                message: rejection.message,
                retryable: false,
            }),
        )
    })?;
    let dispatch_port = Arc::clone(&state.command_dispatch_port);
    let receipt = tokio::task::spawn_blocking(move || dispatch_port.dispatch(envelope))
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UiCommandDispatchFailure {
                    code: "dispatch_join_failed".to_owned(),
                    message: format!("command dispatch task failed: {err}"),
                    retryable: false,
                }),
            )
        })?;
    match receipt {
        Ok(receipt) => Ok((StatusCode::ACCEPTED, Json(receipt))),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(dispatch_port_failure(err)),
        )),
    }
}

async fn handle_query_latest_active_turn(
    State(state): State<WebUiState>,
) -> Result<Json<UiPublicTurnProjection>, StatusCode> {
    let turn = latest_webui_turn_projection(&state.protocol_state).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(public_turn_projection(turn)))
}

async fn handle_query_debug_state(
    Path(turn_id): Path<String>,
    State(state): State<WebUiState>,
) -> Result<Json<DebugStateSnapshot>, StatusCode> {
    let snapshot = match state
        .protocol_state
        .lock()
        .expect("lock protocol state")
        .query(&UiCommand::QueryDebugState {
            turn_id: TurnId::new(turn_id),
        })
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        UiQueryResult::Debug(Some(snapshot)) => snapshot,
        UiQueryResult::Debug(None) => return Err(StatusCode::NOT_FOUND),
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    Ok(Json(snapshot))
}

async fn handle_query_checkpoints(
    State(state): State<WebUiState>,
) -> Result<Json<UiCheckpointSnapshot>, StatusCode> {
    let snapshot = match state
        .protocol_state
        .lock()
        .expect("lock protocol state")
        .query(&UiCommand::QueryCheckpoints)
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        UiQueryResult::Checkpoints(snapshot) => snapshot,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    Ok(Json(snapshot))
}

async fn handle_subscribe_latest_turn(
    State(state): State<WebUiState>,
) -> Result<impl IntoResponse, StatusCode> {
    let command = UiCommand::SubscribeLatestActiveTurn {
        client: UiClientKind::WebUi,
    };
    let selector = subscription_selector(&command).ok_or(StatusCode::BAD_REQUEST)?;
    let (initial_projection, receiver) = {
        let state = state.protocol_state.lock().expect("lock protocol state");
        let turn = match state
            .query(&UiCommand::QueryLatestActiveTurn)
            .map_err(|_| StatusCode::BAD_REQUEST)?
        {
            UiQueryResult::Turn(Some(turn)) => Some(UiProjection::Turn(
                turn_projection_for_client(turn, UiClientKind::WebUi),
            )),
            UiQueryResult::Turn(None) => None,
            _ => return Err(StatusCode::BAD_REQUEST),
        };
        (turn, state.subscribe())
    };
    Ok(Sse::new(subscription_event_stream(
        initial_projection,
        receiver,
        selector,
    )))
}

async fn handle_subscribe_debug_state(
    Path(turn_id): Path<String>,
    State(state): State<WebUiState>,
) -> Result<impl IntoResponse, StatusCode> {
    let command = UiCommand::SubscribeDebugState {
        client: UiClientKind::WebUi,
        turn_id: TurnId::new(turn_id),
    };
    let selector = subscription_selector(&command).ok_or(StatusCode::BAD_REQUEST)?;
    let (initial_projection, receiver) = {
        let state = state.protocol_state.lock().expect("lock protocol state");
        let snapshot = match state
            .query(&UiCommand::QueryDebugState {
                turn_id: selector
                    .target_turn_id
                    .clone()
                    .expect("debug selector requires turn_id"),
            })
            .map_err(|_| StatusCode::BAD_REQUEST)?
        {
            UiQueryResult::Debug(Some(snapshot)) => Some(UiProjection::Debug(snapshot)),
            UiQueryResult::Debug(None) => None,
            _ => return Err(StatusCode::BAD_REQUEST),
        };
        (snapshot, state.subscribe())
    };
    Ok(Sse::new(subscription_event_stream(
        initial_projection,
        receiver,
        selector,
    )))
}

async fn handle_adp_socket(
    ws: WebSocketUpgrade,
    State(state): State<WebUiState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_adp_connection(socket, state))
}

async fn handle_adp_connection(socket: WebSocket, state: WebUiState) {
    let (mut sender, mut receiver) = socket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<UiAdpResponse>();
    let writer = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            let Ok(body) = serde_json::to_string(&message) else {
                continue;
            };
            if sender.send(Message::Text(body.into())).await.is_err() {
                break;
            }
        }
    });

    let mut subscriptions: Vec<(String, SubscriptionSelector)> = Vec::new();
    let mut protocol_receiver = state
        .protocol_state
        .lock()
        .expect("lock protocol state")
        .subscribe();
    loop {
        tokio::select! {
            message = receiver.next() => {
                let Some(message) = message else {
                    break;
                };
                let Ok(message) = message else {
                    break;
                };
                match message {
                    Message::Text(text) => {
                        if let Err(err) = handle_adp_text_message(
                            &state,
                            &outbound_tx,
                            &mut subscriptions,
                            text.to_string(),
                        )
                        .await
                        {
                            let _ = outbound_tx.send(UiAdpResponse::Failure {
                                request_id: "transport".to_owned(),
                                failure: UiAdpFailure {
                                    code: "invalid_adp_message".to_owned(),
                                    message: err,
                                    retryable: false,
                                },
                            });
                        }
                    }
                    Message::Close(_) => break,
                    Message::Ping(_) | Message::Pong(_) => {}
                    Message::Binary(_) => {
                        let _ = outbound_tx.send(UiAdpResponse::Failure {
                            request_id: "transport".to_owned(),
                            failure: UiAdpFailure {
                                code: "binary_frame_unsupported".to_owned(),
                                message: "binary frames are not supported by the ADP transport".to_owned(),
                                retryable: false,
                            },
                        });
                    }
                }
            }
            update = protocol_receiver.recv() => {
                match update {
                    Ok(update) => {
                        for (request_id, selector) in &subscriptions {
                            if subscription_matches(
                                selector,
                                &update.projection,
                                update.latest_active_turn_id.as_ref(),
                            ) {
                                let _ = outbound_tx.send(UiAdpResponse::SubscriptionEvent {
                                    request_id: request_id.clone(),
                                    event: update.clone(),
                                });
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    drop(outbound_tx);
    let _ = writer.await;
}

async fn handle_adp_text_message(
    state: &WebUiState,
    outbound_tx: &mpsc::UnboundedSender<UiAdpResponse>,
    subscriptions: &mut Vec<(String, SubscriptionSelector)>,
    text: String,
) -> Result<(), String> {
    let request: UiAdpRequest =
        serde_json::from_str(&text).map_err(|err| format!("invalid ADP JSON: {err}"))?;
    match request {
        UiAdpRequest::Command {
            request_id,
            command,
        } => {
            handle_adp_command(state, outbound_tx, request_id, command).await;
            Ok(())
        }
        UiAdpRequest::Query { request_id, query } => {
            let _ = handle_adp_query(state, outbound_tx, request_id, query).await;
            Ok(())
        }
        UiAdpRequest::Subscribe {
            request_id,
            subscription,
        } => {
            let _ =
                handle_adp_subscribe(state, outbound_tx, subscriptions, request_id, subscription)
                    .await;
            Ok(())
        }
    }
}

async fn handle_adp_command(
    state: &WebUiState,
    outbound_tx: &mpsc::UnboundedSender<UiAdpResponse>,
    request_id: String,
    command: UiCommand,
) {
    let envelope = match build_command_dispatch_envelope(&command) {
        Ok(envelope) => envelope,
        Err(err) => {
            let rejection = protocol_rejection(err);
            let _ = outbound_tx.send(UiAdpResponse::Failure {
                request_id,
                failure: UiAdpFailure {
                    code: rejection.code,
                    message: rejection.message,
                    retryable: false,
                },
            });
            return;
        }
    };
    let dispatch_port = Arc::clone(&state.command_dispatch_port);
    let tx = outbound_tx.clone();
    tokio::spawn(async move {
        let receipt =
            match tokio::task::spawn_blocking(move || dispatch_port.dispatch(envelope)).await {
                Ok(receipt) => receipt,
                Err(err) => {
                    let _ = tx.send(UiAdpResponse::Failure {
                        request_id,
                        failure: UiAdpFailure {
                            code: "dispatch_join_failed".to_owned(),
                            message: format!("command dispatch task failed: {err}"),
                            retryable: false,
                        },
                    });
                    return;
                }
            };
        match receipt {
            Ok(receipt) => {
                let _ = tx.send(UiAdpResponse::CommandReceipt {
                    request_id,
                    receipt,
                });
            }
            Err(err) => {
                let failure = dispatch_port_failure(err);
                let _ = tx.send(UiAdpResponse::Failure {
                    request_id,
                    failure: UiAdpFailure {
                        code: failure.code,
                        message: failure.message,
                        retryable: failure.retryable,
                    },
                });
            }
        }
    });
}

async fn handle_adp_query(
    state: &WebUiState,
    outbound_tx: &mpsc::UnboundedSender<UiAdpResponse>,
    request_id: String,
    query: UiCommand,
) -> Result<(), String> {
    let runtime_query_port = Arc::clone(&state.runtime_query_port);
    let query_for_runtime = query.clone();
    let runtime_result =
        tokio::task::spawn_blocking(move || runtime_query_port.query_runtime(&query_for_runtime))
            .await
            .map_err(|err| format!("runtime query task failed: {err}"))?;
    match runtime_result {
        Ok(Some(result)) => {
            let _ = outbound_tx.send(UiAdpResponse::QueryResult { request_id, result });
            return Ok(());
        }
        Ok(None) => {}
        Err(err) => {
            let failure = dispatch_port_failure(err);
            let _ = outbound_tx.send(UiAdpResponse::Failure {
                request_id,
                failure: UiAdpFailure {
                    code: failure.code,
                    message: failure.message,
                    retryable: failure.retryable,
                },
            });
            return Ok(());
        }
    };
    let result = {
        let state = state.protocol_state.lock().expect("lock protocol state");
        state.query(&query)
    };
    match result {
        Ok(result) => {
            let _ = outbound_tx.send(UiAdpResponse::QueryResult { request_id, result });
        }
        Err(err) => {
            let rejection = protocol_rejection(err);
            let _ = outbound_tx.send(UiAdpResponse::Failure {
                request_id,
                failure: UiAdpFailure {
                    code: rejection.code,
                    message: rejection.message,
                    retryable: false,
                },
            });
        }
    }
    Ok(())
}

async fn handle_adp_subscribe(
    state: &WebUiState,
    outbound_tx: &mpsc::UnboundedSender<UiAdpResponse>,
    subscriptions: &mut Vec<(String, SubscriptionSelector)>,
    request_id: String,
    subscription: UiCommand,
) -> Result<(), String> {
    let selector = match subscription_selector(&subscription) {
        Some(selector) => selector,
        None => {
            let _ = outbound_tx.send(UiAdpResponse::Failure {
                request_id,
                failure: UiAdpFailure {
                    code: "subscription_kind_mismatch".to_owned(),
                    message: "subscription frame rejected by protocol boundary".to_owned(),
                    retryable: false,
                },
            });
            return Ok(());
        }
    };
    let initial_projection = {
        let runtime_query_port = Arc::clone(&state.runtime_query_port);
        let protocol_state = state.protocol_state.lock().expect("lock protocol state");
        match initial_adp_subscription_projection(
            &protocol_state,
            &subscription,
            &runtime_query_port,
        ) {
            Ok(initial) => initial,
            Err(status) => {
                let _ = outbound_tx.send(UiAdpResponse::Failure {
                    request_id,
                    failure: UiAdpFailure {
                        code: "subscription_initial_query_failed".to_owned(),
                        message: format!("subscription initial query failed with {status}"),
                        retryable: false,
                    },
                });
                return Ok(());
            }
        }
    };
    let _ = outbound_tx.send(UiAdpResponse::SubscriptionAccepted {
        request_id: request_id.clone(),
        selector: selector.clone(),
    });
    subscriptions.push((request_id.clone(), selector));
    if let Some(projection) = initial_projection {
        let event = UiSubscriptionEvent {
            latest_active_turn_id: projection_latest_active_turn_id(&projection),
            projection,
        };
        let _ = outbound_tx.send(UiAdpResponse::SubscriptionEvent {
            request_id: request_id.clone(),
            event,
        });
    }
    Ok(())
}

fn initial_adp_subscription_projection(
    state: &UiProtocolState,
    subscription: &UiCommand,
    runtime_query_port: &Arc<dyn UiRuntimeQueryPort>,
) -> Result<Option<UiProjection>, StatusCode> {
    match subscription {
        UiCommand::SubscribeLatestActiveTurn { client } => match state
            .query(&UiCommand::QueryLatestActiveTurn)
            .map_err(|_| StatusCode::BAD_REQUEST)?
        {
            UiQueryResult::Turn(Some(turn)) => Ok(Some(UiProjection::Turn(
                turn_projection_for_client(turn, *client),
            ))),
            UiQueryResult::Turn(None) => Ok(None),
            _ => Err(StatusCode::BAD_REQUEST),
        },
        UiCommand::SubscribeTurn { client, turn_id } => match state
            .query(&UiCommand::QueryTurn {
                turn_id: turn_id.clone(),
            })
            .map_err(|_| StatusCode::BAD_REQUEST)?
        {
            UiQueryResult::Turn(Some(turn)) => Ok(Some(UiProjection::Turn(
                turn_projection_for_client(turn, *client),
            ))),
            UiQueryResult::Turn(None) => Ok(None),
            _ => Err(StatusCode::BAD_REQUEST),
        },
        UiCommand::SubscribeDebugState { turn_id, .. } => match state
            .query(&UiCommand::QueryDebugState {
                turn_id: turn_id.clone(),
            })
            .map_err(|_| StatusCode::BAD_REQUEST)?
        {
            UiQueryResult::Debug(Some(snapshot)) => Ok(Some(UiProjection::Debug(snapshot))),
            UiQueryResult::Debug(None) => Ok(None),
            _ => Err(StatusCode::BAD_REQUEST),
        },
        UiCommand::SubscribeNodeStatus | UiCommand::SubscribeProgress => Ok(None),
        UiCommand::SubscribeTaskList { status, agent_id } => {
            match runtime_query_port.query_runtime(&UiCommand::QueryTaskList {
                status: status.clone(),
                agent_id: agent_id.clone(),
            }) {
                Ok(Some(UiQueryResult::TaskList(list))) => Ok(Some(UiProjection::TaskList(list))),
                Ok(Some(_)) | Ok(None) => Err(StatusCode::BAD_REQUEST),
                Err(_) => Err(StatusCode::BAD_REQUEST),
            }
        }
        UiCommand::SubscribeErrorCenterEvents {
            session_id,
            trace_id,
            turn_id,
            domain,
        } => match runtime_query_port.query_runtime(&UiCommand::QueryErrorCenterEvents {
            session_id: session_id.clone(),
            trace_id: trace_id.clone(),
            turn_id: turn_id.clone(),
            domain: domain.clone(),
        }) {
            Ok(Some(UiQueryResult::ErrorCenterEvents(events))) => {
                Ok(Some(UiProjection::ErrorCenterEvents(events)))
            }
            Ok(Some(_)) | Ok(None) => Err(StatusCode::BAD_REQUEST),
            Err(_) => Err(StatusCode::BAD_REQUEST),
        },
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

fn projection_latest_active_turn_id(projection: &UiProjection) -> Option<TurnId> {
    match projection {
        UiProjection::Turn(turn) => Some(turn.turn_id.clone()),
        UiProjection::Debug(snapshot) => Some(snapshot.semantic.turn_id.clone()),
        UiProjection::Progress(snapshot) => Some(snapshot.turn_id.clone()),
        UiProjection::NodeStatus(_) | UiProjection::Checkpoints(_) => None,
        UiProjection::TaskList(_) => None,
        UiProjection::ErrorCenterEvents(_) => None,
    }
}

fn subscription_event_stream(
    initial_projection: Option<UiProjection>,
    receiver: broadcast::Receiver<UiSubscriptionEvent>,
    selector: SubscriptionSelector,
) -> impl futures_util::Stream<Item = Result<Event, Infallible>> {
    stream::unfold(
        (initial_projection, receiver, selector),
        |(pending, mut receiver, selector)| async move {
            if let Some(projection) = pending {
                let event = projection_to_sse_event(projection, selector.client);
                return Some((Ok(event), (None, receiver, selector)));
            }
            loop {
                match receiver.recv().await {
                    Ok(update) => {
                        if !subscription_matches(
                            &selector,
                            &update.projection,
                            update.latest_active_turn_id.as_ref(),
                        ) {
                            continue;
                        }
                        let event = projection_to_sse_event(update.projection, selector.client);
                        return Some((Ok(event), (None, receiver, selector)));
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    )
}

fn projection_to_sse_event(projection: UiProjection, client: UiClientKind) -> Event {
    match projection {
        UiProjection::Turn(turn) => Event::default().event("turn").data(
            serde_json::to_string(&public_turn_projection(turn_projection_for_client(
                turn, client,
            )))
            .expect("turn json"),
        ),
        UiProjection::Debug(snapshot) => Event::default()
            .event("debug")
            .data(serde_json::to_string(&snapshot).expect("debug json")),
        UiProjection::Checkpoints(snapshot) => Event::default()
            .event("checkpoints")
            .data(serde_json::to_string(&snapshot).expect("checkpoint json")),
        UiProjection::NodeStatus(snapshot) => Event::default()
            .event("node_status")
            .data(serde_json::to_string(&snapshot).expect("node status json")),
        UiProjection::Progress(snapshot) => Event::default()
            .event("progress")
            .data(serde_json::to_string(&snapshot).expect("progress json")),
        UiProjection::TaskList(snapshot) => Event::default()
            .event("task_list")
            .data(serde_json::to_string(&snapshot).expect("task list json")),
        UiProjection::ErrorCenterEvents(snapshot) => Event::default()
            .event("error_center_events")
            .data(serde_json::to_string(&snapshot).expect("error center json")),
    }
}

fn latest_webui_turn_projection(state: &Arc<Mutex<UiProtocolState>>) -> Option<UiTurnProjection> {
    match state
        .lock()
        .expect("lock protocol state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .ok()?
    {
        UiQueryResult::Turn(Some(turn)) => {
            Some(turn_projection_for_client(turn, UiClientKind::WebUi))
        }
        _ => None,
    }
}

fn sample_slave_turn_projection() -> UiTurnProjection {
    turn_projection_from_events(TurnProjectionInput {
        source_agent_id: AgentId::new("slave-agent"),
        source_node_id: "slave-node".to_owned(),
        session_id: SessionId::new("session-webui-smoke"),
        turn_id: TurnId::new("turn-webui-smoke"),
        cwd: None,
        user_text: Some("inspect slave status".to_owned()),
        semantic_events: vec![
            ReasonResp01SemanticEvent {
                session_id: SessionId::new("session-webui-smoke"),
                turn_id: TurnId::new("turn-webui-smoke"),
                trace_id: TraceId::new("trace-webui-smoke"),
                feature_id: FeatureId::new("app.webui-smoke"),
                agent_id: AgentId::new("slave-agent"),
                kind: SemanticEventKind::Reasoning,
                content: "thinking".to_owned(),
            },
            ReasonResp01SemanticEvent {
                session_id: SessionId::new("session-webui-smoke"),
                turn_id: TurnId::new("turn-webui-smoke"),
                trace_id: TraceId::new("trace-webui-smoke"),
                feature_id: FeatureId::new("app.webui-smoke"),
                agent_id: AgentId::new("slave-agent"),
                kind: SemanticEventKind::Text,
                content: "slave answer".to_owned(),
            },
        ],
        tool_calls: Vec::new(),
        tool_results: Vec::new(),
        usage_events: Vec::new(),
        terminal_event: Some(ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-webui-smoke"),
            turn_id: TurnId::new("turn-webui-smoke"),
            trace_id: TraceId::new("trace-webui-smoke"),
            feature_id: FeatureId::new("app.webui-smoke"),
            agent_id: AgentId::new("slave-agent"),
            status: TerminalStatus::Success,
            summary: "terminal final text".to_owned(),
        }),
        error_events: Vec::new(),
        slave_substream_card: true,
    })
}

fn sample_debug_snapshot() -> DebugStateSnapshot {
    DebugStateSnapshot::new(
        DebugSemanticPosition {
            feature_id: FeatureId::new("app.webui-smoke"),
            session_id: SessionId::new("session-webui-smoke"),
            turn_id: TurnId::new("turn-webui-smoke"),
            trace_id: TraceId::new("trace-webui-smoke"),
            agent_id: Some(AgentId::new("slave-agent")),
            pipeline_node: Some("UiDebugState".to_owned()),
        },
        DebugScenePosition {
            crate_name: "freehand-server".to_owned(),
            file: "src/lib.rs".to_owned(),
            function: "sample_debug_snapshot".to_owned(),
            line: None,
            artifact_path: None,
            raw_exchange_id: None,
        },
        "debug state from protocol query",
        vec![
            "feature=app.webui-smoke".to_owned(),
            "consumer=webui".to_owned(),
        ],
    )
}

fn sample_checkpoint_snapshot() -> UiCheckpointSnapshot {
    checkpoint_projection_from_runtime_summary(
        AgentId::new("slave-agent"),
        "slave-node".to_owned(),
        vec![freehand_ui_protocol::UiCheckpointSummary {
            checkpoint_id: "checkpoint-webui-smoke".to_owned(),
            agent_id: AgentId::new("slave-agent"),
            session_id: SessionId::new("session-webui-smoke"),
            turn_id: TurnId::new("turn-webui-smoke"),
            tool_call_id: "tool-webui-smoke".to_owned(),
            changed_paths: vec!["scratch/webui.txt".to_owned()],
            latest_status: "applied".to_owned(),
            latest_detail: None,
            updated_unix_seconds: 42,
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        ReasonReq04ToolCall, ReasonReq05ToolResultReentry, ToolCallContract, ToolCallId,
        ToolResultContract,
    };
    use freehand_ui_protocol::{
        StaticUiCommandDispatchPort, UiCommand, UiCommandDispatchEnvelope,
        UiCommandDispatchPortError,
    };
    use reqwest::Client;
    use std::time::Duration;
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    struct TestServer {
        base_url: String,
        protocol_state: Arc<Mutex<UiProtocolState>>,
        shutdown: Option<oneshot::Sender<()>>,
        task: tokio::task::JoinHandle<()>,
    }

    impl TestServer {
        async fn spawn() -> Self {
            Self::spawn_with_state(seed_webui_protocol_state()).await
        }

        async fn spawn_empty() -> Self {
            Self::spawn_with_state(UiProtocolState::default()).await
        }

        async fn spawn_with_state(initial_state: UiProtocolState) -> Self {
            Self::spawn_with_state_and_port(
                initial_state,
                Arc::new(StaticUiCommandDispatchPort::default()),
            )
            .await
        }

        async fn spawn_with_state_and_port(
            initial_state: UiProtocolState,
            command_dispatch_port: Arc<dyn UiCommandDispatchPort>,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("local addr");
            let protocol_state = Arc::new(Mutex::new(initial_state));
            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let protocol_state_for_task = Arc::clone(&protocol_state);
            let command_dispatch_port_for_task = Arc::clone(&command_dispatch_port);
            let task = tokio::spawn(async move {
                let shutdown = async move {
                    let _ = shutdown_rx.await;
                };
                serve_webui_listener(
                    listener,
                    protocol_state_for_task,
                    command_dispatch_port_for_task,
                    Arc::new(freehand_ui_protocol::UiProtocolOnlyQueryPort),
                    shutdown,
                )
                .await
                .expect("serve");
            });
            Self {
                base_url: format!("http://{addr}"),
                protocol_state,
                shutdown: Some(shutdown_tx),
                task,
            }
        }

        async fn stop(mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
            self.task.await.expect("join");
        }
    }

    struct FailingUiCommandDispatchPort;

    impl UiCommandDispatchPort for FailingUiCommandDispatchPort {
        fn dispatch(
            &self,
            _envelope: UiCommandDispatchEnvelope,
        ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
            Err(UiCommandDispatchPortError::DispatchFailed(
                "runtime queue unavailable".to_owned(),
            ))
        }
    }

    struct PanicUiCommandDispatchPort;

    impl UiCommandDispatchPort for PanicUiCommandDispatchPort {
        fn dispatch(
            &self,
            _envelope: UiCommandDispatchEnvelope,
        ) -> Result<UiCommandDispatchReceipt, UiCommandDispatchPortError> {
            panic!("dispatch worker panicked");
        }
    }

    async fn read_next_sse_event(response: &mut reqwest::Response, buffer: &mut String) -> String {
        loop {
            if let Some(end) = buffer.find("\n\n") {
                let event = buffer[..end].to_owned();
                let rest = buffer[end + 2..].to_owned();
                *buffer = rest;
                return event;
            }
            let chunk = timeout(Duration::from_secs(5), response.chunk())
                .await
                .expect("sse chunk timeout")
                .expect("sse response")
                .expect("sse stream closed");
            buffer.push_str(&String::from_utf8_lossy(&chunk));
        }
    }

    #[test]
    fn webui_smoke_renders_shell_and_asset_routes() {
        let html = render_webui_smoke();
        assert!(html.contains("data-webui-shell=\"true\""));
        assert!(!html.contains("data-layout-client=\"android-webview\""));
        assert!(!html.contains("data-layout-shape=\"tablet_portrait\""));
        assert!(html.contains("/assets/theme.css"));
        assert!(html.contains("/assets/webui.css"));
        assert!(html.contains("/assets/webui.js"));
        assert!(html.contains("data-adp-endpoint=\"/adp\""));
        assert!(html.contains("id=\"session-list\""));
        assert!(html.contains("id=\"new-conversation-button\""));
        assert!(html.contains("id=\"new-task-button\""));
        assert!(html.contains("id=\"task-cwd-input\""));
        assert!(html.contains("id=\"settings-shell-toggle\""));
        assert!(html.contains("id=\"open-settings-drawer-button\""));
        assert!(html.contains("id=\"settings-shell\""));
        assert!(html.contains("Provider editing locked"));
        assert!(html.contains("Skill settings pending"));
        assert!(!html.contains("type=\"password\""));
        assert!(!html.contains("api-key"));
        assert!(html.contains("id=\"new-session-dialog\""));
        assert!(html.contains("id=\"new-session-form\""));
        assert!(html.contains("id=\"new-task-path-presets\""));
        assert!(html.contains("data-cwd=\"/Volumes/extension/code/freehand\""));
        assert!(!html.contains("Archived sessions"));
        assert!(!html.contains("id=\"archived-session-list\""));
        assert!(!html.contains(">Archive</button>"));
        assert!(!html.contains("id=\"success-sample-button\""));
        assert!(!html.contains("id=\"failure-sample-button\""));
        assert!(html.contains("composer-control-strip"));
        assert!(html.contains("id=\"attach-file-button\""));
        assert!(html.contains("id=\"attach-image-button\""));
        assert!(html.contains("id=\"attach-video-button\""));
        assert!(html.contains("id=\"preview-attachments-button\""));
        assert!(html.contains("id=\"refresh-session-button\""));
        assert!(html.contains("id=\"cwd-input\""));
        assert!(html.contains("id=\"strip-cwd\""));
        assert!(html.contains("id=\"model-selector\""));
        assert!(html.contains("id=\"attachment-tray\""));
        assert!(html.contains("work-context-tags"));
        assert!(html.contains("id=\"worker-context-tag\""));
        assert!(html.contains("id=\"task-context-tag\""));
        assert!(html.contains("id=\"transport-context-tag\""));
        assert!(!html.contains("id=\"conversation-turn\""));
    }

    #[test]
    fn webui_android_client_shell_pins_mobile_initial_layout() {
        let html = render_webui_smoke_for_client(Some("android-webview"));
        assert!(html.contains(
            "<body class=\"theme-light\" data-layout-client=\"android-webview\" data-layout-shape=\"tablet_portrait\">"
        ));
        assert!(html.contains(
            "<main class=\"app-shell\" data-webui-shell=\"true\" data-layout-client=\"android-webview\" data-layout-shape=\"tablet_portrait\""
        ));
        assert!(html.contains("id=\"open-session-drawer-button\""));
        assert!(html.contains("id=\"open-detail-drawer-button\""));
        assert!(html.contains("id=\"open-settings-drawer-button\""));
    }

    #[tokio::test]
    async fn android_mock_route_returns_design_preview() {
        let server = TestServer::spawn().await;
        let client = Client::builder().build().expect("client");

        let page = client
            .get(format!("{}/mock/android", server.base_url))
            .send()
            .await
            .expect("mock page");
        assert_eq!(page.status(), StatusCode::OK);
        let body = page.text().await.expect("mock body");
        assert!(body.contains("mock-mobile"));
        assert!(body.contains("<style>"));
        assert!(body.contains(".mock-mobile"));
        assert!(!body.contains("/assets/mocks/android/mobile-mock.css"));
        assert!(body.contains("快速控制"));

        let css = client
            .get(format!(
                "{}/assets/mocks/android/mobile-mock.css",
                server.base_url
            ))
            .send()
            .await
            .expect("mock css");
        assert_eq!(css.status(), StatusCode::OK);
        let css_body = css.text().await.expect("mock css body");
        assert!(css_body.contains("mock-mobile"));
        assert!(css_body.contains(".drawer.open"));

        server.stop().await;
    }

    #[tokio::test]
    async fn transport_query_smoke_returns_turn_and_debug_protocol_truth() {
        let server = TestServer::spawn().await;
        let client = Client::builder().build().expect("client");

        let turn = client
            .get(format!("{}/ui/query/latest-active-turn", server.base_url))
            .send()
            .await
            .expect("turn response");
        assert_eq!(turn.status(), StatusCode::OK);
        let turn: UiPublicTurnProjection = turn.json().await.expect("turn json");
        assert_eq!(turn.turn.turn_id, TurnId::new("turn-webui-smoke"));
        assert_eq!(
            turn.turn.terminal_text.as_deref(),
            Some("terminal final text")
        );
        assert!(turn.turn.slave_substream_card);
        assert_eq!(turn.public_conversation[0].body, "inspect slave status");
        assert_eq!(turn.public_conversation[1].body, "slave answer");
        assert_eq!(turn.public_conversation[2].body, "terminal final text");

        let debug = client
            .get(format!(
                "{}/ui/query/debug/turn-webui-smoke",
                server.base_url
            ))
            .send()
            .await
            .expect("debug response");
        assert_eq!(debug.status(), StatusCode::OK);
        let debug: DebugStateSnapshot = debug.json().await.expect("debug json");
        assert_eq!(debug.status_text, "debug state from protocol query");
        assert_eq!(
            debug.detail_lines,
            vec!["feature=app.webui-smoke", "consumer=webui"]
        );

        let checkpoints = client
            .get(format!("{}/ui/query/checkpoints", server.base_url))
            .send()
            .await
            .expect("checkpoint response");
        assert_eq!(checkpoints.status(), StatusCode::OK);
        let checkpoints: UiCheckpointSnapshot = checkpoints.json().await.expect("checkpoint json");
        assert_eq!(checkpoints.checkpoints.len(), 1);
        assert_eq!(
            checkpoints.checkpoints[0].checkpoint_id,
            "checkpoint-webui-smoke"
        );
        assert_eq!(checkpoints.checkpoints[0].latest_status, "applied");

        server.stop().await;
    }

    #[tokio::test]
    async fn root_and_asset_routes_return_webui_shell_files() {
        let server = TestServer::spawn().await;
        let client = Client::builder().build().expect("client");

        let root = client
            .get(format!("{}/", server.base_url))
            .send()
            .await
            .expect("root response");
        assert_eq!(root.status(), StatusCode::OK);
        let root_body = root.text().await.expect("root body");
        assert!(root_body.contains("data-webui-shell=\"true\""));
        assert!(!root_body.contains("data-layout-client=\"android-webview\""));
        assert!(!root_body.contains("data-layout-shape=\"tablet_portrait\""));
        assert!(root_body.contains("/assets/theme.css"));
        assert!(root_body.contains("data-adp-endpoint=\"/adp\""));
        assert!(root_body.contains("id=\"session-list\""));
        assert!(root_body.contains("id=\"new-conversation-button\""));
        assert!(root_body.contains("id=\"new-task-button\""));
        assert!(root_body.contains("class=\"session-bulk-summary\""));
        assert!(root_body.contains("class=\"session-bulk-actions\""));
        assert!(root_body.contains("id=\"session-bulk-count\""));
        assert!(root_body.contains("id=\"session-select-all-button\""));
        assert!(root_body.contains("id=\"session-clear-selection-button\""));
        assert!(root_body.contains("id=\"session-delete-selected-button\""));
        assert!(root_body.contains("id=\"settings-shell-toggle\""));
        assert!(root_body.contains("id=\"open-settings-drawer-button\""));
        assert!(root_body.contains("id=\"settings-shell\""));
        assert!(root_body.contains("Provider editing locked"));
        assert!(root_body.contains("Task settings pending"));
        assert!(!root_body.contains("type=\"password\""));
        assert!(!root_body.contains("api-key"));
        assert!(root_body.contains("data-checkpoint-query=\"/ui/query/checkpoints\""));
        assert!(root_body.contains("id=\"debug-details-toggle\""));
        assert!(!root_body.contains(">Success</button>"));
        assert!(!root_body.contains(">Failure</button>"));
        assert!(!root_body.contains("Success sample"));
        assert!(!root_body.contains("Failure sample"));

        let android_root = client
            .get(format!("{}/?client=android-webview", server.base_url))
            .send()
            .await
            .expect("android root response");
        assert_eq!(android_root.status(), StatusCode::OK);
        let android_root_body = android_root.text().await.expect("android root body");
        assert!(android_root_body.contains(
            "<body class=\"theme-light\" data-layout-client=\"android-webview\" data-layout-shape=\"tablet_portrait\">"
        ));
        assert!(android_root_body.contains(
            "<main class=\"app-shell\" data-webui-shell=\"true\" data-layout-client=\"android-webview\" data-layout-shape=\"tablet_portrait\""
        ));

        let theme = client
            .get(format!("{}/assets/theme.css", server.base_url))
            .send()
            .await
            .expect("theme response");
        assert_eq!(theme.status(), StatusCode::OK);
        assert_eq!(
            theme.headers().get("content-type").unwrap(),
            "text/css; charset=utf-8"
        );
        assert!(
            theme
                .text()
                .await
                .expect("theme body")
                .contains("body.theme-dark")
        );
        let webui_css = client
            .get(format!("{}/assets/webui.css", server.base_url))
            .send()
            .await
            .expect("webui css response");
        assert_eq!(webui_css.status(), StatusCode::OK);
        let webui_css_body = webui_css.text().await.expect("webui css body");
        assert!(webui_css_body.contains("@keyframes waitingDot"));
        assert!(webui_css_body.contains("@keyframes toolPulse"));
        assert!(webui_css_body.contains(".chat-empty-title"));
        assert!(webui_css_body.contains(".chat-message-user"));
        assert!(webui_css_body.contains(".chat-message-assistant"));
        assert!(webui_css_body.contains(".chat-section-tool"));
        assert!(webui_css_body.contains(".chat-reasoning-body"));
        assert!(webui_css_body.contains(".tool-command-line"));
        assert!(webui_css_body.contains("width: fit-content"));
        assert!(webui_css_body.contains(".execution-block"));
        assert!(webui_css_body.contains(".execution-block.success-state"));
        assert!(webui_css_body.contains(".execution-block.failed-state"));
        assert!(webui_css_body.contains(".execution-block.running-state"));
        assert!(webui_css_body.contains("border-left-color: var(--success)"));
        assert!(webui_css_body.contains("border-left-color: var(--fail)"));
        assert!(webui_css_body.contains("border-left-color: #2f6fed"));
        assert!(!webui_css_body.contains("var(--ok)"));
        assert!(webui_css_body.contains(".execution-row-tool"));
        assert!(webui_css_body.contains(".debug-toggle"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_portrait\"]"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_landscape\"]"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"tablet_portrait\"]"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"tablet_landscape\"]"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"foldable_unfolded\"]"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"desktop_large\"]"));
        assert!(webui_css_body.contains("body[data-mobile-drawer=\"sessions\"] .sidebar"));
        assert!(webui_css_body.contains("body[data-mobile-drawer=\"details\"] .inspector"));
        assert!(webui_css_body.contains("body[data-mobile-drawer=\"settings\"] .inspector"));
        assert!(webui_css_body.contains(".mobile-drawer-scrim"));
        assert!(webui_css_body.contains(".settings-shell"));
        assert!(webui_css_body.contains(".settings-shell[hidden]"));
        assert!(webui_css_body.contains(".inspector-debug-panel[hidden]"));
        assert!(webui_css_body.contains(".settings-card"));
        assert!(webui_css_body.contains(".settings-readonly-action:disabled"));
        assert!(webui_css_body.contains(".session-agent-group"));
        assert!(webui_css_body.contains(".session-agent-button"));
        assert!(webui_css_body.contains(".session-agent-sessions"));
        assert!(webui_css_body.contains(".session-item[data-session-kind=\"task\"]"));
        assert!(webui_css_body.contains("env(safe-area-inset-bottom)"));
        assert!(webui_css_body.contains(".work-context-tags"));
        assert!(webui_css_body.contains(".context-tag"));
        assert!(webui_css_body.contains(".new-session-dialog"));
        assert!(webui_css_body.contains(".new-task-path-presets"));
        assert!(webui_css_body.contains(".path-preset-button"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_portrait\"][data-composer-focused=\"true\"] .composer-card"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_portrait\"][data-composer-focused=\"true\"] .composer-control-strip"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_portrait\"] #send-button"));

        let js = client
            .get(format!("{}/assets/webui.js", server.base_url))
            .send()
            .await
            .expect("js response");
        assert_eq!(js.status(), StatusCode::OK);
        assert!(
            js.text()
                .await
                .expect("js body")
                .contains("initializeThemeToggle")
        );
        let js_body = client
            .get(format!("{}/assets/webui.js", server.base_url))
            .send()
            .await
            .expect("js response 2")
            .text()
            .await
            .expect("js body 2");
        assert!(js_body.contains("new WebSocket(adpUrl())"));
        assert!(js_body.contains("new EventSource(endpoint)"));
        assert!(js_body.contains("function ensureSseTurnSubscription"));
        assert!(js_body.contains("SSE turn refresh received"));
        assert!(js_body.contains("function classifyLayoutShape"));
        assert!(js_body.contains("function applyLayoutShape"));
        assert!(js_body.contains("function viewportDimensionsForLayout"));
        assert!(js_body.contains("function setMobileDrawer"));
        assert!(js_body.contains("function showInspectorPanel"));
        assert!(js_body.contains("function renderSettingsShell"));
        assert!(js_body.contains("open-settings-drawer-button"));
        assert!(js_body.contains("settings-shell-toggle"));
        assert!(
            js_body.contains(
                "state.inspectorPanel = panel === \"settings\" ? \"settings\" : \"debug\""
            )
        );
        assert!(js_body.contains("const attachments = currentAttachments();"));
        assert!(!js_body.contains("currentDraftAttachments"));
        assert!(js_body.contains("case \"/settings\""));
        assert!(js_body.contains("function syncMobileDrawerForLayout"));
        assert!(js_body.contains("function installMobileSessionSwipeGesture"));
        assert!(js_body.contains("setMobileDrawer(\"sessions\")"));
        assert!(js_body.contains("deltaX >= openThreshold"));
        assert!(js_body.contains("window.__freehandLayout"));
        assert!(js_body.contains("document.body.dataset.layoutShape"));
        assert!(js_body.contains("shell.dataset.layoutShape"));
        assert!(js_body.contains("document.body.dataset.mobileDrawer"));
        assert!(js_body.contains("shell.dataset.mobileDrawer"));
        assert!(js_body.contains("function setComposerFocused"));
        assert!(js_body.contains("document.body.dataset.composerFocused"));
        assert!(js_body.contains("shell.dataset.composerFocused"));
        assert!(js_body.contains("composerInput.addEventListener(\"focus\""));
        assert!(js_body.contains("composerInput.addEventListener(\"blur\""));
        assert!(js_body.contains("function openNewSessionDialog"));
        assert!(js_body.contains("function submitNewSessionDialog"));
        assert!(js_body.contains("newTaskPathPresets.addEventListener(\"click\""));
        assert!(js_body.contains("DeleteSession"));
        assert!(!js_body.contains("ArchiveSession"));
        assert!(!js_body.contains("RestoreSession"));
        assert!(!js_body.contains("QueryArchivedSessionList"));
        assert!(js_body.contains("worker-context-tag"));
        assert!(js_body.contains("task-context-tag"));
        assert!(js_body.contains("transport-context-tag"));
        assert!(js_body.contains("open-session-drawer-button"));
        assert!(js_body.contains("open-detail-drawer-button"));
        assert!(js_body.contains("setMobileDrawer(\"settings\")"));
        assert!(!js_body.contains("localStorage.setItem(\"freehand-config"));
        assert!(!js_body.contains("writeConfig"));
        assert!(!js_body.contains("apiKey"));
        assert!(js_body.contains("window.visualViewport.addEventListener(\"resize\", () =>"));
        assert!(js_body.contains("window.addEventListener(\"orientationchange\", () =>"));
        assert!(js_body.contains("return \"phone_portrait\""));
        assert!(js_body.contains("return \"phone_landscape\""));
        assert!(js_body.contains("return \"tablet_portrait\""));
        assert!(js_body.contains("return \"tablet_landscape\""));
        assert!(js_body.contains("return \"foldable_unfolded\""));
        assert!(js_body.contains("return \"desktop_large\""));
        assert!(js_body.contains("adpSubscribe"));
        assert!(js_body.contains("subscription_accepted"));
        assert!(!js_body.contains("fetch("));
        assert!(js_body.contains("refreshCheckpoints"));
        assert!(js_body.contains("freehand-webui-selected-session"));
        assert!(js_body.contains("window.localStorage.getItem"));
        assert!(js_body.contains("QuerySessionList"));
        assert!(js_body.contains("QuerySessionTurns"));
        assert!(js_body.contains("refreshSelectedSession"));
        assert!(js_body.contains("newDraftSessionId"));
        assert!(js_body.contains("function browserRandomId"));
        assert!(js_body.contains("typeof cryptoApi.randomUUID === \"function\""));
        assert!(js_body.contains("cryptoApi.getRandomValues(bytes)"));
        assert!(!js_body.contains("crypto.randomUUID().slice"));
        assert!(js_body.contains("initialSelectedSessionId"));
        assert!(js_body.contains("isDraftSessionId"));
        assert!(js_body.contains("startNewConversation"));
        assert!(js_body.contains("startNewTask"));
        assert!(js_body.contains("selectedSessionIds"));
        assert!(js_body.contains("expandedAgentIds"));
        assert!(js_body.contains("function sessionAgentId"));
        assert!(js_body.contains("function groupedSessionsByAgent"));
        assert!(js_body.contains("function renderSessionAgentGroup"));
        assert!(js_body.contains("function renderSessionItem"));
        assert!(js_body.contains("session.source_agent_id"));
        assert!(js_body.contains("state.turn.source.source_agent_id"));
        assert!(js_body.contains("selectAllSessions"));
        assert!(js_body.contains("draftSessionId: null"));
        assert!(js_body.contains("state.draftSessionId === sessionId"));
        assert!(!js_body.contains("startsWith(\"webui-session-\")"));
        assert!(js_body.contains("if (state.draftSessionId)"));
        assert!(js_body.contains("Send a message to start this session."));
        assert!(js_body.contains("function pendingExecutionCard"));
        assert!(js_body.contains("function turnExecutionCard"));
        assert!(js_body.contains("function turnChatCards"));
        assert!(js_body.contains("function userChatBubble"));
        assert!(js_body.contains("function assistantChatBubble"));
        assert!(js_body.contains("function renderToolSection"));
        assert!(js_body.contains("function toolSemanticLines"));
        assert!(js_body.contains("function buildConversationRenderModel"));
        assert!(js_body.contains("function buildRenderTurn"));
        assert!(js_body.contains("function buildRenderRows"));
        assert!(js_body.contains("function buildToolActivityRenderRow"));
        assert!(js_body.contains("function buildModelRequestRenderRow"));
        assert!(js_body.contains("function buildObservableLiveTurnRenderRow"));
        assert!(js_body.contains("function inactiveToolLifecycleForRender"));
        assert!(js_body.contains("phase: \"tool_failed\""));
        assert!(js_body.contains("phase: \"tool_completed\""));
        assert!(
            js_body.contains("const inactiveToolLifecycle = inactiveToolLifecycleForRender(turn);")
        );
        assert!(js_body.contains("request accepted; waiting for protocol-visible turn details"));
        assert!(js_body.contains("function pendingUserInputIsMaterialized"));
        assert!(js_body.contains("function clearPendingUserInputIfMaterialized"));
        assert!(js_body.contains("clearPendingUserInputIfMaterialized();"));
        assert!(js_body.contains("function activeTurnForSelectedSession"));
        assert!(js_body.contains(
            "state.selectedSessionId && state.turn.session_id !== state.selectedSessionId"
        ));
        assert!(js_body.contains("function sameRenderableTurn"));
        assert!(js_body.contains("const merged = [];"));
        assert!(js_body.contains("sameRenderableTurn(existing, turn)"));
        assert!(js_body.contains("state.sessionTurns = logicalSessionTurns"));
        assert!(js_body.contains("sameRenderableTurn(existing, state.turn)"));
        assert!(js_body.contains("sameRenderableTurn(turn, latestTurn)"));
        assert!(!js_body.contains("existing.turn_id === state.turn.turn_id"));
        assert!(js_body.contains("function renderModelHasLiveLifecycle"));
        assert!(js_body.contains("function turnIsCurrentLiveTurn"));
        assert!(js_body.contains("conversationTurns.length === 0 && state.turn"));
        assert!(
            !js_body
                .contains("conversationTurns.length === 0 && turnIsCurrentLiveTurn(state.turn)")
        );
        assert!(js_body.contains("fragments.push(...turnChatCards(renderTurn))"));
        assert!(js_body.contains("function uniqueChatFragments"));
        assert!(js_body.contains("uniqueChatFragments(fragments).forEach"));
        assert!(js_body.contains("fragment.dataset.turnId"));
        assert!(js_body.contains("previousAssistantText"));
        assert!(
            js_body
                .find("fragments.push(...turnChatCards(renderTurn))")
                .expect("turn chat render push")
                < js_body
                    .find("fragments.push(...pendingChatCards(renderModel.pendingSubmit))")
                    .expect("pending chat render push")
        );
        assert!(js_body.contains("deleteSelectedSessions"));
        assert!(js_body.contains("RollbackLatestSessionTurn"));
        assert!(js_body.contains("rollbackLatestSessionTurn"));
        assert!(js_body.contains("session-selector"));
        assert!(js_body.contains("session-rename-selected-button"));
        assert!(js_body.contains("renderSessionBulkToolbar"));
        assert!(js_body.contains("selectedWorkspaceCwd"));
        assert!(js_body.contains("requireTaskCwd"));
        assert!(js_body.contains("requires a task target directory"));
        assert!(js_body.contains("CreateSession"));
        assert!(js_body.contains("SubmitUserInput.session_id"));
        assert!(js_body.contains("SubmitUserInput.cwd"));
        assert!(js_body.contains("freehand-webui-selected-cwd"));
        assert!(js_body.contains("turn.session_id !== state.selectedSessionId"));
        assert!(js_body.contains("scrollMessagesToBottom"));
        assert!(js_body.contains("messageListIsNearBottom"));
        assert!(js_body.contains("window.scrollY"));
        assert!(js_body.contains("forceScrollToBottom"));
        assert!(js_body.contains("const streamStage = document.querySelector(\".stream-stage\")"));
        assert!(js_body.contains("streamStage.scrollTop = streamStage.scrollHeight"));
        assert!(!js_body.contains("window.scrollTo({ top: document.documentElement.scrollHeight"));
        assert!(js_body.contains("function conversationTurnsForRender"));
        assert!(js_body.contains("if (!state.selectedSessionId)"));
        assert!(js_body.contains("sessionListLoaded: false"));
        assert!(js_body.contains("state.sessionListLoaded = true"));
        assert!(js_body.contains("function sessionTruthAllowsTurn"));
        assert!(js_body.contains("function sessionTruthAllowsSessionId"));
        assert!(js_body.contains("!state.selectedSessionId && !state.sessionListLoaded"));
        assert!(js_body.contains("if (turn && !sessionTruthAllowsTurn(turn))"));
        assert!(js_body.contains("!sessionTruthAllowsSessionId(projection.session_id)"));
        assert!(js_body.contains("clearLocalConversationTruth"));
        assert!(
            !js_body.contains(
                "state.selectedSessionId && !hasSelectedSessionTranscript && !state.turn"
            )
        );
        assert!(!js_body.contains("WebUI 正在查询最新 turn。"));
        assert!(!js_body.contains("等待数据"));
        assert!(!root_body.contains("WebUI 正在查询最新 turn。"));
        assert!(!root_body.contains("等待数据"));
        assert!(root_body.contains("New conversation"));
        assert!(root_body.contains("Send a message to start this session."));
        assert!(js_body.contains("if (projection && projection.session_id)"));
        assert!(
            !js_body.contains(
                "if (projection && projection.session_id && state.sessionTurns.length > 0)"
            )
        );
        assert!(js_body.contains("turns.filter(Boolean).forEach"));
        assert!(js_body.contains("merged[index] = turn;"));
        assert!(!js_body.contains("function compareTurnIds"));
        assert!(js_body.contains("latestTurn.session_id !== state.selectedSessionId"));
        assert!(js_body.contains("const renderModel = buildConversationRenderModel();"));
        assert!(js_body.contains("function successorBaseTurnIds"));
        assert!(js_body.contains("hideTerminal: successorBaseTurns.has"));
        assert!(js_body.contains("case \"/new\""));
        assert!(js_body.contains("case \"/task\""));
        assert!(js_body.contains("case \"/cwd\""));
        assert!(!js_body.contains("selected session:"));
        assert!(js_body.contains("CancelTurn"));
        assert!(js_body.contains("CancelLatestActiveTurn"));
        assert!(js_body.contains("event.key !== \"Escape\""));
        assert!(js_body.contains("cancelActiveTurn"));
        assert!(js_body.contains("normalizePublicConversation"));
        assert!(js_body.contains("isInternalRuntimePrompt"));
        assert!(!js_body.contains("__hideUserRow"));
        assert!(!js_body.contains("logicalExecutionKey"));
        assert!(!js_body.contains("__supersededRound"));
        assert!(!js_body.contains("continued"));
        assert!(!js_body.contains("modelRequestStatusForTurn"));
        assert!(!js_body.contains("function logicalTurnKey"));
        assert!(!js_body.contains("function mergeLogicalTurnGroup"));
        assert!(js_body.contains("logicalSessionTurns(state.sessionTurns)"));
        assert!(js_body.contains("stripFreehandCompletionBlock"));
        assert!(js_body.contains("stripped.includes(\"</freehand_completion>\")"));
        assert!(js_body.contains("const leftInternal = isInternalRuntimePrompt(left);"));
        assert!(js_body.contains("return leftInternal && rightInternal;"));
        assert!(!js_body.contains(
            "left.session_id &&\n    right.session_id &&\n    left.session_id === right.session_id"
        ));
        assert!(js_body.contains("terminalBodyForDisplay"));
        assert!(js_body.contains("terminalSummaryLine"));
        assert!(js_body.contains("stripDebugTerminalLines"));
        assert!(js_body.contains("debugDetailsVisible"));
        assert!(js_body.contains("debugDetailsToggle"));
        assert!(js_body.contains("Debug off"));
        assert!(js_body.contains("<freehand_completion>"));
        assert!(js_body.contains("toolSummaryBody"));
        assert!(js_body.contains("renderToolBody"));
        assert!(js_body.contains("pushCompactToolLine"));
        assert!(js_body.contains("escapeRegExp"));
        assert!(js_body.contains("display.parameter_summary"));
        assert!(js_body.contains("elapsedSince"));
        assert!(js_body.contains("submitStartedAt"));
        assert!(js_body.contains("pendingSubmitId"));
        assert!(js_body.contains("pendingSubmitSessionId"));
        assert!(js_body.contains("turn.submit_id !== submitId"));
        assert!(js_body.contains("turn.session_id !== submitSessionId"));
        assert!(js_body.contains("modelRequestTimingKey"));
        assert!(js_body.contains("modelRequestKind"));
        assert!(js_body.contains("function modelRequestPhase"));
        assert!(js_body.contains("phase: modelRequestPhase(turn)"));
        assert!(js_body.contains("modelRequestLabel"));
        assert!(js_body.contains("turnIsWaitingForModelResponse"));
        assert!(js_body.contains("schema polishing"));
        assert!(js_body.contains("thinking after tool result"));
        assert!(!js_body.contains("turnIsWaitingForModel("));
        assert!(js_body.contains("rememberInputHistory"));
        assert!(js_body.contains("recallInputHistory"));
        assert!(js_body.contains("ArrowUp"));
        assert!(js_body.contains("ArrowDown"));
        assert!(js_body.contains("toolTimelineLine"));
        assert!(js_body.contains("running-tool-state"));
        assert!(js_body.contains("className: \"pending\""));
        assert!(js_body.contains("label: \"waiting\""));
        assert!(js_body.contains("RenderLifecycle"));
        assert!(!js_body.contains("modelRequestBody"));
        assert!(!js_body.contains("modelWaitBody"));
        assert!(!js_body.contains("shouldRenderLiveWaitForTurn"));
        assert!(js_body.contains("compactToolResultLine"));
        assert!(js_body.contains("succeeded: result returned"));
        assert!(js_body.contains("succeeded: shell command"));
        assert!(js_body.contains("buildConversationRenderModel()"));
        assert!(js_body.contains("renderModelHasLiveLifecycle()"));
        assert!(!js_body.contains("hasPendingSubmit"));
        assert!(!js_body.contains("hasModelRequestWait"));
        assert!(!js_body.contains("hasModelWait"));
        assert!(js_body.contains("waitingToolStatus"));
        assert!(js_body.contains("tool.display || null"));
        assert!(js_body.contains("display.diff"));
        assert!(!js_body.contains("display.result_summary"));
        assert!(js_body.contains("compact-tool-state"));
        assert!(js_body.contains("display.fields"));
        assert!(js_body.contains("function assistantSectionHeadingLabel"));
        assert!(js_body.contains("if (row.kind === \"final\")"));
        assert!(js_body.contains("if (row.kind === \"system\")"));
        assert!(
            js_body.contains("const showSectionStatus = row.kind !== \"assistant\" && row.status;")
        );
        assert!(js_body.contains("if (headingLabel || showSectionStatus)"));
        assert!(js_body.contains("formatDuration"));
        assert!(js_body.contains("composerInput.value = \"\";"));
        assert!(js_body.contains("tool_call_id"));
        assert!(!js_body.contains("ADP"));
        assert!(!js_body.contains("ADP success sample"));
        assert!(!js_body.contains("ADP failure sample"));
        assert!(js_body.contains("scenario loaded"));
        assert!(js_body.contains("loadSamplePrompt"));
        assert!(js_body.contains("shortcutHelp"));
        assert!(js_body.contains("runSlashCommand"));
        assert!(js_body.contains("attachmentDraftStorageKey"));
        assert!(js_body.contains("freehand-webui-attachment-drafts-v1"));
        assert!(js_body.contains("loadAttachmentDrafts"));
        assert!(js_body.contains("persistAttachmentDrafts"));
        assert!(js_body.contains("addAttachmentFiles"));
        assert!(js_body.contains("renderAttachmentTray"));
        assert!(js_body.contains("textWithAttachmentPlaceholders"));
        assert!(js_body.contains("clearCurrentAttachments"));
        assert!(js_body.contains("dispatch failed; use ↑ to recall input"));
        assert!(js_body.contains("case \"/attachments\""));
        assert!(js_body.contains("case \"/model\""));
        assert!(js_body.contains("model selector is read-only"));
        assert!(js_body.contains("setCommandStatus"));
        assert!(js_body.contains("setBackgroundCommandStatus"));
        assert!(js_body.contains("adpRequestTimeoutMs"));
        assert!(js_body.contains("request timed out after"));
        assert!(js_body.contains("commandStatusStickyUntil"));
        assert!(js_body.contains("stickyMs"));
        assert!(js_body.contains("Cmd/Ctrl+Enter"));
        assert!(js_body.contains("requestSubmit()"));
        assert!(js_body.contains("refreshAllProtocolState"));
        assert!(js_body.contains("if (command.startsWith(\"/\"))"));
        assert!(js_body.contains("composerInput.value = \"\";"));
        assert!(js_body.contains("case \"/help\""));
        assert!(js_body.contains("case \"/new\""));
        assert!(js_body.contains("case \"/task\""));
        assert!(js_body.contains("case \"/sessions\""));
        assert!(js_body.contains("case \"/reload\""));
        assert!(js_body.contains("case \"/success\""));
        assert!(js_body.contains("case \"/failure\""));
        assert!(js_body.contains("case \"/cancel\""));
        assert!(js_body.contains("case \"/clear\""));
        assert!(webui_css_body.contains("composer-control-strip"));
        assert!(webui_css_body.contains("session-create-actions"));
        assert!(webui_css_body.contains("task-cwd-control"));
        assert!(webui_css_body.contains("attachment-tray"));
        assert!(webui_css_body.contains("attachment-chip"));
        assert!(!js_body.contains("Tool call requested"));
        assert!(!js_body.contains("Tool result returned for"));
        assert!(!js_body.contains("Tool execution failed for"));
        let turn_render_pos = js_body
            .find("const renderModel = buildConversationRenderModel();")
            .expect("turn render branch present");
        let adp_failure_pos = js_body
            .rfind("if (renderModel.adpFailure)")
            .expect("adp failure branch present");
        assert!(
            adp_failure_pos > turn_render_pos,
            "connection failure card must render after conversation timeline branch"
        );

        server.stop().await;
    }

    #[tokio::test]
    async fn transport_subscribe_smoke_returns_sse_turn_and_debug_events() {
        let server = TestServer::spawn().await;
        let client = Client::builder().build().expect("client");

        let mut turn_sse = client
            .get(format!("{}/ui/subscribe/turn/latest", server.base_url))
            .send()
            .await
            .expect("turn sse");
        assert_eq!(turn_sse.status(), StatusCode::OK);
        let mut turn_buffer = String::new();
        let turn_body = read_next_sse_event(&mut turn_sse, &mut turn_buffer).await;
        assert!(turn_body.contains("event: turn"));
        assert!(turn_body.contains("\"turn_id\":\"turn-webui-smoke\""));
        assert!(turn_body.contains("\"slave_substream_card\":true"));
        assert!(turn_body.contains("\"public_conversation\""));

        server
            .protocol_state
            .lock()
            .expect("lock protocol")
            .apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
                source_agent_id: AgentId::new("slave-agent"),
                source_node_id: "slave-node".to_owned(),
                session_id: SessionId::new("session-webui-smoke"),
                turn_id: TurnId::new("turn-webui-smoke-2"),
                cwd: None,
                user_text: Some("second prompt".to_owned()),
                semantic_events: vec![ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-webui-smoke"),
                    turn_id: TurnId::new("turn-webui-smoke-2"),
                    trace_id: TraceId::new("trace-webui-smoke-2"),
                    feature_id: FeatureId::new("app.webui-smoke"),
                    agent_id: AgentId::new("slave-agent"),
                    kind: SemanticEventKind::Text,
                    content: "second answer".to_owned(),
                }],
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                usage_events: Vec::new(),
                terminal_event: None,
                error_events: Vec::new(),
                slave_substream_card: true,
            }));
        let turn_body = read_next_sse_event(&mut turn_sse, &mut turn_buffer).await;
        assert!(turn_body.contains("\"turn_id\":\"turn-webui-smoke-2\""));
        assert!(turn_body.contains("\"public_conversation\""));
        assert!(turn_body.contains("second answer"));

        let mut debug_sse = client
            .get(format!(
                "{}/ui/subscribe/debug/turn-webui-smoke",
                server.base_url
            ))
            .send()
            .await
            .expect("debug sse");
        assert_eq!(debug_sse.status(), StatusCode::OK);
        let mut debug_buffer = String::new();
        let debug_body = read_next_sse_event(&mut debug_sse, &mut debug_buffer).await;
        assert!(debug_body.contains("event: debug"));
        assert!(debug_body.contains("\"status_text\":\"debug state from protocol query\""));

        server
            .protocol_state
            .lock()
            .expect("lock protocol")
            .set_debug_state(DebugStateSnapshot::new(
                DebugSemanticPosition {
                    feature_id: FeatureId::new("app.webui-smoke"),
                    session_id: SessionId::new("session-webui-smoke"),
                    turn_id: TurnId::new("turn-webui-smoke"),
                    trace_id: TraceId::new("trace-webui-smoke"),
                    agent_id: Some(AgentId::new("slave-agent")),
                    pipeline_node: Some("UiDebugState".to_owned()),
                },
                DebugScenePosition {
                    crate_name: "freehand-server".to_owned(),
                    file: "src/lib.rs".to_owned(),
                    function: "transport_subscribe_smoke_returns_sse_turn_and_debug_events"
                        .to_owned(),
                    line: None,
                    artifact_path: None,
                    raw_exchange_id: None,
                },
                "debug state updated",
                vec!["detail=second".to_owned()],
            ));
        let debug_body = read_next_sse_event(&mut debug_sse, &mut debug_buffer).await;
        assert!(debug_body.contains("\"status_text\":\"debug state updated\""));

        drop(turn_sse);
        drop(debug_sse);
        server.stop().await;
    }

    #[tokio::test]
    async fn latest_turn_subscribe_waits_on_blank_state_until_first_turn() {
        let server = TestServer::spawn_empty().await;
        let client = Client::builder().build().expect("client");

        let query = client
            .get(format!("{}/ui/query/latest-active-turn", server.base_url))
            .send()
            .await
            .expect("blank query");
        assert_eq!(query.status(), StatusCode::NOT_FOUND);

        let mut turn_sse = client
            .get(format!("{}/ui/subscribe/turn/latest", server.base_url))
            .send()
            .await
            .expect("turn sse");
        assert_eq!(turn_sse.status(), StatusCode::OK);

        server
            .protocol_state
            .lock()
            .expect("lock protocol")
            .apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
                source_agent_id: AgentId::new("slave-agent"),
                source_node_id: "slave-node".to_owned(),
                session_id: SessionId::new("session-webui-smoke"),
                turn_id: TurnId::new("turn-webui-first"),
                cwd: None,
                user_text: Some("first prompt".to_owned()),
                semantic_events: vec![ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-webui-smoke"),
                    turn_id: TurnId::new("turn-webui-first"),
                    trace_id: TraceId::new("trace-webui-first"),
                    feature_id: FeatureId::new("app.webui-smoke"),
                    agent_id: AgentId::new("slave-agent"),
                    kind: SemanticEventKind::Text,
                    content: "first answer".to_owned(),
                }],
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                usage_events: Vec::new(),
                terminal_event: None,
                error_events: Vec::new(),
                slave_substream_card: false,
            }));

        let mut turn_buffer = String::new();
        let turn_body = read_next_sse_event(&mut turn_sse, &mut turn_buffer).await;
        assert!(turn_body.contains("event: turn"));
        assert!(turn_body.contains("\"turn_id\":\"turn-webui-first\""));
        assert!(turn_body.contains("first prompt"));
        assert!(turn_body.contains("first answer"));

        drop(turn_sse);
        server.stop().await;
    }

    #[tokio::test]
    async fn debug_subscribe_waits_when_turn_arrives_before_debug_snapshot() {
        let server = TestServer::spawn_empty().await;
        let client = Client::builder().build().expect("client");

        server
            .protocol_state
            .lock()
            .expect("lock protocol")
            .apply_turn_projection(turn_projection_from_events(TurnProjectionInput {
                source_agent_id: AgentId::new("slave-agent"),
                source_node_id: "slave-node".to_owned(),
                session_id: SessionId::new("session-webui-smoke"),
                turn_id: TurnId::new("turn-debug-late"),
                cwd: None,
                user_text: Some("debug should arrive later".to_owned()),
                semantic_events: vec![ReasonResp01SemanticEvent {
                    session_id: SessionId::new("session-webui-smoke"),
                    turn_id: TurnId::new("turn-debug-late"),
                    trace_id: TraceId::new("trace-debug-late"),
                    feature_id: FeatureId::new("app.webui-smoke"),
                    agent_id: AgentId::new("slave-agent"),
                    kind: SemanticEventKind::Text,
                    content: "answer before debug".to_owned(),
                }],
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                usage_events: Vec::new(),
                terminal_event: None,
                error_events: Vec::new(),
                slave_substream_card: false,
            }));

        let debug_query = client
            .get(format!(
                "{}/ui/query/debug/turn-debug-late",
                server.base_url
            ))
            .send()
            .await
            .expect("debug query");
        assert_eq!(debug_query.status(), StatusCode::NOT_FOUND);

        let mut debug_sse = client
            .get(format!(
                "{}/ui/subscribe/debug/turn-debug-late",
                server.base_url
            ))
            .send()
            .await
            .expect("debug sse");
        assert_eq!(debug_sse.status(), StatusCode::OK);

        server
            .protocol_state
            .lock()
            .expect("lock protocol")
            .set_debug_state(DebugStateSnapshot::new(
                DebugSemanticPosition {
                    feature_id: FeatureId::new("app.webui-smoke"),
                    session_id: SessionId::new("session-webui-smoke"),
                    turn_id: TurnId::new("turn-debug-late"),
                    trace_id: TraceId::new("trace-debug-late"),
                    agent_id: Some(AgentId::new("slave-agent")),
                    pipeline_node: Some("UiDebugState".to_owned()),
                },
                DebugScenePosition {
                    crate_name: "freehand-server".to_owned(),
                    file: "src/lib.rs".to_owned(),
                    function: "debug_subscribe_waits_when_turn_arrives_before_debug_snapshot"
                        .to_owned(),
                    line: None,
                    artifact_path: None,
                    raw_exchange_id: None,
                },
                "debug arrived after turn",
                vec!["detail=late-debug".to_owned()],
            ));

        let mut debug_buffer = String::new();
        let debug_body = read_next_sse_event(&mut debug_sse, &mut debug_buffer).await;
        assert!(debug_body.contains("event: debug"));
        assert!(debug_body.contains("\"status_text\":\"debug arrived after turn\""));

        drop(debug_sse);
        server.stop().await;
    }

    #[tokio::test]
    async fn latest_turn_sse_streams_tool_waiting_and_completed_status() {
        let server = TestServer::spawn_empty().await;
        let client = Client::builder().build().expect("client");

        let mut turn_sse = client
            .get(format!("{}/ui/subscribe/turn/latest", server.base_url))
            .send()
            .await
            .expect("turn sse");
        assert_eq!(turn_sse.status(), StatusCode::OK);

        let tool_call = ReasonReq04ToolCall {
            session_id: SessionId::new("session-webui-smoke"),
            turn_id: TurnId::new("turn-tool-sse"),
            trace_id: TraceId::new("trace-tool-sse"),
            feature_id: FeatureId::new("app.webui-smoke"),
            agent_id: AgentId::new("slave-agent"),
            tool_call: ToolCallContract {
                tool_call_id: ToolCallId::new("tool-sse-1"),
                tool_name: "read_file".to_owned(),
                arguments: vec![freehand_contracts::ToolArgument {
                    name: "path".to_owned(),
                    value: serde_json::json!("src/lib.rs"),
                }],
                arguments_complete: true,
            },
        };

        server
            .protocol_state
            .lock()
            .expect("lock protocol")
            .apply_tool_call(
                AgentId::new("slave-agent"),
                "slave-node".to_owned(),
                &tool_call,
                false,
            );

        let mut turn_buffer = String::new();
        let waiting_body = read_next_sse_event(&mut turn_sse, &mut turn_buffer).await;
        assert!(waiting_body.contains("event: turn"));
        assert!(waiting_body.contains("\"turn_id\":\"turn-tool-sse\""));
        assert!(waiting_body.contains("\"status\":\"waiting\""));
        assert!(waiting_body.contains("read_file"));

        server
            .protocol_state
            .lock()
            .expect("lock protocol")
            .apply_tool_result(
                AgentId::new("slave-agent"),
                "slave-node".to_owned(),
                &ReasonReq05ToolResultReentry {
                    session_id: SessionId::new("session-webui-smoke"),
                    turn_id: TurnId::new("turn-tool-sse"),
                    trace_id: TraceId::new("trace-tool-sse"),
                    feature_id: FeatureId::new("app.webui-smoke"),
                    agent_id: AgentId::new("slave-agent"),
                    tool_result: ToolResultContract {
                        tool_call_id: ToolCallId::new("tool-sse-1"),
                        status: freehand_contracts::ToolResultStatus::Success,
                        output: "visible result body".to_owned(),
                    },
                },
                false,
            );

        let completed_body = read_next_sse_event(&mut turn_sse, &mut turn_buffer).await;
        assert!(completed_body.contains("\"status\":\"completed\""));
        assert!(completed_body.contains("\"title\":\"Read file\""));
        assert!(completed_body.contains("\"body\":\"path=src/lib.rs\""));
        assert!(completed_body.contains("\"kind\":\"ReadFile\""));

        drop(turn_sse);
        server.stop().await;
    }

    #[tokio::test]
    async fn transport_command_ingress_smoke_accepts_mutation_and_rejects_query_route_misuse() {
        let server = TestServer::spawn().await;
        let client = Client::builder().build().expect("client");

        let accepted = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "run task".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("command response");
        assert_eq!(accepted.status(), StatusCode::ACCEPTED);
        let accepted: UiCommandDispatchReceipt = accepted.json().await.expect("receipt json");
        assert_eq!(accepted.ingress.command_kind, "submit_user_input");
        assert_eq!(accepted.ingress.mutation_authority, "owner_modules");
        assert_eq!(accepted.target_feature_id, "reason.turn");
        assert_eq!(accepted.dispatch_status, "queued_by_static_dispatch_port");

        let rejected = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::QueryLatestActiveTurn)
            .send()
            .await
            .expect("reject response");
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
        let rejected: UiCommandDispatchFailure = rejected.json().await.expect("reject json");
        assert_eq!(rejected.code, "ingress_command_kind_mismatch");
        assert!(!rejected.retryable);

        server.stop().await;
    }

    #[tokio::test]
    async fn transport_command_ingress_surfaces_dispatch_port_failure_explicitly() {
        let server = TestServer::spawn_with_state_and_port(
            seed_webui_protocol_state(),
            Arc::new(FailingUiCommandDispatchPort),
        )
        .await;
        let client = Client::builder().build().expect("client");

        let failure = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "run task".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("command response");
        assert_eq!(failure.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let failure: UiCommandDispatchFailure = failure.json().await.expect("failure json");
        assert_eq!(failure.code, "command_dispatch_port_failure");
        assert!(failure.retryable);
        assert!(failure.message.contains("runtime queue unavailable"));

        server.stop().await;
    }

    #[tokio::test]
    async fn transport_command_ingress_surfaces_dispatch_join_failure_explicitly() {
        let server = TestServer::spawn_with_state_and_port(
            seed_webui_protocol_state(),
            Arc::new(PanicUiCommandDispatchPort),
        )
        .await;
        let client = Client::builder().build().expect("client");

        let failure = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "run task".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("command response");
        assert_eq!(failure.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let failure: UiCommandDispatchFailure = failure.json().await.expect("failure json");
        assert_eq!(failure.code, "dispatch_join_failed");
        assert!(!failure.retryable);
        assert!(failure.message.contains("command dispatch task failed"));

        server.stop().await;
    }
}
