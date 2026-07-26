use std::env;
use std::path::PathBuf;
use std::process;

use freehand_testkit::{
    ReasonRuntimeSmokeScenario, run_reason_persistence_smoke, run_reason_runtime_smoke,
};

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        return Err(usage());
    };
    match command.as_str() {
        "reason-e2e" => run_reason_e2e(args.collect()),
        "reason-persist-smoke" => run_reason_persist_smoke(args.collect()),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: freehand-reason-smoke reason-e2e --agent <name> --scenario <usage-compaction|recovery-block>\n   or: freehand-reason-smoke reason-persist-smoke --agent <name> --runtime-home <path>".to_owned()
}

fn run_reason_e2e(args: Vec<String>) -> Result<(), String> {
    if args.len() != 4 || args[0] != "--agent" || args[2] != "--scenario" {
        return Err(usage());
    }
    let agent_name = &args[1];
    let scenario = ReasonRuntimeSmokeScenario::parse(&args[3]).ok_or_else(usage)?;
    let report = run_reason_runtime_smoke(agent_name, scenario).map_err(|err| err.to_string())?;
    println!(
        "scenario={} agent={} rewrite_action={} rewrite_version={} latest_usage_tokens={} blocked={}",
        report.scenario.as_str(),
        agent_name,
        report.rewrite_action,
        report.rewrite_version,
        report
            .latest_usage_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        report.blocked
    );
    Ok(())
}

fn run_reason_persist_smoke(args: Vec<String>) -> Result<(), String> {
    if args.len() != 4 || args[0] != "--agent" || args[2] != "--runtime-home" {
        return Err(usage());
    }
    let agent_name = &args[1];
    let runtime_home = PathBuf::from(&args[3]);
    let report =
        run_reason_persistence_smoke(agent_name, &runtime_home).map_err(|err| err.to_string())?;
    println!(
        "agent={} restored_terminal={} reason_seq={} ui_sidecar_exists={} session_index_entries={}",
        agent_name,
        report.restored_terminal_summary,
        report.reason_seq,
        report.ui_sidecar_exists,
        report.session_index_entries
    );
    Ok(())
}
