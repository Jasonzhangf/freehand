use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_config::{
    AgentMode, ProviderAuthSourceKind, ProviderAuthType, ProviderProtocol, ProviderType,
    SelectedAgentConfig, SelectedPeerAgentConfig, SelectedProviderConfig,
};
use freehand_contracts::{AgentId, TerminalStatus, TurnId};
use freehand_task::{
    AgentStatus, TaskActor, TaskClaimRequest, TaskCreateRequest, TaskDispatchRequest, TaskId,
    TaskListQuery, TaskMutationRequest, TaskParentRef, TaskReviewRejection, TaskRuntime,
    TaskStatus, TaskWatermark,
};
use serde_json::Value;

use super::*;

fn home_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct StubExecutor {
    result: Mutex<Option<Result<WorkerTurnExecution, String>>>,
    calls: AtomicUsize,
    prompts: Mutex<Vec<String>>,
}

impl StubExecutor {
    fn new(result: Result<WorkerTurnExecution, String>) -> Self {
        Self {
            result: Mutex::new(Some(result)),
            calls: AtomicUsize::new(0),
            prompts: Mutex::new(Vec::new()),
        }
    }

    fn prompts(&self) -> Vec<String> {
        self.prompts.lock().expect("lock prompts").clone()
    }
}

impl WorkerTurnExecutor for StubExecutor {
    fn execute(
        &self,
        _selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<WorkerTurnExecution, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.prompts
            .lock()
            .expect("lock prompts")
            .push(request.prompt);
        self.result
            .lock()
            .expect("lock stub result")
            .take()
            .expect("stub result")
    }
}

struct CancelDuringExecutionExecutor {
    runtime_home: PathBuf,
    task_id: Mutex<Option<TaskId>>,
}

impl CancelDuringExecutionExecutor {
    fn new(runtime_home: PathBuf) -> Self {
        Self {
            runtime_home,
            task_id: Mutex::new(None),
        }
    }

    fn set_task_id(&self, task_id: TaskId) {
        *self.task_id.lock().expect("lock task id") = Some(task_id);
    }
}

