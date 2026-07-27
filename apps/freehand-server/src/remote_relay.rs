use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as UpstreamMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRelayEndpointCandidate {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adp_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_host_id: Option<String>,
    pub auth_required: bool,
    pub last_seen_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRelayHostRegistration {
    pub account_id: String,
    pub daemon_id: String,
    pub relay_host_id: String,
    pub upstream_base_url: String,
    #[serde(default)]
    pub endpoints: Vec<RemoteRelayEndpointCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRelayHostRecord {
    pub account_id: String,
    pub daemon_id: String,
    pub relay_host_id: String,
    pub upstream_base_url: String,
    pub endpoints: Vec<RemoteRelayEndpointCandidate>,
    pub last_seen_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRelayAccountDirectory {
    pub schema_version: u32,
    pub account_id: String,
    pub daemons: Vec<RemoteRelayHostRecord>,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRelayErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct RemoteRelayDirectory {
    hosts: BTreeMap<String, RemoteRelayHostRecord>,
}

impl RemoteRelayDirectory {
    pub fn publish_host(
        &mut self,
        registration: RemoteRelayHostRegistration,
        now_unix: u64,
    ) -> Result<RemoteRelayHostRecord, RemoteRelayDirectoryError> {
        let account_id = non_empty("accountId", registration.account_id)?;
        let daemon_id = non_empty("daemonId", registration.daemon_id)?;
        let relay_host_id = non_empty("relayHostId", registration.relay_host_id)?;
        let upstream_base_url = normalize_upstream_base_url(&registration.upstream_base_url)?;
        let endpoints = if registration.endpoints.is_empty() {
            vec![RemoteRelayEndpointCandidate {
                id: format!("relay:{relay_host_id}"),
                kind: "relay".to_owned(),
                host: None,
                port: None,
                web_url: Some(format!("/relay/daemon/{relay_host_id}/")),
                adp_url: Some(format!("/relay/daemon/{relay_host_id}/adp")),
                relay_host_id: Some(relay_host_id.clone()),
                auth_required: true,
                last_seen_unix: now_unix,
            }]
        } else {
            registration
                .endpoints
                .into_iter()
                .map(|endpoint| normalize_endpoint(endpoint, now_unix))
                .collect::<Result<Vec<_>, _>>()?
        };
        let record = RemoteRelayHostRecord {
            account_id,
            daemon_id,
            relay_host_id: relay_host_id.clone(),
            upstream_base_url,
            endpoints,
            last_seen_unix: now_unix,
        };
        self.hosts.insert(relay_host_id, record.clone());
        Ok(record)
    }

    pub fn account_directory(
        &self,
        account_id: &str,
        now_unix: u64,
    ) -> RemoteRelayAccountDirectory {
        let mut daemons = self
            .hosts
            .values()
            .filter(|record| record.account_id == account_id)
            .cloned()
            .collect::<Vec<_>>();
        daemons.sort_by(|left, right| {
            left.daemon_id
                .cmp(&right.daemon_id)
                .then_with(|| left.relay_host_id.cmp(&right.relay_host_id))
        });
        RemoteRelayAccountDirectory {
            schema_version: 1,
            account_id: account_id.to_owned(),
            daemons,
            updated_at_unix: now_unix,
        }
    }

    pub fn host(&self, relay_host_id: &str) -> Option<RemoteRelayHostRecord> {
        self.hosts.get(relay_host_id).cloned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRelayDirectoryError {
    code: String,
    message: String,
}

impl RemoteRelayDirectoryError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_relay_directory_input".to_owned(),
            message: message.into(),
        }
    }
}

#[derive(Clone)]
struct RemoteRelayState {
    directory: Arc<Mutex<RemoteRelayDirectory>>,
    http_client: reqwest::Client,
}

pub fn build_remote_relay_router(directory: Arc<Mutex<RemoteRelayDirectory>>) -> Router {
    let http_client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("relay http client");
    Router::new()
        .route("/relay/health", get(handle_relay_health))
        .route("/relay/hosts", post(handle_publish_host))
        .route(
            "/relay/directory/{account_id}",
            get(handle_account_directory),
        )
        .route(
            "/relay/daemon/{relay_host_id}/health",
            get(handle_relay_daemon_health),
        )
        .route(
            "/relay/daemon/{relay_host_id}/adp",
            get(handle_relay_daemon_adp),
        )
        .route(
            "/relay/daemon/{relay_host_id}/",
            get(handle_relay_daemon_http_root),
        )
        .route(
            "/relay/daemon/{relay_host_id}/{*path}",
            get(handle_relay_daemon_http),
        )
        .with_state(RemoteRelayState {
            directory,
            http_client,
        })
}

pub async fn serve_remote_relay_listener<F>(
    listener: TcpListener,
    directory: Arc<Mutex<RemoteRelayDirectory>>,
    shutdown: F,
) -> std::io::Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, build_remote_relay_router(directory))
        .with_graceful_shutdown(shutdown)
        .await
}

async fn handle_relay_health() -> &'static str {
    "ok"
}

