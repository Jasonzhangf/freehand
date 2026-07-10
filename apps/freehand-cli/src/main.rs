use freehand_blocks::strip_completion_submission_block;
use freehand_config::{AgentMode, default_config_path, load_default_config};
use freehand_contracts::{AgentId, SemanticEventKind, SessionId, TerminalStatus, TraceId, TurnId};
use freehand_runtime::{
    LiveReasonRestoreStatus, LiveReasonTurnRequest, load_default_runtime_agent,
    run_live_reason_turn,
};
use freehand_task::{
    ExecutionFact, ExecutionFactKind, TaskActor, TaskAssignRequest, TaskClaimRequest,
    TaskCreateRequest, TaskDispatchRequest, TaskId, TaskParentRef, TaskReviewRejection,
    TaskReviewSubmission, TaskRuntime, TaskWatermark,
};
use freehand_testkit::{
    ReasonRuntimeSmokeScenario, run_reason_persistence_smoke, run_reason_runtime_smoke,
};
use freehand_ui_protocol::{
    UiAdpRequest, UiAdpResponse, UiAgentBoardProjection, UiAgentLifecycleProjection, UiClientKind,
    UiCommand, UiExecutionFactCommand, UiExecutionFactKind, UiMasterPollProjection,
    UiModelRequestKind, UiProviderConfigUpdate, UiQueryResult, UiSchedulerTickCommand,
    UiTaskAgentCreateCommand, UiTaskAssignCommand, UiTaskBoardProjection, UiTaskClaimCommand,
    UiTaskCreateCommand, UiTaskDispatchCommand, UiTaskEventInboxProjection, UiTaskReviewCommand,
    UiTaskReviewRejectionCommand, UiWorkerControlCommand, UiWorkerControlProjection,
};
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
    if flag == "session-continue-sample" {
        return run_session_continue_sample(args.collect());
    }
    if flag == "task-lifecycle-sample" {
        return run_task_lifecycle_sample(args.collect());
    }
    if flag == "task-restart-seed-review" {
        return run_task_restart_seed(args.collect(), RestartSeedState::Review);
    }
    if flag == "task-restart-seed-rejected" {
        return run_task_restart_seed(args.collect(), RestartSeedState::Rejected);
    }
    if flag == "task-restart-seed-blocked" {
        return run_task_restart_seed(args.collect(), RestartSeedState::Blocked);
    }
    if flag == "task-restart-seed-running" {
        return run_task_restart_seed(args.collect(), RestartSeedState::Running);
    }
    if flag == "phase1-foundation-sample" {
        return run_phase1_foundation_sample(args.collect());
    }
    if flag == "master-worker-foundation-sample" {
        return run_master_worker_foundation_sample(args.collect());
    }
    if flag == "master-worker-autonomy-sample" {
        return run_master_worker_autonomy_sample(args.collect());
    }
    if flag == "master-poll-foundation-sample" {
        return run_master_poll_foundation_sample(args.collect());
    }
    if flag == "worker-control-foundation-sample" {
        return run_worker_control_foundation_sample(args.collect());
    }
    if flag == "adp-session-query" {
        return run_adp_session_query(args.collect());
    }
    if flag == "adp-session-manage" {
        return run_adp_session_manage(args.collect());
    }
    if flag == "adp-config-query" {
        return run_adp_config_query(args.collect());
    }
    if flag == "adp-config-update" {
        return run_adp_config_update(args.collect());
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
            "usage: freehand-cli --agent <name>\n   or: freehand-cli reason-e2e --agent <name> --scenario <usage-compaction|recovery-block>\n   or: freehand-cli reason-persist-smoke --agent <name>\n   or: freehand-cli reason-live --agent <name> --prompt <text> [--stream]\n   or: freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp\n   or: freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample <success|failure|schema-mismatch|provider-retry>\n   or: freehand-cli session-continue-sample --url ws://127.0.0.1:4041/adp\n   or: freehand-cli task-lifecycle-sample --url ws://127.0.0.1:4041/adp\n   or: freehand-cli task-restart-seed-review --agent master --task <id> --worker <id> --execution <id> --target-cwd <path> --summary <text>\n   or: freehand-cli task-restart-seed-rejected --agent master --task <id> --worker <id> --execution <id> --target-cwd <path> --summary <text>\n   or: freehand-cli task-restart-seed-blocked --agent master --task <id> --worker <id> --execution <id> --target-cwd <path> --summary <text>\n   or: freehand-cli task-restart-seed-running --agent master --task <id> --worker <id> --execution <id> --target-cwd <path> --summary <text>\n   or: freehand-cli phase1-foundation-sample --url ws://127.0.0.1:4041/adp [--verify-task <task_id> --review-task <task_id> --execution <id> --agent <id>]\n   or: freehand-cli master-worker-foundation-sample --url ws://127.0.0.1:4041/adp [--verify-task <task_id> --execution <id> --agent <id>]\n   or: freehand-cli master-worker-autonomy-sample --url ws://127.0.0.1:4041/adp [--scenario <all|success|execution-error|reject-retry>] [--verify-task <task_id> --execution <id> --agent <id>]\n   or: freehand-cli master-poll-foundation-sample --url ws://127.0.0.1:4041/adp [--verify-task <task_id> --execution <id> --agent <id> --cursor <cursor>]\n   or: freehand-cli worker-control-foundation-sample --url ws://127.0.0.1:4041/adp [--verify-task <task_id> --execution <id> --agent <id> --control <control_id>]\n   or: freehand-cli adp-session-query --url ws://127.0.0.1:4041/adp [--session <id>]\n   or: freehand-cli adp-session-manage --url ws://127.0.0.1:4041/adp --action <create|rename|archive|restore|delete> --session <id> [--title <title>] [--cwd <path>]\n   or: freehand-cli adp-config-query --url ws://127.0.0.1:4041/adp\n   or: freehand-cli adp-config-update --url ws://127.0.0.1:4041/adp --agent <name> --provider <id> --type <openai|anthropic> --protocol <responses|chat_completions|messages> --base-url <url> --model <model> --api-key-env <ENV>\n   or: freehand-cli adp-task-query --url ws://127.0.0.1:4041/adp [--status <status>] [--agent <id>] [--history <task_id>]\n   or: freehand-cli adp-task-subscribe --url ws://127.0.0.1:4041/adp [--status <status>] [--agent <id>]\n   or: freehand-cli adp-error-query --url ws://127.0.0.1:4041/adp --session <id> [--trace <id>] [--turn <id>] [--domain <domain>]"
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
        "agent={} mode={} allowed_pair_ip={} pair_token_env={} provider={} provider_type={} provider_protocol={} default_model={} base_url={} provider_auth_source={} restart_required_on_change={}",
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
        selected.provider.auth_source.as_str(),
        selected.restart_required_on_change
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdpTurnSample {
    Success,
    Failure,
    SchemaMismatch,
    ProviderRetry,
}

impl AdpTurnSample {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "success" => Ok(Self::Success),
            "failure" => Ok(Self::Failure),
            "schema-mismatch" => Ok(Self::SchemaMismatch),
            "provider-retry" => Ok(Self::ProviderRetry),
            _ => Err(
                "sample must be one of: success, failure, schema-mismatch, provider-retry"
                    .to_owned(),
            ),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::SchemaMismatch => "schema-mismatch",
            Self::ProviderRetry => "provider-retry",
        }
    }

    fn expected_status(self) -> TerminalStatus {
        match self {
            Self::Success | Self::Failure | Self::SchemaMismatch => TerminalStatus::Success,
            Self::ProviderRetry => TerminalStatus::Failed,
        }
    }

    fn prompt(self) -> &'static str {
        match self {
            Self::Success => {
                "ADP success sample: answer with one short sentence and a valid Freehand completion schema. Do not call tools."
            }
            Self::Failure => {
                "ADP failure sample: call the read_file tool exactly once with path definitely-missing-freehand-file.txt, then use the failed tool result to continue and report success through the required Freehand completion schema."
            }
            Self::SchemaMismatch => {
                "ADP schema mismatch sample: do not call tools. First answer must intentionally omit the required Freehand completion schema so the client can return the schema issue to you; after that feedback, polish the response into the required schema and finish successfully."
            }
            Self::ProviderRetry => {
                "ADP provider retry sample: make one short answer. This sample is valid only when the daemon/provider path emits provider-domain retry evidence; do not call tools and do not produce schema mismatch."
            }
        }
    }
}

fn run_adp_turn_sample(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample <success|failure|schema-mismatch|provider-retry>"
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

fn run_session_continue_sample(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli session-continue-sample --url ws://127.0.0.1:4041/adp".to_owned();
    if args.len() != 2 || args[0] != "--url" {
        return Err(usage);
    }
    let url = args[1].clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_session_continue_sample_async(url))
}

fn run_task_lifecycle_sample(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli task-lifecycle-sample --url ws://127.0.0.1:4041/adp".to_owned();
    if args.len() != 2 || args[0] != "--url" {
        return Err(usage);
    }
    let url = args[1].clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_task_lifecycle_sample_async(url))
}

#[derive(Debug, Clone, Copy)]
enum RestartSeedState {
    Review,
    Rejected,
    Blocked,
    Running,
}

impl RestartSeedState {
    fn command(self) -> &'static str {
        match self {
            Self::Review => "task-restart-seed-review",
            Self::Rejected => "task-restart-seed-rejected",
            Self::Blocked => "task-restart-seed-blocked",
            Self::Running => "task-restart-seed-running",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Rejected => "rejected",
            Self::Blocked => "blocked",
            Self::Running => "running",
        }
    }
}