impl WorkerTurnExecutor for CancelDuringExecutionExecutor {
    fn execute(
        &self,
        _selected: &SelectedAgentConfig,
        _request: LiveReasonTurnRequest,
    ) -> Result<WorkerTurnExecution, String> {
        let task_id = self
            .task_id
            .lock()
            .expect("lock task id")
            .clone()
            .expect("task id set");
        let runtime =
            TaskRuntime::boot(&self.runtime_home, AgentId::new("master")).expect("task runtime");
        runtime
            .cancel_task(TaskMutationRequest {
                task_id,
                actor: test_actor("master"),
                watermark: test_watermark("external-cancel"),
            })
            .expect("external cancel");
        Ok(WorkerTurnExecution {
            status: TerminalStatus::Success,
            summary: "stale success after external cancel".to_owned(),
            turn_id: TurnId::new("worker-turn-after-external-cancel"),
        })
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
fn production_worker_runner_rejects_result_after_external_cancel_without_terminal_overwrite() {
    let runtime_home = temp_path("external-cancel-during-execution");
    let workspace = temp_path("external-cancel-during-execution-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(CancelDuringExecutionExecutor::new(runtime_home.clone()));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    executor.set_task_id(task_id.clone());

    let error = runner
        .run_once()
        .expect_err("stale worker result after cancel must fail");
    assert!(matches!(error, ProductionWorkerRunnerError::TaskCenter(_)));

    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let task = runtime.query_task(&task_id).expect("task");
    assert_eq!(task.status, TaskStatus::Cancelled);
    let history = runtime.task_history(&task_id).expect("history");
    assert_eq!(
        history.last().map(|event| event.event_type.as_str()),
        Some("TaskCancelled")
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
fn production_worker_runner_provider_error_records_interrupted_and_requeues_same_task() {
    let runtime_home = temp_path("provider-interrupted");
    let workspace = temp_path("provider-interrupted-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let runner = test_runner(
        runtime_home.clone(),
        Arc::new(StubExecutor::new(Err(
            "openai_http_request_failed: error sending request for url".to_owned(),
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
    assert_eq!(task.status, TaskStatus::Interrupted);
    let history = task_runtime
        .task_history(&expected_task_id)
        .expect("task history");
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "TaskInterrupted")
    );
    assert!(
        !history
            .iter()
            .any(|event| event.event_type == "TaskBlocked")
    );
    assert!(
        !history
            .iter()
            .any(|event| event.event_type == "TaskReviewSubmitted")
    );
    let retry_executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "recovered after provider interruption".to_owned(),
        turn_id: TurnId::new("worker-turn-provider-recovered"),
    })));
    let retry_runner = test_runner(runtime_home.clone(), retry_executor.clone());
    let retry_execution_id = match retry_runner.run_once().expect("interrupted retry tick") {
        ProductionWorkerTickOutcome::ReviewReady {
            task_id,
            execution_id,
            ..
        } => {
            assert_eq!(task_id, expected_task_id);
            execution_id
        }
        other => panic!("expected same task review after interruption retry, got {other:?}"),
    };
    assert!(retry_executor.prompts()[0].contains("previous execution was interrupted"));
    assert!(
        task_runtime
            .task_history(&expected_task_id)
            .expect("task history after retry")
            .iter()
            .any(|event| {
                event.event_type == "TaskReviewSubmitted"
                    && event.payload["execution_id"] == retry_execution_id
            })
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn worker_retryable_provider_error_classifier_covers_supported_provider_families() {
    for reason in [
        "anthropic_http_request_failed: error sending request for url",
        "anthropic_stream_read_failed: broken stream",
        "anthropic_http_status_429: rate limited",
        "anthropic_http_status_503: unavailable",
        "openai_http_request_failed: error sending request for url",
        "openai_stream_read_failed: broken stream",
        "openai_http_status_429: rate limited",
        "openai_http_status_503: unavailable",
    ] {
        assert!(
            super::worker_execution_error_is_retryable_system_failure(reason),
            "{reason}"
        );
    }

    for reason in [
        "worker produced invalid deliverable",
        "openai_adapter_failed: invalid response shape",
        "openai_callback_failed: callback failed",
    ] {
        assert!(
            !super::worker_execution_error_is_retryable_system_failure(reason),
            "{reason}"
        );
    }
}

#[test]
fn production_worker_runner_non_provider_execution_error_records_blocked_not_retryable() {
    let runtime_home = temp_path("blocked");
    let workspace = temp_path("blocked-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let runner = test_runner(
        runtime_home.clone(),
        Arc::new(StubExecutor::new(Err(
            "worker produced invalid deliverable".to_owned(),
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
    assert_eq!(
        task_runtime
            .query_task(&expected_task_id)
            .expect("query task")
            .status,
        TaskStatus::Blocked
    );
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
            .any(|event| event.event_type == "TaskInterrupted")
    );
    let retry_executor = Arc::new(StubExecutor::new(Err("must not execute".to_owned())));
    let retry_runner = test_runner(runtime_home.clone(), retry_executor.clone());
    assert_eq!(
        retry_runner.run_once().expect("blocked follow-up tick"),
        ProductionWorkerTickOutcome::Idle
    );
    assert_eq!(retry_executor.calls.load(Ordering::Relaxed), 0);

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_startup_repairs_legacy_blocked_pause() {
    let runtime_home = temp_path("startup-repairs-legacy-blocked-pause");
    let workspace = temp_path("startup-repairs-legacy-blocked-pause-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let runner = test_runner(
        runtime_home.clone(),
        Arc::new(StubExecutor::new(Err("provider unavailable".to_owned()))),
    );
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    let outcome = runner.run_once().expect("blocked worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::Blocked { .. }
    ));

    let agent_path = runtime_home.join("state/agents/worker.json");
    let mut agent_json: Value =
        serde_json::from_slice(&fs::read(&agent_path).expect("read agent snapshot"))
            .expect("parse agent snapshot");
    agent_json["status"] = Value::String("paused".to_owned());
    fs::write(
        &agent_path,
        serde_json::to_vec_pretty(&agent_json).expect("serialize legacy agent snapshot"),
    )
    .expect("write legacy paused agent snapshot");

    let restarted = test_runner(
        runtime_home.clone(),
        Arc::new(StubExecutor::new(Err("must not execute".to_owned()))),
    );
    assert_eq!(
        restarted.run_once().expect("idle startup tick"),
        ProductionWorkerTickOutcome::Idle
    );
    let recovered =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("recover task center");
    assert_eq!(
        recovered
            .query_agent(&AgentId::new("worker"))
            .expect("worker")
            .status,
        AgentStatus::Available
    );
    assert_eq!(
        recovered.query_task(&task_id).expect("blocked task").status,
        TaskStatus::Blocked
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_requeues_interrupted_with_new_execution() {
    let runtime_home = temp_path("interrupted-retry");
    let workspace = temp_path("interrupted-retry-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "recovered and verified".to_owned(),
        turn_id: TurnId::new("worker-turn-after-crash"),
    })));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    let old_execution_id = "exec-before-crash".to_owned();
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: AgentId::new("worker"),
            execution_id: old_execution_id.clone(),
            ttl_seconds: 1,
            actor: test_actor("worker"),
            watermark: test_watermark("old-claim"),
        })
        .expect("claim before crash");
    std::thread::sleep(std::time::Duration::from_secs(2));
    drop(runtime);

    let outcome = runner.run_once().expect("recovery tick");
    let new_execution_id = match outcome {
        ProductionWorkerTickOutcome::ReviewReady {
            execution_id,
            task_id: ref outcome_task_id,
            ..
        } => {
            assert_eq!(outcome_task_id, &task_id);
            execution_id
        }
        other => panic!("expected review ready, got {other:?}"),
    };
    assert_ne!(new_execution_id, old_execution_id);
    assert!(executor.prompts()[0].contains("previous execution was interrupted"));
    let recovered = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("recover");
    let history = recovered.task_history(&task_id).expect("history");
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "TaskInterrupted")
    );
    assert_eq!(
        history
            .iter()
            .filter(|event| event.event_type == "TaskAssigned")
            .count(),
        2
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_requeues_rejected_with_review_requirements() {
    let runtime_home = temp_path("rejected-retry");
    let workspace = temp_path("rejected-retry-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let first_executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "first submission".to_owned(),
        turn_id: TurnId::new("worker-turn-first"),
    })));
    let first_runner = test_runner(runtime_home.clone(), first_executor);
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    let first_execution_id = match first_runner.run_once().expect("first tick") {
        ProductionWorkerTickOutcome::ReviewReady { execution_id, .. } => execution_id,
        other => panic!("expected review ready, got {other:?}"),
    };
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    runtime
        .reject_review(TaskReviewRejection {
            task_id: task_id.clone(),
            reject_reason: "missing regression evidence".to_owned(),
            next_requirements: vec!["run the restart regression".to_owned()],
            actor: test_actor("master"),
            watermark: test_watermark("reject"),
        })
        .expect("reject review");
    drop(runtime);

    let retry_executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "retry complete".to_owned(),
        turn_id: TurnId::new("worker-turn-retry"),
    })));
    let retry_runner = test_runner(runtime_home.clone(), retry_executor.clone());
    let retry_execution_id = match retry_runner.run_once().expect("retry tick") {
        ProductionWorkerTickOutcome::ReviewReady { execution_id, .. } => execution_id,
        other => panic!("expected retry review ready, got {other:?}"),
    };
    assert_ne!(retry_execution_id, first_execution_id);
    let retry_prompt = &retry_executor.prompts()[0];
    assert!(retry_prompt.contains("missing regression evidence"));
    assert!(retry_prompt.contains("run the restart regression"));

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
fn production_worker_runner_expands_tilde_and_prompts_canonical_symlink_preflight() {
    let _home_lock = home_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let original_home = std::env::var_os("HOME");
    let runtime_home = temp_path("tilde-symlink-runtime");
    let fake_home = temp_path("tilde-symlink-home");
    let canonical_parent = temp_path("tilde-symlink-canonical-parent");
    let canonical_workspace = canonical_parent.join("repo");
    let symlink_parent = fake_home.join("github");
    let requested_workspace = "~/github/repo";
    fs::create_dir_all(&canonical_workspace).expect("canonical workspace");
    fs::create_dir_all(&fake_home).expect("fake home");
    create_dir_symlink(&canonical_parent, &symlink_parent);
    unsafe {
        std::env::set_var("HOME", &fake_home);
    }
    let executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "symlink path verified".to_owned(),
        turn_id: TurnId::new("worker-turn-tilde-symlink"),
    })));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let expected_task_id =
        seed_assigned_task_with_target(&runtime_home, Some(requested_workspace.to_owned()));

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::ReviewReady { ref task_id, .. }
            if task_id == &expected_task_id
    ));
    let prompt = &executor.prompts()[0];
    assert!(prompt.contains("Requested target_cwd: ~/github/repo"));
    assert!(prompt.contains("Canonical locked workspace:"));
    assert!(prompt.contains("Path preflight"));
    assert!(prompt.contains("whether the requested path or any parent is a symlink"));
    assert!(prompt.contains("report both requested and canonical paths"));
    assert!(prompt.contains(&canonical_workspace.to_string_lossy().to_string()));
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let task = task_runtime
        .query_task(&expected_task_id)
        .expect("query task");
    assert_eq!(task.status, TaskStatus::ReviewSubmitted);
    assert_eq!(task.target_cwd.as_deref(), Some(requested_workspace));

    restore_home(original_home);
    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(fake_home).expect("cleanup fake home");
    fs::remove_dir_all(canonical_parent).expect("cleanup canonical parent");
}

