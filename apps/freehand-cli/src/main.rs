use freehand_blocks::strip_completion_submission_block;
use freehand_config::{AgentMode, default_config_path, load_default_config};
use freehand_contracts::{SemanticEventKind, SessionId, TerminalStatus, TraceId, TurnId};
use freehand_runtime::{LiveReasonRestoreStatus, LiveReasonTurnRequest, run_live_reason_turn};
use freehand_testkit::{
    ReasonRuntimeSmokeScenario, run_reason_persistence_smoke, run_reason_runtime_smoke,
};
use freehand_ui_protocol::{UiAdpRequest, UiAdpResponse, UiClientKind, UiCommand};
use futures_util::{SinkExt, StreamExt};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};

fn main() {
    match run() {
        Ok(output) => println!("{output}"),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    let Some(flag) = args.next() else {
        return Err("usage: freehand-cli --agent <name>".to_owned());
    };
    if flag == "reason-e2e" {
        return run_reason_e2e_smoke(args.collect());
    }
    if flag == "reason-persist-smoke" {
        return run_reason_persist_smoke(args.collect());
    }
    if flag == "reason-live" {
        return run_reason_live(args.collect());
    }
    if flag == "adp-smoke" {
        return run_adp_smoke(args.collect());
    }
    if flag == "adp-turn-sample" {
        return run_adp_turn_sample(args.collect());
    }
    if flag == "adp-session-query" {
        return run_adp_session_query(args.collect());
    }
    if flag == "adp-session-manage" {
        return run_adp_session_manage(args.collect());
    }
    if flag == "adp-task-query" {
        return run_adp_task_query(args.collect());
    }
    if flag == "adp-task-subscribe" {
        return run_adp_task_subscribe(args.collect());
    }
    if flag == "adp-error-query" {
        return run_adp_error_query(args.collect());
    }
    if flag != "--agent" {
        return Err(
            "usage: freehand-cli --agent <name>\n   or: freehand-cli reason-e2e --agent <name> --scenario <usage-compaction|recovery-block>\n   or: freehand-cli reason-persist-smoke --agent <name>\n   or: freehand-cli reason-live --agent <name> --prompt <text> [--stream]\n   or: freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp\n   or: freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample <success|failure>\n   or: freehand-cli adp-session-query --url ws://127.0.0.1:4041/adp [--session <id>]\n   or: freehand-cli adp-session-manage --url ws://127.0.0.1:4041/adp --action <create|rename|archive|restore|delete> --session <id> [--title <title>] [--cwd <path>]\n   or: freehand-cli adp-task-query --url ws://127.0.0.1:4041/adp [--status <status>] [--agent <id>] [--history <task_id>]\n   or: freehand-cli adp-task-subscribe --url ws://127.0.0.1:4041/adp [--status <status>] [--agent <id>]\n   or: freehand-cli adp-error-query --url ws://127.0.0.1:4041/adp --session <id> [--trace <id>] [--turn <id>] [--domain <domain>]"
                .to_owned(),
        );
    }
    let Some(agent_name) = args.next() else {
        return Err("usage: freehand-cli --agent <name>".to_owned());
    };
    if args.next().is_some() {
        return Err("usage: freehand-cli --agent <name>".to_owned());
    }

    let config = load_default_config().map_err(|err| err.to_string())?;
    let selected = config
        .select_agent(&agent_name)
        .map_err(|err| err.to_string())?;

    Ok(format!(
        "agent={} mode={} allowed_pair_ip={} pair_token_env={} provider={} provider_type={} provider_protocol={} default_model={} base_url={} provider_auth={} restart_required_on_change={}",
        selected.name,
        mode_label(selected.mode),
        selected
            .allowed_pair_ip
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        selected.pair_token_env,
        selected.provider.id,
        provider_type_label(selected.provider.provider_type),
        provider_protocol_label(selected.provider.protocol),
        selected.provider.default_model,
        selected.provider.base_url,
        provider_auth_label(selected.provider.auth_type),
        selected.restart_required_on_change
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdpTurnSample {
    Success,
    Failure,
}

impl AdpTurnSample {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "success" => Ok(Self::Success),
            "failure" => Ok(Self::Failure),
            _ => Err("sample must be one of: success, failure".to_owned()),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }

    fn expected_status(self) -> TerminalStatus {
        TerminalStatus::Success
    }

    fn prompt(self) -> &'static str {
        match self {
            Self::Success => {
                "ADP success sample: answer with one short sentence and a valid Freehand completion schema. Do not call tools."
            }
            Self::Failure => {
                "ADP failure sample: call the read_file tool exactly once with path definitely-missing-freehand-file.txt, then use the failed tool result to continue and report success through the required Freehand completion schema."
            }
        }
    }
}

fn run_adp_turn_sample(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample <success|failure>"
            .to_owned();
    if args.len() != 4 || args[0] != "--url" || args[2] != "--sample" {
        return Err(usage);
    }
    let url = args[1].clone();
    let sample = AdpTurnSample::parse(&args[3])?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_adp_turn_sample_async(url, sample))
}

fn run_adp_session_query(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli adp-session-query --url ws://127.0.0.1:4041/adp [--session <id>]"
            .to_owned();
    if args.len() != 2 && args.len() != 4 {
        return Err(usage);
    }
    if args[0] != "--url" {
        return Err(usage);
    }
    let session_id = if args.len() == 4 {
        if args[2] != "--session" {
            return Err(usage);
        }
        Some(SessionId::new(args[3].clone()))
    } else {
        None
    };
    let url = args[1].clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_adp_session_query_async(url, session_id))
}

