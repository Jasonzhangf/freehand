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
    SubscriptionSelector, UiAdpFailure, UiAdpRequest, UiAdpResponse, UiAgentBoardProjection,
    UiAgentLifecycleProjection, UiAgentSnapshotProjection, UiClientKind, UiCommand,
    UiCommandDispatchReceipt, UiExecutionFactKind, UiMasterPollClassificationProjection,
    UiMasterPollProjection, UiModelRequestActivity, UiModelRequestKind, UiModelRequestStatus,
    UiProjection, UiQueryResult, UiSessionListProjection, UiSessionSummary,
    UiSessionTranscriptProjection, UiSource, UiStreamKind, UiSubscriptionEvent,
    UiTaskBoardProjection, UiTaskDispatchCommand, UiTaskEventInboxEntryProjection,
    UiTaskEventInboxProjection, UiTaskHistoryProjection, UiTaskLedgerEventProjection,
    UiTaskListProjection, UiTaskSnapshotProjection, UiToolActivity, UiToolActivityStatus,
    UiTurnProjection, UiWorkerControlEventProjection, UiWorkerControlProjection,
    build_command_dispatch_envelope,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum MockAdpSampleKind {
    Success,
    Failure,
    SchemaMismatch,
    ProviderRetry,
}

fn spawn_adp_sample_mock_server(kind: MockAdpSampleKind) -> (String, thread::JoinHandle<()>) {
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
                        Ok(Message::Close(_)) => break,
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
                            let turn = test_sample_turn_projection(&text, &session_id, kind);
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
                                        dispatch_status: match kind {
                                            MockAdpSampleKind::Success => "sample_success",
                                            MockAdpSampleKind::Failure => {
                                                "sample_tool_failure_recovered"
                                            }
                                            MockAdpSampleKind::SchemaMismatch => {
                                                "sample_schema_polished"
                                            }
                                            MockAdpSampleKind::ProviderRetry => {
                                                "sample_provider_retry_exhausted"
                                            }
                                        }
                                        .to_owned(),
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
                                let turns = match kind {
                                    MockAdpSampleKind::Success => vec![turn],
                                    MockAdpSampleKind::Failure
                                    | MockAdpSampleKind::SchemaMismatch => {
                                        let mut first = turn.clone();
                                        first.turn_id = TurnId::new("cli-adp-sample-turn");
                                        first.terminal_status = None;
                                        first.terminal_text = None;
                                        if kind == MockAdpSampleKind::SchemaMismatch {
                                            first.model_request = Some(UiModelRequestActivity {
                                                status: UiModelRequestStatus::Waiting,
                                                kind: UiModelRequestKind::SchemaRetry,
                                                detail: Some(
                                                    "schema polishing #1: missing completion schema"
                                                        .to_owned(),
                                                ),
                                            });
                                        }
                                        let mut second = turn;
                                        second.turn_id = TurnId::new("cli-adp-sample-turn-r2");
                                        vec![first, second]
                                    }
                                    MockAdpSampleKind::ProviderRetry => vec![turn],
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

fn spawn_adp_session_continue_mock_server() -> (String, thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp continuation mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    let prompts = Arc::new(Mutex::new(Vec::<(SessionId, String)>::new()));
    listener
        .set_nonblocking(true)
        .expect("set adp continuation mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            for _ in 0..3 {
                let (stream, _) = listener.accept().await.expect("accept adp");
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("accept websocket");
                while let Some(message) = socket.next().await {
                    let text = match message {
                        Ok(Message::Text(text)) => text,
                        Ok(Message::Close(_)) => break,
                        Ok(_) => continue,
                        Err(_) => break,
                    };
                    let request: UiAdpRequest = serde_json::from_str(&text).expect("adp request");
                    match request {
                        UiAdpRequest::Command {
                            request_id,
                            command:
                                UiCommand::SubmitUserInput {
                                    text, session_id, ..
                                },
                        } => {
                            let session_id =
                                session_id.unwrap_or_else(|| SessionId::new("cli-session"));
                            prompts
                                .lock()
                                .expect("continuation prompt lock")
                                .push((session_id, text));
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::CommandReceipt {
                                    request_id,
                                    receipt: UiCommandDispatchReceipt {
                                        ingress: freehand_ui_protocol::UiCommandIngressAck {
                                            command_kind: "submit_user_input".to_owned(),
                                            accepted: true,
                                            status_text: "accepted".to_owned(),
                                            mutation_authority: "runtime".to_owned(),
                                        },
                                        target_feature_id: "reason.turn".to_owned(),
                                        target_owner_module: "crates/freehand-reason".to_owned(),
                                        dispatch_status: "sample_turn_complete".to_owned(),
                                    },
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QuerySessionTurns { session_id },
                        } => {
                            let stored = prompts.lock().expect("continuation prompt lock").clone();
                            let token = stored
                                .first()
                                .and_then(|(_, prompt)| {
                                    prompt
                                        .split_whitespace()
                                        .find(|part| part.starts_with("FHCLI"))
                                })
                                .unwrap_or("FHCLI-missing")
                                .trim_end_matches('.')
                                .to_owned();
                            let mut first = test_turn_projection();
                            first.session_id = session_id.clone();
                            first.turn_id = TurnId::new("cli-session-continue-turn-1");
                            first.user_text = stored.first().map(|(_, text)| text.clone());
                            first.terminal_status = Some(TerminalStatus::Success);
                            first.terminal_text = Some(format!("remembered {token}"));
                            let mut second = test_turn_projection();
                            second.session_id = session_id.clone();
                            second.turn_id = TurnId::new("cli-session-continue-turn-2");
                            second.user_text = stored.get(1).map(|(_, text)| text.clone());
                            second.terminal_status = Some(TerminalStatus::Success);
                            second.terminal_text = Some(format!("token {token}"));
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
                        UiAdpRequest::Query { request_id, .. }
                        | UiAdpRequest::Command { request_id, .. }
                        | UiAdpRequest::Subscribe { request_id, .. } => {
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::Failure {
                                    request_id,
                                    failure: UiAdpFailure {
                                        code: "unexpected_continuation_frame".to_owned(),
                                        message: "unexpected continuation frame".to_owned(),
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

fn spawn_adp_task_lifecycle_mock_server() -> (String, thread::JoinHandle<()>) {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp task lifecycle mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    let token = Arc::new(Mutex::new(None::<String>));
    let task_id = Arc::new(Mutex::new("task-cli-lifecycle-1".to_owned()));
    listener
        .set_nonblocking(true)
        .expect("set adp task lifecycle mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            for _ in 0..6 {
                let (stream, _) = listener.accept().await.expect("accept adp");
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("accept websocket");
                while let Some(message) = socket.next().await {
                    let text = match message {
                        Ok(Message::Text(text)) => text,
                        Ok(Message::Close(_)) => break,
                        Ok(_) => continue,
                        Err(_) => break,
                    };
                    let request: UiAdpRequest = serde_json::from_str(&text).expect("adp request");
                    match request {
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CreateTask { task },
                        } => {
                            let parsed_token = task
                                .title
                                .split_whitespace()
                                .find(|part| part.starts_with("FHTASK"))
                                .unwrap_or("FHTASK-missing")
                                .trim_end_matches(',')
                                .to_owned();
                            *token.lock().expect("task token lock") = Some(parsed_token);
                            *task_id.lock().expect("task id lock") = task
                                .task_id
                                .unwrap_or_else(|| "task-cli-lifecycle-1".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "create_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_created",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::SubmitTaskReview { .. },
                        } => {
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "submit_task_review",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_review_submitted",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::ApproveTaskReview { .. },
                        } => {
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "approve_task_review",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_review_approved",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CloseTask { .. },
                        } => {
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "close_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_closed",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::SubmitUserInput { .. },
                        } => {
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::Failure {
                                    request_id,
                                    failure: UiAdpFailure {
                                        code: "unexpected_submit_user_input".to_owned(),
                                        message: "task lifecycle sample must use task commands"
                                            .to_owned(),
                                        retryable: false,
                                    },
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskList { .. },
                        } => {
                            let token = token
                                .lock()
                                .expect("task token lock")
                                .clone()
                                .unwrap_or_else(|| "FHTASK-missing".to_owned());
                            let task_id = task_id.lock().expect("task id lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskList(UiTaskListProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        status_filter: None,
                                        agent_filter: None,
                                        tasks: vec![UiTaskSnapshotProjection {
                                            task_id,
                                            status: "Closed".to_owned(),
                                            title: format!("Lifecycle {token}"),
                                            goal: format!("Close task {token}"),
                                            priority: 0,
                                            target_cwd: Some("/tmp/cli-session".to_owned()),
                                            parent_session_id: None,
                                            assignee_agent_id: Some(AgentId::new("cli-agent")),
                                            active_execution_id: None,
                                            updated_at: 1,
                                            last_progress_at: Some(1),
                                            last_event_seq: 4,
                                        }],
                                    }),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskHistory { task_id },
                        } => {
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskHistory(UiTaskHistoryProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        task_id,
                                        events: vec![
                                            task_event(1, "TaskCreated"),
                                            task_event(2, "TaskReviewSubmitted"),
                                            task_event(3, "TaskReviewApproved"),
                                            task_event(4, "TaskClosed"),
                                        ],
                                    }),
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
                                        code: "unexpected_task_lifecycle_frame".to_owned(),
                                        message: "unexpected task lifecycle frame".to_owned(),
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

fn spawn_adp_phase1_foundation_mock_server() -> (String, thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp phase1 mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    let blocked_task_id = Arc::new(Mutex::new("task-cli-phase1-blocked".to_owned()));
    let review_task_id = Arc::new(Mutex::new("task-cli-phase1-review".to_owned()));
    let execution_id = Arc::new(Mutex::new("exec-cli-phase1".to_owned()));
    let stale_seen = Arc::new(Mutex::new(false));
    listener
        .set_nonblocking(true)
        .expect("set adp phase1 mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            for _ in 0..12 {
                let (stream, _) = listener.accept().await.expect("accept adp");
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("accept websocket");
                while let Some(message) = socket.next().await {
                    let text = match message {
                        Ok(Message::Text(text)) => text,
                        Ok(Message::Close(_)) => break,
                        Ok(_) => continue,
                        Err(_) => break,
                    };
                    let request: UiAdpRequest = serde_json::from_str(&text).expect("adp request");
                    match request {
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CreateTask { task },
                        } => {
                            let task_id = task.task_id.expect("phase1 task id");
                            if task_id.contains("review") {
                                *review_task_id.lock().expect("review task lock") = task_id;
                            } else {
                                *blocked_task_id.lock().expect("blocked task lock") = task_id;
                            }
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "create_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_created",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::ApplyExecutionFact { fact },
                        } => {
                            *execution_id.lock().expect("execution id lock") = fact.execution_id;
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "apply_execution_fact",
                                "task.orchestration",
                                "crates/freehand-task",
                                "execution_fact_applied",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::RunSchedulerTick { .. },
                        } => {
                            *stale_seen.lock().expect("stale lock") = true;
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "run_scheduler_tick",
                                "task.orchestration",
                                "crates/freehand-task",
                                "scheduler_tick_recorded:facts=2 events=2",
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskBoard { .. },
                        } => {
                            let blocked =
                                blocked_task_id.lock().expect("blocked task lock").clone();
                            let review = review_task_id.lock().expect("review task lock").clone();
                            let stale = *stale_seen.lock().expect("stale lock");
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskBoard(phase1_task_board(
                                        &blocked, &review, stale,
                                    )),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryAgentBoard,
                        } => {
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::AgentBoard(UiAgentBoardProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        agents: vec![phase1_agent_lifecycle("idle")],
                                    }),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryAgentLifecycle { .. },
                        } => {
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::AgentLifecycle(phase1_agent_lifecycle(
                                        "idle",
                                    )),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskHistory { task_id },
                        } => {
                            let execution = execution_id.lock().expect("execution id lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskHistory(UiTaskHistoryProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        task_id,
                                        events: vec![
                                            task_event(1, "TaskCreated"),
                                            task_event(2, "TaskResumed"),
                                            task_event_with_payload(
                                                3,
                                                "TaskExecutionRecovering",
                                                serde_json::json!({
                                                    "execution_id": execution,
                                                }),
                                            ),
                                            task_event(4, "TaskBlocked"),
                                        ],
                                    }),
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
                                        code: "unexpected_phase1_frame".to_owned(),
                                        message: "unexpected phase1 frame".to_owned(),
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

fn spawn_adp_master_worker_foundation_mock_server() -> (String, thread::JoinHandle<()>) {
    spawn_adp_master_worker_foundation_mock_server_with_connections(18)
}

fn spawn_adp_master_worker_foundation_verify_mock_server() -> (String, thread::JoinHandle<()>) {
    spawn_adp_master_worker_foundation_mock_server_with_connections(4)
}

fn spawn_adp_master_worker_foundation_mock_server_with_connections(
    connection_count: usize,
) -> (String, thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp master-worker mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    let task_id = Arc::new(Mutex::new("task-cli-master-worker-verify".to_owned()));
    let execution_id = Arc::new(Mutex::new("exec-cli-master-worker-verify".to_owned()));
    let agent_id = Arc::new(Mutex::new("worker-cli-master-worker-verify".to_owned()));
    let events = Arc::new(Mutex::new(vec![
        "TaskCreated".to_owned(),
        "TaskAssigned".to_owned(),
        "TaskResumed".to_owned(),
        "TaskExecutionRecorded".to_owned(),
        "TaskBlocked".to_owned(),
        "TaskExecutionRecovering".to_owned(),
        "TaskReviewSubmitted".to_owned(),
        "TaskReviewRejected".to_owned(),
        "TaskExecutionRecorded".to_owned(),
        "TaskReviewSubmitted".to_owned(),
        "TaskReviewApproved".to_owned(),
        "TaskClosed".to_owned(),
    ]));
    listener
        .set_nonblocking(true)
        .expect("set adp master-worker mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            for _ in 0..connection_count {
                let (stream, _) = listener.accept().await.expect("accept adp");
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("accept websocket");
                while let Some(message) = socket.next().await {
                    let text = match message {
                        Ok(Message::Text(text)) => text,
                        Ok(Message::Close(_)) => break,
                        Ok(_) => continue,
                        Err(_) => break,
                    };
                    let request: UiAdpRequest = serde_json::from_str(&text).expect("adp request");
                    match request {
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CreateTaskAgent { agent },
                        } => {
                            *agent_id.lock().expect("phase2a agent lock") =
                                agent.agent_id.as_str().to_owned();
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "create_task_agent",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_agent_created",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CreateTask { task },
                        } => {
                            if task.dispatch != Some(UiTaskDispatchCommand::None) {
                                send_adp_response(
                                    &mut socket,
                                    UiAdpResponse::Failure {
                                        request_id,
                                        failure: UiAdpFailure {
                                            code: "missing_dispatch_none".to_owned(),
                                            message:
                                                "master-worker sample must create waiting task"
                                                    .to_owned(),
                                            retryable: false,
                                        },
                                    },
                                )
                                .await;
                                continue;
                            }
                            *task_id.lock().expect("phase2a task lock") =
                                task.task_id.expect("phase2a task id");
                            *events.lock().expect("phase2a events lock") =
                                vec!["TaskCreated".to_owned()];
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "create_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_created",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::AssignTask { .. },
                        } => {
                            events
                                .lock()
                                .expect("phase2a events lock")
                                .push("TaskAssigned".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "assign_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_assigned",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::ClaimNextTask { claim },
                        } => {
                            *execution_id.lock().expect("phase2a execution lock") =
                                claim.execution_id;
                            events
                                .lock()
                                .expect("phase2a events lock")
                                .push("TaskResumed".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "claim_next_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_claimed",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::ApplyExecutionFact { fact },
                        } => {
                            *execution_id.lock().expect("phase2a execution lock") =
                                fact.execution_id;
                            let event_type = match fact.kind {
                                UiExecutionFactKind::Running { .. } => "TaskExecutionRecorded",
                                UiExecutionFactKind::Recovering { .. } => "TaskExecutionRecovering",
                                UiExecutionFactKind::Blocked { .. } => "TaskBlocked",
                                UiExecutionFactKind::Interrupted { .. } => "TaskInterrupted",
                                UiExecutionFactKind::ReviewReady { .. } => "TaskReviewSubmitted",
                            };
                            events
                                .lock()
                                .expect("phase2a events lock")
                                .push(event_type.to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "apply_execution_fact",
                                "task.orchestration",
                                "crates/freehand-task",
                                "execution_fact_applied",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::RejectTaskReview { .. },
                        } => {
                            events
                                .lock()
                                .expect("phase2a events lock")
                                .push("TaskReviewRejected".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "reject_task_review",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_review_rejected",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::ApproveTaskReview { .. },
                        } => {
                            events
                                .lock()
                                .expect("phase2a events lock")
                                .push("TaskReviewApproved".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "approve_task_review",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_review_approved",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CloseTask { .. },
                        } => {
                            events
                                .lock()
                                .expect("phase2a events lock")
                                .push("TaskClosed".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "close_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_closed",
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskBoard { .. },
                        } => {
                            let task = task_id.lock().expect("phase2a task lock").clone();
                            let execution =
                                execution_id.lock().expect("phase2a execution lock").clone();
                            let agent = agent_id.lock().expect("phase2a agent lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskBoard(phase2a_task_board(
                                        &task, &execution, &agent,
                                    )),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryAgentBoard,
                        } => {
                            let agent = agent_id.lock().expect("phase2a agent lock").clone();
                            let execution =
                                execution_id.lock().expect("phase2a execution lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::AgentBoard(UiAgentBoardProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        agents: vec![phase2a_agent_lifecycle(
                                            &agent, &execution, "closed",
                                        )],
                                    }),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryAgentLifecycle { .. },
                        } => {
                            let agent = agent_id.lock().expect("phase2a agent lock").clone();
                            let execution =
                                execution_id.lock().expect("phase2a execution lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::AgentLifecycle(phase2a_agent_lifecycle(
                                        &agent, &execution, "closed",
                                    )),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query:
                                UiCommand::QueryTaskHistory {
                                    task_id: query_task,
                                },
                        } => {
                            let execution =
                                execution_id.lock().expect("phase2a execution lock").clone();
                            let events = events.lock().expect("phase2a events lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskHistory(UiTaskHistoryProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        task_id: query_task,
                                        events: events
                                            .iter()
                                            .enumerate()
                                            .map(|(index, event_type)| {
                                                task_event_with_payload(
                                                    u64::try_from(index + 1).expect("event seq"),
                                                    event_type,
                                                    serde_json::json!({
                                                        "execution_id": execution,
                                                    }),
                                                )
                                            })
                                            .collect(),
                                    }),
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
                                        code: "unexpected_master_worker_frame".to_owned(),
                                        message: "unexpected master worker frame".to_owned(),
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

fn spawn_adp_master_worker_autonomy_mock_server() -> (String, thread::JoinHandle<()>) {
    spawn_adp_master_worker_autonomy_mock_server_with_connections(
        18,
        master_autonomy_truth_for(
            "reject-retry",
            "cli-master-autonomy-reject-retry-verify",
            "task-cli-master-autonomy-reject-retry-verify",
            "exec-cli-master-autonomy-reject-retry-verify",
            "worker",
            "FHAUTO-verify",
        ),
    )
}

fn spawn_adp_master_worker_autonomy_verify_mock_server() -> (String, thread::JoinHandle<()>) {
    spawn_adp_master_worker_autonomy_mock_server_with_connections(
        4,
        master_autonomy_truth_for(
            "reject-retry",
            "cli-master-autonomy-reject-retry-verify",
            "task-cli-master-autonomy-reject-retry-verify",
            "exec-cli-master-autonomy-reject-retry-verify",
            "worker",
            "FHAUTO-verify",
        ),
    )
}

fn spawn_adp_master_worker_autonomy_mock_server_with_connections(
    connection_count: usize,
    initial_truth: MockMasterWorkerAutonomyTruth,
) -> (String, thread::JoinHandle<()>) {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp master-autonomy mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    let truth = Arc::new(Mutex::new(initial_truth));
    listener
        .set_nonblocking(true)
        .expect("set adp master-autonomy mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            for _ in 0..connection_count {
                let (stream, _) = listener.accept().await.expect("accept adp");
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("accept websocket");
                let mut subscription_id = None::<String>;
                while let Some(message) = socket.next().await {
                    let text = match message {
                        Ok(Message::Text(text)) => text,
                        Ok(Message::Close(_)) => break,
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
                                session_id.unwrap_or_else(|| SessionId::new("cli-master-autonomy"));
                            let parsed = master_autonomy_truth_from_prompt(&text, &session_id);
                            *truth.lock().expect("master autonomy truth lock") = parsed.clone();
                            let envelope =
                                build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                                    text: text.clone(),
                                    session_id: Some(session_id.clone()),
                                    cwd: None,
                                })
                                .expect("master autonomy envelope");
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::CommandReceipt {
                                    request_id,
                                    receipt: UiCommandDispatchReceipt {
                                        ingress: envelope.ingress,
                                        target_feature_id: envelope.target_feature_id,
                                        target_owner_module: envelope.target_owner_module,
                                        dispatch_status: format!(
                                            "master_autonomy_model_turn_complete:{}",
                                            parsed.scenario
                                        ),
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
                                            projection: UiProjection::Turn(
                                                master_autonomy_turn_projection(&parsed),
                                            ),
                                            latest_active_turn_id: Some(TurnId::new(
                                                "cli-master-autonomy-turn",
                                            )),
                                        },
                                    },
                                )
                                .await;
                            }
                        }
                        UiAdpRequest::Command { request_id, .. } => {
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::Failure {
                                    request_id,
                                    failure: UiAdpFailure {
                                        code: "direct_task_mutation_forbidden".to_owned(),
                                        message: "master autonomy sample must submit one user prompt; task mutations must come from the model tool loop".to_owned(),
                                        retryable: false,
                                    },
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QuerySessionTurns { .. },
                        } => {
                            let truth = truth.lock().expect("master autonomy truth lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::SessionTurns(
                                        UiSessionTranscriptProjection {
                                            session_id: truth.session_id.clone(),
                                            title: None,
                                            archived: false,
                                            cwd: Some("/tmp/cli-session".to_owned()),
                                            turns: vec![master_autonomy_turn_projection(&truth)],
                                        },
                                    ),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskBoard { .. },
                        } => {
                            let truth = truth.lock().expect("master autonomy truth lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskBoard(master_autonomy_task_board(
                                        &truth,
                                    )),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryAgentBoard,
                        } => {
                            let truth = truth.lock().expect("master autonomy truth lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::AgentBoard(UiAgentBoardProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        agents: vec![phase2a_agent_lifecycle(
                                            &truth.agent_id,
                                            &truth.execution_id,
                                            &truth.lifecycle_state,
                                        )],
                                    }),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryAgentLifecycle { .. },
                        } => {
                            let truth = truth.lock().expect("master autonomy truth lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::AgentLifecycle(
                                        phase2a_agent_lifecycle(
                                            &truth.agent_id,
                                            &truth.execution_id,
                                            &truth.lifecycle_state,
                                        ),
                                    ),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskHistory { task_id },
                        } => {
                            let truth = truth.lock().expect("master autonomy truth lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskHistory(UiTaskHistoryProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        task_id,
                                        events: truth
                                            .events
                                            .iter()
                                            .enumerate()
                                            .map(|(index, event_type)| {
                                                task_event_with_payload(
                                                    u64::try_from(index + 1).expect("event seq"),
                                                    event_type,
                                                    serde_json::json!({
                                                        "execution_id": truth.execution_id,
                                                    }),
                                                )
                                            })
                                            .collect(),
                                    }),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query { request_id, .. } => {
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::Failure {
                                    request_id,
                                    failure: UiAdpFailure {
                                        code: "unexpected_master_autonomy_frame".to_owned(),
                                        message: "unexpected master autonomy frame".to_owned(),
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

fn spawn_adp_master_poll_foundation_mock_server() -> (String, thread::JoinHandle<()>) {
    spawn_adp_master_poll_foundation_mock_server_with_connections(19, false)
}

fn spawn_adp_master_poll_foundation_verify_mock_server() -> (String, thread::JoinHandle<()>) {
    spawn_adp_master_poll_foundation_mock_server_with_connections(4, true)
}

fn spawn_adp_master_poll_foundation_mock_server_with_connections(
    connection_count: usize,
    initial_poll_processed: bool,
) -> (String, thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp master-poll mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    let task_id = Arc::new(Mutex::new("task-cli-master-poll-verify".to_owned()));
    let execution_id = Arc::new(Mutex::new("exec-cli-master-poll-verify".to_owned()));
    let agent_id = Arc::new(Mutex::new("worker-cli-master-poll-verify".to_owned()));
    let cursor = Arc::new(Mutex::new(
        "00000000000000000008:task-cli-master-poll-verify:00000000000000000008:task-cli-master-poll-verify:8".to_owned(),
    ));
    let poll_processed = Arc::new(Mutex::new(initial_poll_processed));
    let events = Arc::new(Mutex::new(vec![
        "TaskCreated".to_owned(),
        "TaskAssigned".to_owned(),
        "TaskResumed".to_owned(),
        "TaskExecutionRecorded".to_owned(),
        "TaskSchedulerTick".to_owned(),
        "TaskBlocked".to_owned(),
        "TaskExecutionRecovering".to_owned(),
        "TaskReviewSubmitted".to_owned(),
    ]));
    listener
        .set_nonblocking(true)
        .expect("set adp master-poll mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            for _ in 0..connection_count {
                let (stream, _) = listener.accept().await.expect("accept adp");
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("accept websocket");
                while let Some(message) = socket.next().await {
                    let text = match message {
                        Ok(Message::Text(text)) => text,
                        Ok(Message::Close(_)) => break,
                        Ok(_) => continue,
                        Err(_) => break,
                    };
                    let request: UiAdpRequest = serde_json::from_str(&text).expect("adp request");
                    match request {
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CreateTaskAgent { agent },
                        } => {
                            *agent_id.lock().expect("phase2b agent lock") =
                                agent.agent_id.as_str().to_owned();
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "create_task_agent",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_agent_created",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CreateTask { task },
                        } => {
                            if task.dispatch != Some(UiTaskDispatchCommand::None) {
                                send_adp_response(
                                    &mut socket,
                                    UiAdpResponse::Failure {
                                        request_id,
                                        failure: UiAdpFailure {
                                            code: "missing_dispatch_none".to_owned(),
                                            message: "master-poll sample must create waiting task"
                                                .to_owned(),
                                            retryable: false,
                                        },
                                    },
                                )
                                .await;
                                continue;
                            }
                            let task = task.task_id.expect("phase2b task id");
                            *task_id.lock().expect("phase2b task lock") = task.clone();
                            *cursor.lock().expect("phase2b cursor lock") =
                                format!("00000000000000000008:{task}:00000000000000000008:{task}:8");
                            *poll_processed.lock().expect("phase2b poll lock") = false;
                            *events.lock().expect("phase2b events lock") =
                                vec!["TaskCreated".to_owned()];
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "create_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_created",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::AssignTask { .. },
                        } => {
                            events
                                .lock()
                                .expect("phase2b events lock")
                                .push("TaskAssigned".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "assign_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_assigned",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::ClaimNextTask { claim },
                        } => {
                            *execution_id.lock().expect("phase2b execution lock") =
                                claim.execution_id;
                            events
                                .lock()
                                .expect("phase2b events lock")
                                .push("TaskResumed".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "claim_next_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_claimed",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::ApplyExecutionFact { fact },
                        } => {
                            *execution_id.lock().expect("phase2b execution lock") =
                                fact.execution_id;
                            let event_type = match fact.kind {
                                UiExecutionFactKind::Running { .. } => "TaskExecutionRecorded",
                                UiExecutionFactKind::Recovering { .. } => "TaskExecutionRecovering",
                                UiExecutionFactKind::Blocked { .. } => "TaskBlocked",
                                UiExecutionFactKind::Interrupted { .. } => "TaskInterrupted",
                                UiExecutionFactKind::ReviewReady { .. } => "TaskReviewSubmitted",
                            };
                            events
                                .lock()
                                .expect("phase2b events lock")
                                .push(event_type.to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "apply_execution_fact",
                                "task.orchestration",
                                "crates/freehand-task",
                                "execution_fact_applied",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::RunSchedulerTick { .. },
                        } => {
                            events
                                .lock()
                                .expect("phase2b events lock")
                                .push("TaskSchedulerTick".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "run_scheduler_tick",
                                "task.orchestration",
                                "crates/freehand-task",
                                "scheduler_tick_recorded",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::RunMasterPoll { .. },
                        } => {
                            *poll_processed.lock().expect("phase2b poll lock") = true;
                            let cursor = cursor.lock().expect("phase2b cursor lock").clone();
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "run_master_poll",
                                "task.orchestration",
                                "crates/freehand-task",
                                &format!("master_poll_recorded:events=0 classifications=2 cursor={cursor}"),
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskBoard { include_terminal, .. },
                        } => {
                            let task = task_id.lock().expect("phase2b task lock").clone();
                            let execution =
                                execution_id.lock().expect("phase2b execution lock").clone();
                            let agent = agent_id.lock().expect("phase2b agent lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskBoard(phase2b_task_board(
                                        &task,
                                        &execution,
                                        &agent,
                                        include_terminal,
                                    )),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskHistory { task_id: query_task },
                        } => {
                            let execution =
                                execution_id.lock().expect("phase2b execution lock").clone();
                            let events = events.lock().expect("phase2b events lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskHistory(UiTaskHistoryProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        task_id: query_task,
                                        events: events
                                            .iter()
                                            .enumerate()
                                            .map(|(index, event_type)| {
                                                task_event_with_payload(
                                                    u64::try_from(index + 1).expect("event seq"),
                                                    event_type,
                                                    serde_json::json!({
                                                        "execution_id": execution,
                                                    }),
                                                )
                                            })
                                            .collect(),
                                    }),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryEventInbox { after_cursor, .. },
                        } => {
                            let task = task_id.lock().expect("phase2b task lock").clone();
                            let execution =
                                execution_id.lock().expect("phase2b execution lock").clone();
                            let agent = agent_id.lock().expect("phase2b agent lock").clone();
                            let cursor = cursor.lock().expect("phase2b cursor lock").clone();
                            let events = events.lock().expect("phase2b events lock").clone();
                            let after_matches = after_cursor.as_deref() == Some(cursor.as_str());
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::EventInbox(phase2b_event_inbox(
                                        &task,
                                        &execution,
                                        &agent,
                                        &cursor,
                                        if after_matches { Vec::new() } else { events },
                                        after_cursor,
                                    )),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::RunMasterPoll { .. },
                        } => {
                            let task = task_id.lock().expect("phase2b task lock").clone();
                            let execution =
                                execution_id.lock().expect("phase2b execution lock").clone();
                            let agent = agent_id.lock().expect("phase2b agent lock").clone();
                            let cursor = cursor.lock().expect("phase2b cursor lock").clone();
                            let events = events.lock().expect("phase2b events lock").clone();
                            let already_processed =
                                *poll_processed.lock().expect("phase2b poll lock");
                            *poll_processed.lock().expect("phase2b poll lock") = true;
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::MasterPoll(phase2b_master_poll(
                                        &task,
                                        &execution,
                                        &agent,
                                        &cursor,
                                        if already_processed { Vec::new() } else { events },
                                        already_processed,
                                    )),
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
                                        code: "unexpected_master_poll_frame".to_owned(),
                                        message: "unexpected master poll frame".to_owned(),
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

fn spawn_adp_worker_control_foundation_mock_server() -> (String, thread::JoinHandle<()>) {
    spawn_adp_worker_control_foundation_mock_server_with_connections(16)
}

fn spawn_adp_worker_control_foundation_verify_mock_server() -> (String, thread::JoinHandle<()>) {
    spawn_adp_worker_control_foundation_mock_server_with_connections(3)
}

fn spawn_adp_worker_control_foundation_mock_server_with_connections(
    connection_count: usize,
) -> (String, thread::JoinHandle<()>) {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind adp worker-control mock");
    let url = format!("ws://{}/adp", listener.local_addr().expect("addr"));
    let task_id = Arc::new(Mutex::new("task-cli-worker-control-verify".to_owned()));
    let execution_id = Arc::new(Mutex::new("exec-cli-worker-control-verify".to_owned()));
    let agent_id = Arc::new(Mutex::new("worker-cli-worker-control-verify".to_owned()));
    let control_events = Arc::new(Mutex::new(worker_control_verify_events(
        "task-cli-worker-control-verify",
        "exec-cli-worker-control-verify",
        "worker-cli-worker-control-verify",
    )));
    let task_events = Arc::new(Mutex::new(vec![
        "TaskCreated".to_owned(),
        "TaskAssigned".to_owned(),
        "TaskResumed".to_owned(),
        "TaskExecutionRecorded".to_owned(),
        "TaskPaused".to_owned(),
        "TaskResumed".to_owned(),
        "TaskCancelled".to_owned(),
    ]));
    listener
        .set_nonblocking(true)
        .expect("set adp worker-control mock nonblocking");
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
            for _ in 0..connection_count {
                let (stream, _) = listener.accept().await.expect("accept adp");
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .expect("accept websocket");
                while let Some(message) = socket.next().await {
                    let text = match message {
                        Ok(Message::Text(text)) => text,
                        Ok(Message::Close(_)) => break,
                        Ok(_) => continue,
                        Err(_) => break,
                    };
                    let request: UiAdpRequest = serde_json::from_str(&text).expect("adp request");
                    match request {
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CreateTaskAgent { agent },
                        } => {
                            *agent_id.lock().expect("phase2c agent lock") =
                                agent.agent_id.as_str().to_owned();
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "create_task_agent",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_agent_created",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::CreateTask { task },
                        } => {
                            if task.dispatch != Some(UiTaskDispatchCommand::None) {
                                send_adp_response(
                                    &mut socket,
                                    UiAdpResponse::Failure {
                                        request_id,
                                        failure: UiAdpFailure {
                                            code: "missing_dispatch_none".to_owned(),
                                            message:
                                                "worker-control sample must create waiting task"
                                                    .to_owned(),
                                            retryable: false,
                                        },
                                    },
                                )
                                .await;
                                continue;
                            }
                            *task_id.lock().expect("phase2c task lock") =
                                task.task_id.expect("phase2c task id");
                            *control_events.lock().expect("phase2c control lock") = Vec::new();
                            *task_events.lock().expect("phase2c events lock") =
                                vec!["TaskCreated".to_owned()];
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "create_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_created",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::AssignTask { .. },
                        } => {
                            task_events
                                .lock()
                                .expect("phase2c events lock")
                                .push("TaskAssigned".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "assign_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_assigned",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::ClaimNextTask { claim },
                        } => {
                            *execution_id.lock().expect("phase2c execution lock") =
                                claim.execution_id;
                            task_events
                                .lock()
                                .expect("phase2c events lock")
                                .push("TaskResumed".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "claim_next_task",
                                "task.orchestration",
                                "crates/freehand-task",
                                "task_claimed",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::ApplyExecutionFact { fact },
                        } => {
                            *execution_id.lock().expect("phase2c execution lock") =
                                fact.execution_id;
                            task_events
                                .lock()
                                .expect("phase2c events lock")
                                .push("TaskExecutionRecorded".to_owned());
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "apply_execution_fact",
                                "task.orchestration",
                                "crates/freehand-task",
                                "execution_fact_applied",
                            )
                            .await;
                        }
                        UiAdpRequest::Command {
                            request_id,
                            command: UiCommand::WorkerControl { control },
                        } => {
                            let status = worker_control_status_for_op(&control.op);
                            let event = worker_control_event_projection(
                                control.control_id.as_deref().unwrap_or("wctl-generated"),
                                &control.op,
                                status,
                                &control.task_id,
                                &control.execution_id,
                                control.agent_id.as_str(),
                                control_events.lock().expect("phase2c control lock").len() + 1,
                            );
                            control_events
                                .lock()
                                .expect("phase2c control lock")
                                .push(event.clone());
                            match control.op.as_str() {
                                "pause" => task_events
                                    .lock()
                                    .expect("phase2c events lock")
                                    .push("TaskPaused".to_owned()),
                                "resume" => task_events
                                    .lock()
                                    .expect("phase2c events lock")
                                    .push("TaskResumed".to_owned()),
                                "cancel" => task_events
                                    .lock()
                                    .expect("phase2c events lock")
                                    .push("TaskCancelled".to_owned()),
                                _ => {}
                            }
                            send_task_command_receipt(
                                &mut socket,
                                request_id,
                                "worker_control",
                                "worker.control",
                                "crates/freehand-task",
                                &format!(
                                    "worker_control_applied:{}:{}:{}",
                                    event.op, event.control_id, event.status
                                ),
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryTaskBoard { .. },
                        } => {
                            let task = task_id.lock().expect("phase2c task lock").clone();
                            let execution =
                                execution_id.lock().expect("phase2c execution lock").clone();
                            let agent = agent_id.lock().expect("phase2c agent lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskBoard(phase2c_task_board(
                                        &task, &execution, &agent,
                                    )),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query: UiCommand::QueryWorkerControl { .. },
                        } => {
                            let events =
                                control_events.lock().expect("phase2c control lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::WorkerControl(Box::new(
                                        UiWorkerControlProjection {
                                            source_agent_id: AgentId::new("cli-agent"),
                                            generated_at: 10,
                                            event: events.last().cloned(),
                                            events,
                                            task: None,
                                            agent: None,
                                            lifecycle: None,
                                            task_event: None,
                                        },
                                    )),
                                },
                            )
                            .await;
                        }
                        UiAdpRequest::Query {
                            request_id,
                            query:
                                UiCommand::QueryTaskHistory {
                                    task_id: query_task,
                                },
                        } => {
                            let execution =
                                execution_id.lock().expect("phase2c execution lock").clone();
                            let events = task_events.lock().expect("phase2c events lock").clone();
                            send_adp_response(
                                &mut socket,
                                UiAdpResponse::QueryResult {
                                    request_id,
                                    result: UiQueryResult::TaskHistory(UiTaskHistoryProjection {
                                        source_agent_id: AgentId::new("cli-agent"),
                                        task_id: query_task,
                                        events: events
                                            .iter()
                                            .enumerate()
                                            .map(|(index, event_type)| {
                                                task_event_with_payload(
                                                    u64::try_from(index + 1).expect("event seq"),
                                                    event_type,
                                                    serde_json::json!({
                                                        "execution_id": execution,
                                                    }),
                                                )
                                            })
                                            .collect(),
                                    }),
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
                                        code: "unexpected_worker_control_frame".to_owned(),
                                        message: "unexpected worker control frame".to_owned(),
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

fn phase1_task_board(
    blocked_task_id: &str,
    review_task_id: &str,
    stale: bool,
) -> UiTaskBoardProjection {
    let blocked = phase1_task(blocked_task_id, "blocked");
    let review = phase1_task(review_task_id, "review_submitted");
    UiTaskBoardProjection {
        source_agent_id: AgentId::new("cli-agent"),
        status_filter: None,
        agent_filter: None,
        include_terminal: false,
        tasks: vec![blocked.clone(), review.clone()],
        agents: vec![UiAgentSnapshotProjection {
            agent_id: AgentId::new("cli-agent"),
            status: "available".to_owned(),
            current_task_id: None,
            current_cwd: Some("/tmp/cli-session".to_owned()),
            running_tasks: 0,
            queued_tasks: 0,
            last_seen_at: 1,
        }],
        blocked: vec![blocked.clone()],
        review_ready: vec![review],
        stale: if stale { vec![blocked] } else { Vec::new() },
    }
}

fn phase2a_task_board(task_id: &str, execution_id: &str, agent_id: &str) -> UiTaskBoardProjection {
    let final_task = phase2a_task(task_id, execution_id, agent_id, "closed");
    let blocked = phase2a_task(task_id, execution_id, agent_id, "blocked");
    let review = phase2a_task(task_id, execution_id, agent_id, "review_submitted");
    UiTaskBoardProjection {
        source_agent_id: AgentId::new("cli-agent"),
        status_filter: None,
        agent_filter: None,
        include_terminal: true,
        tasks: vec![final_task],
        agents: vec![UiAgentSnapshotProjection {
            agent_id: AgentId::new(agent_id.to_owned()),
            status: "available".to_owned(),
            current_task_id: None,
            current_cwd: Some("/tmp/cli-session".to_owned()),
            running_tasks: 0,
            queued_tasks: 0,
            last_seen_at: 1,
        }],
        blocked: vec![blocked],
        review_ready: vec![review],
        stale: Vec::new(),
    }
}

fn phase2a_task(
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
    status: &str,
) -> UiTaskSnapshotProjection {
    UiTaskSnapshotProjection {
        task_id: task_id.to_owned(),
        status: status.to_owned(),
        title: format!("Master worker {task_id}"),
        goal: "phase2a master worker proof".to_owned(),
        priority: 90,
        target_cwd: Some("/tmp/cli-session".to_owned()),
        parent_session_id: None,
        assignee_agent_id: Some(AgentId::new(agent_id.to_owned())),
        active_execution_id: Some(execution_id.to_owned()),
        updated_at: 1,
        last_progress_at: Some(1),
        last_event_seq: 12,
    }
}

fn phase2a_agent_lifecycle(
    agent_id: &str,
    execution_id: &str,
    state: &str,
) -> UiAgentLifecycleProjection {
    UiAgentLifecycleProjection {
        agent_id: AgentId::new(agent_id.to_owned()),
        role: "worker".to_owned(),
        alive: true,
        state: state.to_owned(),
        current_task_id: None,
        current_execution_id: Some(execution_id.to_owned()),
        current_turn_id: None,
        current_activity: None,
        last_activity: None,
        model_request_count: 1,
        model_retry_count: 1,
        tool_call_count: 0,
        tool_failure_count: 1,
        schema_polish_count: 0,
        provider_error_count: 0,
        blocked_count: 1,
        current_model: None,
        last_seen_at: 1,
        elapsed_ms: 0,
    }
}

fn phase2b_task_board(
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
    include_terminal: bool,
) -> UiTaskBoardProjection {
    let review = phase2b_task(task_id, execution_id, agent_id, "review_submitted");
    let blocked = phase2b_task(task_id, execution_id, agent_id, "blocked");
    UiTaskBoardProjection {
        source_agent_id: AgentId::new("cli-agent"),
        status_filter: None,
        agent_filter: None,
        include_terminal,
        tasks: vec![review.clone()],
        agents: vec![UiAgentSnapshotProjection {
            agent_id: AgentId::new(agent_id.to_owned()),
            status: "busy".to_owned(),
            current_task_id: Some(task_id.to_owned()),
            current_cwd: Some("/tmp/cli-session".to_owned()),
            running_tasks: 1,
            queued_tasks: 0,
            last_seen_at: 1,
        }],
        blocked: vec![blocked],
        review_ready: vec![review.clone()],
        stale: vec![review],
    }
}

fn phase2b_task(
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
    status: &str,
) -> UiTaskSnapshotProjection {
    UiTaskSnapshotProjection {
        task_id: task_id.to_owned(),
        status: status.to_owned(),
        title: format!("Master poll {task_id}"),
        goal: "phase2b master poll proof".to_owned(),
        priority: 96,
        target_cwd: Some("/tmp/cli-session".to_owned()),
        parent_session_id: None,
        assignee_agent_id: Some(AgentId::new(agent_id.to_owned())),
        active_execution_id: Some(execution_id.to_owned()),
        updated_at: 1,
        last_progress_at: Some(1),
        last_event_seq: 8,
    }
}

fn phase2b_event_inbox(
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
    cursor: &str,
    events: Vec<String>,
    source_cursor: Option<String>,
) -> UiTaskEventInboxProjection {
    UiTaskEventInboxProjection {
        source_agent_id: AgentId::new("cli-agent"),
        generated_at: 10,
        source_cursor,
        next_cursor: if events.is_empty() {
            None
        } else {
            Some(cursor.to_owned())
        },
        events: events
            .into_iter()
            .enumerate()
            .map(|(index, event_type)| {
                phase2b_event_entry(
                    task_id,
                    execution_id,
                    agent_id,
                    cursor,
                    index + 1,
                    &event_type,
                )
            })
            .collect(),
    }
}

fn phase2b_event_entry(
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
    cursor: &str,
    index: usize,
    event_type: &str,
) -> UiTaskEventInboxEntryProjection {
    let kind = phase2b_event_kind(event_type);
    UiTaskEventInboxEntryProjection {
        cursor: if index == 8 {
            cursor.to_owned()
        } else {
            format!("{cursor}-{index}")
        },
        event_id: format!("phase2b-event-{index}"),
        kind: kind.to_owned(),
        task_id: task_id.to_owned(),
        execution_id: Some(execution_id.to_owned()),
        agent_id: Some(AgentId::new(agent_id.to_owned())),
        created_at: u64::try_from(index).expect("event timestamp"),
        payload: if event_type == "TaskSchedulerTick" {
            serde_json::json!({
                "scheduler_fact": {
                    "fact": {
                        "kind": "stale",
                        "idle_seconds": 2
                    }
                },
                "execution_id": execution_id
            })
        } else {
            serde_json::json!({
                "execution_id": execution_id
            })
        },
    }
}

fn phase2b_event_kind(event_type: &str) -> &'static str {
    match event_type {
        "TaskCreated" => "task_created",
        "TaskAssigned" => "task_assigned",
        "TaskResumed" => "execution_started",
        "TaskExecutionRecorded" => "progress_reported",
        "TaskSchedulerTick" => "scheduler_tick",
        "TaskBlocked" => "execution_blocked",
        "TaskExecutionRecovering" => "execution_recovering",
        "TaskReviewSubmitted" => "review_ready",
        _ => "unknown",
    }
}

fn phase2b_master_poll(
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
    cursor: &str,
    events: Vec<String>,
    already_processed: bool,
) -> UiMasterPollProjection {
    UiMasterPollProjection {
        source_agent_id: AgentId::new("cli-agent"),
        generated_at: 11,
        source_cursor: if already_processed {
            Some(cursor.to_owned())
        } else {
            None
        },
        next_cursor: if events.is_empty() {
            None
        } else {
            Some(cursor.to_owned())
        },
        persisted_cursor: Some(cursor.to_owned()),
        event_inbox: phase2b_event_inbox(
            task_id,
            execution_id,
            agent_id,
            cursor,
            events,
            if already_processed {
                Some(cursor.to_owned())
            } else {
                None
            },
        ),
        task_board: phase2b_task_board(task_id, execution_id, agent_id, true),
        agent_board: UiAgentBoardProjection {
            source_agent_id: AgentId::new("cli-agent"),
            agents: vec![phase2a_agent_lifecycle(
                agent_id,
                execution_id,
                "waiting_review",
            )],
        },
        classifications: vec![
            UiMasterPollClassificationProjection {
                kind: "blocked".to_owned(),
                summary: format!("task {task_id} was blocked"),
                task_id: Some(task_id.to_owned()),
                execution_id: Some(execution_id.to_owned()),
                agent_id: Some(AgentId::new(agent_id.to_owned())),
                recommended_actions: vec!["inspect_blocker".to_owned()],
            },
            UiMasterPollClassificationProjection {
                kind: "review_ready".to_owned(),
                summary: format!("task {task_id} is ready for review"),
                task_id: Some(task_id.to_owned()),
                execution_id: Some(execution_id.to_owned()),
                agent_id: Some(AgentId::new(agent_id.to_owned())),
                recommended_actions: vec!["approve_submission".to_owned()],
            },
            UiMasterPollClassificationProjection {
                kind: "stale".to_owned(),
                summary: format!("task {task_id} has stale facts"),
                task_id: Some(task_id.to_owned()),
                execution_id: Some(execution_id.to_owned()),
                agent_id: Some(AgentId::new(agent_id.to_owned())),
                recommended_actions: vec!["query_agent_lifecycle".to_owned()],
            },
        ],
    }
}

fn phase2c_task_board(task_id: &str, execution_id: &str, agent_id: &str) -> UiTaskBoardProjection {
    UiTaskBoardProjection {
        source_agent_id: AgentId::new("cli-agent"),
        status_filter: None,
        agent_filter: None,
        include_terminal: true,
        tasks: vec![phase2c_task(task_id, execution_id, agent_id, "cancelled")],
        agents: vec![UiAgentSnapshotProjection {
            agent_id: AgentId::new(agent_id.to_owned()),
            status: "available".to_owned(),
            current_task_id: None,
            current_cwd: Some("/tmp/cli-session".to_owned()),
            running_tasks: 0,
            queued_tasks: 0,
            last_seen_at: 1,
        }],
        blocked: Vec::new(),
        review_ready: Vec::new(),
        stale: Vec::new(),
    }
}

fn phase2c_task(
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
    status: &str,
) -> UiTaskSnapshotProjection {
    UiTaskSnapshotProjection {
        task_id: task_id.to_owned(),
        status: status.to_owned(),
        title: format!("Worker control {task_id}"),
        goal: "phase2c worker control proof".to_owned(),
        priority: 98,
        target_cwd: Some("/tmp/cli-session".to_owned()),
        parent_session_id: None,
        assignee_agent_id: Some(AgentId::new(agent_id.to_owned())),
        active_execution_id: Some(execution_id.to_owned()),
        updated_at: 1,
        last_progress_at: Some(1),
        last_event_seq: 7,
    }
}

fn worker_control_verify_events(
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
) -> Vec<UiWorkerControlEventProjection> {
    [
        ("wctl-cli-worker-control-query-verify", "query_status"),
        ("wctl-cli-worker-control-ask-verify", "ask_at_safe_point"),
        (
            "wctl-cli-worker-control-constraint-verify",
            "add_constraint",
        ),
        (
            "wctl-cli-worker-control-checkpoint-verify",
            "request_checkpoint",
        ),
        (
            "wctl-cli-worker-control-submit-verify",
            "request_submission_now",
        ),
        ("wctl-cli-worker-control-pause-verify", "pause"),
        ("wctl-cli-worker-control-resume-verify", "resume"),
        ("wctl-cli-worker-control-cancel-verify", "cancel"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (control_id, op))| {
        worker_control_event_projection(
            control_id,
            op,
            worker_control_status_for_op(op),
            task_id,
            execution_id,
            agent_id,
            index + 1,
        )
    })
    .collect()
}

fn worker_control_event_projection(
    control_id: &str,
    op: &str,
    status: &str,
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
    seq: usize,
) -> UiWorkerControlEventProjection {
    UiWorkerControlEventProjection {
        control_id: control_id.to_owned(),
        op: op.to_owned(),
        status: status.to_owned(),
        task_id: task_id.to_owned(),
        execution_id: execution_id.to_owned(),
        agent_id: AgentId::new(agent_id.to_owned()),
        created_at: u64::try_from(seq).expect("worker control seq"),
        summary: format!("{op} {task_id}"),
        payload: serde_json::json!({
            "op": op,
            "task_status": if op == "cancel" { "cancelled" } else { "running" },
            "active_execution_id": execution_id,
            "agent_status": "busy"
        }),
    }
}

fn worker_control_status_for_op(op: &str) -> &'static str {
    match op {
        "query_status" => "observed",
        "ask_at_safe_point"
        | "add_constraint"
        | "request_checkpoint"
        | "request_submission_now" => "queued",
        "pause" | "resume" | "cancel" => "applied",
        _ => "unknown",
    }
}

#[derive(Clone)]
struct MockMasterWorkerAutonomyTruth {
    scenario: String,
    session_id: SessionId,
    prompt: String,
    task_id: String,
    execution_id: String,
    agent_id: String,
    final_status: String,
    lifecycle_state: String,
    events: Vec<String>,
    tool_executions: usize,
}

fn master_autonomy_truth_from_prompt(
    prompt: &str,
    session_id: &SessionId,
) -> MockMasterWorkerAutonomyTruth {
    let scenario = master_autonomy_prompt_value(prompt, "FHMA_SCENARIO")
        .unwrap_or_else(|| "reject-retry".to_owned());
    let token = master_autonomy_prompt_value(prompt, "FHMA_TOKEN")
        .unwrap_or_else(|| "FHAUTO-missing".to_owned());
    let task_id = master_autonomy_prompt_value(prompt, "FHMA_TASK_ID")
        .unwrap_or_else(|| format!("task-cli-master-autonomy-{scenario}-{token}"));
    let execution_id = master_autonomy_prompt_value(prompt, "FHMA_EXECUTION_ID")
        .unwrap_or_else(|| format!("exec-cli-master-autonomy-{scenario}-{token}"));
    let agent_id = master_autonomy_prompt_value(prompt, "FHMA_WORKER_ID")
        .unwrap_or_else(|| "worker".to_owned());
    let mut truth = master_autonomy_truth_for(
        &scenario,
        session_id.as_str(),
        &task_id,
        &execution_id,
        &agent_id,
        &token,
    );
    truth.prompt = prompt.to_owned();
    truth
}

fn master_autonomy_prompt_value(prompt: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    prompt
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix).map(str::to_owned))
}

fn master_autonomy_truth_for(
    scenario: &str,
    session_id: &str,
    task_id: &str,
    execution_id: &str,
    agent_id: &str,
    token: &str,
) -> MockMasterWorkerAutonomyTruth {
    let (final_status, lifecycle_state, events, tool_executions) = match scenario {
        "success" => (
            "closed",
            "closed",
            vec![
                "TaskCreated",
                "TaskAssigned",
                "TaskResumed",
                "TaskExecutionRecorded",
                "TaskReviewSubmitted",
                "TaskReviewApproved",
                "TaskClosed",
            ],
            7,
        ),
        "execution-error" => (
            "blocked",
            "blocked",
            vec![
                "TaskCreated",
                "TaskAssigned",
                "TaskResumed",
                "TaskExecutionRecorded",
                "TaskBlocked",
            ],
            5,
        ),
        "reject-retry" => (
            "closed",
            "closed",
            vec![
                "TaskCreated",
                "TaskAssigned",
                "TaskResumed",
                "TaskReviewSubmitted",
                "TaskReviewRejected",
                "TaskExecutionRecovering",
                "TaskReviewSubmitted",
                "TaskReviewApproved",
                "TaskClosed",
            ],
            9,
        ),
        other => panic!("unknown master autonomy scenario {other}"),
    };
    MockMasterWorkerAutonomyTruth {
        scenario: scenario.to_owned(),
        session_id: SessionId::new(session_id.to_owned()),
        prompt: format!("Master worker autonomy sample {token}"),
        task_id: task_id.to_owned(),
        execution_id: execution_id.to_owned(),
        agent_id: agent_id.to_owned(),
        final_status: final_status.to_owned(),
        lifecycle_state: lifecycle_state.to_owned(),
        events: events.into_iter().map(str::to_owned).collect(),
        tool_executions,
    }
}

fn master_autonomy_task_board(truth: &MockMasterWorkerAutonomyTruth) -> UiTaskBoardProjection {
    let task = phase2a_task(
        &truth.task_id,
        &truth.execution_id,
        &truth.agent_id,
        &truth.final_status,
    );
    let blocked = if truth.final_status == "blocked" {
        vec![task.clone()]
    } else {
        Vec::new()
    };
    UiTaskBoardProjection {
        source_agent_id: AgentId::new("cli-agent"),
        status_filter: None,
        agent_filter: None,
        include_terminal: true,
        tasks: vec![task],
        agents: vec![UiAgentSnapshotProjection {
            agent_id: AgentId::new(truth.agent_id.clone()),
            status: if truth.final_status == "blocked" {
                "blocked"
            } else {
                "available"
            }
            .to_owned(),
            current_task_id: if truth.final_status == "blocked" {
                Some(truth.task_id.clone())
            } else {
                None
            },
            current_cwd: Some("/tmp/cli-session".to_owned()),
            running_tasks: 0,
            queued_tasks: 0,
            last_seen_at: 1,
        }],
        blocked,
        review_ready: Vec::new(),
        stale: Vec::new(),
    }
}

fn master_autonomy_turn_projection(truth: &MockMasterWorkerAutonomyTruth) -> UiTurnProjection {
    UiTurnProjection {
        source: UiSource {
            source_agent_id: AgentId::new("cli-agent"),
            source_node_id: "cli-node".to_owned(),
            source_turn_id: Some(TurnId::new("cli-master-autonomy-turn")),
            stream_kind: UiStreamKind::Turn,
        },
        session_id: truth.session_id.clone(),
        turn_id: TurnId::new("cli-master-autonomy-turn"),
        cwd: Some("/tmp/cli-session".to_owned()),
        user_text: Some(truth.prompt.clone()),
        model_request: None,
        reasoning: Vec::new(),
        text: vec![format!("master autonomy {} complete", truth.scenario)],
        tool_calls: (0..truth.tool_executions)
            .map(|index| format!("task:{}", index + 1))
            .collect(),
        tool_activities: (0..truth.tool_executions)
            .map(|index| UiToolActivity {
                tool_call_id: format!("toolu_task_{}_{}", truth.scenario, index + 1),
                tool_name: "task".to_owned(),
                status: UiToolActivityStatus::Completed,
                detail: Some(format!("task op {}", index + 1)),
                display: None,
            })
            .collect(),
        usage: Vec::new(),
        terminal_status: Some(TerminalStatus::Success),
        terminal_text: Some(format!(
            "master autonomy {} terminal status {}",
            truth.scenario, truth.final_status
        )),
        errors: Vec::new(),
        slave_substream_card: false,
    }
}

fn phase1_task(task_id: &str, status: &str) -> UiTaskSnapshotProjection {
    UiTaskSnapshotProjection {
        task_id: task_id.to_owned(),
        status: status.to_owned(),
        title: format!("Phase1 {task_id}"),
        goal: "phase1 foundation proof".to_owned(),
        priority: 50,
        target_cwd: Some("/tmp/cli-session".to_owned()),
        parent_session_id: None,
        assignee_agent_id: Some(AgentId::new("cli-agent")),
        active_execution_id: None,
        updated_at: 1,
        last_progress_at: Some(1),
        last_event_seq: 4,
    }
}

fn phase1_agent_lifecycle(state: &str) -> UiAgentLifecycleProjection {
    UiAgentLifecycleProjection {
        agent_id: AgentId::new("cli-agent"),
        role: "worker".to_owned(),
        alive: true,
        state: state.to_owned(),
        current_task_id: None,
        current_execution_id: None,
        current_turn_id: None,
        current_activity: None,
        last_activity: None,
        model_request_count: 1,
        model_retry_count: 0,
        tool_call_count: 0,
        tool_failure_count: 1,
        schema_polish_count: 0,
        provider_error_count: 0,
        blocked_count: 1,
        current_model: None,
        last_seen_at: 1,
        elapsed_ms: 0,
    }
}

fn task_event(seq: u64, event_type: &str) -> UiTaskLedgerEventProjection {
    task_event_with_payload(seq, event_type, serde_json::json!({}))
}

fn task_event_with_payload(
    seq: u64,
    event_type: &str,
    payload: serde_json::Value,
) -> UiTaskLedgerEventProjection {
    UiTaskLedgerEventProjection {
        seq,
        event_id: format!("event-{seq}"),
        event_type: event_type.to_owned(),
        from_status: None,
        to_status: event_type.to_owned(),
        timestamp: seq,
        actor_agent_id: AgentId::new("cli-agent"),
        payload,
    }
}

async fn send_task_command_receipt(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    request_id: String,
    command_kind: &str,
    target_feature_id: &str,
    target_owner_module: &str,
    dispatch_status: &str,
) {
    send_adp_response(
        socket,
        UiAdpResponse::CommandReceipt {
            request_id,
            receipt: UiCommandDispatchReceipt {
                ingress: freehand_ui_protocol::UiCommandIngressAck {
                    command_kind: command_kind.to_owned(),
                    accepted: true,
                    status_text: "accepted".to_owned(),
                    mutation_authority: "runtime".to_owned(),
                },
                target_feature_id: target_feature_id.to_owned(),
                target_owner_module: target_owner_module.to_owned(),
                dispatch_status: dispatch_status.to_owned(),
            },
        },
    )
    .await;
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
    kind: MockAdpSampleKind,
) -> UiTurnProjection {
    let failed_tool = kind == MockAdpSampleKind::Failure;
    let terminal_status = if kind == MockAdpSampleKind::ProviderRetry {
        TerminalStatus::Failed
    } else {
        TerminalStatus::Success
    };
    UiTurnProjection {
        source: UiSource {
            source_agent_id: AgentId::new("cli-agent"),
            source_node_id: "cli-node".to_owned(),
            source_turn_id: Some(TurnId::new("cli-adp-sample-turn")),
            stream_kind: UiStreamKind::Turn,
        },
        session_id: session_id.clone(),
        turn_id: TurnId::new(
            if failed_tool || kind == MockAdpSampleKind::SchemaMismatch {
                "cli-adp-sample-turn-r2"
            } else {
                "cli-adp-sample-turn"
            },
        ),
        cwd: Some("/tmp/cli-session".to_owned()),
        user_text: Some(prompt.to_owned()),
        model_request: None,
        reasoning: Vec::new(),
        text: if failed_tool {
            Vec::new()
        } else {
            vec!["sample success answer".to_owned()]
        },
        tool_calls: if failed_tool {
            vec!["ls".to_owned()]
        } else {
            Vec::new()
        },
        tool_activities: if failed_tool {
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
        terminal_text: Some(match kind {
            MockAdpSampleKind::Failure => "sample recovered after tool failure".to_owned(),
            MockAdpSampleKind::ProviderRetry => {
                "provider retry exhausted anthropic_http_status_500".to_owned()
            }
            _ => "sample success terminal".to_owned(),
        }),
        errors: if kind == MockAdpSampleKind::ProviderRetry {
            vec!["provider retry exhausted anthropic_http_status_500".to_owned()]
        } else {
            Vec::new()
        },
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
paired_agents = ["worker"]
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
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
    assert!(stdout.contains("provider_auth_source=inline"));
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
    let (url, handle) = spawn_adp_sample_mock_server(MockAdpSampleKind::Success);

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
    let (url, handle) = spawn_adp_sample_mock_server(MockAdpSampleKind::Failure);

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
fn cli_runs_adp_schema_mismatch_turn_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_sample_mock_server(MockAdpSampleKind::SchemaMismatch);

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("adp-turn-sample")
        .arg("--url")
        .arg(&url)
        .arg("--sample")
        .arg("schema-mismatch")
        .output()
        .expect("run adp schema mismatch sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("adp_turn_sample_ok"));
    assert!(stdout.contains("sample=schema-mismatch"));
    assert!(stdout.contains("schema_retries=1"));
    assert!(stdout.contains("tool_executions=0"));
    assert!(stdout.contains("rounds=2"));

    handle.join().expect("adp schema mismatch sample mock join");
}

#[test]
fn cli_runs_adp_provider_retry_turn_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_sample_mock_server(MockAdpSampleKind::ProviderRetry);

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("adp-turn-sample")
        .arg("--url")
        .arg(&url)
        .arg("--sample")
        .arg("provider-retry")
        .output()
        .expect("run adp provider retry sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("adp_turn_sample_ok"));
    assert!(stdout.contains("sample=provider-retry"));
    assert!(stdout.contains("provider_retries=1"));

    handle.join().expect("adp provider retry sample mock join");
}

#[test]
fn cli_runs_session_continue_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_session_continue_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("session-continue-sample")
        .arg("--url")
        .arg(&url)
        .output()
        .expect("run session continuation sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("session_continue_sample_ok"));
    assert!(stdout.contains("turns=2"));
    assert!(stdout.contains("first_turn=cli-session-continue-turn-1"));
    assert!(stdout.contains("second_turn=cli-session-continue-turn-2"));

    handle.join().expect("adp session continuation mock join");
}

#[test]
fn cli_runs_task_lifecycle_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_task_lifecycle_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("task-lifecycle-sample")
        .arg("--url")
        .arg(&url)
        .output()
        .expect("run task lifecycle sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("task_lifecycle_sample_ok"));
    assert!(stdout.contains("task=task-cli-FHTASK"));
    assert!(stdout.contains("status=Closed"));
    assert!(stdout.contains("TaskCreated,TaskReviewSubmitted,TaskReviewApproved,TaskClosed"));

    handle.join().expect("adp task lifecycle mock join");
}

#[test]
fn cli_runs_phase1_foundation_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_phase1_foundation_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("phase1-foundation-sample")
        .arg("--url")
        .arg(&url)
        .output()
        .expect("run phase1 foundation sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("phase1_foundation_sample_ok"));
    assert!(stdout.contains("blocked_task=task-cli-phase1-blocked-FHPHASE1"));
    assert!(stdout.contains("review_task=task-cli-phase1-review-FHPHASE1"));
    assert!(stdout.contains("execution=exec-cli-phase1-FHPHASE1"));
    assert!(stdout.contains("agent=cli-agent"));
    assert!(stdout.contains("blocked=1"));
    assert!(stdout.contains("review_ready=1"));
    assert!(stdout.contains("stale=1"));
    assert!(stdout.contains("recovering_event=true"));

    handle.join().expect("adp phase1 foundation mock join");
}

#[test]
fn cli_runs_master_worker_foundation_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_master_worker_foundation_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("master-worker-foundation-sample")
        .arg("--url")
        .arg(&url)
        .output()
        .expect("run master worker foundation sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("master_worker_foundation_sample_ok"));
    assert!(stdout.contains("task=task-cli-master-worker-FHPHASE2A"));
    assert!(stdout.contains("execution=exec-cli-master-worker-FHPHASE2A"));
    assert!(stdout.contains("agent=worker-cli-master-worker-FHPHASE2A"));
    assert!(stdout.contains("status=closed"));
    assert!(stdout.contains("blocked_seen=true"));
    assert!(stdout.contains("review_ready_seen=true"));
    assert!(stdout.contains("TaskReviewRejected"));
    assert!(stdout.contains("TaskReviewApproved"));
    assert!(stdout.contains("TaskClosed"));

    handle.join().expect("adp master worker mock join");
}

#[test]
fn cli_runs_master_worker_foundation_verify_against_mock_websocket() {
    let (url, handle) = spawn_adp_master_worker_foundation_verify_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("master-worker-foundation-sample")
        .arg("--url")
        .arg(&url)
        .arg("--verify-task")
        .arg("task-cli-master-worker-verify")
        .arg("--execution")
        .arg("exec-cli-master-worker-verify")
        .arg("--agent")
        .arg("worker-cli-master-worker-verify")
        .output()
        .expect("run master worker foundation verify");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("master_worker_foundation_verify_ok"));
    assert!(stdout.contains("task=task-cli-master-worker-verify"));
    assert!(stdout.contains("execution=exec-cli-master-worker-verify"));
    assert!(stdout.contains("agent=worker-cli-master-worker-verify"));
    assert!(stdout.contains("status=closed"));
    assert!(stdout.contains("blocked_seen=true"));
    assert!(stdout.contains("review_ready_seen=true"));

    handle.join().expect("adp master worker verify mock join");
}

#[test]
fn cli_runs_master_worker_autonomy_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_master_worker_autonomy_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("master-worker-autonomy-sample")
        .arg("--url")
        .arg(&url)
        .arg("--scenario")
        .arg("all")
        .output()
        .expect("run master worker autonomy sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("master_worker_autonomy_sample_ok"));
    assert!(stdout.contains("scenario=success"));
    assert!(stdout.contains("scenario=execution-error"));
    assert!(stdout.contains("scenario=reject-retry"));
    assert!(stdout.contains("task=task-cli-master-autonomy-success-FHAUTO"));
    assert!(stdout.contains("task=task-cli-master-autonomy-execution-error-FHAUTO"));
    assert!(stdout.contains("task=task-cli-master-autonomy-reject-retry-FHAUTO"));
    assert!(stdout.contains("status=closed"));
    assert!(stdout.contains("status=blocked"));
    assert!(stdout.contains("tool_executions=7"));
    assert!(stdout.contains("tool_executions=5"));
    assert!(stdout.contains("tool_executions=9"));
    assert!(stdout.contains("TaskReviewRejected"));
    assert!(stdout.contains("TaskExecutionRecovering"));
    assert!(stdout.contains("master_autonomy_model_turn_complete:reject-retry"));

    handle.join().expect("adp master autonomy mock join");
}

#[test]
fn cli_runs_master_worker_autonomy_verify_against_mock_websocket() {
    let (url, handle) = spawn_adp_master_worker_autonomy_verify_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("master-worker-autonomy-sample")
        .arg("--url")
        .arg(&url)
        .arg("--scenario")
        .arg("reject-retry")
        .arg("--verify-task")
        .arg("task-cli-master-autonomy-reject-retry-verify")
        .arg("--execution")
        .arg("exec-cli-master-autonomy-reject-retry-verify")
        .arg("--agent")
        .arg("worker")
        .output()
        .expect("run master worker autonomy verify");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("master_worker_autonomy_verify_ok"));
    assert!(stdout.contains("scenario=reject-retry"));
    assert!(stdout.contains("task=task-cli-master-autonomy-reject-retry-verify"));
    assert!(stdout.contains("execution=exec-cli-master-autonomy-reject-retry-verify"));
    assert!(stdout.contains("agent=worker"));
    assert!(stdout.contains("status=closed"));
    assert!(stdout.contains("lifecycle_state=closed"));
    assert!(stdout.contains("review_submissions=2"));
    assert!(stdout.contains("TaskReviewRejected"));
    assert!(stdout.contains("TaskClosed"));

    handle.join().expect("adp master autonomy verify mock join");
}

#[test]
fn cli_runs_master_poll_foundation_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_master_poll_foundation_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("master-poll-foundation-sample")
        .arg("--url")
        .arg(&url)
        .output()
        .expect("run master poll foundation sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("master_poll_foundation_sample_ok"));
    assert!(stdout.contains("task=task-cli-master-poll-FHPHASE2B"));
    assert!(stdout.contains("execution=exec-cli-master-poll-FHPHASE2B"));
    assert!(stdout.contains("agent=worker-cli-master-poll-FHPHASE2B"));
    assert!(stdout.contains("status=review_submitted"));
    assert!(stdout.contains("inbox_events=8"));
    assert!(stdout.contains("poll_events=0"));
    assert!(stdout.contains("classifications=blocked,review_ready,stale"));
    assert!(stdout.contains("master_poll_recorded:events=0"));

    handle.join().expect("adp master poll mock join");
}

#[test]
fn cli_runs_master_poll_foundation_verify_against_mock_websocket() {
    let (url, handle) = spawn_adp_master_poll_foundation_verify_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("master-poll-foundation-sample")
        .arg("--url")
        .arg(&url)
        .arg("--verify-task")
        .arg("task-cli-master-poll-verify")
        .arg("--execution")
        .arg("exec-cli-master-poll-verify")
        .arg("--agent")
        .arg("worker-cli-master-poll-verify")
        .arg("--cursor")
        .arg("00000000000000000008:task-cli-master-poll-verify:00000000000000000008:task-cli-master-poll-verify:8")
        .output()
        .expect("run master poll foundation verify");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("master_poll_foundation_verify_ok"));
    assert!(stdout.contains("task=task-cli-master-poll-verify"));
    assert!(stdout.contains("execution=exec-cli-master-poll-verify"));
    assert!(stdout.contains("agent=worker-cli-master-poll-verify"));
    assert!(stdout.contains("status=review_submitted"));
    assert!(stdout.contains("inbox_after_cursor_events=0"));
    assert!(stdout.contains("poll_events=0"));
    assert!(stdout.contains("classifications=blocked,review_ready,stale"));

    handle.join().expect("adp master poll verify mock join");
}

#[test]
fn cli_runs_worker_control_foundation_sample_against_mock_websocket() {
    let (url, handle) = spawn_adp_worker_control_foundation_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("worker-control-foundation-sample")
        .arg("--url")
        .arg(&url)
        .output()
        .expect("run worker control foundation sample");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("worker_control_foundation_sample_ok"));
    assert!(stdout.contains("task=task-cli-worker-control-FHPHASE2C"));
    assert!(stdout.contains("execution=exec-cli-worker-control-FHPHASE2C"));
    assert!(stdout.contains("agent=worker-cli-worker-control-FHPHASE2C"));
    assert!(stdout.contains("status=cancelled"));
    assert!(stdout.contains("control_events=8"));
    assert!(stdout.contains("query_status:observed"));
    assert!(stdout.contains("ask_at_safe_point:queued"));
    assert!(stdout.contains("add_constraint:queued"));
    assert!(stdout.contains("request_checkpoint:queued"));
    assert!(stdout.contains("request_submission_now:queued"));
    assert!(stdout.contains("pause:applied"));
    assert!(stdout.contains("resume:applied"));
    assert!(stdout.contains("cancel:applied"));
    assert!(stdout.contains("TaskPaused"));
    assert!(stdout.contains("TaskResumed"));
    assert!(stdout.contains("TaskCancelled"));

    handle.join().expect("adp worker control mock join");
}

#[test]
fn cli_runs_worker_control_foundation_verify_against_mock_websocket() {
    let (url, handle) = spawn_adp_worker_control_foundation_verify_mock_server();

    let output = Command::new(env!("CARGO_BIN_EXE_freehand-cli"))
        .arg("worker-control-foundation-sample")
        .arg("--url")
        .arg(&url)
        .arg("--verify-task")
        .arg("task-cli-worker-control-verify")
        .arg("--execution")
        .arg("exec-cli-worker-control-verify")
        .arg("--agent")
        .arg("worker-cli-worker-control-verify")
        .arg("--control")
        .arg("wctl-cli-worker-control-cancel-verify")
        .output()
        .expect("run worker control foundation verify");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("worker_control_foundation_verify_ok"));
    assert!(stdout.contains("task=task-cli-worker-control-verify"));
    assert!(stdout.contains("execution=exec-cli-worker-control-verify"));
    assert!(stdout.contains("agent=worker-cli-worker-control-verify"));
    assert!(stdout.contains("control=wctl-cli-worker-control-cancel-verify"));
    assert!(stdout.contains("status=cancelled"));
    assert!(stdout.contains("control_events=8"));
    assert!(stdout.contains("cancel:applied"));
    assert!(stdout.contains("TaskPaused"));
    assert!(stdout.contains("TaskResumed"));
    assert!(stdout.contains("TaskCancelled"));

    handle.join().expect("adp worker control verify mock join");
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
paired_agents = ["worker"]
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
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
paired_agents = ["worker"]
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
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
paired_agents = ["worker"]
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
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
paired_agents = ["worker"]
pair_token = "FREEHAND_CLI_TOKEN"
provider = "minimonth"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
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
paired_agents = ["worker"]
pair_token = "FREEHAND_CLI_TOKEN"
provider = "minimonth"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
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
paired_agents = ["worker"]
pair_token = "FREEHAND_CLI_TOKEN"
provider = "minimonth"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
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
paired_agents = ["worker"]
pair_token = "FREEHAND_CLI_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
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
