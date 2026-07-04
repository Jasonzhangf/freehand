use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_contracts::{AgentId, SessionId, TerminalStatus, TurnId};
use freehand_ui_protocol::{
    SubscriptionSelector, UiAdpFailure, UiAdpRequest, UiAdpResponse, UiClientKind, UiCommand,
    UiCommandDispatchReceipt, UiProjection, UiQueryResult, UiSessionListProjection,
    UiSessionSummary, UiSessionTranscriptProjection, UiSource, UiStreamKind, UiSubscriptionEvent,
    UiToolActivity, UiToolActivityStatus, UiTurnProjection, build_command_dispatch_envelope,
};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_home_dir() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time drift")
        .as_nanos();
    let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("freehand-cli-home-{nanos}-{counter}"))
}

fn spawn_mock_server(
    status: u16,
    content_type: &'static str,
    response_body: String,
) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
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
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        for response_body in response_bodies {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
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

fn spawn_adp_mock_server() -> (String, thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    listener
        .set_nonblocking(true)
        .expect("set adp mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            let (stream, _) = listener.accept().await.expect("accept adp");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("accept websocket");
            while let Some(message) = socket.next().await {
                let text = match message {
                    Ok(Message::Text(text)) => text,
                    Ok(_) => continue,
                    Err(_) => break,
                };
                let request: UiAdpRequest = serde_json::from_str(&text).expect("adp request");
                match request {
                    UiAdpRequest::Subscribe { request_id, .. } => {
                        send_adp_response(
                            &mut socket,
                            UiAdpResponse::SubscriptionAccepted {
                                request_id: request_id.clone(),
                                selector: SubscriptionSelector {
                                    client: UiClientKind::Cli,
                                    stream_kind: UiStreamKind::Turn,
                                    target_turn_id: None,
                                },
                            },
                        )
                        .await;
                        send_adp_response(
                            &mut socket,
                            UiAdpResponse::SubscriptionEvent {
                                request_id,
                                event: UiSubscriptionEvent {
                                    projection: UiProjection::Turn(test_turn_projection()),
                                    latest_active_turn_id: Some(TurnId::new("cli-adp-turn")),
                                },
                            },
                        )
                        .await;
                    }
                    UiAdpRequest::Query { request_id, .. } => {
                        send_adp_response(
                            &mut socket,
                            UiAdpResponse::QueryResult {
                                request_id,
                                result: UiQueryResult::Turn(Some(test_turn_projection())),
                            },
                        )
                        .await;
                    }
                    UiAdpRequest::Command {
                        request_id,
                        command,
                    } => {
                        let code = match command {
                            freehand_ui_protocol::UiCommand::QueryLatestActiveTurn => {
                                "ingress_command_kind_mismatch"
                            }
                            _ => "unexpected_command",
                        };
                        send_adp_response(
                            &mut socket,
                            UiAdpResponse::Failure {
                                request_id,
                                failure: freehand_ui_protocol::UiAdpFailure {
                                    code: code.to_owned(),
                                    message: "command frame rejected".to_owned(),
                                    retryable: false,
                                },
                            },
                        )
                        .await;
                    }
                }
            }
        });
    });
    (url, handle)
}