#[test]
fn production_worker_runner_missing_tilde_path_blocks_before_model_execution() {
    let _home_lock = home_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let original_home = std::env::var_os("HOME");
    let runtime_home = temp_path("missing-tilde-runtime");
    let fake_home = temp_path("missing-tilde-home");
    fs::create_dir_all(&fake_home).expect("fake home");
    unsafe {
        std::env::set_var("HOME", &fake_home);
    }
    let executor = Arc::new(StubExecutor::new(Err("must not execute".to_owned())));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let expected_task_id =
        seed_assigned_task_with_target(&runtime_home, Some("~/github/missing".to_owned()));

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::Blocked { ref task_id, ref reason, .. }
            if task_id == &expected_task_id
                && reason.contains("target_cwd `~/github/missing`")
                && reason.contains("path cannot be resolved because one of its parent directories does not exist")
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);
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

    restore_home(original_home);
    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(fake_home).expect("cleanup fake home");
}

#[test]
fn production_worker_runner_missing_workspace_under_existing_parent_explains_target_cwd_misuse() {
    let _home_lock = home_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_path("missing-output-like-runtime");
    let existing_parent = temp_path("missing-output-like-parent");
    fs::create_dir_all(&existing_parent).expect("existing parent");
    let missing_workspace = existing_parent.join("analysis-output");
    let executor = Arc::new(StubExecutor::new(Err("must not execute".to_owned())));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let expected_task_id = seed_assigned_task_with_target(
        &runtime_home,
        Some(missing_workspace.display().to_string()),
    );

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::Blocked { ref task_id, ref reason, .. }
            if task_id == &expected_task_id
                && reason.contains("does not exist")
                && reason.contains("not a repository-permission denial")
                && reason.contains("used target_cwd for a not-yet-created output directory")
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);

    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let task = task_runtime
        .query_task(&expected_task_id)
        .expect("query task");
    assert_eq!(task.status, TaskStatus::Blocked);

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(existing_parent).expect("cleanup existing parent");
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
    seed_assigned_task_with_target(
        runtime_home,
        workspace.map(|path| path.display().to_string()),
    )
}

