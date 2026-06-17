use std::future::pending;
use std::sync::Arc;

use freehand_runtime::RuntimeCommandDispatcher;
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
            let bind_addr = parse_bind_arg(args.into_iter().skip(2))?;
            let dispatcher = build_runtime_dispatcher_from_default_config(&agent_name)?;
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
            serve_webui_listener(listener, ui_state, dispatch_port, pending::<()>())
                .await
                .map_err(|err| format!("daemon server error: {err}"))?;
            Ok(String::new())
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: freehand-daemon serve --agent <name> [--bind HOST:PORT]".to_owned()
}

fn build_runtime_dispatcher_from_default_config(
    agent_name: &str,
) -> Result<Arc<RuntimeCommandDispatcher>, String> {
    RuntimeCommandDispatcher::from_default_config(agent_name)
        .map(Arc::new)
        .map_err(|err| format!("failed to build runtime dispatcher: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::TurnId;
    use freehand_ui_protocol::{
        UiCheckpointSnapshot, UiCommand, UiCommandDispatchReceipt, UiPublicTurnProjection,
    };
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
            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let task = tokio::spawn(async move {
                let shutdown = async move {
                    let _ = shutdown_rx.await;
                };
                serve_webui_listener(listener, ui_state, dispatch_port, shutdown)
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
        TempWorkspace {
            root,
            original,
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
        let client = Client::builder().build().expect("client");

        let submitted = client
            .post(format!("{}/ui/command", server.base_url))
            .json(&UiCommand::SubmitUserInput {
                text: "create writable checkpoint".to_owned(),
            })
            .send()
            .await
            .expect("submit response");
        assert_eq!(submitted.status(), reqwest::StatusCode::ACCEPTED);
        let submitted: UiCommandDispatchReceipt = submitted.json().await.expect("receipt json");
        assert!(
            submitted
                .dispatch_status
                .contains("reason_live_turn_completed")
        );
        let _ = request_rx.recv().expect("first request");
        let _ = request_rx.recv().expect("second request");
        provider_handle.join().expect("join provider");

        let file_path = workspace.root().join("scratch/daemon-rewind.txt");
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
    fn daemon_bootstrap_rejects_slave_mode_agent() {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let home =
            write_test_home(&slave_config_text("https://example.invalid")).expect("test home");
        let old_home = env::var_os("HOME");
        let old_pair_token = env::var_os("FREEHAND_PAIR_TOKEN_SHARED");
        unsafe { env::set_var("HOME", &home) };
        unsafe { env::set_var("FREEHAND_PAIR_TOKEN_SHARED", "pair-token-shared") };

        let err = match build_runtime_dispatcher_from_default_config("worker") {
            Ok(_) => panic!("slave-mode agent must be rejected"),
            Err(err) => err,
        };

        restore_env(old_home, "FREEHAND_PAIR_TOKEN_SHARED", old_pair_token);

        assert!(err.contains("runtime host requires a master agent"));
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