fn run_task_restart_seed(
    args: Vec<String>,
    seed_state: RestartSeedState,
) -> Result<String, String> {
    let usage = format!(
        "usage: freehand-cli {} --agent master --task <id> --worker <id> --execution <id> --target-cwd <path> --summary <text> [--ttl-seconds <seconds>]",
        seed_state.command()
    );
    let mut agent = None::<String>;
    let mut task = None::<String>;
    let mut worker = None::<String>;
    let mut execution = None::<String>;
    let mut target_cwd = None::<String>;
    let mut summary = None::<String>;
    let mut ttl_seconds = 300_u64;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--agent" if index + 1 < args.len() => {
                agent = Some(args[index + 1].clone());
                index += 2;
            }
            "--task" if index + 1 < args.len() => {
                task = Some(args[index + 1].clone());
                index += 2;
            }
            "--worker" if index + 1 < args.len() => {
                worker = Some(args[index + 1].clone());
                index += 2;
            }
            "--execution" if index + 1 < args.len() => {
                execution = Some(args[index + 1].clone());
                index += 2;
            }
            "--target-cwd" if index + 1 < args.len() => {
                target_cwd = Some(args[index + 1].clone());
                index += 2;
            }
            "--summary" if index + 1 < args.len() => {
                summary = Some(args[index + 1].clone());
                index += 2;
            }
            "--ttl-seconds" if index + 1 < args.len() => {
                ttl_seconds = args[index + 1].parse::<u64>().map_err(|_| usage.clone())?;
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let agent = agent.ok_or_else(|| usage.clone())?;
    let task_id = TaskId::new(task.ok_or_else(|| usage.clone())?);
    let worker_id = AgentId::new(worker.ok_or_else(|| usage.clone())?);
    let execution_id = execution.ok_or_else(|| usage.clone())?;
    let target_cwd = target_cwd.ok_or_else(|| usage.clone())?;
    let summary = summary.ok_or(usage)?;

    let bootstrap = load_default_runtime_agent(&agent).map_err(|err| err.to_string())?;
    if bootstrap.selected_agent.mode != AgentMode::Master {
        return Err(format!(
            "task restart seed requires a master agent, got {} for {}",
            bootstrap.selected_agent.mode.as_str(),
            bootstrap.selected_agent.name
        ));
    }
    let owner = AgentId::new(bootstrap.selected_agent.name.clone());
    let runtime =
        TaskRuntime::boot(&bootstrap.runtime_home, owner.clone()).map_err(|err| err.to_string())?;
    let actor = cli_task_actor(&owner, seed_state.command());
    let watermark = cli_task_watermark(seed_state.command());
    let session_id = SessionId::new(format!("cli-restart-seed-{}", live_id_stamp()?));
    let turn_id = TurnId::new(format!("turn-restart-seed-{}", task_id.as_str()));
    let trace_id = TraceId::new(format!("trace-restart-seed-{}", task_id.as_str()));

    runtime
        .create_task(TaskCreateRequest {
            task_id: Some(task_id.clone()),
            title: format!("Restart seed {}", task_id.as_str()),
            content: summary.clone(),
            goal: format!(
                "seed {} task while master daemon is stopped",
                seed_state.label()
            ),
            deliverables: vec![summary.clone()],
            acceptance: vec![
                format!(
                    "master lifecycle runner consumes this {} truth after restart",
                    seed_state.label()
                ),
                "task reaches the expected post-restart lifecycle decision".to_owned(),
            ],
            priority: 100,
            target_cwd: Some(target_cwd.clone()),
            dispatch: TaskDispatchRequest::None,
            parent: TaskParentRef {
                session_id: Some(session_id),
                turn_id: Some(turn_id),
                trace_id: Some(trace_id),
            },
            actor: actor.clone(),
            watermark: watermark.clone(),
        })
        .map_err(|err| err.to_string())?;
    runtime
        .assign_task(TaskAssignRequest {
            task_id: task_id.clone(),
            agent_id: worker_id.clone(),
            actor: actor.clone(),
            watermark: watermark.clone(),
        })
        .map_err(|err| err.to_string())?;
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: worker_id.clone(),
            execution_id: execution_id.clone(),
            ttl_seconds,
            actor: cli_task_actor(&worker_id, seed_state.command()),
            watermark: watermark.clone(),
        })
        .map_err(|err| err.to_string())?;
    let worker_actor = cli_task_actor(&worker_id, seed_state.command());
    match seed_state {
        RestartSeedState::Review => {
            runtime
                .submit_review(TaskReviewSubmission {
                    task_id: task_id.clone(),
                    summary: summary.clone(),
                    deliverables: vec![summary],
                    evidence: vec![
                        format!("target_cwd={target_cwd}"),
                        format!("execution_id={execution_id}"),
                    ],
                    actor: worker_actor,
                    watermark,
                })
                .map_err(|err| err.to_string())?;
        }
        RestartSeedState::Rejected => {
            runtime
                .submit_review(TaskReviewSubmission {
                    task_id: task_id.clone(),
                    summary: format!("bad pre-restart review: {summary}"),
                    deliverables: vec!["pre-restart incomplete deliverable".to_owned()],
                    evidence: vec![
                        format!("target_cwd={target_cwd}"),
                        format!("execution_id={execution_id}"),
                    ],
                    actor: worker_actor,
                    watermark: watermark.clone(),
                })
                .map_err(|err| err.to_string())?;
            runtime
                .reject_review(TaskReviewRejection {
                    task_id: task_id.clone(),
                    reject_reason: format!("pre-restart rejection for {}", task_id.as_str()),
                    next_requirements: vec![summary],
                    actor,
                    watermark,
                })
                .map_err(|err| err.to_string())?;
        }
        RestartSeedState::Blocked => {
            runtime
                .apply_execution_fact(ExecutionFact {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_id.clone(),
                    turn_id: None,
                    occurred_at: unix_now_seconds()?,
                    watermark,
                    kind: ExecutionFactKind::Blocked {
                        reason: summary.clone(),
                        evidence: vec![
                            format!("target_cwd={target_cwd}"),
                            format!("execution_id={execution_id}"),
                        ],
                    },
                })
                .map_err(|err| err.to_string())?;
        }
        RestartSeedState::Running => {}
    }
    let history = runtime
        .task_history(&task_id)
        .map_err(|err| err.to_string())?;
    Ok(format!(
        "task_restart_seed_{}_ok agent={} task={} worker={} execution={} events={}",
        seed_state.label(),
        agent,
        task_id.as_str(),
        worker_id.as_str(),
        execution_id,
        history
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>()
            .join(",")
    ))
}

fn cli_task_actor(agent_id: &AgentId, source: &str) -> TaskActor {
    TaskActor {
        agent_id: agent_id.clone(),
        source: format!("app.cli-runtime-smoke.{source}"),
        session_id: None,
        turn_id: None,
        trace_id: None,
    }
}

fn cli_task_watermark(hook: &str) -> TaskWatermark {
    TaskWatermark {
        metadata_id: None,
        hook: Some(hook.to_owned()),
        action_tool_call_id: None,
    }
}

fn unix_now_seconds() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_secs())
}