async fn handle_publish_host(
    State(state): State<RemoteRelayState>,
    Json(registration): Json<RemoteRelayHostRegistration>,
) -> Result<(StatusCode, Json<RemoteRelayHostRecord>), (StatusCode, Json<RemoteRelayErrorBody>)> {
    let record = state
        .directory
        .lock()
        .expect("lock relay directory")
        .publish_host(registration, now_unix())
        .map_err(remote_relay_bad_request)?;
    Ok((StatusCode::ACCEPTED, Json(record)))
}

async fn handle_account_directory(
    Path(account_id): Path<String>,
    State(state): State<RemoteRelayState>,
) -> Json<RemoteRelayAccountDirectory> {
    Json(
        state
            .directory
            .lock()
            .expect("lock relay directory")
            .account_directory(&account_id, now_unix()),
    )
}

async fn handle_relay_daemon_health(
    Path(relay_host_id): Path<String>,
    State(state): State<RemoteRelayState>,
) -> Response {
    let Some(record) = state
        .directory
        .lock()
        .expect("lock relay directory")
        .host(&relay_host_id)
    else {
        return remote_relay_error_response(
            StatusCode::NOT_FOUND,
            "relay_host_not_found",
            format!("relay host `{relay_host_id}` is not registered"),
        );
    };
    let upstream_url = match join_upstream_url(&record.upstream_base_url, "health") {
        Ok(url) => url,
        Err(error) => {
            return remote_relay_error_response(
                StatusCode::BAD_GATEWAY,
                "invalid_upstream_url",
                error.message,
            );
        }
    };
    match state.http_client.get(upstream_url).send().await {
        Ok(response) => proxy_http_response(response, None).await,
        Err(error) => remote_relay_error_response(
            StatusCode::BAD_GATEWAY,
            "relay_upstream_unreachable",
            format!("upstream health request failed: {error}"),
        ),
    }
}

async fn handle_relay_daemon_http_root(
    Path(relay_host_id): Path<String>,
    State(state): State<RemoteRelayState>,
    OriginalUri(original_uri): OriginalUri,
) -> Response {
    proxy_relay_daemon_http(relay_host_id, String::new(), state, original_uri).await
}

async fn handle_relay_daemon_http(
    Path((relay_host_id, path)): Path<(String, String)>,
    State(state): State<RemoteRelayState>,
    OriginalUri(original_uri): OriginalUri,
) -> Response {
    proxy_relay_daemon_http(relay_host_id, path, state, original_uri).await
}