fn run_adp_session_manage(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli adp-session-manage --url ws://127.0.0.1:4041/adp --action <create|rename|archive|restore|delete> --session <id> [--title <title>] [--cwd <path>]"
            .to_owned();
    let mut url = None::<String>;
    let mut action = None::<String>;
    let mut session_id = None::<String>;
    let mut title = None::<String>;
    let mut cwd = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--action" if index + 1 < args.len() => {
                action = Some(args[index + 1].clone());
                index += 2;
            }
            "--session" if index + 1 < args.len() => {
                session_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--title" if index + 1 < args.len() => {
                title = Some(args[index + 1].clone());
                index += 2;
            }
            "--cwd" if index + 1 < args.len() => {
                cwd = Some(args[index + 1].clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let url = url.ok_or_else(|| usage.clone())?;
    let action = action.ok_or_else(|| usage.clone())?;
    let session_id = SessionId::new(session_id.ok_or_else(|| usage.clone())?);
    let command = match action.as_str() {
        "create" => UiCommand::CreateSession {
            session_id,
            title,
            cwd,
        },
        "rename" => UiCommand::RenameSession {
            session_id,
            title: title.ok_or_else(|| "--title is required for rename".to_owned())?,
        },
        "archive" => UiCommand::ArchiveSession { session_id },
        "restore" => UiCommand::RestoreSession { session_id },
        "delete" => UiCommand::DeleteSession { session_id },
        _ => {
            return Err(
                "action must be one of: create, rename, archive, restore, delete".to_owned(),
            );
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_adp_session_manage_async(url, command))
}

fn run_adp_task_query(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli adp-task-query --url ws://127.0.0.1:4041/adp [--status <status>] [--agent <id>] [--history <task_id>]"
            .to_owned();
    let mut url = None::<String>;
    let mut status = None::<String>;
    let mut agent_id = None::<String>;
    let mut history = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--status" if index + 1 < args.len() => {
                status = Some(args[index + 1].clone());
                index += 2;
            }
            "--agent" if index + 1 < args.len() => {
                agent_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--history" if index + 1 < args.len() => {
                history = Some(args[index + 1].clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let url = url.ok_or_else(|| usage.clone())?;
    let query = if let Some(task_id) = history {
        UiCommand::QueryTaskHistory { task_id }
    } else {
        UiCommand::QueryTaskList {
            status,
            agent_id: agent_id.map(freehand_contracts::AgentId::new),
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_adp_task_query_async(url, query))
}

fn run_adp_error_query(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli adp-error-query --url ws://127.0.0.1:4041/adp --session <id> [--trace <id>] [--turn <id>] [--domain <domain>]"
            .to_owned();
    let mut url = None::<String>;
    let mut session_id = None::<String>;
    let mut trace_id = None::<String>;
    let mut turn_id = None::<String>;
    let mut domain = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--session" if index + 1 < args.len() => {
                session_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--trace" if index + 1 < args.len() => {
                trace_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--turn" if index + 1 < args.len() => {
                turn_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--domain" if index + 1 < args.len() => {
                domain = Some(args[index + 1].clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let url = url.ok_or_else(|| usage.clone())?;
    let session_id = session_id.ok_or_else(|| usage.clone())?;
    let query = UiCommand::QueryErrorCenterEvents {
        session_id: SessionId::new(session_id),
        trace_id,
        turn_id: turn_id.map(TurnId::new),
        domain,
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_adp_error_query_async(url, query))
}

fn run_adp_task_subscribe(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli adp-task-subscribe --url ws://127.0.0.1:4041/adp [--status <status>] [--agent <id>]"
            .to_owned();
    let mut url = None::<String>;
    let mut status = None::<String>;
    let mut agent_id = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--status" if index + 1 < args.len() => {
                status = Some(args[index + 1].clone());
                index += 2;
            }
            "--agent" if index + 1 < args.len() => {
                agent_id = Some(args[index + 1].clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let url = url.ok_or_else(|| usage.clone())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_adp_task_subscribe_async(url, status, agent_id))
}

async fn run_adp_session_query_async(
    url: String,
    session_id: Option<SessionId>,
) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(&url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;

    let list_request_id = "cli-session-list-1".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: list_request_id.clone(),
            query: UiCommand::QuerySessionList,
        },
    )
    .await?;

    let mut list_summary = None::<String>;
    let mut selected_session = session_id;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while list_summary.is_none() {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err("ADP session list timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP session list timeout".to_owned())??;
        match response {
            UiAdpResponse::QueryResult { request_id, result } if request_id == list_request_id => {
                let freehand_ui_protocol::UiQueryResult::SessionList(list) = result else {
                    return Err("ADP session list returned non-session result".to_owned());
                };
                if selected_session.is_none() {
                    selected_session = list
                        .sessions
                        .last()
                        .map(|session| session.session_id.clone());
                }
                list_summary = Some(format!(
                    "sessions={} ids={}",
                    list.sessions.len(),
                    list.sessions
                        .iter()
                        .map(|session| format!(
                            "{}:{}:{}",
                            session.session_id.as_str(),
                            session.turn_count,
                            session.latest_status
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
            UiAdpResponse::Failure {
                request_id,
                failure,
            } if request_id == list_request_id => {
                return Err(format!(
                    "ADP session list failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }

    let Some(session_id) = selected_session else {
        let _ = socket.close(None).await;
        return Ok(format!(
            "adp_session_query_ok url={} {} selected_session=none turns=0",
            url,
            list_summary.expect("list summary")
        ));
    };

    let turns_request_id = "cli-session-turns-1".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: turns_request_id.clone(),
            query: UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            },
        },
    )
    .await?;

    let mut transcript_summary = None::<String>;
    while transcript_summary.is_none() {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err("ADP session turns timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP session turns timeout".to_owned())??;
        match response {
            UiAdpResponse::QueryResult { request_id, result } if request_id == turns_request_id => {
                let freehand_ui_protocol::UiQueryResult::SessionTurns(transcript) = result else {
                    return Err("ADP session turns returned non-transcript result".to_owned());
                };
                transcript_summary = Some(format!(
                    "selected_session={} turns={} turn_ids={}",
                    transcript.session_id.as_str(),
                    transcript.turns.len(),
                    transcript
                        .turns
                        .iter()
                        .map(|turn| turn.turn_id.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
            UiAdpResponse::Failure {
                request_id,
                failure,
            } if request_id == turns_request_id => {
                return Err(format!(
                    "ADP session turns failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }

    let _ = socket.close(None).await;
    Ok(format!(
        "adp_session_query_ok url={} {} {}",
        url,
        list_summary.expect("list summary"),
        transcript_summary.expect("transcript summary")
    ))
}

async fn run_adp_session_manage_async(url: String, command: UiCommand) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(&url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;
    let request_id = "cli-session-manage-1".to_owned();
    let command_kind = match &command {
        UiCommand::CreateSession { .. } => "create",
        UiCommand::RenameSession { .. } => "rename",
        UiCommand::ArchiveSession { .. } => "archive",
        UiCommand::RestoreSession { .. } => "restore",
        UiCommand::DeleteSession { .. } => "delete",
        _ => "unknown",
    };
    send_adp(
        &mut socket,
        UiAdpRequest::Command {
            request_id: request_id.clone(),
            command,
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err("ADP session manage timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP session manage timeout".to_owned())??;
        match response {
            UiAdpResponse::CommandReceipt {
                request_id: response_id,
                receipt,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Ok(format!(
                    "adp_session_manage_ok url={} action={} target={} status={}",
                    url, command_kind, receipt.target_feature_id, receipt.dispatch_status
                ));
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP session manage failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }
}

async fn run_adp_task_query_async(url: String, query: UiCommand) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(&url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;
    let request_id = "cli-task-query-1".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: request_id.clone(),
            query,
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("ADP task query timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP task query timeout".to_owned())??;
        match response {
            UiAdpResponse::QueryResult {
                request_id: response_id,
                result,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return summarize_task_query_result(&url, &result);
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP task query failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }
}

async fn run_adp_error_query_async(url: String, query: UiCommand) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(&url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;
    let request_id = "cli-error-query-1".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: request_id.clone(),
            query,
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("ADP error query timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP error query timeout".to_owned())??;
        match response {
            UiAdpResponse::QueryResult {
                request_id: response_id,
                result,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return summarize_error_query_result(&url, &result);
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP error query failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }
}

async fn run_adp_task_subscribe_async(
    url: String,
    status: Option<String>,
    agent_id: Option<String>,
) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(&url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;
    let request_id = "cli-task-subscribe-1".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Subscribe {
            request_id: request_id.clone(),
            subscription: UiCommand::SubscribeTaskList {
                status,
                agent_id: agent_id.map(freehand_contracts::AgentId::new),
            },
        },
    )
    .await?;
    let mut accepted = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("ADP task subscribe timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP task subscribe timeout".to_owned())??;
        match response {
            UiAdpResponse::SubscriptionAccepted {
                request_id: response_id,
                ..
            } if response_id == request_id => {
                accepted = true;
            }
            UiAdpResponse::SubscriptionEvent {
                request_id: response_id,
                event,
            } if response_id == request_id => {
                let freehand_ui_protocol::UiProjection::TaskList(list) = event.projection else {
                    continue;
                };
                let _ = socket.close(None).await;
                return Ok(format!(
                    "adp_task_subscribe_ok url={} accepted={} source_agent={} count={} tasks={}",
                    url,
                    accepted,
                    list.source_agent_id.as_str(),
                    list.tasks.len(),
                    list.tasks
                        .iter()
                        .map(|task| format!("{}:{}:{}", task.task_id, task.status, task.priority))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP task subscribe failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }
}

fn summarize_task_query_result(
    url: &str,
    result: &freehand_ui_protocol::UiQueryResult,
) -> Result<String, String> {
    match result {
        freehand_ui_protocol::UiQueryResult::TaskList(list) => Ok(format!(
            "adp_task_query_ok url={} kind=list source_agent={} count={} tasks={}",
            url,
            list.source_agent_id.as_str(),
            list.tasks.len(),
            list.tasks
                .iter()
                .map(|task| format!("{}:{}:{}", task.task_id, task.status, task.priority))
                .collect::<Vec<_>>()
                .join(",")
        )),
        freehand_ui_protocol::UiQueryResult::TaskHistory(history) => Ok(format!(
            "adp_task_query_ok url={} kind=history source_agent={} task_id={} events={} event_types={}",
            url,
            history.source_agent_id.as_str(),
            history.task_id,
            history.events.len(),
            history
                .events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>()
                .join(",")
        )),
        _ => Err("ADP task query returned non-task result".to_owned()),
    }
}

fn summarize_error_query_result(
    url: &str,
    result: &freehand_ui_protocol::UiQueryResult,
) -> Result<String, String> {
    match result {
        freehand_ui_protocol::UiQueryResult::ErrorCenterEvents(list) => Ok(format!(
            "adp_error_query_ok url={} source_agent={} session={} count={} events={}",
            url,
            list.source_agent_id.as_str(),
            list.session_id.as_str(),
            list.events.len(),
            list.events
                .iter()
                .map(|event| format!(
                    "{}:{}:{}:{}:{}",
                    event.domain, event.class, event.recovery_action, event.code, event.raw_hash
                ))
                .collect::<Vec<_>>()
                .join(",")
        )),
        _ => Err("ADP error query returned non-error-center result".to_owned()),
    }
}

async fn run_adp_turn_sample_async(url: String, sample: AdpTurnSample) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(&url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;
    let sub_id = format!("cli-sample-{}-sub", sample.label());
    let cmd_id = format!("cli-sample-{}-cmd", sample.label());
    let query_id = format!("cli-sample-{}-query", sample.label());
    let transcript_query_id = format!("cli-sample-{}-transcript", sample.label());
    let session_id = SessionId::new(format!(
        "cli-adp-sample-{}-{}",
        sample.label(),
        live_id_stamp()?
    ));

    send_adp(
        &mut socket,
        UiAdpRequest::Subscribe {
            request_id: sub_id.clone(),
            subscription: UiCommand::SubscribeLatestActiveTurn {
                client: UiClientKind::Cli,
            },
        },
    )
    .await?;
    send_adp(
        &mut socket,
        UiAdpRequest::Command {
            request_id: cmd_id.clone(),
            command: UiCommand::SubmitUserInput {
                text: sample.prompt().to_owned(),
                session_id: Some(session_id.clone()),
                cwd: None,
            },
        },
    )
    .await?;

    let mut accepted = false;
    let mut command_observed = false;
    let mut query_sent = false;
    let mut seen = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);

    while !command_observed {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(format!(
                "ADP {} sample timeout seen={}",
                sample.label(),
                seen.join(",")
            ));
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| {
                format!(
                    "ADP {} sample timeout seen={}",
                    sample.label(),
                    seen.join(",")
                )
            })??;
        match response {
            UiAdpResponse::SubscriptionAccepted { request_id, .. } => {
                seen.push(format!("subscription_accepted:{request_id}"));
                if request_id == sub_id {
                    accepted = true;
                }
            }
            UiAdpResponse::CommandReceipt {
                request_id,
                receipt,
            } => {
                seen.push(format!(
                    "command_receipt:{request_id}:{}",
                    receipt.dispatch_status
                ));
                if request_id == cmd_id {
                    command_observed = true;
                }
                if !query_sent {
                    send_adp(
                        &mut socket,
                        UiAdpRequest::Query {
                            request_id: query_id.clone(),
                            query: UiCommand::QueryLatestActiveTurn,
                        },
                    )
                    .await?;
                    query_sent = true;
                }
            }
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                seen.push(format!("failure:{request_id}:{}", failure.code));
                if request_id == cmd_id {
                    command_observed = true;
                }
                if !query_sent {
                    send_adp(
                        &mut socket,
                        UiAdpRequest::Query {
                            request_id: query_id.clone(),
                            query: UiCommand::QueryLatestActiveTurn,
                        },
                    )
                    .await?;
                    query_sent = true;
                }
            }
            UiAdpResponse::SubscriptionEvent { request_id, event } => {
                seen.push(format!("subscription_event:{request_id}"));
                if let Some(reason) =
                    sample_terminal_failure_reason(sample, &session_id, &event.projection)
                {
                    return Err(format!(
                        "ADP {} sample terminal failure: {reason} seen={}",
                        sample.label(),
                        seen.join(",")
                    ));
                }
            }
            UiAdpResponse::QueryResult { request_id, result } => {
                seen.push(format!("query_result:{request_id}"));
                if let Some(reason) =
                    sample_query_terminal_failure_reason(sample, &session_id, &result)
                {
                    return Err(format!(
                        "ADP {} sample terminal failure: {reason} seen={}",
                        sample.label(),
                        seen.join(",")
                    ));
                }
            }
        }
    }

    if !accepted {
        return Err(format!(
            "ADP {} sample missed subscription ack",
            sample.label()
        ));
    }
    if !command_observed {
        return Err(format!(
            "ADP {} sample missed command outcome",
            sample.label()
        ));
    }
    let _ = socket.close(None).await;
    let evidence =
        query_adp_sample_transcript_evidence(&url, sample, &session_id, &transcript_query_id)
            .await?;
    if !sample_evidence_complete(sample, Some(&evidence)) {
        return Err(format!(
            "ADP {} sample transcript incomplete rounds={} tool_executions={} failed_tools={} seen={}",
            sample.label(),
            evidence.rounds,
            evidence.tool_executions,
            evidence.failed_tools,
            seen.join(",")
        ));
    }
    Ok(format!(
        "adp_turn_sample_ok sample={} url={} session={} turn={} rounds={} tool_executions={} failed_tools={} seen={}",
        sample.label(),
        url,
        session_id.as_str(),
        evidence.terminal_turn_id,
        evidence.rounds,
        evidence.tool_executions,
        evidence.failed_tools,
        seen.join(",")
    ))
}

async fn query_adp_sample_transcript_evidence(
    url: &str,
    sample: AdpTurnSample,
    session_id: &SessionId,
    request_id: &str,
) -> Result<AdpTurnSampleEvidence, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(url))
        .await
        .map_err(|_| format!("ADP transcript connect timeout: {url}"))?
        .map_err(|err| format!("ADP transcript connect failed: {err}"))?;
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: request_id.to_owned(),
            query: UiCommand::QuerySessionTurns {
                session_id: session_id.clone(),
            },
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err(format!("ADP {} sample transcript timeout", sample.label()));
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| format!("ADP {} sample transcript timeout", sample.label()))??;
        match response {
            UiAdpResponse::QueryResult {
                request_id: response_id,
                result,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return sample_transcript_evidence(sample, session_id, &result).ok_or_else(|| {
                    format!(
                        "ADP {} sample transcript missing expected evidence",
                        sample.label()
                    )
                });
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP {} sample transcript failure {}: {}",
                    sample.label(),
                    failure.code,
                    failure.message
                ));
            }
            _ => {}
        }
    }
}

fn sample_query_terminal_failure_reason(
    sample: AdpTurnSample,
    session_id: &SessionId,
    result: &freehand_ui_protocol::UiQueryResult,
) -> Option<String> {
    match result {
        freehand_ui_protocol::UiQueryResult::Turn(Some(turn)) => {
            sample_turn_terminal_failure_reason(sample, session_id, turn)
        }
        freehand_ui_protocol::UiQueryResult::SessionTurns(transcript) => transcript
            .turns
            .iter()
            .find_map(|turn| sample_turn_terminal_failure_reason(sample, session_id, turn)),
        _ => None,
    }
}

fn sample_terminal_failure_reason(
    sample: AdpTurnSample,
    session_id: &SessionId,
    projection: &freehand_ui_protocol::UiProjection,
) -> Option<String> {
    match projection {
        freehand_ui_protocol::UiProjection::Turn(turn) => {
            sample_turn_terminal_failure_reason(sample, session_id, turn)
        }
        _ => None,
    }
}

fn sample_turn_terminal_failure_reason(
    sample: AdpTurnSample,
    session_id: &SessionId,
    turn: &freehand_ui_protocol::UiTurnProjection,
) -> Option<String> {
    if &turn.session_id != session_id {
        return None;
    }
    if turn.terminal_status.as_ref() == Some(&sample.expected_status()) {
        return None;
    }
    let status = turn.terminal_status.as_ref()?;
    Some(format!(
        "turn={} status={status:?} terminal={} errors={}",
        turn.turn_id.as_str(),
        turn.terminal_text.as_deref().unwrap_or("none"),
        if turn.errors.is_empty() {
            "none".to_owned()
        } else {
            turn.errors.join(" | ")
        }
    ))
}

#[derive(Debug, Clone)]
struct AdpTurnSampleEvidence {
    terminal_turn_id: String,
    rounds: usize,
    tool_executions: usize,
    failed_tools: usize,
}

fn sample_evidence_complete(
    sample: AdpTurnSample,
    evidence: Option<&AdpTurnSampleEvidence>,
) -> bool {
    let Some(evidence) = evidence else {
        return false;
    };
    match sample {
        AdpTurnSample::Success => evidence.rounds == 1 && evidence.tool_executions == 0,
        AdpTurnSample::Failure => {
            evidence.rounds >= 2 && evidence.tool_executions >= 1 && evidence.failed_tools >= 1
        }
    }
}

fn sample_transcript_evidence(
    sample: AdpTurnSample,
    session_id: &SessionId,
    result: &freehand_ui_protocol::UiQueryResult,
) -> Option<AdpTurnSampleEvidence> {
    let freehand_ui_protocol::UiQueryResult::SessionTurns(transcript) = result else {
        return None;
    };
    if &transcript.session_id != session_id {
        return None;
    }
    let rounds = transcript.turns.len();
    let mut terminal_turn_id = None::<String>;
    let mut tool_executions = BTreeSet::new();
    let mut failed_tools = BTreeSet::new();
    for turn in &transcript.turns {
        for activity in &turn.tool_activities {
            tool_executions.insert(activity.tool_call_id.clone());
            if activity.status.as_str() == "failed" {
                failed_tools.insert(activity.tool_call_id.clone());
            }
        }
        if turn.user_text.as_deref() == Some(sample.prompt())
            && turn.terminal_status.as_ref() == Some(&sample.expected_status())
        {
            terminal_turn_id = Some(turn.turn_id.as_str().to_owned());
        }
        if sample == AdpTurnSample::Failure
            && turn
                .turn_id
                .as_str()
                .rsplit_once("-r")
                .and_then(|(_, suffix)| suffix.parse::<usize>().ok())
                .is_some_and(|round| round >= 2)
            && turn.terminal_status.as_ref() == Some(&sample.expected_status())
        {
            terminal_turn_id = Some(turn.turn_id.as_str().to_owned());
        }
    }
    let terminal_turn_id = terminal_turn_id?;
    if transcript
        .turns
        .iter()
        .any(|turn| sample_turn_terminal_failure_reason(sample, session_id, turn).is_some())
    {
        return None;
    }
    Some(AdpTurnSampleEvidence {
        terminal_turn_id,
        rounds,
        tool_executions: tool_executions.len(),
        failed_tools: failed_tools.len(),
    })
}

fn run_adp_smoke(args: Vec<String>) -> Result<String, String> {
    if args.len() != 2 || args[0] != "--url" {
        return Err("usage: freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp".to_owned());
    }
    let url = args[1].clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_adp_smoke_async(url))
}

async fn run_adp_smoke_async(url: String) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(&url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;

    send_adp(
        &mut socket,
        UiAdpRequest::Subscribe {
            request_id: "cli-sub-1".to_owned(),
            subscription: UiCommand::SubscribeLatestActiveTurn {
                client: UiClientKind::Cli,
            },
        },
    )
    .await?;
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: "cli-query-1".to_owned(),
            query: UiCommand::QueryLatestActiveTurn,
        },
    )
    .await?;
    send_adp(
        &mut socket,
        UiAdpRequest::Command {
            request_id: "cli-bad-command-1".to_owned(),
            command: UiCommand::QueryLatestActiveTurn,
        },
    )
    .await?;

    let mut accepted = false;
    let mut event = false;
    let mut query = false;
    let mut mismatch_failure = false;
    let mut seen = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !(accepted && event && query && mismatch_failure) {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(format!("ADP smoke timeout seen={}", seen.join(",")));
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| format!("ADP smoke timeout seen={}", seen.join(",")))??;
        match response {
            UiAdpResponse::SubscriptionAccepted { request_id, .. } => {
                seen.push(format!("subscription_accepted:{request_id}"));
                if request_id == "cli-sub-1" {
                    accepted = true;
                }
            }
            UiAdpResponse::SubscriptionEvent { request_id, .. } => {
                seen.push(format!("subscription_event:{request_id}"));
                if request_id == "cli-sub-1" {
                    event = true;
                }
            }
            UiAdpResponse::QueryResult { request_id, .. } => {
                seen.push(format!("query_result:{request_id}"));
                if request_id == "cli-query-1" {
                    query = true;
                }
            }
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                seen.push(format!("failure:{request_id}:{}", failure.code));
                if request_id == "cli-bad-command-1"
                    && failure.code == "ingress_command_kind_mismatch"
                {
                    mismatch_failure = true;
                }
            }
            UiAdpResponse::CommandReceipt {
                request_id,
                receipt,
            } => {
                seen.push(format!(
                    "command_receipt:{request_id}:{}",
                    receipt.dispatch_status
                ));
            }
        }
    }
    let _ = socket.close(None).await;
    Ok(format!("adp_smoke_ok url={} seen={}", url, seen.join(",")))
}

async fn send_adp(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    request: UiAdpRequest,
) -> Result<(), String> {
    let body = serde_json::to_string(&request).map_err(|err| err.to_string())?;
    socket
        .send(Message::Text(body.into()))
        .await
        .map_err(|err| format!("ADP send failed: {err}"))
}

async fn next_adp(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Result<UiAdpResponse, String> {
    let Some(message) = socket.next().await else {
        return Err("ADP socket closed".to_owned());
    };
    let message = message.map_err(|err| format!("ADP receive failed: {err}"))?;
    match message {
        Message::Text(text) => {
            serde_json::from_str(&text).map_err(|err| format!("ADP response decode failed: {err}"))
        }
        other => Err(format!("unexpected ADP websocket message: {other:?}")),
    }
}

fn run_reason_live(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli reason-live --agent <name> --prompt <text> [--stream] [--session <id>]"
            .to_owned();
    if args.len() < 4 {
        return Err(usage);
    }
    if args[0] != "--agent" || args[2] != "--prompt" {
        return Err(usage);
    }
    let mut stream = false;
    let mut session_id = None::<String>;
    let mut index = 4;
    while index < args.len() {
        match args[index].as_str() {
            "--stream" => {
                stream = true;
                index += 1;
            }
            "--session" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(usage);
                };
                session_id = Some(value.clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }

    let config = load_default_config().map_err(|err| err.to_string())?;
    let selected = config
        .select_agent(&args[1])
        .map_err(|err| err.to_string())?;
    let runtime_home = default_config_path()
        .map_err(|err| err.to_string())?
        .parent()
        .ok_or_else(|| "default config path has no runtime home parent".to_owned())?
        .to_path_buf();
    let session_id =
        SessionId::new(session_id.unwrap_or_else(|| format!("cli-live-{}", selected.name)));
    let stamp = live_id_stamp()?;
    let outcome = run_live_reason_turn(
        &selected,
        LiveReasonTurnRequest {
            runtime_home,
            session_id,
            turn_id: TurnId::new(format!("cli-live-turn-{stamp}")),
            trace_id: TraceId::new(format!("cli-live-trace-{stamp}")),
            prompt: args[3].clone(),
            cwd: None,
            stream,
            cancel_token: None,
        },
    )
    .map_err(|err| err.to_string())?;

    let raw_text = outcome
        .turn
        .semantic_events
        .iter()
        .filter(|event| event.kind == SemanticEventKind::Text)
        .map(|event| event.content.as_str())
        .collect::<Vec<_>>()
        .join("");
    let text = strip_completion_submission_block(&raw_text);
    let reasoning_events = outcome
        .turn
        .semantic_events
        .iter()
        .filter(|event| event.kind == SemanticEventKind::Reasoning)
        .count();
    let latest_usage = outcome.turn.usage_events.last().map(|event| &event.usage);

    Ok(format!(
        "agent={} provider={} stream={} text={} reasoning_events={} usage_input_tokens={} usage_output_tokens={} broadcasts={} rounds={} schema_rejections={} tool_executions={} restore_status={} restored_closed_turns={} terminal={}",
        selected.name,
        selected.provider.id,
        stream,
        text.trim(),
        reasoning_events,
        latest_usage
            .map(|usage| usage.input_tokens.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        latest_usage
            .map(|usage| usage.output_tokens.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        outcome.broadcasts.len(),
        outcome.rounds,
        outcome.schema_rejections.len(),
        outcome.tool_executions,
        live_restore_status_label(outcome.restore_status),
        outcome.restored_closed_turns,
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.summary.as_str())
            .unwrap_or("none")
    ))
}

fn live_id_stamp() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(|err| err.to_string())
}

fn run_reason_persist_smoke(args: Vec<String>) -> Result<String, String> {
    if args.len() != 2 || args[0] != "--agent" {
        return Err("usage: freehand-cli reason-persist-smoke --agent <name>".to_owned());
    }
    let config = load_default_config().map_err(|err| err.to_string())?;
    let selected = config
        .select_agent(&args[1])
        .map_err(|err| err.to_string())?;
    let runtime_home = default_config_path()
        .map_err(|err| err.to_string())?
        .parent()
        .ok_or_else(|| "default config path has no runtime home parent".to_owned())?
        .to_path_buf();
    let report = run_reason_persistence_smoke(&selected.name, &runtime_home)
        .map_err(|err| err.to_string())?;
    Ok(format!(
        "agent={} restored_terminal={} reason_seq={} ui_sidecar_exists={} session_index_entries={}",
        selected.name,
        report.restored_terminal_summary,
        report.reason_seq,
        report.ui_sidecar_exists,
        report.session_index_entries
    ))
}

fn run_reason_e2e_smoke(args: Vec<String>) -> Result<String, String> {
    if args.len() != 4 || args[0] != "--agent" || args[2] != "--scenario" {
        return Err(
            "usage: freehand-cli reason-e2e --agent <name> --scenario <usage-compaction|recovery-block>"
                .to_owned(),
        );
    }
    let agent_name = &args[1];
    let scenario = ReasonRuntimeSmokeScenario::parse(&args[3]).ok_or_else(|| {
        "usage: freehand-cli reason-e2e --agent <name> --scenario <usage-compaction|recovery-block>"
            .to_owned()
    })?;

    let config = load_default_config().map_err(|err| err.to_string())?;
    let selected = config
        .select_agent(agent_name)
        .map_err(|err| err.to_string())?;

    let report =
        run_reason_runtime_smoke(&selected.name, scenario).map_err(|err| err.to_string())?;

    Ok(format!(
        "scenario={} agent={} rewrite_action={} rewrite_version={} latest_usage_tokens={} blocked={}",
        report.scenario.as_str(),
        selected.name,
        report.rewrite_action,
        report.rewrite_version,
        report
            .latest_usage_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        report.blocked
    ))
}

fn mode_label(mode: AgentMode) -> &'static str {
    mode.as_str()
}

fn provider_type_label(provider_type: freehand_config::ProviderType) -> &'static str {
    provider_type.as_str()
}

fn provider_protocol_label(protocol: freehand_config::ProviderProtocol) -> &'static str {
    protocol.as_str()
}

fn provider_auth_label(auth_type: freehand_config::ProviderAuthType) -> &'static str {
    auth_type.as_str()
}

fn live_restore_status_label(status: LiveReasonRestoreStatus) -> &'static str {
    match status {
        LiveReasonRestoreStatus::CreatedNew => "created_new",
        LiveReasonRestoreStatus::RestoredExisting => "restored_existing",
    }
}
