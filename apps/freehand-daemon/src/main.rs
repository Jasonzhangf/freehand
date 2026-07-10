use std::future::pending;
use std::sync::Arc;

use freehand_runtime::{
    ProductionWorkerRunner, RuntimeAgentBootstrap, RuntimeCommandDispatcher,
    load_default_runtime_agent,
};
use freehand_server::{parse_bind_arg, serve_webui_listener};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    match run().await {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    }
}

async fn run() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(usage());
    };
    match command.as_str() {
        "serve" => {
            let args: Vec<String> = args.collect();
            if args.len() < 2 || args[0] != "--agent" {
                return Err(usage());
            }
            let agent_name = args[1].clone();
            let trailing_args = args.into_iter().skip(2).collect::<Vec<_>>();
            let bootstrap = load_default_runtime_agent(&agent_name)
                .map_err(|err| format!("failed to load daemon agent: {err}"))?;
            if bootstrap.selected_agent.mode.as_str() == "slave" {
                if !trailing_args.is_empty() {
                    return Err("slave worker mode does not accept --bind".to_owned());
                }
                return run_worker_mode(agent_name, bootstrap).await;
            }
            let bind_addr = parse_bind_arg(trailing_args.into_iter())?;
            let dispatcher = RuntimeCommandDispatcher::from_selected_agent_with_live(
                &bootstrap.selected_agent,
                bootstrap.runtime_home,
                false,
            )
            .map(Arc::new)
            .map_err(|err| format!("failed to build runtime dispatcher: {err}"))?;
            let listener = TcpListener::bind(bind_addr)
                .await
                .map_err(|err| format!("failed to bind {bind_addr}: {err}"))?;
            let local_addr = listener
                .local_addr()
                .map_err(|err| format!("failed to read local addr: {err}"))?;
            println!("freehand-daemon listening on http://{local_addr}");
            let ui_state = dispatcher.ui_state();
            let dispatch_port: Arc<dyn freehand_ui_protocol::UiCommandDispatchPort> =
                dispatcher.clone();
            let query_port: Arc<dyn freehand_ui_protocol::UiRuntimeQueryPort> = dispatcher.clone();
            serve_webui_listener(
                listener,
                ui_state,
                dispatch_port,
                query_port,
                pending::<()>(),
            )
            .await
            .map_err(|err| format!("daemon server error: {err}"))?;
            Ok(String::new())
        }
        _ => Err(usage()),
    }
}

async fn run_worker_mode(
    agent_name: String,
    bootstrap: RuntimeAgentBootstrap,
) -> Result<String, String> {
    let runner = ProductionWorkerRunner::from_selected_agent(
        bootstrap.selected_agent,
        bootstrap.runtime_home,
    )
    .map_err(|err| format!("failed to build production worker runner: {err}"))?;
    println!("freehand-daemon worker runner started for {agent_name}");
    run_blocking_worker_service(move || runner.run()).await?;
    Ok(String::new())
}

async fn run_blocking_worker_service<F>(service: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), freehand_runtime::ProductionWorkerRunnerError> + Send + 'static,
{
    tokio::task::spawn_blocking(service)
        .await
        .map_err(|error| format!("worker runner task failed: {error}"))?
        .map_err(|error| format!("worker runner stopped: {error}"))
}

fn usage() -> String {
    "usage: freehand-daemon serve --agent <name> [--bind HOST:PORT]".to_owned()
}

#[cfg(test)]
fn build_runtime_dispatcher_from_default_config(
    agent_name: &str,
) -> Result<Arc<RuntimeCommandDispatcher>, String> {
    RuntimeCommandDispatcher::from_default_config(agent_name)
        .map(Arc::new)
        .map_err(|err| format!("failed to build runtime dispatcher: {err}"))
}