async fn proxy_relay_daemon_http(
    relay_host_id: String,
    path: String,
    state: RemoteRelayState,
    original_uri: axum::http::Uri,
) -> Response {
    let Some(record) = state
        .directory
        .lock()
        .expect("lock relay directory")
        .host(&relay_host_id)
    else {
        return remote_relay_error_response(
            StatusCode::NOT_FOUND,
            "relay_host_not_found",
            format!("relay host `{relay_host_id}` is not registered"),
        );
    };
    let mut upstream_url = match join_upstream_url(&record.upstream_base_url, &path) {
        Ok(url) => url,
        Err(error) => {
            return remote_relay_error_response(
                StatusCode::BAD_GATEWAY,
                "invalid_upstream_url",
                error.message,
            );
        }
    };
    upstream_url.set_query(original_uri.query());
    match state.http_client.get(upstream_url).send().await {
        Ok(response) => {
            let relay_prefix = format!("/relay/daemon/{relay_host_id}/");
            proxy_http_response(response, Some(&relay_prefix)).await
        }
        Err(error) => remote_relay_error_response(
            StatusCode::BAD_GATEWAY,
            "relay_upstream_unreachable",
            format!("upstream HTTP request failed: {error}"),
        ),
    }
}

async fn handle_relay_daemon_adp(
    Path(relay_host_id): Path<String>,
    State(state): State<RemoteRelayState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let Some(record) = state
        .directory
        .lock()
        .expect("lock relay directory")
        .host(&relay_host_id)
    else {
        return remote_relay_error_response(
            StatusCode::NOT_FOUND,
            "relay_host_not_found",
            format!("relay host `{relay_host_id}` is not registered"),
        );
    };
    let upstream_url =
        match join_upstream_url(&record.upstream_base_url, "adp").and_then(http_url_to_ws_url) {
            Ok(url) => url,
            Err(error) => {
                return remote_relay_error_response(
                    StatusCode::BAD_GATEWAY,
                    "invalid_upstream_url",
                    error.message,
                );
            }
        };
    ws.on_upgrade(move |socket| relay_adp_socket(socket, upstream_url, headers))
        .into_response()
}