fn spawn_adp_sample_mock_server(status: TerminalStatus) -> (String, thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp sample mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    let sample_turn = Arc::new(Mutex::new(None::<UiTurnProjection>));
    listener
        .set_nonblocking(true)
        .expect("set adp sample mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.expect("accept adp");
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("accept websocket");
                let mut subscription_id = None::<String>;
                while let Some(message) = socket.next().await {
                    let text = match message {
                        Ok(Message::Text(text)) => text,
                        Ok(_) => continue,
                        Err(_) => break,
                    };
                    let request: UiAdpRequest = serde_json::from_str(&text).expect("adp request");
                    match request {
                        UiAdpRequest::Subscribe { request_id, .. } => {
                            subscription_id = Some(request_id.clone());
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::SubscriptionAccepted {
                                    request_id,
                                    selector: SubscriptionSelector {
                                        client: UiClientKind::Cli,
                                        stream_kind: UiStreamKind::Turn,
                                        target_turn_id: None,
                                    },
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command:
                                UiCommand::SubmitUserInput {
                                    text, session_id, ..
                                },
                        } => {
                            let session_id =
                                session_id.unwrap_or_else(|| SessionId::new("cli-session"));
                            let turn =
                                test_sample_turn_projection(&text, &session_id, status.clone());
                            *sample_turn.lock().expect("sample turn lock") = Some(turn.clone());
                            let envelope =
                                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                                    text: text.clone(),
                                    session_id: Some(session_id.clone()),
                                    cwd: None,
                                })
                                .expect("sample envelope");
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::CommandReceipt {
                                    request_id,
                                    receipt: UiCommandDispatchReceipt {
                                        ingress: envelope.ingress,
                                        target_feature_id: envelope.target_feature_id,
                                        target_owner_module: envelope.target_owner_module,
                                        dispatch_status: if status == TerminalStatus::Success {
                                            "sample_success".to_owned()
                                        } else {
                                            "sample_tool_failure_recovered".to_owned()
                                        },
                                    },
                                },
                            )
                            .await;
                            if let Some(sub_id) = subscription_id.clone() {
                                send_adp_response(
                                    &mut socket,
                                    UiAdpResponse::SubscriptionEvent {
                                        request_id: sub_id,
                                        event: UiSubscriptionEvent {
                                            projection: UiProjection::Turn(turn),
                                            latest_active_turn_id: Some(TurnId::new(
                                                "cli-adp-sample-turn",
                                            )),
                                        },
                                    },
                                )
                                .await;
                            }
                        }
                        UiAdpRequest::Query { request_id, .. } => {
                            let turn = sample_turn
                                .lock()
                                .expect("sample turn lock")
                                .clone()
                                .expect("sample turn");
                            if request_id.contains("-transcript") {
                                let turns = if status == TerminalStatus::Failed {
                                    let mut first = turn.clone();
                                    first.turn_id = TurnId::new("cli-adp-sample-turn");
                                    first.terminal_status = None;
                                    first.terminal_text = None;
                                    let mut second = turn;
                                    second.turn_id = TurnId::new("cli-adp-sample-turn-r2");
                                    vec![first, second]
                                } else {
                                    vec![turn]
                                };
                                send_adp_response(
                                    &mut socket,
                                    UiAdpResponse::QueryResult {
                                        request_id,
                                        result: UiQueryResult::SessionTurns(
                                            UiSessionTranscriptProjection {
                                                session_id: turns[0].session_id.clone(),
                                                title: None,
                                                archived: false,
                                                cwd: Some("/tmp/cli-session".to_owned()),
                                                turns,
                                            },
                                        ),
                                    },
                                )
                                .await;
                                continue;
                            }
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::Turn(Some(turn)),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Command { request_id, .. } => {
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::Failure {
                                    request_id,
                                    failure: UiAdpFailure {
                                        code: "unexpected_command".to_owned(),
                                        message: "unexpected sample command".to_owned(),
                                        retryable: false,
                                    },
                                },
                            )
                            .await;
                        }
                    }
                }
            }
        });
    });
    (url, handle)
}

