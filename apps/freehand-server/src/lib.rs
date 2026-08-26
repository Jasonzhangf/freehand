mod assets;
mod page;

use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::io::Read;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Html;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::response::sse::{Event, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use freehand_contracts::{
    AgentId, FREEHAND_REMOTE_ACCESS_SCOPE_HEADER, FREEHAND_REMOTE_ACCESS_SCOPE_VALUE, FeatureId,
    ReasonResp01SemanticEvent, ReasonResp03TerminalEvent, SemanticEventKind, SessionId,
    TerminalStatus, TraceId, TurnId,
};
use freehand_ui_protocol::{
    DebugScenePosition, DebugSemanticPosition, DebugStateSnapshot, SubscriptionSelector,
    TurnProjectionInput, UiAdpFailure, UiAdpRequest, UiAdpResponse, UiCheckpointSnapshot,
    UiClientKind, UiCommand, UiCommandDispatchFailure, UiCommandDispatchPort,
    UiCommandDispatchReceipt, UiProjection, UiProtocolState, UiPublicTurnProjection,
    UiQueryAccessScope, UiQueryResult, UiRuntimeQueryPort, UiSubscriptionEvent, UiTurnProjection,
    accept_query_ingress, adp_internal_command_token_from_capability, adp_server_capabilities,
    build_command_dispatch_envelope, checkpoint_projection_from_runtime_summary,
    dispatch_port_failure, is_internal_adp_command, is_public_adp_command, protocol_rejection,
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
    internal_adp_command_token: Option<String>,
    adp_auth_token: String,
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
    build_webui_router_with_internal_adp_command_token(
        protocol_state,
        command_dispatch_port,
        runtime_query_port,
        std::env::var("FREEHAND_ADP_INTERNAL_COMMAND_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty()),
        load_adp_auth_token(),
    )
}

fn build_webui_router_with_internal_adp_command_token(
    protocol_state: Arc<Mutex<UiProtocolState>>,
    command_dispatch_port: Arc<dyn UiCommandDispatchPort>,
    runtime_query_port: Arc<dyn UiRuntimeQueryPort>,
    internal_adp_command_token: Option<String>,
    adp_auth_token: String,
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
            internal_adp_command_token,
            adp_auth_token,
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

const FREEHAND_ADP_AUTH_TOKEN_ENV: &str = "FREEHAND_ADP_AUTH_TOKEN";
const FREEHAND_ADP_AUTH_COOKIE: &str = "freehand_adp_auth";

fn load_adp_auth_token() -> String {
    if let Ok(token) = std::env::var(FREEHAND_ADP_AUTH_TOKEN_ENV) {
        let token = token.trim().to_owned();
        if !token.is_empty() {
            return token;
        }
    }
    generate_adp_auth_token().expect("failed to generate ADP auth token from OS entropy")
}

fn generate_adp_auth_token() -> std::io::Result<String> {
    let mut bytes = [0_u8; 32];
    let mut file = fs::File::open("/dev/urandom")?;
    file.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn adp_auth_cookie_header(token: &str) -> String {
    format!("{FREEHAND_ADP_AUTH_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict")
}

fn adp_request_authorized(headers: &HeaderMap, expected_token: &str) -> bool {
    adp_bearer_token(headers).is_some_and(|token| token == expected_token)
        || adp_cookie_token(headers).is_some_and(|token| token == expected_token)
}

fn adp_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.trim().is_empty())
}

fn adp_cookie_token(headers: &HeaderMap) -> Option<&str> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == FREEHAND_ADP_AUTH_COOKIE && !value.trim().is_empty()).then_some(value)
    })
}

