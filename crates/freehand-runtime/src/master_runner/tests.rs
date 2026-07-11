use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_config::{
    AgentMode, ProviderAuthSourceKind, ProviderAuthType, ProviderProtocol, ProviderType,
    SelectedAgentConfig, SelectedProviderConfig,
};
use freehand_contracts::{AgentId, TurnId};
use freehand_task::{
    AgentCreateRequest, ExecutionFact, ExecutionFactKind, TaskActor, TaskAppendRequest,
    TaskClaimRequest, TaskCreateRequest, TaskDispatchRequest, TaskId, TaskMutationRequest,
    TaskParentRef, TaskReviewRejection, TaskRuntime, TaskStatus, TaskWatermark,
};

use super::*;

type MasterExecutorAction = dyn Fn(&LiveReasonTurnRequest) -> Result<String, String> + Send + Sync;

struct StubMasterExecutor {
    action: Arc<MasterExecutorAction>,
    calls: AtomicUsize,
}

impl StubMasterExecutor {
    fn new(
        action: impl Fn(&LiveReasonTurnRequest) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            action: Arc::new(action),
            calls: AtomicUsize::new(0),
        }
    }
}

impl MasterTurnExecutor for StubMasterExecutor {
    fn execute(
        &self,
        _selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
        _decision_boundary: LiveReasonTaskDecisionBoundary,
    ) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        (self.action)(&request)
    }
}

