use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::directory_socket::directory_subscription;
use crate::http_tunnel::{proxy_http_path, proxy_http_root};
use crate::model::{AgentHeartbeat, AuthRequest, AuthResponse, ErrorBody};
use crate::store::{RelayStore, RelayStoreError};
use crate::tunnel::RelayTunnelRegistry;
use crate::websocket_tunnel::{control_tunnel, data_tunnel, error_tunnel, proxy_adp};
use crate::websocket_tunnel::{proxy_websocket_path, proxy_websocket_root};

const SESSION_COOKIE: &str = "freehand_relay_session";

#[derive(Debug, Clone)]
pub struct RelayServiceConfig {
    pub presence_lease_seconds: u64,
    pub secure_cookie: bool,
}

#[derive(Clone)]
pub(crate) struct RelayState {
    pub(crate) store: Arc<Mutex<RelayStore>>,
    pub(crate) tunnels: Arc<Mutex<RelayTunnelRegistry>>,
    pub(crate) exchange_sequence: Arc<AtomicU64>,
    pub(crate) presence_updates: watch::Sender<u64>,
    pub(crate) config: RelayServiceConfig,
}

pub struct RelayService {
    state: RelayState,
}

impl RelayService {
    pub fn new(store: RelayStore, config: RelayServiceConfig) -> Result<Self, RelayStoreError> {
        let (presence_updates, _) = watch::channel(0);
        Ok(Self {
            state: RelayState {
                store: Arc::new(Mutex::new(store)),
                tunnels: Arc::new(Mutex::new(RelayTunnelRegistry::default())),
                exchange_sequence: Arc::new(AtomicU64::new(1)),
                presence_updates,
                config,
            },
        })
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/relay/health", get(health))
            .route("/relay/api/auth/register", post(register))
            .route("/relay/api/auth/login", post(login))
            .route("/relay/api/auth/me", get(me))
            .route("/relay/api/agents/heartbeat", post(heartbeat))
            .route("/relay/api/agents", get(directory))
            .route("/relay/api/agents/subscribe", get(directory_subscription))
            .route("/relay/tunnel/control/{agent_id}", get(control_tunnel))
            .route("/relay/tunnel/data/{agent_id}", get(data_tunnel))
            .route("/relay/tunnel/error/{agent_id}", get(error_tunnel))
            .route("/relay/agents/{agent_id}/adp", get(proxy_adp))
            .route(
                "/relay/agents/{agent_id}/connect",
                get(proxy_websocket_root),
            )
            .route(
                "/relay/agents/{agent_id}/connect/{*path}",
                get(proxy_websocket_path),
            )
            .route("/relay/agents/{agent_id}", any(proxy_http_root))
            .route("/relay/agents/{agent_id}/", any(proxy_http_root))
            .route("/relay/agents/{agent_id}/{*path}", any(proxy_http_path))
            .with_state(self.state.clone())
    }

    pub async fn serve(self, listener: TcpListener) -> Result<(), RelayStoreError> {
        axum::serve(listener, self.router())
            .await
            .map_err(|error| RelayStoreError::Io(error.to_string()))
    }
}

async fn health() -> &'static str {
    "ok"
}

async fn register(State(state): State<RelayState>, Json(request): Json<AuthRequest>) -> Response {
    let now = match now_unix() {
        Ok(now) => now,
        Err(error) => return error_response(error),
    };
    let result = with_store_mut(&state, move |store| {
        store.register(request.username, request.password, now)
    })
    .await;
    auth_response(result, StatusCode::CREATED, state.config.secure_cookie)
}

async fn login(State(state): State<RelayState>, Json(request): Json<AuthRequest>) -> Response {
    let now = match now_unix() {
        Ok(now) => now,
        Err(error) => return error_response(error),
    };
    let result = with_store_mut(&state, move |store| {
        store.login(request.username, request.password, now)
    })
    .await;
    auth_response(result, StatusCode::OK, state.config.secure_cookie)
}

async fn me(State(state): State<RelayState>, headers: HeaderMap) -> Response {
    match authenticated_account(&state, &headers).await {
        Ok(account_id) => Json(serde_json::json!({ "accountId": account_id })).into_response(),
        Err(error) => error_response(error),
    }
}

async fn heartbeat(
    State(state): State<RelayState>,
    headers: HeaderMap,
    Json(heartbeat): Json<AgentHeartbeat>,
) -> Response {
    let account_id = match authenticated_account(&state, &headers).await {
        Ok(account_id) => account_id,
        Err(error) => return error_response(error),
    };
    match record_heartbeat(&state, account_id, heartbeat).await {
        Ok(presence) => (StatusCode::ACCEPTED, Json(presence)).into_response(),
        Err(error) => error_response(error),
    }
}

async fn directory(State(state): State<RelayState>, headers: HeaderMap) -> Response {
    let account_id = match authenticated_account(&state, &headers).await {
        Ok(account_id) => account_id,
        Err(error) => return error_response(error),
    };
    match project_directory(&state, account_id).await {
        Ok(directory) => Json(directory).into_response(),
        Err(error) => error_response(error),
    }
}

pub(crate) async fn record_heartbeat(
    state: &RelayState,
    account_id: String,
    heartbeat: AgentHeartbeat,
) -> Result<crate::model::AgentPresence, RelayStoreError> {
    let now = now_unix()?;
    let presence = with_store_mut(state, move |store| {
        store.heartbeat(account_id, heartbeat, now)
    })
    .await?;
    notify_presence_changed(state);
    Ok(presence)
}