async fn relay_adp_socket(
    client_socket: WebSocket,
    upstream_url: String,
    client_headers: HeaderMap,
) {
    let mut upstream_request = match upstream_url.as_str().into_client_request() {
        Ok(request) => request,
        Err(_) => return,
    };
    for header_name in [header::AUTHORIZATION, header::COOKIE] {
        if let Some(value) = client_headers.get(&header_name) {
            upstream_request
                .headers_mut()
                .insert(header_name, value.clone());
        }
    }
    let (upstream_socket, _) = match connect_async(upstream_request).await {
        Ok(socket) => socket,
        Err(_) => return,
    };
    let (mut client_sender, mut client_receiver) = client_socket.split();
    let (mut upstream_sender, mut upstream_receiver) = upstream_socket.split();

    let client_to_upstream = async {
        while let Some(message) = client_receiver.next().await {
            let Ok(message) = message else {
                break;
            };
            let Some(upstream_message) = client_to_upstream_message(message) else {
                break;
            };
            if upstream_sender.send(upstream_message).await.is_err() {
                break;
            }
        }
    };

    let upstream_to_client = async {
        while let Some(message) = upstream_receiver.next().await {
            let Ok(message) = message else {
                break;
            };
            let Some(client_message) = upstream_to_client_message(message) else {
                break;
            };
            if client_sender.send(client_message).await.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = client_to_upstream => {}
        _ = upstream_to_client => {}
    }
}

fn client_to_upstream_message(message: Message) -> Option<UpstreamMessage> {
    match message {
        Message::Text(text) => Some(UpstreamMessage::Text(text.to_string().into())),
        Message::Binary(bytes) => Some(UpstreamMessage::Binary(bytes.to_vec().into())),
        Message::Ping(bytes) => Some(UpstreamMessage::Ping(bytes.to_vec().into())),
        Message::Pong(bytes) => Some(UpstreamMessage::Pong(bytes.to_vec().into())),
        Message::Close(_) => None,
    }
}

fn upstream_to_client_message(message: UpstreamMessage) -> Option<Message> {
    match message {
        UpstreamMessage::Text(text) => Some(Message::Text(text.to_string().into())),
        UpstreamMessage::Binary(bytes) => Some(Message::Binary(bytes.to_vec().into())),
        UpstreamMessage::Ping(bytes) => Some(Message::Ping(bytes.to_vec().into())),
        UpstreamMessage::Pong(bytes) => Some(Message::Pong(bytes.to_vec().into())),
        UpstreamMessage::Close(_) => None,
        UpstreamMessage::Frame(_) => None,
    }
}

async fn proxy_http_response(upstream: reqwest::Response, relay_prefix: Option<&str>) -> Response {
    let status = StatusCode::from_u16(upstream.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| HeaderValue::from_str(value).ok());
    let cache_control = upstream
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| HeaderValue::from_str(value).ok());
    let set_cookie = upstream
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| HeaderValue::from_str(value).ok());
    let should_rewrite = relay_prefix.is_some()
        && content_type
            .as_ref()
            .and_then(|value| value.to_str().ok())
            .is_some_and(is_rewriteable_webui_content_type);
    if !should_rewrite {
        let mut builder = Response::builder().status(status);
        if let Some(content_type) = content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        if let Some(cache_control) = cache_control {
            builder = builder.header(header::CACHE_CONTROL, cache_control);
        }
        if let Some(set_cookie) = set_cookie {
            builder = builder.header(header::SET_COOKIE, set_cookie);
        }
        return builder
            .body(Body::from_stream(upstream.bytes_stream()))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }
    let body = match upstream.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return remote_relay_error_response(
                StatusCode::BAD_GATEWAY,
                "relay_upstream_read_failed",
                format!("upstream response read failed: {error}"),
            );
        }
    };
    let body = match String::from_utf8(body.to_vec()) {
        Ok(text) => rewrite_relay_webui_paths(&text, relay_prefix.expect("rewrite prefix")),
        Err(_) => {
            return remote_relay_error_response(
                StatusCode::BAD_GATEWAY,
                "relay_upstream_rewrite_failed",
                "upstream WebUI response was not valid UTF-8",
            );
        }
    };
    let mut builder = Response::builder().status(status);
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    if let Some(cache_control) = cache_control {
        builder = builder.header(header::CACHE_CONTROL, cache_control);
    }
    if let Some(set_cookie) = set_cookie {
        builder = builder.header(header::SET_COOKIE, set_cookie);
    }
    builder
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn is_rewriteable_webui_content_type(value: &str) -> bool {
    value.starts_with("text/html")
        || value.starts_with("text/javascript")
        || value.starts_with("application/javascript")
}

fn rewrite_relay_webui_paths(body: &str, relay_prefix: &str) -> String {
    let prefix = relay_prefix.trim_end_matches('/');
    body.replace("href=\"/assets/", &format!("href=\"{prefix}/assets/"))
        .replace("src=\"/assets/", &format!("src=\"{prefix}/assets/"))
        .replace("from \"/assets/", &format!("from \"{prefix}/assets/"))
        .replace(
            "data-adp-endpoint=\"/adp\"",
            &format!("data-adp-endpoint=\"{prefix}/adp\""),
        )
        .replace(
            "data-turn-query=\"/ui/query/latest-active-turn\"",
            &format!("data-turn-query=\"{prefix}/ui/query/latest-active-turn\""),
        )
        .replace(
            "data-turn-subscribe=\"/ui/subscribe/turn/latest\"",
            &format!("data-turn-subscribe=\"{prefix}/ui/subscribe/turn/latest\""),
        )
        .replace(
            "data-debug-query-base=\"/ui/query/debug/\"",
            &format!("data-debug-query-base=\"{prefix}/ui/query/debug/\""),
        )
        .replace(
            "data-debug-subscribe-base=\"/ui/subscribe/debug/\"",
            &format!("data-debug-subscribe-base=\"{prefix}/ui/subscribe/debug/\""),
        )
        .replace(
            "data-checkpoint-query=\"/ui/query/checkpoints\"",
            &format!("data-checkpoint-query=\"{prefix}/ui/query/checkpoints\""),
        )
        .replace(
            "data-command-endpoint=\"/ui/command\"",
            &format!("data-command-endpoint=\"{prefix}/ui/command\""),
        )
}