fn run_phase1_foundation_sample(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli phase1-foundation-sample --url ws://127.0.0.1:4041/adp [--verify-task <task_id> --review-task <task_id> --execution <id> --agent <id>]"
            .to_owned();
    let mut url = None::<String>;
    let mut verify_task = None::<String>;
    let mut review_task = None::<String>;
    let mut execution = None::<String>;
    let mut agent = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--verify-task" if index + 1 < args.len() => {
                verify_task = Some(args[index + 1].clone());
                index += 2;
            }
            "--review-task" if index + 1 < args.len() => {
                review_task = Some(args[index + 1].clone());
                index += 2;
            }
            "--execution" if index + 1 < args.len() => {
                execution = Some(args[index + 1].clone());
                index += 2;
            }
            "--agent" if index + 1 < args.len() => {
                agent = Some(args[index + 1].clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let url = url.ok_or_else(|| usage.clone())?;
    let verify = match (verify_task, review_task, execution, agent) {
        (None, None, None, None) => None,
        (Some(task_id), Some(review_task_id), Some(execution_id), Some(agent_id)) => {
            Some(Phase1VerifyIds {
                blocked_task_id: task_id,
                review_task_id,
                execution_id,
                agent_id,
            })
        }
        _ => return Err(usage),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_phase1_foundation_sample_async(url, verify))
}

fn run_master_worker_foundation_sample(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli master-worker-foundation-sample --url ws://127.0.0.1:4041/adp [--verify-task <task_id> --execution <id> --agent <id>]"
            .to_owned();
    let mut url = None::<String>;
    let mut task_id = None::<String>;
    let mut execution = None::<String>;
    let mut agent = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--verify-task" if index + 1 < args.len() => {
                task_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--execution" if index + 1 < args.len() => {
                execution = Some(args[index + 1].clone());
                index += 2;
            }
            "--agent" if index + 1 < args.len() => {
                agent = Some(args[index + 1].clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let url = url.ok_or_else(|| usage.clone())?;
    let verify = match (task_id, execution, agent) {
        (None, None, None) => None,
        (Some(task_id), Some(execution_id), Some(agent_id)) => Some(MasterWorkerVerifyIds {
            task_id,
            execution_id,
            agent_id,
        }),
        _ => return Err(usage),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_master_worker_foundation_sample_async(url, verify))
}

fn run_master_worker_autonomy_sample(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli master-worker-autonomy-sample --url ws://127.0.0.1:4041/adp [--scenario <all|success|execution-error|reject-retry>] [--verify-task <task_id> --execution <id> --agent <id>]"
            .to_owned();
    let mut url = None::<String>;
    let mut scenario = None::<MasterWorkerAutonomyScenario>;
    let mut task_id = None::<String>;
    let mut execution = None::<String>;
    let mut agent = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--scenario" if index + 1 < args.len() => {
                if args[index + 1] == "all" {
                    scenario = None;
                } else {
                    scenario = Some(MasterWorkerAutonomyScenario::parse(&args[index + 1])?);
                }
                index += 2;
            }
            "--verify-task" if index + 1 < args.len() => {
                task_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--execution" if index + 1 < args.len() => {
                execution = Some(args[index + 1].clone());
                index += 2;
            }
            "--agent" if index + 1 < args.len() => {
                agent = Some(args[index + 1].clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let url = url.ok_or_else(|| usage.clone())?;
    let verify = match (task_id, execution, agent) {
        (None, None, None) => None,
        (Some(task_id), Some(execution_id), Some(agent_id)) => {
            let scenario = scenario.ok_or_else(|| {
                "--scenario is required with master-worker-autonomy verify".to_owned()
            })?;
            Some(MasterWorkerAutonomyVerifyIds {
                scenario,
                task_id,
                execution_id,
                agent_id,
            })
        }
        _ => return Err(usage),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_master_worker_autonomy_sample_async(
        url, scenario, verify,
    ))
}

fn run_master_poll_foundation_sample(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli master-poll-foundation-sample --url ws://127.0.0.1:4041/adp [--verify-task <task_id> --execution <id> --agent <id> --cursor <cursor>]"
            .to_owned();
    let mut url = None::<String>;
    let mut task_id = None::<String>;
    let mut execution = None::<String>;
    let mut agent = None::<String>;
    let mut cursor = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--verify-task" if index + 1 < args.len() => {
                task_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--execution" if index + 1 < args.len() => {
                execution = Some(args[index + 1].clone());
                index += 2;
            }
            "--agent" if index + 1 < args.len() => {
                agent = Some(args[index + 1].clone());
                index += 2;
            }
            "--cursor" if index + 1 < args.len() => {
                cursor = Some(args[index + 1].clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let url = url.ok_or_else(|| usage.clone())?;
    let verify = match (task_id, execution, agent, cursor) {
        (None, None, None, None) => None,
        (Some(task_id), Some(execution_id), Some(agent_id), Some(cursor)) => {
            Some(MasterPollVerifyIds {
                task_id,
                execution_id,
                agent_id,
                cursor,
            })
        }
        _ => return Err(usage),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_master_poll_foundation_sample_async(url, verify))
}

fn run_worker_control_foundation_sample(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli worker-control-foundation-sample --url ws://127.0.0.1:4041/adp [--verify-task <task_id> --execution <id> --agent <id> --control <control_id>]"
            .to_owned();
    let mut url = None::<String>;
    let mut task_id = None::<String>;
    let mut execution = None::<String>;
    let mut agent = None::<String>;
    let mut control = None::<String>;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--verify-task" if index + 1 < args.len() => {
                task_id = Some(args[index + 1].clone());
                index += 2;
            }
            "--execution" if index + 1 < args.len() => {
                execution = Some(args[index + 1].clone());
                index += 2;
            }
            "--agent" if index + 1 < args.len() => {
                agent = Some(args[index + 1].clone());
                index += 2;
            }
            "--control" if index + 1 < args.len() => {
                control = Some(args[index + 1].clone());
                index += 2;
            }
            _ => return Err(usage),
        }
    }
    let url = url.ok_or_else(|| usage.clone())?;
    let verify = match (task_id, execution, agent, control) {
        (None, None, None, None) => None,
        (Some(task_id), Some(execution_id), Some(agent_id), Some(control_id)) => {
            Some(WorkerControlVerifyIds {
                task_id,
                execution_id,
                agent_id,
                control_id,
            })
        }
        _ => return Err(usage),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_worker_control_foundation_sample_async(url, verify))
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
        "usage: freehand-cli adp-session-manage --url ws://127.0.0.1:4041/adp --action <create|rename|archive|restore|delete|rollback> --session <id> [--title <title>] [--cwd <path>]"
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
        "rollback" => UiCommand::RollbackLatestSessionTurn { session_id },
        _ => {
            return Err(
                "action must be one of: create, rename, archive, restore, delete, rollback"
                    .to_owned(),
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

fn run_adp_config_query(args: Vec<String>) -> Result<String, String> {
    let usage = "usage: freehand-cli adp-config-query --url ws://127.0.0.1:4041/adp".to_owned();
    if args.len() != 2 || args[0] != "--url" {
        return Err(usage);
    }
    let url = args[1].clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(run_adp_config_query_async(url))
}

fn run_adp_config_update(args: Vec<String>) -> Result<String, String> {
    let usage =
        "usage: freehand-cli adp-config-update --url ws://127.0.0.1:4041/adp --agent <name> --provider <id> --type <openai|anthropic> --protocol <responses|chat_completions|messages> --base-url <url> --model <model> --api-key-env <ENV>"
            .to_owned();
    let mut url = None::<String>;
    let mut update = UiProviderConfigUpdate {
        agent_name: String::new(),
        provider_id: String::new(),
        provider_type: String::new(),
        provider_protocol: String::new(),
        base_url: String::new(),
        default_model: String::new(),
        api_key_env: String::new(),
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--url" if index + 1 < args.len() => {
                url = Some(args[index + 1].clone());
                index += 2;
            }
            "--agent" if index + 1 < args.len() => {
                update.agent_name = args[index + 1].clone();
                index += 2;
            }
            "--provider" if index + 1 < args.len() => {
                update.provider_id = args[index + 1].clone();
                index += 2;
            }
            "--type" if index + 1 < args.len() => {
                update.provider_type = args[index + 1].clone();
                index += 2;
            }
            "--protocol" if index + 1 < args.len() => {
                update.provider_protocol = args[index + 1].clone();
                index += 2;
            }
            "--base-url" if index + 1 < args.len() => {
                update.base_url = args[index + 1].clone();
                index += 2;
            }
            "--model" if index + 1 < args.len() => {
                update.default_model = args[index + 1].clone();
                index += 2;
            }
            "--api-key-env" if index + 1 < args.len() => {
                update.api_key_env = args[index + 1].clone();
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
    runtime.block_on(run_adp_config_update_async(url, update))
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
        UiCommand::RollbackLatestSessionTurn { .. } => "rollback",
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

async fn run_adp_config_query_async(url: String) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(&url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;
    let request_id = "cli-config-query-1".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: request_id.clone(),
            query: UiCommand::QueryConfigStatus,
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("ADP config query timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP config query timeout".to_owned())??;
        match response {
            UiAdpResponse::QueryResult {
                request_id: response_id,
                result,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return summarize_config_query_result(&url, &result);
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP config query failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }
}

async fn run_adp_config_update_async(
    url: String,
    update: UiProviderConfigUpdate,
) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(&url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;
    let request_id = "cli-config-update-1".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Command {
            request_id: request_id.clone(),
            command: UiCommand::UpdateProviderConfig { update },
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let receipt = loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("ADP config update timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP config update timeout".to_owned())??;
        match response {
            UiAdpResponse::CommandReceipt {
                request_id: response_id,
                receipt,
            } if response_id == request_id => break receipt,
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP config update failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    };
    let query_request_id = "cli-config-update-query-1".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: query_request_id.clone(),
            query: UiCommand::QueryConfigStatus,
        },
    )
    .await?;
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("ADP config update projection timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP config update projection timeout".to_owned())??;
        match response {
            UiAdpResponse::QueryResult {
                request_id: response_id,
                result,
            } if response_id == query_request_id => {
                let _ = socket.close(None).await;
                let summary = summarize_config_query_result(&url, &result)?;
                return Ok(format!(
                    "adp_config_update_ok status={} {}",
                    receipt.dispatch_status, summary
                ));
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == query_request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP config update projection failure {}: {}",
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

fn summarize_config_query_result(
    url: &str,
    result: &freehand_ui_protocol::UiQueryResult,
) -> Result<String, String> {
    match result {
        freehand_ui_protocol::UiQueryResult::ConfigStatus(status) => {
            let output = format!(
                "adp_config_query_ok url={} agent={} mode={} node={} paired_agent={} paired_mode={} paired_node={} provider={} provider_type={} provider_protocol={} base_url_host={} default_model={} auth_type={} auth_source={} restart_required_on_change={}",
                url,
                status.agent_name,
                status.agent_mode,
                status.node_id,
                status.paired_agent_name,
                status.paired_agent_mode,
                status.paired_node_id,
                status.provider_id,
                status.provider_type,
                status.provider_protocol,
                status.provider_base_url_host,
                status.default_model,
                status.provider_auth_type,
                status.provider_auth_source,
                status.restart_required_on_change
            );
            if output.contains("api_key")
                || output.contains("pair_token")
                || output.contains("sk-")
                || output.contains("secret")
            {
                return Err("ADP config query attempted to print secret-bearing fields".to_owned());
            }
            Ok(output)
        }
        _ => Err("ADP config query returned non-config result".to_owned()),
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
            "ADP {} sample transcript incomplete rounds={} tool_executions={} failed_tools={} schema_retries={} provider_retries={} seen={}",
            sample.label(),
            evidence.rounds,
            evidence.tool_executions,
            evidence.failed_tools,
            evidence.schema_retries,
            evidence.provider_retries,
            seen.join(",")
        ));
    }
    Ok(format!(
        "adp_turn_sample_ok sample={} url={} session={} turn={} rounds={} tool_executions={} failed_tools={} schema_retries={} provider_retries={} seen={}",
        sample.label(),
        url,
        session_id.as_str(),
        evidence.terminal_turn_id,
        evidence.rounds,
        evidence.tool_executions,
        evidence.failed_tools,
        evidence.schema_retries,
        evidence.provider_retries,
        seen.join(",")
    ))
}

async fn run_session_continue_sample_async(url: String) -> Result<String, String> {
    let session_id = SessionId::new(format!("cli-session-continue-{}", live_id_stamp()?));
    let token = format!("FHCLI{}", live_id_stamp()?);
    let first_prompt = format!(
        "Session continuation sample first turn: remember token {token}. Answer briefly with the required Freehand completion schema."
    );
    let second_prompt = "Session continuation sample second turn: reply with the exact token from the previous turn and finish with the required Freehand completion schema.".to_owned();

    let first_seen =
        submit_adp_sample_prompt(&url, &session_id, "session-continue-first", first_prompt).await?;
    let second_seen =
        submit_adp_sample_prompt(&url, &session_id, "session-continue-second", second_prompt)
            .await?;
    let transcript = query_session_transcript(
        &url,
        &session_id,
        "cli-session-continue-transcript",
        Duration::from_secs(20),
    )
    .await?;
    if transcript.turns.len() < 2 {
        return Err(format!(
            "session continuation transcript incomplete turns={}",
            transcript.turns.len()
        ));
    }
    let Some(last_turn) = transcript.turns.last() else {
        return Err("session continuation transcript missing last turn".to_owned());
    };
    let terminal_text = last_turn.terminal_text.as_deref().unwrap_or("");
    let text = last_turn.text.join("\n");
    if !terminal_text.contains(&token) && !text.contains(&token) {
        return Err(format!(
            "session continuation missing token evidence session={} token={} turns={} last_turn={} terminal={}",
            session_id.as_str(),
            token,
            transcript.turns.len(),
            last_turn.turn_id.as_str(),
            terminal_text
        ));
    }
    Ok(format!(
        "session_continue_sample_ok url={} session={} token={} turns={} first_turn={} second_turn={} first_seen={} second_seen={}",
        url,
        session_id.as_str(),
        token,
        transcript.turns.len(),
        transcript.turns[0].turn_id.as_str(),
        last_turn.turn_id.as_str(),
        first_seen.join(","),
        second_seen.join(",")
    ))
}

async fn run_task_lifecycle_sample_async(url: String) -> Result<String, String> {
    let session_id = SessionId::new(format!("cli-task-lifecycle-{}", live_id_stamp()?));
    let token = format!("FHTASK{}", live_id_stamp()?);
    let task_id = format!("task-cli-{token}");
    let mut seen = Vec::new();
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-task-lifecycle-create",
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some(task_id.clone()),
                    title: format!("Lifecycle {token}"),
                    content: format!("Headless task lifecycle sample {token}"),
                    goal: format!("Close task {token}"),
                    deliverables: vec!["headless task lifecycle sample".to_owned()],
                    acceptance: vec!["accepted summary recorded".to_owned()],
                    priority: 50,
                    target_cwd: None,
                    session_id: Some(session_id.clone()),
                    turn_id: None,
                    dispatch: None,
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-task-lifecycle-review",
            UiCommand::SubmitTaskReview {
                review: UiTaskReviewCommand {
                    task_id: task_id.clone(),
                    summary: format!("Accepted summary for {token}"),
                    deliverables: vec!["headless task lifecycle sample".to_owned()],
                    evidence: vec![format!("token {token}")],
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-task-lifecycle-approve",
            UiCommand::ApproveTaskReview {
                task_id: task_id.clone(),
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-task-lifecycle-close",
            UiCommand::CloseTask {
                task_id: task_id.clone(),
            },
        )
        .await?,
    );
    let task = query_task_list_find_token(&url, &token).await?;
    if task.task_id != task_id {
        return Err(format!(
            "task lifecycle sample returned wrong task expected={} actual={}",
            task_id, task.task_id
        ));
    }
    if !task.status.eq_ignore_ascii_case("closed") {
        return Err(format!(
            "task lifecycle sample task not closed task={} status={} token={}",
            task.task_id, task.status, token
        ));
    }
    let history = query_task_history(&url, &task.task_id).await?;
    let event_types = history
        .events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    for required in [
        "TaskCreated",
        "TaskReviewSubmitted",
        "TaskReviewApproved",
        "TaskClosed",
    ] {
        if !event_types
            .iter()
            .any(|event_type| event_type.eq_ignore_ascii_case(required))
        {
            return Err(format!(
                "task lifecycle sample missing event task={} required={} events={}",
                task.task_id,
                required,
                event_types.join(",")
            ));
        }
    }
    Ok(format!(
        "task_lifecycle_sample_ok url={} session={} task={} status={} events={} seen={}",
        &url,
        session_id.as_str(),
        task.task_id,
        task.status,
        event_types.join(","),
        seen.join(",")
    ))
}

#[derive(Debug, Clone)]
struct Phase1VerifyIds {
    blocked_task_id: String,
    review_task_id: String,
    execution_id: String,
    agent_id: String,
}

async fn run_phase1_foundation_sample_async(
    url: String,
    verify: Option<Phase1VerifyIds>,
) -> Result<String, String> {
    if let Some(ids) = verify {
        return run_phase1_foundation_verify_async(url, ids).await;
    }

    let session_id = SessionId::new(format!("cli-phase1-foundation-{}", live_id_stamp()?));
    let token = format!("FHPHASE1{}", live_id_stamp()?);
    let review_task_id = format!("task-cli-phase1-review-{token}");
    let blocked_task_id = format!("task-cli-phase1-blocked-{token}");
    let execution_id = format!("exec-cli-phase1-{token}");
    let turn_id = TurnId::new(format!("turn-cli-phase1-{token}"));
    let mut seen = Vec::new();

    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-phase1-create-review",
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some(review_task_id.clone()),
                    title: format!("Phase1 review {token}"),
                    content: format!("Phase1 review-ready task {token}"),
                    goal: "prove review_ready execution fact".to_owned(),
                    deliverables: vec!["review-ready fact".to_owned()],
                    acceptance: vec!["TaskBoard review queue contains the task".to_owned()],
                    priority: 80,
                    target_cwd: None,
                    session_id: Some(session_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    dispatch: None,
                },
            },
        )
        .await?,
    );

    let review_board = query_task_board(&url, "cli-phase1-board-after-review-create").await?;
    let agent_id = task_agent_from_board(&review_board, &review_task_id)?;

    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-phase1-review-ready",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: format!("{execution_id}-review"),
                    task_id: review_task_id.clone(),
                    agent_id: freehand_contracts::AgentId::new(agent_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::ReviewReady {
                        summary: format!("review ready {token}"),
                        deliverables: vec!["review-ready deliverable".to_owned()],
                        evidence: vec![format!("token {token}")],
                    },
                },
            },
        )
        .await?,
    );

    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-phase1-create-blocked",
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some(blocked_task_id.clone()),
                    title: format!("Phase1 blocked {token}"),
                    content: format!("Phase1 blocked/recovering task {token}"),
                    goal: "prove running/recovering/blocked execution facts".to_owned(),
                    deliverables: vec!["blocked fact".to_owned()],
                    acceptance: vec![
                        "TaskBoard blocked and stale views contain the task".to_owned(),
                    ],
                    priority: 70,
                    target_cwd: None,
                    session_id: Some(session_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    dispatch: None,
                },
            },
        )
        .await?,
    );

    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-phase1-running",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: blocked_task_id.clone(),
                    agent_id: freehand_contracts::AgentId::new(agent_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Running {
                        phase: "phase1_running".to_owned(),
                        summary: format!("running {token}"),
                        evidence: vec![format!("execution {execution_id}")],
                    },
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-phase1-recovering",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: blocked_task_id.clone(),
                    agent_id: freehand_contracts::AgentId::new(agent_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Recovering {
                        summary: format!("recovering {token}"),
                        evidence: vec!["paired execution error returned to model".to_owned()],
                        retry_count: 1,
                    },
                },
            },
        )
        .await?,
    );

    tokio::time::sleep(Duration::from_secs(2)).await;
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-phase1-scheduler",
            UiCommand::RunSchedulerTick {
                tick: UiSchedulerTickCommand {
                    stale_after_seconds: 1,
                    soft_timeout_seconds: 1,
                    hard_timeout_seconds: 30,
                },
            },
        )
        .await?,
    );

    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-phase1-blocked",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: blocked_task_id.clone(),
                    agent_id: freehand_contracts::AgentId::new(agent_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Blocked {
                        reason: format!("blocked {token}"),
                        evidence: vec!["master-visible blocker".to_owned()],
                    },
                },
            },
        )
        .await?,
    );

    let evidence = verify_phase1_foundation_truth(
        &url,
        &Phase1VerifyIds {
            blocked_task_id: blocked_task_id.clone(),
            review_task_id: review_task_id.clone(),
            execution_id: execution_id.clone(),
            agent_id: agent_id.clone(),
        },
    )
    .await?;

    Ok(format!(
        "phase1_foundation_sample_ok url={} session={} blocked_task={} review_task={} execution={} agent={} blocked={} review_ready={} stale={} recovering_event={} lifecycle_state={} seen={}",
        url,
        session_id.as_str(),
        blocked_task_id,
        review_task_id,
        execution_id,
        agent_id,
        evidence.blocked_count,
        evidence.review_ready_count,
        evidence.stale_count,
        evidence.recovering_event_seen,
        evidence.lifecycle_state,
        seen.join(",")
    ))
}

