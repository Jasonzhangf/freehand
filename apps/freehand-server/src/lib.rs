mod assets;
mod page;
mod remote_relay;

use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Html;
use axum::response::IntoResponse;
use axum::response::Response;
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

pub use remote_relay::{
    RemoteRelayAccountDirectory, RemoteRelayDirectory, RemoteRelayEndpointCandidate,
    RemoteRelayHostRecord, RemoteRelayHostRegistration, build_remote_relay_router,
    serve_remote_relay_listener,
};

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
        .route("/android/update.json", get(handle_android_update_manifest))
        .route(
            "/android/freehand-android.apk",
            get(handle_android_update_apk),
        )
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

async fn handle_root(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    (
        [(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, max-age=0"),
        )],
        Html(render_webui_smoke_for_client(
            params.get("client").map(String::as_str),
        )),
    )
}

async fn handle_android_update_manifest() -> Result<Response, StatusCode> {
    let body = android_update_manifest_body()?;
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        )
        .header(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, max-age=0"),
        )
        .body(Body::from(body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn android_update_manifest_body() -> Result<String, StatusCode> {
    if let Some(body) = android_update_manifest_env_body()? {
        return Ok(body);
    }
    read_android_update_manifest_file(&android_update_manifest_path())
}

fn android_update_manifest_env_body() -> Result<Option<String>, StatusCode> {
    let version_code = std::env::var("FREEHAND_ANDROID_VERSION_CODE").ok();
    let version_name = std::env::var("FREEHAND_ANDROID_VERSION_NAME").ok();
    if version_code.is_none() && version_name.is_none() {
        return Ok(None);
    }
    let version_code = version_code
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let version_name = version_name
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let body = serde_json::json!({
        "versionCode": version_code,
        "versionName": version_name,
        "apkUrl": "/android/freehand-android.apk",
        "releaseNotes": "Freehand Android release artifact served by the current daemon.",
        "required": false
    })
    .to_string();
    Ok(Some(body))
}

fn read_android_update_manifest_file(path: &FsPath) -> Result<String, StatusCode> {
    let body = fs::read_to_string(path).map_err(|_| StatusCode::NOT_FOUND)?;
    validate_android_update_manifest_body(&body)?;
    Ok(body)
}

fn validate_android_update_manifest_body(body: &str) -> Result<(), StatusCode> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let version_code = value
        .get("versionCode")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if version_code == 0 {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let apk_url = value
        .get("apkUrl")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| *value == "/android/freehand-android.apk")
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if apk_url.is_empty() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(())
}

fn android_update_manifest_path() -> PathBuf {
    std::env::var_os("FREEHAND_ANDROID_UPDATE_MANIFEST_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_android_update_dist_path("update.json"))
}

async fn handle_android_update_apk() -> Result<Response, StatusCode> {
    let path = android_update_apk_path();
    let body = fs::read(path).map_err(|_| StatusCode::NOT_FOUND)?;
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.android.package-archive",
        )
        .header(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, max-age=0"),
        )
        .body(Body::from(body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(response)
}

fn android_update_apk_path() -> PathBuf {
    std::env::var_os("FREEHAND_ANDROID_APK_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_android_update_dist_path("freehand-android-release.apk"))
}

fn default_android_update_dist_path(file_name: &str) -> PathBuf {
    let runtime_home = std::env::var_os("FREEHAND_RUNTIME_HOME")
        .or_else(|| std::env::var_os("FREEHAND_DAEMON_WORKDIR"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".freehand")))
        .unwrap_or_else(|| PathBuf::from(".freehand"));
    runtime_home.join("dist").join("android").join(file_name)
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
        created_at: Some(10),
        timing: None,
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
        StaticUiCommandDispatchPort, UiAdpRequest, UiAdpResponse, UiCommand,
        UiCommandDispatchEnvelope, UiCommandDispatchPortError, UiQueryResult,
    };
    use futures_util::{SinkExt, StreamExt};
    use reqwest::Client;
    use std::ffi::OsString;
    use std::sync::OnceLock;
    use std::time::Duration;
    use tokio::sync::oneshot;
    use tokio::time::timeout;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    fn android_update_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[tokio::test]
    async fn asset_response_serves_shared_logo_png() {
        let response = assets::asset_response("logo.png").expect("logo asset response");
        assert_eq!(response.headers().get("content-type").unwrap(), "image/png");
        assert_eq!(
            response.headers().get("cache-control").unwrap(),
            "no-store, max-age=0"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("logo asset body");
        assert!(body.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(body.len() > 1024);
    }

    struct EnvSnapshot {
        values: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvSnapshot {
        fn capture(keys: &[&'static str]) -> Self {
            Self {
                values: keys
                    .iter()
                    .map(|key| (*key, std::env::var_os(key)))
                    .collect(),
            }
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (key, value) in &self.values {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }

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

    struct RelayTestServer {
        base_url: String,
        shutdown: Option<oneshot::Sender<()>>,
        task: tokio::task::JoinHandle<()>,
    }

    impl RelayTestServer {
        async fn spawn() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind relay");
            let addr = listener.local_addr().expect("relay local addr");
            let directory = Arc::new(Mutex::new(RemoteRelayDirectory::default()));
            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let task = tokio::spawn(async move {
                let shutdown = async move {
                    let _ = shutdown_rx.await;
                };
                serve_remote_relay_listener(listener, directory, shutdown)
                    .await
                    .expect("serve relay");
            });
            Self {
                base_url: format!("http://{addr}"),
                shutdown: Some(shutdown_tx),
                task,
            }
        }

        fn ws_url(&self, path: &str) -> String {
            let ws_base = self.base_url.replacen("http://", "ws://", 1);
            format!("{ws_base}{path}")
        }

        async fn stop(mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
            self.task.await.expect("join relay");
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

    #[tokio::test]
    async fn remote_relay_registers_directory_and_proxies_http_and_adp() {
        let upstream = TestServer::spawn().await;
        let relay = RelayTestServer::spawn().await;
        let client = Client::builder().build().expect("client");

        let registration = serde_json::json!({
            "accountId": "jason",
            "daemonId": "studio",
            "relayHostId": "studio-host",
            "upstreamBaseUrl": upstream.base_url,
            "endpoints": [
                {
                    "id": "relay:studio-host",
                    "kind": "relay",
                    "webUrl": "/relay/daemon/studio-host/",
                    "adpUrl": "/relay/daemon/studio-host/adp",
                    "relayHostId": "studio-host",
                    "authRequired": true,
                    "lastSeenUnix": 10
                }
            ]
        });
        let published = client
            .post(format!("{}/relay/hosts", relay.base_url))
            .json(&registration)
            .send()
            .await
            .expect("publish relay host");
        assert_eq!(published.status(), StatusCode::ACCEPTED);
        let published: RemoteRelayHostRecord = published.json().await.expect("published host");
        assert_eq!(published.account_id, "jason");
        assert_eq!(published.daemon_id, "studio");
        assert_eq!(published.relay_host_id, "studio-host");

        let directory = client
            .get(format!("{}/relay/directory/jason", relay.base_url))
            .send()
            .await
            .expect("relay directory");
        assert_eq!(directory.status(), StatusCode::OK);
        let directory: RemoteRelayAccountDirectory =
            directory.json().await.expect("directory json");
        assert_eq!(directory.schema_version, 1);
        assert_eq!(directory.account_id, "jason");
        assert_eq!(directory.daemons.len(), 1);
        assert_eq!(directory.daemons[0].relay_host_id, "studio-host");

        let health = client
            .get(format!(
                "{}/relay/daemon/studio-host/health",
                relay.base_url
            ))
            .send()
            .await
            .expect("relay health proxy");
        assert_eq!(health.status(), StatusCode::OK);
        assert_eq!(health.text().await.expect("health body"), "ok");

        let root = client
            .get(format!(
                "{}/relay/daemon/studio-host/?client=android-webview",
                relay.base_url
            ))
            .send()
            .await
            .expect("relay root proxy");
        assert_eq!(root.status(), StatusCode::OK);
        let root_body = root.text().await.expect("relay root body");
        assert!(root_body.contains("data-webui-shell=\"true\""));
        assert!(
            root_body.contains("href=\"/relay/daemon/studio-host/assets/theme.css?v="),
            "{root_body}"
        );
        assert!(
            root_body.contains("src=\"/relay/daemon/studio-host/assets/webui.js?v="),
            "{root_body}"
        );
        assert!(root_body.contains("data-adp-endpoint=\"/relay/daemon/studio-host/adp\""));
        assert!(root_body.contains(
            "data-turn-subscribe=\"/relay/daemon/studio-host/ui/subscribe/turn/latest\""
        ));
        assert!(!root_body.contains("href=\"/assets/theme.css"));
        assert!(!root_body.contains("data-adp-endpoint=\"/adp\""));

        let webui_css = client
            .get(format!(
                "{}/relay/daemon/studio-host/assets/webui.css?v=relay-test",
                relay.base_url
            ))
            .send()
            .await
            .expect("relay webui css proxy");
        assert_eq!(webui_css.status(), StatusCode::OK);
        assert!(
            webui_css
                .text()
                .await
                .expect("webui css body")
                .contains(".app-shell")
        );

        let webui_js = client
            .get(format!(
                "{}/relay/daemon/studio-host/assets/webui.js?v=relay-test",
                relay.base_url
            ))
            .send()
            .await
            .expect("relay webui js proxy");
        assert_eq!(webui_js.status(), StatusCode::OK);
        let webui_js_body = webui_js.text().await.expect("webui js body");
        assert!(webui_js_body.contains("from \"/relay/daemon/studio-host/assets/theme.js?v="));
        assert!(!webui_js_body.contains("from \"/assets/theme.js"));

        let turn_query = client
            .get(format!(
                "{}/relay/daemon/studio-host/ui/query/latest-active-turn",
                relay.base_url
            ))
            .send()
            .await
            .expect("relay latest-turn query proxy");
        assert_eq!(turn_query.status(), StatusCode::OK);
        let turn_query_body = turn_query.text().await.expect("turn query body");
        assert!(turn_query_body.contains("\"turn_id\":\"turn-webui-smoke\""));

        let (mut socket, _) = connect_async(relay.ws_url("/relay/daemon/studio-host/adp"))
            .await
            .expect("relay adp connect");
        socket
            .send(WsMessage::Text(
                serde_json::to_string(&UiAdpRequest::Query {
                    request_id: "relay-q".to_owned(),
                    query: UiCommand::QueryLatestActiveTurn,
                })
                .expect("request json")
                .into(),
            ))
            .await
            .expect("send relay adp query");
        let message = timeout(Duration::from_secs(10), socket.next())
            .await
            .expect("relay adp response timeout")
            .expect("relay adp response")
            .expect("relay adp message");
        let WsMessage::Text(text) = message else {
            panic!("unexpected relay ADP message: {message:?}");
        };
        let response: UiAdpResponse = serde_json::from_str(&text).expect("adp response");
        match response {
            UiAdpResponse::QueryResult {
                request_id,
                result: UiQueryResult::Turn(Some(turn)),
            } => {
                assert_eq!(request_id, "relay-q");
                assert_eq!(turn.turn_id, TurnId::new("turn-webui-smoke"));
            }
            other => panic!("unexpected relay ADP response: {other:?}"),
        }

        relay.stop().await;
        upstream.stop().await;
    }

    #[tokio::test]
    async fn remote_relay_rejects_unregistered_host_explicitly() {
        let relay = RelayTestServer::spawn().await;
        let client = Client::builder().build().expect("client");

        let response = client
            .get(format!(
                "{}/relay/daemon/missing-host/health",
                relay.base_url
            ))
            .send()
            .await
            .expect("missing relay host response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body: serde_json::Value = response.json().await.expect("error body");
        assert_eq!(body["code"], "relay_host_not_found");

        relay.stop().await;
    }

    #[test]
    fn webui_smoke_renders_shell_and_asset_routes() {
        let html = render_webui_smoke();
        assert!(html.contains("data-webui-shell=\"true\""));
        assert!(!html.contains("data-layout-client=\"android-webview\""));
        assert!(!html.contains("data-layout-shape=\"tablet_portrait\""));
        assert!(html.contains("/assets/theme.css"));
        assert!(html.contains("/assets/webui.css"));
        assert!(html.contains("/assets/logo.png"));
        assert!(html.contains("/assets/webui.js"));
        assert!(html.contains("20260724-timer-ui-phase2"));
        assert!(html.contains("data-adp-endpoint=\"/adp\""));
        assert!(html.contains("data-selected-session=\"\""));
        assert!(html.contains("data-selected-turn=\"\""));
        assert!(html.contains("id=\"session-list\""));
        assert!(html.contains("id=\"new-conversation-button\""));
        assert!(html.contains("id=\"new-task-button\""));
        assert!(html.contains("id=\"task-cwd-input\""));
        assert!(html.contains("id=\"settings-shell-toggle\""));
        assert!(html.contains("id=\"open-settings-drawer-button\""));
        assert!(html.contains("id=\"open-timer-dashboard-button\""));
        assert!(html.contains("id=\"open-tools-dashboard-button\""));
        assert!(html.contains("id=\"mobile-new-entry-button\""));
        assert!(html.contains("class=\"mobile-bottom-entries\""));
        assert!(html.contains("id=\"mobile-home-dashboard\""));
        assert!(html.contains("id=\"mobile-home-timer-marker\""));
        assert!(html.contains("id=\"mobile-home-timer-list\""));
        assert!(html.contains("Timer owner truth"));
        assert!(html.contains("id=\"timer-dashboard-dialog\""));
        assert!(html.contains("id=\"timer-dashboard-form\""));
        assert!(html.contains("id=\"timer-mode-input\""));
        assert!(html.contains("id=\"timer-repeat-kind-input\""));
        assert!(html.contains("id=\"timer-source-session-input\""));
        assert!(html.contains("id=\"timer-dashboard-refresh-button\""));
        assert!(html.contains("id=\"timer-dashboard-create-button\""));
        assert!(html.contains("id=\"timer-dashboard-list\""));
        assert!(html.contains("id=\"timer-dashboard-history\""));
        assert!(html.contains("Timers are independent runtime truth"));
        assert!(html.contains("id=\"settings-shell\""));
        assert!(html.contains("id=\"settings-review-tree\""));
        assert!(html.contains("id=\"settings-provider-form\""));
        assert!(html.contains("id=\"settings-provider-current-select\""));
        assert!(html.contains("id=\"settings-provider-fallback-select\""));
        assert!(html.contains("id=\"settings-provider-switch-button\""));
        assert!(html.contains("id=\"settings-provider-registry-list\""));
        assert!(html.contains("Add/update provider"));
        assert!(html.contains("Android APK update"));
        assert!(html.contains("id=\"settings-apk-update-check-button\""));
        assert!(html.contains("id=\"settings-apk-update-status\""));
        assert!(html.contains("Check APK update"));
        assert!(!html.contains("rootfs"));
        assert!(!html.contains("shared-folder"));
        assert!(!html.contains("mount-directory"));
        assert!(!html.contains("Skill settings pending"));
        assert!(!html.contains("Task settings pending"));
        assert!(!html.contains("Active agent"));
        assert!(!html.contains("Sessions and workspace"));
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
        assert!(html.contains("id=\"model-selector\""));
        assert!(html.contains("id=\"attachment-tray\""));
        assert!(!html.contains("work-context-tags"));
        assert!(!html.contains("topbar-strip"));
        assert!(!html.contains("slave-drawer"));
        assert!(!html.contains("id=\"strip-session\""));
        assert!(!html.contains("id=\"strip-turn\""));
        assert!(!html.contains("id=\"strip-cwd\""));
        assert!(!html.contains("id=\"slave-chip\""));
        assert!(!html.contains("id=\"worker-context-tag\""));
        assert!(!html.contains("id=\"task-context-tag\""));
        assert!(!html.contains("id=\"transport-context-tag\""));
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
        assert!(html.contains("id=\"open-settings-drawer-button\""));
        assert!(html.contains("id=\"open-timer-dashboard-button\""));
        assert!(html.contains("id=\"open-tools-dashboard-button\""));
        assert!(html.contains("id=\"mobile-new-entry-button\""));
    }

    #[tokio::test]
    async fn android_mock_route_is_removed() {
        let server = TestServer::spawn().await;
        let client = Client::builder().build().expect("client");

        let page = client
            .get(format!("{}/mock/android", server.base_url))
            .send()
            .await
            .expect("removed mock page response");
        assert_eq!(page.status(), StatusCode::NOT_FOUND);

        let css = client
            .get(format!(
                "{}/assets/mocks/android/mobile-mock.css",
                server.base_url
            ))
            .send()
            .await
            .expect("removed mock css response");
        assert_eq!(css.status(), StatusCode::NOT_FOUND);

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
        assert_eq!(
            root.headers().get("cache-control").unwrap(),
            "no-store, max-age=0"
        );
        let root_body = root.text().await.expect("root body");
        assert!(root_body.contains("data-webui-shell=\"true\""));
        assert!(!root_body.contains("data-layout-client=\"android-webview\""));
        assert!(!root_body.contains("data-layout-shape=\"tablet_portrait\""));
        assert!(root_body.contains("/assets/theme.css"));
        assert!(root_body.contains("/assets/logo.png"));
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
        assert!(root_body.contains("id=\"open-timer-dashboard-button\""));
        assert!(root_body.contains("id=\"open-tools-dashboard-button\""));
        assert!(root_body.contains("id=\"mobile-new-entry-button\""));
        assert!(root_body.contains("class=\"mobile-bottom-entries\""));
        assert!(root_body.contains("id=\"mobile-home-dashboard\""));
        assert!(root_body.contains("id=\"settings-shell\""));
        assert!(root_body.contains("id=\"settings-review-tree\""));
        assert!(root_body.contains("id=\"settings-provider-host\""));
        assert!(root_body.contains("id=\"settings-provider-auth\""));
        assert!(root_body.contains("id=\"settings-config-error\""));
        assert!(root_body.contains("id=\"settings-provider-current-select\""));
        assert!(root_body.contains("id=\"settings-provider-fallback-select\""));
        assert!(root_body.contains("id=\"settings-provider-switch-button\""));
        assert!(root_body.contains("id=\"settings-provider-registry-list\""));
        assert!(root_body.contains("id=\"settings-provider-form\""));
        assert!(root_body.contains("id=\"task-board-status\""));
        assert!(root_body.contains("id=\"task-board-list\""));
        assert!(root_body.contains("id=\"agent-board-status\""));
        assert!(root_body.contains("id=\"agent-board-list\""));
        assert!(root_body.contains("id=\"event-inbox-status\""));
        assert!(root_body.contains("id=\"event-inbox-list\""));
        assert!(root_body.contains("id=\"task-history-status\""));
        assert!(root_body.contains("id=\"task-history-list\""));
        assert!(root_body.contains("id=\"worker-control-status\""));
        assert!(root_body.contains("id=\"worker-control-list\""));
        assert!(root_body.contains("id=\"mobile-agent-summary-strip\""));
        assert!(root_body.contains("id=\"open-mobile-agent-sheet-button\""));
        assert!(root_body.contains("id=\"mobile-agent-sheet\""));
        assert!(root_body.contains("id=\"close-mobile-agent-sheet-button\""));
        assert!(root_body.contains("id=\"session-relation-header\""));
        assert!(root_body.contains("id=\"session-relation-toggle-button\""));
        assert!(root_body.contains("id=\"session-tree-dropdown\""));
        assert!(root_body.contains("id=\"session-tree\""));
        assert!(root_body.contains("id=\"worker-session-nav\""));
        assert!(root_body.contains("Back to Master"));
        assert!(root_body.contains("id=\"settings-agent-resource-count\""));
        assert!(root_body.contains("id=\"settings-agent-resource-increment\""));
        assert!(root_body.contains("id=\"settings-agent-resource-decrement\""));
        assert!(root_body.contains("id=\"settings-agent-resource-save\""));
        assert!(root_body.contains("Worker limit"));
        assert!(!root_body.contains("id=\"mobile-agent-resource-save\""));
        assert!(!root_body.contains("Agent resources"));
        assert!(root_body.contains("Worker tasks"));
        assert!(root_body.contains("Tap a task to open its Worker conversation"));
        assert!(!root_body.contains("id=\"mobile-agent-master-card\""));
        assert!(!root_body.contains("id=\"mobile-agent-agent-list\""));
        assert!(!root_body.contains("id=\"mobile-agent-history-list\""));
        assert!(!root_body.contains("id=\"mobile-agent-control-list\""));
        assert!(!root_body.contains("Master evaluation"));
        assert!(root_body.contains("aria-modal=\"true\""));
        assert!(root_body.contains("Task and Agent Lifecycle"));
        assert!(root_body.contains("lifecycle observer"));
        assert!(root_body.contains("Add/update provider"));
        assert!(!root_body.contains("id=\"settings-agent-value\""));
        assert!(!root_body.contains("Task settings pending"));
        assert!(!root_body.contains("Active agent"));
        assert!(!root_body.contains("Sessions and workspace"));
        assert!(!root_body.contains("type=\"password\""));
        assert!(!root_body.contains("api-key"));
        assert!(!root_body.contains("rootfs"));
        assert!(!root_body.contains("shared-folder"));
        assert!(!root_body.contains("mount-directory"));
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
        let logo = client
            .get(format!("{}/assets/logo.png", server.base_url))
            .send()
            .await
            .expect("logo response");
        assert_eq!(logo.status(), StatusCode::OK);
        assert_eq!(logo.headers().get("content-type").unwrap(), "image/png");
        assert!(logo.bytes().await.expect("logo body").len() > 1024);
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
        assert!(webui_css_body.contains(".turn-cycle-card"));
        assert!(webui_css_body.contains(".turn-cycle-header"));
        assert!(webui_css_body.contains(".turn-cycle-header-pill"));
        assert!(webui_css_body.contains(".turn-cycle-card[data-live=\"true\"]"));
        assert!(webui_css_body.contains(".chat-message-user"));
        assert!(webui_css_body.contains(".chat-message-assistant"));
        assert!(webui_css_body.contains(".final-summary"));
        assert!(webui_css_body.contains(".final-summary-item"));
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
        assert!(webui_css_body.contains("body[data-mobile-drawer=\"settings\"] .inspector"));
        assert!(webui_css_body.contains(".inspector > .drawer-panel-head"));
        assert!(webui_css_body.contains("position: sticky"));
        assert!(webui_css_body.contains("top: calc(-14px - env(safe-area-inset-top))"));
        assert!(webui_css_body.contains(".mobile-drawer-scrim"));
        assert!(webui_css_body.contains(".settings-shell"));
        assert!(webui_css_body.contains(".settings-shell[hidden]"));
        assert!(webui_css_body.contains(".inspector-debug-panel[hidden]"));
        assert!(webui_css_body.contains(".settings-card"));
        assert!(webui_css_body.contains(".settings-provider-switch"));
        assert!(webui_css_body.contains(".settings-provider-registry"));
        assert!(webui_css_body.contains(".settings-provider-card"));
        assert!(webui_css_body.contains(".mobile-corner-button svg"));
        assert!(webui_css_body.contains(".mobile-home-dashboard"));
        assert!(webui_css_body.contains(".mobile-bottom-entries"));
        assert!(webui_css_body.contains(".settings-review-tree"));
        assert!(webui_css_body.contains(".settings-status-marker"));
        assert!(webui_css_body.contains(".timer-dashboard-dialog"));
        assert!(webui_css_body.contains(".timer-form-grid"));
        assert!(webui_css_body.contains(".timer-dashboard-list"));
        assert!(webui_css_body.contains(".timer-dashboard-history"));
        assert!(webui_css_body.contains(".timer-row"));
        assert!(webui_css_body.contains(".timer-event-row"));
        assert!(webui_css_body.contains("border: 2px solid var(--fail)"));
        assert!(webui_css_body.contains("border-color: var(--logo-green)"));
        assert!(webui_css_body.contains(".phase2-board-block"));
        assert!(webui_css_body.contains(".phase2-list"));
        assert!(webui_css_body.contains(".phase2-card"));
        assert!(webui_css_body.contains(".phase2-agent-card"));
        assert!(webui_css_body.contains(".phase2-agent-active"));
        assert!(webui_css_body.contains(".phase2-event"));
        assert!(webui_css_body.contains(".phase2-action-row"));
        assert!(webui_css_body.contains(".phase2-action.danger"));
        assert!(webui_css_body.contains(".mobile-agent-summary-strip"));
        assert!(webui_css_body.contains(".mobile-agent-sheet"));
        assert!(webui_css_body.contains(".mobile-agent-sheet-handle"));
        assert!(webui_css_body.contains(".session-relation-header"));
        assert!(webui_css_body.contains(".session-dashbar"));
        assert!(webui_css_body.contains(".session-tree-dropdown"));
        assert!(webui_css_body.contains("max-height: min(50vh, 440px)"));
        assert!(webui_css_body.contains(".session-tree-node.is-worker"));
        assert!(webui_css_body.contains(".turn-action-bar"));
        assert!(webui_css_body.contains(".turn-action-button"));
        assert!(webui_css_body.contains(".tool-field-grid"));
        assert!(webui_css_body.contains(".tool-raw-details"));
        assert!(webui_css_body.contains(".tool-chat-line-secondary.tool-chat-line-success"));
        assert!(webui_css_body.contains(".tool-chat-line-secondary.tool-chat-line-failed"));
        assert!(
            webui_css_body.contains("body[data-mobile-agent-sheet=\"open\"] .mobile-agent-sheet")
        );
        assert!(
            webui_css_body.contains("body[data-mobile-agent-sheet=\"open\"] .mobile-drawer-scrim")
        );
        assert!(webui_css_body.contains("--mobile-agent-blue: #1f5fbf"));
        assert!(webui_css_body.contains("--mobile-agent-green: #1f7a4d"));
        assert!(webui_css_body.contains("--mobile-agent-panel: #ffffff"));
        assert!(!webui_css_body.contains(".settings-readonly-action"));
        assert!(webui_css_body.contains(".session-with-workers"));
        assert!(webui_css_body.contains(".session-worker-children"));
        assert!(webui_css_body.contains(".session-item[data-session-kind=\"worker\"]"));
        assert!(webui_css_body.contains(".session-item[data-session-kind=\"task\"]"));
        assert!(webui_css_body.contains("env(safe-area-inset-bottom)"));
        assert!(!webui_css_body.contains(".work-context-tags"));
        assert!(!webui_css_body.contains(".context-tag"));
        assert!(webui_css_body.contains(".new-session-dialog"));
        assert!(webui_css_body.contains(".new-task-path-presets"));
        assert!(webui_css_body.contains(".path-preset-button"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_portrait\"][data-composer-focused=\"true\"] .composer-card"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_portrait\"][data-composer-focused=\"true\"] .composer-control-strip"));
        assert!(webui_css_body.contains("max-height: min(20svh, 158px)"));
        assert!(webui_css_body.contains("max-height: 132px"));
        assert!(
            webui_css_body.contains("padding-bottom: calc(112px + env(safe-area-inset-bottom))")
        );
        assert!(!webui_css_body.contains("padding-bottom: min(46svh, 330px)"));
        assert!(!webui_css_body.contains("inset 2px 0 0"));
        assert!(
            webui_css_body
                .contains("body[data-layout-shape=\"phone_portrait\"] .final-summary-item")
        );
        assert!(webui_css_body.contains("background: #eef3fb"));
        assert!(webui_css_body.contains("background: #edf6ef"));
        assert!(webui_css_body.contains("background: #f8ece9"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_portrait\"][data-composer-focused=\"true\"] .command-status"));
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_portrait\"] #send-button"));

        let js = client
            .get(format!("{}/assets/webui.js", server.base_url))
            .send()
            .await
            .expect("js response");
        assert_eq!(js.status(), StatusCode::OK);
        assert_eq!(
            js.headers().get("cache-control").unwrap(),
            "no-store, max-age=0"
        );
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
        assert!(js_body.contains("function markWebUiJavascriptReady"));
        assert!(js_body.contains("webuiJsReady"));
        assert!(js_body.contains("function viewportDimensionsForLayout"));
        assert!(js_body.contains("function setMobileDrawer"));
        assert!(js_body.contains("function handleBackNavigationIntent"));
        assert!(js_body.contains("function turnTimingProjection"));
        assert!(js_body.contains("function turnTimingLine"));
        assert!(js_body.contains("first response"));
        assert!(js_body.contains("total_elapsed_ms"));
        assert!(js_body.contains("turn-cycle-header-pill"));
        assert!(
            js_body.contains("window.__freehandHandleAndroidBack = handleBackNavigationIntent")
        );
        assert!(js_body.contains("function closeVisibleNavigationSurface"));
        assert!(js_body.contains("function showInspectorPanel"));
        assert!(js_body.contains("function renderSettingsShell"));
        assert!(js_body.contains("function topLevelPersistedSessions"));
        assert!(js_body.contains("function renderMobileHomeDashboard"));
        assert!(js_body.contains("function renderMobileHomeSessionList"));
        assert!(js_body.contains("phaseOneSettingsTree"));
        assert!(js_body.contains("function renderSettingsReviewTree"));
        assert!(js_body.contains("Add provider family"));
        assert!(js_body.contains("Provider detail"));
        assert!(js_body.contains("Model group"));
        assert!(js_body.contains("QueryTimerList"));
        assert!(js_body.contains("ScheduleTimer"));
        assert!(js_body.contains("CancelTimer"));
        assert!(js_body.contains("function renderTimerDashboard"));
        assert!(js_body.contains("function scheduleTimerFromForm"));
        assert!(js_body.contains("Built-in Tools entry is UI-only in Phase 1"));
        assert!(!js_body.contains("rootfs"));
        assert!(!js_body.contains("shared-folder"));
        assert!(!js_body.contains("mount-directory"));
        assert!(js_body.contains("QueryConfigStatus"));
        assert!(js_body.contains("QueryTaskBoard"));
        assert!(js_body.contains("QueryAgentBoard"));
        assert!(js_body.contains("QueryEventInbox"));
        assert!(js_body.contains("QueryTaskHistory"));
        assert!(js_body.contains("QueryWorkerControl"));
        assert!(js_body.contains("WorkerControl"));
        assert!(js_body.contains("function applyPhase2QueryResult"));
        assert!(js_body.contains("function refreshPhase2Status"));
        assert!(js_body.contains("function renderPhase2Dashboard"));
        assert!(js_body.contains("function renderTaskBoardProjection"));
        assert!(js_body.contains("function renderAgentBoardProjection"));
        assert!(js_body.contains("function phase2SortedAgents"));
        assert!(js_body.contains("function openWorkerTaskSession"));
        assert!(js_body.contains("function returnToParentSession"));
        assert!(js_body.contains("function renderWorkerSessionNavigation"));
        assert!(js_body.contains("function switchConversationSession"));
        assert!(js_body.contains("sessionRefreshInFlight"));
        assert!(js_body.contains("function selectedSessionIsLoading"));
        assert!(js_body.contains("function loadingConversationBubble"));
        assert!(js_body.contains("function workerTranscriptWaitingBubble"));
        assert!(js_body.contains("function workerTranscriptRetryContext"));
        assert!(js_body.contains("function selectedWorkerTranscriptRefreshRetryable"));
        assert!(js_body.contains("function scheduleSessionRefreshRetry"));
        assert!(js_body.contains("Loading selected session transcript from runtime truth."));
        assert!(js_body.contains("Worker transcript is not persisted yet; TaskBoard still shows this Worker task as active."));
        assert!(js_body.contains("Refreshing the same owner-projected Worker session; this is not a task dispatch failure."));
        assert!(js_body.contains("const workerTranscriptRefreshRetryDelayMs = 3000"));
        assert!(js_body.contains("function renderSessionRefreshFailure"));
        assert!(js_body.contains("clearConversationForSessionSwitch"));
        assert!(js_body.contains("const adpRequestTimeoutMs = 45000"));
        assert!(js_body.contains("state.adpFailure = message"));
        assert!(js_body.contains("const requestedSessionId = state.selectedSessionId"));
        assert!(js_body.contains("state.selectedSessionId !== requestedSessionId"));
        assert!(js_body.contains("projection.session_id !== state.selectedSessionId"));
        assert!(js_body.contains("waiting lifecycle"));
        assert!(js_body.contains("function renderEventInboxProjection"));
        assert!(js_body.contains("function renderTaskHistoryProjection"));
        assert!(js_body.contains("function renderWorkerControlProjection"));
        assert!(js_body.contains("function sendWorkerControl"));
        assert!(js_body.contains("function buildMobileAgentDashboardModel"));
        assert!(js_body.contains("function currentSessionTasks"));
        assert!(js_body.contains("function currentSessionAgents"));
        assert!(js_body.contains("function currentSessionEvents"));
        assert!(js_body.contains("function currentSessionTaskStatusLabel"));
        assert!(js_body.contains("function taskVisibleInSession"));
        assert!(js_body.contains("function workerOrdinalFromAgentId"));
        assert!(js_body.contains("normalizeAgentId(agent.agent_id) !== \"master\""));
        assert!(js_body.contains("task.parent_session_id === sessionId"));
        assert!(js_body.contains("task.attached_session_ids.includes(sessionId)"));
        assert!(js_body.contains("currentSessionTaskStatusLabel(tasks)"));
        assert!(js_body.contains("currentSessionEvents()"));
        assert!(js_body.contains("currentSessionAgents()"));
        assert!(js_body.contains("state.selectedSessionId !== nextSessionId"));
        assert!(js_body.contains("state.taskHistory = null"));
        assert!(js_body.contains("state.workerControl = null"));
        assert!(!js_body.contains("parentTasks.length > 0 ? parentTasks : allTasks"));
        assert!(!js_body.contains("(taskBoard && taskBoard.tasks || []).length"));
        assert!(js_body.contains("function renderMobileAgentSummaryStrip"));
        assert!(js_body.contains("function renderMobileAgentSheet"));
        assert!(js_body.contains("function setMobileAgentSheetOpen"));
        assert!(js_body.contains("function renderSessionRelationHeader"));
        assert!(js_body.contains("function renderSessionTree"));
        assert!(js_body.contains("state.sessionTreeOpen"));
        assert!(js_body.contains("sessionTreeDropdown.hidden = !state.sessionTreeOpen"));
        assert!(js_body.contains("node.dataset.relationSchema"));
        assert!(js_body.contains("TaskBoard.worker_session_id"));
        assert!(js_body.contains("node.dataset.sessionId"));
        assert!(js_body.contains("node.dataset.taskId"));
        assert!(js_body.contains("state.mobileAgentSheetOpen"));
        assert!(js_body.contains("runningAgents.length"));
        assert!(js_body.contains("mobileAgentLifecycleSummary(counts)"));
        assert!(js_body.contains("running task"));
        assert!(js_body.contains("blocked task"));
        assert!(!js_body.contains("delegated task"));
        assert!(js_body.contains("limit ${workerLimit}"));
        assert!(js_body.contains("UpdateAgentResourceConfig"));
        assert!(js_body.contains("function submitAgentResourceConfigUpdate"));
        assert!(js_body.contains("function renderSystemAgentResourceConfig"));
        assert!(js_body.contains("Save Worker limit"));
        assert!(js_body.contains("agent_resource_config_saved_restart_required:count="));
        assert!(js_body.contains("restart and Worker process startup required"));
        assert!(!js_body.contains("freehand-webui-agent-resource-count"));
        assert!(!js_body.contains("mobileAgentResourceSave"));
        assert!(js_body.contains("No Worker task in this session"));
        assert!(!js_body.contains("Awaiting Master evaluation"));
        assert!(!js_body.contains("Master evaluating"));
        assert!(!js_body.contains("Rework required"));
        assert!(!js_body.contains("Goal complete"));
        assert!(js_body.contains("data-worker-control-op"));
        assert!(js_body.contains("state.taskBoard"));
        assert!(js_body.contains("state.agentBoard"));
        assert!(js_body.contains("state.eventInbox"));
        assert!(js_body.contains("state.taskHistory"));
        assert!(js_body.contains("state.workerControl"));
        assert!(js_body.contains("function commandReceiptStatus(receipt)"));
        assert!(js_body.contains("function commandReceiptCode(dispatchStatus)"));
        assert!(js_body.contains("unsupported command receipt"));
        assert!(!js_body.contains("function commandReceiptStatus(receipt, fallback"));
        assert!(!js_body.contains("return fallback"));
        assert!(!js_body.contains("request processed"));
        assert!(!js_body.contains("status.includes(\"task_\")"));
        assert!(!js_body.contains("status.includes(\"worker_control\")"));
        assert!(!js_body.contains("status.includes(\"reason_turn_started\")"));
        assert!(!js_body.contains("freehand-webui-task-board"));
        assert!(!js_body.contains("freehand-webui-worker-control"));
        assert!(!js_body.contains("freehand-webui-mobile-agent-dashboard"));
        assert!(!js_body.contains("aggregate worker results"));
        assert!(!js_body.contains("allTasksClosed ? \"Goal complete\""));
        assert!(js_body.contains("refreshConfigStatus"));
        assert!(js_body.contains("variantPayload(result, \"ConfigStatus\")"));
        assert!(js_body.contains("state.configStatus"));
        assert!(js_body.contains("settings-provider-host"));
        assert!(js_body.contains("settings-provider-auth"));
        assert!(js_body.contains("settings-provider-current-select"));
        assert!(js_body.contains("settings-provider-fallback-select"));
        assert!(js_body.contains("settings-provider-registry-list"));
        assert!(js_body.contains("providerSelectionDraft"));
        assert!(js_body.contains("function updateProviderSelectionDraftFromControls"));
        assert!(!js_body.contains("settingsProviderFallbackSelect?.dataset.optionsInitialized"));
        assert!(js_body.contains("settings-config-error"));
        assert!(js_body.contains("UpsertProviderConfig"));
        assert!(js_body.contains("UpdateAgentProviderSelection"));
        assert!(js_body.contains("function submitProviderConfigUpdate"));
        assert!(js_body.contains("function submitProviderSelectionUpdate"));
        assert!(js_body.contains("function requestAndroidApkUpdateCheck"));
        assert!(js_body.contains("function receiveAndroidApkUpdateStatus"));
        assert!(js_body.contains("function optionalAndroidApkUpdateNumber"));
        assert!(js_body.contains("\"already_checking\""));
        assert!(js_body.contains("window.__freehandAndroidApkUpdateStatus"));
        assert!(js_body.contains("window.FreehandAndroidApkUpdate"));
        assert!(js_body.contains("bridge.check();"));
        assert!(js_body.contains("settings-apk-update-check-button"));
        assert!(js_body.contains("settings-apk-update-status"));
        assert!(
            js_body.contains("APK update check is available only inside the Freehand Android app.")
        );
        assert!(js_body.contains("function providerConfigReceiptStatus"));
        assert!(js_body.contains("function providerConfigUpsertReceiptStatus"));
        assert!(js_body.contains("function providerSelectionReceiptStatus"));
        assert!(js_body.contains("provider_config_upserted_restart_required"));
        assert!(js_body.contains("agent_provider_selection_saved_restart_required"));
        assert!(js_body.contains("Provider config saved. Restart required."));
        assert!(js_body.contains("Config save returned an unexpected service status."));
        assert!(!js_body.contains("return \"Provider config saved.\""));
        assert!(!js_body.contains("`${receipt.dispatch_status} -> ${receipt.target_feature_id}`"));
        assert!(js_body.contains("function settingsAuthTypeLabel"));
        assert!(js_body.contains("authType === \"apikey\" ? \"credential\""));
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
        assert!(!js_body.contains("worker-context-tag"));
        assert!(!js_body.contains("task-context-tag"));
        assert!(!js_body.contains("transport-context-tag"));
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
        let start_new_conversation_pos = js_body
            .find("async function startNewConversation()")
            .expect("new conversation flow exists");
        let start_new_task_pos = js_body
            .find("async function startNewTask")
            .expect("new task flow exists");
        let start_new_conversation_body = &js_body[start_new_conversation_pos..start_new_task_pos];
        assert!(start_new_conversation_body.contains("CreateSession"));
        assert!(start_new_conversation_body.contains("title: \"New conversation\""));
        assert!(start_new_conversation_body.contains("await refreshSessions();"));
        assert!(start_new_conversation_body.contains("await refreshSelectedSession();"));
        assert!(js_body.contains("selectedSessionIds"));
        assert!(js_body.contains("function workerChildSessionsForParent"));
        assert!(js_body.contains("parent_session_id"));
        assert!(js_body.contains("task.worker_session_id"));
        assert!(js_body.contains("card.dataset.taskId"));
        assert!(js_body.contains("card.dataset.workerSessionId"));
        assert!(!js_body.contains("model.tasks.slice(0, 8)"));
        assert!(js_body.contains("worker session unavailable in TaskBoard projection"));
        assert!(js_body.contains("session refresh failed"));
        assert!(js_body.contains("function renderSessionWithWorkerChildren"));
        assert!(js_body.contains("function renderSessionAgentGroup"));
        assert!(js_body.contains("group.className = \"session-agent-group\""));
        assert!(js_body.contains("sessionNodes.className = \"session-agent-sessions\""));
        assert!(js_body.contains("function renderSessionItem"));
        assert!(!js_body.contains("return `worker-task-${sanitizeRuntimeIdentifier(taskId)}`"));
        assert!(js_body.contains("session.temporary"));
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
        assert!(js_body.contains("function appendTurnActionBar"));
        assert!(js_body.contains("function editAndRerunFromTurn"));
        assert!(js_body.contains("function rollbackEffectiveTranscriptThroughTurn"));
        assert!(js_body.contains("function startNewConversationFromText"));
        assert!(js_body.contains("function renderToolSection"));
        assert!(js_body.contains("function renderFinalSummary"));
        assert!(js_body.contains("function finalSummaryBlocks"));
        assert!(js_body.contains("function normalizeFinalSummaryLine"));
        assert!(!js_body.contains("function splitFinalSummaryInlineStructure"));
        assert!(!js_body.contains("function inlineStructureIndexes"));
        assert!(!js_body.contains("function collectInlineStructureIndexes"));
        assert!(js_body.contains("function toolSemanticLines"));
        assert!(js_body.contains("function buildConversationRenderModel"));
        assert!(js_body.contains("function buildRenderTurn"));
        assert!(js_body.contains("function buildRenderRows"));
        assert!(js_body.contains("function buildToolActivityRenderRow"));
        assert!(js_body.contains("function buildModelRequestRenderRow"));
        assert!(js_body.contains("function conversationTimelineItems"));
        assert!(js_body.contains("function timelineItemChatCards"));
        assert!(js_body.contains("function timelineItemCycleCard"));
        assert!(js_body.contains("function renderConversationFragments"));
        assert!(js_body.contains("function reconcileCycleCardFragments"));
        assert!(js_body.contains("function frozenCycleCardNeedsAuthoritativeMetadataRefresh"));
        assert!(js_body.contains("existing.dataset.frozen === \"true\""));
        assert!(
            js_body
                .contains("!frozenCycleCardNeedsAuthoritativeMetadataRefresh(existing, nextCard)")
        );
        assert!(
            js_body
                .contains("!existing.dataset.totalElapsedMs && !!nextCard.dataset.totalElapsedMs")
        );
        assert!(js_body.contains("function cycleCardFromChatCards"));
        assert!(js_body.contains("function cycleCardKey"));
        assert!(js_body.contains("function cycleCardKeyFromNode"));
        assert!(js_body.contains("function latestClosedTurnForSession"));
        assert!(js_body.contains("function closedTurnObsoletesSessionSummary"));
        assert!(js_body.contains("closedTurnObsoletesSessionSummary(latestClosedTurn, summary)"));
        assert!(js_body.contains("function sessionLiveObservation"));
        assert!(js_body.contains("function globalLiveSessionObservation"));
        assert!(
            js_body.contains("return parent ? { ...parent, scope: \"parent Master\" } : null;")
        );
        assert!(js_body.contains("function liveObservationLine"));
        assert!(js_body.contains("function cycleCardMetaForTimelineItem"));
        assert!(js_body.contains("function cycleCardIsTerminal"));
        assert!(js_body.contains("function pendingSubmitTimelineIndex"));
        assert!(js_body.contains("function acceptedSubmitReceiptTimelineIndex"));
        assert!(js_body.contains("function firstCycleTurnIndex"));
        assert!(js_body.contains("function renderTurnCreatedAtMs"));
        assert!(js_body.contains("submitId: turn.submit_id || \"\""));
        assert!(js_body.contains("startedAt: pendingStartedAt"));
        assert!(js_body.contains(
            "sessionId: state.pendingSubmitSessionId || state.selectedSessionId || \"\""
        ));
        assert!(js_body.contains("submitId: state.pendingSubmitId || \"\""));
        assert!(js_body.contains("function modelRequestStaticStatus"));
        assert!(js_body.contains("function buildObservableLiveTurnRenderRow"));
        assert!(js_body.contains("function inactiveToolLifecycleForRender"));
        assert!(js_body.contains("function isToolPendingStatus"));
        assert!(js_body.contains("function terminalTurnStatusLabel"));
        assert!(js_body.contains("isToolPendingStatus(turn.terminal_status)"));
        assert!(js_body.contains("function turnRequiresLifecycleTruthRefresh"));
        assert!(
            js_body.contains("turnRequiresLifecycleTruthRefresh(activeTurnForSelectedSession())")
        );
        assert!(js_body.contains("title: isToolPending ? \"Lifecycle\" : \"Final\""));
        assert!(js_body.contains("? terminalTurnStatusLabel(turn.terminal_status)"));
        assert!(!js_body.contains("turn.terminal_text\n    ? \"completed\""));
        assert!(js_body.contains("phase: \"tool_failed\""));
        assert!(js_body.contains("phase: \"tool_completed\""));
        assert!(
            js_body.contains("const inactiveToolLifecycle = inactiveToolLifecycleForRender(turn);")
        );
        assert!(js_body.contains("request accepted; waiting for protocol-visible turn details"));
        assert!(js_body.contains("function pendingUserInputIsMaterialized"));
        assert!(js_body.contains("function clearPendingUserInputIfMaterialized"));
        assert!(js_body.contains("function acceptedSubmitReceiptChatCards"));
        assert!(js_body.contains("function acceptedSubmitReceiptForRender"));
        assert!(js_body.contains("Service accepted this request through TaskBoard truth."));
        assert!(js_body.contains("clearPendingUserInputIfMaterialized();"));
        assert!(js_body.contains("async function refreshAfterAmbiguousSubmitFailure"));
        assert!(js_body.contains("await refreshAfterAmbiguousSubmitFailure(error)"));
        assert!(js_body.contains("request is visible after service refresh"));
        assert!(js_body.contains("submit receipt not verified after service refresh"));
        assert!(js_body.contains("function installWebUiTestHooks"));
        assert!(js_body.contains("__freehandEnableTestHooks"));
        assert!(js_body.contains("captureAmbiguousSubmitState"));
        assert!(js_body.contains("renderAll()"));
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
        assert!(js_body.contains("const leftSessionId = `${left.session_id || \"\"}`.trim();"));
        assert!(js_body.contains("const rightSessionId = `${right.session_id || \"\"}`.trim();"));
        assert!(js_body.contains("return !!leftSessionId && leftSessionId === rightSessionId;"));
        assert!(!js_body.contains("existing.turn_id === state.turn.turn_id"));
        assert!(js_body.contains("function renderModelHasLiveLifecycle"));
        assert!(js_body.contains("function turnIsCurrentLiveTurn"));
        assert!(js_body.contains("conversationTurns.length === 0 && state.turn"));
        assert!(
            !js_body
                .contains("conversationTurns.length === 0 && turnIsCurrentLiveTurn(state.turn)")
        );
        assert!(js_body.contains("conversationTimelineItems(renderModel).forEach"));
        assert!(js_body.contains("fragments.push(timelineItemCycleCard(item))"));
        assert!(!js_body.contains("fragments.push(...timelineItemChatCards(item))"));
        assert!(js_body.contains("return turnChatCards(item.renderTurn);"));
        assert!(js_body.contains("return pendingChatCards(item.pendingSubmit);"));
        assert!(js_body.contains("article.className = `turn-cycle-card"));
        assert!(js_body.contains("article.dataset.cycleKey = cycleCardKey(meta)"));
        assert!(js_body.contains("article.dataset.cycleKind = kind"));
        assert!(js_body.contains("article.dataset.sessionId"));
        assert!(js_body.contains("article.dataset.submitId"));
        assert!(js_body.contains("article.dataset.terminal = \"true\""));
        assert!(js_body.contains("article.dataset.frozen = \"true\""));
        assert!(js_body.contains("if (kind === \"turn\" && turnId)"));
        assert!(js_body.contains("return `turn:${sessionId}:${turnId}`"));
        assert!(js_body.contains("sessionRelationHeader.dataset.liveSessionId"));
        assert!(js_body.contains("sessionRelationHeader.dataset.liveTurnId"));
        assert!(js_body.contains("active · ${observation.label}"));
        assert!(js_body.contains("existing.dataset.frozen === \"true\""));
        assert!(js_body.contains("return `submit:${sessionId}:${submitId}`"));
        assert!(js_body.contains("state.renderedCycleSessionId"));
        assert!(!js_body.contains("messageList.replaceChildren();"));
        assert!(!js_body.contains("function uniqueChatFragments"));
        assert!(!js_body.contains("uniqueChatFragments(fragments).forEach"));
        assert!(js_body.contains("article.dataset.turnId"));
        assert!(!js_body.contains("previousAssistantText"));
        assert!(
            !js_body.contains("fragments.push(...pendingChatCards(renderModel.pendingSubmit))")
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
        assert!(js_body.contains("userScrollLocked"));
        assert!(js_body.contains("syncUserScrollLock"));
        assert!(js_body.contains("scrollHostForConversation"));
        assert!(js_body.contains("updateComposerClearance"));
        assert!(js_body.contains("--composer-clearance"));
        assert!(js_body.contains("forceScrollToBottom"));
        assert!(js_body.contains("const streamStage = document.querySelector(\".stream-stage\")"));
        assert!(js_body.contains("host.scrollTop = host.scrollHeight"));
        assert!(!js_body.contains("scrollIntoView"));
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
        assert!(!js_body.contains("function successorBaseTurnIds"));
        assert!(!js_body.contains("successorBaseTurns"));
        assert!(!js_body.contains("hideTerminal"));
        assert!(!js_body.contains("hideUser"));
        assert!(js_body.contains("case \"/new\""));
        assert!(js_body.contains("case \"/task\""));
        assert!(js_body.contains("case \"/cwd\""));
        assert!(!js_body.contains("selected session:"));
        assert!(js_body.contains("CancelTurn"));
        assert!(js_body.contains("CancelLatestActiveTurn"));
        assert!(js_body.contains("event.key !== \"Escape\""));
        assert!(js_body.contains("cancelActiveTurn"));
        assert!(js_body.contains("normalizePublicConversation"));
        assert!(!js_body.contains("const toolIndex = new Map()"));
        assert!(!js_body.contains("normalized[existingIndex] = item"));
        assert!(!js_body.contains("function compactToolBodyLines"));
        assert!(!js_body.contains("function isGenericToolResult"));
        assert!(!js_body.contains("function shouldShowToolParameterSummary"));
        assert!(!js_body.contains("function compactToolDisplayFields"));
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
        assert!(!js_body.contains(
            "left.session_id && right.session_id && left.session_id !== right.session_id"
        ));
        assert!(!js_body.contains("normalizeVisibleText(left.user_text)"));
        assert!(!js_body.contains("normalizeVisibleText(right.user_text)"));
        assert!(js_body.contains("terminalBodyForDisplay"));
        assert!(js_body.contains("terminalSummaryBlock"));
        assert!(!js_body.contains("function terminalSummaryLine"));
        assert!(js_body.contains("stripDebugTerminalLines"));
        assert!(js_body.contains("debugDetailsVisible"));
        assert!(js_body.contains("debugDetailsToggle"));
        assert!(js_body.contains("Debug off"));
        assert!(js_body.contains("<freehand_completion>"));
        assert!(js_body.contains("toolSummaryBody"));
        assert!(js_body.contains("renderToolBody"));
        assert!(js_body.contains("pushToolLine"));
        assert!(js_body.contains("splitToolDetailLines"));
        assert!(js_body.contains("display.parameter_summary"));
        assert!(js_body.contains("display.result_summary"));
        assert!(js_body.contains("elapsedSince"));
        assert!(js_body.contains("submitStartedAt"));
        assert!(js_body.contains("pendingSubmitId"));
        assert!(js_body.contains("pendingSubmitSessionId"));
        assert!(js_body.contains("pendingSubmitError"));
        assert!(js_body.contains("turn.submit_id !== submitId"));
        assert!(js_body.contains("turn.session_id !== submitSessionId"));
        assert!(js_body.contains("modelRequestTimingKey"));
        assert!(js_body.contains("modelRequestKind"));
        assert!(js_body.contains("function modelRequestPhase"));
        assert!(js_body.contains("phase: modelRequestPhase(turn)"));
        assert!(js_body.contains("modelRequestLabel"));
        assert!(js_body.contains("function modelRequestTransport"));
        assert!(js_body.contains("function modelRequestTransportPhase"));
        assert!(js_body.contains("function modelRequestTransportLabel"));
        assert!(!js_body.contains("function modelRequestKindIsLegacyTransport"));
        assert!(js_body.contains("turnIsWaitingForModelResponse"));
        assert!(js_body.contains("schema polishing"));
        assert!(js_body.contains("thinking after tool result"));
        assert!(js_body.contains("transport retry"));
        assert!(js_body.contains("transport switch"));
        assert!(js_body.contains("return \"provider_retry\";"));
        assert!(js_body.contains("return \"provider_failover\";"));
        assert!(!js_body.contains("return \"provider retry\";"));
        assert!(!js_body.contains("return \"provider failover\";"));
        assert!(!js_body.contains("label.startsWith(\"provider \")"));
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
        assert!(!js_body.contains("compactToolResultLine"));
        assert!(!js_body.contains("succeeded: result returned"));
        assert!(!js_body.contains("succeeded: shell command"));
        assert!(js_body.contains("buildConversationRenderModel()"));
        assert!(js_body.contains("renderModelHasLiveLifecycle()"));
        assert!(!js_body.contains("hasPendingSubmit"));
        assert!(!js_body.contains("hasModelRequestWait"));
        assert!(!js_body.contains("hasModelWait"));
        assert!(js_body.contains("waitingToolStatus"));
        assert!(js_body.contains("tool.display || null"));
        assert!(js_body.contains("display.diff"));
        assert!(js_body.contains("display.result_summary"));
        assert!(js_body.contains("compact-tool-state"));
        assert!(js_body.contains("display.fields"));
        assert!(js_body.contains("/^(result|failure):/i.test(line)"));
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
        assert!(js_body.contains("definitely-missing-freehand-task"));
        assert!(js_body.contains(r#""op":"query""#));
        assert!(!js_body.contains("read_file tool exactly"));
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
        assert!(js_body.contains("textWithAttachmentDisplay"));
        assert!(js_body.contains("attachmentsForSubmit"));
        assert!(js_body.contains("FreehandAndroidNotifications"));
        assert!(js_body.contains("androidObservedNonTerminalTurns"));
        assert!(js_body.contains("data_base64"));
        assert!(js_body.contains("attachment-preview-overlay"));
        assert!(js_body.contains("clearCurrentAttachments"));
        assert!(js_body.contains(
            "submit receipt not verified after service refresh; checking service truth before duplicate send"
        ));
        assert!(js_body.contains("Submit receipt is being verified against service truth."));
        assert!(js_body.contains("Do not send a duplicate until the service refresh finishes."));
        assert!(js_body.contains("if (!state.selectedSessionId)"));
        assert!(js_body.contains("state.draftSessionId = sessionId"));
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
        assert!(js_body.contains("foregroundRefreshMinIntervalMs"));
        assert!(js_body.contains("function refreshProtocolStateAfterForeground"));
        assert!(js_body.contains("function shouldRefreshAfterForeground"));
        assert!(js_body.contains("window.addEventListener(\"pageshow\""));
        assert!(js_body.contains("window.addEventListener(\"focus\""));
        assert!(js_body.contains("window.addEventListener(\"online\""));
        assert!(js_body.contains("document.addEventListener(\"visibilitychange\""));
        assert!(js_body.contains("refreshProtocolStateAfterForeground(\"app resume\")"));
        assert!(js_body.contains("checking service truth after"));
        assert!(js_body.contains("foregroundRefreshInFlight"));
        assert!(js_body.contains("foregroundRefreshLastAt"));
        assert!(js_body.contains("if (command.startsWith(\"/\")"));
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
    async fn android_update_routes_return_manifest_and_explicit_missing_apk() {
        let _env_guard = android_update_env_lock().lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture(&[
            "FREEHAND_ANDROID_VERSION_CODE",
            "FREEHAND_ANDROID_VERSION_NAME",
            "FREEHAND_ANDROID_APK_PATH",
            "FREEHAND_ANDROID_UPDATE_MANIFEST_PATH",
        ]);
        unsafe {
            std::env::set_var("FREEHAND_ANDROID_VERSION_CODE", "42");
            std::env::set_var("FREEHAND_ANDROID_VERSION_NAME", "0.4.2");
            std::env::set_var(
                "FREEHAND_ANDROID_APK_PATH",
                "/tmp/freehand-missing-test-android.apk",
            );
            std::env::remove_var("FREEHAND_ANDROID_UPDATE_MANIFEST_PATH");
        }

        let server = TestServer::spawn_empty().await;
        let client = Client::builder().build().expect("client");

        let manifest = client
            .get(format!("{}/android/update.json", server.base_url))
            .send()
            .await
            .expect("manifest response");
        assert_eq!(manifest.status(), StatusCode::OK);
        assert_eq!(
            manifest.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-store, max-age=0"))
        );
        let manifest_json: serde_json::Value = manifest.json().await.expect("manifest json");
        assert_eq!(manifest_json["versionCode"], 42);
        assert_eq!(manifest_json["versionName"], "0.4.2");
        assert_eq!(
            manifest_json["apkUrl"],
            serde_json::Value::String("/android/freehand-android.apk".to_owned())
        );

        let apk = client
            .get(format!("{}/android/freehand-android.apk", server.base_url))
            .send()
            .await
            .expect("apk response");
        assert_eq!(apk.status(), StatusCode::NOT_FOUND);

        server.stop().await;
    }

    #[tokio::test]
    async fn android_update_manifest_uses_compiled_sidecar_without_env_override() {
        let _env_guard = android_update_env_lock().lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture(&[
            "FREEHAND_ANDROID_VERSION_CODE",
            "FREEHAND_ANDROID_VERSION_NAME",
            "FREEHAND_ANDROID_APK_PATH",
            "FREEHAND_ANDROID_UPDATE_MANIFEST_PATH",
        ]);
        let temp_dir = std::env::temp_dir().join(format!(
            "freehand-android-update-sidecar-{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let manifest_path = temp_dir.join("update.json");
        fs::write(
            &manifest_path,
            r#"{"versionCode":7,"versionName":"0.7.0","apkUrl":"/android/freehand-android.apk","releaseNotes":"sidecar","required":false}"#,
        )
        .expect("write sidecar");
        unsafe {
            std::env::remove_var("FREEHAND_ANDROID_VERSION_CODE");
            std::env::remove_var("FREEHAND_ANDROID_VERSION_NAME");
            std::env::set_var("FREEHAND_ANDROID_UPDATE_MANIFEST_PATH", &manifest_path);
            std::env::set_var(
                "FREEHAND_ANDROID_APK_PATH",
                "/tmp/freehand-missing-test-android.apk",
            );
        }

        let server = TestServer::spawn_empty().await;
        let client = Client::builder().build().expect("client");
        let manifest = client
            .get(format!("{}/android/update.json", server.base_url))
            .send()
            .await
            .expect("manifest response");
        assert_eq!(manifest.status(), StatusCode::OK);
        assert_eq!(
            manifest.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-store, max-age=0"))
        );
        let manifest_json: serde_json::Value = manifest.json().await.expect("manifest json");
        assert_eq!(manifest_json["versionCode"], 7);
        assert_eq!(manifest_json["versionName"], "0.7.0");
        assert_eq!(manifest_json["releaseNotes"], "sidecar");

        server.stop().await;
        fs::remove_dir_all(&temp_dir).expect("remove temp dir");
    }

    #[tokio::test]
    async fn android_update_manifest_missing_sidecar_does_not_report_false_current_version() {
        let _env_guard = android_update_env_lock().lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture(&[
            "FREEHAND_ANDROID_VERSION_CODE",
            "FREEHAND_ANDROID_VERSION_NAME",
            "FREEHAND_ANDROID_APK_PATH",
            "FREEHAND_ANDROID_UPDATE_MANIFEST_PATH",
        ]);
        let missing_path = std::env::temp_dir().join(format!(
            "freehand-missing-update-sidecar-{}-update.json",
            std::process::id()
        ));
        unsafe {
            std::env::remove_var("FREEHAND_ANDROID_VERSION_CODE");
            std::env::remove_var("FREEHAND_ANDROID_VERSION_NAME");
            std::env::set_var("FREEHAND_ANDROID_UPDATE_MANIFEST_PATH", &missing_path);
        }

        let server = TestServer::spawn_empty().await;
        let client = Client::builder().build().expect("client");
        let manifest = client
            .get(format!("{}/android/update.json", server.base_url))
            .send()
            .await
            .expect("manifest response");
        assert_eq!(manifest.status(), StatusCode::NOT_FOUND);

        server.stop().await;
    }

    #[test]
    fn android_update_default_paths_use_runtime_home_not_process_cwd() {
        let _env_guard = android_update_env_lock().lock().expect("env lock");
        let _snapshot = EnvSnapshot::capture(&[
            "FREEHAND_ANDROID_VERSION_CODE",
            "FREEHAND_ANDROID_VERSION_NAME",
            "FREEHAND_ANDROID_APK_PATH",
            "FREEHAND_ANDROID_UPDATE_MANIFEST_PATH",
            "FREEHAND_RUNTIME_HOME",
            "FREEHAND_DAEMON_WORKDIR",
        ]);
        let runtime_home = std::env::temp_dir().join(format!(
            "freehand-android-update-runtime-home-{}",
            std::process::id()
        ));
        unsafe {
            std::env::remove_var("FREEHAND_ANDROID_APK_PATH");
            std::env::remove_var("FREEHAND_ANDROID_UPDATE_MANIFEST_PATH");
            std::env::set_var("FREEHAND_RUNTIME_HOME", &runtime_home);
            std::env::remove_var("FREEHAND_DAEMON_WORKDIR");
        }

        assert_eq!(
            android_update_manifest_path(),
            runtime_home
                .join("dist")
                .join("android")
                .join("update.json")
        );
        assert_eq!(
            android_update_apk_path(),
            runtime_home
                .join("dist")
                .join("android")
                .join("freehand-android-release.apk")
        );
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
                created_at: Some(20),
                timing: None,
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
                created_at: Some(30),
                timing: None,
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
                created_at: Some(40),
                timing: None,
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
        assert!(completed_body.contains("path=src/lib.rs"));
        assert!(completed_body.contains("visible result body"));
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
                metadata: None,
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
                metadata: None,
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
                metadata: None,
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