pub(crate) async fn record_disconnect(
    state: &RelayState,
    account_id: String,
    agent_id: String,
) -> Result<(), RelayStoreError> {
    with_store_mut(state, move |store| store.disconnect(&account_id, &agent_id)).await?;
    notify_presence_changed(state);
    Ok(())
}

pub(crate) async fn project_directory(
    state: &RelayState,
    account_id: String,
) -> Result<crate::model::AgentDirectory, RelayStoreError> {
    let now = now_unix()?;
    let lease_seconds = state.config.presence_lease_seconds;
    with_store(state, move |store| {
        Ok(store.directory(&account_id, now, lease_seconds))
    })
    .await
}

fn notify_presence_changed(state: &RelayState) {
    state
        .presence_updates
        .send_modify(|revision| *revision = revision.wrapping_add(1));
}

pub(crate) async fn authenticated_account(
    state: &RelayState,
    headers: &HeaderMap,
) -> Result<String, RelayStoreError> {
    let token = bearer_token(headers)
        .or_else(|| cookie_token(headers))
        .ok_or(RelayStoreError::Unauthorized)?
        .to_owned();
    with_store(state, move |store| store.authenticate(&token)).await
}

pub(crate) fn raw_agent_route_path(
    agent_id: &str,
    uri: &axum::http::Uri,
    namespace: Option<&str>,
) -> Result<String, RelayStoreError> {
    let route = uri.path().strip_prefix("/relay/agents/").ok_or_else(|| {
        RelayStoreError::Invalid("Relay Agent route prefix is invalid".to_owned())
    })?;
    let (raw_agent_id, remainder) = route.split_once('/').unwrap_or((route, ""));
    let decoded_agent_id = percent_encoding::percent_decode_str(raw_agent_id)
        .decode_utf8()
        .map_err(|_| {
            RelayStoreError::Invalid("Relay Agent route identity is invalid".to_owned())
        })?;
    if decoded_agent_id != agent_id {
        return Err(RelayStoreError::Invalid(
            "Relay Agent route identity does not match the typed route".to_owned(),
        ));
    }

    let remainder = match namespace {
        Some(namespace) => remainder
            .strip_prefix(namespace)
            .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
            .ok_or_else(|| {
                RelayStoreError::Invalid("Relay Agent route namespace is invalid".to_owned())
            })?,
        None => remainder,
    };
    Ok(if remainder.is_empty() {
        "/".to_owned()
    } else {
        format!(
            "/{remainder}",
            remainder = remainder.trim_start_matches('/')
        )
    })
}

fn auth_response(
    result: Result<AuthResponse, RelayStoreError>,
    status: StatusCode,
    secure_cookie: bool,
) -> Response {
    match result {
        Ok(auth) => {
            let secure_attribute = if secure_cookie { "; Secure" } else { "" };
            let cookie = format!(
                "{SESSION_COOKIE}={}; Path=/relay; HttpOnly{secure_attribute}; SameSite=Lax; Max-Age=2592000",
                auth.access_token,
            );
            let mut response = (status, Json(auth)).into_response();
            match HeaderValue::from_str(&cookie) {
                Ok(value) => {
                    response.headers_mut().insert(header::SET_COOKIE, value);
                    response
                }
                Err(error) => error_response(RelayStoreError::Io(error.to_string())),
            }
        }
        Err(error) => error_response(error),
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn cookie_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&format!("{SESSION_COOKIE}=")))
        .filter(|value| !value.is_empty())
}

pub(crate) async fn with_store<T>(
    state: &RelayState,
    operation: impl FnOnce(&RelayStore) -> Result<T, RelayStoreError> + Send + 'static,
) -> Result<T, RelayStoreError>
where
    T: Send + 'static,
{
    let store = Arc::clone(&state.store);
    tokio::task::spawn_blocking(move || {
        let store = store
            .lock()
            .map_err(|_| RelayStoreError::Io("relay store lock poisoned".to_owned()))?;
        operation(&store)
    })
    .await
    .map_err(|error| RelayStoreError::Io(format!("relay store executor failed: {error}")))?
}

pub(crate) async fn with_store_mut<T>(
    state: &RelayState,
    operation: impl FnOnce(&mut RelayStore) -> Result<T, RelayStoreError> + Send + 'static,
) -> Result<T, RelayStoreError>
where
    T: Send + 'static,
{
    let store = Arc::clone(&state.store);
    tokio::task::spawn_blocking(move || {
        let mut store = store
            .lock()
            .map_err(|_| RelayStoreError::Io("relay store lock poisoned".to_owned()))?;
        operation(&mut store)
    })
    .await
    .map_err(|error| RelayStoreError::Io(format!("relay store executor failed: {error}")))?
}

pub(crate) fn error_response(error: RelayStoreError) -> Response {
    let (status, code) = match error {
        RelayStoreError::Invalid(_) => (StatusCode::BAD_REQUEST, "relay_invalid_request"),
        RelayStoreError::Conflict => (StatusCode::CONFLICT, "relay_account_conflict"),
        RelayStoreError::StoreAlreadyExists => (StatusCode::CONFLICT, "relay_store_already_exists"),
        RelayStoreError::Unauthorized => (StatusCode::UNAUTHORIZED, "relay_unauthorized"),
        RelayStoreError::AgentNotFound => (StatusCode::NOT_FOUND, "relay_agent_not_found"),
        RelayStoreError::Upstream(_) => (StatusCode::BAD_GATEWAY, "relay_tunnel_failed"),
        RelayStoreError::Io(_) | RelayStoreError::Corrupt(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "relay_store_failed")
        }
    };
    (
        status,
        Json(ErrorBody {
            code: code.to_owned(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn now_unix() -> Result<u64, RelayStoreError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| RelayStoreError::Io(format!("system clock is before Unix epoch: {error}")))
}