async fn run_phase1_foundation_verify_async(
    url: String,
    ids: Phase1VerifyIds,
) -> Result<String, String> {
    let evidence = verify_phase1_foundation_truth(&url, &ids).await?;
    Ok(format!(
        "phase1_foundation_verify_ok url={} blocked_task={} review_task={} execution={} agent={} blocked={} review_ready={} stale={} recovering_event={} lifecycle_state={}",
        url,
        ids.blocked_task_id,
        ids.review_task_id,
        ids.execution_id,
        ids.agent_id,
        evidence.blocked_count,
        evidence.review_ready_count,
        evidence.stale_count,
        evidence.recovering_event_seen,
        evidence.lifecycle_state
    ))
}

#[derive(Debug, Clone)]
struct MasterWorkerVerifyIds {
    task_id: String,
    execution_id: String,
    agent_id: String,
}

#[derive(Debug, Clone)]
struct MasterWorkerFoundationEvidence {
    final_status: String,
    blocked_seen: bool,
    review_ready_seen: bool,
    history_events: Vec<String>,
    lifecycle_state: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MasterWorkerAutonomyScenario {
    Success,
    ExecutionError,
    RejectRetry,
}

impl MasterWorkerAutonomyScenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "success" => Ok(Self::Success),
            "execution-error" => Ok(Self::ExecutionError),
            "reject-retry" => Ok(Self::RejectRetry),
            _ => Err(
                "scenario must be one of: all, success, execution-error, reject-retry".to_owned(),
            ),
        }
    }

    fn all() -> [Self; 3] {
        [Self::Success, Self::ExecutionError, Self::RejectRetry]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::ExecutionError => "execution-error",
            Self::RejectRetry => "reject-retry",
        }
    }

    fn id_label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::ExecutionError => "execution-error",
            Self::RejectRetry => "reject-retry",
        }
    }

    fn expected_final_status(self) -> &'static str {
        match self {
            Self::Success | Self::RejectRetry => "closed",
            Self::ExecutionError => "blocked",
        }
    }

    fn expected_tool_executions(self) -> usize {
        match self {
            Self::Success => 7,
            Self::ExecutionError => 5,
            Self::RejectRetry => 9,
        }
    }

    fn required_ordered_events(self) -> &'static [&'static str] {
        match self {
            Self::Success => &[
                "TaskCreated",
                "TaskAssigned",
                "TaskResumed",
                "TaskExecutionRecorded",
                "TaskReviewSubmitted",
                "TaskReviewApproved",
                "TaskClosed",
            ],
            Self::ExecutionError => &[
                "TaskCreated",
                "TaskAssigned",
                "TaskResumed",
                "TaskExecutionRecorded",
                "TaskBlocked",
            ],
            Self::RejectRetry => &[
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
        }
    }

    fn forbidden_events(self) -> &'static [&'static str] {
        match self {
            Self::Success => &["TaskBlocked", "TaskReviewRejected"],
            Self::ExecutionError => &[
                "TaskReviewSubmitted",
                "TaskReviewApproved",
                "TaskClosed",
                "TaskReviewRejected",
            ],
            Self::RejectRetry => &["TaskBlocked"],
        }
    }

    fn expected_review_submissions(self) -> usize {
        match self {
            Self::Success => 1,
            Self::ExecutionError => 0,
            Self::RejectRetry => 2,
        }
    }
}

#[derive(Debug, Clone)]
struct MasterWorkerAutonomyVerifyIds {
    scenario: MasterWorkerAutonomyScenario,
    task_id: String,
    execution_id: String,
    agent_id: String,
}

#[derive(Debug, Clone)]
struct MasterWorkerAutonomyEvidence {
    final_status: String,
    lifecycle_state: String,
    history_events: Vec<String>,
    review_submissions: usize,
    transcript_turns: Option<usize>,
    transcript_tool_executions: Option<usize>,
}

async fn run_master_worker_foundation_sample_async(
    url: String,
    verify: Option<MasterWorkerVerifyIds>,
) -> Result<String, String> {
    if let Some(ids) = verify {
        let evidence = verify_master_worker_foundation_truth(&url, &ids).await?;
        return Ok(format!(
            "master_worker_foundation_verify_ok url={} task={} execution={} agent={} status={} blocked_seen={} review_ready_seen={} lifecycle_state={} events={}",
            url,
            ids.task_id,
            ids.execution_id,
            ids.agent_id,
            evidence.final_status,
            evidence.blocked_seen,
            evidence.review_ready_seen,
            evidence.lifecycle_state,
            evidence.history_events.join(",")
        ));
    }

    let session_id = SessionId::new(format!("cli-master-worker-{}", live_id_stamp()?));
    let token = format!("FHPHASE2A{}", live_id_stamp()?);
    let task_id = format!("task-cli-master-worker-{token}");
    let execution_id = format!("exec-cli-master-worker-{token}");
    let worker_id = format!("worker-cli-master-worker-{token}");
    let turn_id = TurnId::new(format!("turn-cli-master-worker-{token}"));
    let worker_agent = AgentId::new(worker_id.clone());
    let mut seen = Vec::new();

    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-create-agent",
            UiCommand::CreateTaskAgent {
                agent: UiTaskAgentCreateCommand {
                    agent_id: worker_agent.clone(),
                    capabilities: vec!["code_edit".to_owned(), "test_run".to_owned()],
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-create-task",
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some(task_id.clone()),
                    title: format!("Master worker {token}"),
                    content: format!("Phase2A master worker task {token}"),
                    goal: "prove headless master worker execution loop".to_owned(),
                    deliverables: vec!["worker execution loop".to_owned()],
                    acceptance: vec!["task closes only after approved review".to_owned()],
                    priority: 90,
                    target_cwd: None,
                    session_id: Some(session_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    dispatch: Some(UiTaskDispatchCommand::None),
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-assign",
            UiCommand::AssignTask {
                assignment: UiTaskAssignCommand {
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-claim",
            UiCommand::ClaimNextTask {
                claim: UiTaskClaimCommand {
                    agent_id: worker_agent.clone(),
                    execution_id: execution_id.clone(),
                    ttl_seconds: Some(300),
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-progress",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Running {
                        phase: "phase2a_progress".to_owned(),
                        summary: format!("worker progress {token}"),
                        evidence: vec![format!("execution {execution_id}")],
                    },
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-blocked",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Blocked {
                        reason: format!("blocked {token}"),
                        evidence: vec!["master visible blocker".to_owned()],
                    },
                },
            },
        )
        .await?,
    );
    let blocked_board = query_task_board(&url, "cli-master-worker-blocked-board").await?;
    if !blocked_board
        .blocked
        .iter()
        .any(|task| task.task_id == task_id)
    {
        return Err(format!(
            "master worker sample blocked board missing task={} blocked_count={}",
            task_id,
            blocked_board.blocked.len()
        ));
    }
    seen.push("query_blocked_board:blocked=1".to_owned());

    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-recovering",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Recovering {
                        summary: format!("recovering {token}"),
                        evidence: vec!["master unblock guidance".to_owned()],
                        retry_count: 1,
                    },
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-review-1",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::ReviewReady {
                        summary: format!("first review {token}"),
                        deliverables: vec!["first deliverable".to_owned()],
                        evidence: vec!["first review evidence".to_owned()],
                    },
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-reject",
            UiCommand::RejectTaskReview {
                rejection: UiTaskReviewRejectionCommand {
                    task_id: task_id.clone(),
                    reject_reason: format!("needs retry {token}"),
                    next_requirements: vec!["retry with corrected evidence".to_owned()],
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-retry-progress",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Running {
                        phase: "phase2a_retry".to_owned(),
                        summary: format!("retry progress {token}"),
                        evidence: vec!["retry execution evidence".to_owned()],
                    },
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-review-2",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::ReviewReady {
                        summary: format!("second review {token}"),
                        deliverables: vec!["accepted deliverable".to_owned()],
                        evidence: vec!["accepted review evidence".to_owned()],
                    },
                },
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-approve",
            UiCommand::ApproveTaskReview {
                task_id: task_id.clone(),
            },
        )
        .await?,
    );
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-worker-close",
            UiCommand::CloseTask {
                task_id: task_id.clone(),
            },
        )
        .await?,
    );

    let ids = MasterWorkerVerifyIds {
        task_id: task_id.clone(),
        execution_id: execution_id.clone(),
        agent_id: worker_id.clone(),
    };
    let evidence = verify_master_worker_foundation_truth(&url, &ids).await?;
    Ok(format!(
        "master_worker_foundation_sample_ok url={} session={} task={} execution={} agent={} status={} blocked_seen={} review_ready_seen={} lifecycle_state={} events={} seen={}",
        url,
        session_id.as_str(),
        task_id,
        execution_id,
        worker_id,
        evidence.final_status,
        evidence.blocked_seen,
        evidence.review_ready_seen,
        evidence.lifecycle_state,
        evidence.history_events.join(","),
        seen.join(",")
    ))
}

async fn verify_master_worker_foundation_truth(
    url: &str,
    ids: &MasterWorkerVerifyIds,
) -> Result<MasterWorkerFoundationEvidence, String> {
    let board = query_task_board_including_terminal(url, "cli-master-worker-verify-board").await?;
    let Some(task) = board.tasks.iter().find(|task| task.task_id == ids.task_id) else {
        return Err(format!(
            "master worker task missing from terminal board task={} count={}",
            ids.task_id,
            board.tasks.len()
        ));
    };
    if !task.status.eq_ignore_ascii_case("closed") {
        return Err(format!(
            "master worker task not closed task={} status={}",
            ids.task_id, task.status
        ));
    }
    if task.assignee_agent_id.as_ref().map(AgentId::as_str) != Some(ids.agent_id.as_str()) {
        return Err(format!(
            "master worker task assignee mismatch task={} expected_agent={} actual={}",
            ids.task_id,
            ids.agent_id,
            task.assignee_agent_id
                .as_ref()
                .map(AgentId::as_str)
                .unwrap_or("none")
        ));
    }
    if task.active_execution_id.as_deref() != Some(ids.execution_id.as_str()) {
        return Err(format!(
            "master worker task execution mismatch task={} expected_execution={} actual={}",
            ids.task_id,
            ids.execution_id,
            task.active_execution_id.as_deref().unwrap_or("none")
        ));
    }

    let agent_board = query_agent_board(url, "cli-master-worker-verify-agent-board").await?;
    if !agent_board
        .agents
        .iter()
        .any(|agent| agent.agent_id.as_str() == ids.agent_id)
    {
        return Err(format!(
            "master worker agent missing agent={} board_count={}",
            ids.agent_id,
            agent_board.agents.len()
        ));
    }
    let lifecycle = query_agent_lifecycle(
        url,
        "cli-master-worker-verify-agent-lifecycle",
        &ids.agent_id,
    )
    .await?;
    let history = query_task_history(url, &ids.task_id).await?;
    let event_types = history
        .events
        .iter()
        .map(|event| event.event_type.clone())
        .collect::<Vec<_>>();
    assert_ordered_events(
        &event_types,
        &[
            "TaskCreated",
            "TaskAssigned",
            "TaskResumed",
            "TaskExecutionRecorded",
            "TaskBlocked",
            "TaskExecutionRecovering",
            "TaskReviewSubmitted",
            "TaskReviewRejected",
            "TaskExecutionRecorded",
            "TaskReviewSubmitted",
            "TaskReviewApproved",
            "TaskClosed",
        ],
    )
    .map_err(|message| {
        format!(
            "master worker history sequence invalid task={} execution={} {message}",
            ids.task_id, ids.execution_id
        )
    })?;
    let execution_events = history
        .events
        .iter()
        .filter(|event| {
            event
                .payload
                .get("execution_id")
                .and_then(serde_json::Value::as_str)
                == Some(ids.execution_id.as_str())
        })
        .count();
    if execution_events < 5 {
        return Err(format!(
            "master worker execution evidence too weak task={} execution={} matching_events={}",
            ids.task_id, ids.execution_id, execution_events
        ));
    }
    let blocked_seen = event_types.iter().any(|event| event == "TaskBlocked");
    let review_ready_seen = event_types
        .iter()
        .filter(|event| event.as_str() == "TaskReviewSubmitted")
        .count()
        >= 2;
    Ok(MasterWorkerFoundationEvidence {
        final_status: task.status.clone(),
        blocked_seen,
        review_ready_seen,
        history_events: event_types,
        lifecycle_state: lifecycle.state,
    })
}