fn non_empty(field_name: &'static str, value: String) -> Result<String, RemoteRelayDirectoryError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(RemoteRelayDirectoryError::invalid(format!(
            "{field_name} is required"
        )));
    }
    Ok(value)
}

fn normalize_endpoint(
    endpoint: RemoteRelayEndpointCandidate,
    now_unix: u64,
) -> Result<RemoteRelayEndpointCandidate, RemoteRelayDirectoryError> {
    let id = non_empty("endpoint.id", endpoint.id)?;
    let kind = non_empty("endpoint.kind", endpoint.kind)?;
    Ok(RemoteRelayEndpointCandidate {
        id,
        kind,
        host: endpoint.host.filter(|value| !value.trim().is_empty()),
        port: endpoint.port,
        web_url: endpoint.web_url.filter(|value| !value.trim().is_empty()),
        adp_url: endpoint.adp_url.filter(|value| !value.trim().is_empty()),
        relay_host_id: endpoint
            .relay_host_id
            .filter(|value| !value.trim().is_empty()),
        auth_required: endpoint.auth_required,
        last_seen_unix: if endpoint.last_seen_unix == 0 {
            now_unix
        } else {
            endpoint.last_seen_unix
        },
    })
}

fn normalize_upstream_base_url(raw: &str) -> Result<String, RemoteRelayDirectoryError> {
    let value = raw.trim();
    let url = Url::parse(value).map_err(|error| {
        RemoteRelayDirectoryError::invalid(format!("upstreamBaseUrl is invalid: {error}"))
    })?;
    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(RemoteRelayDirectoryError::invalid(format!(
                "upstreamBaseUrl scheme `{other}` is not supported"
            )));
        }
    }
    if url.host_str().is_none() {
        return Err(RemoteRelayDirectoryError::invalid(
            "upstreamBaseUrl requires a host",
        ));
    }
    Ok(url.to_string().trim_end_matches('/').to_owned())
}

fn join_upstream_url(
    base_url: &str,
    relative_path: &str,
) -> Result<Url, RemoteRelayDirectoryError> {
    let mut base = Url::parse(base_url).map_err(|error| {
        RemoteRelayDirectoryError::invalid(format!("upstream base URL is invalid: {error}"))
    })?;
    if !base.path().ends_with('/') {
        let path = format!("{}/", base.path().trim_end_matches('/'));
        base.set_path(&path);
    }
    base.join(relative_path.trim_start_matches('/'))
        .map_err(|error| {
            RemoteRelayDirectoryError::invalid(format!("upstream path join failed: {error}"))
        })
}

fn http_url_to_ws_url(mut url: Url) -> Result<String, RemoteRelayDirectoryError> {
    let scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        other => {
            return Err(RemoteRelayDirectoryError::invalid(format!(
                "cannot convert upstream scheme `{other}` to websocket"
            )));
        }
    };
    url.set_scheme(scheme).map_err(|_| {
        RemoteRelayDirectoryError::invalid("failed to convert upstream URL to websocket")
    })?;
    Ok(url.to_string())
}

fn remote_relay_bad_request(
    error: RemoteRelayDirectoryError,
) -> (StatusCode, Json<RemoteRelayErrorBody>) {
    (
        StatusCode::BAD_REQUEST,
        Json(RemoteRelayErrorBody {
            code: error.code,
            message: error.message,
        }),
    )
}

fn remote_relay_error_response(
    status: StatusCode,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Response {
    (
        status,
        Json(RemoteRelayErrorBody {
            code: code.into(),
            message: message.into(),
        }),
    )
        .into_response()
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