fn spawn_adp_session_mock_server() -> (String, thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp session mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    listener
        .set_nonblocking(true)
        .expect("set adp session mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            let (stream, _) = listener.accept().await.expect("accept adp");
            let mut socket = tokio_tungstenite::accept_async(stream)
                .await
                .expect("accept websocket");
            while let Some(message) = socket.next().await {
                let text = match message {
                    Ok(Message::Text(text)) => text,
                    Ok(_) => continue,
                    Err(_) => break,
                };
                let request: UiAdpRequest = serde_json::from_str(&text).expect("adp request");
                match request {
                    UiAdpRequest::Query {
                        request_id,
                        query: UiCommand::QuerySessionList,
                    } => {
                        send_adp_response(
                            &mut socket,
                            UiAdpResponse::QueryResult {
                                request_id,
                                result: UiQueryResult::SessionList(UiSessionListProjection {
                                    sessions: vec![UiSessionSummary {
                                        session_id: SessionId::new("cli-session"),
                                        title: None,
                                        archived: false,
                                        cwd: Some("/tmp/cli-session".to_owned()),
                                        latest_turn_id: Some(TurnId::new("runtime-turn-10")),
                                        active_turn_id: None,
                                        turn_count: 2,
                                        latest_status: "success".to_owned(),
                                        latest_summary: Some("second answer".to_owned()),
                                    }],
                                }),
                            },
                        )
                        .await;
                    }
                    UiAdpRequest::Query {
                        request_id,
                        query: UiCommand::QuerySessionTurns { session_id },
                    } => {
                        let mut first = test_turn_projection();
                        first.session_id = session_id.clone();
                        first.turn_id = TurnId::new("runtime-turn-2");
                        first.user_text = Some("first prompt".to_owned());
                        first.terminal_text = Some("first answer".to_owned());
                        first.terminal_status = Some(TerminalStatus::Success);
                        let mut second = test_turn_projection();
                        second.session_id = session_id.clone();
                        second.turn_id = TurnId::new("runtime-turn-10");
                        second.user_text = Some("second prompt".to_owned());
                        second.terminal_text = Some("second answer".to_owned());
                        second.terminal_status = Some(TerminalStatus::Success);
                        send_adp_response(
                            &mut socket,
                            UiAdpResponse::QueryResult {
                                request_id,
                                result: UiQueryResult::SessionTurns(
                                    UiSessionTranscriptProjection {
                                        session_id,
                                        title: None,
                                        archived: false,
                                        cwd: Some("/tmp/cli-session".to_owned()),
                                        turns: vec![first, second],
                                    },
                                ),
                            },
                        )
                        .await;
                    }
                    UiAdpRequest::Command {
                        request_id,
                        command:
                            UiCommand::CreateSession { .. }
                            | UiCommand::RenameSession { .. }
                            | UiCommand::ArchiveSession { .. }
                            | UiCommand::RestoreSession { .. }
                            | UiCommand::DeleteSession { .. }
                            | UiCommand::RollbackLatestSessionTurn { .. },
                    } => {
                        send_adp_response(
                            &mut socket,
                            UiAdpResponse::CommandReceipt {
                                request_id,
                                receipt: UiCommandDispatchReceipt {
                                    ingress: freehand_ui_protocol::UiCommandIngressAck {
                                        command_kind: "session_manage".to_owned(),
                                        accepted: true,
                                        status_text: "accepted".to_owned(),
                                        mutation_authority: "runtime".to_owned(),
                                    },
                                    target_feature_id: "reason.persistence".to_owned(),
                                    target_owner_module: "crates/freehand-reason".to_owned(),
                                    dispatch_status: "session_turn_rolled_back:runtime-turn-10"
                                        .to_owned(),
                                },
                            },
                        )
                        .await;
                    }
                    UiAdpRequest::Query { request_id, .. }
                    | UiAdpRequest::Command { request_id, .. }
                    | UiAdpRequest::Subscribe { request_id, .. } => {
                        send_adp_response(
                            &mut socket,
                            UiAdpResponse::Failure {
                                request_id,
                                failure: UiAdpFailure {
                                    code: "unexpected_session_query_frame".to_owned(),
                                    message: "unexpected session query frame".to_owned(),
                                    retryable: false,
                                },
                            },
                        )
                        .await;
                    }
                }
            }
        });
    });
    (url, handle)
}

async fn send_adp_response(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    response: UiAdpResponse,
) {
    let body = serde_json::to_string(&response).expect("response json");
    socket.send(Message::Text(body.into())).await.expect("send");
}