async fn run_master_worker_autonomy_sample_async(
    url: String,
    scenario: Option<MasterWorkerAutonomyScenario>,
    verify: Option<MasterWorkerAutonomyVerifyIds>,
) -> Result<String, String> {
    if let Some(ids) = verify {
        let evidence = verify_master_worker_autonomy_truth(&url, &ids).await?;
        return Ok(format!(
            "master_worker_autonomy_verify_ok scenario={} url={} task={} execution={} agent={} status={} lifecycle_state={} review_submissions={} events={}",
            ids.scenario.label(),
            url,
            ids.task_id,
            ids.execution_id,
            ids.agent_id,
            evidence.final_status,
            evidence.lifecycle_state,
            evidence.review_submissions,
            evidence.history_events.join(",")
        ));
    }

    let scenarios = scenario
        .map(|scenario| vec![scenario])
        .unwrap_or_else(|| MasterWorkerAutonomyScenario::all().to_vec());
    let mut lines = vec![format!(
        "master_worker_autonomy_sample_ok url={} count={}",
        url,
        scenarios.len()
    )];
    for scenario in scenarios {
        lines.push(run_master_worker_autonomy_scenario(&url, scenario).await?);
    }
    Ok(lines.join("\n"))
}

async fn run_master_worker_autonomy_scenario(
    url: &str,
    scenario: MasterWorkerAutonomyScenario,
) -> Result<String, String> {
    let stamp = live_id_stamp()?;
    let session_id = SessionId::new(format!(
        "cli-master-autonomy-{}-{}",
        scenario.id_label(),
        stamp
    ));
    let token = format!("FHAUTO{stamp}");
    let task_id = format!("task-cli-master-autonomy-{}-{token}", scenario.id_label());
    let execution_id = format!("exec-cli-master-autonomy-{}-{token}", scenario.id_label());
    let worker_id = "worker".to_owned();
    let prompt =
        master_worker_autonomy_prompt(scenario, &task_id, &worker_id, &execution_id, &token);
    let label = format!("master-autonomy-{}", scenario.id_label());
    let seen = submit_adp_sample_prompt_with_timeout(
        url,
        &session_id,
        &label,
        prompt.clone(),
        Duration::from_secs(180),
    )
    .await?;
    let transcript = query_session_transcript(
        url,
        &session_id,
        &format!("cli-{label}-transcript"),
        Duration::from_secs(20),
    )
    .await?;
    let transcript_evidence =
        master_worker_autonomy_transcript_evidence(&transcript, scenario, &prompt)?;
    let ids = MasterWorkerAutonomyVerifyIds {
        scenario,
        task_id: task_id.clone(),
        execution_id: execution_id.clone(),
        agent_id: worker_id.clone(),
    };
    let mut evidence = verify_master_worker_autonomy_truth(url, &ids).await?;
    evidence.transcript_turns = Some(transcript_evidence.0);
    evidence.transcript_tool_executions = Some(transcript_evidence.1);
    Ok(format!(
        "master_worker_autonomy_scenario_ok scenario={} session={} task={} execution={} agent={} status={} lifecycle_state={} review_submissions={} transcript_turns={} tool_executions={} events={} seen={}",
        scenario.label(),
        session_id.as_str(),
        task_id,
        execution_id,
        worker_id,
        evidence.final_status,
        evidence.lifecycle_state,
        evidence.review_submissions,
        evidence.transcript_turns.unwrap_or(0),
        evidence.transcript_tool_executions.unwrap_or(0),
        evidence.history_events.join(","),
        seen.join(",")
    ))
}

fn master_worker_autonomy_prompt(
    scenario: MasterWorkerAutonomyScenario,
    task_id: &str,
    worker_id: &str,
    execution_id: &str,
    token: &str,
) -> String {
    let scenario_instruction = match scenario {
        MasterWorkerAutonomyScenario::Success => {
            "Run the success path: create and assign a task to the configured Worker, claim it, record running progress, record review_ready, approve, close, then finish."
        }
        MasterWorkerAutonomyScenario::ExecutionError => {
            "Run the execution-error path: create and assign a task to the configured Worker, claim it, record running progress, record a blocked execution error, do not approve or close, then finish with the task blocked."
        }
        MasterWorkerAutonomyScenario::RejectRetry => {
            "Run the rejected-review path: create and assign a task to the configured Worker, claim it, record incomplete review_ready, reject it, record recovering with retry_count=1, record a second review_ready, approve, close, then finish."
        }
    };
    format!(
        "Master worker autonomy sample.\n\
         FHMA_SCENARIO={scenario}\n\
         FHMA_TOKEN={token}\n\
         FHMA_TASK_ID={task_id}\n\
         FHMA_WORKER_ID={worker_id}\n\
         FHMA_EXECUTION_ID={execution_id}\n\n\
         {scenario_instruction}\n\
         Use only the owner-scoped task tool as task(op=...). Do not ask the user. Do not create task state in prose.\n\
         The framework will validate Task Center truth, task history order, Agent Lifecycle state, and the tool-result transcript.",
        scenario = scenario.label()
    )
}

fn master_worker_autonomy_transcript_evidence(
    transcript: &freehand_ui_protocol::UiSessionTranscriptProjection,
    scenario: MasterWorkerAutonomyScenario,
    prompt: &str,
) -> Result<(usize, usize), String> {
    let mut terminal_success = false;
    let mut saw_prompt = false;
    let mut tool_executions = BTreeSet::new();
    for turn in &transcript.turns {
        if turn.user_text.as_deref() == Some(prompt) {
            saw_prompt = true;
        }
        if turn.terminal_status.as_ref() == Some(&TerminalStatus::Success) {
            terminal_success = true;
        }
        for activity in &turn.tool_activities {
            if activity.tool_name == "task" {
                tool_executions.insert(activity.tool_call_id.clone());
            }
        }
    }
    if !saw_prompt {
        return Err(format!(
            "master autonomy transcript missing submitted prompt session={} turns={}",
            transcript.session_id.as_str(),
            transcript.turns.len()
        ));
    }
    if !terminal_success {
        return Err(format!(
            "master autonomy transcript missing terminal success session={} turns={}",
            transcript.session_id.as_str(),
            transcript.turns.len()
        ));
    }
    if tool_executions.len() < scenario.expected_tool_executions() {
        return Err(format!(
            "master autonomy transcript has too few task tool executions scenario={} expected_at_least={} actual={}",
            scenario.label(),
            scenario.expected_tool_executions(),
            tool_executions.len()
        ));
    }
    Ok((transcript.turns.len(), tool_executions.len()))
}

async fn verify_master_worker_autonomy_truth(
    url: &str,
    ids: &MasterWorkerAutonomyVerifyIds,
) -> Result<MasterWorkerAutonomyEvidence, String> {
    let board = query_task_board_including_terminal(
        url,
        &format!(
            "cli-master-autonomy-{}-verify-board",
            ids.scenario.id_label()
        ),
    )
    .await?;
    let Some(task) = board.tasks.iter().find(|task| task.task_id == ids.task_id) else {
        return Err(format!(
            "master autonomy task missing scenario={} task={} count={}",
            ids.scenario.label(),
            ids.task_id,
            board.tasks.len()
        ));
    };
    if !task
        .status
        .eq_ignore_ascii_case(ids.scenario.expected_final_status())
    {
        return Err(format!(
            "master autonomy task status mismatch scenario={} task={} expected={} actual={}",
            ids.scenario.label(),
            ids.task_id,
            ids.scenario.expected_final_status(),
            task.status
        ));
    }
    if task.assignee_agent_id.as_ref().map(AgentId::as_str) != Some(ids.agent_id.as_str()) {
        return Err(format!(
            "master autonomy task assignee mismatch scenario={} task={} expected_agent={} actual={}",
            ids.scenario.label(),
            ids.task_id,
            ids.agent_id,
            task.assignee_agent_id
                .as_ref()
                .map(AgentId::as_str)
                .unwrap_or("none")
        ));
    }
    if task.active_execution_id.as_deref() != Some(ids.execution_id.as_str()) {
        return Err(format!(
            "master autonomy task execution mismatch scenario={} task={} expected_execution={} actual={}",
            ids.scenario.label(),
            ids.task_id,
            ids.execution_id,
            task.active_execution_id.as_deref().unwrap_or("none")
        ));
    }

    let agent_board = query_agent_board(
        url,
        &format!(
            "cli-master-autonomy-{}-verify-agent-board",
            ids.scenario.id_label()
        ),
    )
    .await?;
    if !agent_board
        .agents
        .iter()
        .any(|agent| agent.agent_id.as_str() == ids.agent_id)
    {
        return Err(format!(
            "master autonomy agent missing scenario={} agent={} board_count={}",
            ids.scenario.label(),
            ids.agent_id,
            agent_board.agents.len()
        ));
    }
    let lifecycle = query_agent_lifecycle(
        url,
        &format!(
            "cli-master-autonomy-{}-verify-lifecycle",
            ids.scenario.id_label()
        ),
        &ids.agent_id,
    )
    .await?;

    let history = query_task_history(url, &ids.task_id).await?;
    let event_types = history
        .events
        .iter()
        .map(|event| event.event_type.clone())
        .collect::<Vec<_>>();
    assert_ordered_events(&event_types, ids.scenario.required_ordered_events()).map_err(
        |message| {
            format!(
                "master autonomy history sequence invalid scenario={} task={} execution={} {message}",
                ids.scenario.label(),
                ids.task_id,
                ids.execution_id
            )
        },
    )?;
    for forbidden in ids.scenario.forbidden_events() {
        if event_types.iter().any(|event| event == forbidden) {
            return Err(format!(
                "master autonomy forbidden event scenario={} task={} forbidden={} events={}",
                ids.scenario.label(),
                ids.task_id,
                forbidden,
                event_types.join(",")
            ));
        }
    }
    let review_submissions = event_types
        .iter()
        .filter(|event| event.as_str() == "TaskReviewSubmitted")
        .count();
    if review_submissions != ids.scenario.expected_review_submissions() {
        return Err(format!(
            "master autonomy review count mismatch scenario={} task={} expected={} actual={} events={}",
            ids.scenario.label(),
            ids.task_id,
            ids.scenario.expected_review_submissions(),
            review_submissions,
            event_types.join(",")
        ));
    }
    let execution_events = history
        .events
        .iter()
        .filter(|event| {
            event
                .payload
                .get("execution_id")
                .and_then(serde_json::Value::as_str)
                == Some(ids.execution_id.as_str())
        })
        .count();
    if execution_events < 3 {
        return Err(format!(
            "master autonomy execution evidence too weak scenario={} task={} execution={} matching_events={}",
            ids.scenario.label(),
            ids.task_id,
            ids.execution_id,
            execution_events
        ));
    }
    Ok(MasterWorkerAutonomyEvidence {
        final_status: task.status.clone(),
        lifecycle_state: lifecycle.state,
        history_events: event_types,
        review_submissions,
        transcript_turns: None,
        transcript_tool_executions: None,
    })
}

#[derive(Debug, Clone)]
struct MasterPollVerifyIds {
    task_id: String,
    execution_id: String,
    agent_id: String,
    cursor: String,
}

#[derive(Debug, Clone)]
struct MasterPollFoundationEvidence {
    task_status: String,
    inbox_after_cursor_events: usize,
    poll_events: usize,
    classifications: Vec<String>,
    source_cursor: Option<String>,
    persisted_cursor: String,
}

