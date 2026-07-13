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
use freehand_contracts::{AgentId, FeatureId, SessionId, TerminalStatus, TraceId, TurnId};
use freehand_reason::{ReasonTurnEngine, SessionHistory, TurnStartInput};
use freehand_task::{
    AgentCreateRequest, ExecutionFact, ExecutionFactKind, TaskActor, TaskAppendRequest,
    TaskClaimRequest, TaskCreateRequest, TaskDispatchRequest, TaskId, TaskMutationRequest,
    TaskParentRef, TaskReviewRejection, TaskRuntime, TaskStatus, TaskWatermark,
};

use super::*;
use crate::{TimerRepeatRule, TimerSchedule, TimerStore};

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

    fn execute_timer(
        &self,
        _selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        (self.action)(&request)
    }

    fn execute_parent_evaluation(
        &self,
        _selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
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

#[test]
fn production_master_runner_fires_due_timer_without_task_truth() {
    let runtime_home = temp_path("timer-due");
    bootstrap_runner(&runtime_home);
    let store = TimerStore::new(&runtime_home, &AgentId::new("master"));
    let timer = TimerSchedule {
        schema_version: 1,
        timer_id: "timer-due-proof".to_owned(),
        agent_id: AgentId::new("master"),
        status: "active".to_owned(),
        reason: "check worker progress".to_owned(),
        prompt: "Read TaskBoard and continue any pending Master work.".to_owned(),
        next_due_at: now_unix_seconds().saturating_sub(1),
        created_at: now_unix_seconds().saturating_sub(10),
        updated_at: now_unix_seconds().saturating_sub(10),
        fired_count: 0,
        max_runs: 1,
        repeat: None,
        source_session_id: None,
        source_turn_id: None,
        source_trace_id: None,
    };
    store.upsert_schedule(timer).expect("schedule timer");
    let observed_prompt = Arc::new(Mutex::new(String::new()));
    let prompt_out = Arc::clone(&observed_prompt);
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        *prompt_out.lock().expect("prompt lock") = request.prompt.clone();
        Ok("timer wakeup complete".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    assert!(matches!(
        runner.run_once().expect("timer tick"),
        ProductionMasterTickOutcome::TimerFired {
            ref timer_id,
            ..
        } if timer_id == "timer-due-proof"
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);
    assert!(
        observed_prompt
            .lock()
            .expect("prompt lock")
            .contains("Read TaskBoard and continue")
    );
    let schedules = store.active_schedules().expect("active schedules");
    assert!(schedules.is_empty(), "one-shot timer must complete");
    let task_runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    assert!(
        task_runtime
            .list_tasks(Default::default())
            .expect("list tasks")
            .is_empty(),
        "timer must not create task truth"
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_reschedules_recurring_timer_until_max_runs() {
    let runtime_home = temp_path("timer-repeat");
    bootstrap_runner(&runtime_home);
    let store = TimerStore::new(&runtime_home, &AgentId::new("master"));
    let timer = TimerSchedule {
        schema_version: 1,
        timer_id: "timer-repeat-proof".to_owned(),
        agent_id: AgentId::new("master"),
        status: "active".to_owned(),
        reason: "repeat check".to_owned(),
        prompt: "Check the recurring condition.".to_owned(),
        next_due_at: now_unix_seconds().saturating_sub(1),
        created_at: now_unix_seconds().saturating_sub(10),
        updated_at: now_unix_seconds().saturating_sub(10),
        fired_count: 0,
        max_runs: 2,
        repeat: Some(TimerRepeatRule::Interval {
            interval_seconds: 60,
            max_runs: Some(2),
        }),
        source_session_id: None,
        source_turn_id: None,
        source_trace_id: None,
    };
    store.upsert_schedule(timer).expect("schedule timer");
    let executor = Arc::new(StubMasterExecutor::new(|_| Ok("timer fired".to_owned())));
    let runner = test_runner(runtime_home.clone(), executor);

    assert!(matches!(
        runner.run_once().expect("first timer tick"),
        ProductionMasterTickOutcome::TimerFired { .. }
    ));
    let active = store.active_schedules().expect("active schedules");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].fired_count, 1);
    assert_eq!(active[0].status, "active");
    assert!(active[0].next_due_at > now_unix_seconds());

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_releases_due_timer_after_wakeup_failure() {
    let runtime_home = temp_path("timer-failure-release");
    bootstrap_runner(&runtime_home);
    let store = TimerStore::new(&runtime_home, &AgentId::new("master"));
    let timer = TimerSchedule {
        schema_version: 1,
        timer_id: "timer-failure-proof".to_owned(),
        agent_id: AgentId::new("master"),
        status: "active".to_owned(),
        reason: "retry failed wakeup".to_owned(),
        prompt: "Retry the internal wakeup after provider recovery.".to_owned(),
        next_due_at: now_unix_seconds().saturating_sub(1),
        created_at: now_unix_seconds().saturating_sub(10),
        updated_at: now_unix_seconds().saturating_sub(10),
        fired_count: 0,
        max_runs: 1,
        repeat: None,
        source_session_id: None,
        source_turn_id: None,
        source_trace_id: None,
    };
    store.upsert_schedule(timer).expect("schedule timer");
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("provider temporarily unavailable".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor);

    let error = runner
        .run_once()
        .expect_err("timer wakeup failure must surface");
    assert!(matches!(error, ProductionMasterRunnerError::Execution(_)));
    let active = store.active_schedules().expect("active schedules");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].timer_id, "timer-failure-proof");
    assert_eq!(active[0].status, "active");
    assert_eq!(active[0].fired_count, 0);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_prioritizes_task_events_over_due_timer_failure() {
    let runtime_home = temp_path("task-before-failed-timer");
    bootstrap_runner(&runtime_home);
    let store = TimerStore::new(&runtime_home, &AgentId::new("master"));
    let timer = TimerSchedule {
        schema_version: 1,
        timer_id: "timer-must-not-starve-review".to_owned(),
        agent_id: AgentId::new("master"),
        status: "active".to_owned(),
        reason: "stale timer provider failure".to_owned(),
        prompt: "This timer would fail if it ran before review handling.".to_owned(),
        next_due_at: now_unix_seconds().saturating_sub(1),
        created_at: now_unix_seconds().saturating_sub(10),
        updated_at: now_unix_seconds().saturating_sub(10),
        fired_count: 0,
        max_runs: 1,
        repeat: None,
        source_session_id: None,
        source_turn_id: None,
        source_trace_id: None,
    };
    store.upsert_schedule(timer).expect("schedule timer");
    let task_id = seed_review_ready_task(&runtime_home);
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        if request
            .prompt
            .contains("production Master resumed by an internal timer")
        {
            return Err("timer provider unavailable".to_owned());
        }
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .approve_review(TaskMutationRequest {
                task_id: action_task_id.clone(),
                actor: test_actor("master"),
                watermark: test_watermark("task-priority-approve"),
            })
            .map_err(to_string)?;
        runtime
            .close_task(TaskMutationRequest {
                task_id: action_task_id.clone(),
                actor: test_actor("master"),
                watermark: test_watermark("task-priority-close"),
            })
            .map_err(to_string)?;
        Ok("review closed before timer".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor);

    assert!(matches!(
        runner.run_once().expect("task event must run before timer"),
        ProductionMasterTickOutcome::TaskAdvanced {
            task_id: ref outcome_task_id,
            from: TaskStatus::ReviewSubmitted,
            to: TaskStatus::Closed,
            ..
        } if outcome_task_id == &task_id
    ));
    assert_eq!(
        TaskRuntime::boot(&runtime_home, AgentId::new("master"))
            .expect("runtime")
            .query_task(&task_id)
            .expect("task")
            .status,
        TaskStatus::Closed
    );
    let active = store.active_schedules().expect("active schedules");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].timer_id, "timer-must-not-starve-review");
    assert_eq!(active[0].fired_count, 0);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_auto_appends_blocked_decision_after_retry_cap() {
    let runtime_home = temp_path("blocked-auto-append-after-retry");
    bootstrap_runner(&runtime_home);
    let task_id = seed_blocked_task(&runtime_home);
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("provider unavailable for blocked decision".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    for _ in 0..MASTER_BLOCKED_DECISION_AUTO_APPEND_ATTEMPTS {
        assert!(matches!(
            runner
                .run_once()
                .expect_err("blocked decision provider failure"),
            ProductionMasterRunnerError::Execution(_)
        ));
    }
    assert!(matches!(
        runner.run_once().expect("retry cap appends blocked decision"),
        ProductionMasterTickOutcome::BlockedObserved {
            task_id: ref outcome_task_id,
            ..
        } if outcome_task_id == &task_id
    ));
    assert_eq!(
        executor.calls.load(Ordering::Relaxed),
        MASTER_BLOCKED_DECISION_AUTO_APPEND_ATTEMPTS as usize
    );
    let history = TaskRuntime::boot(&runtime_home, AgentId::new("master"))
        .expect("runtime")
        .task_history(&task_id)
        .expect("history");
    let last = history.last().expect("last event");
    assert_eq!(last.event_type, "TaskProgressed");
    assert_eq!(last.actor.agent_id.as_str(), "master");
    assert!(
        last.payload
            .to_string()
            .contains("Master lifecycle provider remained unavailable")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_evaluates_closed_children_against_parent_goal() {
    let runtime_home = temp_path("parent-evaluation");
    bootstrap_runner(&runtime_home);
    let parent_session_id = SessionId::new("parent-session-evaluation");
    persist_parent_user_objective(
        &runtime_home,
        &parent_session_id,
        "Overall goal: deliver a verified three-part implementation, not only three summaries.",
    );
    persist_parent_internal_repair_turn(
        &runtime_home,
        &parent_session_id,
        "Your Freehand completion schema was rejected. Internal repair text is not a user goal.",
    );
    let child_ids = seed_parent_children(
        &runtime_home,
        &parent_session_id,
        &[("alpha", true), ("beta", true), ("gamma", true)],
    );
    let observed_request = Arc::new(Mutex::new(None::<LiveReasonTurnRequest>));
    let request_out = Arc::clone(&observed_request);
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        *request_out.lock().expect("request lock") = Some(request.clone());
        Ok("all child results evaluated against the overall goal".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    let outcome = runner.run_once().expect("parent evaluation tick");
    assert!(matches!(
        outcome,
        ProductionMasterTickOutcome::ParentEvaluated {
            parent_session_id: ref outcome_parent,
            evaluated_child_task_ids: ref outcome_children,
            ref summary,
        } if outcome_parent == &parent_session_id
            && outcome_children == &child_ids
            && summary == "all child results evaluated against the overall goal"
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);
    let request = observed_request
        .lock()
        .expect("request lock")
        .clone()
        .expect("evaluation request");
    assert_eq!(request.session_id, parent_session_id);
    assert!(request.turn_id.as_str().starts_with("runtime-turn-"));
    assert!(request.prompt.contains("<freehand_parent_evaluation id=\""));
    assert!(request.prompt.contains(
        "Overall goal: deliver a verified three-part implementation, not only three summaries."
    ));
    assert!(
        !request
            .prompt
            .contains("Internal repair text is not a user goal")
    );
    assert!(request.prompt.contains("overall-goal evaluation turn"));
    assert!(request.prompt.contains("Do not merely summarize"));
    for name in ["alpha", "beta", "gamma"] {
        assert!(request.prompt.contains(&format!("{name} review summary")));
        assert!(request.prompt.contains(&format!("{name}.md")));
        assert!(request.prompt.contains(&format!("{name} evidence")));
        assert!(request.prompt.contains(&format!("complete {name} child")));
    }
    assert!(
        request
            .prompt
            .contains("Completed subtask and accepted review truth")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_does_not_evaluate_while_sibling_open() {
    let runtime_home = temp_path("parent-evaluation-open-sibling");
    bootstrap_runner(&runtime_home);
    let parent_session_id = SessionId::new("parent-session-open-sibling");
    persist_parent_user_objective(
        &runtime_home,
        &parent_session_id,
        "Overall goal requires both alpha and beta.",
    );
    seed_parent_children(
        &runtime_home,
        &parent_session_id,
        &[("alpha", true), ("beta", false)],
    );
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("parent evaluation must not execute".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    assert_eq!(
        runner.run_once().expect("open sibling tick"),
        ProductionMasterTickOutcome::Idle
    );
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_rejects_parent_evaluation_without_persisted_goal_truth() {
    let runtime_home = temp_path("parent-evaluation-missing-goal");
    bootstrap_runner(&runtime_home);
    let parent_session_id = SessionId::new("parent-session-missing-goal");
    seed_parent_children(
        &runtime_home,
        &parent_session_id,
        &[("alpha", true), ("beta", true)],
    );
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("evaluation must not execute without parent goal truth".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    let error = runner
        .run_once()
        .expect_err("missing parent goal truth must fail explicitly");
    assert!(matches!(
        error,
        ProductionMasterRunnerError::State(ref message)
            if message.contains("has no persisted user objective truth")
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_parent_evaluation_is_idempotent_on_event_replay() {
    let runtime_home = temp_path("parent-evaluation-idempotent");
    bootstrap_runner(&runtime_home);
    let parent_session_id = SessionId::new("parent-session-idempotent");
    persist_parent_user_objective(
        &runtime_home,
        &parent_session_id,
        "Overall goal may require another task after alpha and beta.",
    );
    seed_parent_children(
        &runtime_home,
        &parent_session_id,
        &[("alpha", true), ("beta", true)],
    );
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Ok("parent evaluation complete".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());
    assert!(matches!(
        runner.run_once().expect("initial evaluation"),
        ProductionMasterTickOutcome::ParentEvaluated { .. }
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);

    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let mut children = task_runtime
        .query_task_board(TaskBoardQuery {
            status: None,
            assignee: None,
            include_terminal: true,
        })
        .expect("task board")
        .tasks
        .into_iter()
        .filter(|task| task.parent.session_id.as_ref() == Some(&parent_session_id))
        .collect::<Vec<_>>();
    children.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let evaluation_key = parent_evaluation_key(&parent_session_id, &children);
    let evaluation_marker = parent_evaluation_marker(&evaluation_key);
    persist_parent_evaluation_turn(
        &runtime_home,
        &parent_session_id,
        &TurnId::new("runtime-turn-2"),
        &evaluation_marker,
        TerminalStatus::ToolPending,
        "persisted parent evaluation created next work",
    );
    let state_path = runner.state_path();
    let mut state: MasterLoopState =
        serde_json::from_str(&fs::read_to_string(&state_path).expect("read state"))
            .expect("parse state");
    state.cursor = None;
    state.completed_parent_evaluations.clear();
    fs::write(
        &state_path,
        serde_json::to_string_pretty(&state).expect("render state"),
    )
    .expect("rewind cursor");
    let restarted = test_runner(runtime_home.clone(), executor.clone());
    assert!(matches!(
        restarted.run_once().expect("replayed event tick"),
        ProductionMasterTickOutcome::ParentEvaluated {
            ref summary,
            ..
        } if summary == "persisted parent evaluation created next work"
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_parent_evaluation_can_create_next_round_task() {
    let runtime_home = temp_path("parent-evaluation-next-round");
    bootstrap_runner(&runtime_home);
    let parent_session_id = SessionId::new("parent-session-next-round");
    persist_parent_user_objective(
        &runtime_home,
        &parent_session_id,
        "Overall goal requires an integrated report after alpha and beta.",
    );
    seed_parent_children(
        &runtime_home,
        &parent_session_id,
        &[("alpha", true), ("beta", true)],
    );
    let next_task_id = TaskId::new("task-parent-next-round-integration");
    let action_task_id = next_task_id.clone();
    let action_parent_session_id = parent_session_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        if !request
            .prompt
            .contains("create and assign the next required child tasks")
        {
            return Err("parent evaluation prompt forbids next-round work".to_owned());
        }
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .create_task(TaskCreateRequest {
                task_id: Some(action_task_id.clone()),
                title: "integrate accepted results".to_owned(),
                content: "integrate alpha and beta into the requested final artifact".to_owned(),
                goal: "close the remaining overall-goal integration gap".to_owned(),
                deliverables: vec!["integrated-report.md".to_owned()],
                acceptance: vec!["report proves alpha and beta are integrated".to_owned()],
                priority: 95,
                target_cwd: Some(request.runtime_home.display().to_string()),
                dispatch: TaskDispatchRequest::Agent {
                    agent_id: AgentId::new("worker"),
                },
                parent: TaskParentRef {
                    session_id: Some(action_parent_session_id.clone()),
                    turn_id: Some(request.turn_id.clone()),
                    trace_id: Some(request.trace_id.clone()),
                },
                actor: test_actor("master"),
                watermark: test_watermark("parent-evaluation-next-round"),
            })
            .map_err(to_string)?;
        Ok("overall goal incomplete; integration task created".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor);

    assert!(matches!(
        runner.run_once().expect("next-round evaluation"),
        ProductionMasterTickOutcome::ParentEvaluated {
            ref summary,
            ..
        } if summary == "overall goal incomplete; integration task created"
    ));
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("task runtime");
    let next_task = runtime.query_task(&next_task_id).expect("next-round task");
    assert_eq!(next_task.status, TaskStatus::Assigned);
    assert_eq!(
        next_task.parent.session_id.as_ref(),
        Some(&parent_session_id)
    );

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

fn seed_parent_children(
    runtime_home: &Path,
    parent_session_id: &SessionId,
    children: &[(&str, bool)],
) -> Vec<TaskId> {
    let runtime = TaskRuntime::boot(runtime_home, AgentId::new("master")).expect("runtime");
    runtime
        .create_agent(AgentCreateRequest {
            agent_id: AgentId::new("worker"),
            capabilities: vec!["workspace".to_owned()],
            actor: test_actor("master"),
            watermark: test_watermark("parent-create-worker"),
        })
        .expect("create worker");
    let mut task_ids = Vec::new();
    for (index, (name, close)) in children.iter().enumerate() {
        let task_id = TaskId::new(format!(
            "task-parent-{}-{name}",
            sanitize_identifier(parent_session_id.as_str())
        ));
        runtime
            .create_task(TaskCreateRequest {
                task_id: Some(task_id.clone()),
                title: format!("{name} child"),
                content: format!("produce {name} result"),
                goal: format!("complete {name} child"),
                deliverables: vec![format!("{name}.md")],
                acceptance: vec![format!("{name} evidence")],
                priority: 100 - index as i64,
                target_cwd: Some(runtime_home.display().to_string()),
                dispatch: TaskDispatchRequest::Agent {
                    agent_id: AgentId::new("worker"),
                },
                parent: TaskParentRef {
                    session_id: Some(parent_session_id.clone()),
                    turn_id: Some(TurnId::new("runtime-turn-parent")),
                    trace_id: None,
                },
                actor: test_actor("master"),
                watermark: test_watermark("parent-create-task"),
            })
            .expect("create child task");
        if *close {
            let execution_id = format!("exec-parent-{name}-{}", now_unix_nanos());
            runtime
                .claim_next_task(TaskClaimRequest {
                    agent_id: AgentId::new("worker"),
                    execution_id: execution_id.clone(),
                    ttl_seconds: 300,
                    actor: test_actor("worker"),
                    watermark: test_watermark("parent-claim"),
                })
                .expect("claim child");
            runtime
                .apply_execution_fact(ExecutionFact {
                    execution_id,
                    task_id: task_id.clone(),
                    agent_id: AgentId::new("worker"),
                    turn_id: Some(TurnId::new(format!("worker-turn-{name}"))),
                    occurred_at: now_unix_seconds(),
                    kind: ExecutionFactKind::ReviewReady {
                        summary: format!("{name} review summary"),
                        deliverables: vec![format!("{name}.md")],
                        evidence: vec![format!("{name} evidence")],
                    },
                    watermark: test_watermark("parent-review-ready"),
                })
                .expect("review child");
            runtime
                .approve_review(TaskMutationRequest {
                    task_id: task_id.clone(),
                    actor: test_actor("master"),
                    watermark: test_watermark("parent-approve"),
                })
                .expect("approve child");
            runtime
                .close_task(TaskMutationRequest {
                    task_id: task_id.clone(),
                    actor: test_actor("master"),
                    watermark: test_watermark("parent-close"),
                })
                .expect("close child");
        }
        task_ids.push(task_id);
    }
    task_ids.sort();
    task_ids
}

fn persist_parent_evaluation_turn(
    runtime_home: &Path,
    parent_session_id: &SessionId,
    turn_id: &TurnId,
    evaluation_marker: &str,
    terminal_status: TerminalStatus,
    summary: &str,
) {
    let mut history =
        SessionHistory::new(parent_session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: parent_session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: TraceId::new(format!("trace-{}", turn_id.as_str())),
                feature_id: FeatureId::new("runtime.master-worker-loop"),
                agent_id: AgentId::new("master"),
                user_text: format!(
                    "<freehand_parent_evaluation id=\"{evaluation_marker}\">\ninternal parent evaluation"
                ),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "master-model".to_owned(),
            },
        )
        .expect("start turn");
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), AgentId::new("master"));
    persistence
        .record_turn_started(&history, &turn, 0)
        .expect("persist turn start");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: parent_session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: FeatureId::new("runtime.master-worker-loop"),
        agent_id: AgentId::new("master"),
        status: terminal_status,
        summary: summary.to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist turn close");
}

fn persist_parent_user_objective(
    runtime_home: &Path,
    parent_session_id: &SessionId,
    objective: &str,
) {
    let mut history =
        SessionHistory::new(parent_session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: parent_session_id.clone(),
                turn_id: TurnId::new("runtime-turn-1"),
                trace_id: TraceId::new("parent-objective-trace"),
                feature_id: FeatureId::new("reason.turn"),
                agent_id: AgentId::new("master"),
                user_text: objective.to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "master-model".to_owned(),
            },
        )
        .expect("start objective turn");
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), AgentId::new("master"));
    persistence
        .record_turn_started(&history, &turn, 0)
        .expect("persist objective start");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: parent_session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: FeatureId::new("reason.turn"),
        agent_id: AgentId::new("master"),
        status: TerminalStatus::Blocked,
        summary: "waiting for delegated child work".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist objective close");
}

fn persist_parent_internal_repair_turn(
    runtime_home: &Path,
    parent_session_id: &SessionId,
    repair_text: &str,
) {
    let mut history =
        SessionHistory::new(parent_session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: parent_session_id.clone(),
                turn_id: TurnId::new("runtime-turn-1-r2"),
                trace_id: TraceId::new("parent-objective-repair-trace"),
                feature_id: FeatureId::new("reason.turn"),
                agent_id: AgentId::new("master"),
                user_text: repair_text.to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "master-model".to_owned(),
            },
        )
        .expect("start repair turn");
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), AgentId::new("master"));
    persistence
        .record_turn_started(&history, &turn, 0)
        .expect("persist repair start");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: parent_session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: FeatureId::new("reason.turn"),
        agent_id: AgentId::new("master"),
        status: TerminalStatus::Blocked,
        summary: "internal repair exhausted".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist repair close");
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
        fallback_provider: None,
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