fn test_turn_projection() -> UiTurnProjection {
    UiTurnProjection {
        source: UiSource {
            source_agent_id: AgentId::new("cli-agent"),
            source_node_id: "cli-node".to_owned(),
            source_turn_id: Some(TurnId::new("cli-adp-turn")),
            stream_kind: UiStreamKind::Turn,
        },
        session_id: SessionId::new("cli-session"),
        turn_id: TurnId::new("cli-adp-turn"),
        cwd: Some("/tmp/cli-session".to_owned()),
        user_text: Some("cli adp smoke".to_owned()),
        model_request: None,
        reasoning: Vec::new(),
        text: Vec::new(),
        tool_calls: Vec::new(),
        tool_activities: Vec::new(),
        usage: Vec::new(),
        terminal_status: None,
        terminal_text: None,
        errors: Vec::new(),
        slave_substream_card: false,
    }
}

fn test_sample_turn_projection(
    prompt: &str,
    session_id: &SessionId,
    status: TerminalStatus,
) -> UiTurnProjection {
    let failed = status == TerminalStatus::Failed;
    let terminal_status = TerminalStatus::Success;
    UiTurnProjection {
        source: UiSource {
            source_agent_id: AgentId::new("cli-agent"),
            source_node_id: "cli-node".to_owned(),
            source_turn_id: Some(TurnId::new("cli-adp-sample-turn")),
            stream_kind: UiStreamKind::Turn,
        },
        session_id: session_id.clone(),
        turn_id: TurnId::new(if failed {
            "cli-adp-sample-turn-r2"
        } else {
            "cli-adp-sample-turn"
        }),
        cwd: Some("/tmp/cli-session".to_owned()),
        user_text: Some(prompt.to_owned()),
        model_request: None,
        reasoning: Vec::new(),
        text: if failed {
            Vec::new()
        } else {
            vec!["sample success answer".to_owned()]
        },
        tool_calls: if failed {
            vec!["ls".to_owned()]
        } else {
            Vec::new()
        },
        tool_activities: if failed {
            vec![UiToolActivity {
                tool_call_id: "toolu_missing_read_1".to_owned(),
                tool_name: "read_file".to_owned(),
                status: UiToolActivityStatus::Failed,
                detail: Some("tool execution returned failure result".to_owned()),
                display: None,
            }]
        } else {
            Vec::new()
        },
        usage: Vec::new(),
        terminal_status: Some(terminal_status),
        terminal_text: Some(if failed {
            "sample recovered after tool failure".to_owned()
        } else {
            "sample success terminal".to_owned()
        }),
        errors: Vec::new(),
        slave_substream_card: false,
    }
}

fn tagged_completion_json(body: &str) -> String {
    format!("<freehand_completion>\n{body}\n</freehand_completion>")
}

fn complete_single_response(visible_text: &str) -> String {
    let tagged = tagged_completion_json(
        r#"{"claim":"complete","completion_reason":"done","evidence":"provider returned pong","summary":"pong","learned":"keep tagged completion strict"}"#,
    );
    format!(
        r#"{{"content":[{{"type":"text","text":"{visible}\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":82}},"stop_reason":"end_turn"}}"#,
        visible = visible_text,
        tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
    )
}

fn tool_use_single_response() -> String {
    r#"{"content":[{"type":"tool_use","id":"toolu_read_1","name":"read_file","input":{"path":"Cargo.toml","offset":0,"limit":2}}],"usage":{"input_tokens":20,"output_tokens":16},"stop_reason":"tool_use"}"#.to_owned()
}