async fn run_master_poll_foundation_sample_async(
    url: String,
    verify: Option<MasterPollVerifyIds>,
) -> Result<String, String> {
    if let Some(ids) = verify {
        let evidence = verify_master_poll_foundation_truth(&url, &ids).await?;
        return Ok(format!(
            "master_poll_foundation_verify_ok url={} task={} execution={} agent={} cursor={} status={} inbox_after_cursor_events={} poll_events={} source_cursor={} persisted_cursor={} classifications={}",
            url,
            ids.task_id,
            ids.execution_id,
            ids.agent_id,
            ids.cursor,
            evidence.task_status,
            evidence.inbox_after_cursor_events,
            evidence.poll_events,
            evidence.source_cursor.as_deref().unwrap_or("none"),
            evidence.persisted_cursor,
            evidence.classifications.join(",")
        ));
    }

    let session_id = SessionId::new(format!("cli-master-poll-{}", live_id_stamp()?));
    let token = format!("FHPHASE2B{}", live_id_stamp()?);
    let task_id = format!("task-cli-master-poll-{token}");
    let execution_id = format!("exec-cli-master-poll-{token}");
    let worker_id = format!("worker-cli-master-poll-{token}");
    let turn_id = TurnId::new(format!("turn-cli-master-poll-{token}"));
    let worker_agent = AgentId::new(worker_id.clone());
    let mut seen = Vec::new();

    for (request_id, command) in [
        (
            "cli-master-poll-create-agent",
            UiCommand::CreateTaskAgent {
                agent: UiTaskAgentCreateCommand {
                    agent_id: worker_agent.clone(),
                    capabilities: vec!["code_edit".to_owned(), "test_run".to_owned()],
                },
            },
        ),
        (
            "cli-master-poll-create-task",
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some(task_id.clone()),
                    title: format!("Master poll {token}"),
                    content: format!("Phase2B master poll task {token}"),
                    goal: "prove master poll consumes task truth without business mutation"
                        .to_owned(),
                    deliverables: vec!["event inbox".to_owned(), "master poll".to_owned()],
                    acceptance: vec![
                        "poll persists cursor and leaves task state unchanged".to_owned(),
                    ],
                    priority: 96,
                    target_cwd: None,
                    session_id: Some(session_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    dispatch: Some(UiTaskDispatchCommand::None),
                },
            },
        ),
        (
            "cli-master-poll-assign",
            UiCommand::AssignTask {
                assignment: UiTaskAssignCommand {
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                },
            },
        ),
        (
            "cli-master-poll-claim",
            UiCommand::ClaimNextTask {
                claim: UiTaskClaimCommand {
                    agent_id: worker_agent.clone(),
                    execution_id: execution_id.clone(),
                    ttl_seconds: Some(300),
                },
            },
        ),
        (
            "cli-master-poll-running",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Running {
                        phase: "phase2b_running".to_owned(),
                        summary: format!("worker running {token}"),
                        evidence: vec![format!("execution {execution_id}")],
                    },
                },
            },
        ),
    ] {
        seen.push(send_adp_command_receipt(&url, request_id, command).await?);
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    seen.push(
        send_adp_command_receipt(
            &url,
            "cli-master-poll-scheduler",
            UiCommand::RunSchedulerTick {
                tick: UiSchedulerTickCommand {
                    stale_after_seconds: 1,
                    soft_timeout_seconds: 10,
                    hard_timeout_seconds: 30,
                },
            },
        )
        .await?,
    );

    for (request_id, command) in [
        (
            "cli-master-poll-blocked",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Blocked {
                        reason: format!("blocked {token}"),
                        evidence: vec!["master visible blocker".to_owned()],
                    },
                },
            },
        ),
        (
            "cli-master-poll-recovering",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Recovering {
                        summary: format!("recovering {token}"),
                        evidence: vec!["master unblock guidance".to_owned()],
                        retry_count: 1,
                    },
                },
            },
        ),
        (
            "cli-master-poll-review",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id),
                    kind: UiExecutionFactKind::ReviewReady {
                        summary: format!("review ready {token}"),
                        deliverables: vec!["poll deliverable".to_owned()],
                        evidence: vec!["review evidence".to_owned()],
                    },
                },
            },
        ),
    ] {
        seen.push(send_adp_command_receipt(&url, request_id, command).await?);
    }

    let before_poll_board =
        query_task_board_including_terminal(&url, "cli-master-poll-before-board").await?;
    let before_status = before_poll_board
        .tasks
        .iter()
        .find(|task| task.task_id == task_id)
        .map(|task| task.status.clone())
        .ok_or_else(|| format!("master poll task missing before poll task={task_id}"))?;

    let inbox = query_event_inbox(&url, "cli-master-poll-inbox", None, None).await?;
    let inbox_kinds = inbox
        .events
        .iter()
        .map(|event| event.kind.clone())
        .collect::<Vec<_>>();
    require_kind(&inbox_kinds, "execution_blocked", "master poll event inbox")?;
    require_kind(&inbox_kinds, "review_ready", "master poll event inbox")?;
    require_kind(&inbox_kinds, "scheduler_tick", "master poll event inbox")?;

    let poll = query_master_poll(&url, "cli-master-poll-query", None, None, true, true).await?;
    let poll_cursor = poll
        .persisted_cursor
        .clone()
        .or_else(|| poll.next_cursor.clone())
        .ok_or_else(|| "master poll missing persisted cursor".to_owned())?;
    let classification_kinds = master_poll_classification_kinds(&poll);
    require_kind(
        &classification_kinds,
        "blocked",
        "master poll classifications",
    )?;
    require_kind(
        &classification_kinds,
        "review_ready",
        "master poll classifications",
    )?;
    require_kind(
        &classification_kinds,
        "stale",
        "master poll classifications",
    )?;
    if poll.next_cursor.as_deref() != Some(poll_cursor.as_str()) {
        return Err(format!(
            "master poll cursor mismatch next={} persisted={}",
            poll.next_cursor.as_deref().unwrap_or("none"),
            poll_cursor
        ));
    }

    let poll_receipt = send_adp_command_receipt(
        &url,
        "cli-master-poll-command",
        UiCommand::RunMasterPoll {
            after_cursor: None,
            limit: None,
            include_terminal: true,
            replay_from_start: true,
        },
    )
    .await?;
    seen.push(poll_receipt);
    let final_poll = query_master_poll(
        &url,
        "cli-master-poll-final-cursor",
        None,
        None,
        true,
        false,
    )
    .await?;
    let cursor = final_poll
        .persisted_cursor
        .clone()
        .or_else(|| final_poll.source_cursor.clone())
        .ok_or_else(|| "master poll final cursor missing persisted cursor".to_owned())?;
    if final_poll.source_cursor.as_deref() != Some(cursor.as_str()) {
        return Err(format!(
            "master poll final cursor source mismatch expected={} actual={}",
            cursor,
            final_poll.source_cursor.as_deref().unwrap_or("none")
        ));
    }
    if !final_poll.event_inbox.events.is_empty() {
        return Err(format!(
            "master poll final cursor expected drained event inbox cursor={} count={}",
            cursor,
            final_poll.event_inbox.events.len()
        ));
    }

    let after_poll_board =
        query_task_board_including_terminal(&url, "cli-master-poll-after-board").await?;
    let after_status = after_poll_board
        .tasks
        .iter()
        .find(|task| task.task_id == task_id)
        .map(|task| task.status.clone())
        .ok_or_else(|| format!("master poll task missing after poll task={task_id}"))?;
    if after_status != before_status {
        return Err(format!(
            "master poll mutated task status task={} before={} after={}",
            task_id, before_status, after_status
        ));
    }

    let ids = MasterPollVerifyIds {
        task_id: task_id.clone(),
        execution_id: execution_id.clone(),
        agent_id: worker_id.clone(),
        cursor: cursor.clone(),
    };
    let evidence = verify_master_poll_foundation_truth(&url, &ids).await?;
    Ok(format!(
        "master_poll_foundation_sample_ok url={} session={} task={} execution={} agent={} cursor={} status={} inbox_events={} poll_events={} source_cursor={} persisted_cursor={} classifications={} seen={}",
        url,
        session_id.as_str(),
        task_id,
        execution_id,
        worker_id,
        cursor,
        evidence.task_status,
        inbox.events.len(),
        evidence.poll_events,
        evidence.source_cursor.as_deref().unwrap_or("none"),
        evidence.persisted_cursor,
        evidence.classifications.join(","),
        seen.join(",")
    ))
}

async fn verify_master_poll_foundation_truth(
    url: &str,
    ids: &MasterPollVerifyIds,
) -> Result<MasterPollFoundationEvidence, String> {
    let board = query_task_board_including_terminal(url, "cli-master-poll-verify-board").await?;
    let Some(task) = board.tasks.iter().find(|task| task.task_id == ids.task_id) else {
        return Err(format!(
            "master poll task missing from board task={} count={}",
            ids.task_id,
            board.tasks.len()
        ));
    };
    if !task.status.eq_ignore_ascii_case("review_submitted") {
        return Err(format!(
            "master poll task expected review_submitted task={} status={}",
            ids.task_id, task.status
        ));
    }
    if task.assignee_agent_id.as_ref().map(AgentId::as_str) != Some(ids.agent_id.as_str()) {
        return Err(format!(
            "master poll assignee mismatch task={} expected_agent={} actual={}",
            ids.task_id,
            ids.agent_id,
            task.assignee_agent_id
                .as_ref()
                .map(AgentId::as_str)
                .unwrap_or("none")
        ));
    }
    if task.active_execution_id.as_deref() != Some(ids.execution_id.as_str()) {
        return Err(format!(
            "master poll execution mismatch task={} expected_execution={} actual={}",
            ids.task_id,
            ids.execution_id,
            task.active_execution_id.as_deref().unwrap_or("none")
        ));
    }

    let history = query_task_history(url, &ids.task_id).await?;
    let event_types = history
        .events
        .iter()
        .map(|event| event.event_type.clone())
        .collect::<Vec<_>>();
    assert_ordered_events(
        &event_types,
        &[
            "TaskCreated",
            "TaskAssigned",
            "TaskResumed",
            "TaskExecutionRecorded",
            "TaskSchedulerTick",
            "TaskBlocked",
            "TaskExecutionRecovering",
            "TaskReviewSubmitted",
        ],
    )
    .map_err(|message| {
        format!(
            "master poll history sequence invalid task={} execution={} {message}",
            ids.task_id, ids.execution_id
        )
    })?;
    let execution_events = history
        .events
        .iter()
        .filter(|event| {
            event
                .payload
                .get("execution_id")
                .and_then(serde_json::Value::as_str)
                == Some(ids.execution_id.as_str())
        })
        .count();
    if execution_events < 4 {
        return Err(format!(
            "master poll execution evidence too weak task={} execution={} matching_events={}",
            ids.task_id, ids.execution_id, execution_events
        ));
    }

    let inbox_after_cursor = query_event_inbox(
        url,
        "cli-master-poll-verify-inbox",
        Some(ids.cursor.clone()),
        None,
    )
    .await?;
    if !inbox_after_cursor.events.is_empty() {
        return Err(format!(
            "master poll cursor replay returned new events cursor={} count={}",
            ids.cursor,
            inbox_after_cursor.events.len()
        ));
    }

    let poll =
        query_master_poll(url, "cli-master-poll-verify-poll", None, None, true, false).await?;
    if poll.source_cursor.as_deref() != Some(ids.cursor.as_str()) {
        return Err(format!(
            "master poll verify source cursor mismatch expected={} actual={}",
            ids.cursor,
            poll.source_cursor.as_deref().unwrap_or("none")
        ));
    }
    if poll.persisted_cursor.as_deref() != Some(ids.cursor.as_str()) {
        return Err(format!(
            "master poll verify persisted cursor mismatch expected={} actual={}",
            ids.cursor,
            poll.persisted_cursor.as_deref().unwrap_or("none")
        ));
    }
    if !poll.event_inbox.events.is_empty() {
        return Err(format!(
            "master poll verify expected empty event inbox after cursor cursor={} count={}",
            ids.cursor,
            poll.event_inbox.events.len()
        ));
    }
    let classifications = master_poll_classification_kinds(&poll);
    require_kind(
        &classifications,
        "review_ready",
        "master poll verify classifications",
    )?;

    Ok(MasterPollFoundationEvidence {
        task_status: task.status.clone(),
        inbox_after_cursor_events: inbox_after_cursor.events.len(),
        poll_events: poll.event_inbox.events.len(),
        classifications,
        source_cursor: poll.source_cursor,
        persisted_cursor: poll
            .persisted_cursor
            .ok_or_else(|| "master poll verify missing persisted cursor".to_owned())?,
    })
}

#[derive(Debug, Clone)]
struct WorkerControlVerifyIds {
    task_id: String,
    execution_id: String,
    agent_id: String,
    control_id: String,
}

#[derive(Debug, Clone)]
struct WorkerControlFoundationEvidence {
    task_status: String,
    control_count: usize,
    event_statuses: Vec<String>,
    task_history_events: Vec<String>,
}