async fn handle_root(
    State(state): State<WebUiState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    (
        [
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-store, max-age=0"),
            ),
            (
                header::SET_COOKIE,
                HeaderValue::from_str(&adp_auth_cookie_header(&state.adp_auth_token))
                    .expect("ADP auth token must be cookie-safe hex"),
            ),
        ],
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
    if !is_public_adp_command(&command) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(UiCommandDispatchFailure {
                code: "adp_command_not_public".to_owned(),
                message: "command frame is not exposed on the public command surface".to_owned(),
                retryable: false,
            }),
        ));
    }
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
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<WebUiState>,
) -> impl IntoResponse {
    if !adp_request_authorized(&headers, &state.adp_auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let scope = query_access_scope(&headers);
    ws.on_upgrade(move |socket| handle_adp_connection(socket, state, scope))
        .into_response()
}

fn query_access_scope(headers: &HeaderMap) -> UiQueryAccessScope {
    if headers
        .get(FREEHAND_REMOTE_ACCESS_SCOPE_HEADER)
        .and_then(|value| value.to_str().ok())
        == Some(FREEHAND_REMOTE_ACCESS_SCOPE_VALUE)
    {
        return UiQueryAccessScope::Remote;
    }
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let host = host
        .strip_prefix('[')
        .and_then(|value| value.split(']').next())
        .or_else(|| host.rsplit_once(':').map(|(value, _)| value))
        .unwrap_or(host);
    match host {
        "localhost" | "127.0.0.1" | "::1" => UiQueryAccessScope::LocalLoopback,
        _ => UiQueryAccessScope::Remote,
    }
}

async fn handle_adp_connection(socket: WebSocket, state: WebUiState, scope: UiQueryAccessScope) {
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
    let mut adp_handshake_accepted = false;
    let mut adp_internal_commands_accepted = false;
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
                            &mut adp_handshake_accepted,
                            &mut adp_internal_commands_accepted,
                            scope,
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
    adp_handshake_accepted: &mut bool,
    adp_internal_commands_accepted: &mut bool,
    scope: UiQueryAccessScope,
    text: String,
) -> Result<(), String> {
    let request: UiAdpRequest =
        serde_json::from_str(&text).map_err(|err| format!("invalid ADP JSON: {err}"))?;
    match request {
        UiAdpRequest::Handshake {
            request_id,
            capabilities,
            ..
        } => {
            if *adp_handshake_accepted {
                let _ = outbound_tx.send(UiAdpResponse::Failure {
                    request_id,
                    failure: UiAdpFailure {
                        code: "adp_handshake_already_accepted".to_owned(),
                        message: "ADP handshake has already been accepted for this connection"
                            .to_owned(),
                        retryable: false,
                    },
                });
                return Ok(());
            }
            *adp_handshake_accepted = true;
            *adp_internal_commands_accepted = adp_internal_commands_allowed(
                state.internal_adp_command_token.as_deref(),
                &capabilities,
            );
            let _ = outbound_tx.send(UiAdpResponse::HandshakeAccepted {
                request_id,
                server_capabilities: adp_server_capabilities(),
            });
            Ok(())
        }
        UiAdpRequest::Command {
            request_id,
            command,
        } => {
            if !*adp_handshake_accepted {
                send_adp_handshake_required(outbound_tx, request_id);
                return Ok(());
            }
            handle_adp_command(
                state,
                outbound_tx,
                request_id,
                command,
                *adp_internal_commands_accepted,
            )
            .await;
            Ok(())
        }
        UiAdpRequest::Query { request_id, query } => {
            if !*adp_handshake_accepted {
                send_adp_handshake_required(outbound_tx, request_id);
                return Ok(());
            }
            let _ = handle_adp_query(state, outbound_tx, request_id, query, scope).await;
            Ok(())
        }
        UiAdpRequest::Subscribe {
            request_id,
            subscription,
        } => {
            if !*adp_handshake_accepted {
                send_adp_handshake_required(outbound_tx, request_id);
                return Ok(());
            }
            let _ =
                handle_adp_subscribe(state, outbound_tx, subscriptions, request_id, subscription)
                    .await;
            Ok(())
        }
    }
}

fn adp_internal_commands_allowed(server_token: Option<&str>, capabilities: &[String]) -> bool {
    let Some(expected_token) = server_token else {
        return false;
    };
    capabilities.iter().any(|capability| {
        adp_internal_command_token_from_capability(capability.as_str()) == Some(expected_token)
    })
}

fn send_adp_handshake_required(
    outbound_tx: &mpsc::UnboundedSender<UiAdpResponse>,
    request_id: String,
) {
    let _ = outbound_tx.send(UiAdpResponse::Failure {
        request_id,
        failure: UiAdpFailure {
            code: "adp_handshake_required".to_owned(),
            message: "ADP handshake is required before command/query/subscribe frames".to_owned(),
            retryable: false,
        },
    });
}

async fn handle_adp_command(
    state: &WebUiState,
    outbound_tx: &mpsc::UnboundedSender<UiAdpResponse>,
    request_id: String,
    command: UiCommand,
    internal_commands_allowed: bool,
) {
    if !(is_public_adp_command(&command)
        || internal_commands_allowed && is_internal_adp_command(&command))
    {
        let _ = outbound_tx.send(UiAdpResponse::Failure {
            request_id,
            failure: UiAdpFailure {
                code: "adp_command_not_public".to_owned(),
                message: "command frame is not exposed on the public ADP command surface"
                    .to_owned(),
                retryable: false,
            },
        });
        return;
    }
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
    scope: UiQueryAccessScope,
) -> Result<(), String> {
    if let Err(err) = accept_query_ingress(&query) {
        let rejection = protocol_rejection(err);
        let _ = outbound_tx.send(UiAdpResponse::Failure {
            request_id,
            failure: UiAdpFailure {
                code: rejection.code,
                message: rejection.message,
                retryable: false,
            },
        });
        return Ok(());
    }
    let runtime_query_port = Arc::clone(&state.runtime_query_port);
    let query_for_runtime = query.clone();
    let runtime_result = tokio::task::spawn_blocking(move || {
        runtime_query_port.query_runtime_with_scope(&query_for_runtime, scope)
    })
    .await
    .map_err(|err| format!("运行时 query task failed: {err}"))?;
    let runtime_owned_page_query = matches!(query, UiCommand::QuerySessionTurnsPage { .. });
    match runtime_result {
        Ok(Some(result)) => {
            let _ = outbound_tx.send(UiAdpResponse::QueryResult { request_id, result });
            return Ok(());
        }
        Ok(None) if runtime_owned_page_query => {
            let _ = outbound_tx.send(UiAdpResponse::Failure {
                request_id,
                failure: UiAdpFailure {
                    code: "session_turn_page_not_found".to_owned(),
                    message: "requested session transcript page is not available".to_owned(),
                    retryable: false,
                },
            });
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
            user_options: None,
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
        StaticUiCommandDispatchPort, UiAdpRequest, UiAdpResponse, UiAgentBoardProjection,
        UiCommand, UiCommandDispatchEnvelope, UiCommandDispatchPortError, UiMasterPollProjection,
        UiQueryResult, UiRuntimeQueryPort, UiTaskBoardProjection, UiTaskEventInboxProjection,
    };
    use futures_util::{SinkExt, StreamExt};
    use reqwest::Client;
    use std::time::Duration;
    use tokio::sync::oneshot;
    use tokio::time::timeout;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message as WsMessage;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    #[test]
    fn query_access_scope_classifies_loopback_and_remote_hosts() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4042"));
        assert_eq!(
            query_access_scope(&headers),
            UiQueryAccessScope::LocalLoopback
        );

        headers.insert(header::HOST, HeaderValue::from_static("localhost"));
        assert_eq!(
            query_access_scope(&headers),
            UiQueryAccessScope::LocalLoopback
        );

        headers.insert(header::HOST, HeaderValue::from_static("[::1]:4042"));
        assert_eq!(
            query_access_scope(&headers),
            UiQueryAccessScope::LocalLoopback
        );

        headers.insert(header::HOST, HeaderValue::from_static("agent.example.test"));
        assert_eq!(query_access_scope(&headers), UiQueryAccessScope::Remote);
    }

    #[test]
    fn query_access_scope_remote_marker_is_explicit_and_authoritative() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:4042"));
        headers.insert(
            FREEHAND_REMOTE_ACCESS_SCOPE_HEADER,
            HeaderValue::from_static(FREEHAND_REMOTE_ACCESS_SCOPE_VALUE),
        );
        assert_eq!(query_access_scope(&headers), UiQueryAccessScope::Remote);
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
            Self::spawn_with_state_port_and_query(
                initial_state,
                command_dispatch_port,
                Arc::new(freehand_ui_protocol::UiProtocolOnlyQueryPort),
            )
            .await
        }

        async fn spawn_with_state_port_and_query(
            initial_state: UiProtocolState,
            command_dispatch_port: Arc<dyn UiCommandDispatchPort>,
            runtime_query_port: Arc<dyn UiRuntimeQueryPort>,
        ) -> Self {
            Self::spawn_with_state_port_query_and_internal_token(
                initial_state,
                command_dispatch_port,
                runtime_query_port,
                None,
            )
            .await
        }

        async fn spawn_with_state_port_query_and_internal_token(
            initial_state: UiProtocolState,
            command_dispatch_port: Arc<dyn UiCommandDispatchPort>,
            runtime_query_port: Arc<dyn UiRuntimeQueryPort>,
            internal_adp_command_token: Option<String>,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("local addr");
            let protocol_state = Arc::new(Mutex::new(initial_state));
            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let protocol_state_for_task = Arc::clone(&protocol_state);
            let command_dispatch_port_for_task = Arc::clone(&command_dispatch_port);
            let runtime_query_port_for_task = Arc::clone(&runtime_query_port);
            let task = tokio::spawn(async move {
                let shutdown = async move {
                    let _ = shutdown_rx.await;
                };
                axum::serve(
                    listener,
                    build_webui_router_with_internal_adp_command_token(
                        protocol_state_for_task,
                        command_dispatch_port_for_task,
                        runtime_query_port_for_task,
                        internal_adp_command_token,
                        "test-adp-auth-token".to_owned(),
                    ),
                )
                .with_graceful_shutdown(shutdown)
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
                "运行时 queue 不可用".to_owned(),
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

    #[derive(Default)]
    struct CountingRuntimeQueryPort {
        queries: Mutex<Vec<String>>,
    }

    impl UiRuntimeQueryPort for CountingRuntimeQueryPort {
        fn query_runtime(
            &self,
            command: &UiCommand,
        ) -> Result<Option<UiQueryResult>, UiCommandDispatchPortError> {
            self.queries
                .lock()
                .expect("query log lock")
                .push(format!("{command:?}"));
            match command {
                UiCommand::QueryMasterPoll { .. } => {
                    Ok(Some(UiQueryResult::MasterPoll(UiMasterPollProjection {
                        source_agent_id: AgentId::new("master"),
                        generated_at: 10,
                        source_cursor: None,
                        next_cursor: None,
                        persisted_cursor: None,
                        event_inbox: UiTaskEventInboxProjection {
                            source_agent_id: AgentId::new("master"),
                            generated_at: 10,
                            source_cursor: None,
                            next_cursor: None,
                            events: Vec::new(),
                        },
                        task_board: UiTaskBoardProjection {
                            source_agent_id: AgentId::new("master"),
                            status_filter: None,
                            agent_filter: None,
                            include_terminal: false,
                            tasks: Vec::new(),
                            agents: Vec::new(),
                            blocked: Vec::new(),
                            review_ready: Vec::new(),
                            stale: Vec::new(),
                        },
                        agent_board: UiAgentBoardProjection {
                            source_agent_id: AgentId::new("master"),
                            agents: Vec::new(),
                        },
                        classifications: Vec::new(),
                    })))
                }
                _ => Ok(None),
            }
        }
    }

    async fn next_adp_response(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> UiAdpResponse {
        let message = timeout(Duration::from_secs(10), socket.next())
            .await
            .expect("adp response timeout")
            .expect("adp response")
            .expect("adp message");
        let WsMessage::Text(text) = message else {
            panic!("unexpected ADP message: {message:?}");
        };
        serde_json::from_str(&text).expect("adp response json")
    }

    async fn connect_adp_with_bearer(
        ws_url: &str,
    ) -> Result<
        (
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            tokio_tungstenite::tungstenite::handshake::client::Response,
        ),
        tokio_tungstenite::tungstenite::Error,
    > {
        let mut request = ws_url.into_client_request().expect("test ADP request");
        request.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test-adp-auth-token"),
        );
        connect_async(request).await
    }

    async fn send_adp_request(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        request: UiAdpRequest,
    ) {
        socket
            .send(WsMessage::Text(
                serde_json::to_string(&request)
                    .expect("adp request json")
                    .into(),
            ))
            .await
            .expect("send adp request");
    }
    async fn perform_adp_handshake(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) {
        perform_adp_handshake_with_capabilities(
            socket,
            vec![freehand_ui_protocol::UI_ADP_HANDSHAKE_CAPABILITY.to_owned()],
        )
        .await;
    }

    async fn perform_adp_handshake_with_capabilities(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        capabilities: Vec<String>,
    ) {
        send_adp_request(
            socket,
            UiAdpRequest::Handshake {
                request_id: "test-handshake".to_owned(),
                client_name: "freehand-server-test".to_owned(),
                capabilities,
            },
        )
        .await;
        match next_adp_response(socket).await {
            UiAdpResponse::HandshakeAccepted { request_id, .. } => {
                assert_eq!(request_id, "test-handshake");
            }
            other => panic!("unexpected ADP handshake response: {other:?}"),
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
    async fn adp_websocket_requires_auth_before_upgrade() {
        let server = TestServer::spawn_empty().await;
        let ws_url = server.base_url.replacen("http://", "ws://", 1) + "/adp";
        let unauth = connect_async(ws_url.clone()).await;
        assert!(
            unauth.is_err(),
            "raw ADP websocket upgrade must require auth"
        );

        let root = Client::new()
            .get(&server.base_url)
            .send()
            .await
            .expect("root response");
        let cookie = root
            .headers()
            .get(header::SET_COOKIE)
            .expect("root sets ADP auth cookie")
            .to_str()
            .expect("cookie header")
            .split(';')
            .next()
            .expect("cookie pair")
            .to_owned();
        let mut cookie_request = ws_url.into_client_request().expect("cookie ws request");
        cookie_request.headers_mut().insert(
            header::COOKIE,
            HeaderValue::from_str(&cookie).expect("cookie header value"),
        );
        let (mut socket, _) = connect_async(cookie_request)
            .await
            .expect("cookie-authenticated ADP connect");
        perform_adp_handshake(&mut socket).await;
        let _ = socket.close(None).await;
        server.stop().await;
    }

    #[tokio::test]
    async fn adp_requires_handshake_before_query_and_accepts_after_handshake() {
        let runtime_query_port = Arc::new(CountingRuntimeQueryPort::default());
        let server = TestServer::spawn_with_state_port_and_query(
            seed_webui_protocol_state(),
            Arc::new(StaticUiCommandDispatchPort::default()),
            runtime_query_port.clone(),
        )
        .await;
        let ws_url = server.base_url.replacen("http://", "ws://", 1) + "/adp";
        let (mut socket, _) = connect_adp_with_bearer(&ws_url).await.expect("adp connect");

        send_adp_request(
            &mut socket,
            UiAdpRequest::Query {
                request_id: "pre-handshake-query".to_owned(),
                query: UiCommand::QueryMasterPoll {
                    after_cursor: None,
                    limit: Some(1),
                    include_terminal: false,
                    replay_from_start: false,
                },
            },
        )
        .await;
        match next_adp_response(&mut socket).await {
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                assert_eq!(request_id, "pre-handshake-query");
                assert_eq!(failure.code, "adp_handshake_required");
                assert!(!failure.retryable);
            }
            other => panic!("query before ADP handshake must fail: {other:?}"),
        }
        assert_eq!(
            runtime_query_port.queries.lock().expect("query log").len(),
            0,
            "pre-handshake query must not reach runtime query port"
        );

        perform_adp_handshake(&mut socket).await;
        send_adp_request(
            &mut socket,
            UiAdpRequest::Query {
                request_id: "post-handshake-query".to_owned(),
                query: UiCommand::QueryMasterPoll {
                    after_cursor: None,
                    limit: Some(1),
                    include_terminal: false,
                    replay_from_start: false,
                },
            },
        )
        .await;
        match next_adp_response(&mut socket).await {
            UiAdpResponse::QueryResult { request_id, .. } => {
                assert_eq!(request_id, "post-handshake-query");
            }
            other => panic!("query after ADP handshake should pass: {other:?}"),
        }
        assert_eq!(
            runtime_query_port.queries.lock().expect("query log").len(),
            1,
            "post-handshake query reaches runtime query port"
        );

        send_adp_request(
            &mut socket,
            UiAdpRequest::Handshake {
                request_id: "duplicate-handshake".to_owned(),
                client_name: "freehand-server-test".to_owned(),
                capabilities: vec![freehand_ui_protocol::UI_ADP_HANDSHAKE_CAPABILITY.to_owned()],
            },
        )
        .await;
        match next_adp_response(&mut socket).await {
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                assert_eq!(request_id, "duplicate-handshake");
                assert_eq!(failure.code, "adp_handshake_already_accepted");
                assert!(!failure.retryable);
            }
            other => panic!("duplicate ADP handshake must fail: {other:?}"),
        }

        let _ = socket.close(None).await;
        server.stop().await;
    }

    #[tokio::test]
    async fn adp_query_frame_rejects_mutations_before_runtime_query_port() {
        let runtime_query_port = Arc::new(CountingRuntimeQueryPort::default());
        let server = TestServer::spawn_with_state_port_and_query(
            seed_webui_protocol_state(),
            Arc::new(StaticUiCommandDispatchPort::default()),
            runtime_query_port.clone(),
        )
        .await;
        let ws_url = server.base_url.replacen("http://", "ws://", 1) + "/adp";
        let (mut socket, _) = connect_adp_with_bearer(&ws_url).await.expect("adp connect");
        perform_adp_handshake(&mut socket).await;

        send_adp_request(
            &mut socket,
            UiAdpRequest::Query {
                request_id: "read-master-poll".to_owned(),
                query: UiCommand::QueryMasterPoll {
                    after_cursor: None,
                    limit: Some(1),
                    include_terminal: false,
                    replay_from_start: false,
                },
            },
        )
        .await;
        match next_adp_response(&mut socket).await {
            UiAdpResponse::QueryResult { request_id, .. } => {
                assert_eq!(request_id, "read-master-poll");
            }
            other => panic!("read-only query should pass ADP query route: {other:?}"),
        }
        assert_eq!(
            runtime_query_port.queries.lock().expect("query log").len(),
            1,
            "read-only query reaches runtime query port"
        );

        send_adp_request(
            &mut socket,
            UiAdpRequest::Query {
                request_id: "mutating-master-poll".to_owned(),
                query: UiCommand::RunMasterPoll {
                    after_cursor: None,
                    limit: Some(1),
                    include_terminal: false,
                    replay_from_start: false,
                },
            },
        )
        .await;
        match next_adp_response(&mut socket).await {
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                assert_eq!(request_id, "mutating-master-poll");
                assert_eq!(failure.code, "direct_task_mutation_forbidden");
                assert!(!failure.retryable);
            }
            other => panic!("mutating query frame should be rejected: {other:?}"),
        }

        send_adp_request(
            &mut socket,
            UiAdpRequest::Query {
                request_id: "execution-fact-query".to_owned(),
                query: UiCommand::ApplyExecutionFact {
                    fact: freehand_ui_protocol::UiExecutionFactCommand {
                        execution_id: "exec-1".to_owned(),
                        task_id: "task-1".to_owned(),
                        agent_id: AgentId::new("worker-1"),
                        turn_id: None,
                        kind: freehand_ui_protocol::UiExecutionFactKind::Blocked {
                            reason: "query frame must not mutate".to_owned(),
                            evidence: Vec::new(),
                        },
                    },
                },
            },
        )
        .await;
        match next_adp_response(&mut socket).await {
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                assert_eq!(request_id, "execution-fact-query");
                assert_eq!(failure.code, "direct_task_mutation_forbidden");
            }
            other => panic!("execution fact query frame should be rejected: {other:?}"),
        }
        assert_eq!(
            runtime_query_port.queries.lock().expect("query log").len(),
            1,
            "mutating query frames must be rejected before runtime query port"
        );

        send_adp_request(
            &mut socket,
            UiAdpRequest::Command {
                request_id: "execution-fact-command".to_owned(),
                command: UiCommand::ApplyExecutionFact {
                    fact: freehand_ui_protocol::UiExecutionFactCommand {
                        execution_id: "exec-2".to_owned(),
                        task_id: "task-2".to_owned(),
                        agent_id: AgentId::new("worker-2"),
                        turn_id: None,
                        kind: freehand_ui_protocol::UiExecutionFactKind::ReviewReady {
                            summary: "internal command must not be public ADP".to_owned(),
                            deliverables: Vec::new(),
                            evidence: Vec::new(),
                        },
                    },
                },
            },
        )
        .await;
        match next_adp_response(&mut socket).await {
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                assert_eq!(request_id, "execution-fact-command");
                assert_eq!(failure.code, "adp_command_not_public");
                assert!(!failure.retryable);
            }
            other => panic!("internal command frame should be rejected: {other:?}"),
        }

        let token_query_port = Arc::new(CountingRuntimeQueryPort::default());
        let token_server = TestServer::spawn_with_state_port_query_and_internal_token(
            UiProtocolState::default(),
            Arc::new(StaticUiCommandDispatchPort::default()),
            token_query_port,
            Some("server-owned-internal-token".to_owned()),
        )
        .await;
        let token_ws_url = format!("{}/adp", token_server.base_url.replace("http://", "ws://"));
        let (mut token_socket, _) = connect_adp_with_bearer(&token_ws_url)
            .await
            .expect("token adp connect");
        perform_adp_handshake_with_capabilities(
            &mut token_socket,
            vec![
                freehand_ui_protocol::UI_ADP_HANDSHAKE_CAPABILITY.to_owned(),
                freehand_ui_protocol::adp_internal_command_capability(
                    "server-owned-internal-token",
                ),
            ],
        )
        .await;
        send_adp_request(
            &mut token_socket,
            UiAdpRequest::Command {
                request_id: "execution-fact-token-command".to_owned(),
                command: UiCommand::ApplyExecutionFact {
                    fact: freehand_ui_protocol::UiExecutionFactCommand {
                        execution_id: "exec-3".to_owned(),
                        task_id: "task-3".to_owned(),
                        agent_id: AgentId::new("worker-3"),
                        turn_id: None,
                        kind: freehand_ui_protocol::UiExecutionFactKind::ReviewReady {
                            summary: "server-owned token admits internal ADP".to_owned(),
                            deliverables: Vec::new(),
                            evidence: Vec::new(),
                        },
                    },
                },
            },
        )
        .await;
        match next_adp_response(&mut token_socket).await {
            UiAdpResponse::CommandReceipt {
                request_id,
                receipt,
            } => {
                assert_eq!(request_id, "execution-fact-token-command");
                assert_eq!(receipt.target_feature_id, "task.orchestration");
            }
            other => panic!("server-owned token should admit internal command: {other:?}"),
        }
        let _ = token_socket.close(None).await;
        token_server.stop().await;

        let _ = socket.close(None).await;
        server.stop().await;
    }

    #[test]
    fn webui_smoke_renders_shell_and_asset_routes() {
        let html = render_webui_smoke();
        assert!(html.contains("data-webui-shell=\"true\""));
        assert!(!html.contains("data-layout-client=\"android-webview\""));
        assert!(!html.contains("data-layout-shape=\"tablet_portrait\""));
        assert!(html.contains("href=\"assets/theme.css"));
        assert!(html.contains("href=\"assets/webui.css"));
        assert!(html.contains("src=\"assets/logo.png"));
        assert!(html.contains("src=\"assets/webui.js"));
        assert!(html.contains(crate::assets::WEBUI_ASSET_VERSION));
        assert!(!html.contains("__WEBUI_ASSET_VERSION__"));
        assert!(html.contains("data-adp-endpoint=\"adp\""));
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
        assert!(html.contains("id=\"mobile-home-active-marker\""));
        assert!(html.contains("id=\"mobile-home-active-list\""));
        assert!(html.contains("mobile-running-session-list"));
        assert!(html.contains("mobile-static-session-list"));
        assert!(html.contains("mobile-active-card"));
        assert!(html.contains("mobile-history-card"));
        assert!(html.contains("正在运行"));
        assert!(html.contains("历史会话"));
        assert!(!html.contains("id=\"mobile-home-timer-marker\""));
        assert!(!html.contains("id=\"mobile-home-timer-list\""));
        assert!(!html.contains("mobile-current-card"));
        assert!(html.contains("id=\"timer-dashboard-dialog\""));
        assert!(html.contains("id=\"timer-dashboard-form\""));
        assert!(html.contains("id=\"timer-mode-input\""));
        assert!(html.contains("id=\"timer-repeat-kind-input\""));
        assert!(html.contains("id=\"timer-source-session-input\""));
        assert!(html.contains("id=\"timer-dashboard-refresh-button\""));
        assert!(html.contains("id=\"timer-dashboard-create-button\""));
        assert!(html.contains("id=\"timer-dashboard-list\""));
        assert!(html.contains("id=\"timer-dashboard-history\""));
        assert!(html.contains("定时器是独立的运行时真源"));
        assert!(html.contains("id=\"tools-dashboard-dialog\""));
        assert!(html.contains("id=\"tools-dashboard-refresh-button\""));
        assert!(html.contains("id=\"tools-dashboard-guidance\""));
        assert!(html.contains("id=\"tools-dashboard-list\""));
        assert!(html.contains("工具来自权威真源注册表投影"));
        assert!(html.contains("id=\"settings-shell\""));
        assert!(!html.contains("id=\"settings-review-tree\""));
        assert!(!html.contains("settings-hero"));
        assert!(html.contains("settings-nav-grid"));
        assert!(html.contains("data-settings-section=\"models\""));
        assert!(html.contains("data-settings-section=\"agent-runtime\""));
        assert!(html.contains("data-settings-section=\"connectivity\""));
        assert!(html.contains("data-settings-section=\"observability\""));
        assert!(html.contains("data-settings-section=\"appearance\""));
        assert!(html.contains("data-settings-section=\"about\""));
        assert!(html.contains("data-settings-state=\"partial\""));
        assert!(html.contains("settings-status-legend"));
        assert!(html.contains("权威真源"));
        assert!(html.contains("placeholder"));
        assert!(!html.contains("<strong>LLM Provider</strong>"));
        assert!(html.contains("id=\"settings-provider-config-page\""));
        assert!(html.contains("id=\"settings-provider-strategy-page\""));
        assert!(html.contains("settings-diagnostics-page"));
        assert!(html.contains("data-settings-group=\"models\""));
        assert!(html.contains("data-settings-group=\"agent-runtime\""));
        assert!(html.contains("data-settings-group=\"connectivity\""));
        assert!(html.contains("data-settings-group=\"observability\""));
        assert!(html.contains("id=\"settings-provider-form\""));
        assert!(html.contains("id=\"settings-provider-current-select\""));
        assert!(html.contains("id=\"settings-provider-fallback-select\""));
        assert!(html.contains("id=\"settings-provider-switch-button\""));
        assert!(html.contains("id=\"settings-provider-registry-list\""));
        assert!(html.contains("新增/更新模型服务"));
        assert!(html.contains("安卓 APK 升级"));
        assert!(html.contains("id=\"settings-apk-update-check-button\""));
        assert!(html.contains("id=\"settings-apk-update-status\""));
        assert!(html.contains("检查 APK 升级"));
        assert!(html.contains("id=\"settings-account-config-sync\""));
        assert!(html.contains("id=\"settings-account-config-pull-button\""));
        assert!(html.contains("id=\"settings-account-config-push-button\""));
        assert_eq!(html.matches("type=\"password\"").count(), 1);
        assert!(html.contains("id=\"settings-relay-password-input\""));
        assert!(!html.contains("rootfs"));
        assert!(!html.contains("shared-folder"));
        assert!(!html.contains("mount-directory"));
        assert!(!html.contains("Skill settings pending"));
        assert!(!html.contains("Task settings pending"));
        assert!(!html.contains("Active agent"));
        assert!(!html.contains("Sessions and workspace"));
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
        assert!(html.contains("class=\"composer-input-shell\""));
        assert!(html.contains("id=\"composer-command-menu-button\""));
        assert!(html.contains("id=\"composer-command-menu\""));
        assert!(html.contains("data-composer-command=\"/help\""));
        assert!(html.contains("data-composer-action=\"tools\""));
        assert!(html.contains("data-composer-action=\"compact\""));
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
        assert!(root_body.contains("href=\"assets/theme.css"));
        assert!(root_body.contains("src=\"assets/logo.png"));
        assert!(root_body.contains("data-adp-endpoint=\"adp\""));
        assert!(root_body.contains("id=\"session-list\""));
        assert!(root_body.contains("id=\"new-conversation-button\""));
        assert!(root_body.contains("id=\"new-task-button\""));
        assert!(root_body.contains("class=\"session-bulk-summary\""));
        assert!(root_body.contains("class=\"session-bulk-actions\""));
        assert!(root_body.contains("id=\"session-bulk-count\""));
        assert!(root_body.contains("id=\"session-select-all-button\""));
        assert!(root_body.contains("id=\"session-clear-selection-button\""));
        assert!(root_body.contains("id=\"session-delete-selected-button\""));
        assert!(!root_body.contains("id=\"session-rename-selected-button\""));
        assert!(root_body.contains("id=\"settings-shell-toggle\""));
        assert!(root_body.contains("id=\"open-settings-drawer-button\""));
        assert!(root_body.contains("id=\"open-timer-dashboard-button\""));
        assert!(root_body.contains("id=\"open-tools-dashboard-button\""));
        assert!(root_body.contains("id=\"mobile-new-entry-button\""));
        assert!(root_body.contains("class=\"mobile-bottom-entries\""));
        assert!(root_body.contains("id=\"mobile-home-dashboard\""));
        assert!(root_body.contains("id=\"mobile-home-active-list\""));
        assert!(root_body.contains("mobile-active-card"));
        assert!(root_body.contains("mobile-history-card"));
        assert!(!root_body.contains("id=\"mobile-home-timer-list\""));
        assert!(!root_body.contains("mobile-current-card"));
        assert!(root_body.contains("id=\"settings-shell\""));
        assert!(!root_body.contains("id=\"settings-review-tree\""));
        assert!(!root_body.contains("settings-hero"));
        assert!(root_body.contains("id=\"settings-provider-config-page\""));
        assert!(root_body.contains("id=\"settings-provider-strategy-page\""));
        assert!(root_body.contains("settings-diagnostics-page"));
        assert!(root_body.contains("id=\"settings-diagnostics-refresh-button\""));
        assert!(root_body.contains("id=\"settings-diagnostics-list\""));
        assert!(root_body.contains("id=\"settings-provider-host\""));
        assert!(root_body.contains("id=\"settings-provider-auth\""));
        assert!(root_body.contains("id=\"settings-config-error\""));
        assert!(root_body.contains("id=\"settings-provider-current-select\""));
        assert!(root_body.contains("id=\"settings-provider-fallback-select\""));
        assert!(root_body.contains("id=\"settings-provider-switch-button\""));
        assert!(root_body.contains("id=\"settings-provider-registry-list\""));
        assert!(root_body.contains("id=\"settings-provider-form\""));
        assert!(root_body.contains("id=\"tools-dashboard-dialog\""));
        assert!(root_body.contains("id=\"tools-dashboard-list\""));
        assert!(root_body.contains("工具来自权威真源注册表投影"));
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
        assert!(root_body.contains("id=\"selected-session-rename-button\""));
        assert!(root_body.contains("id=\"selected-session-rename-button\" type=\"button\" aria-label=\"重命名当前会话\" hidden disabled"));
        assert!(root_body.contains("id=\"session-relation-toggle-button\""));
        assert!(root_body.contains("id=\"session-worker-rail\""));
        assert!(root_body.contains("id=\"session-tree-dropdown\""));
        assert!(root_body.contains("id=\"session-tree\""));
        assert!(root_body.contains("id=\"worker-session-nav\""));
        assert!(root_body.contains("返回主控"));
        assert!(root_body.contains("id=\"settings-agent-resource-count\""));
        assert!(root_body.contains("id=\"settings-agent-resource-increment\""));
        assert!(root_body.contains("id=\"settings-agent-resource-decrement\""));
        assert!(root_body.contains("id=\"settings-agent-resource-save\""));
        assert!(root_body.contains("工作器上限"));
        assert!(!root_body.contains("id=\"mobile-agent-resource-save\""));
        assert!(!root_body.contains("Agent resources"));
        assert!(root_body.contains("Agent 与 Worker"));
        assert!(root_body.contains("选择 Agent 或当前会话的 Worker 任务"));
        assert!(!root_body.contains("id=\"mobile-agent-master-card\""));
        assert!(!root_body.contains("id=\"mobile-agent-agent-list\""));
        assert!(!root_body.contains("id=\"mobile-agent-history-list\""));
        assert!(!root_body.contains("id=\"mobile-agent-control-list\""));
        assert!(!root_body.contains("Master evaluation"));
        assert!(root_body.contains("aria-modal=\"true\""));
        assert!(root_body.contains("任务与智能体生命周期"));
        assert!(root_body.contains("生命周期观察"));
        assert!(root_body.contains("新增/更新模型服务"));
        assert!(root_body.contains("id=\"settings-account-config-sync\""));
        assert!(root_body.contains("id=\"settings-account-config-pull-button\""));
        assert!(root_body.contains("id=\"settings-account-config-push-button\""));
        assert!(!root_body.contains("id=\"settings-agent-value\""));
        assert!(!root_body.contains("Task settings pending"));
        assert!(!root_body.contains("Active agent"));
        assert!(!root_body.contains("Sessions and workspace"));
        assert_eq!(root_body.matches("type=\"password\"").count(), 1);
        assert!(!root_body.contains("api-key"));
        assert!(!root_body.contains("rootfs"));
        assert!(!root_body.contains("shared-folder"));
        assert!(!root_body.contains("mount-directory"));
        assert!(root_body.contains("data-checkpoint-query=\"ui/query/checkpoints\""));
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
        assert!(webui_css_body.contains(".settings-nav-grid"));
        assert!(webui_css_body.contains(".settings-subpage-head"));
        assert!(webui_css_body.contains(".settings-provider-hierarchy"));
        assert!(!webui_css_body.contains(".settings-card"));
        assert!(webui_css_body.contains(".settings-provider-switch"));
        assert!(webui_css_body.contains(".settings-provider-registry"));
        assert!(webui_css_body.contains(".settings-provider-card"));
        assert!(webui_css_body.contains(".mobile-corner-button svg"));
        assert!(webui_css_body.contains(".mobile-home-dashboard"));
        assert!(webui_css_body.contains(".mobile-bottom-entries"));
        assert!(!webui_css_body.contains(".settings-review-tree"));
        assert!(webui_css_body.contains(".settings-diagnostic-row"));
        assert!(webui_css_body.contains(".settings-status-marker"));
        assert!(webui_css_body.contains(".settings-status-marker.partial"));
        assert!(webui_css_body.contains("border-color: var(--running)"));
        assert!(webui_css_body.contains(".settings-status-legend"));
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
        assert!(webui_css_body.contains(".session-worker-rail"));
        assert!(webui_css_body.contains(".session-worker-pill"));
        assert!(webui_css_body.contains(".session-worker-detail"));
        assert!(webui_css_body.contains(".session-tree-dropdown"));
        assert!(webui_css_body.contains("max-height: min(34vh, 320px)"));
        assert!(!webui_css_body.contains("position: absolute;\n  top: calc(100% + 8px)"));
        assert!(webui_css_body.contains(".session-tree-node.is-worker"));
        assert!(webui_css_body.contains(".mobile-running-session-list"));
        assert!(webui_css_body.contains(".mobile-static-session-list"));
        assert!(webui_css_body.contains(".turn-action-bar"));
        assert!(webui_css_body.contains(".turn-action-button"));
        assert!(webui_css_body.contains(".tool-field-grid"));
        assert!(webui_css_body.contains(".tool-raw-details"));
        assert!(webui_css_body.contains(".tool-chat-line-secondary.tool-chat-line-success"));
        assert!(webui_css_body.contains(".tool-chat-line-secondary.tool-chat-line-failed"));
        assert!(
            webui_css_body.contains("body[data-mobile-agent-sheet=\"open\"] .mobile-agent-sheet")
        );
        assert!(webui_css_body.contains(
            "body[data-layout-shape=\"tablet_portrait\"][data-webui-route=\"home_dashboard\"] .mobile-agent-summary-strip {\n  display: block;\n}"
        ));
        assert!(!webui_css_body.contains(
            "body[data-layout-shape=\"tall_phone\"][data-webui-route=\"home_dashboard\"] .mobile-agent-summary-strip,\nbody[data-layout-shape=\"tablet_portrait\"][data-webui-route=\"home_dashboard\"] .mobile-agent-summary-strip,\nbody[data-layout-shape=\"phone_portrait\"][data-webui-route=\"home_dashboard\"] .session-relation-header"
        ));
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
        assert!(webui_css_body.contains(".composer-input-shell"));
        assert!(webui_css_body.contains(".composer-attach-image"));
        assert!(webui_css_body.contains(".composer-command-menu"));
        assert!(
            webui_css_body
                .contains("body[data-layout-shape=\"phone_portrait\"] .composer-command-menu")
        );
        assert!(
            webui_css_body
                .contains("body[data-layout-shape=\"tall_phone\"] .composer-command-menu")
        );
        assert!(
            webui_css_body
                .contains("body[data-layout-shape=\"tablet_portrait\"] .composer-command-menu")
        );
        assert!(webui_css_body.contains("  left: 0;\n  right: auto;"));
        assert!(
            webui_css_body
                .contains("body[data-layout-shape=\"phone_portrait\"] .composer-command-strip")
        );
        assert!(webui_css_body.contains("body[data-layout-shape=\"phone_portrait\"][data-composer-focused=\"true\"] .composer-command-strip"));
        assert!(webui_css_body.contains("max-height: min(20svh, 158px)"));
        assert!(webui_css_body.contains("max-height: min(30svh, 208px)"));
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
        let js_body = js.text().await.expect("js body");
        assert!(js_body.contains("initializeMobileWebui"));
        assert!(js_body.contains(&format!(
            "./webui/bootstrap.js?v={}",
            crate::assets::WEBUI_ASSET_VERSION
        )));
        assert!(
            !js_body.contains("__WEBUI_ASSET_VERSION__"),
            "served assets must have the version token stamped"
        );
        assert!(!js_body.contains("initializeThemeToggle"));

        let bootstrap = client
            .get(format!("{}/assets/webui/bootstrap.js", server.base_url))
            .send()
            .await
            .expect("bootstrap response");
        assert_eq!(bootstrap.status(), StatusCode::OK);
        let bootstrap_body = bootstrap.text().await.expect("bootstrap body");
        assert!(bootstrap_body.contains("initializeThemeToggle"));
        assert!(bootstrap_body.contains("surfaceContracts"));
        assert!(bootstrap_body.contains("./legacy-monolith.js"));
        assert!(bootstrap_body.contains("__freehandWebUiSurfaceContracts"));
        assert!(bootstrap_body.contains("__freehandSharedStateContract"));

        let legacy = client
            .get(format!(
                "{}/assets/webui/legacy-monolith.js",
                server.base_url
            ))
            .send()
            .await
            .expect("legacy response");
        assert_eq!(legacy.status(), StatusCode::OK);
        let legacy_body = legacy.text().await.expect("legacy body");
        assert!(
            legacy_body.contains("function requireSessionListPageProjection"),
            "legacy WebUI must expose the SessionListPage projection validator",
        );
        assert!(
            legacy_body.contains(
                "cursor !== undefined && cursor !== null && typeof cursor !== \"string\"",
            ),
            "legacy SessionListPage validator must accept omitted/undefined next_cursor",
        );
        assert!(
            legacy_body.contains("!Array.isArray(page.unavailable_sessions)"),
            "legacy SessionListPage validator must require unavailable_sessions array",
        );
        assert!(
            legacy_body.contains("(page.has_older && typeof cursor !== \"string\")"),
            "legacy SessionListPage validator must require a cursor when older pages exist",
        );
        assert!(legacy_body.contains("function renderMobileHomeDashboard"));
        assert!(legacy_body.contains("function mobileHomeSessionButton"));
        assert!(legacy_body.contains("renderHomeDashboardSurface"));
        assert!(legacy_body.contains("renderToolsRegistrySurface"));
        assert!(legacy_body.contains("renderTimerDashboardSurface"));
        assert!(legacy_body.contains("renderSessionSearchSurface"));
        assert!(legacy_body.contains("renderSettingsShellSurface"));
        assert!(legacy_body.contains("openNewSessionSurface"));
        assert!(legacy_body.contains("switchConversationSessionInSurface"));
        assert!(legacy_body.contains("createAdpClient"));
        assert!(legacy_body.contains("function agentNavigationUrl"));
        assert!(legacy_body.contains("function phase2AgentLabel"));
        assert!(legacy_body.contains("function sessionAgentGroupLabel"));
        assert!(legacy_body.contains("name.textContent = sessionAgentGroupLabel();"));
        assert!(!legacy_body.contains("name.textContent = \"Master\";"));
        assert!(legacy_body.contains("当前配置中没有可访问的 Agent"));
        assert!(legacy_body.contains("进入独立会话"));
        assert!(legacy_body.contains("adpQueryOf"));
        assert!(legacy_body.contains("adpCommandOf"));
        assert!(legacy_body.contains("generated/adp-protocol.js"));
        assert!(legacy_body.contains("dispatchWebUiEdge"));
        assert!(legacy_body.contains("function setComposerCommandMenuOpen"));
        assert!(legacy_body.contains("composerCommandMenuButton"));
        assert!(legacy_body.contains("data-composer-action"));
        assert!(legacy_body.contains("__freehandOpenAndroidComposerForReadyHost"));
        assert!(legacy_body.contains("openAndroidComposerForReadyHost"));
        assert!(legacy_body.contains("scheduleAndroidComposerFocus"));
        assert!(legacy_body.contains("function formatCacheHitPercent"));
        assert!(legacy_body.contains("((read / total) * 100).toFixed(3)"));
        assert!(legacy_body.contains("if (total <= 0)"));
        assert!(!legacy_body.contains("if (read <= 0 || total <= 0)"));
        assert!(legacy_body.contains("counts.activeCount} 活动"));
        assert!(legacy_body.contains("counts.reviewCount} 审核"));
        assert!(legacy_body.contains("counts.blockedCount} 阻塞"));
        assert!(legacy_body.contains("counts.closedCount} 关闭"));
        assert!(!legacy_body.contains("counts.activeCount}A"));

        let layout_shape = client
            .get(format!(
                "{}/assets/webui/app-shell/layout-shape.js",
                server.base_url
            ))
            .send()
            .await
            .expect("layout-shape response");
        assert_eq!(layout_shape.status(), StatusCode::OK);
        let layout_shape_body = layout_shape.text().await.expect("layout-shape body");
        assert!(layout_shape_body.contains("export function classifyLayoutShape"));
        assert!(layout_shape_body.contains("export function viewportDimensionsForLayout"));

        let edge_registry = client
            .get(format!(
                "{}/assets/webui/app-shell/edge-registry.js",
                server.base_url
            ))
            .send()
            .await
            .expect("edge-registry response");
        assert_eq!(edge_registry.status(), StatusCode::OK);
        let edge_registry_body = edge_registry.text().await.expect("edge-registry body");
        assert!(edge_registry_body.contains("root.open_home"));
        assert!(edge_registry_body.contains("home.open_agent_directory"));
        assert!(edge_registry_body.contains("session.open_parent_session"));
        assert!(edge_registry_body.contains("session.rename_session"));

        let route_controller = client
            .get(format!(
                "{}/assets/webui/app-shell/route-controller.js",
                server.base_url
            ))
            .send()
            .await
            .expect("route-controller response");
        assert_eq!(route_controller.status(), StatusCode::OK);
        let route_controller_body = route_controller
            .text()
            .await
            .expect("route-controller body");
        assert!(route_controller_body.contains("session.open_parent_session"));
        assert!(route_controller_body.contains("root.open_home"));

        let home_surface = client
            .get(format!(
                "{}/assets/webui/surfaces/home-dashboard/index.js",
                server.base_url
            ))
            .send()
            .await
            .expect("home-surface response");
        assert_eq!(home_surface.status(), StatusCode::OK);
        assert!(
            home_surface
                .text()
                .await
                .expect("home-surface body")
                .contains("renderSurface")
        );

        let home_view = client
            .get(format!(
                "{}/assets/webui/surfaces/home-dashboard/view.js",
                server.base_url
            ))
            .send()
            .await
            .expect("home-view response");
        assert_eq!(home_view.status(), StatusCode::OK);
        assert!(
            home_view
                .text()
                .await
                .expect("home-view body")
                .contains("renderHomeDashboard")
        );

        let home_controls = client
            .get(format!(
                "{}/assets/webui/surfaces/home-dashboard/controls.js",
                server.base_url
            ))
            .send()
            .await
            .expect("home-controls response");
        assert_eq!(home_controls.status(), StatusCode::OK);
        let home_controls_body = home_controls.text().await.expect("home-controls body");
        assert!(home_controls_body.contains("mobile-home-session-checkbox"));
        assert!(!home_controls_body.contains("data-session-action = 'rename'"));
        assert!(!home_controls_body.contains("data-session-action=\"rename\""));

        let tools_view = client
            .get(format!(
                "{}/assets/webui/surfaces/tools-registry/view.js",
                server.base_url
            ))
            .send()
            .await
            .expect("tools-view response");
        assert_eq!(tools_view.status(), StatusCode::OK);
        assert!(
            tools_view
                .text()
                .await
                .expect("tools-view body")
                .contains("renderToolsRegistrySurface")
        );

        let adp_client = client
            .get(format!(
                "{}/assets/webui/app-shell/adp-client.js",
                server.base_url
            ))
            .send()
            .await
            .expect("adp-client response");
        assert_eq!(adp_client.status(), StatusCode::OK);
        let adp_client_body = adp_client.text().await.expect("adp-client body");
        assert!(adp_client_body.contains("export function createAdpClient"));

        for (asset, symbol) in [
            ("app-shell/shared-states/index.js", "sharedStateContract"),
            ("app-shell/shared-states/model.js", "createSharedStateModel"),
            ("app-shell/shared-states/view.js", "renderSharedState"),
            ("app-shell/surface-registry.js", "surfaceContracts"),
        ] {
            let response = client
                .get(format!("{}/assets/webui/{}", server.base_url, asset))
                .send()
                .await
                .expect("shared-state asset response");
            assert_eq!(response.status(), StatusCode::OK);
            assert!(
                response
                    .text()
                    .await
                    .expect("shared-state asset body")
                    .contains(symbol)
            );
        }
        assert!(adp_client_body.contains("generated/adp-protocol.js"));
        assert!(adp_client_body.contains("ADP_PROTOCOL_VERSION"));

        let adp_protocol = client
            .get(format!(
                "{}/assets/webui/generated/adp-protocol.js",
                server.base_url
            ))
            .send()
            .await
            .expect("adp-protocol response");
        assert_eq!(adp_protocol.status(), StatusCode::OK);
        let adp_protocol_body = adp_protocol.text().await.expect("adp-protocol body");
        assert!(adp_protocol_body.contains("export function adpQueryOf"));
        assert!(adp_protocol_body.contains("export function adpCommandOf"));
        assert!(adp_protocol_body.contains("\"protocol_version\": 4"));
        assert!(adp_protocol_body.contains("QueryConfigStatus"));
        assert!(adp_protocol_body.contains("CreateSession"));

        let session_detail_surface = client
            .get(format!(
                "{}/assets/webui/surfaces/session-detail/index.js",
                server.base_url
            ))
            .send()
            .await
            .expect("session-detail surface response");
        assert_eq!(session_detail_surface.status(), StatusCode::OK);
        assert!(
            session_detail_surface
                .text()
                .await
                .expect("session-detail body")
                .contains("message-list")
        );

        let modular_surface_assets = [
            ("session-detail/controls.js", "switchConversationSession"),
            ("session-search/view.js", "renderSessionSearchSurface"),
            ("new-session/controls.js", "openNewSessionSurface"),
            ("settings/view.js", "renderSettingsShellSurface"),
            (
                "session-detail/recovery.js",
                "recoveredHistoricalWorkerFailureTurnIds",
            ),
            ("session-detail/recovery.js", "historical_failure_recovered"),
            (
                "settings/diagnostics.js",
                "renderSettingsDiagnosticsSurface",
            ),
            ("tools-registry/controls.js", "refreshToolsRegistrySurface"),
            ("tools-registry/controls.js", "adpQueryOf"),
            ("tools-registry/controls.js", "QueryToolRegistry"),
            ("timer-dashboard/controls.js", "scheduleTimerFromSurface"),
            ("timer-dashboard/controls.js", "adpQueryOf"),
            ("timer-dashboard/controls.js", "QueryTimerList"),
        ];
        for (asset, symbol) in modular_surface_assets {
            let response = client
                .get(format!(
                    "{}/assets/webui/surfaces/{}",
                    server.base_url, asset
                ))
                .send()
                .await
                .expect("modular surface asset response");
            assert_eq!(response.status(), StatusCode::OK, "asset {asset}");
            assert!(
                response
                    .text()
                    .await
                    .expect("modular surface asset body")
                    .contains(symbol),
                "asset {asset} should contain {symbol}"
            );
        }

        assert!(root_body.contains("模型服务配置"));
        assert!(root_body.contains("模型服务切换与策略"));
        assert!(root_body.contains("诊断日志"));
        assert!(root_body.contains("id=\"mobile-home-dashboard\""));
        assert!(root_body.contains("id=\"mobile-home-session-list\""));
        assert!(root_body.contains("id=\"open-timer-dashboard-button\""));
        assert!(root_body.contains("id=\"open-tools-dashboard-button\""));
        assert!(root_body.contains("id=\"mobile-new-entry-button\""));
        assert!(root_body.contains("id=\"session-relation-header\""));
        assert!(root_body.contains("id=\"tools-dashboard-dialog\""));
        assert!(root_body.contains("id=\"timer-dashboard-dialog\""));
        assert!(root_body.contains("id=\"session-search-dialog\""));
        assert!(root_body.contains("id=\"new-session-dialog\""));
        assert!(root_body.contains("id=\"settings-shell\""));

        assert!(legacy_body.contains("QueryTimerList"));
        assert!(legacy_body.contains("QueryDiagnostics"));
        assert!(legacy_body.contains("QueryConfigStatus"));
        assert!(legacy_body.contains("QueryTaskBoard"));
        assert!(legacy_body.contains("QueryAgentBoard"));
        assert!(legacy_body.contains("QueryEventInbox"));
        assert!(legacy_body.contains("QueryTaskHistory"));
        assert!(legacy_body.contains("QueryWorkerControl"));
        assert!(legacy_body.contains("function renderMobileHomeDashboard"));
        assert!(legacy_body.contains("function mobileHomeSessionButton"));
        assert!(legacy_body.contains("function renderToolsDashboard"));
        assert!(legacy_body.contains("function renderTimerDashboard"));
        assert!(legacy_body.contains("function renderSettingsDiagnostics"));
        assert!(legacy_body.contains("function renderPhase2Dashboard"));
        assert!(legacy_body.contains("function openWorkerTaskSession"));
        assert!(legacy_body.contains("function switchConversationSession"));
        assert!(legacy_body.contains("function returnToParentSession"));
        assert!(legacy_body.contains("function sessionLiveObservation"));
        assert!(legacy_body.contains("function globalLiveSessionObservation"));
        assert!(legacy_body.contains("function mobileHomeHistoryBuckets"));
        assert!(legacy_body.contains("function mobileHomeHistoryBucketId"));
        assert!(legacy_body.contains("function renameCurrentSession"));
        assert!(legacy_body.contains("session.rename_session"));
        assert!(legacy_body.contains("function deleteSessionFromHome"));
        assert!(!legacy_body.contains("function renameSelectedSession"));
        assert!(!legacy_body.contains("function renameSessionFromHome"));
        assert!(!legacy_body.contains("home.rename_session"));
        assert!(!legacy_body.contains("session-rename-selected-button"));
        assert!(legacy_body.contains("dispatchWebUiEdge"));
        assert!(legacy_body.contains("session.open_parent_session"));
        assert!(legacy_body.contains("home.open_agent_directory"));
        assert!(
            legacy_body.contains("state.route === \"home_dashboard\" && !state.selectedSessionId")
        );
        assert!(legacy_body.contains("root.open_home"));
        assert!(legacy_body.contains("renderHomeDashboardSurface"));
        assert!(!legacy_body.contains("运行、重试、等待用户选择的会话"));
        assert!(!legacy_body.contains("parentTasks.length > 0 ? parentTasks : allTasks"));
        assert!(!legacy_body.contains("function renderSettingsReviewTree"));

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
                        search_evidence: None,
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

        let internal_rejected = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::ApplyExecutionFact {
                fact: freehand_ui_protocol::UiExecutionFactCommand {
                    execution_id: "legacy-http-exec".to_owned(),
                    task_id: "legacy-http-task".to_owned(),
                    agent_id: AgentId::new("worker-legacy-http"),
                    turn_id: None,
                    kind: freehand_ui_protocol::UiExecutionFactKind::ReviewReady {
                        summary: "legacy HTTP must not bypass public command surface".to_owned(),
                        deliverables: Vec::new(),
                        evidence: Vec::new(),
                    },
                },
            })
            .send()
            .await
            .expect("internal reject response");
        assert_eq!(internal_rejected.status(), StatusCode::BAD_REQUEST);
        let internal_rejected: UiCommandDispatchFailure = internal_rejected
            .json()
            .await
            .expect("internal reject json");
        assert_eq!(internal_rejected.code, "adp_command_not_public");
        assert!(!internal_rejected.retryable);

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
        assert!(failure.message.contains("运行时 queue 不可用"));

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