fn complete_stream_response(visible_text: &str) -> String {
    let tagged = tagged_completion_json(
        r#"{"claim":"complete","completion_reason":"done","evidence":"provider returned pong","summary":"pong","learned":"keep tagged completion strict"}"#,
    );
    format!(
        concat!(
            "event: content_block_start\n",
            "data: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"thinking\",\"thinking\":\"\"}}}}\n\n",
            "event: content_block_delta\n",
            "data: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"thinking_delta\",\"thinking\":\"thinking\"}}}}\n\n",
            "event: content_block_stop\n",
            "data: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n",
            "event: content_block_start\n",
            "data: {{\"type\":\"content_block_start\",\"index\":1,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n",
            "event: content_block_delta\n",
            "data: {{\"type\":\"content_block_delta\",\"index\":1,\"delta\":{{\"type\":\"text_delta\",\"text\":\"{text}\"}}}}\n\n",
            "event: content_block_stop\n",
            "data: {{\"type\":\"content_block_stop\",\"index\":1}}\n\n",
            "event: message_delta\n",
            "data: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"input_tokens\":14,\"output_tokens\":82}}}}\n\n",
            "event: message_stop\n",
            "data: {{\"type\":\"message_stop\"}}\n\n"
        ),
        text = format!("{visible_text}\\n{tagged}")
            .replace('\n', "\\n")
            .replace('"', "\\\"")
    )
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

#[test]
fn cli_selects_named_agent_from_default_config_path() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "chat_completions"
baseURL = "http://guizhouyun.site:2080"
defaultModel = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
apiKey = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
pair_token = "FREEHAND_WORKER_TOKEN"
provider = "mini27"
"#,
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .arg("--agent")
        .arg("master")
        .output()
        .expect("run cli");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("mode=master"));
    assert!(stdout.contains("pair_token_env=FREEHAND_CLI_TOKEN"));
    assert!(stdout.contains("provider=mini27"));
    assert!(stdout.contains("provider_type=openai"));
    assert!(stdout.contains("provider_protocol=chat_completions"));
    assert!(stdout.contains("default_model=MiniMax-M2.7"));
    assert!(stdout.contains("base_url=http://guizhouyun.site:2080"));
    assert!(stdout.contains("provider_auth=apikey"));
    assert!(stdout.contains("restart_required_on_change=true"));
    assert!(!stdout.contains("sk-inline"));

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_adp_smoke_against_mock_websocket() {
    let (url, handle) = spawn_adp_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("adp-smoke")
        .arg("--url")
        .arg(&url)
        .output()
        .expect("run adp smoke");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("adp_smoke_ok"));
    assert!(stdout.contains("subscription_accepted:cli-sub-1"));
    assert!(stdout.contains("subscription_event:cli-sub-1"));
    assert!(stdout.contains("query_result:cli-query-1"));
    assert!(stdout.contains("failure:cli-bad-command-1:ingress_command_kind_mismatch"));

    handle.join().expect("adp mock join");
}

#[test]
fn cli_runs_adp_success_turn_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_sample_mock_server(TerminalStatus::Success);

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("adp-turn-sample")
        .arg("--url")
        .arg(&url)
        .arg("--sample")
        .arg("success")
        .output()
        .expect("run adp success sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("adp_turn_sample_ok"));
    assert!(stdout.contains("sample=success"));
    assert!(stdout.contains("subscription_accepted:cli-sample-success-sub"));
    assert!(stdout.contains("command_receipt:cli-sample-success-cmd:sample_success"));
    assert!(stdout.contains("rounds=1"));

    handle.join().expect("adp success sample mock join");
}

#[test]
fn cli_runs_adp_failure_turn_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_sample_mock_server(TerminalStatus::Failed);

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("adp-turn-sample")
        .arg("--url")
        .arg(&url)
        .arg("--sample")
        .arg("failure")
        .output()
        .expect("run adp failure sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("adp_turn_sample_ok"));
    assert!(stdout.contains("sample=failure"));
    assert!(stdout.contains("subscription_accepted:cli-sample-failure-sub"));
    assert!(
        stdout.contains("command_receipt:cli-sample-failure-cmd:sample_tool_failure_recovered")
    );
    assert!(stdout.contains("rounds=2"));
    assert!(stdout.contains("tool_executions=1"));
    assert!(stdout.contains("failed_tools=1"));

    handle.join().expect("adp failure sample mock join");
}