async fn run_worker_control_foundation_sample_async(
    url: String,
    verify: Option<WorkerControlVerifyIds>,
) -> Result<String, String> {
    if let Some(ids) = verify {
        let evidence = verify_worker_control_foundation_truth(&url, &ids).await?;
        return Ok(format!(
            "worker_control_foundation_verify_ok url={} task={} execution={} agent={} control={} status={} control_events={} event_statuses={} task_events={}",
            url,
            ids.task_id,
            ids.execution_id,
            ids.agent_id,
            ids.control_id,
            evidence.task_status,
            evidence.control_count,
            evidence.event_statuses.join(","),
            evidence.task_history_events.join(",")
        ));
    }

    let session_id = SessionId::new(format!("cli-worker-control-{}", live_id_stamp()?));
    let token = format!("FHPHASE2C{}", live_id_stamp()?);
    let task_id = format!("task-cli-worker-control-{token}");
    let execution_id = format!("exec-cli-worker-control-{token}");
    let worker_id = format!("worker-cli-worker-control-{token}");
    let turn_id = TurnId::new(format!("turn-cli-worker-control-{token}"));
    let worker_agent = AgentId::new(worker_id.clone());
    let cancel_control_id = format!("wctl-cli-worker-control-cancel-{token}");
    let mut seen = Vec::new();

    for (request_id, command) in [
        (
            "cli-worker-control-create-agent",
            UiCommand::CreateTaskAgent {
                agent: UiTaskAgentCreateCommand {
                    agent_id: worker_agent.clone(),
                    capabilities: vec!["code_edit".to_owned(), "test_run".to_owned()],
                },
            },
        ),
        (
            "cli-worker-control-create-task",
            UiCommand::CreateTask {
                task: UiTaskCreateCommand {
                    task_id: Some(task_id.clone()),
                    title: format!("Worker control {token}"),
                    content: format!("Phase2C worker control task {token}"),
                    goal: "prove safe-point runtime control channel".to_owned(),
                    deliverables: vec!["worker control ledger".to_owned()],
                    acceptance: vec!["control events survive restart".to_owned()],
                    priority: 98,
                    target_cwd: None,
                    session_id: Some(session_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    dispatch: Some(UiTaskDispatchCommand::None),
                },
            },
        ),
        (
            "cli-worker-control-assign",
            UiCommand::AssignTask {
                assignment: UiTaskAssignCommand {
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                },
            },
        ),
        (
            "cli-worker-control-claim",
            UiCommand::ClaimNextTask {
                claim: UiTaskClaimCommand {
                    agent_id: worker_agent.clone(),
                    execution_id: execution_id.clone(),
                    ttl_seconds: Some(300),
                },
            },
        ),
        (
            "cli-worker-control-running",
            UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    agent_id: worker_agent.clone(),
                    turn_id: Some(turn_id),
                    kind: UiExecutionFactKind::Running {
                        phase: "phase2c_running".to_owned(),
                        summary: format!("worker running {token}"),
                        evidence: vec![format!("execution {execution_id}")],
                    },
                },
            },
        ),
    ] {
        seen.push(send_adp_command_receipt(&url, request_id, command).await?);
    }

    let controls = [
        (
            "cli-worker-control-query",
            format!("wctl-cli-worker-control-query-{token}"),
            "query_status",
            None,
            None,
        ),
        (
            "cli-worker-control-ask",
            format!("wctl-cli-worker-control-ask-{token}"),
            "ask_at_safe_point",
            Some("report the current blocker at the next safe point".to_owned()),
            None,
        ),
        (
            "cli-worker-control-constraint",
            format!("wctl-cli-worker-control-constraint-{token}"),
            "add_constraint",
            None,
            Some("keep evidence compact and durable".to_owned()),
        ),
        (
            "cli-worker-control-checkpoint",
            format!("wctl-cli-worker-control-checkpoint-{token}"),
            "request_checkpoint",
            None,
            None,
        ),
        (
            "cli-worker-control-submit-now",
            format!("wctl-cli-worker-control-submit-{token}"),
            "request_submission_now",
            None,
            None,
        ),
        (
            "cli-worker-control-pause",
            format!("wctl-cli-worker-control-pause-{token}"),
            "pause",
            None,
            None,
        ),
        (
            "cli-worker-control-resume",
            format!("wctl-cli-worker-control-resume-{token}"),
            "resume",
            None,
            None,
        ),
        (
            "cli-worker-control-cancel",
            cancel_control_id.clone(),
            "cancel",
            None,
            None,
        ),
    ];

    for (request_id, control_id, op, question, constraint) in controls {
        seen.push(
            send_adp_command_receipt(
                &url,
                request_id,
                UiCommand::WorkerControl {
                    control: UiWorkerControlCommand {
                        control_id: Some(control_id),
                        task_id: task_id.clone(),
                        execution_id: execution_id.clone(),
                        agent_id: worker_agent.clone(),
                        op: op.to_owned(),
                        question,
                        constraint,
                        note: Some(format!("worker control proof {token}")),
                    },
                },
            )
            .await?,
        );
    }

    let ids = WorkerControlVerifyIds {
        task_id: task_id.clone(),
        execution_id: execution_id.clone(),
        agent_id: worker_id.clone(),
        control_id: cancel_control_id.clone(),
    };
    let evidence = verify_worker_control_foundation_truth(&url, &ids).await?;
    Ok(format!(
        "worker_control_foundation_sample_ok url={} session={} task={} execution={} agent={} control={} status={} control_events={} event_statuses={} task_events={} seen={}",
        url,
        session_id.as_str(),
        task_id,
        execution_id,
        worker_id,
        cancel_control_id,
        evidence.task_status,
        evidence.control_count,
        evidence.event_statuses.join(","),
        evidence.task_history_events.join(","),
        seen.join(",")
    ))
}

async fn verify_worker_control_foundation_truth(
    url: &str,
    ids: &WorkerControlVerifyIds,
) -> Result<WorkerControlFoundationEvidence, String> {
    let board = query_task_board_including_terminal(url, "cli-worker-control-verify-board").await?;
    let Some(task) = board.tasks.iter().find(|task| task.task_id == ids.task_id) else {
        return Err(format!(
            "worker control task missing from board task={} count={}",
            ids.task_id,
            board.tasks.len()
        ));
    };
    if !task.status.eq_ignore_ascii_case("cancelled") {
        return Err(format!(
            "worker control task expected cancelled task={} status={}",
            ids.task_id, task.status
        ));
    }
    if task.assignee_agent_id.as_ref().map(AgentId::as_str) != Some(ids.agent_id.as_str()) {
        return Err(format!(
            "worker control assignee mismatch task={} expected_agent={} actual={}",
            ids.task_id,
            ids.agent_id,
            task.assignee_agent_id
                .as_ref()
                .map(AgentId::as_str)
                .unwrap_or("none")
        ));
    }
    if task.active_execution_id.as_deref() != Some(ids.execution_id.as_str()) {
        return Err(format!(
            "worker control execution mismatch task={} expected_execution={} actual={}",
            ids.task_id,
            ids.execution_id,
            task.active_execution_id.as_deref().unwrap_or("none")
        ));
    }

    let control = query_worker_control(&ids.task_id, &ids.execution_id, url).await?;
    if !control.events.iter().any(|event| {
        event.control_id == ids.control_id
            && event.execution_id == ids.execution_id
            && event.agent_id.as_str() == ids.agent_id
    }) {
        return Err(format!(
            "worker control event missing task={} execution={} agent={} control={} events={}",
            ids.task_id,
            ids.execution_id,
            ids.agent_id,
            ids.control_id,
            control
                .events
                .iter()
                .map(|event| format!("{}:{}:{}", event.control_id, event.op, event.status))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    let event_ops = control
        .events
        .iter()
        .map(|event| event.op.clone())
        .collect::<Vec<_>>();
    for required in [
        "query_status",
        "ask_at_safe_point",
        "add_constraint",
        "request_checkpoint",
        "request_submission_now",
        "pause",
        "resume",
        "cancel",
    ] {
        require_kind(&event_ops, required, "worker control events")?;
    }

    let history = query_task_history(url, &ids.task_id).await?;
    let task_events = history
        .events
        .iter()
        .map(|event| event.event_type.clone())
        .collect::<Vec<_>>();
    assert_ordered_events(
        &task_events,
        &["TaskPaused", "TaskResumed", "TaskCancelled"],
    )
    .map_err(|message| {
        format!(
            "worker control task consequence sequence invalid task={} execution={} {message}",
            ids.task_id, ids.execution_id
        )
    })?;

    Ok(WorkerControlFoundationEvidence {
        task_status: task.status.clone(),
        control_count: control.events.len(),
        event_statuses: control
            .events
            .iter()
            .map(|event| format!("{}:{}", event.op, event.status))
            .collect(),
        task_history_events: task_events,
    })
}

fn assert_ordered_events(actual: &[String], required: &[&str]) -> Result<(), String> {
    let mut cursor = 0_usize;
    for required_event in required {
        let Some(offset) = actual[cursor..]
            .iter()
            .position(|event| event == required_event)
        else {
            return Err(format!(
                "missing required_event={} actual={}",
                required_event,
                actual.join(",")
            ));
        };
        cursor += offset + 1;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct Phase1FoundationEvidence {
    blocked_count: usize,
    review_ready_count: usize,
    stale_count: usize,
    recovering_event_seen: bool,
    lifecycle_state: String,
}

async fn verify_phase1_foundation_truth(
    url: &str,
    ids: &Phase1VerifyIds,
) -> Result<Phase1FoundationEvidence, String> {
    let board = query_task_board(url, "cli-phase1-verify-board").await?;
    let blocked_count = board
        .blocked
        .iter()
        .filter(|task| task.task_id == ids.blocked_task_id)
        .count();
    let review_ready_count = board
        .review_ready
        .iter()
        .filter(|task| task.task_id == ids.review_task_id)
        .count();
    let stale_count = board
        .stale
        .iter()
        .filter(|task| task.task_id == ids.blocked_task_id)
        .count();
    if blocked_count == 0 {
        return Err(format!(
            "phase1 foundation blocked task missing task={} blocked_count={}",
            ids.blocked_task_id,
            board.blocked.len()
        ));
    }
    if review_ready_count == 0 {
        return Err(format!(
            "phase1 foundation review task missing task={} review_ready_count={}",
            ids.review_task_id,
            board.review_ready.len()
        ));
    }
    if stale_count == 0 {
        return Err(format!(
            "phase1 foundation stale fact missing task={} stale_count={}",
            ids.blocked_task_id,
            board.stale.len()
        ));
    }
    let agent_board = query_agent_board(url, "cli-phase1-verify-agent-board").await?;
    if !agent_board
        .agents
        .iter()
        .any(|agent| agent.agent_id.as_str() == ids.agent_id)
    {
        return Err(format!(
            "phase1 foundation agent missing agent={} board_count={}",
            ids.agent_id,
            agent_board.agents.len()
        ));
    }
    let lifecycle =
        query_agent_lifecycle(url, "cli-phase1-verify-agent-lifecycle", &ids.agent_id).await?;
    let history = query_task_history(url, &ids.blocked_task_id).await?;
    let recovering_event_seen = history.events.iter().any(|event| {
        event.event_type == "TaskExecutionRecovering"
            && event
                .payload
                .get("execution_id")
                .and_then(serde_json::Value::as_str)
                == Some(ids.execution_id.as_str())
    });
    if !recovering_event_seen {
        return Err(format!(
            "phase1 foundation recovering event missing task={} execution={} events={}",
            ids.blocked_task_id,
            ids.execution_id,
            history
                .events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    Ok(Phase1FoundationEvidence {
        blocked_count,
        review_ready_count,
        stale_count,
        recovering_event_seen,
        lifecycle_state: lifecycle.state,
    })
}

async fn send_adp_command_receipt(
    url: &str,
    request_id: &str,
    command: UiCommand,
) -> Result<String, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(url))
        .await
        .map_err(|_| format!("ADP command connect timeout: {url}"))?
        .map_err(|err| format!("ADP command connect failed: {err}"))?;
    send_adp(
        &mut socket,
        UiAdpRequest::Command {
            request_id: request_id.to_owned(),
            command,
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err(format!(
                "ADP command receipt timeout request_id={request_id}"
            ));
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| format!("ADP command receipt timeout request_id={request_id}"))??;
        match response {
            UiAdpResponse::CommandReceipt {
                request_id: response_id,
                receipt,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Ok(format!("{response_id}:{}", receipt.dispatch_status));
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP command failure request_id={} code={} message={}",
                    response_id, failure.code, failure.message
                ));
            }
            _ => {}
        }
    }
}

async fn query_task_list_find_token(
    url: &str,
    token: &str,
) -> Result<freehand_ui_protocol::UiTaskSnapshotProjection, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(url))
        .await
        .map_err(|_| format!("ADP task list connect timeout: {url}"))?
        .map_err(|err| format!("ADP task list connect failed: {err}"))?;
    let request_id = "cli-task-lifecycle-list".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: request_id.clone(),
            query: UiCommand::QueryTaskList {
                status: None,
                agent_id: None,
            },
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("ADP task list timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP task list timeout".to_owned())??;
        match response {
            UiAdpResponse::QueryResult {
                request_id: response_id,
                result,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                let freehand_ui_protocol::UiQueryResult::TaskList(list) = result else {
                    return Err("ADP task lifecycle list returned non-task result".to_owned());
                };
                let task_count = list.tasks.len();
                return list
                    .tasks
                    .into_iter()
                    .find(|task| task.title.contains(token) || task.goal.contains(token))
                    .ok_or_else(|| {
                        format!(
                            "task lifecycle sample task not found token={} count={}",
                            token, task_count
                        )
                    });
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP task lifecycle list failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }
}

async fn query_task_history(
    url: &str,
    task_id: &str,
) -> Result<freehand_ui_protocol::UiTaskHistoryProjection, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(url))
        .await
        .map_err(|_| format!("ADP task history connect timeout: {url}"))?
        .map_err(|err| format!("ADP task history connect failed: {err}"))?;
    let request_id = "cli-task-lifecycle-history".to_owned();
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: request_id.clone(),
            query: UiCommand::QueryTaskHistory {
                task_id: task_id.to_owned(),
            },
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("ADP task history timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP task history timeout".to_owned())??;
        match response {
            UiAdpResponse::QueryResult {
                request_id: response_id,
                result,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                let freehand_ui_protocol::UiQueryResult::TaskHistory(history) = result else {
                    return Err("ADP task lifecycle history returned non-task result".to_owned());
                };
                return Ok(history);
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP task lifecycle history failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }
}

async fn query_task_board(url: &str, request_id: &str) -> Result<UiTaskBoardProjection, String> {
    query_task_board_with_terminal(url, request_id, false).await
}

async fn query_task_board_including_terminal(
    url: &str,
    request_id: &str,
) -> Result<UiTaskBoardProjection, String> {
    query_task_board_with_terminal(url, request_id, true).await
}

async fn query_task_board_with_terminal(
    url: &str,
    request_id: &str,
    include_terminal: bool,
) -> Result<UiTaskBoardProjection, String> {
    let result = query_adp_once(
        url,
        request_id,
        UiCommand::QueryTaskBoard {
            status: None,
            agent_id: None,
            include_terminal,
        },
        "task board",
    )
    .await?;
    let UiQueryResult::TaskBoard(board) = result else {
        return Err("ADP task board query returned non-task-board result".to_owned());
    };
    Ok(board)
}

async fn query_event_inbox(
    url: &str,
    request_id: &str,
    after_cursor: Option<String>,
    limit: Option<usize>,
) -> Result<UiTaskEventInboxProjection, String> {
    let result = query_adp_once(
        url,
        request_id,
        UiCommand::QueryEventInbox {
            after_cursor,
            limit,
        },
        "event inbox",
    )
    .await?;
    let UiQueryResult::EventInbox(inbox) = result else {
        return Err("ADP event inbox query returned non-event-inbox result".to_owned());
    };
    Ok(inbox)
}

async fn query_master_poll(
    url: &str,
    request_id: &str,
    after_cursor: Option<String>,
    limit: Option<usize>,
    include_terminal: bool,
    replay_from_start: bool,
) -> Result<UiMasterPollProjection, String> {
    let result = query_adp_once(
        url,
        request_id,
        UiCommand::RunMasterPoll {
            after_cursor,
            limit,
            include_terminal,
            replay_from_start,
        },
        "master poll",
    )
    .await?;
    let UiQueryResult::MasterPoll(poll) = result else {
        return Err("ADP master poll query returned non-master-poll result".to_owned());
    };
    Ok(poll)
}

async fn query_agent_board(url: &str, request_id: &str) -> Result<UiAgentBoardProjection, String> {
    let result = query_adp_once(url, request_id, UiCommand::QueryAgentBoard, "agent board").await?;
    let UiQueryResult::AgentBoard(board) = result else {
        return Err("ADP agent board query returned non-agent-board result".to_owned());
    };
    Ok(board)
}

async fn query_agent_lifecycle(
    url: &str,
    request_id: &str,
    agent_id: &str,
) -> Result<UiAgentLifecycleProjection, String> {
    let result = query_adp_once(
        url,
        request_id,
        UiCommand::QueryAgentLifecycle {
            agent_id: freehand_contracts::AgentId::new(agent_id.to_owned()),
        },
        "agent lifecycle",
    )
    .await?;
    let UiQueryResult::AgentLifecycle(lifecycle) = result else {
        return Err("ADP agent lifecycle query returned non-agent-lifecycle result".to_owned());
    };
    Ok(lifecycle)
}

async fn query_worker_control(
    task_id: &str,
    execution_id: &str,
    url: &str,
) -> Result<UiWorkerControlProjection, String> {
    let result = query_adp_once(
        url,
        "cli-worker-control-query-events",
        UiCommand::QueryWorkerControl {
            task_id: task_id.to_owned(),
            execution_id: execution_id.to_owned(),
        },
        "worker control",
    )
    .await?;
    let UiQueryResult::WorkerControl(control) = result else {
        return Err("ADP worker control query returned non-worker-control result".to_owned());
    };
    Ok(*control)
}

fn master_poll_classification_kinds(poll: &UiMasterPollProjection) -> Vec<String> {
    poll.classifications
        .iter()
        .map(|classification| classification.kind.clone())
        .collect()
}

fn require_kind(kinds: &[String], required: &str, label: &str) -> Result<(), String> {
    if kinds.iter().any(|kind| kind == required) {
        return Ok(());
    }
    Err(format!(
        "{label} missing kind={required} kinds={}",
        kinds.join(",")
    ))
}

async fn query_adp_once(
    url: &str,
    request_id: &str,
    query: UiCommand,
    label: &str,
) -> Result<UiQueryResult, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(url))
        .await
        .map_err(|_| format!("ADP {label} connect timeout: {url}"))?
        .map_err(|err| format!("ADP {label} connect failed: {err}"))?;
    send_adp(
        &mut socket,
        UiAdpRequest::Query {
            request_id: request_id.to_owned(),
            query,
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err(format!("ADP {label} query timeout"));
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| format!("ADP {label} query timeout"))??;
        match response {
            UiAdpResponse::QueryResult {
                request_id: response_id,
                result,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Ok(result);
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP {label} query failure {}: {}",
                    failure.code, failure.message
                ));
            }
            _ => {}
        }
    }
}