#[test]
fn production_master_runner_approves_and_closes_review_ready_task() {
    let runtime_home = temp_path("review-close");
    bootstrap_runner(&runtime_home);
    let task_id = seed_review_ready_task(&runtime_home);
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .approve_review(TaskMutationRequest {
                task_id: action_task_id.clone(),
                actor: test_actor("master"),
                watermark: test_watermark("approve"),
            })
            .map_err(to_string)?;
        runtime
            .close_task(TaskMutationRequest {
                task_id: action_task_id.clone(),
                actor: test_actor("master"),
                watermark: test_watermark("close"),
            })
            .map_err(to_string)?;
        Ok("approved and closed".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    let outcome = runner.run_once().expect("master tick");
    assert!(matches!(
        outcome,
        ProductionMasterTickOutcome::TaskAdvanced {
            task_id: ref outcome_task_id,
            from: TaskStatus::ReviewSubmitted,
            to: TaskStatus::Closed,
            ..
        } if outcome_task_id == &task_id
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        runner.run_once().expect("second tick"),
        ProductionMasterTickOutcome::Idle
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_rejects_review_with_requirements() {
    let runtime_home = temp_path("review-reject");
    bootstrap_runner(&runtime_home);
    let task_id = seed_review_ready_task(&runtime_home);
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .reject_review(TaskReviewRejection {
                task_id: action_task_id.clone(),
                reject_reason: "missing restart evidence".to_owned(),
                next_requirements: vec!["run restart recovery proof".to_owned()],
                actor: test_actor("master"),
                watermark: test_watermark("reject"),
            })
            .map_err(to_string)?;
        Ok("rejected with requirements".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor);

    assert!(matches!(
        runner.run_once().expect("master tick"),
        ProductionMasterTickOutcome::TaskAdvanced {
            from: TaskStatus::ReviewSubmitted,
            to: TaskStatus::Rejected,
            ..
        }
    ));
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let task = runtime.query_task(&task_id).expect("task");
    assert_eq!(
        task.review.reject_reason.as_deref(),
        Some("missing restart evidence")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_request_reuses_task_lifecycle_session_and_isolates_turns_by_event() {
    let runtime_home = temp_path("event-session-isolation");
    let task_id = seed_review_ready_task(&runtime_home);
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let task = runtime.query_task(&task_id).expect("task");
    let inbox = runtime
        .query_event_inbox(TaskEventInboxQuery {
            after_cursor: None,
            limit: usize::MAX,
        })
        .expect("event inbox");
    let first = inbox.events.first().expect("first event");
    let last = inbox.events.last().expect("last event");
    assert_ne!(first.event_id, last.event_id);

    let first_request =
        master_live_request(&runtime_home, "worker", &task, first, 0).expect("first request");
    let last_request =
        master_live_request(&runtime_home, "worker", &task, last, 0).expect("last request");

    assert_eq!(first_request.session_id, last_request.session_id);
    assert!(
        first_request
            .session_id
            .as_str()
            .contains(&sanitize_identifier(task_id.as_str()))
    );
    assert_ne!(first_request.turn_id, last_request.turn_id);
    assert_ne!(first_request.trace_id, last_request.trace_id);
    assert!(
        !first_request
            .turn_id
            .as_str()
            .contains(&sanitize_identifier(&last.event_id))
    );
    assert!(
        last_request
            .turn_id
            .as_str()
            .contains(&sanitize_identifier(&last.event_id))
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_retry_reuses_task_session_with_fresh_attempt_turn() {
    let runtime_home = temp_path("event-attempt-isolation");
    bootstrap_runner(&runtime_home);
    let task_id = seed_review_ready_task(&runtime_home);
    let request_keys = Arc::new(Mutex::new(Vec::new()));
    let observed_keys = Arc::clone(&request_keys);
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        observed_keys.lock().expect("request keys").push((
            request.session_id.as_str().to_owned(),
            request.turn_id.as_str().to_owned(),
            request.trace_id.as_str().to_owned(),
        ));
        if observed_keys.lock().expect("request keys").len() == 1 {
            return Ok("missing decision".to_owned());
        }
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .reject_review(TaskReviewRejection {
                task_id: action_task_id.clone(),
                reject_reason: "retry with isolated context".to_owned(),
                next_requirements: vec!["use fresh lifecycle attempt".to_owned()],
                actor: test_actor("master"),
                watermark: test_watermark("attempt-reject"),
            })
            .map_err(to_string)?;
        Ok("rejected".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor);

    assert!(matches!(
        runner.run_once().expect_err("first attempt must retry"),
        ProductionMasterRunnerError::MissingReviewDecision { .. }
    ));
    let restarted = test_runner(
        runtime_home.clone(),
        Arc::new(StubMasterExecutor::new({
            let request_keys = Arc::clone(&request_keys);
            let action_task_id = task_id.clone();
            move |request| {
                request_keys.lock().expect("request keys").push((
                    request.session_id.as_str().to_owned(),
                    request.turn_id.as_str().to_owned(),
                    request.trace_id.as_str().to_owned(),
                ));
                let runtime = TaskRuntime::boot(&request.runtime_home, AgentId::new("master"))
                    .map_err(to_string)?;
                runtime
                    .reject_review(TaskReviewRejection {
                        task_id: action_task_id.clone(),
                        reject_reason: "retry with isolated context".to_owned(),
                        next_requirements: vec!["use fresh lifecycle attempt".to_owned()],
                        actor: test_actor("master"),
                        watermark: test_watermark("attempt-restart-reject"),
                    })
                    .map_err(to_string)?;
                Ok("rejected".to_owned())
            }
        })),
    );
    assert!(matches!(
        restarted.run_once().expect("restart retry"),
        ProductionMasterTickOutcome::TaskAdvanced {
            to: TaskStatus::Rejected,
            ..
        }
    ));
    let request_keys = request_keys.lock().expect("request keys");
    assert_eq!(request_keys.len(), 2);
    assert_eq!(request_keys[0].0, request_keys[1].0);
    assert_ne!(request_keys[0].1, request_keys[1].1);
    assert_ne!(request_keys[0].2, request_keys[1].2);
    assert!(request_keys[0].1.contains("-attempt-0-decision"));
    assert!(request_keys[1].1.contains("-attempt-1-decision"));

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_retries_unresolved_review_event() {
    let runtime_home = temp_path("review-missing-decision");
    bootstrap_runner(&runtime_home);
    let task_id = seed_review_ready_task(&runtime_home);
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Ok("reviewed without task mutation".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    let first = runner.run_once().expect_err("missing decision must fail");
    assert!(matches!(
        first,
        ProductionMasterRunnerError::MissingReviewDecision { .. }
    ));
    let second = runner.run_once().expect_err("event must remain retryable");
    assert!(matches!(
        second,
        ProductionMasterRunnerError::MissingReviewDecision { .. }
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 2);
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    assert_eq!(
        runtime.query_task(&task_id).expect("task").status,
        TaskStatus::ReviewSubmitted
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_retries_approved_but_unclosed_review() {
    let runtime_home = temp_path("review-approved-unclosed");
    bootstrap_runner(&runtime_home);
    let task_id = seed_review_ready_task(&runtime_home);
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        let task = runtime.query_task(&action_task_id).map_err(to_string)?;
        if task.status == TaskStatus::ReviewSubmitted {
            runtime
                .approve_review(TaskMutationRequest {
                    task_id: action_task_id.clone(),
                    actor: test_actor("master"),
                    watermark: test_watermark("approve-only"),
                })
                .map_err(to_string)?;
        } else {
            runtime
                .close_task(TaskMutationRequest {
                    task_id: action_task_id.clone(),
                    actor: test_actor("master"),
                    watermark: test_watermark("close-retry"),
                })
                .map_err(to_string)?;
        }
        Ok("review lifecycle advanced".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    assert!(matches!(
        runner.run_once().expect_err("approved-only must retry"),
        ProductionMasterRunnerError::IncompleteReviewDecision { .. }
    ));
    assert!(matches!(
        runner.run_once().expect("retry closes task"),
        ProductionMasterTickOutcome::TaskAdvanced {
            from: TaskStatus::Approved,
            to: TaskStatus::Closed,
            ..
        }
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 2);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_requires_persisted_blocked_decision() {
    let runtime_home = temp_path("blocked-decision");
    bootstrap_runner(&runtime_home);
    let task_id = seed_blocked_task(&runtime_home);
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Ok("wait for external action".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    assert!(matches!(
        runner
            .run_once()
            .expect_err("prose-only blocked decision must fail"),
        ProductionMasterRunnerError::MissingBlockedDecision { .. }
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        TaskRuntime::boot(&runtime_home, AgentId::new("master"))
            .expect("runtime")
            .query_task(&task_id)
            .expect("task")
            .status,
        TaskStatus::Blocked
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_accepts_persisted_blocked_append_decision() {
    let runtime_home = temp_path("blocked-append-decision");
    bootstrap_runner(&runtime_home);
    let task_id = seed_blocked_task(&runtime_home);
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .append_task(TaskAppendRequest {
                task_id: action_task_id.clone(),
                note: "blocked_decision: provider access must be restored".to_owned(),
                actor: test_actor("master"),
                watermark: test_watermark("blocked-append"),
            })
            .map_err(to_string)?;
        Ok("blocked decision persisted".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor);

    assert!(matches!(
        runner.run_once().expect("blocked decision tick"),
        ProductionMasterTickOutcome::BlockedObserved {
            task_id: ref outcome_task_id,
            ..
        } if outcome_task_id == &task_id
    ));
    let history = TaskRuntime::boot(&runtime_home, AgentId::new("master"))
        .expect("runtime")
        .task_history(&task_id)
        .expect("history");
    assert_eq!(
        history.last().expect("last event").event_type,
        "TaskProgressed"
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_loop_retries_executor_failure_and_closes_same_event() {
    let runtime_home = temp_path("loop-executor-retry");
    bootstrap_runner(&runtime_home);
    let task_id = seed_review_ready_task(&runtime_home);
    let action_task_id = task_id.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let executor_cancel = Arc::clone(&cancel);
    let attempts = Arc::new(AtomicUsize::new(0));
    let executor_attempts = Arc::clone(&attempts);
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        let attempt = executor_attempts.fetch_add(1, Ordering::Relaxed);
        if attempt == 0 {
            return Err("provider temporarily unavailable".to_owned());
        }
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .approve_review(TaskMutationRequest {
                task_id: action_task_id.clone(),
                actor: test_actor("master"),
                watermark: test_watermark("retry-approve"),
            })
            .map_err(to_string)?;
        runtime
            .close_task(TaskMutationRequest {
                task_id: action_task_id.clone(),
                actor: test_actor("master"),
                watermark: test_watermark("retry-close"),
            })
            .map_err(to_string)?;
        executor_cancel.store(true, Ordering::Release);
        Ok("closed after provider recovery".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor);

    runner
        .run_until_with_policy(
            Arc::clone(&cancel),
            MasterLoopRetryPolicy {
                initial_backoff: Duration::ZERO,
                max_backoff: Duration::ZERO,
            },
        )
        .expect("retryable executor failure must not stop loop");
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert_eq!(
        TaskRuntime::boot(&runtime_home, AgentId::new("master"))
            .expect("runtime")
            .query_task(&task_id)
            .expect("task")
            .status,
        TaskStatus::Closed
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_loop_retries_missing_decision_and_keeps_cursor() {
    let runtime_home = temp_path("loop-missing-decision-retry");
    bootstrap_runner(&runtime_home);
    let task_id = seed_review_ready_task(&runtime_home);
    let action_task_id = task_id.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let executor_cancel = Arc::clone(&cancel);
    let attempts = Arc::new(AtomicUsize::new(0));
    let executor_attempts = Arc::clone(&attempts);
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        let attempt = executor_attempts.fetch_add(1, Ordering::Relaxed);
        if attempt == 0 {
            return Ok("prose without task mutation".to_owned());
        }
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .reject_review(TaskReviewRejection {
                task_id: action_task_id.clone(),
                reject_reason: "missing required evidence".to_owned(),
                next_requirements: vec!["add required evidence".to_owned()],
                actor: test_actor("master"),
                watermark: test_watermark("retry-reject"),
            })
            .map_err(to_string)?;
        executor_cancel.store(true, Ordering::Release);
        Ok("rejected after retry".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor);

    runner
        .run_until_with_policy(
            Arc::clone(&cancel),
            MasterLoopRetryPolicy {
                initial_backoff: Duration::ZERO,
                max_backoff: Duration::ZERO,
            },
        )
        .expect("missing decision must remain retryable");
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert_eq!(
        TaskRuntime::boot(&runtime_home, AgentId::new("master"))
            .expect("runtime")
            .query_task(&task_id)
            .expect("task")
            .status,
        TaskStatus::Rejected
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_loop_stops_on_corrupt_cursor_state() {
    let runtime_home = temp_path("loop-fatal-state");
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("executor must not run".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    let state_path = runner.state_path();
    fs::create_dir_all(state_path.parent().expect("state parent")).expect("state directory");
    fs::write(&state_path, "{not-json").expect("corrupt state");

    let error = runner
        .run_until_with_policy(
            Arc::new(AtomicBool::new(false)),
            MasterLoopRetryPolicy {
                initial_backoff: Duration::ZERO,
                max_backoff: Duration::ZERO,
            },
        )
        .expect_err("corrupt owner truth must stop the loop");
    assert!(matches!(error, ProductionMasterRunnerError::State(_)));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

fn test_runner(
    runtime_home: PathBuf,
    executor: Arc<dyn MasterTurnExecutor>,
) -> ProductionMasterRunner {
    ProductionMasterRunner::from_selected_agent_with_executor(
        selected_master(),
        runtime_home,
        executor,
    )
    .expect("master runner")
}

fn bootstrap_runner(runtime_home: &Path) {
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("bootstrap must not execute".to_owned())
    }));
    let runner = test_runner(runtime_home.to_path_buf(), executor.clone());
    assert_eq!(
        runner.run_once().expect("bootstrap tick"),
        ProductionMasterTickOutcome::Idle
    );
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);
}

fn seed_review_ready_task(runtime_home: &Path) -> TaskId {
    let runtime = TaskRuntime::boot(runtime_home, AgentId::new("master")).expect("runtime");
    runtime
        .create_agent(AgentCreateRequest {
            agent_id: AgentId::new("worker"),
            capabilities: vec!["workspace".to_owned(), "shell".to_owned()],
            actor: test_actor("master"),
            watermark: test_watermark("create-worker"),
        })
        .expect("create worker");
    let task_id = TaskId::new(format!("task-{}", now_unix_nanos()));
    runtime
        .create_task(TaskCreateRequest {
            task_id: Some(task_id.clone()),
            title: "review task".to_owned(),
            content: "produce verified output".to_owned(),
            goal: "complete lifecycle".to_owned(),
            deliverables: vec!["result.md".to_owned()],
            acceptance: vec!["evidence exists".to_owned()],
            priority: 90,
            target_cwd: Some(runtime_home.display().to_string()),
            dispatch: TaskDispatchRequest::Agent {
                agent_id: AgentId::new("worker"),
            },
            parent: TaskParentRef {
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            actor: test_actor("master"),
            watermark: test_watermark("create-task"),
        })
        .expect("create task");
    let execution_id = format!("exec-{}", now_unix_nanos());
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: AgentId::new("worker"),
            execution_id: execution_id.clone(),
            ttl_seconds: 300,
            actor: test_actor("worker"),
            watermark: test_watermark("claim"),
        })
        .expect("claim");
    runtime
        .apply_execution_fact(ExecutionFact {
            execution_id,
            task_id: task_id.clone(),
            agent_id: AgentId::new("worker"),
            turn_id: Some(TurnId::new("worker-turn-review")),
            occurred_at: now_unix_seconds(),
            kind: ExecutionFactKind::ReviewReady {
                summary: "implementation complete".to_owned(),
                deliverables: vec!["result.md".to_owned()],
                evidence: vec!["tests passed".to_owned()],
            },
            watermark: test_watermark("review-ready"),
        })
        .expect("review ready");
    task_id
}

fn seed_blocked_task(runtime_home: &Path) -> TaskId {
    let task_id = seed_review_ready_task(runtime_home);
    let runtime = TaskRuntime::boot(runtime_home, AgentId::new("master")).expect("runtime");
    let review_task = runtime.query_task(&task_id).expect("review task");
    runtime
        .reject_review(TaskReviewRejection {
            task_id: task_id.clone(),
            reject_reason: "replace review fixture with blocked execution".to_owned(),
            next_requirements: vec!["retry fixture".to_owned()],
            actor: test_actor("master"),
            watermark: test_watermark("fixture-reject"),
        })
        .expect("reject fixture");
    runtime
        .assign_task(freehand_task::TaskAssignRequest {
            task_id: task_id.clone(),
            agent_id: AgentId::new("worker"),
            actor: test_actor("master"),
            watermark: test_watermark("fixture-reassign"),
        })
        .expect("reassign fixture");
    let execution_id = format!("exec-blocked-{}", now_unix_nanos());
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: AgentId::new("worker"),
            execution_id: execution_id.clone(),
            ttl_seconds: 300,
            actor: test_actor("worker"),
            watermark: test_watermark("fixture-claim"),
        })
        .expect("claim fixture");
    runtime
        .apply_execution_fact(ExecutionFact {
            execution_id,
            task_id: task_id.clone(),
            agent_id: AgentId::new("worker"),
            turn_id: Some(TurnId::new("worker-turn-blocked")),
            occurred_at: now_unix_seconds(),
            kind: ExecutionFactKind::Blocked {
                reason: "provider unavailable".to_owned(),
                evidence: vec!["provider_error=timeout".to_owned()],
            },
            watermark: test_watermark("fixture-blocked"),
        })
        .expect("blocked fixture");
    assert_eq!(review_task.assignee.unwrap().agent_id.as_str(), "worker");
    task_id
}

fn selected_master() -> SelectedAgentConfig {
    SelectedAgentConfig {
        name: "master".to_owned(),
        mode: AgentMode::Master,
        node_id: "master-node".to_owned(),
        paired_agent_name: "worker".to_owned(),
        paired_agent_mode: AgentMode::Slave,
        paired_node_id: "worker-node".to_owned(),
        paired_allowed_pair_ip: None,
        paired_pair_token_env: "FREEHAND_PAIR_TOKEN_WORKER".to_owned(),
        allowed_pair_ip: None,
        pair_token_env: "FREEHAND_PAIR_TOKEN_MASTER".to_owned(),
        pair_token: "pair-token".to_owned(),
        provider: SelectedProviderConfig {
            id: "master-provider".to_owned(),
            provider_type: ProviderType::Anthropic,
            protocol: ProviderProtocol::Messages,
            base_url: "https://example.invalid".to_owned(),
            default_model: "master-model".to_owned(),
            auth_type: ProviderAuthType::ApiKey,
            auth_source: ProviderAuthSourceKind::Inline,
            api_key: "test-key".to_owned(),
        },
        restart_required_on_change: true,
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

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}

fn now_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos()
}

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "freehand-production-master-runner-{label}-{}",
        now_unix_nanos()
    ))
}

fn to_string(error: impl ToString) -> String {
    error.to_string()
}
