use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use freehand_config::{
    AgentMode, ProviderAuthSourceKind, ProviderAuthType, ProviderProtocol, ProviderType,
    ProviderWebSearchMode, ProviderWebSearchWire, SelectedAgentConfig, SelectedPeerAgentConfig,
    SelectedProviderConfig,
};
use freehand_contracts::{AgentId, TerminalStatus, TurnId};
use freehand_task::{
    AgentStatus, TaskActor, TaskAssignRequest, TaskClaimRequest, TaskCreateRequest,
    TaskDispatchRequest, TaskExecutionProfile, TaskId, TaskListQuery, TaskMutationRequest,
    TaskParentRef, TaskReviewRejection, TaskRuntime, TaskStatus, TaskWatermark, WorkerControlOp,
    WorkerControlRequest,
};
use serde_json::Value;

use super::*;
use crate::{RuntimeAgentActivityProjection, RuntimeAgentActivityStatus};

fn home_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct StubExecutor {
    result: Mutex<Option<Result<WorkerTurnExecution, String>>>,
    calls: AtomicUsize,
    prompts: Mutex<Vec<String>>,
    requests: Mutex<Vec<LiveReasonTurnRequest>>,
}

impl StubExecutor {
    fn new(result: Result<WorkerTurnExecution, String>) -> Self {
        Self {
            result: Mutex::new(Some(result)),
            calls: AtomicUsize::new(0),
            prompts: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn prompts(&self) -> Vec<String> {
        self.prompts.lock().expect("lock prompts").clone()
    }

    fn requests(&self) -> Vec<LiveReasonTurnRequest> {
        self.requests.lock().expect("lock requests").clone()
    }
}

impl WorkerTurnExecutor for StubExecutor {
    fn execute(
        &self,
        _selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<WorkerTurnExecution, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.requests
            .lock()
            .expect("lock requests")
            .push(request.clone());
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
        request: LiveReasonTurnRequest,
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
            .apply_worker_control(worker_control_request(
                &task_id,
                request
                    .turn_id
                    .as_str()
                    .strip_prefix("worker-turn-")
                    .expect("worker turn id carries execution id"),
                WorkerControlOp::Cancel,
                "external-cancel",
            ))
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
    let lifecycle = task_runtime
        .query_agent_lifecycle(&AgentId::new("worker"))
        .expect("worker lifecycle");
    assert!(lifecycle.alive);
    assert_eq!(lifecycle.process_id, Some(std::process::id()));
    assert!(lifecycle.process_instance_id.is_some());
    assert!(lifecycle.process_started_at.is_some());
    assert!(lifecycle.process_heartbeat_at.is_some());
    assert_eq!(lifecycle.restart_count, 0);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_worker_runner_restart_reuses_agent_id_and_increments_process_identity() {
    let runtime_home = temp_path("process-restart");
    let first = test_runner(
        runtime_home.clone(),
        Arc::new(StubExecutor::new(Err("must not execute".to_owned()))),
    );
    first.run_once().expect("first idle tick");
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let first_lifecycle = task_runtime
        .query_agent_lifecycle(&AgentId::new("worker"))
        .expect("first lifecycle");

    let restarted = test_runner(
        runtime_home.clone(),
        Arc::new(StubExecutor::new(Err("must not execute".to_owned()))),
    );
    restarted.run_once().expect("restarted idle tick");
    let restarted_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("restarted task runtime");
    let restarted_lifecycle = restarted_runtime
        .query_agent_lifecycle(&AgentId::new("worker"))
        .expect("restarted lifecycle");

    assert_eq!(restarted_lifecycle.agent_id, first_lifecycle.agent_id);
    assert_ne!(
        restarted_lifecycle.process_instance_id,
        first_lifecycle.process_instance_id
    );
    assert_eq!(restarted_lifecycle.restart_count, 1);
    assert!(restarted_lifecycle.alive);

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
fn production_worker_runner_clean_search_runs_without_target_cwd_on_hosted_provider() {
    let runtime_home = temp_path("clean-search-success");
    let executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "search evidence returned".to_owned(),
        turn_id: TurnId::new("worker-turn-clean-search"),
    })));
    let runner = ProductionWorkerRunner::from_selected_agent_with_executor(
        selected_worker_openai_responses_search(),
        runtime_home.clone(),
        executor.clone(),
    )
    .expect("worker runner");
    let expected_task_id = seed_assigned_clean_search_task(&runtime_home);

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::ReviewReady { ref task_id, .. }
            if task_id == &expected_task_id
    ));
    let requests = executor.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].execution_profile,
        LiveReasonExecutionProfile::CleanSearch
    );
    assert!(
        requests[0].cwd.is_none(),
        "clean_search must not bind or scan a local workspace cwd"
    );
    assert!(
        requests[0]
            .prompt
            .contains("Execution profile: clean_search")
    );
    assert!(requests[0].prompt.contains("provider-hosted web_search"));
    assert!(requests[0].prompt.contains("No target_cwd is needed"));
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let task = task_runtime
        .query_task(&expected_task_id)
        .expect("query clean search task");
    assert_eq!(task.status, TaskStatus::ReviewSubmitted);
    assert_eq!(task.execution_profile, TaskExecutionProfile::CleanSearch);
    assert!(task.target_cwd.is_none());

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
}

