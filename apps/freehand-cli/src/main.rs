use freehand_blocks::strip_completion_submission_block;
use freehand_config::{AgentMode, default_config_path, load_default_config};
use freehand_contracts::{SemanticEventKind, SessionId, TraceId, TurnId};
use freehand_testkit::{
    LiveReasonTurnRequest, ReasonRuntimeSmokeScenario, run_live_reason_turn,
    run_reason_persistence_smoke, run_reason_runtime_smoke,
};

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
    if flag != "--agent" {
        return Err(
            "usage: freehand-cli --agent <name>\n   or: freehand-cli reason-e2e --agent <name> --scenario <usage-compaction|recovery-block>\n   or: freehand-cli reason-persist-smoke --agent <name>\n   or: freehand-cli reason-live --agent <name> --prompt <text> [--stream]"
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

fn run_reason_live(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli reason-live --agent <name> --prompt <text> [--stream]".to_owned();
    if args.len() != 4 && args.len() != 5 {
        return Err(usage);
    }
    if args[0] != "--agent" || args[2] != "--prompt" {
        return Err(usage);
    }
    let stream = match args.get(4) {
        None => false,
        Some(flag) if flag == "--stream" => true,
        Some(_) => return Err(usage),
    };

    let config = load_default_config().map_err(|err| err.to_string())?;
    let selected = config
        .select_agent(&args[1])
        .map_err(|err| err.to_string())?;
    let outcome = run_live_reason_turn(
        &selected,
        LiveReasonTurnRequest {
            session_id: SessionId::new("cli-live-session"),
            turn_id: TurnId::new("cli-live-turn-1"),
            trace_id: TraceId::new("cli-live-trace-1"),
            prompt: args[3].clone(),
            stream,
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
        "agent={} provider={} stream={} text={} reasoning_events={} usage_input_tokens={} usage_output_tokens={} broadcasts={} rounds={} schema_rejections={} terminal={}",
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
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.summary.as_str())
            .unwrap_or("none")
    ))
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
