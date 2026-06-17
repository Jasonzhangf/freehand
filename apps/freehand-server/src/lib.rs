mod assets;
mod page;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
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
    TurnProjectionInput, UiCheckpointSnapshot, UiClientKind, UiCommand, UiCommandDispatchFailure,
    UiCommandDispatchPort, UiCommandDispatchReceipt, UiProjection, UiProtocolState,
    UiPublicTurnProjection, UiQueryResult, UiSubscriptionEvent, UiTurnProjection,
    build_command_dispatch_envelope, checkpoint_projection_from_runtime_summary,
    dispatch_port_failure, protocol_rejection, public_turn_projection, subscription_matches,
    subscription_selector, turn_projection_for_client, turn_projection_from_events,
};
use futures_util::stream;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

#[derive(Clone)]
struct WebUiState {
    protocol_state: Arc<Mutex<UiProtocolState>>,
    command_dispatch_port: Arc<dyn UiCommandDispatchPort>,
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
) -> Router {
    Router::new()
        .route("/", get(handle_root))
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
        .with_state(WebUiState {
            protocol_state,
            command_dispatch_port,
        })
}

pub async fn serve_webui_listener<F>(
    listener: TcpListener,
    protocol_state: Arc<Mutex<UiProtocolState>>,
    command_dispatch_port: Arc<dyn UiCommandDispatchPort>,
    shutdown: F,
) -> std::io::Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    axum::serve(
        listener,
        build_webui_router(protocol_state, command_dispatch_port),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

pub fn render_webui_smoke() -> String {
    page::render_webui_smoke()
}

pub fn seed_webui_protocol_state() -> UiProtocolState {
    let mut state = UiProtocolState::default();
    let projection = sample_slave_turn_projection();
    state.apply_turn_projection(projection);
    state.set_debug_state(sample_debug_snapshot());
    state.set_checkpoint_snapshot(sample_checkpoint_snapshot());
    state
}

async fn handle_root() -> Html<String> {
    Html(render_webui_smoke())
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
            UiQueryResult::Debug(Some(snapshot)) => snapshot,
            UiQueryResult::Debug(None) => return Err(StatusCode::NOT_FOUND),
            _ => return Err(StatusCode::BAD_REQUEST),
        };
        (UiProjection::Debug(snapshot), state.subscribe())
    };
    Ok(Sse::new(subscription_event_stream(
        Some(initial_projection),
        receiver,
        selector,
    )))
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
    use freehand_ui_protocol::{StaticUiCommandDispatchPort, UiCommand};
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
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("local addr");
            let protocol_state = Arc::new(Mutex::new(initial_state));
            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let protocol_state_for_task = Arc::clone(&protocol_state);
            let task = tokio::spawn(async move {
                let shutdown = async move {
                    let _ = shutdown_rx.await;
                };
                serve_webui_listener(
                    listener,
                    protocol_state_for_task,
                    Arc::new(StaticUiCommandDispatchPort::default()),
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
        assert!(html.contains("/assets/theme.css"));
        assert!(html.contains("/assets/webui.css"));
        assert!(html.contains("/assets/webui.js"));
        assert!(html.contains("data-turn-query=\"/ui/query/latest-active-turn\""));
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
        assert!(root_body.contains("/assets/theme.css"));
        assert!(root_body.contains("data-checkpoint-query=\"/ui/query/checkpoints\""));

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
        assert!(js_body.contains("await refreshTurn();"));
        assert!(js_body.contains("refreshCheckpoints"));

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
    async fn transport_command_ingress_smoke_accepts_mutation_and_rejects_query_route_misuse() {
        let server = TestServer::spawn().await;
        let client = Client::builder().build().expect("client");

        let accepted = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "run task".to_owned(),
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
}