#[test]
fn production_worker_runner_clean_search_blocks_when_provider_has_no_hosted_search() {
    let runtime_home = temp_path("clean-search-unsupported");
    let executor = Arc::new(StubExecutor::new(Err("must not execute".to_owned())));
    let mut selected = selected_worker();
    selected.provider.web_search = ProviderWebSearchMode::Disabled;
    let runner = ProductionWorkerRunner::from_selected_agent_with_executor(
        selected,
        runtime_home.clone(),
        executor.clone(),
    )
    .expect("worker runner");
    let expected_task_id = seed_assigned_clean_search_task(&runtime_home);

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::Blocked {
            ref task_id,
            ref reason,
            ..
        } if task_id == &expected_task_id
            && reason.contains("requires provider-hosted web_search")
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let task = task_runtime
        .query_task(&expected_task_id)
        .expect("query clean search task");
    assert_eq!(task.status, TaskStatus::Blocked);
    assert_eq!(task.execution_profile, TaskExecutionProfile::CleanSearch);
    assert!(task.target_cwd.is_none());

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
}

#[test]
fn production_worker_runner_sourced_search_runs_without_target_cwd_on_hosted_provider() {
    let runtime_home = temp_path("sourced-search-success");
    let executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "verified search evidence returned".to_owned(),
        turn_id: TurnId::new("worker-turn-sourced-search"),
    })));
    let runner = ProductionWorkerRunner::from_selected_agent_with_executor(
        selected_worker_openai_responses_search(),
        runtime_home.clone(),
        executor.clone(),
    )
    .expect("worker runner");
    let expected_task_id = seed_assigned_sourced_search_task(&runtime_home);

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::ReviewReady { ref task_id, .. }
            if task_id == &expected_task_id
    ));
    let requests = executor.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].execution_profile,
        LiveReasonExecutionProfile::SourcedSearch
    );
    assert!(
        requests[0].cwd.is_none(),
        "sourced_search must not bind or scan a local workspace cwd"
    );
    assert!(
        requests[0]
            .prompt
            .contains("Execution profile: sourced_search")
    );
    assert!(requests[0].prompt.contains("provider-hosted web_search"));
    assert!(requests[0].prompt.contains("camo verification"));
    assert!(requests[0].prompt.contains("No target_cwd is needed"));
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let task = task_runtime
        .query_task(&expected_task_id)
        .expect("query sourced search task");
    assert_eq!(task.status, TaskStatus::ReviewSubmitted);
    assert_eq!(task.execution_profile, TaskExecutionProfile::SourcedSearch);
    assert!(task.target_cwd.is_none());

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
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

    assert_eq!(
        runner.run_once().expect("cancelled worker tick"),
        ProductionWorkerTickOutcome::Idle
    );

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
fn production_worker_runner_provider_error_waits_for_master_reassignment() {
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
    assert_eq!(
        retry_runner.run_once().expect("interrupted idle tick"),
        ProductionWorkerTickOutcome::Idle
    );
    assert_eq!(retry_executor.calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        task_runtime
            .task_history(&expected_task_id)
            .expect("history before Master reassignment")
            .iter()
            .filter(|event| event.event_type == "TaskAssigned")
            .count(),
        1
    );
    task_runtime
        .assign_task(TaskAssignRequest {
            task_id: expected_task_id.clone(),
            agent_id: AgentId::new("worker"),
            actor: test_actor("master"),
            watermark: test_watermark("master-reassign-after-interruption"),
        })
        .expect("Master reassign interrupted task");
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
fn production_worker_runner_interrupted_terminal_emits_attention_required() {
    let runtime_home = temp_path("terminal-attention-required");
    let workspace = temp_path("terminal-attention-required-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let runner = test_runner(
        runtime_home.clone(),
        Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
            status: TerminalStatus::Interrupted,
            summary: "task contract changed while execution was in progress".to_owned(),
            turn_id: TurnId::new("worker-turn-attention-required"),
        }))),
    );
    let expected_task_id = seed_assigned_task(&runtime_home, Some(&workspace));

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::AttentionRequired {
            ref task_id,
            ref reason,
            ..
        } if task_id == &expected_task_id
            && reason.contains("task contract changed")
    ));
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    assert_eq!(
        task_runtime
            .query_task(&expected_task_id)
            .expect("query task")
            .status,
        TaskStatus::Interrupted
    );
    let history = task_runtime
        .task_history(&expected_task_id)
        .expect("task history");
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "TaskAttentionRequired")
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
fn production_worker_runner_expired_lease_waits_for_master_reassignment() {
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

    assert_eq!(
        runner.run_once().expect("expired lease recovery tick"),
        ProductionWorkerTickOutcome::Idle
    );
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);
    let interrupted = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("recover");
    let history = interrupted.task_history(&task_id).expect("history");
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
        1
    );
    interrupted
        .assign_task(TaskAssignRequest {
            task_id: task_id.clone(),
            agent_id: AgentId::new("worker"),
            actor: test_actor("master"),
            watermark: test_watermark("master-reassign-after-expired-lease"),
        })
        .expect("Master reassign expired task");

    let outcome = runner.run_once().expect("Master-directed recovery tick");
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
fn production_worker_runner_pause_stops_before_submission() {
    let runtime_home = temp_path("pause-stops-before-submission");
    let workspace = temp_path("pause-stops-before-submission-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(PauseAwareExecutor::new(runtime_home.clone()));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    executor.set_task_id(task_id.clone());

    assert_eq!(
        runner.run_once().expect("in-flight paused tick"),
        ProductionWorkerTickOutcome::Idle
    );
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);
    assert!(
        executor.pause_token_observed.load(Ordering::Relaxed),
        "runner must wire Worker pause truth into the live cancel token"
    );
    let recovered = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    assert_eq!(
        recovered.query_task(&task_id).expect("task").status,
        TaskStatus::Paused
    );
    let event_types = recovered
        .task_history(&task_id)
        .expect("history")
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"TaskPaused".to_owned()));
    assert!(
        !event_types.contains(&"TaskBlocked".to_owned()),
        "pause acknowledgement must not be materialized as task blockage"
    );
    assert!(
        !event_types.contains(&"TaskReviewSubmitted".to_owned()),
        "pause safe point must stop before review submission"
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_paused_without_resume_stays_idle() {
    let runtime_home = temp_path("paused-without-resume");
    let workspace = temp_path("paused-without-resume-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(StubExecutor::new(Err(
        "paused task must not execute".to_owned()
    )));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    let execution_id = "exec-paused-without-resume".to_owned();
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: AgentId::new("worker"),
            execution_id: execution_id.clone(),
            ttl_seconds: 300,
            actor: test_actor("worker"),
            watermark: test_watermark("paused-claim"),
        })
        .expect("claim before pause");
    runtime
        .apply_worker_control(worker_control_request(
            &task_id,
            &execution_id,
            WorkerControlOp::Pause,
            "pause-before-runner",
        ))
        .expect("pause task");

    assert_eq!(
        runner.run_once().expect("paused tick"),
        ProductionWorkerTickOutcome::Idle
    );
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);
    assert_eq!(
        TaskRuntime::boot(&runtime_home, AgentId::new("master"))
            .expect("runtime")
            .query_task(&task_id)
            .expect("task")
            .status,
        TaskStatus::Paused
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_resume_reenters_reasoning_and_submits_review() {
    let runtime_home = temp_path("resume-reenters-reasoning");
    let workspace = temp_path("resume-reenters-reasoning-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "continued after pause".to_owned(),
        turn_id: TurnId::new("worker-turn-resumed"),
    })));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let expected_task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    let execution_id = "exec-resume-reenters-reasoning".to_owned();
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: AgentId::new("worker"),
            execution_id: execution_id.clone(),
            ttl_seconds: 300,
            actor: test_actor("worker"),
            watermark: test_watermark("resume-claim"),
        })
        .expect("claim before pause");
    runtime
        .apply_worker_control(worker_control_request(
            &expected_task_id,
            &execution_id,
            WorkerControlOp::Pause,
            "pause-before-resume",
        ))
        .expect("pause task");
    runtime
        .apply_worker_control(worker_control_request(
            &expected_task_id,
            &execution_id,
            WorkerControlOp::Resume,
            "resume-before-runner",
        ))
        .expect("resume task");

    let outcome = runner.run_once().expect("resumed tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::ReviewReady {
            task_id: ref outcome_task_id,
            execution_id: ref outcome_execution_id,
            ..
        } if outcome_task_id == &expected_task_id && outcome_execution_id == &execution_id
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);
    let recovered = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let task = recovered.query_task(&expected_task_id).expect("task");
    assert_eq!(task.status, TaskStatus::ReviewSubmitted);
    assert_eq!(
        task.active_execution_id.as_deref(),
        Some(execution_id.as_str())
    );
    let event_types = recovered
        .task_history(&expected_task_id)
        .expect("history")
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"TaskPaused".to_owned()));
    assert!(event_types.contains(&"TaskResumed".to_owned()));
    assert!(event_types.contains(&"TaskReviewSubmitted".to_owned()));

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_renders_queued_controls_before_provider_request() {
    let runtime_home = temp_path("queued-controls-prompt");
    let workspace = temp_path("queued-controls-prompt-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(StubExecutor::new(Ok(WorkerTurnExecution {
        status: TerminalStatus::Success,
        summary: "control-aware execution".to_owned(),
        turn_id: TurnId::new("worker-turn-controls"),
    })));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    let execution_id = "exec-queued-controls".to_owned();
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: AgentId::new("worker"),
            execution_id: execution_id.clone(),
            ttl_seconds: 300,
            actor: test_actor("worker"),
            watermark: test_watermark("queued-controls-claim"),
        })
        .expect("claim task");
    let mut question = worker_control_request(
        &task_id,
        &execution_id,
        WorkerControlOp::AskAtSafePoint,
        "question",
    );
    question.question = Some("please report the current blocker".to_owned());
    runtime
        .apply_worker_control(question)
        .expect("queue question");
    let mut constraint = worker_control_request(
        &task_id,
        &execution_id,
        WorkerControlOp::AddConstraint,
        "constraint",
    );
    constraint.constraint = Some("do not change generated files".to_owned());
    runtime
        .apply_worker_control(constraint)
        .expect("queue constraint");
    runtime
        .apply_worker_control(worker_control_request(
            &task_id,
            &execution_id,
            WorkerControlOp::RequestCheckpoint,
            "checkpoint-now",
        ))
        .expect("queue checkpoint");
    runtime
        .apply_worker_control(worker_control_request(
            &task_id,
            &execution_id,
            WorkerControlOp::RequestSubmissionNow,
            "submit-now",
        ))
        .expect("queue submission");
    runtime
        .apply_worker_control(worker_control_request(
            &task_id,
            &execution_id,
            WorkerControlOp::Pause,
            "pause-before-control-aware-run",
        ))
        .expect("pause before control-aware run");
    runtime
        .apply_worker_control(worker_control_request(
            &task_id,
            &execution_id,
            WorkerControlOp::Resume,
            "resume-before-control-aware-run",
        ))
        .expect("resume before control-aware run");

    let outcome = runner.run_once().expect("worker tick");
    assert!(matches!(
        outcome,
        ProductionWorkerTickOutcome::ReviewReady { .. }
    ));
    let prompt = &executor.prompts()[0];
    assert!(prompt.contains("Master question: please report the current blocker"));
    assert!(prompt.contains("Master constraint: do not change generated files"));
    assert!(prompt.contains("Master requested a checkpoint at the next safe point."));
    assert!(prompt.contains("Master requested submission at the next safe point."));
    let history = TaskRuntime::boot(&runtime_home, AgentId::new("master"))
        .expect("runtime")
        .task_history(&task_id)
        .expect("history");
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "TaskExecutionRecorded")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_cancel_does_not_publish_stale_result() {
    let runtime_home = temp_path("cancel-before-result");
    let workspace = temp_path("cancel-before-result-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(CancelDuringExecutionExecutor::new(runtime_home.clone()));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    executor.set_task_id(task_id.clone());

    assert_eq!(
        runner.run_once().expect("cancelled worker tick"),
        ProductionWorkerTickOutcome::Idle
    );
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let history = runtime.task_history(&task_id).expect("history");
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "TaskCancelled")
    );
    assert!(
        !history
            .iter()
            .any(|event| event.event_type == "TaskReviewSubmitted")
    );
    assert!(
        !history
            .iter()
            .any(|event| event.event_type == "TaskBlocked")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn production_worker_runner_paused_execution_cannot_publish_stale_success() {
    let runtime_home = temp_path("paused-stale-success");
    let workspace = temp_path("paused-stale-success-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(PauseDuringExecutionExecutor::new(runtime_home.clone()));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    executor.set_task_id(task_id.clone());

    assert_eq!(
        runner.run_once().expect("stale paused success tick"),
        ProductionWorkerTickOutcome::Idle
    );
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let task = runtime.query_task(&task_id).expect("task");
    assert_eq!(task.status, TaskStatus::Paused);
    let history = runtime.task_history(&task_id).expect("history");
    assert!(history.iter().any(|event| event.event_type == "TaskPaused"));
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

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[derive(Clone)]
struct PauseAwareExecutor {
    runtime_home: PathBuf,
    task_id: Arc<Mutex<Option<TaskId>>>,
    calls: Arc<AtomicUsize>,
    pause_token_observed: Arc<std::sync::atomic::AtomicBool>,
}

impl PauseAwareExecutor {
    fn new(runtime_home: PathBuf) -> Self {
        Self {
            runtime_home,
            task_id: Arc::new(Mutex::new(None)),
            calls: Arc::new(AtomicUsize::new(0)),
            pause_token_observed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn set_task_id(&self, task_id: TaskId) {
        *self.task_id.lock().expect("lock task id") = Some(task_id);
    }
}

impl WorkerTurnExecutor for PauseAwareExecutor {
    fn execute(
        &self,
        _selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<WorkerTurnExecution, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let task_id = self
            .task_id
            .lock()
            .expect("lock task id")
            .clone()
            .expect("task id set");
        let runtime =
            TaskRuntime::boot(&self.runtime_home, AgentId::new("master")).expect("task runtime");
        runtime
            .apply_worker_control(worker_control_request(
                &task_id,
                request
                    .turn_id
                    .as_str()
                    .strip_prefix("worker-turn-")
                    .expect("worker turn id carries execution id"),
                WorkerControlOp::Pause,
                "pause-during-live-execution",
            ))
            .expect("pause during execution");
        let cancel_token = request
            .cancel_token
            .expect("Worker runner must pass a live cancel token");
        for _ in 0..80 {
            if cancel_token.load(Ordering::SeqCst) {
                self.pause_token_observed.store(true, Ordering::Relaxed);
                return Err("live turn cancelled".to_owned());
            }
            thread::sleep(Duration::from_millis(25));
        }
        Err("pause token was not set before safe point timeout".to_owned())
    }
}

#[derive(Clone)]
struct HeartbeatFailureAwareExecutor {
    runtime_home: PathBuf,
    task_id: Arc<Mutex<Option<TaskId>>>,
    cancel_token_observed: Arc<std::sync::atomic::AtomicBool>,
}

impl HeartbeatFailureAwareExecutor {
    fn new(runtime_home: PathBuf) -> Self {
        Self {
            runtime_home,
            task_id: Arc::new(Mutex::new(None)),
            cancel_token_observed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn set_task_id(&self, task_id: TaskId) {
        *self.task_id.lock().expect("lock task id") = Some(task_id);
    }
}

impl WorkerTurnExecutor for HeartbeatFailureAwareExecutor {
    fn execute(
        &self,
        _selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
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
                watermark: test_watermark("heartbeat-failure-cancel"),
            })
            .expect("external cancellation makes next heartbeat fail");
        let cancel_token = request
            .cancel_token
            .expect("Worker runner must pass a live cancel token");
        for _ in 0..160 {
            if cancel_token.load(Ordering::SeqCst) {
                self.cancel_token_observed.store(true, Ordering::Relaxed);
                return Err("live turn cancelled after heartbeat failure".to_owned());
            }
            thread::sleep(Duration::from_millis(25));
        }
        Err("heartbeat failure did not trip the live cancel token".to_owned())
    }
}

#[test]
fn production_worker_runner_heartbeat_failure_trips_live_cancel_token() {
    let runtime_home = temp_path("heartbeat-failure-cancel-token");
    let workspace = temp_path("heartbeat-failure-cancel-token-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let executor = Arc::new(HeartbeatFailureAwareExecutor::new(runtime_home.clone()));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    executor.set_task_id(task_id.clone());

    let error = runner
        .run_once()
        .expect_err("heartbeat failure should stop the worker tick explicitly");

    assert!(matches!(
        error,
        ProductionWorkerRunnerError::BlockedFactPersistence(_)
            | ProductionWorkerRunnerError::Heartbeat(_)
    ));
    assert!(executor.cancel_token_observed.load(Ordering::Relaxed));
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let history = runtime.task_history(&task_id).expect("history");
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "TaskCancelled")
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

#[derive(Clone)]
struct PauseDuringExecutionExecutor {
    runtime_home: PathBuf,
    task_id: Arc<Mutex<Option<TaskId>>>,
}

impl PauseDuringExecutionExecutor {
    fn new(runtime_home: PathBuf) -> Self {
        Self {
            runtime_home,
            task_id: Arc::new(Mutex::new(None)),
        }
    }

    fn set_task_id(&self, task_id: TaskId) {
        *self.task_id.lock().expect("lock task id") = Some(task_id);
    }
}

impl WorkerTurnExecutor for PauseDuringExecutionExecutor {
    fn execute(
        &self,
        _selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
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
            .apply_worker_control(worker_control_request(
                &task_id,
                request
                    .turn_id
                    .as_str()
                    .strip_prefix("worker-turn-")
                    .expect("worker turn id carries execution id"),
                WorkerControlOp::Pause,
                "pause-during-execution",
            ))
            .expect("pause during execution");
        Ok(WorkerTurnExecution {
            status: TerminalStatus::Success,
            summary: "stale success after pause".to_owned(),
            turn_id: request.turn_id,
        })
    }
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
fn production_worker_runner_missing_symlink_leaf_reports_path_diagnostic() {
    let runtime_home = temp_path("missing-symlink-leaf-runtime");
    let base = temp_path("missing-symlink-leaf-base");
    let canonical_parent = base.join("Documents").join("workspace-parent");
    let symlink_parent = base.join("workspace-link");
    fs::create_dir_all(&canonical_parent).expect("canonical parent");
    let canonical_parent_resolved =
        fs::canonicalize(&canonical_parent).expect("canonical parent resolved");
    create_dir_symlink(&canonical_parent, &symlink_parent);
    let missing_workspace = symlink_parent.join("missing-workspace");
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
                && reason.contains("target_cwd_path_diagnostic")
                && reason.contains("exists=false")
                && reason.contains(&format!("nearest_existing=`{}`", symlink_parent.display()))
                && reason.contains(&format!("nearest_existing_canonical=`{}`", canonical_parent_resolved.display()))
                && reason.contains("missing_suffix=`missing-workspace`")
                && reason.contains(&format!("`{}` -> `{}`", symlink_parent.display(), canonical_parent.display()))
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);

    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let task = task_runtime
        .query_task(&expected_task_id)
        .expect("query task");
    assert_eq!(task.status, TaskStatus::Blocked);

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(base).expect("cleanup base");
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

#[test]
fn production_worker_runner_honors_preexisting_host_cancellation() {
    let runtime_home = temp_path("host-cancelled");
    let runner =
        ProductionWorkerRunner::from_selected_agent(selected_worker(), runtime_home.clone())
            .expect("worker runner");
    let cancelled = Arc::new(AtomicBool::new(true));
    assert_eq!(
        runner.current_agent_activity().expect("owner activity"),
        RuntimeAgentActivityProjection {
            status: RuntimeAgentActivityStatus::Idle,
            active_session_count: 0,
        }
    );

    runner
        .run_until(cancelled)
        .expect("cancelled worker lifetime closes cleanly");

    if runtime_home.exists() {
        fs::remove_dir_all(runtime_home).expect("cleanup");
    }
}

#[test]
fn production_worker_runner_projects_assigned_task_as_waiting_activity() {
    let runtime_home = temp_path("assigned-activity");
    let bootstrap =
        ProductionWorkerRunner::from_selected_agent(selected_worker(), runtime_home.clone())
            .expect("worker runner");
    let _task_id = seed_assigned_task(&runtime_home, None);
    drop(bootstrap);
    let runner =
        ProductionWorkerRunner::from_selected_agent(selected_worker(), runtime_home.clone())
            .expect("restarted worker runner");

    assert_eq!(
        runner.current_agent_activity().expect("assigned activity"),
        RuntimeAgentActivityProjection {
            status: RuntimeAgentActivityStatus::Waiting,
            active_session_count: 1,
        }
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[derive(Clone)]
struct HostCancellationAwareExecutor {
    started: Arc<AtomicBool>,
    cancel_token_observed: Arc<AtomicBool>,
}

impl WorkerTurnExecutor for HostCancellationAwareExecutor {
    fn execute(
        &self,
        _selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<WorkerTurnExecution, String> {
        self.started.store(true, Ordering::Release);
        let cancel_token = request
            .cancel_token
            .expect("Worker runner must pass a live cancel token");
        for _ in 0..160 {
            if cancel_token.load(Ordering::Acquire) {
                self.cancel_token_observed.store(true, Ordering::Release);
                return Err("live turn cancelled by Worker host shutdown".to_owned());
            }
            thread::sleep(Duration::from_millis(25));
        }
        Err("Worker host cancellation did not reach the live turn".to_owned())
    }
}

#[test]
fn production_worker_runner_host_cancellation_interrupts_active_task_before_exit() {
    let runtime_home = temp_path("host-cancelled-active-task");
    let workspace = temp_path("host-cancelled-active-task-workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let started = Arc::new(AtomicBool::new(false));
    let cancel_token_observed = Arc::new(AtomicBool::new(false));
    let executor = Arc::new(HostCancellationAwareExecutor {
        started: Arc::clone(&started),
        cancel_token_observed: Arc::clone(&cancel_token_observed),
    });
    let runner = test_runner(runtime_home.clone(), executor);
    let task_id = seed_assigned_task(&runtime_home, Some(&workspace));
    let cancel = Arc::new(AtomicBool::new(false));
    let runner_cancel = Arc::clone(&cancel);
    let handle = thread::spawn(move || runner.run_until(runner_cancel));

    for _ in 0..80 {
        if started.load(Ordering::Acquire) {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(started.load(Ordering::Acquire));
    cancel.store(true, Ordering::Release);
    handle
        .join()
        .expect("worker runner thread")
        .expect("cancelled worker runner closes cleanly");

    assert!(cancel_token_observed.load(Ordering::Acquire));
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    assert_eq!(
        runtime.query_task(&task_id).expect("task").status,
        TaskStatus::Interrupted
    );
    assert!(
        runtime
            .task_history(&task_id)
            .expect("history")
            .iter()
            .any(|event| event.event_type == "TaskInterrupted")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
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
            execution_profile: TaskExecutionProfile::Workspace,
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

fn seed_assigned_clean_search_task(runtime_home: &Path) -> TaskId {
    let task_runtime =
        TaskRuntime::boot(runtime_home, AgentId::new("master")).expect("task runtime");
    let task_id = TaskId::new(format!("task-clean-search-{}", now_unix_seconds()));
    task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(task_id.clone()),
            title: "clean search task".to_owned(),
            content: "search broad current web evidence".to_owned(),
            goal: "return sourced search conclusion".to_owned(),
            deliverables: vec!["search summary".to_owned()],
            acceptance: vec!["sources and gaps reported".to_owned()],
            priority: 80,
            target_cwd: None,
            execution_profile: TaskExecutionProfile::CleanSearch,
            dispatch: TaskDispatchRequest::Agent {
                agent_id: AgentId::new("worker"),
            },
            parent: TaskParentRef {
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            actor: worker_actor(&AgentId::new("master"), None),
            watermark: worker_watermark("seed-clean-search", "create"),
        })
        .expect("create clean search task");
    task_id
}

fn seed_assigned_sourced_search_task(runtime_home: &Path) -> TaskId {
    let task_runtime =
        TaskRuntime::boot(runtime_home, AgentId::new("master")).expect("task runtime");
    let task_id = TaskId::new(format!("task-sourced-search-{}", now_unix_seconds()));
    task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(task_id.clone()),
            title: "sourced search task".to_owned(),
            content: "search and verify current web evidence".to_owned(),
            goal: "return verified sourced conclusion".to_owned(),
            deliverables: vec!["verified sources summary".to_owned()],
            acceptance: vec!["sources verified through camo".to_owned()],
            priority: 80,
            target_cwd: None,
            execution_profile: TaskExecutionProfile::SourcedSearch,
            dispatch: TaskDispatchRequest::Agent {
                agent_id: AgentId::new("worker"),
            },
            parent: TaskParentRef {
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            actor: worker_actor(&AgentId::new("master"), None),
            watermark: worker_watermark("seed-sourced-search", "create"),
        })
        .expect("create sourced search task");
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

fn worker_control_request(
    task_id: &TaskId,
    execution_id: &str,
    op: WorkerControlOp,
    control_id: &str,
) -> WorkerControlRequest {
    WorkerControlRequest {
        control_id: Some(control_id.to_owned()),
        task_id: task_id.clone(),
        execution_id: execution_id.to_owned(),
        agent_id: AgentId::new("worker"),
        op,
        question: None,
        constraint: None,
        note: Some(control_id.to_owned()),
        actor: test_actor("master"),
        watermark: test_watermark(control_id),
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
            provider_id: "master-provider".to_owned(),
            fallback_provider_id: None,
            model_group_id: None,
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
            web_search: ProviderWebSearchMode::Auto,
            web_search_wire: ProviderWebSearchWire::WebSearch,
            auth_type: ProviderAuthType::ApiKey,
            auth_source: ProviderAuthSourceKind::Inline,
            api_key: "test-key".to_owned(),
        },
        fallback_provider: None,
        model_group_id: None,
        context_window_tokens: 128_000,
        compaction_threshold_tokens: 100_000,
        restart_required_on_change: true,
        relay_connection: None,
    }
}

fn selected_worker_openai_responses_search() -> SelectedAgentConfig {
    let mut selected = selected_worker();
    selected.provider.provider_type = ProviderType::OpenAi;
    selected.provider.protocol = ProviderProtocol::Responses;
    selected.provider.default_model = "gpt-5.5".to_owned();
    selected.provider.base_url = "https://openai.example.invalid".to_owned();
    selected
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
