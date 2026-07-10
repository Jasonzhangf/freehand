use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_config::{
    AgentMode, ProviderAuthSourceKind, ProviderAuthType, ProviderProtocol, ProviderType,
    SelectedAgentConfig, SelectedProviderConfig,
};
use freehand_contracts::{AgentId, TerminalStatus, TurnId};
use freehand_task::{
    TaskCreateRequest, TaskDispatchRequest, TaskId, TaskListQuery, TaskParentRef, TaskRuntime,
    TaskStatus,
};

use super::*;

struct StubExecutor {
    result: Mutex<Option<Result<WorkerTurnExecution, String>>>,
    calls: AtomicUsize,
}

impl StubExecutor {
    fn new(result: Result<WorkerTurnExecution, String>) -> Self {
        Self {
            result: Mutex::new(Some(result)),
            calls: AtomicUsize::new(0),
        }
    }
}

impl WorkerTurnExecutor for StubExecutor {
    fn execute(
        &self,
        _selected: &SelectedAgentConfig,
        _request: LiveReasonTurnRequest,
    ) -> Result<WorkerTurnExecution, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.result
            .lock()
            .expect("lock stub result")
            .take()
            .expect("stub result")
    }
}

#[test]
fn production_worker_runner_idle_does_not_create_task_truth() {
    let runtime_home = temp_path("idle");
    let runner = test_runner(
        runtime_home.clone(),
        Arc::new(StubExecutor::new(Err("must not execute".to_owned()))),
    );

    assert_eq!(
        runner.run_once().expect("idle tick"),
        ProductionWorkerTickOutcome::Idle
    );
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    assert!(
        task_runtime
            .list_tasks(TaskListQuery::default())
            .expect("list tasks")
            .is_empty()
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_worker_runner_success_claims_heartbeats_and_submits_review() {
    let runtime_home = temp_path("success");
    let workspace = temp_path("success-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "implemented and verified".to_owned(),
        turn_id: TurnId::new("worker-turn-success"),
    })));
    let runner = test_runner(runtime_home.clone(), executor);
    let expected_task_id = seed_assigned_task(&runtime_home, Some(&workspace));

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::ReviewReady { ref task_id, .. }
            if task_id == &expected_task_id
    ));
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let task = task_runtime
        .query_task(&expected_task_id)
        .expect("query task");
    assert_eq!(task.status, TaskStatus::ReviewSubmitted);
    let history = task_runtime
        .task_history(&expected_task_id)
        .expect("task history");
    let event_types = history
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"TaskResumed"));
    assert!(event_types.contains(&"TaskHeartbeat"));
    assert!(event_types.contains(&"TaskReviewSubmitted"));

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_provider_error_records_blocked_not_review_ready() {
    let runtime_home = temp_path("blocked");
    let workspace = temp_path("blocked-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let runner = test_runner(
        runtime_home.clone(),
        Arc::new(StubExecutor::new(Err(
            "provider unavailable after retries".to_owned()
        ))),
    );
    let expected_task_id = seed_assigned_task(&runtime_home, Some(&workspace));

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::Blocked { ref task_id, .. }
            if task_id == &expected_task_id
    ));
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let task = task_runtime
        .query_task(&expected_task_id)
        .expect("query task");
    assert_eq!(task.status, TaskStatus::Blocked);
    let history = task_runtime
        .task_history(&expected_task_id)
        .expect("task history");
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "TaskBlocked")
    );
    assert!(
        !history
            .iter()
            .any(|event| event.event_type == "TaskReviewSubmitted")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_missing_workspace_blocks_before_model_execution() {
    let runtime_home = temp_path("missing-workspace");
    let executor = Arc::new(StubExecutor::new(Err("must not execute".to_owned())));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let expected_task_id = seed_assigned_task(&runtime_home, None);

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::Blocked { ref task_id, .. }
            if task_id == &expected_task_id
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    assert_eq!(
        task_runtime
            .query_task(&expected_task_id)
            .expect("task")
            .status,
        TaskStatus::Blocked
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_worker_runner_rejects_master_mode() {
    let runtime_home = temp_path("master-rejected");
    let mut selected = selected_worker();
    selected.mode = AgentMode::Master;

    let error = match ProductionWorkerRunner::from_selected_agent(selected, runtime_home.clone()) {
        Ok(_) => panic!("master mode must be rejected"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        ProductionWorkerRunnerError::RequiresSlaveMode { .. }
    ));
    if runtime_home.exists() {
        fs::remove_dir_all(runtime_home).expect("cleanup");
    }
}

fn test_runner(
    runtime_home: PathBuf,
    executor: Arc<dyn WorkerTurnExecutor>,
) -> ProductionWorkerRunner {
    ProductionWorkerRunner::from_selected_agent_with_executor(
        selected_worker(),
        runtime_home,
        executor,
    )
    .expect("worker runner")
}

fn seed_assigned_task(runtime_home: &Path, workspace: Option<&Path>) -> TaskId {
    let task_runtime =
        TaskRuntime::boot(runtime_home, AgentId::new("master")).expect("task runtime");
    let task_id = TaskId::new(format!("task-{}", now_unix_seconds()));
    task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(task_id.clone()),
            title: "worker task".to_owned(),
            content: "inspect and implement".to_owned(),
            goal: "complete assigned work".to_owned(),
            deliverables: vec!["implementation".to_owned()],
            acceptance: vec!["tests pass".to_owned()],
            priority: 80,
            target_cwd: workspace.map(|path| path.display().to_string()),
            dispatch: TaskDispatchRequest::Agent {
                agent_id: AgentId::new("worker"),
            },
            parent: TaskParentRef {
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            actor: worker_actor(&AgentId::new("master"), None),
            watermark: worker_watermark("seed", "create"),
        })
        .expect("create assigned task");
    task_id
}

fn selected_worker() -> SelectedAgentConfig {
    SelectedAgentConfig {
        name: "worker".to_owned(),
        mode: AgentMode::Slave,
        node_id: "worker-node".to_owned(),
        paired_agent_name: "master".to_owned(),
        paired_agent_mode: AgentMode::Master,
        paired_node_id: "master-node".to_owned(),
        paired_allowed_pair_ip: None,
        paired_pair_token_env: "FREEHAND_PAIR_TOKEN_MASTER".to_owned(),
        allowed_pair_ip: None,
        pair_token_env: "FREEHAND_PAIR_TOKEN_WORKER".to_owned(),
        pair_token: "pair-token".to_owned(),
        provider: SelectedProviderConfig {
            id: "worker-provider".to_owned(),
            provider_type: ProviderType::Anthropic,
            protocol: ProviderProtocol::Messages,
            base_url: "https://example.invalid".to_owned(),
            default_model: "worker-model".to_owned(),
            auth_type: ProviderAuthType::ApiKey,
            auth_source: ProviderAuthSourceKind::Inline,
            api_key: "test-key".to_owned(),
        },
        restart_required_on_change: true,
    }
}

fn temp_path(label: &str) -> PathBuf {
    let counter = EXECUTION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "freehand-production-worker-runner-{label}-{nanos}-{counter}"
    ))
}