#[cfg(test)]
fn build_worker_runner_from_default_config(
    agent_name: &str,
) -> Result<ProductionWorkerRunner, String> {
    ProductionWorkerRunner::from_default_config(agent_name)
        .map_err(|err| format!("failed to build production worker runner: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{FeatureId, SessionId, TraceId, TurnId};
    use freehand_metadata::{
        MetadataCenter, MetadataEntry, MetadataEnvelope, MetadataId, MetadataKind, MetadataSubject,
        MetadataWriteNode, MetadataWriteOwner,
    };
    use freehand_task::{
        TaskActor, TaskCreateRequest, TaskDispatchRequest, TaskParentRef, TaskRuntime,
        TaskWatermark,
    };
    use freehand_ui_protocol::{
        UiAdpRequest, UiAdpResponse, UiCheckpointSnapshot, UiClientKind, UiCommand,
        UiCommandDispatchFailure, UiCommandDispatchReceipt, UiPublicTurnProjection, UiQueryResult,
    };
    use futures_util::{SinkExt, StreamExt};
    use reqwest::Client;
    use serde_json::Value;
    use serial_test::serial;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener as StdTcpListener;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tokio::sync::oneshot;
    use tokio::time::timeout;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    struct TestServer {
        base_url: String,
        home: PathBuf,
        cleanup_on_stop: bool,
        shutdown: Option<oneshot::Sender<()>>,
        task: tokio::task::JoinHandle<()>,
    }

    impl TestServer {
        async fn spawn(config_text: String) -> Self {
            let home = write_test_home(&config_text).expect("test home");
            Self::spawn_existing_home(home, true).await
        }

        async fn spawn_existing_home(home: PathBuf, cleanup_on_stop: bool) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("local addr");
            let _guard = HOME_LOCK.lock().unwrap_or_else(|err| err.into_inner());
            let old_home = env::var_os("HOME");
            let old_pair_token = env::var_os("FREEHAND_PAIR_TOKEN_SHARED");
            unsafe { env::set_var("HOME", &home) };
            unsafe { env::set_var("FREEHAND_PAIR_TOKEN_SHARED", "pair-token-shared") };
            let dispatcher =
                build_runtime_dispatcher_from_default_config("master").expect("runtime dispatcher");
            restore_env(old_home, "FREEHAND_PAIR_TOKEN_SHARED", old_pair_token);
            let ui_state = dispatcher.ui_state();
            let dispatch_port: Arc<dyn freehand_ui_protocol::UiCommandDispatchPort> =
                dispatcher.clone();
            let query_port: Arc<dyn freehand_ui_protocol::UiRuntimeQueryPort> = dispatcher.clone();
            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let task = tokio::spawn(async move {
                let shutdown = async move {
                    let _ = shutdown_rx.await;
                };
                serve_webui_listener(listener, ui_state, dispatch_port, query_port, shutdown)
                    .await
                    .expect("serve");
            });
            Self {
                base_url: format!("http://{addr}"),
                home,
                cleanup_on_stop,
                shutdown: Some(shutdown_tx),
                task,
            }
        }

        async fn stop(mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
            self.task.await.expect("join");
            if self.cleanup_on_stop {
                let _ = fs::remove_dir_all(&self.home);
            }
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

    async fn read_sse_event_matching(
        response: &mut reqwest::Response,
        buffer: &mut String,
        needle: &str,
    ) -> String {
        loop {
            let event = read_next_sse_event(response, buffer).await;
            if event.contains(needle) {
                return event;
            }
        }
    }

    fn enter_temp_workspace() -> TempWorkspace<'static> {
        let lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = env::current_dir().expect("current dir");
        let old_workspace_root = env::var_os("FREEHAND_WORKSPACE_ROOT");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "freehand-daemon-workspace-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp workspace");
        env::set_current_dir(&root).expect("set cwd");
        unsafe { env::set_var("FREEHAND_WORKSPACE_ROOT", &root) };
        TempWorkspace {
            root,
            original,
            old_workspace_root,
            _lock: lock,
        }
    }

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct TempWorkspace<'a> {
        root: PathBuf,
        original: PathBuf,
        old_workspace_root: Option<OsString>,
        _lock: std::sync::MutexGuard<'a, ()>,
    }

    impl TempWorkspace<'_> {
        fn root(&self) -> &std::path::Path {
            &self.root
        }
    }

    impl Drop for TempWorkspace<'_> {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.original);
            match self.old_workspace_root.clone() {
                Some(value) => unsafe { env::set_var("FREEHAND_WORKSPACE_ROOT", value) },
                None => unsafe { env::remove_var("FREEHAND_WORKSPACE_ROOT") },
            }
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn checkpoint_id_from_home(home: &std::path::Path) -> String {
        let path = home
            .join(".freehand")
            .join("ledgers")
            .join("checkpoints")
            .join("master")
            .join("runtime-session-master.jsonl");
        let raw = fs::read_to_string(path).expect("read checkpoint ledger");
        raw.lines()
            .next()
            .and_then(|line| serde_json::from_str::<Value>(line).ok())
            .and_then(|row| {
                row.get("checkpoint_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .expect("checkpoint id")
    }

    async fn next_adp_response(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        label: &str,
    ) -> UiAdpResponse {
        let message = timeout(Duration::from_secs(10), socket.next())
            .await
            .unwrap_or_else(|_| panic!("adp response timeout while waiting for {label}"))
            .expect("adp response")
            .expect("adp websocket message");
        match message {
            Message::Text(text) => serde_json::from_str(&text).expect("adp response json"),
            other => panic!("unexpected ADP websocket message: {other:?}"),
        }
    }

    async fn next_adp_response_matching(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        label: &str,
        matches: impl Fn(&UiAdpResponse) -> bool,
    ) -> UiAdpResponse {
        loop {
            let response = next_adp_response(socket, label).await;
            if matches(&response) {
                return response;
            }
        }
    }

    async fn collect_adp_receipt_and_turn_event(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        receipt_request_id: &str,
        subscription_request_id: &str,
        event_needle: &str,
    ) -> (
        UiCommandDispatchReceipt,
        freehand_ui_protocol::UiSubscriptionEvent,
    ) {
        let mut receipt = None;
        let mut event = None;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while receipt.is_none() || event.is_none() {
            let now = tokio::time::Instant::now();
            assert!(now < deadline, "ADP receipt/event collection timeout");
            let response = timeout(deadline - now, socket.next())
                .await
                .expect("ADP response timeout")
                .expect("ADP response")
                .expect("ADP websocket message");
            let Message::Text(text) = response else {
                panic!("unexpected ADP websocket message: {response:?}");
            };
            let response: UiAdpResponse = serde_json::from_str(&text).expect("adp response json");
            match response {
                UiAdpResponse::CommandReceipt {
                    request_id,
                    receipt: got_receipt,
                } if request_id == receipt_request_id => {
                    receipt = Some(got_receipt);
                }
                UiAdpResponse::SubscriptionEvent {
                    request_id,
                    event: got_event,
                } if request_id == subscription_request_id
                    && serde_json::to_string(&got_event)
                        .expect("event json")
                        .contains(event_needle) =>
                {
                    event = Some(got_event);
                }
                UiAdpResponse::Failure {
                    request_id,
                    failure,
                } => panic!("unexpected ADP failure {request_id}: {failure:?}"),
                _ => {}
            }
        }
        (receipt.expect("receipt"), event.expect("event"))
    }

    async fn send_adp_command_and_wait_receipt(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        request_id: &str,
        command: UiCommand,
    ) -> UiCommandDispatchReceipt {
        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Command {
                    request_id: request_id.to_owned(),
                    command,
                })
                .expect("command json")
                .into(),
            ))
            .await
            .expect("send command");
        match next_adp_response_matching(socket, "command receipt", |response| {
            matches!(
                response,
                UiAdpResponse::CommandReceipt { request_id: got, .. } if got == request_id
            ) || matches!(
                response,
                UiAdpResponse::Failure { request_id: got, .. } if got == request_id
            )
        })
        .await
        {
            UiAdpResponse::CommandReceipt { receipt, .. } => receipt,
            UiAdpResponse::Failure { failure, .. } => {
                panic!("unexpected ADP command failure {request_id}: {failure:?}")
            }
            other => panic!("unexpected ADP command response: {other:?}"),
        }
    }

    async fn send_adp_query_and_wait_result(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        request_id: &str,
        query: UiCommand,
    ) -> UiQueryResult {
        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Query {
                    request_id: request_id.to_owned(),
                    query,
                })
                .expect("query json")
                .into(),
            ))
            .await
            .expect("send query");
        match next_adp_response_matching(socket, "query result", |response| {
            matches!(
                response,
                UiAdpResponse::QueryResult { request_id: got, .. } if got == request_id
            ) || matches!(
                response,
                UiAdpResponse::Failure { request_id: got, .. } if got == request_id
            )
        })
        .await
        {
            UiAdpResponse::QueryResult { result, .. } => result,
            UiAdpResponse::Failure { failure, .. } => {
                panic!("unexpected ADP query failure {request_id}: {failure:?}")
            }
            other => panic!("unexpected ADP query response: {other:?}"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn daemon_submit_input_updates_runtime_backed_latest_turn_query() {
        let (provider_url, request_rx, provider_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("tool done"),
            ],
        );
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let client = Client::builder().build().expect("client");

        let accepted = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "daemon turn".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("command response");
        if accepted.status() != reqwest::StatusCode::ACCEPTED {
            let status = accepted.status();
            let body = accepted.text().await.expect("failure body");
            panic!("expected 202 from daemon submit, got {status}: {body}");
        }
        let accepted: UiCommandDispatchReceipt = accepted.json().await.expect("receipt json");
        assert_eq!(
            accepted.dispatch_status,
            "reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 restored_closed_turns=0"
        );
        let first_request = request_rx.recv().expect("first request");
        let second_request = request_rx.recv().expect("second request");
        provider_handle.join().expect("join provider");
        assert!(first_request.starts_with("POST /v1/messages HTTP/1.1"));
        assert!(first_request.contains("\"name\":\"read_file\""));
        assert!(second_request.contains("\"type\":\"tool_result\""));
        assert!(second_request.contains("toolu_read_1"));
        assert!(second_request.contains("Cargo.toml"));

        let queried = client
            .get(format!("{}/ui/query/latest-active-turn", server.base_url))
            .send()
            .await
            .expect("query response");
        assert_eq!(queried.status(), reqwest::StatusCode::OK);
        let queried: UiPublicTurnProjection = queried.json().await.expect("query json");
        assert_eq!(queried.turn.turn_id, TurnId::new("runtime-turn-1-r2"));
        assert_eq!(queried.turn.source.source_node_id, "master-node");
        assert_eq!(queried.turn.user_text.as_deref(), Some("daemon turn"));
        assert_eq!(queried.public_conversation[0].body, "daemon turn");
        assert!(
            queried
                .turn
                .terminal_text
                .as_deref()
                .is_some_and(|text| text.contains("Summary: tool done"))
        );

        server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_adp_websocket_controls_command_query_and_subscription() {
        let (provider_url, request_rx, provider_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("adp done"),
            ],
        );
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let ws_url = server.base_url.replace("http://", "ws://") + "/adp";
        let (mut socket, _) = connect_async(ws_url).await.expect("connect adp");

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Subscribe {
                    request_id: "sub-1".to_owned(),
                    subscription: UiCommand::SubscribeLatestActiveTurn {
                        client: UiClientKind::WebUi,
                    },
                })
                .expect("subscribe json")
                .into(),
            ))
            .await
            .expect("send subscribe");
        match next_adp_response(&mut socket, "subscription accepted").await {
            UiAdpResponse::SubscriptionAccepted {
                request_id,
                selector,
            } => {
                assert_eq!(request_id, "sub-1");
                assert_eq!(
                    selector.stream_kind,
                    freehand_ui_protocol::UiStreamKind::Turn
                );
            }
            other => panic!("unexpected ADP subscription ack: {other:?}"),
        }
        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Query {
                    request_id: "pre-query-1".to_owned(),
                    query: UiCommand::QueryLatestActiveTurn,
                })
                .expect("pre query json")
                .into(),
            ))
            .await
            .expect("send pre query");
        match next_adp_response(&mut socket, "pre-query result").await {
            UiAdpResponse::QueryResult {
                request_id,
                result: UiQueryResult::Turn(None),
            } => assert_eq!(request_id, "pre-query-1"),
            other => panic!("unexpected ADP pre-query response: {other:?}"),
        }

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Command {
                    request_id: "cmd-1".to_owned(),
                    command: UiCommand::SubmitUserInput {
                        text: "daemon adp turn".to_owned(),
                        session_id: None,
                        cwd: None,
                    },
                })
                .expect("command json")
                .into(),
            ))
            .await
            .expect("send command");
        let (receipt, event) =
            collect_adp_receipt_and_turn_event(&mut socket, "cmd-1", "sub-1", "daemon adp turn")
                .await;
        let _ = request_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("first provider request");
        let _ = request_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("second provider request");
        provider_handle.join().expect("join provider");
        assert_eq!(
            receipt.dispatch_status,
            "reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 restored_closed_turns=0"
        );
        assert!(
            serde_json::to_string(&event)
                .expect("event json")
                .contains("daemon adp turn")
        );

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Query {
                    request_id: "query-1".to_owned(),
                    query: UiCommand::QueryLatestActiveTurn,
                })
                .expect("query json")
                .into(),
            ))
            .await
            .expect("send query");
        let queried = next_adp_response_matching(&mut socket, "query result", |response| {
            matches!(
                response,
                UiAdpResponse::QueryResult { request_id, .. } if request_id == "query-1"
            )
        })
        .await;
        match queried {
            UiAdpResponse::QueryResult {
                request_id,
                result: UiQueryResult::Turn(Some(turn)),
            } => {
                assert_eq!(request_id, "query-1");
                assert_eq!(turn.user_text.as_deref(), Some("daemon adp turn"));
                assert!(
                    turn.terminal_text
                        .as_deref()
                        .is_some_and(|text| text.contains("Summary: adp done"))
                );
            }
            other => panic!("unexpected ADP query response: {other:?}"),
        }

        let _ = socket.close(None).await;
        server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_adp_manages_sessions_and_rolls_back_effective_transcript() {
        let (provider_url, request_rx, provider_handle) = spawn_sequence_server(
            "application/json",
            vec![
                complete_single_response("first session turn"),
                complete_single_response("second session turn"),
            ],
        );
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let ws_url = server.base_url.replace("http://", "ws://") + "/adp";
        let (mut socket, _) = connect_async(ws_url).await.expect("connect adp");
        let session_id = SessionId::new("daemon-adp-session-crud-rollback");

        let create = send_adp_command_and_wait_receipt(
            &mut socket,
            "session-create-1",
            UiCommand::CreateSession {
                session_id: session_id.clone(),
                title: Some("ADP rollback draft".to_owned()),
                cwd: None,
            },
        )
        .await;
        assert_eq!(create.target_feature_id, "reason.persistence");

        let rename = send_adp_command_and_wait_receipt(
            &mut socket,
            "session-rename-1",
            UiCommand::RenameSession {
                session_id: session_id.clone(),
                title: "ADP rollback renamed".to_owned(),
            },
        )
        .await;
        assert_eq!(rename.dispatch_status, "session_metadata_updated");

        send_adp_command_and_wait_receipt(
            &mut socket,
            "session-archive-1",
            UiCommand::ArchiveSession {
                session_id: session_id.clone(),
            },
        )
        .await;
        match send_adp_query_and_wait_result(
            &mut socket,
            "session-active-list-1",
            UiCommand::QuerySessionList,
        )
        .await
        {
            UiQueryResult::SessionList(list) => assert!(
                !list
                    .sessions
                    .iter()
                    .any(|session| session.session_id == session_id)
            ),
            other => panic!("unexpected active session list: {other:?}"),
        }
        match send_adp_query_and_wait_result(
            &mut socket,
            "session-archived-list-1",
            UiCommand::QueryArchivedSessionList,
        )
        .await
        {
            UiQueryResult::SessionList(list) => {
                let archived = list
                    .sessions
                    .iter()
                    .find(|session| session.session_id == session_id)
                    .expect("archived session");
                assert!(archived.archived);
                assert_eq!(archived.title.as_deref(), Some("ADP rollback renamed"));
            }
            other => panic!("unexpected archived session list: {other:?}"),
        }

        send_adp_command_and_wait_receipt(
            &mut socket,
            "session-restore-1",
            UiCommand::RestoreSession {
                session_id: session_id.clone(),
            },
        )
        .await;
        match send_adp_query_and_wait_result(
            &mut socket,
            "session-active-list-2",
            UiCommand::QuerySessionList,
        )
        .await
        {
            UiQueryResult::SessionList(list) => {
                let restored = list
                    .sessions
                    .iter()
                    .find(|session| session.session_id == session_id)
                    .expect("restored session");
                assert!(!restored.archived);
                assert_eq!(restored.title.as_deref(), Some("ADP rollback renamed"));
            }
            other => panic!("unexpected restored session list: {other:?}"),
        }

        send_adp_command_and_wait_receipt(
            &mut socket,
            "session-submit-1",
            UiCommand::SubmitUserInput {
                text: "first prompt for rollback session".to_owned(),
                session_id: Some(session_id.clone()),
                cwd: None,
            },
        )
        .await;
        send_adp_command_and_wait_receipt(
            &mut socket,
            "session-submit-2",
            UiCommand::SubmitUserInput {
                text: "second prompt to roll back".to_owned(),
                session_id: Some(session_id.clone()),
                cwd: None,
            },
        )
        .await;
        let _ = request_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("first provider request");
        let _ = request_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("second provider request");
        provider_handle.join().expect("join provider");

        match send_adp_query_and_wait_result(
            &mut socket,
            "session-turns-before-rollback",
            UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            },
        )
        .await
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 2);
                assert_eq!(
                    transcript.turns[0].user_text.as_deref(),
                    Some("first prompt for rollback session")
                );
                assert_eq!(
                    transcript.turns[1].user_text.as_deref(),
                    Some("second prompt to roll back")
                );
            }
            other => panic!("unexpected transcript before rollback: {other:?}"),
        }

        let rollback = send_adp_command_and_wait_receipt(
            &mut socket,
            "session-rollback-1",
            UiCommand::RollbackLatestSessionTurn {
                session_id: session_id.clone(),
            },
        )
        .await;
        assert_eq!(rollback.target_feature_id, "reason.persistence");
        assert!(
            rollback
                .dispatch_status
                .contains("session_turn_rolled_back:runtime-turn-2")
        );

        match send_adp_query_and_wait_result(
            &mut socket,
            "session-turns-after-rollback",
            UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            },
        )
        .await
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 1);
                assert_eq!(
                    transcript.turns[0].user_text.as_deref(),
                    Some("first prompt for rollback session")
                );
            }
            other => panic!("unexpected transcript after rollback: {other:?}"),
        }

        let _ = socket.close(None).await;
        server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_adp_rejects_query_sent_as_command_frame() {
        let (provider_url, _request_rx, provider_handle) =
            spawn_sequence_server("application/json", Vec::new());
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let ws_url = server.base_url.replace("http://", "ws://") + "/adp";
        let (mut socket, _) = connect_async(ws_url).await.expect("connect adp");

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Command {
                    request_id: "bad-cmd-1".to_owned(),
                    command: UiCommand::QueryLatestActiveTurn,
                })
                .expect("bad command json")
                .into(),
            ))
            .await
            .expect("send bad command");

        match next_adp_response(&mut socket, "query-as-command failure").await {
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                assert_eq!(request_id, "bad-cmd-1");
                assert_eq!(failure.code, "ingress_command_kind_mismatch");
                assert!(!failure.retryable);
            }
            other => panic!("unexpected ADP failure response: {other:?}"),
        }

        let _ = socket.close(None).await;
        server.stop().await;
        provider_handle.join().expect("join provider");
    }

    #[tokio::test]
    #[serial]
    async fn daemon_adp_queries_runtime_task_truth() {
        let home =
            write_test_home(&master_config_text("https://example.invalid")).expect("test home");
        let runtime_home = home.join(".freehand");
        let task_runtime =
            TaskRuntime::boot(&runtime_home, freehand_contracts::AgentId::new("master"))
                .expect("task runtime");
        let outcome = task_runtime
            .create_task(TaskCreateRequest {
                task_id: Some(freehand_task::TaskId::new("adp-task-1")),
                title: "ADP task query".to_owned(),
                content: "Expose task truth through ADP".to_owned(),
                goal: "ADP reads persisted task snapshots".to_owned(),
                deliverables: vec!["task list projection".to_owned()],
                acceptance: vec!["history projection".to_owned()],
                priority: 80,
                target_cwd: Some("/tmp".to_owned()),
                dispatch: TaskDispatchRequest::None,
                parent: TaskParentRef {
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                actor: TaskActor {
                    agent_id: freehand_contracts::AgentId::new("master"),
                    source: "daemon_adp_test".to_owned(),
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                watermark: TaskWatermark {
                    metadata_id: None,
                    hook: Some("daemon_adp_test".to_owned()),
                    action_tool_call_id: None,
                },
            })
            .expect("create task");
        assert_eq!(outcome.task.task_id.as_str(), "adp-task-1");

        let server = TestServer::spawn_existing_home(home, true).await;
        let ws_url = server.base_url.replace("http://", "ws://") + "/adp";
        let (mut socket, _) = connect_async(ws_url).await.expect("connect adp");

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Query {
                    request_id: "task-list-1".to_owned(),
                    query: UiCommand::QueryTaskList {
                        status: Some("waiting_agent".to_owned()),
                        agent_id: None,
                    },
                })
                .expect("task list query json")
                .into(),
            ))
            .await
            .expect("send task list query");
        match next_adp_response(&mut socket, "task list query").await {
            UiAdpResponse::QueryResult {
                request_id,
                result: UiQueryResult::TaskList(list),
            } => {
                assert_eq!(request_id, "task-list-1");
                assert_eq!(list.source_agent_id.as_str(), "master");
                assert_eq!(list.tasks.len(), 1);
                assert_eq!(list.tasks[0].task_id, "adp-task-1");
                assert_eq!(list.tasks[0].status, "waiting_agent");
                assert_eq!(list.tasks[0].priority, 80);
            }
            other => panic!("unexpected task list response: {other:?}"),
        }

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Query {
                    request_id: "task-history-1".to_owned(),
                    query: UiCommand::QueryTaskHistory {
                        task_id: "adp-task-1".to_owned(),
                    },
                })
                .expect("task history query json")
                .into(),
            ))
            .await
            .expect("send task history query");
        match next_adp_response(&mut socket, "task history query").await {
            UiAdpResponse::QueryResult {
                request_id,
                result: UiQueryResult::TaskHistory(history),
            } => {
                assert_eq!(request_id, "task-history-1");
                assert_eq!(history.task_id, "adp-task-1");
                assert_eq!(history.events.len(), 2);
                assert_eq!(history.events[0].event_type, "TaskCreated");
                assert_eq!(history.events[1].event_type, "TaskWaitingAgent");
            }
            other => panic!("unexpected task history response: {other:?}"),
        }

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Query {
                    request_id: "task-missing-1".to_owned(),
                    query: UiCommand::QueryTaskHistory {
                        task_id: "missing-task".to_owned(),
                    },
                })
                .expect("missing task history query json")
                .into(),
            ))
            .await
            .expect("send missing task history query");
        match next_adp_response(&mut socket, "missing task history query").await {
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                assert_eq!(request_id, "task-missing-1");
                assert_eq!(failure.code, "command_dispatch_target_not_found");
                assert!(!failure.retryable);
            }
            other => panic!("unexpected missing task response: {other:?}"),
        }

        let _ = socket.close(None).await;
        server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_adp_queries_runtime_error_center_truth() {
        let home =
            write_test_home(&master_config_text("https://example.invalid")).expect("test home");
        let runtime_home = home.join(".freehand");
        let session_id = SessionId::new("runtime-session-master");
        let trace_id = TraceId::new("runtime-trace-adp-error");
        let turn_id = TurnId::new("runtime-turn-adp-error");
        let ledger_path = runtime_home
            .join("ledgers")
            .join("metadata")
            .join("master")
            .join(format!("{}.jsonl", session_id.as_str()));
        let mut center = MetadataCenter::with_ledger_path(ledger_path).expect("metadata center");
        center
            .write(
                MetadataEnvelope::new(
                    MetadataId::new("error.center:runtime-trace-adp-error:provider"),
                    MetadataKind::RuntimeState,
                    MetadataWriteOwner {
                        feature_id: FeatureId::new("error.center"),
                        crate_name: "freehand-control".to_owned(),
                        module_path: "freehand_control".to_owned(),
                        symbol_path: "classify_error_center_failure".to_owned(),
                    },
                    MetadataWriteNode {
                        pipeline_node: "RuntimeLive03ProviderResponseRaw".to_owned(),
                        runtime_node_id: None,
                    },
                    MetadataSubject {
                        agent_id: Some(freehand_contracts::AgentId::new("master")),
                        session_id: Some(session_id.clone()),
                        turn_id: Some(turn_id.clone()),
                        trace_id: trace_id.clone(),
                    },
                    vec![
                        MetadataEntry {
                            key: "error.domain".to_owned(),
                            value: serde_json::json!("provider"),
                        },
                        MetadataEntry {
                            key: "error.class".to_owned(),
                            value: serde_json::json!("recoverable"),
                        },
                        MetadataEntry {
                            key: "error.code".to_owned(),
                            value: serde_json::json!("provider_executor_failure"),
                        },
                        MetadataEntry {
                            key: "error.source_owner".to_owned(),
                            value: serde_json::json!("provider.reason-live-bridge"),
                        },
                        MetadataEntry {
                            key: "error.source_pipeline_node".to_owned(),
                            value: serde_json::json!("RuntimeLive03ProviderResponseRaw"),
                        },
                        MetadataEntry {
                            key: "error.recovery_action".to_owned(),
                            value: serde_json::json!("fail_turn"),
                        },
                        MetadataEntry {
                            key: "error.retry_index".to_owned(),
                            value: serde_json::json!(0),
                        },
                        MetadataEntry {
                            key: "error.retry_cap".to_owned(),
                            value: serde_json::json!(0),
                        },
                        MetadataEntry {
                            key: "error.public_visibility".to_owned(),
                            value: serde_json::json!("public_summary"),
                        },
                        MetadataEntry {
                            key: "error.owner_target".to_owned(),
                            value: serde_json::json!("provider.semantic"),
                        },
                        MetadataEntry {
                            key: "error.repair_fields".to_owned(),
                            value: serde_json::json!([]),
                        },
                        MetadataEntry {
                            key: "error.raw_hash".to_owned(),
                            value: serde_json::json!("provider-hash-only"),
                        },
                    ],
                )
                .expect("error center envelope"),
            )
            .expect("write error center metadata");

        let server = TestServer::spawn_existing_home(home, true).await;
        let ws_url = server.base_url.replace("http://", "ws://") + "/adp";
        let (mut socket, _) = connect_async(ws_url).await.expect("connect adp");

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Query {
                    request_id: "error-center-1".to_owned(),
                    query: UiCommand::QueryErrorCenterEvents {
                        session_id: session_id.clone(),
                        trace_id: Some(trace_id.as_str().to_owned()),
                        turn_id: Some(turn_id.clone()),
                        domain: Some("provider".to_owned()),
                    },
                })
                .expect("error center query json")
                .into(),
            ))
            .await
            .expect("send error center query");
        match next_adp_response(&mut socket, "error center query").await {
            UiAdpResponse::QueryResult {
                request_id,
                result: UiQueryResult::ErrorCenterEvents(list),
            } => {
                assert_eq!(request_id, "error-center-1");
                assert_eq!(list.source_agent_id.as_str(), "master");
                assert_eq!(list.events.len(), 1);
                assert_eq!(list.events[0].domain, "provider");
                assert_eq!(list.events[0].class, "recoverable");
                assert_eq!(list.events[0].recovery_action, "fail_turn");
                assert_eq!(list.events[0].raw_hash, "provider-hash-only");
            }
            other => panic!("unexpected error center response: {other:?}"),
        }

        let _ = socket.close(None).await;
        server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_adp_subscribes_runtime_task_truth() {
        let (provider_url, request_rx, provider_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_named_response(
                    "toolu_task_adp_push_1",
                    "task",
                    serde_json::json!({
                        "op":"create",
                        "task_id":"adp-task-push-1",
                        "title":"ADP task push",
                        "content":"Publish task truth to ADP subscribers",
                        "goal":"Task subscriber receives push",
                        "deliverables":["task list subscription"],
                        "acceptance":["subscription event contains task"],
                        "dispatch":{"mode":"none"},
                        "priority":88
                    }),
                ),
                complete_single_response("task push done"),
            ],
        );
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let ws_url = server.base_url.replace("http://", "ws://") + "/adp";
        let (mut socket, _) = connect_async(ws_url).await.expect("connect adp");

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Subscribe {
                    request_id: "task-sub-1".to_owned(),
                    subscription: UiCommand::SubscribeTaskList {
                        status: Some("waiting_agent".to_owned()),
                        agent_id: None,
                    },
                })
                .expect("task list subscribe json")
                .into(),
            ))
            .await
            .expect("send task subscribe");
        match next_adp_response(&mut socket, "task subscription accepted").await {
            UiAdpResponse::SubscriptionAccepted {
                request_id,
                selector,
            } => {
                assert_eq!(request_id, "task-sub-1");
                assert_eq!(
                    selector.stream_kind,
                    freehand_ui_protocol::UiStreamKind::TaskList
                );
            }
            other => panic!("unexpected task subscription ack: {other:?}"),
        }
        match next_adp_response(&mut socket, "initial task list event").await {
            UiAdpResponse::SubscriptionEvent { request_id, event } => {
                assert_eq!(request_id, "task-sub-1");
                match event.projection {
                    freehand_ui_protocol::UiProjection::TaskList(list) => {
                        assert!(list.tasks.is_empty());
                    }
                    other => panic!("unexpected initial task projection: {other:?}"),
                }
            }
            other => panic!("unexpected initial task response: {other:?}"),
        }

        socket
            .send(Message::Text(
                serde_json::to_string(&UiAdpRequest::Command {
                    request_id: "task-submit-1".to_owned(),
                    command: UiCommand::SubmitUserInput {
                        text: "create task through provider".to_owned(),
                        session_id: None,
                        cwd: None,
                    },
                })
                .expect("task submit json")
                .into(),
            ))
            .await
            .expect("send task submit");

        let mut saw_task_push = false;
        let mut saw_receipt = false;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !saw_task_push || !saw_receipt {
            let now = tokio::time::Instant::now();
            assert!(now < deadline, "ADP task subscribe timeout");
            let response = timeout(deadline - now, socket.next())
                .await
                .expect("ADP response timeout")
                .expect("ADP response")
                .expect("ADP websocket message");
            let Message::Text(text) = response else {
                panic!("unexpected ADP websocket message: {response:?}");
            };
            let response: UiAdpResponse = serde_json::from_str(&text).expect("adp response json");
            match response {
                UiAdpResponse::SubscriptionEvent { request_id, event }
                    if request_id == "task-sub-1" =>
                {
                    if let freehand_ui_protocol::UiProjection::TaskList(list) = event.projection
                        && list
                            .tasks
                            .iter()
                            .any(|task| task.task_id == "adp-task-push-1")
                    {
                        saw_task_push = true;
                    }
                }
                UiAdpResponse::CommandReceipt {
                    request_id,
                    receipt,
                } if request_id == "task-submit-1" => {
                    assert!(
                        receipt
                            .dispatch_status
                            .contains("reason_live_turn_completed")
                    );
                    saw_receipt = true;
                }
                UiAdpResponse::Failure {
                    request_id,
                    failure,
                } => panic!("unexpected ADP failure {request_id}: {failure:?}"),
                _ => {}
            }
        }

        let _ = request_rx.recv().expect("first provider request");
        let _ = request_rx.recv().expect("second provider request");
        provider_handle.join().expect("join provider");
        let _ = socket.close(None).await;
        server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_blank_latest_sse_streams_user_prompt_before_provider_output() {
        let (provider_url, request_rx, provider_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("tool done"),
            ],
        );
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let client = Client::builder().build().expect("client");

        let mut turn_sse = client
            .get(format!("{}/ui/subscribe/turn/latest", server.base_url))
            .send()
            .await
            .expect("turn sse");
        assert_eq!(turn_sse.status(), reqwest::StatusCode::OK);

        let accepted = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "daemon streamed user prompt".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("command response");
        assert_eq!(accepted.status(), reqwest::StatusCode::ACCEPTED);
        let _ = request_rx.recv().expect("first request");
        let _ = request_rx.recv().expect("second request");
        provider_handle.join().expect("join provider");

        let mut buffer = String::new();
        let user_event =
            read_sse_event_matching(&mut turn_sse, &mut buffer, "daemon streamed user prompt")
                .await;
        assert!(user_event.contains("event: turn"));
        assert!(user_event.contains("\"kind\":\"UserText\""));
        assert!(
            user_event.contains("\"turn_id\":\"runtime-turn-1\"")
                || user_event.contains("\"turn_id\":\"runtime-turn-1-r2\""),
            "unexpected SSE event: {user_event}"
        );

        drop(turn_sse);
        server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_submit_input_surfaces_provider_failure_from_runtime_owner() {
        let (provider_url, _request_rx, provider_handle) = spawn_mock_server(
            500,
            "application/json",
            r#"{"type":"error","error":{"type":"api_error","message":"upstream failure"}}"#
                .to_owned(),
        );
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let client = Client::builder().build().expect("client");

        let failed = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "daemon turn".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("submit response");
        assert_eq!(failed.status(), reqwest::StatusCode::INTERNAL_SERVER_ERROR);
        let failed: freehand_ui_protocol::UiCommandDispatchFailure =
            failed.json().await.expect("failure json");
        assert_eq!(failed.code, "command_dispatch_port_failure");
        assert!(failed.message.contains("anthropic live executor failed"));
        assert!(failed.retryable);
        provider_handle.join().expect("join provider");

        server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_rewind_checkpoint_dispatch_restores_workspace_state() {
        let workspace = enter_temp_workspace();
        fs::create_dir_all(workspace.root().join("scratch")).expect("create parent dir");
        let (provider_url, request_rx, provider_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_write_file_response("scratch/daemon-rewind.txt", "daemon rewind\n"),
                complete_single_response("write done"),
            ],
        );
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let runtime_workspace = server.home.join(".freehand");
        fs::create_dir_all(runtime_workspace.join("scratch")).expect("create runtime parent dir");
        let client = Client::builder().build().expect("client");

        let submitted = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "create writable checkpoint".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("submit response");
        let submitted_status = submitted.status();
        if submitted_status != reqwest::StatusCode::ACCEPTED {
            let body = submitted.text().await.expect("submit failure body");
            panic!("expected checkpoint submit 202, got {submitted_status}: {body}");
        }
        let submitted: UiCommandDispatchReceipt = submitted.json().await.expect("receipt json");
        assert!(
            submitted
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        let _ = request_rx.recv().expect("first request");
        let _ = request_rx.recv().expect("second request");
        provider_handle.join().expect("join provider");

        let file_path = runtime_workspace.join("scratch/daemon-rewind.txt");
        assert_eq!(
            fs::read_to_string(&file_path).expect("written file"),
            "daemon rewind\n"
        );
        let checkpoint_id = checkpoint_id_from_home(&server.home);
        let checkpoint_query = client
            .get(format!("{}/ui/query/checkpoints", server.base_url))
            .send()
            .await
            .expect("checkpoint query response");
        assert_eq!(checkpoint_query.status(), reqwest::StatusCode::OK);
        let checkpoint_query: UiCheckpointSnapshot = checkpoint_query
            .json()
            .await
            .expect("checkpoint query json");
        assert_eq!(checkpoint_query.checkpoints.len(), 1);
        assert_eq!(checkpoint_query.checkpoints[0].checkpoint_id, checkpoint_id);
        assert_eq!(checkpoint_query.checkpoints[0].latest_status, "applied");

        let rewind = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::RewindCheckpoint {
                checkpoint_id: checkpoint_id.clone(),
            })
            .send()
            .await
            .expect("rewind response");
        assert_eq!(rewind.status(), reqwest::StatusCode::ACCEPTED);
        let rewind: UiCommandDispatchReceipt = rewind.json().await.expect("rewind receipt json");
        assert_eq!(
            rewind.dispatch_status,
            format!("runtime_checkpoint_rewound checkpoint_id={checkpoint_id}")
        );
        assert!(!file_path.exists());
        let checkpoint_query = client
            .get(format!("{}/ui/query/checkpoints", server.base_url))
            .send()
            .await
            .expect("post-rewind checkpoint query response");
        assert_eq!(checkpoint_query.status(), reqwest::StatusCode::OK);
        let checkpoint_query: UiCheckpointSnapshot = checkpoint_query
            .json()
            .await
            .expect("post-rewind checkpoint json");
        assert_eq!(checkpoint_query.checkpoints[0].latest_status, "restored");

        server.stop().await;
        drop(workspace);
    }

    #[tokio::test]
    #[serial]
    async fn daemon_rewind_checkpoint_missing_manifest_surfaces_protocol_failure() {
        let workspace = enter_temp_workspace();
        fs::create_dir_all(workspace.root().join("scratch")).expect("create parent dir");
        let (provider_url, request_rx, provider_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_write_file_response(
                    "scratch/daemon-rewind-missing.txt",
                    "daemon rewind missing\n",
                ),
                complete_single_response("write done"),
            ],
        );
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let runtime_workspace = server.home.join(".freehand");
        fs::create_dir_all(runtime_workspace.join("scratch")).expect("create runtime parent dir");
        let client = Client::builder().build().expect("client");

        let submitted = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "create writable checkpoint".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("submit response");
        let submitted_status = submitted.status();
        if submitted_status != reqwest::StatusCode::ACCEPTED {
            let body = submitted.text().await.expect("submit failure body");
            panic!("expected checkpoint submit 202, got {submitted_status}: {body}");
        }
        let _: UiCommandDispatchReceipt = submitted.json().await.expect("receipt json");
        let _ = request_rx.recv().expect("first request");
        let _ = request_rx.recv().expect("second request");
        provider_handle.join().expect("join provider");

        let file_path = runtime_workspace.join("scratch/daemon-rewind-missing.txt");
        assert_eq!(
            fs::read_to_string(&file_path).expect("written file"),
            "daemon rewind missing\n"
        );

        let checkpoint_id = checkpoint_id_from_home(&server.home);
        let manifest_path = server
            .home
            .join(".freehand")
            .join("state")
            .join("checkpoints")
            .join("master")
            .join("runtime-session-master")
            .join(&checkpoint_id)
            .join("manifest.json");
        fs::remove_file(&manifest_path).expect("remove manifest");

        let rewind = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::RewindCheckpoint {
                checkpoint_id: checkpoint_id.clone(),
            })
            .send()
            .await
            .expect("rewind response");
        assert_eq!(rewind.status(), reqwest::StatusCode::INTERNAL_SERVER_ERROR);
        let rewind: UiCommandDispatchFailure = rewind.json().await.expect("rewind failure json");
        assert_eq!(rewind.code, "command_dispatch_target_not_found");
        assert!(!rewind.retryable);
        assert!(rewind.message.contains(&checkpoint_id));
        assert_eq!(
            fs::read_to_string(&file_path).expect("file should remain after failed rewind"),
            "daemon rewind missing\n"
        );

        server.stop().await;
        drop(workspace);
    }

    #[tokio::test]
    #[serial]
    async fn daemon_restart_restores_query_and_sse_then_continues_with_next_turn_id() {
        let (first_provider_url, first_request_rx, first_provider_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("first done"),
            ],
        );
        let home = write_test_home(&master_config_text(&first_provider_url)).expect("test home");
        let first_server = TestServer::spawn_existing_home(home.clone(), false).await;
        let client = Client::builder().build().expect("client");

        let first_submit = client
            .post(format!("{}/ui/command", first_server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "first daemon turn".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("first submit");
        assert_eq!(first_submit.status(), reqwest::StatusCode::ACCEPTED);
        let first_submit: UiCommandDispatchReceipt =
            first_submit.json().await.expect("first receipt");
        assert_eq!(
            first_submit.dispatch_status,
            "reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 restored_closed_turns=0"
        );
        let _ = first_request_rx.recv().expect("first provider request");
        let _ = first_request_rx.recv().expect("first tool-result request");
        first_provider_handle.join().expect("join first provider");
        first_server.stop().await;

        let restored_server = TestServer::spawn_existing_home(home.clone(), false).await;
        let restored_query = client
            .get(format!(
                "{}/ui/query/latest-active-turn",
                restored_server.base_url
            ))
            .send()
            .await
            .expect("restored query");
        assert_eq!(restored_query.status(), reqwest::StatusCode::OK);
        let restored_query: UiPublicTurnProjection =
            restored_query.json().await.expect("query json");
        assert_eq!(
            restored_query.turn.turn_id,
            TurnId::new("runtime-turn-1-r2")
        );
        assert!(
            restored_query
                .turn
                .terminal_text
                .as_deref()
                .is_some_and(|text| text.contains("Summary: first done"))
        );

        let mut restored_sse = client
            .get(format!(
                "{}/ui/subscribe/turn/latest",
                restored_server.base_url
            ))
            .send()
            .await
            .expect("restored sse");
        assert_eq!(restored_sse.status(), reqwest::StatusCode::OK);
        let mut restored_sse_buffer = String::new();
        let restored_sse_event =
            read_next_sse_event(&mut restored_sse, &mut restored_sse_buffer).await;
        assert!(restored_sse_event.contains("\"turn_id\":\"runtime-turn-1-r2\""));
        assert!(restored_sse_event.contains("Summary: first done"));
        drop(restored_sse);
        restored_server.stop().await;

        let (second_provider_url, second_request_rx, second_provider_handle) =
            spawn_sequence_server(
                "application/json",
                vec![
                    tool_use_single_response(),
                    complete_single_response("second done"),
                ],
            );
        write_config_home(&home, &master_config_text(&second_provider_url)).expect("rewrite home");
        let resumed_server = TestServer::spawn_existing_home(home.clone(), true).await;
        let resumed_submit = client
            .post(format!("{}/ui/command", resumed_server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "second daemon turn".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("resumed submit");
        assert_eq!(resumed_submit.status(), reqwest::StatusCode::ACCEPTED);
        let resumed_submit: UiCommandDispatchReceipt =
            resumed_submit.json().await.expect("resumed receipt");
        assert_eq!(
            resumed_submit.dispatch_status,
            "reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 restored_closed_turns=1"
        );
        let _ = second_request_rx.recv().expect("second provider request");
        let _ = second_request_rx
            .recv()
            .expect("second tool-result request");
        second_provider_handle.join().expect("join second provider");

        let resumed_query = client
            .get(format!(
                "{}/ui/query/latest-active-turn",
                resumed_server.base_url
            ))
            .send()
            .await
            .expect("resumed query");
        assert_eq!(resumed_query.status(), reqwest::StatusCode::OK);
        let resumed_query: UiPublicTurnProjection = resumed_query.json().await.expect("query json");
        assert_eq!(resumed_query.turn.turn_id, TurnId::new("runtime-turn-2-r2"));
        assert!(
            resumed_query
                .turn
                .terminal_text
                .as_deref()
                .is_some_and(|text| text.contains("Summary: second done"))
        );

        resumed_server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_turn_sse_stream_continues_across_new_runtime_turns() {
        let (provider_url, request_rx, provider_handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_single_response(),
                complete_single_response("first stream done"),
                tool_use_single_response(),
                complete_single_response("second stream done"),
            ],
        );
        let server = TestServer::spawn(master_config_text(&provider_url)).await;
        let client = Client::builder().build().expect("client");

        let first_submit = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "first streamed daemon turn".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("first submit");
        assert_eq!(first_submit.status(), reqwest::StatusCode::ACCEPTED);
        let _: UiCommandDispatchReceipt = first_submit.json().await.expect("first receipt");

        let first_query = client
            .get(format!("{}/ui/query/latest-active-turn", server.base_url))
            .send()
            .await
            .expect("first query");
        assert_eq!(first_query.status(), reqwest::StatusCode::OK);
        let first_query: UiPublicTurnProjection =
            first_query.json().await.expect("first query json");
        assert_eq!(first_query.turn.turn_id, TurnId::new("runtime-turn-1-r2"));

        let mut sse = client
            .get(format!("{}/ui/subscribe/turn/latest", server.base_url))
            .send()
            .await
            .expect("turn sse");
        assert_eq!(sse.status(), reqwest::StatusCode::OK);
        let mut sse_buffer = String::new();
        let first_event = read_next_sse_event(&mut sse, &mut sse_buffer).await;
        assert!(first_event.contains("\"turn_id\":\"runtime-turn-1-r2\""));
        assert!(first_event.contains("Summary: first stream done"));

        let second_submit = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "second streamed daemon turn".to_owned(),
                session_id: None,
                cwd: None,
            })
            .send()
            .await
            .expect("second submit");
        assert_eq!(second_submit.status(), reqwest::StatusCode::ACCEPTED);
        let _: UiCommandDispatchReceipt = second_submit.json().await.expect("second receipt");

        let second_event =
            read_sse_event_matching(&mut sse, &mut sse_buffer, "Summary: second stream done").await;
        assert!(second_event.contains("\"turn_id\":\"runtime-turn-2-r2\""));
        assert!(second_event.contains("Summary: second stream done"));

        let _ = request_rx.recv().expect("first provider request");
        let _ = request_rx.recv().expect("first tool-result request");
        let _ = request_rx.recv().expect("second provider request");
        let _ = request_rx.recv().expect("second tool-result request");
        provider_handle.join().expect("join provider");

        drop(sse);
        server.stop().await;
    }

    #[tokio::test]
    #[serial]
    async fn daemon_direct_message_dispatch_returns_runtime_receipt() {
        let server = TestServer::spawn(master_config_text("https://example.invalid")).await;
        let client = Client::builder().build().expect("client");

        let dispatched = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SendDirectMessageToSlave {
                node_id: "worker-node".to_owned(),
                text: "ping slave".to_owned(),
            })
            .send()
            .await
            .expect("direct message response");
        assert_eq!(dispatched.status(), reqwest::StatusCode::ACCEPTED);
        let dispatched: UiCommandDispatchReceipt = dispatched.json().await.expect("receipt json");
        assert_eq!(dispatched.dispatch_status, "node_direct_message_dispatched");
        assert_eq!(dispatched.target_feature_id, "node.master-slave");

        server.stop().await;
    }

    #[test]
    #[serial]
    fn daemon_bootstrap_reads_selected_master_from_default_config() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let home =
            write_test_home(&master_config_text("https://example.invalid")).expect("test home");
        let old_home = env::var_os("HOME");
        let old_pair_token = env::var_os("FREEHAND_PAIR_TOKEN_SHARED");
        unsafe { env::set_var("HOME", &home) };
        unsafe { env::set_var("FREEHAND_PAIR_TOKEN_SHARED", "pair-token-shared") };

        let dispatcher =
            build_runtime_dispatcher_from_default_config("master").expect("runtime dispatcher");

        restore_env(old_home, "FREEHAND_PAIR_TOKEN_SHARED", old_pair_token);

        let ui_state = dispatcher.ui_state();
        let snapshot = ui_state
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryNodeStatus {
                node_id: "worker-node".to_owned(),
            })
            .expect("query");
        match snapshot {
            freehand_ui_protocol::UiQueryResult::NodeStatus(Some(status)) => {
                assert_eq!(status.pairing_state, "paired");
            }
            other => panic!("unexpected node status query: {other:?}"),
        }
    }

    #[test]
    #[serial]
    fn daemon_worker_mode_builds_production_runner_for_slave_agent() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let home =
            write_test_home(&slave_config_text("https://example.invalid")).expect("test home");
        let old_home = env::var_os("HOME");
        let old_pair_token = env::var_os("FREEHAND_PAIR_TOKEN_SHARED");
        unsafe { env::set_var("HOME", &home) };
        unsafe { env::set_var("FREEHAND_PAIR_TOKEN_SHARED", "pair-token-shared") };

        build_worker_runner_from_default_config("worker").expect("worker runner");

        restore_env(old_home, "FREEHAND_PAIR_TOKEN_SHARED", old_pair_token);
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[tokio::test]
    async fn daemon_worker_service_runs_blocking_runtime_outside_async_context() {
        run_blocking_worker_service(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("nested blocking runtime");
            drop(runtime);
            Ok(())
        })
        .await
        .expect("blocking worker service");
    }

    #[tokio::test]
    async fn daemon_worker_service_surfaces_blocking_task_panic() {
        let error = run_blocking_worker_service(
            || -> Result<(), freehand_runtime::ProductionWorkerRunnerError> {
                panic!("worker service panic probe")
            },
        )
        .await
        .expect_err("blocking task panic must be explicit");

        assert!(error.contains("worker runner task failed"));
    }

    #[test]
    #[serial]
    fn daemon_bootstrap_rejects_corrupt_checkpoint_projection_truth() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let home =
            write_test_home(&master_config_text("https://example.invalid")).expect("test home");
        let checkpoint_ledger = home
            .join(".freehand")
            .join("ledgers")
            .join("checkpoints")
            .join("master");
        fs::create_dir_all(&checkpoint_ledger).expect("create checkpoint ledger dir");
        fs::write(
            checkpoint_ledger.join("runtime-session-master.jsonl"),
            "{not-json}\n",
        )
        .expect("write corrupt checkpoint ledger");

        let old_home = env::var_os("HOME");
        let old_pair_token = env::var_os("FREEHAND_PAIR_TOKEN_SHARED");
        unsafe { env::set_var("HOME", &home) };
        unsafe { env::set_var("FREEHAND_PAIR_TOKEN_SHARED", "pair-token-shared") };

        let err = match build_runtime_dispatcher_from_default_config("master") {
            Ok(_) => panic!("corrupt checkpoint projection truth must fail"),
            Err(err) => err,
        };

        restore_env(old_home, "FREEHAND_PAIR_TOKEN_SHARED", old_pair_token);

        assert!(err.contains("failed to build runtime dispatcher"));
        assert!(err.contains("checkpoint projection bootstrap failed"));
        assert!(err.contains("checkpoint ledger line 1 failed to parse"));
    }

    fn master_config_text(base_url: &str) -> String {
        format!(
            r#"
[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "minimonth"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
allowed_pair_ip = "127.0.0.1"
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "minimonth"

[providers.minimonth]
id = "minimonth"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "{base_url}"
default_model = "MiniMax-M2.7"

[providers.minimonth.auth]
type = "apikey"
api_key = "test-api-key"
"#
        )
    }

    fn slave_config_text(base_url: &str) -> String {
        format!(
            r#"
[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
allowed_pair_ip = "127.0.0.1"
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "minimonth"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "minimonth"

[providers.minimonth]
id = "minimonth"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "{base_url}"
default_model = "MiniMax-M2.7"

[providers.minimonth.auth]
type = "apikey"
api_key = "test-api-key"
"#
        )
    }

    fn write_test_home(config_text: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos();
        let home = env::temp_dir().join(format!("freehand-daemon-test-{stamp}"));
        write_config_home(&home, config_text)?;
        Ok(home)
    }

    fn write_config_home(home: &std::path::Path, config_text: &str) -> Result<(), String> {
        let config_dir = home.join(".freehand");
        fs::create_dir_all(&config_dir).map_err(|err| err.to_string())?;
        fs::write(config_dir.join("config.toml"), config_text).map_err(|err| err.to_string())
    }

    fn spawn_mock_server(
        status: u16,
        content_type: &'static str,
        response_body: String,
    ) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .expect("timeout");
            let mut raw = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read");
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..read]);
                if request_is_complete(&raw) {
                    break;
                }
            }
            tx.send(String::from_utf8(raw).expect("utf8"))
                .expect("send");
            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
                response_body.len()
            );
            stream.write_all(response.as_bytes()).expect("write");
        });
        (base_url, rx, handle)
    }

    fn spawn_sequence_server(
        content_type: &'static str,
        response_bodies: Vec<String>,
    ) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            for response_body in response_bodies {
                let (mut stream, _) = listener.accept().expect("accept");
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .expect("timeout");
                let mut raw = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let read = stream.read(&mut buffer).expect("read");
                    if read == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buffer[..read]);
                    if request_is_complete(&raw) {
                        break;
                    }
                }
                tx.send(String::from_utf8(raw).expect("utf8"))
                    .expect("send");
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
                    response_body.len()
                );
                stream.write_all(response.as_bytes()).expect("write");
            }
        });
        (base_url, rx, handle)
    }

    fn request_is_complete(raw: &[u8]) -> bool {
        let text = String::from_utf8_lossy(raw);
        let Some(header_end) = text.find("\r\n\r\n") else {
            return false;
        };
        let content_length = text[..header_end]
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length: ")
                    .or_else(|| line.strip_prefix("Content-Length: "))
            })
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        raw.len() >= header_end + 4 + content_length
    }

    fn tagged_completion_json(body: &str) -> String {
        format!("<freehand_completion>\n{body}\n</freehand_completion>")
    }

    fn complete_single_response(visible_text: &str) -> String {
        let tagged = tagged_completion_json(&format!(
            r#"{{"claim":"complete","completion_reason":"done","evidence":"provider returned {visible_text}","summary":"{visible_text}","learned":"keep tagged completion strict"}}"#
        ));
        format!(
            r#"{{"content":[{{"type":"text","text":"{visible}\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":82}},"stop_reason":"end_turn"}}"#,
            visible = visible_text,
            tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
        )
    }

    fn tool_use_single_response() -> String {
        r#"{"content":[{"type":"tool_use","id":"toolu_read_1","name":"read_file","input":{"path":"Cargo.toml","offset":0,"limit":2}}],"usage":{"input_tokens":20,"output_tokens":16},"stop_reason":"tool_use"}"#.to_owned()
    }

    fn tool_use_named_response(tool_call_id: &str, tool_name: &str, input: Value) -> String {
        serde_json::json!({
            "content": [{
                "type": "tool_use",
                "id": tool_call_id,
                "name": tool_name,
                "input": input
            }],
            "usage": {"input_tokens": 20, "output_tokens": 16},
            "stop_reason": "tool_use"
        })
        .to_string()
    }

    fn tool_use_write_file_response(path: &str, content: &str) -> String {
        serde_json::json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_write_1",
                "name": "write_file",
                "input": {
                    "path": path,
                    "content": content
                }
            }],
            "usage": {"input_tokens": 20, "output_tokens": 16},
            "stop_reason": "tool_use"
        })
        .to_string()
    }

    fn restore_env(old_home: Option<OsString>, token_name: &str, old_token: Option<OsString>) {
        match old_home {
            Some(value) => unsafe { env::set_var("HOME", value) },
            None => unsafe { env::remove_var("HOME") },
        }
        match old_token {
            Some(value) => unsafe { env::set_var(token_name, value) },
            None => unsafe { env::remove_var(token_name) },
        }
    }
}