#[test]
fn cli_runs_adp_session_query_against_mock_websocket() {
    let (url, handle) = spawn_adp_session_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("adp-session-query")
        .arg("--url")
        .arg(&url)
        .arg("--session")
        .arg("cli-session")
        .output()
        .expect("run adp session query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("adp_session_query_ok"));
    assert!(stdout.contains("sessions=1"));
    assert!(stdout.contains("ids=cli-session:2:success"));
    assert!(stdout.contains("selected_session=cli-session"));
    assert!(stdout.contains("turns=2"));
    assert!(stdout.contains("turn_ids=runtime-turn-2,runtime-turn-10"));

    handle.join().expect("adp session query mock join");
}

#[test]
fn cli_runs_adp_session_manage_rollback_against_mock_websocket() {
    let (url, handle) = spawn_adp_session_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("adp-session-manage")
        .arg("--url")
        .arg(&url)
        .arg("--action")
        .arg("rollback")
        .arg("--session")
        .arg("cli-session")
        .output()
        .expect("run adp session manage");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("adp_session_manage_ok"));
    assert!(stdout.contains("action=rollback"));
    assert!(stdout.contains("target=reason.persistence"));
    assert!(stdout.contains("session_turn_rolled_back:runtime-turn-10"));

    handle.join().expect("adp session manage mock join");
}

