use freehand_blocks::strip_completion_submission_block;
use freehand_config::{AgentMode, default_config_path, load_default_config};
use freehand_contracts::{SemanticEventKind, SessionId, TraceId, TurnId};
use freehand_runtime::{LiveReasonRestoreStatus, LiveReasonTurnRequest, run_live_reason_turn};
use freehand_testkit::{
    ReasonRuntimeSmokeScenario, run_reason_persistence_smoke, run_reason_runtime_smoke,
};
use freehand_ui_protocol::{UiAdpRequest, UiAdpResponse, UiClientKind, UiCommand};
use futures_util::{SinkExt, StreamExt};
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
    if flag != "--agent" {
        return Err(
            "usage: freehand-cli --agent <name>\n   or: freehand-cli reason-e2e --agent <name> --scenario <usage-compaction|recovery-block>\n   or: freehand-cli reason-persist-smoke --agent <name>\n   or: freehand-cli reason-live --agent <name> --prompt <text> [--stream]\n   or: freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp"
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