fn seed_assigned_task_with_target(runtime_home: &Path, target_cwd: Option<String>) -> TaskId {
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
            target_cwd,
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

#[cfg(unix)]
fn create_dir_symlink(source: &Path, link: &Path) {
    std::os::unix::fs::symlink(source, link).expect("create dir symlink");
}

#[cfg(windows)]
fn create_dir_symlink(source: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(source, link).expect("create dir symlink");
}

fn restore_home(original_home: Option<std::ffi::OsString>) {
    unsafe {
        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
    }
}

fn test_actor(agent_id: &str) -> TaskActor {
    TaskActor {
        agent_id: AgentId::new(agent_id),
        source: "runtime.master-worker-loop.test".to_owned(),
        session_id: None,
        turn_id: None,
        trace_id: None,
    }
}

fn test_watermark(hook: &str) -> TaskWatermark {
    TaskWatermark {
        metadata_id: None,
        hook: Some(hook.to_owned()),
        action_tool_call_id: None,
    }
}

fn selected_worker() -> SelectedAgentConfig {
    SelectedAgentConfig {
        name: "worker".to_owned(),
        mode: AgentMode::Slave,
        node_id: "worker-node".to_owned(),
        paired_agents: vec![SelectedPeerAgentConfig {
            name: "master".to_owned(),
            mode: AgentMode::Master,
            node_id: "master-node".to_owned(),
            allowed_pair_ip: None,
            pair_token_env: "FREEHAND_PAIR_TOKEN_MASTER".to_owned(),
        }],
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
        fallback_provider: None,
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