#[test]
fn cli_runs_reason_e2e_usage_compaction_smoke() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
pair_token = "FREEHAND_WORKER_TOKEN"
provider = "mini27"
"#,
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .env("FREEHAND_WORKER_TOKEN", "worker-secret")
        .arg("reason-e2e")
        .arg("--agent")
        .arg("master")
        .arg("--scenario")
        .arg("usage-compaction")
        .output()
        .expect("run cli");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("scenario=usage-compaction"));
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("rewrite_action=StageCompaction"));
    assert!(stdout.contains("rewrite_version=1"));
    assert!(stdout.contains("latest_usage_tokens=80"));
    assert!(stdout.contains("blocked=false"));

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_e2e_recovery_block_smoke() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
pair_token = "FREEHAND_WORKER_TOKEN"
provider = "mini27"
"#,
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .env("FREEHAND_WORKER_TOKEN", "worker-secret")
        .arg("reason-e2e")
        .arg("--agent")
        .arg("master")
        .arg("--scenario")
        .arg("recovery-block")
        .output()
        .expect("run cli");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("scenario=recovery-block"));
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("rewrite_action=Block"));
    assert!(stdout.contains("rewrite_version=0"));
    assert!(stdout.contains("latest_usage_tokens=none"));
    assert!(stdout.contains("blocked=true"));

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_persist_smoke() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
pair_token = "FREEHAND_WORKER_TOKEN"
provider = "mini27"
"#,
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .env("FREEHAND_WORKER_TOKEN", "worker-secret")
        .arg("reason-persist-smoke")
        .arg("--agent")
        .arg("master")
        .output()
        .expect("run cli");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("restored_terminal=persisted smoke terminal"));
    assert!(stdout.contains("reason_seq=3"));
    assert!(stdout.contains("ui_sidecar_exists=true"));
    assert!(stdout.contains("session_index_entries=1"));

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_live_single_shot_mock() {
    let (base_url, rx, handle) =
        spawn_mock_server(200, "application/json", complete_single_response("pong"));
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        format!(
            r#"
[providers.minimonth]
id = "minimonth"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "{base_url}"
default_model = "MiniMax-M2.7"

[providers.minimonth.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "minimonth"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
pair_token = "FREEHAND_WORKER_TOKEN"
provider = "minimonth"
"#
        ),
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .env("FREEHAND_WORKER_TOKEN", "worker-secret")
        .arg("reason-live")
        .arg("--agent")
        .arg("master")
        .arg("--prompt")
        .arg("reply exactly pong")
        .output()
        .expect("run cli");

    let raw_request = rx.recv().expect("request");
    handle.join().expect("join");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(raw_request.starts_with("POST /v1/messages HTTP/1.1"));
    assert!(stdout.contains("agent=master"));
    assert!(stdout.contains("provider=minimonth"));
    assert!(stdout.contains("stream=false"));
    assert!(stdout.contains("text=pong"));
    assert!(stdout.contains("usage_input_tokens=14"));
    assert!(stdout.contains("usage_output_tokens=82"));
    assert!(stdout.contains("rounds=1"));
    assert!(stdout.contains("schema_rejections=0"));
    assert!(stdout.contains("terminal=Summary: pong"));

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_live_stream_mock() {
    let (base_url, rx, handle) =
        spawn_mock_server(200, "text/event-stream", complete_stream_response("pong"));
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        format!(
            r#"
[providers.minimonth]
id = "minimonth"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "{base_url}"
default_model = "MiniMax-M2.7"

[providers.minimonth.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "minimonth"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
pair_token = "FREEHAND_WORKER_TOKEN"
provider = "minimonth"
"#
        ),
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .env("FREEHAND_WORKER_TOKEN", "worker-secret")
        .arg("reason-live")
        .arg("--agent")
        .arg("master")
        .arg("--prompt")
        .arg("reply exactly pong")
        .arg("--stream")
        .output()
        .expect("run cli");

    let raw_request = rx.recv().expect("request");
    handle.join().expect("join");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(raw_request.starts_with("POST /v1/messages HTTP/1.1"));
    assert!(stdout.contains("stream=true"));
    assert!(stdout.contains("text=pong"));
    assert!(stdout.contains("reasoning_events="));
    assert!(stdout.contains("usage_input_tokens=14"));
    assert!(stdout.contains("rounds=1"));
    assert!(stdout.contains("schema_rejections=0"));
    assert!(stdout.contains("terminal=Summary: pong"));

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_live_tool_call_mock_and_persists() {
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_single_response(),
            complete_single_response("tool done"),
        ],
    );
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        format!(
            r#"
[providers.minimonth]
id = "minimonth"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "{base_url}"
default_model = "MiniMax-M2.7"

[providers.minimonth.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "minimonth"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
pair_token = "FREEHAND_WORKER_TOKEN"
provider = "minimonth"
"#
        ),
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .env("FREEHAND_WORKER_TOKEN", "worker-secret")
        .arg("reason-live")
        .arg("--agent")
        .arg("master")
        .arg("--prompt")
        .arg("call read_file then finish")
        .arg("--session")
        .arg("cli-tool-session")
        .output()
        .expect("run cli");

    let first_request = rx.recv().expect("first request");
    let second_request = rx.recv().expect("second request");
    handle.join().expect("join");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(first_request.contains("\"tools\""));
    assert!(first_request.contains("\"name\":\"read_file\""));
    assert!(!first_request.contains("\"tool_choice\""));
    assert!(second_request.contains("\"type\":\"tool_result\""));
    assert!(second_request.contains("toolu_read_1"));
    assert!(second_request.contains("Cargo.toml"));
    assert!(stdout.contains("text=tool done"));
    assert!(stdout.contains("rounds=2"));
    assert!(stdout.contains("tool_executions=1"));
    assert!(stdout.contains("restore_status=created_new"));
    assert!(stdout.contains("terminal=Summary: pong"));
    assert!(
        freehand_dir
            .join("state")
            .join("turns")
            .join("master")
            .join("cli-tool-session")
            .join("session-history.json")
            .is_file()
    );
    assert!(
        freehand_dir
            .join("ledgers")
            .join("reason")
            .join("master")
            .join("cli-tool-session.jsonl")
            .is_file()
    );

    fs::remove_dir_all(home).expect("cleanup");
}

#[test]
fn cli_runs_reason_live_unsupported_provider_smoke() {
    let home = unique_home_dir();
    let freehand_dir = home.join(".freehand");
    fs::create_dir_all(&freehand_dir).expect("create runtime home");
    fs::write(
        freehand_dir.join("config.toml"),
        r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "chat_completions"
base_url = "http://127.0.0.1:1"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
pair_token = "FREEHAND_WORKER_TOKEN"
provider = "mini27"
"#,
    )
    .expect("write config");

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .env("HOME", &home)
        .env("FREEHAND_CLI_TOKEN", "cli-secret")
        .env("FREEHAND_WORKER_TOKEN", "worker-secret")
        .arg("reason-live")
        .arg("--agent")
        .arg("master")
        .arg("--prompt")
        .arg("reply exactly pong")
        .output()
        .expect("run cli");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(stderr.contains("is not supported"));

    fs::remove_dir_all(home).expect("cleanup");
}