fn task_agent_from_board(board: &UiTaskBoardProjection, task_id: &str) -> Result<String, String> {
    board
        .tasks
        .iter()
        .find(|task| task.task_id == task_id)
        .and_then(|task| task.assignee_agent_id.as_ref())
        .map(|agent_id| agent_id.as_str().to_owned())
        .ok_or_else(|| {
            format!(
                "task board missing assigned agent task={} task_count={}",
                task_id,
                board.tasks.len()
            )
        })
}

async fn submit_adp_sample_prompt(
    url: &str,
    session_id: &SessionId,
    label: &str,
    text: String,
) -> Result<Vec<String>, String> {
    submit_adp_sample_prompt_with_timeout(url, session_id, label, text, Duration::from_secs(90))
        .await
}

async fn submit_adp_sample_prompt_with_timeout(
    url: &str,
    session_id: &SessionId,
    label: &str,
    text: String,
    wait_duration: Duration,
) -> Result<Vec<String>, String> {
    let (mut socket, _) = timeout(Duration::from_secs(10), connect_async(url))
        .await
        .map_err(|_| format!("ADP connect timeout: {url}"))?
        .map_err(|err| format!("ADP connect failed: {err}"))?;
    let request_id = format!("cli-{label}-cmd");
    send_adp(
        &mut socket,
        UiAdpRequest::Command {
            request_id: request_id.clone(),
            command: UiCommand::SubmitUserInput {
                text,
                session_id: Some(session_id.clone()),
                cwd: None,
            },
        },
    )
    .await?;
    let deadline = tokio::time::Instant::now() + wait_duration;
    let mut seen = Vec::new();
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err(format!("{label} submit timeout seen={}", seen.join(",")));
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| format!("{label} submit timeout seen={}", seen.join(",")))??;
        match response {
            UiAdpResponse::CommandReceipt {
                request_id: response_id,
                receipt,
            } if response_id == request_id => {
                seen.push(format!(
                    "command_receipt:{response_id}:{}",
                    receipt.dispatch_status
                ));
                let _ = socket.close(None).await;
                return Ok(seen);
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "{label} submit failure {}: {}",
                    failure.code, failure.message
                ));
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
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => {
                seen.push(format!("failure:{request_id}:{}", failure.code));
            }
            UiAdpResponse::SubscriptionAccepted { request_id, .. } => {
                seen.push(format!("subscription_accepted:{request_id}"));
            }
            UiAdpResponse::SubscriptionEvent { request_id, .. } => {
                seen.push(format!("subscription_event:{request_id}"));
            }
            UiAdpResponse::QueryResult { request_id, .. } => {
                seen.push(format!("query_result:{request_id}"));
            }
        }
    }
}

async fn query_adp_sample_transcript_evidence(
    url: &str,
    sample: AdpTurnSample,
    session_id: &SessionId,
    request_id: &str,
) -> Result<AdpTurnSampleEvidence, String> {
    let transcript =
        query_session_transcript(url, session_id, request_id, Duration::from_secs(10)).await?;
    sample_transcript_evidence(sample, session_id, &transcript).ok_or_else(|| {
        format!(
            "ADP {} sample transcript missing expected evidence",
            sample.label()
        )
    })
}

async fn query_session_transcript(
    url: &str,
    session_id: &SessionId,
    request_id: &str,
    timeout_duration: Duration,
) -> Result<freehand_ui_protocol::UiSessionTranscriptProjection, String> {
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
    let deadline = tokio::time::Instant::now() + timeout_duration;
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let _ = socket.close(None).await;
            return Err("ADP session transcript timeout".to_owned());
        }
        let response = timeout(deadline - now, next_adp(&mut socket))
            .await
            .map_err(|_| "ADP session transcript timeout".to_owned())??;
        match response {
            UiAdpResponse::QueryResult {
                request_id: response_id,
                result,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                let freehand_ui_protocol::UiQueryResult::SessionTurns(transcript) = result else {
                    return Err("ADP session transcript returned non-transcript result".to_owned());
                };
                if &transcript.session_id != session_id {
                    return Err(format!(
                        "ADP session transcript wrong session expected={} actual={}",
                        session_id.as_str(),
                        transcript.session_id.as_str()
                    ));
                }
                return Ok(transcript);
            }
            UiAdpResponse::Failure {
                request_id: response_id,
                failure,
            } if response_id == request_id => {
                let _ = socket.close(None).await;
                return Err(format!(
                    "ADP session transcript failure {}: {}",
                    failure.code, failure.message
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
    schema_retries: usize,
    provider_retries: usize,
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
        AdpTurnSample::SchemaMismatch => {
            evidence.rounds >= 2 && evidence.schema_retries >= 1 && evidence.tool_executions == 0
        }
        AdpTurnSample::ProviderRetry => evidence.provider_retries >= 1,
    }
}

fn sample_transcript_evidence(
    sample: AdpTurnSample,
    session_id: &SessionId,
    transcript: &freehand_ui_protocol::UiSessionTranscriptProjection,
) -> Option<AdpTurnSampleEvidence> {
    if &transcript.session_id != session_id {
        return None;
    }
    let rounds = transcript.turns.len();
    let mut terminal_turn_id = None::<String>;
    let mut tool_executions = BTreeSet::new();
    let mut failed_tools = BTreeSet::new();
    let mut schema_retries = 0_usize;
    let mut provider_retries = 0_usize;
    for turn in &transcript.turns {
        if turn
            .model_request
            .as_ref()
            .is_some_and(|activity| activity.kind == UiModelRequestKind::SchemaRetry)
            || turn
                .model_request
                .as_ref()
                .and_then(|activity| activity.detail.as_deref())
                .is_some_and(|detail| detail.contains("schema polishing"))
            || turn
                .errors
                .iter()
                .any(|error| error.contains("schema") || error.contains("polishing"))
        {
            schema_retries += 1;
        }
        if turn.errors.iter().any(|error| {
            error.contains("provider") || error.contains("retry") || error.contains("http_status")
        }) || turn.terminal_text.as_deref().is_some_and(|text| {
            text.contains("provider") || text.contains("retry") || text.contains("http_status")
        }) {
            provider_retries += 1;
        }
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
        if matches!(
            sample,
            AdpTurnSample::Failure | AdpTurnSample::SchemaMismatch
        ) && turn
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
        schema_retries,
        provider_retries,
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

fn live_restore_status_label(status: LiveReasonRestoreStatus) -> &'static str {
    match status {
        LiveReasonRestoreStatus::CreatedNew => "created_new",
        LiveReasonRestoreStatus::RestoredExisting => "restored_existing",
    }
}
